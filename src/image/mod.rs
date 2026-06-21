// ============================================================
// RetroX Image Decoder
// Supports PNG (ISO/IEC 15948) and JPEG (ISO/IEC 10918-1).
// Zero external dependencies. No compression on output pixels.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

mod png;
mod jpeg;

pub use png::decode_png;
pub use jpeg::decode_jpeg;

// ─── Image ─────────────────────────────────────────────────

/// Decoded image. Pixels stored as RGBA, 4 bytes per pixel,
/// row-major, top-to-bottom, left-to-right. No compression.
#[derive(Debug, Clone)]
pub struct Image {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Image {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x >= self.width || y >= self.height { return; }
        let i = ((y * self.width + x) * 4) as usize;
        self.pixels[i]     = r;
        self.pixels[i + 1] = g;
        self.pixels[i + 2] = b;
        self.pixels[i + 3] = a;
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        if x >= self.width || y >= self.height { return (0, 0, 0, 255); }
        let i = ((y * self.width + x) * 4) as usize;
        (self.pixels[i], self.pixels[i+1], self.pixels[i+2], self.pixels[i+3])
    }
}

// ─── Error ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct ImageError(pub String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Image Error] {}", self.0)
    }
}

impl From<&str> for ImageError {
    fn from(s: &str) -> Self { ImageError(s.to_string()) }
}

// ─── Loader ────────────────────────────────────────────────

/// Load an image from a file path, auto-detecting PNG or JPEG.
pub fn load_image(path: &str) -> Result<Image, ImageError> {
    let data = std::fs::read(path)
        .map_err(|e| ImageError(format!("Cannot read '{}': {}", path, e)))?;

    if data.len() < 4 {
        return Err(ImageError(format!("File '{}' is too small to be an image", path)));
    }

    // PNG magic: 89 50 4E 47
    if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
        return decode_png(&data)
            .map_err(|e| ImageError(format!("PNG decode failed for '{}': {}", path, e.0)));
    }

    // JPEG magic: FF D8 FF
    if data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
        return decode_jpeg(&data)
            .map_err(|e| ImageError(format!("JPEG decode failed for '{}': {}", path, e.0)));
    }

    Err(ImageError(format!(
        "Unsupported image format for '{}'. Supported: PNG, JPEG (jpg/jpeg)", path
    )))
}