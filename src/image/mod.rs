// ============================================================
// RetroX Image Decoder
// Supports PNG and JPEG. Zero external dependencies.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

mod png;
mod jpeg;

pub use png::decode_png;
pub use jpeg::decode_jpeg;

#[derive(Debug, Clone)]
pub struct Image {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA, 4 bytes per pixel
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Image {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x >= self.width || y >= self.height { return; }
        let idx = ((y * self.width + x) * 4) as usize;
        self.pixels[idx]     = r;
        self.pixels[idx + 1] = g;
        self.pixels[idx + 2] = b;
        self.pixels[idx + 3] = a;
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        if x >= self.width || y >= self.height { return (0, 0, 0, 255); }
        let idx = ((y * self.width + x) * 4) as usize;
        (
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        )
    }
}

#[derive(Debug)]
pub struct ImageError(pub String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Image Error] {}", self.0)
    }
}

/// Load an image from file, auto-detecting format
pub fn load_image(path: &str) -> Result<Image, ImageError> {
    let data = std::fs::read(path).map_err(|e| {
        ImageError(format!("Cannot read file '{}': {}", path, e))
    })?;

    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        decode_png(&data)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg(&data)
    } else {
        Err(ImageError(format!(
            "Unsupported image format for '{}'. Supported: PNG, JPEG", path
        )))
    }
}
