// ============================================================
// RetroX Browser
// Main graphical browser loop. Handles navigation, scrolling,
// rendering, and link clicks.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use crate::platform::{PixelBuffer, PlatformWindow, NativeWindow, Event, Key, MouseButton};
use crate::ast::Node;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::validator::Validator;
use crate::font;
use super::layout::{Layout, Element, MARGIN_X, NAV_H, STATUS_H, H1_SCALE, H2_SCALE, H3_SCALE, BODY_SCALE};
use super::paint::{self, Color};
use std::collections::HashMap;
use std::path::Path;

// ─── Browser State ─────────────────────────────────────────

pub struct Browser {
    window:      NativeWindow,
    buf:         PixelBuffer,
    scroll_y:    i32,
    history:     Vec<String>,        // navigation history
    current:     String,             // current file path
    asset_base:  String,             // base path for assets
    layout:      Option<Layout>,
    hover_link:  Option<usize>,      // index of hovered link element
    status_msg:  String,
}

impl Browser {
    pub fn new(title: &str, width: u32, height: u32) -> Self {
        let window = NativeWindow::new(title, width, height);
        let buf    = PixelBuffer::new(width, height);
        Browser {
            window,
            buf,
            scroll_y:   0,
            history:    Vec::new(),
            current:    String::new(),
            asset_base: String::new(),
            layout:     None,
            hover_link: None,
            status_msg: String::new(),
        }
    }

    // ─── Load a .rnmdl file ────────────────────────────────

    pub fn load(&mut self, path: &str) -> Result<(), String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read '{}': {}", path, e))?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()
            .map_err(|e| format!("{}", e))?;

        let mut parser = Parser::new(tokens);
        let ast = parser.parse()
            .map_err(|e| format!("{}", e))?;

        let validator = Validator::new();
        let errors    = validator.validate(&ast);
        if !errors.is_empty() {
            let msg = errors.iter()
                .map(|e| format!("{}", e))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(msg);
        }

        // Set asset base to directory containing the file
        let asset_base = Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        self.asset_base = asset_base.clone();
        self.current    = path.to_string();
        self.scroll_y   = 0;
        self.hover_link = None;
        self.status_msg = format!("Loaded: {}", path);

        let vw = self.buf.width as i32;
        self.layout = Some(Layout::build(&ast, vw, &asset_base));

        self.window.set_title(&format!("RetroX — {}", path));
        Ok(())
    }

    // ─── Navigate to a link target ─────────────────────────

    fn navigate(&mut self, target: &str) {
        let base = Path::new(&self.current)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let full = if base.is_empty() {
            target.to_string()
        } else {
            format!("{}/{}", base, target)
        };

        self.history.push(self.current.clone());
        if let Err(e) = self.load(&full) {
            self.status_msg = format!("Error: {}", e);
            self.history.pop();
        }
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            let _ = self.load(&prev);
        }
    }

    // ─── Main Run Loop ─────────────────────────────────────

    pub fn run(&mut self) {
        let mut needs_redraw = true;

        loop {
            // Handle events
            while let Some(event) = self.window.next_event() {
                match event {
                    Event::Close => return,

                    Event::Resize { width, height } => {
                        self.buf.resize(width, height);
                        // Rebuild layout for new width
                        if !self.current.is_empty() {
                            let _ = self.load(&self.current.clone());
                        }
                        needs_redraw = true;
                    }

                    Event::Scroll(delta) => {
                        let max_scroll = self.max_scroll();
                        self.scroll_y  = (self.scroll_y + delta * 12).clamp(0, max_scroll);
                        needs_redraw   = true;
                    }

                    Event::KeyPress(key) => {
                        match key {
                            Key::PageUp => {
                                self.scroll_y = (self.scroll_y - self.buf.height as i32)
                                    .max(0);
                                needs_redraw = true;
                            }
                            Key::PageDown => {
                                let max = self.max_scroll();
                                self.scroll_y = (self.scroll_y + self.buf.height as i32)
                                    .min(max);
                                needs_redraw = true;
                            }
                            Key::Up => {
                                self.scroll_y = (self.scroll_y - 20).max(0);
                                needs_redraw = true;
                            }
                            Key::Down => {
                                let max = self.max_scroll();
                                self.scroll_y = (self.scroll_y + 20).min(max);
                                needs_redraw = true;
                            }
                            Key::Escape => return,
                            _ => {}
                        }
                    }

                    Event::MouseMove { x, y } => {
                        let prev_hover = self.hover_link;
                        self.hover_link = self.link_at(x as i32, y as i32);
                        if self.hover_link != prev_hover {
                            if let Some(idx) = self.hover_link {
                                if let Some(ref layout) = self.layout {
                                    if let Element::Link { target, .. } = &layout.elements[idx] {
                                        self.status_msg = format!("→ {}", target);
                                    }
                                }
                            } else {
                                self.status_msg = String::new();
                            }
                            needs_redraw = true;
                        }
                    }

                    Event::MouseClick { x, y, button: MouseButton::Left } => {
                        // Check nav buttons
                        if y < NAV_H as u32 {
                            if x < 80 && !self.history.is_empty() {
                                self.go_back();
                                needs_redraw = true;
                            }
                            continue;
                        }

                        // Check link clicks
                        if let Some(idx) = self.link_at(x as i32, y as i32) {
                            let target = if let Some(ref layout) = self.layout {
                                if let Element::Link { target, .. } = &layout.elements[idx] {
                                    Some(target.clone())
                                } else { None }
                            } else { None };

                            if let Some(t) = target {
                                self.navigate(&t);
                                needs_redraw = true;
                            }
                        }
                    }

                    _ => {}
                }
            }

            if needs_redraw {
                self.render();
                self.window.present(&self.buf);
                needs_redraw = false;
            }

            // Small sleep to avoid busy loop
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
    }

    // ─── Hit Testing ───────────────────────────────────────

    fn link_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let scroll = self.scroll_y;
        let vh     = self.buf.height as i32 - NAV_H - STATUS_H;

        for (i, elem) in layout.elements.iter().enumerate() {
            if let Element::Link { y: ey, x: ex, w, .. } = elem {
                let screen_y = ey - scroll + NAV_H;
                if screen_y < NAV_H || screen_y > NAV_H + vh { continue; }
                if x >= *ex && x <= ex + w && y >= screen_y && y <= screen_y + 16 {
                    return Some(i);
                }
            }
        }
        None
    }

    fn max_scroll(&self) -> i32 {
        let content_h = self.layout.as_ref().map(|l| l.content_h).unwrap_or(0);
        let viewport_h = self.buf.height as i32 - NAV_H - STATUS_H;
        (content_h - viewport_h).max(0)
    }

    // ─── Render ────────────────────────────────────────────

    fn render(&mut self) {
        let w = self.buf.width  as i32;
        let h = self.buf.height as i32;

        // Clear background
        self.buf.clear(Color::BG.r, Color::BG.g, Color::BG.b);

        // Draw nav bar
        self.draw_nav(w);

        // Draw content
        self.draw_content(w, h);

        // Draw status bar
        self.draw_status(w, h);

        // Draw scrollbar
        let content_h  = self.layout.as_ref().map(|l| l.content_h).unwrap_or(0);
        let viewport_h = h - NAV_H - STATUS_H;
        paint::draw_scrollbar(&mut self.buf, self.scroll_y, content_h, viewport_h);
    }

    fn draw_nav(&mut self, w: i32) {
        paint::fill_rect(&mut self.buf, 0, 0, w, NAV_H, Color::NAV_BG);
        paint::draw_horizontal_rule(&mut self.buf, 0, NAV_H - 1, w, Color::DIVIDER);

        // Back button
        let has_history = !self.history.is_empty();
        let btn_color = if has_history { Color::NAV_BTN_HOVER } else { Color::NAV_BTN };
        paint::fill_rect(&mut self.buf, 8, 6, 64, NAV_H - 12, btn_color);
        paint::draw_text(&mut self.buf, "< Back", 14, 11, Color::NAV_TEXT, 1);

        // Current file name
        let name = Path::new(&self.current)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let tx = w / 2 - (name.len() as i32 * 8) / 2;
        paint::draw_text(&mut self.buf, &name, tx, 11, Color::NAV_TEXT, 1);

        // RetroX label (right side)
        let label = "RetroX GN-Z11";
        let lx = w - label.len() as i32 * 8 - 16;
        paint::draw_text(&mut self.buf, label, lx, 11, Color::DIVIDER, 1);
    }

    fn draw_content(&mut self, w: i32, h: i32) {
        let layout = match &self.layout {
            Some(l) => l,
            None    => {
                let msg = "No document loaded. Use: retrox gui <file.rnmdl>";
                paint::draw_text(&mut self.buf, msg, MARGIN_X, NAV_H + 32, Color::FG, 1);
                return;
            }
        };

        let scroll   = self.scroll_y;
        let viewport = h - NAV_H - STATUS_H;

        // Clip rect for content area
        for elem in &layout.elements {
            let ey       = elem.y() - scroll + NAV_H;
            let elem_h   = elem.height(w);

            // Skip offscreen elements
            if ey + elem_h < NAV_H { continue; }
            if ey > NAV_H + viewport { continue; }

            match elem {
                Element::Heading { text, level, .. } => {
                    let (color, scale) = match level {
                        1 => (Color::H1, H1_SCALE),
                        2 => (Color::H2, H2_SCALE),
                        _ => (Color::H3, H3_SCALE),
                    };
                    paint::draw_text(&mut self.buf, text, MARGIN_X, ey, color, scale);
                }

                Element::Paragraph { text, .. } => {
                    let max_w = w - MARGIN_X * 2 - 16;
                    paint::draw_text_wrapped(
                        &mut self.buf, text,
                        MARGIN_X, ey,
                        max_w, Color::FG, BODY_SCALE,
                    );
                }

                Element::Image { image, alt, w: iw, h: ih, .. } => {
                    if let Some(img) = image {
                        paint::draw_image(&mut self.buf, img, MARGIN_X, ey, *iw, *ih);
                    } else {
                        paint::draw_image_placeholder(
                            &mut self.buf, alt,
                            MARGIN_X, ey, *iw, *ih,
                        );
                    }
                }

                Element::Link { text, target, y: _, x: lx, w: lw } => {
                    let is_hover = self.hover_link
                        .map(|hi| {
                            layout.elements.get(hi)
                                .map(|e| matches!(e, Element::Link { target: t, .. } if t == target))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);

                    let color = if is_hover { Color::LINK_HOVER } else { Color::LINK };
                    paint::draw_text(&mut self.buf, text, *lx, ey, color, BODY_SCALE);
                    paint::draw_link_underline(&mut self.buf, *lx, ey + 9, *lw, is_hover);
                }

                Element::HRule { .. } => {
                    paint::draw_horizontal_rule(
                        &mut self.buf, MARGIN_X, ey,
                        w - MARGIN_X * 2, Color::DIVIDER,
                    );
                }

                Element::Spacer { .. } => {}
            }
        }
    }

    fn draw_status(&mut self, w: i32, h: i32) {
        let sy = h - STATUS_H;
        paint::fill_rect(&mut self.buf, 0, sy, w, STATUS_H, Color::STATUS_BG);
        paint::draw_horizontal_rule(&mut self.buf, 0, sy, w, Color::DIVIDER);

        let msg = if self.status_msg.is_empty() {
            format!("RetroX RNMDL GN-Z11 | Scroll: {}px", self.scroll_y)
        } else {
            self.status_msg.clone()
        };

        paint::draw_text(&mut self.buf, &msg, 8, sy + 6, Color::STATUS_TEXT, 1);
    }
}
