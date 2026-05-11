// ============================================================
// RetroX Paint - Drawing Primitives
// All drawing operations on PixelBuffer.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use crate::platform::PixelBuffer;
use crate::font;
use crate::image::Image;

// ─── Colors ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    // RetroX color palette
    pub const BG:          Self = Self::rgb(18,  18,  24);
    pub const FG:          Self = Self::rgb(220, 220, 220);
    pub const H1:          Self = Self::rgb(255, 200, 80);
    pub const H2:          Self = Self::rgb(100, 200, 140);
    pub const H3:          Self = Self::rgb(200, 200, 255);
    pub const LINK:        Self = Self::rgb(80,  160, 255);
    pub const LINK_HOVER:  Self = Self::rgb(140, 200, 255);
    pub const LINK_UNDERLINE: Self = Self::rgb(60, 120, 200);
    pub const IMAGE_BG:    Self = Self::rgb(30,  30,  40);
    pub const IMAGE_BORDER:Self = Self::rgb(60,  60,  80);
    pub const SCROLLBAR:   Self = Self::rgb(60,  60,  80);
    pub const SCROLLTHUMB: Self = Self::rgb(100, 100, 130);
    pub const DIVIDER:     Self = Self::rgb(50,  50,  65);
    pub const NAV_BG:      Self = Self::rgb(12,  12,  18);
    pub const NAV_BTN:     Self = Self::rgb(40,  40,  60);
    pub const NAV_BTN_HOVER: Self = Self::rgb(70, 70, 100);
    pub const NAV_TEXT:    Self = Self::rgb(180, 180, 200);
    pub const STATUS_BG:   Self = Self::rgb(12,  12,  18);
    pub const STATUS_TEXT: Self = Self::rgb(120, 120, 150);
}

// ─── Drawing Primitives ────────────────────────────────────

pub fn fill_rect(
    buf: &mut PixelBuffer,
    x: i32, y: i32,
    w: i32, h: i32,
    color: Color,
) {
    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let x1 = (x + w).min(buf.width as i32) as u32;
    let y1 = (y + h).min(buf.height as i32) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            buf.set_pixel(px, py, color.r, color.g, color.b, color.a);
        }
    }
}

pub fn draw_rect(
    buf: &mut PixelBuffer,
    x: i32, y: i32,
    w: i32, h: i32,
    color: Color,
) {
    fill_rect(buf, x,         y,         w, 1, color);
    fill_rect(buf, x,         y + h - 1, w, 1, color);
    fill_rect(buf, x,         y,         1, h, color);
    fill_rect(buf, x + w - 1, y,         1, h, color);
}

pub fn draw_text(
    buf:   &mut PixelBuffer,
    text:  &str,
    x:     i32,
    y:     i32,
    color: Color,
    scale: u32,
) {
    if y < -(font::GLYPH_H as i32 * scale as i32) { return; }
    if y > buf.height as i32 { return; }
    font::draw_str(buf, text, x as u32, y as u32, color.r, color.g, color.b, scale);
}

pub fn draw_text_wrapped(
    buf:    &mut PixelBuffer,
    text:   &str,
    x:      i32,
    y:      i32,
    max_w:  i32,
    color:  Color,
    scale:  u32,
) -> i32 {
    let glyph_w = (font::GLYPH_W * scale) as i32;
    let glyph_h = (font::GLYPH_H * scale) as i32;
    let chars_per_line = (max_w / glyph_w).max(1) as usize;

    let mut cy = y;
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut line = String::new();

    for word in &words {
        if line.is_empty() {
            line.push_str(word);
        } else if line.len() + 1 + word.len() <= chars_per_line {
            line.push(' ');
            line.push_str(word);
        } else {
            draw_text(buf, &line, x, cy, color, scale);
            cy += glyph_h + 2;
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        draw_text(buf, &line, x, cy, color, scale);
        cy += glyph_h + 2;
    }

    cy
}

pub fn draw_image(
    buf:    &mut PixelBuffer,
    image:  &Image,
    x:      i32,
    y:      i32,
    w:      i32,
    h:      i32,
) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(buf.width as i32);
    let y1 = (y + h).min(buf.height as i32);

    for py in y0..y1 {
        for px in x0..x1 {
            let sx = ((px - x) as f32 / w as f32 * image.width  as f32) as u32;
            let sy = ((py - y) as f32 / h as f32 * image.height as f32) as u32;
            let (r, g, b, a) = image.get_pixel(sx, sy);
            if a > 0 {
                buf.set_pixel(px as u32, py as u32, r, g, b, a);
            }
        }
    }
}

pub fn draw_image_placeholder(
    buf:  &mut PixelBuffer,
    alt:  &str,
    x:    i32,
    y:    i32,
    w:    i32,
    h:    i32,
) {
    fill_rect(buf, x, y, w, h, Color::IMAGE_BG);
    draw_rect(buf, x, y, w, h, Color::IMAGE_BORDER);

    let label = format!("[IMG] {}", &alt[..alt.len().min(30)]);
    let tx = x + 8;
    let ty = y + h / 2 - 4;
    if ty > y && ty < y + h {
        draw_text(buf, &label, tx, ty, Color::FG, 1);
    }
}

pub fn draw_horizontal_rule(
    buf:   &mut PixelBuffer,
    x:     i32,
    y:     i32,
    w:     i32,
    color: Color,
) {
    fill_rect(buf, x, y, w, 1, color);
}

pub fn draw_scrollbar(
    buf:        &mut PixelBuffer,
    scroll_y:   i32,
    content_h:  i32,
    viewport_h: i32,
) {
    let bar_w   = 8i32;
    let bar_x   = buf.width as i32 - bar_w;
    let bar_h   = viewport_h;

    fill_rect(buf, bar_x, 0, bar_w, bar_h, Color::SCROLLBAR);

    if content_h > viewport_h {
        let thumb_h = ((viewport_h as f32 / content_h as f32) * bar_h as f32)
            .max(20.0) as i32;
        let thumb_y = ((scroll_y as f32 / content_h as f32) * bar_h as f32) as i32;
        fill_rect(buf, bar_x, thumb_y, bar_w, thumb_h, Color::SCROLLTHUMB);
    }
}

pub fn draw_link_underline(
    buf:   &mut PixelBuffer,
    x:     i32,
    y:     i32,
    w:     i32,
    hover: bool,
) {
    let color = if hover { Color::LINK_HOVER } else { Color::LINK_UNDERLINE };
    fill_rect(buf, x, y, w, 1, color);
}
