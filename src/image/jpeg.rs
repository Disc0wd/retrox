// ============================================================
// RetroX JPEG Decoder
// Implements baseline JPEG (ISO/IEC 10918-1).
// Supports: YCbCr (3-component) and Grayscale (1-component).
// No progressive, no lossless, no arithmetic coding.
// Zero external dependencies.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use super::{Image, ImageError};

pub fn decode_jpeg(data: &[u8]) -> Result<Image, ImageError> {
    JpegDecoder::new(data)?.decode()
}

// ─── Markers ───────────────────────────────────────────────

const M_SOI:  u8 = 0xD8;
const M_EOI:  u8 = 0xD9;
const M_SOF0: u8 = 0xC0; // Baseline DCT
const M_DHT:  u8 = 0xC4;
const M_DQT:  u8 = 0xDB;
const M_SOS:  u8 = 0xDA;
const M_DRI:  u8 = 0xDD;

// ─── Zigzag order ──────────────────────────────────────────

const ZIGZAG: [u8; 64] = [
     0, 1, 8,16, 9, 2, 3,10,
    17,24,32,25,18,11, 4, 5,
    12,19,26,33,40,48,41,34,
    27,20,13, 6, 7,14,21,28,
    35,42,49,56,57,50,43,36,
    29,22,15,23,30,37,44,51,
    58,59,52,45,38,31,39,46,
    53,60,61,54,47,55,62,63,
];

// ─── Byte Reader ───────────────────────────────────────────

struct Reader<'a> {
    data: &'a [u8],
    pos:  usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self { Reader { data, pos: 0 } }

    fn read_u8(&mut self) -> Result<u8, ImageError> {
        self.data.get(self.pos).copied().ok_or_else(|| ImageError("Unexpected end of JPEG".into()))
            .map(|b| { self.pos += 1; b })
    }

    fn read_u16_be(&mut self) -> Result<u16, ImageError> {
        let hi = self.read_u8()? as u16;
        let lo = self.read_u8()? as u16;
        Ok((hi << 8) | lo)
    }

    fn skip(&mut self, n: usize) -> Result<(), ImageError> {
        if self.pos + n > self.data.len() {
            return Err(ImageError("Unexpected end of JPEG while skipping".into()));
        }
        self.pos += n; Ok(())
    }

    fn remaining_slice(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }
}

// ─── Huffman Table ─────────────────────────────────────────

#[derive(Clone, Default)]
struct HuffTable {
    // Canonical Huffman: lookup by (code, length)
    // We store: for each bit-length (1..=16), the start code and symbols
    delta:   [i32; 17],   // delta[len] = first_code[len] - first_index[len]
    maxcode: [i32; 18],   // maxcode[len] = last valid code for this length, or -1
    huffval: Vec<u8>,     // symbol values
}

impl HuffTable {
    fn build(lengths: &[u8; 16], values: &[u8]) -> Self {
        // Count total symbols
        let mut huffval = Vec::new();
        for &v in values { huffval.push(v); }

        let mut huffcode = [0u16; 256];
        let mut huffsize = [0u8;  256];

        // Generate codes
        let mut k = 0usize;
        for (i, &count) in lengths.iter().enumerate() {
            let len = (i + 1) as u8;
            for _ in 0..count {
                if k >= huffval.len() { break; }
                huffsize[k] = len;
                k += 1;
            }
        }

        let mut code = 0u16;
        let mut si   = huffsize[0];
        k = 0;
        loop {
            loop {
                huffcode[k] = code;
                code += 1;
                k    += 1;
                if k >= huffval.len() || huffsize[k] != si { break; }
            }
            if k >= huffval.len() { break; }
            loop {
                code <<= 1;
                si   += 1;
                if si == huffsize[k] { break; }
            }
        }

        // Build lookup arrays (JPEG spec APPENDIX F)
        let mut maxcode = [-1i32; 18];
        let mut delta   = [0i32; 17];
        let mut j       = 0usize;
        for i in 1usize..=16 {
            if lengths[i-1] == 0 {
                maxcode[i] = -1;
            } else {
                delta[i]   = j as i32 - huffcode[j] as i32;
                j += lengths[i-1] as usize;
                maxcode[i] = huffcode[j-1] as i32;
            }
        }
        maxcode[17] = 0x3FFFF;

        HuffTable { delta, maxcode, huffval }
    }

    fn is_empty(&self) -> bool { self.huffval.is_empty() }
}

// ─── Bit Reader for scan data ──────────────────────────────

struct ScanBits<'a> {
    data:    &'a [u8],
    pos:     usize,
    buf:     u32,
    bits_in: i32,
}

impl<'a> ScanBits<'a> {
    fn new(data: &'a [u8]) -> Self {
        ScanBits { data, pos: 0, buf: 0, bits_in: 0 }
    }

    fn fill(&mut self) -> Result<(), ImageError> {
        while self.bits_in <= 24 {
            if self.pos >= self.data.len() { break; }
            let b = self.data[self.pos]; self.pos += 1;
            if b == 0xFF {
                // Check next byte: 0x00 = stuffed, else it's a marker
                if self.pos < self.data.len() {
                    let next = self.data[self.pos];
                    if next == 0x00 {
                        self.pos += 1; // consume stuffed zero
                    } else if next >= 0xD0 && next <= 0xD7 {
                        self.pos += 1; // RST marker, ignore
                        continue;
                    } else {
                        // Real marker — stop filling
                        self.pos -= 1;
                        break;
                    }
                }
            }
            self.buf     = (self.buf << 8) | b as u32;
            self.bits_in += 8;
        }
        Ok(())
    }

    fn get_bits(&mut self, n: i32) -> Result<i32, ImageError> {
        if n == 0 { return Ok(0); }
        if self.bits_in < n { self.fill()?; }
        if self.bits_in < n {
            return Err(ImageError(format!("JPEG scan: need {} bits, have {}", n, self.bits_in)));
        }
        self.bits_in -= n;
        Ok(((self.buf >> self.bits_in) & ((1 << n) - 1)) as i32)
    }

    fn huff_decode(&mut self, table: &HuffTable) -> Result<u8, ImageError> {
        if table.is_empty() {
            return Err(ImageError("JPEG: Huffman table not defined".into()));
        }
        if self.bits_in < 16 { self.fill()?; }

        let mut code  = 0i32;
        let mut i     = 0i32;
        loop {
            i += 1;
            if i > 16 { break; }
            if self.bits_in < 1 { self.fill()?; }
            self.bits_in -= 1;
            code = (code << 1) | ((self.buf >> self.bits_in) & 1) as i32;
            if code <= table.maxcode[i as usize] {
                let idx = (code + table.delta[i as usize]) as usize;
                return table.huffval.get(idx).copied()
                    .ok_or_else(|| ImageError("JPEG: Huffman value index out of range".into()));
            }
        }
        Err(ImageError("JPEG: Bad Huffman code".into()))
    }

    fn extend(v: i32, t: u8) -> i32 {
        if t == 0 { return 0; }
        if v < (1 << (t - 1)) { v - (1 << t) + 1 } else { v }
    }
}

// ─── Component ─────────────────────────────────────────────

#[derive(Clone)]
struct Component {
    id:      u8,
    h_samp:  u8,   // horizontal sampling factor
    v_samp:  u8,   // vertical sampling factor
    qt_id:   u8,   // quantization table id
    dc_id:   u8,
    ac_id:   u8,
    dc_pred: i32,  // DC predictor (reset per restart interval)
}

// ─── Decoder ───────────────────────────────────────────────

struct JpegDecoder<'a> {
    r:          Reader<'a>,
    width:      u32,
    height:     u32,
    ncomp:      usize,
    comps:      Vec<Component>,
    qt:         [[[i32; 64]; 4]; 1], // qt[0][table_id][coeff]
    dc_tables:  [HuffTable; 4],
    ac_tables:  [HuffTable; 4],
    restart_interval: u32,
}

// Work around array-of-non-Copy
struct QTables([[i32; 64]; 4]);

impl<'a> JpegDecoder<'a> {
    fn new(data: &'a [u8]) -> Result<Self, ImageError> {
        if data.len() < 4 || data[0] != 0xFF || data[1] != M_SOI {
            return Err(ImageError("Not a JPEG file (missing SOI)".into()));
        }
        Ok(JpegDecoder {
            r: Reader::new(data),
            width: 0, height: 0,
            ncomp: 0, comps: Vec::new(),
            qt: [[[0i32; 64]; 4]],
            dc_tables: Default::default(),
            ac_tables: Default::default(),
            restart_interval: 0,
        })
    }

    fn next_marker(&mut self) -> Result<u8, ImageError> {
        // Skip to next 0xFF, then return the marker byte
        loop {
            let b = self.r.read_u8()?;
            if b == 0xFF {
                let m = self.r.read_u8()?;
                if m != 0x00 && m != 0xFF { return Ok(m); }
            }
        }
    }

    fn decode(mut self) -> Result<Image, ImageError> {
        // Already verified SOI above; skip it
        self.r.skip(2)?;

        loop {
            let marker = self.next_marker()?;
            match marker {
                M_SOF0 => self.parse_sof0()?,
                M_DHT  => self.parse_dht()?,
                M_DQT  => self.parse_dqt()?,
                M_DRI  => self.parse_dri()?,
                M_SOS  => return self.parse_sos_and_decode(),
                M_EOI  => return Err(ImageError("JPEG ended before scan data".into())),
                0xE0..=0xEF | 0xFE => {
                    // APP / COM markers — skip
                    let len = self.r.read_u16_be()? as usize;
                    self.r.skip(len.saturating_sub(2))?;
                }
                _ => {
                    // Unknown marker — try to skip
                    if let Ok(len) = self.r.read_u16_be() {
                        self.r.skip((len as usize).saturating_sub(2))?;
                    }
                }
            }
        }
    }

    // ─── SOF0 ──────────────────────────────────────────────

    fn parse_sof0(&mut self) -> Result<(), ImageError> {
        let _len       = self.r.read_u16_be()?;
        let precision  = self.r.read_u8()?;
        if precision != 8 {
            return Err(ImageError(format!("JPEG: unsupported precision {} (only 8-bit supported)", precision)));
        }
        self.height = self.r.read_u16_be()? as u32;
        self.width  = self.r.read_u16_be()? as u32;
        self.ncomp  = self.r.read_u8()? as usize;

        if self.ncomp != 1 && self.ncomp != 3 {
            return Err(ImageError(format!("JPEG: unsupported component count {}", self.ncomp)));
        }

        self.comps.clear();
        for _ in 0..self.ncomp {
            let id    = self.r.read_u8()?;
            let samp  = self.r.read_u8()?;
            let qt_id = self.r.read_u8()?;
            self.comps.push(Component {
                id, qt_id,
                h_samp:  (samp >> 4) & 0xF,
                v_samp:  samp & 0xF,
                dc_id:   0, ac_id: 0,
                dc_pred: 0,
            });
        }
        Ok(())
    }

    // ─── DQT ───────────────────────────────────────────────

    fn parse_dqt(&mut self) -> Result<(), ImageError> {
        let mut remaining = self.r.read_u16_be()? as usize - 2;
        while remaining > 0 {
            let pq_tq = self.r.read_u8()?;
            let pq    = pq_tq >> 4;  // 0 = 8-bit, 1 = 16-bit
            let tq    = (pq_tq & 0xF) as usize;
            if tq >= 4 {
                return Err(ImageError(format!("JPEG: DQT table id {} out of range", tq)));
            }
            for i in 0..64 {
                let val = if pq == 0 {
                    self.r.read_u8()? as i32
                } else {
                    self.r.read_u16_be()? as i32
                };
                self.qt[0][tq][ZIGZAG[i] as usize] = val;
            }
            remaining -= 1 + 64 * if pq == 0 { 1 } else { 2 };
        }
        Ok(())
    }

    // ─── DHT ───────────────────────────────────────────────

    fn parse_dht(&mut self) -> Result<(), ImageError> {
        let mut remaining = self.r.read_u16_be()? as usize - 2;
        while remaining > 0 {
            let tc_th = self.r.read_u8()?;
            let tc    = (tc_th >> 4) & 1; // 0 = DC, 1 = AC
            let th    = (tc_th & 0xF) as usize;
            if th >= 4 {
                return Err(ImageError(format!("JPEG: DHT table id {} out of range", th)));
            }

            let mut lengths = [0u8; 16];
            let mut total   = 0usize;
            for i in 0..16 {
                lengths[i] = self.r.read_u8()?;
                total += lengths[i] as usize;
            }

            let mut values = vec![0u8; total];
            for i in 0..total { values[i] = self.r.read_u8()?; }

            let table = HuffTable::build(&lengths, &values);
            if tc == 0 { self.dc_tables[th] = table; }
            else        { self.ac_tables[th] = table; }

            remaining -= 1 + 16 + total;
        }
        Ok(())
    }

    // ─── DRI ───────────────────────────────────────────────

    fn parse_dri(&mut self) -> Result<(), ImageError> {
        let _len = self.r.read_u16_be()?;
        self.restart_interval = self.r.read_u16_be()? as u32;
        Ok(())
    }

    // ─── SOS + Decode ──────────────────────────────────────

    fn parse_sos_and_decode(mut self) -> Result<Image, ImageError> {
        let len   = self.r.read_u16_be()? as usize;
        let ns    = self.r.read_u8()? as usize;
        if ns != self.ncomp {
            return Err(ImageError(format!("JPEG SOS: {} components, expected {}", ns, self.ncomp)));
        }

        // Component selector and Huffman table mapping
        let mut scan_order: Vec<(usize, u8, u8)> = Vec::new(); // (comp_idx, dc_id, ac_id)
        for _ in 0..ns {
            let cs   = self.r.read_u8()?;
            let tdta = self.r.read_u8()?;
            let dc   = (tdta >> 4) & 0xF;
            let ac   = tdta & 0xF;
            let ci   = self.comps.iter().position(|c| c.id == cs)
                .ok_or_else(|| ImageError(format!("JPEG SOS: unknown component id {}", cs)))?;
            scan_order.push((ci, dc, ac));
        }

        // Ss, Se, Ah, Al (spectral selection — baseline uses 0, 63, 0, 0)
        self.r.skip(3)?;

        // Everything after SOS header is scan data
        let scan_data = self.r.remaining_slice();

        if self.width == 0 || self.height == 0 {
            return Err(ImageError("JPEG: SOF0 not seen before SOS".into()));
        }

        // Determine max sampling factors
        let max_h = self.comps.iter().map(|c| c.h_samp).max().unwrap_or(1) as usize;
        let max_v = self.comps.iter().map(|c| c.v_samp).max().unwrap_or(1) as usize;

        // MCU dimensions
        let mcu_w = max_h * 8;
        let mcu_h = max_v * 8;
        let mcus_x = (self.width  as usize + mcu_w - 1) / mcu_w;
        let mcus_y = (self.height as usize + mcu_h - 1) / mcu_h;

        // Allocate per-component sample buffers (full image, padded to MCU grid)
        let pw = mcus_x * mcu_w;
        let ph = mcus_y * mcu_h;
        let mut channels: Vec<Vec<i32>> = vec![vec![0i32; pw * ph]; self.ncomp];

        let mut bits = ScanBits::new(scan_data);
        let mut dc_preds = vec![0i32; self.ncomp];
        let mut mcu_count = 0u32;

        for my in 0..mcus_y {
            for mx in 0..mcus_x {
                // Restart interval
                if self.restart_interval > 0 && mcu_count > 0
                    && mcu_count % self.restart_interval == 0
                {
                    // Reset DC predictors
                    for p in dc_preds.iter_mut() { *p = 0; }
                }

                for &(ci, dc_id, ac_id) in &scan_order {
                    let comp    = &self.comps[ci];
                    let h_samp  = comp.h_samp as usize;
                    let v_samp  = comp.v_samp as usize;
                    let qt_id   = comp.qt_id  as usize;

                    // Each component may have multiple data units per MCU
                    for vy in 0..v_samp {
                        for hx in 0..h_samp {
                            let block = decode_block(
                                &mut bits,
                                &mut dc_preds[ci],
                                &self.dc_tables[dc_id as usize],
                                &self.ac_tables[ac_id as usize],
                                &self.qt[0][qt_id],
                            )?;

                            // Write 8x8 block into channel buffer
                            let bx = mx * h_samp * 8 + hx * 8;
                            let by = my * v_samp * 8 + vy * 8;
                            for row in 0..8 {
                                for col in 0..8 {
                                    let px = bx + col;
                                    let py = by + row;
                                    if px < pw && py < ph {
                                        channels[ci][py * pw + px] = block[row * 8 + col];
                                    }
                                }
                            }
                        }
                    }
                }
                mcu_count += 1;
            }
        }

        // Build output image
        let mut image = Image::new(self.width, self.height);

        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                // For subsampled components, we need to scale coordinates
                let (r, g, b) = if self.ncomp == 1 {
                    let v = clamp8(channels[0][y * pw + x] + 128);
                    (v, v, v)
                } else {
                    // YCbCr → RGB
                    // Component 0: Y  (full resolution)
                    // Component 1: Cb (possibly subsampled)
                    // Component 2: Cr (possibly subsampled)
                    let yy = channels[0][y * pw + x] + 128;

                    let cb_x = x * self.comps[1].h_samp as usize / max_h;
                    let cb_y = y * self.comps[1].v_samp as usize / max_v;
                    let cb   = channels[1][cb_y * pw + cb_x];

                    let cr_x = x * self.comps[2].h_samp as usize / max_h;
                    let cr_y = y * self.comps[2].v_samp as usize / max_v;
                    let cr   = channels[2][cr_y * pw + cr_x];

                    let r = clamp8(yy + (1.402 * cr as f32) as i32);
                    let g = clamp8(yy + (-0.34414 * cb as f32 - 0.71414 * cr as f32) as i32);
                    let b = clamp8(yy + (1.772  * cb as f32) as i32);
                    (r, g, b)
                };
                image.set_pixel(x as u32, y as u32, r, g, b, 255);
            }
        }

        Ok(image)
    }
}

// ─── Block Decode + IDCT ───────────────────────────────────

fn decode_block(
    bits:     &mut ScanBits,
    dc_pred:  &mut i32,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    qt:       &[i32; 64],
) -> Result<[i32; 64], ImageError> {
    let mut coeffs = [0i32; 64];

    // DC coefficient
    let t    = bits.huff_decode(dc_table)? as i32;
    let diff = ScanBits::extend(bits.get_bits(t)?, t as u8);
    *dc_pred += diff;
    coeffs[0] = *dc_pred;

    // AC coefficients
    let mut k = 1usize;
    while k < 64 {
        let rs   = bits.huff_decode(ac_table)?;
        let run  = (rs >> 4) as usize;
        let size = (rs & 0xF) as i32;

        if size == 0 {
            if run == 15 { k += 16; }  // ZRL
            else         { break; }     // EOB
        } else {
            k += run;
            if k >= 64 { break; }
            let val = ScanBits::extend(bits.get_bits(size)?, size as u8);
            coeffs[ZIGZAG[k] as usize] = val;
            k += 1;
        }
    }

    // Dequantize
    for i in 0..64 { coeffs[i] *= qt[i]; }

    // IDCT
    let mut out = [0i32; 64];
    idct(&coeffs, &mut out);

    Ok(out)
}

// ─── Floating-point IDCT (AAN algorithm) ───────────────────
// Reference: Arai, Agui, Nakajima (1988). Produces values in [-1024, 1023].

const W1: f32 = 0.707106781;  // cos(pi/4)
const W2: f32 = 0.541196100;  // cos(3pi/8) (scaled)
const W3: f32 = 1.306562965;
const W4: f32 = 0.382683432;
const W5: f32 = 1.847759065;

fn idct_1d(s: &[f32; 8], d: &mut [f32; 8]) {
    let s0 = s[0]; let s1 = s[1]; let s2 = s[2]; let s3 = s[3];
    let s4 = s[4]; let s5 = s[5]; let s6 = s[6]; let s7 = s[7];

    // Even part
    let tmp0 = s0 + s4;
    let tmp1 = s0 - s4;
    let tmp2 = s2 * W1;        // c4
    let tmp3 = s6 * W1;
    let tmp4 = tmp2 + tmp3;    // this isn't quite right; use standard AAN
    let tmp5 = tmp2 - tmp3;

    // Full 8-point 1D IDCT via Lee's recursive factoring
    // (Standard clean implementation)
    let x0 = s0;
    let x1 = s4;
    let x2 = s2;
    let x3 = s6;
    let x4 = s1;
    let x5 = s5;
    let x6 = s3;
    let x7 = s7;

    // Stage 1
    let t0  =  x0 + x1;
    let t1  =  x0 - x1;
    let t2  =  x2 * 1.41421356 - x3;
    let t3  =  x2 + x3 * 1.41421356;

    // Stage 2
    let p0  =  t0 + t3;
    let p1  =  t1 + t2;
    let p2  =  t1 - t2;
    let p3  =  t0 - t3;

    // Odd part
    let t4  = -x4 - x5;
    let t5  =  x4 + x6;
    let t6  =  x5 + x7;
    let t7  =  x6 - x7;

    let s_  = (t5 + t6) * 0.70710678;
    let t5_ = t5 * 0.70710678 - s_;
    let t6_ = s_ - t6 * 0.70710678;

    let u4  = t4 * 0.76536686 + t6_ * 1.84775907;
    let u5  = t5_ * 1.84775907 - t4 * 1.84775907 * 0.41421356;
    let u6  = -t7 * 0.76536686 + t5_ * 0.76536686;
    let u7  = t7 * 1.84775907 + u6;

    // Actually let's just use the standard separable IDCT correctly
    // (the above is getting complicated — use the reference scalar version)
    let _ = (t0,t1,t2,t3,p0,p1,p2,p3,t4,t5,t6,t7,s_,t5_,t6_,u4,u5,u6,u7,
             tmp0,tmp1,tmp2,tmp3,tmp4,tmp5,x0,x1,x2,x3,x4,x5,x6,x7);

    // Reference implementation: standard 8-point IDCT
    let c1: f32 = 0.980785280;
    let c2: f32 = 0.923879532;
    let c3: f32 = 0.831469612;
    let c4: f32 = 0.707106781;
    let c5: f32 = 0.555570233;
    let c6: f32 = 0.382683432;
    let c7: f32 = 0.195090322;

    let y0 = s[0]; let y1 = s[1]; let y2 = s[2]; let y3 = s[3];
    let y4 = s[4]; let y5 = s[5]; let y6 = s[6]; let y7 = s[7];

    let e0 = y0 + y4*c4*2.0 + y2*c2*2.0 - y6*c6*2.0
           + y1*c1*2.0 + y3*c3*2.0 + y5*c5*2.0 + y7*c7*2.0;
    let _ = e0; // placate compiler while we use the proven implementation below

    // Use the Loeffler factored IDCT (same as original codebase but corrected)
    let s0 = s[0]; let s1 = s[1]; let s2 = s[2]; let s3 = s[3];
    let s4 = s[4]; let s5 = s[5]; let s6 = s[6]; let s7 = s[7];

    let p2  = s2;  let p3  = s6;
    let p1  = (p2+p3) * 0.5411961;
    let t2_ = p1 + p3 * -1.847759065;
    let t3_ = p1 + p2 *  0.765366865;
    let p2  = s0;  let p3  = s4;
    let t0_ = (p2+p3) * 0.7071068;
    let t1_ = (p2-p3) * 0.7071068;
    let x0  = t0_ + t3_;
    let x3  = t0_ - t3_;
    let x1  = t1_ + t2_;
    let x2  = t1_ - t2_;

    let t0_ = s7; let t1_ = s5; let t2_ = s3; let t3_ = s1;
    let p3  = t0_ + t2_; let p4 = t1_ + t3_;
    let p1  = t0_ + t3_; let p2 = t1_ + t2_;
    let p5  = (p3+p4) * 1.175875602;
    let t0_ = t0_ * 0.298631336;
    let t1_ = t1_ * 2.053119869;
    let t2_ = t2_ * 3.072711026;
    let t3_ = t3_ * 1.501321110;
    let p1  = p5 + p1 * -0.899976223;
    let p2  = p5 + p2 * -2.562915447;
    let p3  = p3 * -1.961570560;
    let p4  = p4 * -0.390180644;
    let t3_ = t3_ + p1 + p4;
    let t2_ = t2_ + p2 + p3;
    let t1_ = t1_ + p2 + p4;
    let t0_ = t0_ + p1 + p3;

    d[0] = x0 + t3_; d[7] = x0 - t3_;
    d[1] = x1 + t2_; d[6] = x1 - t2_;
    d[2] = x2 + t1_; d[5] = x2 - t1_;
    d[3] = x3 + t0_; d[4] = x3 - t0_;
}

fn idct(coeffs: &[i32; 64], out: &mut [i32; 64]) {
    let mut tmp = [0f32; 64];

    // Convert to float
    let mut f = [0f32; 64];
    for i in 0..64 { f[i] = coeffs[i] as f32; }

    // Row IDCT
    for i in 0..8 {
        let mut row_in  = [0f32; 8];
        let mut row_out = [0f32; 8];
        for j in 0..8 { row_in[j] = f[i*8 + j]; }
        idct_1d(&row_in, &mut row_out);
        for j in 0..8 { tmp[i*8 + j] = row_out[j]; }
    }

    // Column IDCT
    for j in 0..8 {
        let mut col_in  = [0f32; 8];
        let mut col_out = [0f32; 8];
        for i in 0..8 { col_in[i] = tmp[i*8 + j]; }
        idct_1d(&col_in, &mut col_out);
        for i in 0..8 {
            out[i*8 + j] = (col_out[i] / 8.0).round() as i32;
        }
    }
}

fn clamp8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}