// ============================================================
// RetroX Bitmap Font Renderer
// Embedded 8x8 monospace bitmap font.
// No external dependencies. Frozen.
// Rust 1.95.0 | Edition 2021
// ============================================================

mod bitmap;
pub use bitmap::FONT_8X8;

pub const GLYPH_W: u32 = 8;
pub const GLYPH_H: u32 = 8;

/// Draw a single character into a pixel buffer at (x, y)
pub fn draw_char(
    buf:    &mut crate::platform::PixelBuffer,
    ch:     char,
    x:      u32,
    y:      u32,
    r:      u8,
    g:      u8,
    b:      u8,
    scale:  u32,
) {
    let idx = glyph_index(ch);
    let glyph = &FONT_8X8[idx];

    for row in 0..8u32 {
        for col in 0..8u32 {
            let bit = (glyph[row as usize] >> col) & 1;
            if bit == 1 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        buf.set_pixel(
                            x + col * scale + sx,
                            y + row * scale + sy,
                            r, g, b, 255
                        );
                    }
                }
            }
        }
    }
}

/// Draw a string into a pixel buffer
pub fn draw_str(
    buf:    &mut crate::platform::PixelBuffer,
    text:   &str,
    x:      u32,
    y:      u32,
    r:      u8,
    g:      u8,
    b:      u8,
    scale:  u32,
) -> u32 {
    let mut cx = x;
    for ch in text.chars() {
        if ch == '\n' { break; }
        draw_char(buf, ch, cx, y, r, g, b, scale);
        cx += GLYPH_W * scale;
    }
    cx
}

/// Measure text width in pixels
pub fn measure_str(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * GLYPH_W * scale
}

fn glyph_index(ch: char) -> usize {
    let c = ch as usize;
    if c >= 32 && c < 127 { c - 32 } else { 0 }
}
