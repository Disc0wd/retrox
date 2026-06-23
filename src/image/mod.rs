// ============================================================
// RetroX Image Decoder
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

pub mod stb;

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

#[derive(Debug)]
pub struct ImageError(pub String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Image Error] {}", self.0)
    }
}

pub fn load_image(path: &str) -> Result<Image, ImageError> {
    let data = std::fs::read(path)
        .map_err(|e| ImageError(format!("Cannot read '{}': {}", path, e)))?;

    stb::decode_stb(&data)
        .map_err(|e| ImageError(format!("STB decode failed for '{}': {}", path, e)))
}