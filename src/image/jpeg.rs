// ============================================================
// RetroX JPEG Decoder
// Implements baseline JPEG (ISO/IEC 10918-1).
// Supports: YCbCr, Grayscale. No progressive/lossless.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use super::{Image, ImageError};

pub fn decode_jpeg(data: &[u8]) -> Result<Image, ImageError> {
    let mut dec = JpegDecoder::new(data);
    dec.decode()
}

// ─── JPEG Markers ──────────────────────────────────────────

const SOI:  u8 = 0xD8; // Start of Image
const EOI:  u8 = 0xD9; // End of Image
const SOF0: u8 = 0xC0; // Start of Frame (Baseline DCT)
const DHT:  u8 = 0xC4; // Define Huffman Table
const DQT:  u8 = 0xDB; // Define Quantization Table
const SOS:  u8 = 0xDA; // Start of Scan
const APP0: u8 = 0xE0; // JFIF header
const COM:  u8 = 0xFE; // Comment

// ─── Zigzag Order ──────────────────────────────────────────

const ZIGZAG: [usize; 64] = [
     0, 1, 8,16, 9, 2, 3,10,
    17,24,32,25,18,11, 4, 5,
    12,19,26,33,40,48,41,34,
    27,20,13, 6, 7,14,21,28,
    35,42,49,56,57,50,43,36,
    29,22,15,23,30,37,44,51,
    58,59,52,45,38,31,39,46,
    53,60,61,54,47,55,62,63,
];

// ─── DCT Coefficients ──────────────────────────────────────

fn idct_1d(v: &mut [f32; 8]) {
    let s0 = v[0]; let s1 = v[1]; let s2 = v[2]; let s3 = v[3];
    let s4 = v[4]; let s5 = v[5]; let s6 = v[6]; let s7 = v[7];

    let p2 = s2; let p3 = s6;
    let p1 = (p2 + p3) * 0.5411961;
    let t2 = p1 + p3 * (-1.847759065);
    let t3 = p1 + p2 * 0.765366865;
    let p2 = s0; let p3 = s4;
    let t0 = (p2 + p3) * 0.7071068;
    let t1 = (p2 - p3) * 0.7071068;
    let x0 = t0 + t3; let x3 = t0 - t3;
    let x1 = t1 + t2; let x2 = t1 - t2;

    let t0 = s7; let t1 = s5; let t2 = s3; let t3 = s1;
    let p3 = t0 + t2; let p4 = t1 + t3; let p1 = t0 + t3;
    let p2 = t1 + t2; let p5 = (p3 + p4) * 1.175875602;
    let t0 = t0 * 0.298631336;
    let t1 = t1 * 2.053119869;
    let t2 = t2 * 3.072711026;
    let t3 = t3 * 1.501321110;
    let p1 = p5 + p1 * (-0.899976223);
    let p2 = p5 + p2 * (-2.562915447);
    let p3 = p3 * (-1.961570560);
    let p4 = p4 * (-0.390180644);
    let t3 = t3 + p1 + p4;
    let t2 = t2 + p2 + p3;
    let t1 = t1 + p2 + p4;
    let t0 = t0 + p1 + p3;

    v[0] = x0 + t3; v[7] = x0 - t3;
    v[1] = x1 + t2; v[6] = x1 - t2;
    v[2] = x2 + t1; v[5] = x2 - t1;
    v[3] = x3 + t0; v[4] = x3 - t0;
}

fn idct_block(block: &mut [f32; 64]) {
    // Row IDCT
    for i in 0..8 {
        let mut row = [
            block[i*8], block[i*8+1], block[i*8+2], block[i*8+3],
            block[i*8+4], block[i*8+5], block[i*8+6], block[i*8+7],
        ];
        idct_1d(&mut row);
        for j in 0..8 { block[i*8+j] = row[j]; }
    }
    // Column IDCT
    for i in 0..8 {
        let mut col = [
            block[i], block[8+i], block[16+i], block[24+i],
            block[32+i], block[40+i], block[48+i], block[56+i],
        ];
        idct_1d(&mut col);
        for j in 0..8 { block[j*8+i] = col[j]; }
    }
}

// ─── Huffman Table ─────────────────────────────────────────

#[derive(Clone)]
struct HuffTable {
    codes: Vec<(u16, u8, u8)>, // (code, length, value)
}

impl HuffTable {
    fn new() -> Self { HuffTable { codes: Vec::new() } }

    fn build(&mut self, lengths: &[u8; 16], values: &[u8]) {
        self.codes.clear();
        let mut code = 0u16;
        let mut val_idx = 0usize;
        for len in 1u8..=16 {
            let count = lengths[(len-1) as usize] as usize;
            for _ in 0..count {
                if val_idx < values.len() {
                    self.codes.push((code, len, values[val_idx]));
                    val_idx += 1;
                }
                code += 1;
            }
            code <<= 1;
        }
    }

    fn decode(&self, bits: &mut JpegBitReader) -> Result<u8, ImageError> {
        let mut code = 0u16;
        let mut len  = 0u8;
        for _ in 0..16 {
            code = (code << 1) | bits.read_bit()? as u16;
            len += 1;
            for &(c, l, v) in &self.codes {
                if l == len && c == code { return Ok(v); }
            }
        }
        Err(ImageError("Invalid JPEG Huffman code".into()))
    }
}

// ─── JPEG Bit Reader ───────────────────────────────────────

struct JpegBitReader<'a> {
    data:    &'a [u8],
    pos:     usize,
    buf:     u32,
    bits_in: u32,
}

impl<'a> JpegBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        JpegBitReader { data, pos: 0, buf: 0, bits_in: 0 }
    }

    fn read_bit(&mut self) -> Result<u8, ImageError> {
        if self.bits_in == 0 {
            if self.pos >= self.data.len() {
                return Err(ImageError("Unexpected end of JPEG scan data".into()));
            }
            let mut byte = self.data[self.pos];
            self.pos += 1;
            // Skip stuffed bytes
            if byte == 0xFF {
                let next = self.data.get(self.pos).copied().unwrap_or(0);
                if next == 0x00 { self.pos += 1; }
                else { byte = 0xFF; }
            }
            self.buf     = byte as u32;
            self.bits_in = 8;
        }
        let bit = (self.buf >> (self.bits_in - 1)) & 1;
        self.bits_in -= 1;
        Ok(bit as u8)
    }

    fn read_bits(&mut self, n: u8) -> Result<i32, ImageError> {
        let mut val = 0i32;
        for _ in 0..n {
            val = (val << 1) | self.read_bit()? as i32;
        }
        Ok(val)
    }

    fn receive_and_extend(&mut self, n: u8) -> Result<i32, ImageError> {
        if n == 0 { return Ok(0); }
        let val = self.read_bits(n)?;
        if val < (1 << (n - 1)) {
            Ok(val - (1 << n) + 1)
        } else {
            Ok(val)
        }
    }
}

// ─── Component ─────────────────────────────────────────────

#[derive(Clone)]
struct Component {
    id:       u8,
    h_samp:   u8,
    v_samp:   u8,
    qt_idx:   u8,
    dc_idx:   u8,
    ac_idx:   u8,
    dc_pred:  i32,
}

// ─── JPEG Decoder ──────────────────────────────────────────

struct JpegDecoder<'a> {
    data:        &'a [u8],
    pos:         usize,
    width:       u32,
    height:      u32,
    components:  Vec<Component>,
    qt:          [[u16; 64]; 4],
    dc_tables:   [HuffTable; 4],
    ac_tables:   [HuffTable; 4],
}

impl<'a> JpegDecoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        JpegDecoder {
            data, pos: 0,
            width: 0, height: 0,
            components: Vec::new(),
            qt: [[0u16; 64]; 4],
            dc_tables: [HuffTable::new(), HuffTable::new(), HuffTable::new(), HuffTable::new()],
            ac_tables: [HuffTable::new(), HuffTable::new(), HuffTable::new(), HuffTable::new()],
        }
    }

    fn read_u8(&mut self) -> Result<u8, ImageError> {
        if self.pos >= self.data.len() {
            return Err(ImageError("Unexpected end of JPEG data".into()));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_u16_be(&mut self) -> Result<u16, ImageError> {
        let hi = self.read_u8()? as u16;
        let lo = self.read_u8()? as u16;
        Ok((hi << 8) | lo)
    }

    fn decode(&mut self) -> Result<Image, ImageError> {
        // Check SOI marker
        if self.read_u8()? != 0xFF || self.read_u8()? != SOI {
            return Err(ImageError("Not a valid JPEG file".into()));
        }

        loop {
            let b = self.read_u8()?;
            if b != 0xFF { continue; }
            let marker = self.read_u8()?;

            match marker {
                SOF0 => self.parse_sof0()?,
                DHT  => self.parse_dht()?,
                DQT  => self.parse_dqt()?,
                SOS  => return self.parse_sos(),
                EOI  => return Err(ImageError("JPEG ended without scan data".into())),
                APP0 | COM | 0xE1..=0xEF => {
                    let len = self.read_u16_be()? as usize;
                    self.pos += len - 2;
                }
                0xD0..=0xD7 => {} // RST markers
                _  => {
                    let len = self.read_u16_be()? as usize;
                    self.pos += len.saturating_sub(2);
                }
            }
        }
    }

    fn parse_sof0(&mut self) -> Result<(), ImageError> {
        let _len       = self.read_u16_be()?;
        let _precision = self.read_u8()?;
        self.height    = self.read_u16_be()? as u32;
        self.width     = self.read_u16_be()? as u32;
        let ncomp      = self.read_u8()?;

        self.components.clear();
        for _ in 0..ncomp {
            let id     = self.read_u8()?;
            let samp   = self.read_u8()?;
            let qt_idx = self.read_u8()?;
            self.components.push(Component {
                id,
                h_samp:  (samp >> 4) & 0xF,
                v_samp:  samp & 0xF,
                qt_idx,
                dc_idx:  0,
                ac_idx:  0,
                dc_pred: 0,
            });
        }
        Ok(())
    }

    fn parse_dqt(&mut self) -> Result<(), ImageError> {
        let mut len = self.read_u16_be()? as usize - 2;
        while len > 0 {
            let pq_tq = self.read_u8()?;
            let pq    = (pq_tq >> 4) & 0xF;
            let tq    = (pq_tq & 0xF) as usize;
            for i in 0..64 {
                self.qt[tq][ZIGZAG[i]] = if pq == 0 {
                    self.read_u8()? as u16
                } else {
                    self.read_u16_be()?
                };
            }
            len -= 1 + 64 * (if pq == 0 { 1 } else { 2 });
        }
        Ok(())
    }

    fn parse_dht(&mut self) -> Result<(), ImageError> {
        let mut len = self.read_u16_be()? as usize - 2;
        while len > 0 {
            let tc_th  = self.read_u8()?;
            let tc     = (tc_th >> 4) & 1; // 0=DC, 1=AC
            let th     = (tc_th & 0xF) as usize;
            let mut lengths = [0u8; 16];
            let mut total   = 0usize;
            for i in 0..16 {
                lengths[i] = self.read_u8()?;
                total += lengths[i] as usize;
            }
            let mut values = vec![0u8; total];
            for i in 0..total { values[i] = self.read_u8()?; }
            if tc == 0 {
                self.dc_tables[th].build(&lengths, &values);
            } else {
                self.ac_tables[th].build(&lengths, &values);
            }
            len -= 1 + 16 + total;
        }
        Ok(())
    }

    fn parse_sos(&mut self) -> Result<Image, ImageError> {
        let len   = self.read_u16_be()? as usize;
        let ncomp = self.read_u8()? as usize;

        let mut order: Vec<(usize, u8, u8)> = Vec::new(); // (comp_idx, dc_idx, ac_idx)

        for _ in 0..ncomp {
            let cs  = self.read_u8()?;
            let tdta = self.read_u8()?;
            let dc_idx = (tdta >> 4) & 0xF;
            let ac_idx = tdta & 0xF;
            let comp_idx = self.components.iter().position(|c| c.id == cs)
                .ok_or_else(|| ImageError(format!("Unknown component id {}", cs)))?;
            order.push((comp_idx, dc_idx, ac_idx));
        }

        // Skip Ss, Se, Ah/Al
        self.pos += len - 2 - 1 - ncomp * 2;

        let scan_data = &self.data[self.pos..];
        let mut bits  = JpegBitReader::new(scan_data);

        let mcu_w = 8; let mcu_h = 8;
        let mcus_x = (self.width  + mcu_w - 1) / mcu_w;
        let mcus_y = (self.height + mcu_h - 1) / mcu_h;

        // Allocate channel buffers
        let pw = (self.width  as usize + 7) & !7;
        let ph = (self.height as usize + 7) & !7;
        let mut channels: Vec<Vec<u8>> = vec![vec![128u8; pw * ph]; ncomp];

        // Reset DC predictors
        let mut dc_preds = vec![0i32; ncomp];

        for my in 0..mcus_y as usize {
            for mx in 0..mcus_x as usize {
                for (i, &(ci, dc_idx, ac_idx)) in order.iter().enumerate() {
                    let qt_idx = self.components[ci].qt_idx as usize;
                    let block = self.decode_block(
                        &mut bits,
                        &mut dc_preds[i],
                        dc_idx as usize,
                        ac_idx as usize,
                        qt_idx,
                    )?;

                    // Write block into channel
                    let bx = mx * 8;
                    let by = my * 8;
                    for row in 0..8usize {
                        for col in 0..8usize {
                            let px = bx + col;
                            let py = by + row;
                            if px < pw && py < ph {
                                let val = (block[row * 8 + col] + 128.0)
                                    .round().clamp(0.0, 255.0) as u8;
                                channels[i][py * pw + px] = val;
                            }
                        }
                    }
                }
            }
        }

        // Convert to RGBA
        let mut image = Image::new(self.width, self.height);

        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let idx = y * pw + x;
                let (r, g, b) = if ncomp == 1 {
                    let v = channels[0][idx];
                    (v, v, v)
                } else {
                    let yy = channels[0][idx] as f32;
                    let cb = channels[1][idx] as f32 - 128.0;
                    let cr = channels[2][idx] as f32 - 128.0;
                    let r = (yy + 1.402 * cr).round().clamp(0.0, 255.0) as u8;
                    let g = (yy - 0.344136 * cb - 0.714136 * cr).round().clamp(0.0, 255.0) as u8;
                    let b = (yy + 1.772 * cb).round().clamp(0.0, 255.0) as u8;
                    (r, g, b)
                };
                image.set_pixel(x as u32, y as u32, r, g, b, 255);
            }
        }

        Ok(image)
    }

    fn decode_block(
        &self,
        bits:    &mut JpegBitReader,
        dc_pred: &mut i32,
        dc_idx:  usize,
        ac_idx:  usize,
        qt_idx:  usize,
    ) -> Result<[f32; 64], ImageError> {
        let mut coeffs = [0i32; 64];

        // DC coefficient
        let dc_cat = self.dc_tables[dc_idx].decode(bits)?;
        let diff   = bits.receive_and_extend(dc_cat)?;
        *dc_pred  += diff;
        coeffs[0]  = *dc_pred;

        // AC coefficients
        let mut k = 1;
        while k < 64 {
            let rs   = self.ac_tables[ac_idx].decode(bits)?;
            let run  = (rs >> 4) as usize;
            let size = rs & 0xF;
            if size == 0 {
                if run == 15 { k += 16; } else { break; }
            } else {
                k += run;
                if k >= 64 { break; }
                coeffs[ZIGZAG[k]] = bits.receive_and_extend(size)?;
                k += 1;
            }
        }

        // Dequantize and IDCT
        let mut block = [0.0f32; 64];
        for i in 0..64 {
            block[i] = coeffs[i] as f32 * self.qt[qt_idx][i] as f32;
        }
        idct_block(&mut block);

        // Scale
        for v in block.iter_mut() {
            *v /= 8.0;
        }

        Ok(block)
    }
}
