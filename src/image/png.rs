// ============================================================
// RetroX PNG Decoder
// Implements PNG spec (ISO/IEC 15948:2004). Zero dependencies.
// Supports: RGB, RGBA, Grayscale, Indexed color, bit depths 1-16
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use super::{Image, ImageError};

// ─── Public Entry Point ────────────────────────────────────

pub fn decode_png(data: &[u8]) -> Result<Image, ImageError> {
    let mut r = PngReader::new(data);
    r.decode()
}

// ─── PNG Constants ─────────────────────────────────────────

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

// Color types
const CT_GRAYSCALE:  u8 = 0;
const CT_RGB:        u8 = 2;
const CT_INDEXED:    u8 = 3;
const CT_GRAY_ALPHA: u8 = 4;
const CT_RGBA:       u8 = 6;

// Filter types
const FILTER_NONE:    u8 = 0;
const FILTER_SUB:     u8 = 1;
const FILTER_UP:      u8 = 2;
const FILTER_AVERAGE: u8 = 3;
const FILTER_PAETH:   u8 = 4;

// ─── PNG Reader ────────────────────────────────────────────

struct PngReader<'a> {
    data:       &'a [u8],
    pos:        usize,
    width:      u32,
    height:     u32,
    bit_depth:  u8,
    color_type: u8,
    palette:    Vec<(u8, u8, u8)>,
    idat:       Vec<u8>,
}

impl<'a> PngReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        PngReader {
            data, pos: 0,
            width: 0, height: 0,
            bit_depth: 0, color_type: 0,
            palette: Vec::new(),
            idat: Vec::new(),
        }
    }

    fn read_u8(&mut self) -> Result<u8, ImageError> {
        if self.pos >= self.data.len() {
            return Err(ImageError("Unexpected end of PNG data".into()));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_u32_be(&mut self) -> Result<u32, ImageError> {
        let b0 = self.read_u8()? as u32;
        let b1 = self.read_u8()? as u32;
        let b2 = self.read_u8()? as u32;
        let b3 = self.read_u8()? as u32;
        Ok((b0 << 24) | (b1 << 16) | (b2 << 8) | b3)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ImageError> {
        if self.pos + n > self.data.len() {
            return Err(ImageError("Unexpected end of PNG data".into()));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<(), ImageError> {
        if self.pos + n > self.data.len() {
            return Err(ImageError("Unexpected end of PNG data".into()));
        }
        self.pos += n;
        Ok(())
    }

    fn decode(&mut self) -> Result<Image, ImageError> {
        // Verify signature
        let sig = self.read_bytes(8)?;
        if sig != PNG_SIGNATURE {
            return Err(ImageError("Not a valid PNG file".into()));
        }

        // Parse chunks
        loop {
            let length = self.read_u32_be()? as usize;
            let chunk_type = self.read_bytes(4)?;
            let tag = [chunk_type[0], chunk_type[1], chunk_type[2], chunk_type[3]];

            match &tag {
                b"IHDR" => self.parse_ihdr()?,
                b"PLTE" => self.parse_plte(length)?,
                b"IDAT" => {
                    let d = self.read_bytes(length)?.to_vec();
                    self.idat.extend_from_slice(&d);
                    self.skip(4)?; // CRC
                    continue;
                }
                b"IEND" => {
                    self.skip(length)?;
                    self.skip(4)?;
                    break;
                }
                _ => {
                    self.skip(length)?;
                }
            }
            self.skip(4)?; // CRC
        }

        // Decompress IDAT
        let decompressed = inflate_zlib(&self.idat)?;

        // Reconstruct image
        self.reconstruct(&decompressed)
    }

    fn parse_ihdr(&mut self) -> Result<(), ImageError> {
        self.width      = self.read_u32_be()?;
        self.height     = self.read_u32_be()?;
        self.bit_depth  = self.read_u8()?;
        self.color_type = self.read_u8()?;
        let _compression = self.read_u8()?;
        let _filter      = self.read_u8()?;
        let interlace    = self.read_u8()?;
        if interlace != 0 {
            return Err(ImageError("Interlaced PNG not supported".into()));
        }
        Ok(())
    }

    fn parse_plte(&mut self, length: usize) -> Result<(), ImageError> {
        if length % 3 != 0 {
            return Err(ImageError("Invalid PLTE chunk length".into()));
        }
        let data = self.read_bytes(length)?.to_vec();
        self.palette.clear();
        for i in 0..length / 3 {
            self.palette.push((data[i*3], data[i*3+1], data[i*3+2]));
        }
        Ok(())
    }

    fn reconstruct(&self, filtered: &[u8]) -> Result<Image, ImageError> {
        let channels: u32 = match self.color_type {
            CT_GRAYSCALE  => 1,
            CT_RGB        => 3,
            CT_INDEXED    => 1,
            CT_GRAY_ALPHA => 2,
            CT_RGBA       => 4,
            _ => return Err(ImageError(format!("Unsupported PNG color type {}", self.color_type))),
        };

        let bytes_per_channel = ((self.bit_depth as u32 + 7) / 8).max(1);
        let stride = self.width * channels * bytes_per_channel;
        let row_len = (stride + 1) as usize; // +1 for filter byte

        let mut raw_rows: Vec<Vec<u8>> = Vec::with_capacity(self.height as usize);
        let mut pos = 0usize;

        for y in 0..self.height as usize {
            if pos + row_len > filtered.len() {
                return Err(ImageError("PNG data truncated".into()));
            }

            let filter_type = filtered[pos];
            let row_data    = &filtered[pos+1..pos+row_len];
            pos += row_len;

            let prev = if y > 0 { &raw_rows[y-1] } else { &[][..] };
            let reconstructed = apply_filter(filter_type, row_data, prev, (channels * bytes_per_channel) as usize)?;
            raw_rows.push(reconstructed);
        }

        // Convert to RGBA image
        let mut image = Image::new(self.width, self.height);

        for y in 0..self.height {
            let row = &raw_rows[y as usize];
            for x in 0..self.width {
                let (r, g, b, a) = self.pixel_to_rgba(row, x)?;
                image.set_pixel(x, y, r, g, b, a);
            }
        }

        Ok(image)
    }

    fn pixel_to_rgba(&self, row: &[u8], x: u32) -> Result<(u8,u8,u8,u8), ImageError> {
        match self.color_type {
            CT_GRAYSCALE => {
                let v = sample(row, x, 0, self.bit_depth);
                Ok((v, v, v, 255))
            }
            CT_RGB => {
                let r = sample(row, x, 0, self.bit_depth);
                let g = sample(row, x, 1, self.bit_depth);
                let b = sample(row, x, 2, self.bit_depth);
                Ok((r, g, b, 255))
            }
            CT_INDEXED => {
                let idx = sample(row, x, 0, self.bit_depth) as usize;
                let (r,g,b) = self.palette.get(idx).copied().unwrap_or((0,0,0));
                Ok((r, g, b, 255))
            }
            CT_GRAY_ALPHA => {
                let v = sample(row, x, 0, self.bit_depth);
                let a = sample(row, x, 1, self.bit_depth);
                Ok((v, v, v, a))
            }
            CT_RGBA => {
                let r = sample(row, x, 0, self.bit_depth);
                let g = sample(row, x, 1, self.bit_depth);
                let b = sample(row, x, 2, self.bit_depth);
                let a = sample(row, x, 3, self.bit_depth);
                Ok((r, g, b, a))
            }
            _ => Err(ImageError("Unsupported color type".into()))
        }
    }
}

// ─── Filter Reconstruction ─────────────────────────────────

fn apply_filter(
    filter: u8,
    row:    &[u8],
    prev:   &[u8],
    bpp:    usize,
) -> Result<Vec<u8>, ImageError> {
    let mut out = vec![0u8; row.len()];

    match filter {
        FILTER_NONE => {
            out.copy_from_slice(row);
        }
        FILTER_SUB => {
            for i in 0..row.len() {
                let a = if i >= bpp { out[i - bpp] } else { 0 };
                out[i] = row[i].wrapping_add(a);
            }
        }
        FILTER_UP => {
            for i in 0..row.len() {
                let b = if !prev.is_empty() { prev[i] } else { 0 };
                out[i] = row[i].wrapping_add(b);
            }
        }
        FILTER_AVERAGE => {
            for i in 0..row.len() {
                let a = if i >= bpp { out[i - bpp] as u16 } else { 0 };
                let b = if !prev.is_empty() { prev[i] as u16 } else { 0 };
                out[i] = row[i].wrapping_add(((a + b) / 2) as u8);
            }
        }
        FILTER_PAETH => {
            for i in 0..row.len() {
                let a = if i >= bpp { out[i - bpp] } else { 0 };
                let b = if !prev.is_empty() { prev[i] } else { 0 };
                let c = if !prev.is_empty() && i >= bpp { prev[i - bpp] } else { 0 };
                out[i] = row[i].wrapping_add(paeth(a, b, c));
            }
        }
        _ => return Err(ImageError(format!("Unknown PNG filter type {}", filter))),
    }

    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i32;
    let b = b as i32;
    let c = c as i32;
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc { a as u8 }
    else if pb <= pc { b as u8 }
    else { c as u8 }
}

fn sample(row: &[u8], x: u32, channel: u32, bit_depth: u8) -> u8 {
    match bit_depth {
        8 => {
            let channels = match channel { 0|1|2|3 => channel, _ => 0 };
            let idx = (x * (channel + 1).max(1)) as usize;
            if idx < row.len() { row[idx] } else { 0 }
        }
        16 => {
            let idx = (x * 2 * (channel + 1)) as usize;
            if idx + 1 < row.len() { row[idx] } else { 0 }
        }
        1 | 2 | 4 => {
            let bits_per_pixel = bit_depth as u32;
            let pixels_per_byte = 8 / bits_per_pixel;
            let byte_idx = (x / pixels_per_byte) as usize;
            let bit_offset = 8 - bit_depth as u32 * ((x % pixels_per_byte) + 1);
            let mask = (1u8 << bit_depth) - 1;
            let val = if byte_idx < row.len() {
                (row[byte_idx] >> bit_offset) & mask
            } else { 0 };
            // Scale to 8-bit
            match bit_depth {
                1 => val * 255,
                2 => val * 85,
                4 => val * 17,
                _ => val,
            }
        }
        _ => 0,
    }
}

// ─── DEFLATE/ZLIB Decompressor ─────────────────────────────

fn inflate_zlib(data: &[u8]) -> Result<Vec<u8>, ImageError> {
    if data.len() < 2 {
        return Err(ImageError("ZLIB data too short".into()));
    }
    // Skip zlib header (2 bytes)
    inflate_raw(&data[2..data.len().saturating_sub(4)])
}

fn inflate_raw(data: &[u8]) -> Result<Vec<u8>, ImageError> {
    let mut bits   = BitReader::new(data);
    let mut output = Vec::new();

    loop {
        let bfinal = bits.read_bits(1)?;
        let btype  = bits.read_bits(2)?;

        match btype {
            0 => {
                // Uncompressed block
                bits.align_to_byte();
                let len  = bits.read_u16_le()? as usize;
                let nlen = bits.read_u16_le()? as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err(ImageError("Invalid stored block length".into()));
                }
                for _ in 0..len {
                    output.push(bits.read_byte()?);
                }
            }
            1 => {
                // Fixed Huffman
                let (lit_tree, dist_tree) = fixed_huffman_trees();
                inflate_block(&mut bits, &lit_tree, &dist_tree, &mut output)?;
            }
            2 => {
                // Dynamic Huffman
                let (lit_tree, dist_tree) = read_dynamic_trees(&mut bits)?;
                inflate_block(&mut bits, &lit_tree, &dist_tree, &mut output)?;
            }
            _ => return Err(ImageError("Invalid DEFLATE block type".into())),
        }

        if bfinal == 1 { break; }
    }

    Ok(output)
}

// ─── Huffman Trees ─────────────────────────────────────────

struct HuffTree {
    codes: Vec<(u16, u8, u16)>, // (symbol, bit_len, code)
}

impl HuffTree {
    fn from_lengths(lengths: &[u8]) -> Self {
        let max_bits = *lengths.iter().max().unwrap_or(&0) as usize;
        let mut bl_count = vec![0u32; max_bits + 1];
        for &l in lengths { if l > 0 { bl_count[l as usize] += 1; } }

        let mut next_code = vec![0u16; max_bits + 2];
        let mut code = 0u16;
        for bits in 1..=max_bits {
            code = (code + bl_count[bits - 1] as u16) << 1;
            next_code[bits] = code;
        }

        let mut codes = Vec::new();
        for (sym, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let c = next_code[len as usize];
                next_code[len as usize] += 1;
                codes.push((sym as u16, len, c));
            }
        }

        HuffTree { codes }
    }

    fn decode(&self, bits: &mut BitReader) -> Result<u16, ImageError> {
        let mut code = 0u16;
        let mut len  = 0u8;

        for _ in 0..16 {
            code = (code << 1) | bits.read_bits(1)? as u16;
            len += 1;
            for &(sym, slen, scode) in &self.codes {
                if slen == len && scode == code {
                    return Ok(sym);
                }
            }
        }

        Err(ImageError("Invalid Huffman code".into()))
    }
}

fn fixed_huffman_trees() -> (HuffTree, HuffTree) {
    let mut lit_lengths = vec![0u8; 288];
    for i in 0..=143  { lit_lengths[i] = 8; }
    for i in 144..=255 { lit_lengths[i] = 9; }
    for i in 256..=279 { lit_lengths[i] = 7; }
    for i in 280..=287 { lit_lengths[i] = 8; }

    let dist_lengths = vec![5u8; 32];

    (HuffTree::from_lengths(&lit_lengths), HuffTree::from_lengths(&dist_lengths))
}

fn read_dynamic_trees(bits: &mut BitReader) -> Result<(HuffTree, HuffTree), ImageError> {
    let hlit  = bits.read_bits(5)? as usize + 257;
    let hdist = bits.read_bits(5)? as usize + 1;
    let hclen = bits.read_bits(4)? as usize + 4;

    let order = [16u8,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
    let mut cl_lengths = vec![0u8; 19];
    for i in 0..hclen {
        cl_lengths[order[i] as usize] = bits.read_bits(3)? as u8;
    }
    let cl_tree = HuffTree::from_lengths(&cl_lengths);

    let total = hlit + hdist;
    let mut lengths = vec![0u8; total];
    let mut i = 0;

    while i < total {
        let sym = cl_tree.decode(bits)?;
        match sym {
            0..=15 => { lengths[i] = sym as u8; i += 1; }
            16 => {
                let rep = bits.read_bits(2)? as usize + 3;
                let val = if i > 0 { lengths[i-1] } else { 0 };
                for _ in 0..rep { if i < total { lengths[i] = val; i += 1; } }
            }
            17 => {
                let rep = bits.read_bits(3)? as usize + 3;
                for _ in 0..rep { if i < total { lengths[i] = 0; i += 1; } }
            }
            18 => {
                let rep = bits.read_bits(7)? as usize + 11;
                for _ in 0..rep { if i < total { lengths[i] = 0; i += 1; } }
            }
            _ => return Err(ImageError("Invalid code length symbol".into())),
        }
    }

    Ok((
        HuffTree::from_lengths(&lengths[..hlit]),
        HuffTree::from_lengths(&lengths[hlit..]),
    ))
}

fn inflate_block(
    bits:      &mut BitReader,
    lit_tree:  &HuffTree,
    dist_tree: &HuffTree,
    output:    &mut Vec<u8>,
) -> Result<(), ImageError> {
    const LENGTH_BASE:  [u16; 29] = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
    const LENGTH_EXTRA: [u8;  29] = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
    const DIST_BASE:    [u16; 30] = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
    const DIST_EXTRA:   [u8;  30] = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];

    loop {
        let sym = lit_tree.decode(bits)?;
        match sym {
            0..=255 => output.push(sym as u8),
            256     => break,
            257..=285 => {
                let len_idx = (sym - 257) as usize;
                let extra   = bits.read_bits(LENGTH_EXTRA[len_idx] as usize)? as u16;
                let length  = (LENGTH_BASE[len_idx] + extra) as usize;

                let dist_sym = dist_tree.decode(bits)? as usize;
                let dextra   = bits.read_bits(DIST_EXTRA[dist_sym] as usize)? as u16;
                let distance = (DIST_BASE[dist_sym] + dextra) as usize;

                let start = output.len().saturating_sub(distance);
                for i in 0..length {
                    let byte = output[start + (i % distance)];
                    output.push(byte);
                }
            }
            _ => return Err(ImageError("Invalid literal/length symbol".into())),
        }
    }

    Ok(())
}

// ─── Bit Reader ────────────────────────────────────────────

struct BitReader<'a> {
    data:     &'a [u8],
    pos:      usize,
    bit_buf:  u32,
    bits_in:  u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, bit_buf: 0, bits_in: 0 }
    }

    fn read_bits(&mut self, n: usize) -> Result<u32, ImageError> {
        while self.bits_in < n as u32 {
            if self.pos >= self.data.len() {
                return Err(ImageError("Unexpected end of deflate stream".into()));
            }
            self.bit_buf |= (self.data[self.pos] as u32) << self.bits_in;
            self.pos     += 1;
            self.bits_in += 8;
        }
        let val = self.bit_buf & ((1 << n) - 1);
        self.bit_buf >>= n;
        self.bits_in -= n as u32;
        Ok(val)
    }

    fn read_byte(&mut self) -> Result<u8, ImageError> {
        Ok(self.read_bits(8)? as u8)
    }

    fn read_u16_le(&mut self) -> Result<u16, ImageError> {
        let lo = self.read_byte()? as u16;
        let hi = self.read_byte()? as u16;
        Ok(lo | (hi << 8))
    }

    fn align_to_byte(&mut self) {
        let rem = self.bits_in % 8;
        if rem > 0 {
            self.bit_buf >>= rem;
            self.bits_in -= rem;
        }
    }
}
