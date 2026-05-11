// ============================================================
// RetroX Layout Engine
// Converts AST nodes into a flat list of render elements.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use crate::ast::Node;
use crate::image::{Image, load_image};
use crate::font;
use super::paint::Color;

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

    pub fn height(&self, viewport_w: i32) -> i32 {
        match self {
            Element::Heading { level, text, .. } => {
                let scale = match level {
                    1 => H1_SCALE,
                    2 => H2_SCALE,
                    _ => H3_SCALE,
                };
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

// ─── Layout Pass ───────────────────────────────────────────

pub struct Layout {
    pub elements:   Vec<Element>,
    pub content_h:  i32,
}

impl Layout {
    pub fn build(node: &Node, viewport_w: i32, asset_base: &str) -> Self {
        let mut elements = Vec::new();
        let mut y = NAV_H + 16;

        match node {
            Node::Document { children, .. } => {
                for child in children {
                    layout_node(child, &mut elements, &mut y, viewport_w, asset_base);
                }
            }
            _ => {
                layout_node(node, &mut elements, &mut y, viewport_w, asset_base);
            }
        }

        let content_h = y + 32;
        Layout { elements, content_h }
    }
}

fn layout_node(
    node:       &Node,
    elements:   &mut Vec<Element>,
    y:          &mut i32,
    viewport_w: i32,
    asset_base: &str,
) {
    match node {
        Node::Container { children, .. } | Node::Section { children, .. } => {
            for child in children {
                layout_node(child, elements, y, viewport_w, asset_base);
            }
        }

        Node::Heading { level, text, .. } => {
            let scale = match level {
                1 => H1_SCALE,
                2 => H2_SCALE,
                _ => H3_SCALE,
            };
            *y += PARA_GAP;
            if *level == 1 {
                elements.push(Element::HRule { y: *y });
                *y += 8;
            }
            elements.push(Element::Heading {
                text:  text.clone(),
                level: *level,
                y:     *y,
            });
            *y += font::GLYPH_H as i32 * scale as i32 + PARA_GAP;
            if *level == 1 {
                elements.push(Element::HRule { y: *y });
                *y += 8;
            }
        }

        Node::Paragraph { text, .. } => {
            let max_w     = viewport_w - MARGIN_X * 2 - 16;
            let glyph_w   = (font::GLYPH_W * BODY_SCALE) as i32;
            let glyph_h   = (font::GLYPH_H * BODY_SCALE) as i32;
            let chars_per = (max_w / glyph_w).max(1) as usize;

            // Calculate wrapped height
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
            elements.push(Element::Paragraph {
                text: text.clone(),
                y:    *y,
                h,
            });
            *y += h;
        }

        Node::Image { path, alt, .. } => {
            let full_path = if asset_base.is_empty() {
                path.clone()
            } else {
                format!("{}/{}", asset_base.trim_end_matches('/'), path)
            };

            let loaded = load_image(&full_path).ok();
            let (iw, ih) = if let Some(ref img) = loaded {
                let max_w = (viewport_w - MARGIN_X * 2) as u32;
                let scale = if img.width > max_w {
                    max_w as f32 / img.width as f32
                } else { 1.0 };
                ((img.width as f32 * scale) as i32,
                 (img.height as f32 * scale) as i32)
            } else {
                (viewport_w - MARGIN_X * 2, 120)
            };

            elements.push(Element::Image {
                path:  path.clone(),
                alt:   alt.clone(),
                y:     *y,
                w:     iw,
                h:     ih,
                image: loaded,
            });
            *y += ih + PARA_GAP;
        }

        Node::Comment { .. } => {}

        Node::Document { .. } => {}
    }
}
