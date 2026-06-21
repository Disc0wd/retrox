// ============================================================
// RetroX Layout Engine
// Converts AST nodes into a flat list of render elements.
// Images are decoded in parallel threads and joined before layout.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use crate::ast::Node;
use crate::image::{Image, load_image};
use crate::font;

pub const MARGIN_X:   i32 = 40;
pub const LINE_H:     i32 = 20;
pub const PARA_GAP:   i32 = 14;
pub const H1_SCALE:   u32 = 3;
pub const H2_SCALE:   u32 = 2;
pub const H3_SCALE:   u32 = 2;
pub const BODY_SCALE: u32 = 1;
pub const NAV_H:      i32 = 32;
pub const STATUS_H:   i32 = 20;

// ─── Render Elements ───────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Element {
    Heading {
        text:  String,
        level: u8,
        y:     i32,
    },
    Paragraph {
        text: String,
        y:    i32,
        h:    i32,
    },
    Image {
        path:  String,
        alt:   String,
        y:     i32,
        w:     i32,
        h:     i32,
        image: Option<Image>,
    },
    Link {
        text:   String,
        target: String,
        y:      i32,
        x:      i32,
        w:      i32,
    },
    HRule {
        y: i32,
    },
    Spacer {
        y: i32,
        h: i32,
    },
}

impl Element {
    pub fn y(&self) -> i32 {
        match self {
            Element::Heading   { y, .. } => *y,
            Element::Paragraph { y, .. } => *y,
            Element::Image     { y, .. } => *y,
            Element::Link      { y, .. } => *y,
            Element::HRule     { y }     => *y,
            Element::Spacer    { y, .. } => *y,
        }
    }

    pub fn height(&self, _viewport_w: i32) -> i32 {
        match self {
            Element::Heading { level, .. } => {
                let scale = match level { 1 => H1_SCALE, 2 => H2_SCALE, _ => H3_SCALE };
                font::GLYPH_H as i32 * scale as i32 + PARA_GAP * 2
            }
            Element::Paragraph { h, .. } => *h,
            Element::Image     { h, .. } => *h + PARA_GAP,
            Element::Link      { .. }    => LINE_H + 4,
            Element::HRule     { .. }    => 8,
            Element::Spacer    { h, .. } => *h,
        }
    }
}

// ─── Image Job ─────────────────────────────────────────────

struct ImageJob {
    path:      String,
    alt:       String,
    full_path: String,
    target_w:  u32,
    target_h_hint: u32, // max height hint
}

struct ImageResult {
    path:  String,
    alt:   String,
    image: Option<Image>,
    w:     i32,
    h:     i32,
}

// ─── Layout Pass ───────────────────────────────────────────

pub struct Layout {
    pub elements:  Vec<Element>,
    pub content_h: i32,
}

impl Layout {
    pub fn build(node: &Node, viewport_w: i32, asset_base: &str) -> Self {
        // First pass: collect image jobs
        let mut jobs: Vec<ImageJob> = Vec::new();
        collect_image_jobs(node, viewport_w, asset_base, &mut jobs);

        // Decode all images in parallel
        let results = decode_images_parallel(jobs);

        // Build a map from path -> result for lookup during layout
        let mut image_map: std::collections::HashMap<String, ImageResult> =
            std::collections::HashMap::new();
        for r in results {
            image_map.insert(r.path.clone(), r);
        }

        // Second pass: build element list
        let mut elements = Vec::new();
        let mut y = NAV_H + 16;

        match node {
            Node::Document { children, .. } => {
                for child in children {
                    layout_node(child, &mut elements, &mut y, viewport_w, &mut image_map);
                }
            }
            _ => layout_node(node, &mut elements, &mut y, viewport_w, &mut image_map),
        }

        let content_h = y + 32;
        Layout { elements, content_h }
    }
}

// ─── Collect Image Jobs ────────────────────────────────────

fn collect_image_jobs(node: &Node, viewport_w: i32, asset_base: &str, jobs: &mut Vec<ImageJob>) {
    match node {
        Node::Document { children, .. }
        | Node::Container { children, .. }
        | Node::Section   { children, .. } => {
            for child in children {
                collect_image_jobs(child, viewport_w, asset_base, jobs);
            }
        }
        Node::Image { path, alt, .. } => {
            let full_path = if asset_base.is_empty() {
                path.clone()
            } else {
                format!("{}/{}", asset_base.trim_end_matches('/'), path)
            };
            let target_w = (viewport_w - MARGIN_X * 2).max(1) as u32;
            jobs.push(ImageJob {
                path:          path.clone(),
                alt:           alt.clone(),
                full_path,
                target_w,
                target_h_hint: 800,
            });
        }
        _ => {}
    }
}

// ─── Parallel Decode ───────────────────────────────────────

fn decode_images_parallel(jobs: Vec<ImageJob>) -> Vec<ImageResult> {
    use std::thread;

    let handles: Vec<_> = jobs.into_iter().map(|job| {
        thread::spawn(move || {
            match load_image(&job.full_path) {
                Ok(img) => {
                    let scale_w = job.target_w as f32 / img.width as f32;
                    let scale_h = job.target_h_hint as f32 / img.height as f32;
                    let scale   = scale_w.min(scale_h).min(1.0); // never upscale
                    let sw = ((img.width  as f32 * scale) as u32).max(1);
                    let sh = ((img.height as f32 * scale) as u32).max(1);
                    let scaled = scale_image(&img, sw, sh);
                    ImageResult {
                        path:  job.path,
                        alt:   job.alt,
                        image: Some(scaled),
                        w:     sw as i32,
                        h:     sh as i32,
                    }
                }
                Err(e) => {
                    eprintln!("[RetroX] Image load failed '{}': {}", job.full_path, e);
                    let w = job.target_w as i32;
                    ImageResult { path: job.path, alt: job.alt, image: None, w, h: 120 }
                }
            }
        })
    }).collect();

    handles.into_iter()
        .filter_map(|h| h.join().ok())
        .collect()
}

// ─── Layout Node ───────────────────────────────────────────

fn layout_node(
    node:      &Node,
    elements:  &mut Vec<Element>,
    y:         &mut i32,
    vw:        i32,
    image_map: &mut std::collections::HashMap<String, ImageResult>,
) {
    match node {
        Node::Container { children, .. } | Node::Section { children, .. } => {
            for child in children {
                layout_node(child, elements, y, vw, image_map);
            }
        }

        Node::Heading { level, text, .. } => {
            let scale = match level { 1 => H1_SCALE, 2 => H2_SCALE, _ => H3_SCALE };
            *y += PARA_GAP;
            if *level == 1 {
                elements.push(Element::HRule { y: *y });
                *y += 8;
            }
            elements.push(Element::Heading { text: text.clone(), level: *level, y: *y });
            *y += font::GLYPH_H as i32 * scale as i32 + PARA_GAP;
            if *level == 1 {
                elements.push(Element::HRule { y: *y });
                *y += 8;
            }
        }

        Node::Paragraph { text, .. } => {
            let max_w     = vw - MARGIN_X * 2 - 16;
            let glyph_w   = (font::GLYPH_W * BODY_SCALE) as i32;
            let glyph_h   = (font::GLYPH_H * BODY_SCALE) as i32;
            let chars_per = (max_w / glyph_w).max(1) as usize;

            let words: Vec<&str> = text.split_whitespace().collect();
            let mut lines = 1usize;
            let mut line_len = 0usize;
            for word in &words {
                if line_len == 0 {
                    line_len = word.len();
                } else if line_len + 1 + word.len() <= chars_per {
                    line_len += 1 + word.len();
                } else {
                    lines   += 1;
                    line_len = word.len();
                }
            }

            let h = lines as i32 * (glyph_h + 2) + PARA_GAP;
            elements.push(Element::Paragraph { text: text.clone(), y: *y, h });
            *y += h;
        }

        Node::Image { path, alt, .. } => {
            if let Some(result) = image_map.remove(path.as_str()) {
                elements.push(Element::Image {
                    path:  result.path,
                    alt:   result.alt,
                    y:     *y,
                    w:     result.w,
                    h:     result.h,
                    image: result.image,
                });
                *y += result.h + PARA_GAP;
            } else {
                let w = vw - MARGIN_X * 2;
                elements.push(Element::Image {
                    path:  path.clone(),
                    alt:   alt.clone(),
                    y:     *y,
                    w,
                    h:     120,
                    image: None,
                });
                *y += 120 + PARA_GAP;
            }
        }

        Node::Comment { .. } | Node::Document { .. } => {}
    }
}

// ─── Image Scaling ─────────────────────────────────────────

fn scale_image(src: &Image, w: u32, h: u32) -> Image {
    let mut dst = Image::new(w, h);
    if w == 0 || h == 0 { return dst; }
    for dy in 0..h {
        let sy            = (dy as u64 * src.height as u64 / h as u64) as u32;
        let src_row_start = (sy * src.width) as usize * 4;
        let dst_row_start = (dy * w) as usize * 4;
        for dx in 0..w {
            let sx = (dx as u64 * src.width as u64 / w as u64) as u32;
            let si = src_row_start + sx as usize * 4;
            let di = dst_row_start + dx as usize * 4;
            dst.pixels[di..di+4].copy_from_slice(&src.pixels[si..si+4]);
        }
    }
    dst
}