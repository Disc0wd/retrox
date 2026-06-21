// ============================================================
// RetroX PNG Decoder
// Implements PNG spec (ISO/IEC 15948:2004).
// Supports: RGB, RGBA, Grayscale, Gray+Alpha, Indexed.
// Bit depths: 1, 2, 4, 8, 16. No interlace.
// Zero external dependencies.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use super::{Image, ImageError};

pub fn decode_png(data: &[u8]) -> Result<Image, ImageError> {
    PngDecoder::new(data).decode()
}

const SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

const CT_GRAY:       u8 = 0;
const CT_RGB:        u8 = 2;
const CT_INDEXED:    u8 = 3;
const CT_GRAY_ALPHA: u8 = 4;
const CT_RGBA:       u8 = 6;

// ─── Byte Reader ───────────────────────────────────────────

struct Reader<'a> { data: &'a [u8], pos: usize }

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self { Reader { data, pos: 0 } }

    fn read_u8(&mut self) -> Result<u8, ImageError> {
        self.data.get(self.pos)
            .copied()
            .ok_or_else(|| ImageError("Unexpected end of PNG".into()))
            .map(|b| { self.pos += 1; b })
    }

    fn read_u32_be(&mut self) -> Result<u32, ImageError> {
        let a = self.read_u8()? as u32; let b = self.read_u8()? as u32;
        let c = self.read_u8()? as u32; let d = self.read_u8()? as u32;
        Ok((a << 24) | (b << 16) | (c << 8) | d)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ImageError> {
        if self.pos + n > self.data.len() {
            return Err(ImageError("Unexpected end of PNG".into()));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n; Ok(s)
    }

    fn skip(&mut self, n: usize) -> Result<(), ImageError> {
        if self.pos + n > self.data.len() {
            return Err(ImageError("Unexpected end of PNG while skipping".into()));
        }
        self.pos += n; Ok(())
    }
}

// ─── Decoder ───────────────────────────────────────────────

struct PngDecoder<'a> {
    r:          Reader<'a>,
    width:      u32,
    height:     u32,
    bit_depth:  u8,
    color_type: u8,
    palette:    Vec<(u8, u8, u8)>,
    idat:       Vec<u8>,
}

impl<'a> PngDecoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        PngDecoder {
            r: Reader::new(data),
            width: 0, height: 0,
            bit_depth: 0, color_type: 0,
            palette: Vec::new(),
            idat: Vec::new(),
        }
    }

    fn decode(mut self) -> Result<Image, ImageError> {
        if self.r.read_bytes(8)? != SIG {
            return Err(ImageError("Not a valid PNG file".into()));
        }
        loop {
            let length = self.r.read_u32_be()? as usize;
            let tag    = self.r.read_bytes(4)?.to_owned();
            match tag.as_slice() {
                b"IHDR" => { self.parse_ihdr()?; self.r.skip(4)?; }
                b"PLTE" => { self.parse_plte(length)?; self.r.skip(4)?; }
                b"IDAT" => {
                    let chunk = self.r.read_bytes(length)?.to_vec();
                    self.idat.extend_from_slice(&chunk);
                    self.r.skip(4)?;
                }
                b"IEND" => { self.r.skip(length + 4)?; break; }
                _       => { self.r.skip(length + 4)?; }
            }
        }
        if self.width == 0 || self.height == 0 {
            return Err(ImageError("PNG has zero dimensions".into()));
        }
        let raw = inflate_zlib(&self.idat)?;
        self.reconstruct(&raw)
    }

    fn parse_ihdr(&mut self) -> Result<(), ImageError> {
        self.width      = self.r.read_u32_be()?;
        self.height     = self.r.read_u32_be()?;
        self.bit_depth  = self.r.read_u8()?;
        self.color_type = self.r.read_u8()?;
        let _c          = self.r.read_u8()?;
        let _f          = self.r.read_u8()?;
        let interlace   = self.r.read_u8()?;
        if interlace != 0 {
            return Err(ImageError("Interlaced PNG not supported".into()));
        }
        Ok(())
    }

    fn parse_plte(&mut self, length: usize) -> Result<(), ImageError> {
        if length % 3 != 0 {
            return Err(ImageError("PLTE length not divisible by 3".into()));
        }
        let data = self.r.read_bytes(length)?.to_vec();
        self.palette.clear();
        for i in 0..length/3 {
            self.palette.push((data[i*3], data[i*3+1], data[i*3+2]));
        }
        Ok(())
    }

    fn channels(&self) -> usize {
        match self.color_type {
            CT_GRAY | CT_INDEXED => 1,
            CT_GRAY_ALPHA        => 2,
            CT_RGB               => 3,
            CT_RGBA              => 4,
            _                    => 0,
        }
    }

    fn reconstruct(&self, filtered: &[u8]) -> Result<Image, ImageError> {
        let channels = self.channels();
        if channels == 0 {
            return Err(ImageError(format!("Unknown PNG color type {}", self.color_type)));
        }

        let bpp = ((channels * self.bit_depth as usize) + 7) / 8;
        let bpp = bpp.max(1);

        let row_bytes = (self.width as usize * channels * self.bit_depth as usize + 7) / 8;

        let mut image    = Image::new(self.width, self.height);
        let mut prev_row = vec![0u8; row_bytes];
        let mut pos      = 0usize;

        for y in 0..self.height as usize {
            if pos + 1 + row_bytes > filtered.len() {
                return Err(ImageError(format!(
                    "PNG data truncated at row {} (have {} bytes, need {})",
                    y, filtered.len() - pos, 1 + row_bytes
                )));
            }
            let filter = filtered[pos]; pos += 1;
            let raw    = &filtered[pos..pos + row_bytes]; pos += row_bytes;

            let row = unfilter(filter, raw, &prev_row, bpp)?;

            for x in 0..self.width as usize {
                let (r, g, b, a) = self.pixel_rgba(&row, x, channels)?;
                image.set_pixel(x as u32, y as u32, r, g, b, a);
            }

            prev_row = row;
        }

        Ok(image)
    }

    fn pixel_rgba(&self, row: &[u8], x: usize, channels: usize) -> Result<(u8,u8,u8,u8), ImageError> {
        let s = |c: usize| -> u8 {
            match self.bit_depth {
                8  => row.get(x * channels + c).copied().unwrap_or(0),
                16 => row.get((x * channels + c) * 2).copied().unwrap_or(0),
                _  => sub_byte(row, x, self.bit_depth),
            }
        };

        match self.color_type {
            CT_GRAY       => { let v = s(0); Ok((v, v, v, 255)) }
            CT_RGB        => Ok((s(0), s(1), s(2), 255)),
            CT_INDEXED    => {
                let idx = sub_byte_or_8(row, x, self.bit_depth, channels) as usize;
                let (r, g, b) = self.palette.get(idx).copied().unwrap_or((0,0,0));
                Ok((r, g, b, 255))
            }
            CT_GRAY_ALPHA => { let v = s(0); Ok((v, v, v, s(1))) }
            CT_RGBA       => Ok((s(0), s(1), s(2), s(3))),
            _             => Err(ImageError(format!("Unhandled color type {}", self.color_type))),
        }
    }
}

fn sub_byte(row: &[u8], x: usize, bit_depth: u8) -> u8 {
    match bit_depth {
        1 => { let b = row.get(x/8).copied().unwrap_or(0); ((b >> (7-(x%8))) & 1) * 255 }
        2 => { let b = row.get(x/4).copied().unwrap_or(0); ((b >> (6-(x%4)*2)) & 3) * 85 }
        4 => { let b = row.get(x/2).copied().unwrap_or(0);
               (if x%2==0 { (b>>4)&0xF } else { b&0xF }) * 17 }
        _ => row.get(x).copied().unwrap_or(0),
    }
}

fn sub_byte_or_8(row: &[u8], x: usize, bit_depth: u8, channels: usize) -> u8 {
    if bit_depth < 8 { sub_byte(row, x, bit_depth) }
    else             { row.get(x * channels).copied().unwrap_or(0) }
}

// ─── PNG Filters ───────────────────────────────────────────

fn unfilter(filter: u8, raw: &[u8], prev: &[u8], bpp: usize) -> Result<Vec<u8>, ImageError> {
    let n = raw.len();
    let mut out = vec![0u8; n];
    match filter {
        0 => out.copy_from_slice(raw),
        1 => for i in 0..n {
                let a = if i >= bpp { out[i-bpp] } else { 0 };
                out[i] = raw[i].wrapping_add(a);
             },
        2 => for i in 0..n {
                let b = prev.get(i).copied().unwrap_or(0);
                out[i] = raw[i].wrapping_add(b);
             },
        3 => for i in 0..n {
                let a = if i >= bpp { out[i-bpp] as u16 } else { 0 };
                let b = prev.get(i).copied().unwrap_or(0) as u16;
                out[i] = raw[i].wrapping_add(((a+b)/2) as u8);
             },
        4 => for i in 0..n {
                let a = if i >= bpp { out[i-bpp] } else { 0 };
                let b = prev.get(i).copied().unwrap_or(0);
                let c = if i >= bpp { prev.get(i-bpp).copied().unwrap_or(0) } else { 0 };
                out[i] = raw[i].wrapping_add(paeth(a,b,c));
             },
        _ => return Err(ImageError(format!("Unknown PNG filter type {}", filter))),
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a,b,c) = (a as i32, b as i32, c as i32);
    let p = a+b-c;
    let pa = (p-a).abs(); let pb = (p-b).abs(); let pc = (p-c).abs();
    if pa<=pb && pa<=pc { a as u8 } else if pb<=pc { b as u8 } else { c as u8 }
}

// ─── DEFLATE ───────────────────────────────────────────────

fn inflate_zlib(data: &[u8]) -> Result<Vec<u8>, ImageError> {
    if data.len() < 6 { return Err(ImageError("ZLIB data too short".into())); }
    inflate_deflate(&data[2..data.len().saturating_sub(4)])
}

fn inflate_deflate(data: &[u8]) -> Result<Vec<u8>, ImageError> {
    let mut bits   = BitReader::new(data);
    let mut output = Vec::with_capacity(65536);
    loop {
        let bfinal = bits.read_bits(1)?;
        let btype  = bits.read_bits(2)?;
        match btype {
            0 => {
                bits.byte_align();
                let len  = bits.read_u16_le()? as usize;
                let nlen = bits.read_u16_le()? as usize;
                if (len ^ nlen) != 0xFFFF {
                    return Err(ImageError("DEFLATE stored block LEN/NLEN mismatch".into()));
                }
                for _ in 0..len { output.push(bits.read_byte()?); }
            }
            1 => { let (l,d) = fixed_trees(); inflate_block(&mut bits,&l,&d,&mut output)?; }
            2 => { let (l,d) = dynamic_trees(&mut bits)?; inflate_block(&mut bits,&l,&d,&mut output)?; }
            _ => return Err(ImageError("DEFLATE invalid block type".into())),
        }
        if bfinal == 1 { break; }
    }
    Ok(output)
}

// ─── Huffman ───────────────────────────────────────────────

struct HuffTree {
    entries: Vec<(u32, u8, u16)>, // (code, len, symbol)
}

impl HuffTree {
    fn build(lengths: &[u8]) -> Self {
        let max_len = lengths.iter().copied().max().unwrap_or(0) as usize;
        let mut bl_count = vec![0u32; max_len + 1];
        for &l in lengths { if l > 0 { bl_count[l as usize] += 1; } }
        let mut next_code = vec![0u32; max_len + 2];
        for bits in 1..=max_len {
            next_code[bits] = (next_code[bits-1] + bl_count[bits-1]) << 1;
        }
        let mut entries = Vec::new();
        for (sym, &len) in lengths.iter().enumerate() {
            if len == 0 { continue; }
            let code = next_code[len as usize];
            next_code[len as usize] += 1;
            entries.push((code, len, sym as u16));
        }
        HuffTree { entries }
    }

    fn decode(&self, bits: &mut BitReader) -> Result<u16, ImageError> {
        let mut code = 0u32;
        let mut len  = 0u8;
        for _ in 0..16 {
            code = (code << 1) | bits.read_bits(1)?;
            len += 1;
            for &(c, l, sym) in &self.entries {
                if l == len && c == code { return Ok(sym); }
            }
        }
        Err(ImageError("Invalid Huffman code in PNG DEFLATE stream".into()))
    }
}

fn fixed_trees() -> (HuffTree, HuffTree) {
    let mut lit = vec![0u8; 288];
    for i in   0..=143 { lit[i] = 8; }
    for i in 144..=255 { lit[i] = 9; }
    for i in 256..=279 { lit[i] = 7; }
    for i in 280..=287 { lit[i] = 8; }
    let dist = vec![5u8; 32];
    (HuffTree::build(&lit), HuffTree::build(&dist))
}

fn dynamic_trees(bits: &mut BitReader) -> Result<(HuffTree, HuffTree), ImageError> {
    let hlit  = bits.read_bits(5)? as usize + 257;
    let hdist = bits.read_bits(5)? as usize + 1;
    let hclen = bits.read_bits(4)? as usize + 4;
    const ORDER: [usize;19] = [16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
    let mut cl = [0u8; 19];
    for i in 0..hclen { cl[ORDER[i]] = bits.read_bits(3)? as u8; }
    let cl_tree = HuffTree::build(&cl);
    let total = hlit + hdist;
    let mut lens = vec![0u8; total];
    let mut i = 0;
    while i < total {
        let sym = cl_tree.decode(bits)?;
        match sym {
            0..=15 => { lens[i] = sym as u8; i += 1; }
            16 => { let rep = bits.read_bits(2)? as usize + 3;
                    let v = if i>0 {lens[i-1]} else {0};
                    for _ in 0..rep { if i<total { lens[i]=v; i+=1; } } }
            17 => { let rep = bits.read_bits(3)? as usize + 3;
                    for _ in 0..rep { if i<total { lens[i]=0; i+=1; } } }
            18 => { let rep = bits.read_bits(7)? as usize + 11;
                    for _ in 0..rep { if i<total { lens[i]=0; i+=1; } } }
            _  => return Err(ImageError("Invalid code-length symbol".into())),
        }
    }
    Ok((HuffTree::build(&lens[..hlit]), HuffTree::build(&lens[hlit..])))
}

fn inflate_block(bits: &mut BitReader, lit: &HuffTree, dist: &HuffTree, out: &mut Vec<u8>) -> Result<(), ImageError> {
    const LB: [u16;29] = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
    const LE: [u8; 29] = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
    const DB: [u32;30] = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
    const DE: [u8; 30] = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];
    loop {
        let sym = lit.decode(bits)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256     => break,
            s @ 257..=285 => {
                let li  = (s-257) as usize;
                let len = LB[li] as usize + bits.read_bits(LE[li] as usize)? as usize;
                let di  = dist.decode(bits)? as usize;
                if di >= 30 { return Err(ImageError("DEFLATE dist code out of range".into())); }
                let dst = DB[di] as usize + bits.read_bits(DE[di] as usize)? as usize;
                if dst > out.len() { return Err(ImageError("DEFLATE invalid back-reference".into())); }
                let start = out.len() - dst;
                for k in 0..len { let b = out[start + k%dst]; out.push(b); }
            }
            _ => return Err(ImageError(format!("DEFLATE bad symbol {}", sym))),
        }
    }
    Ok(())
}

// ─── Bit Reader ────────────────────────────────────────────

struct BitReader<'a> { data: &'a [u8], pos: usize, buf: u64, bits_in: u32 }

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self { BitReader { data, pos:0, buf:0, bits_in:0 } }

    fn refill(&mut self) {
        while self.bits_in <= 56 && self.pos < self.data.len() {
            self.buf |= (self.data[self.pos] as u64) << self.bits_in;
            self.pos += 1; self.bits_in += 8;
        }
    }

    fn read_bits(&mut self, n: usize) -> Result<u32, ImageError> {
        if n == 0 { return Ok(0); }
        self.refill();
        if self.bits_in < n as u32 {
            return Err(ImageError("Unexpected end of DEFLATE stream".into()));
        }
        let v = (self.buf & ((1u64<<n)-1)) as u32;
        self.buf >>= n; self.bits_in -= n as u32;
        Ok(v)
    }

    fn read_byte(&mut self) -> Result<u8, ImageError> { Ok(self.read_bits(8)? as u8) }

    fn read_u16_le(&mut self) -> Result<u16, ImageError> {
        let lo = self.read_byte()? as u16;
        let hi = self.read_byte()? as u16;
        Ok(lo | (hi<<8))
    }

    fn byte_align(&mut self) {
        let rem = self.bits_in % 8;
        if rem > 0 { self.buf >>= rem; self.bits_in -= rem; }
    }
}