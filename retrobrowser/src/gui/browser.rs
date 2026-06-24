// ============================================================
// RetroX Browser
// Main graphical browser loop. Handles navigation, scrolling,
// rendering, and link clicks.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use crate::platform::{PixelBuffer, PlatformWindow, NativeWindow, Event, Key, MouseButton};
use rnmdl::lexer::Lexer;
use rnmdl::parser::Parser;
use rnmdl::validator::Validator;
use super::layout::{Layout, Element, MARGIN_X, NAV_H, STATUS_H, H1_SCALE, H2_SCALE, H3_SCALE, BODY_SCALE};
use super::paint::{self, Color};
use std::path::Path;
use std::time::{Instant, Duration};

pub struct Browser {
    window:           NativeWindow,
    buf:              PixelBuffer,
    page_buf:         Option<PixelBuffer>,
    scroll_y:         i32,
    scroll_velocity:  f32,
    history:          Vec<String>,
    current:          String,
    asset_base:       String,
    layout:           Option<Layout>,
    hover_link:       Option<usize>,
    status_msg:       String,
    last_vw:          i32,
    resize_pending:   bool,
    resize_pending_w: u32,
    resize_pending_h: u32,
    resize_at:        Option<Instant>,
}

impl Browser {
    pub fn new(title: &str, width: u32, height: u32) -> Self {
        let window = NativeWindow::new(title, width, height);
        let buf    = PixelBuffer::new(width, height);
        Browser {
            window,
            buf,
            page_buf:         None,
            scroll_y:         0,
            scroll_velocity:  0.0,
            history:          Vec::new(),
            current:          String::new(),
            asset_base:       String::new(),
            layout:           None,
            hover_link:       None,
            status_msg:       String::new(),
            last_vw:          0,
            resize_pending:   false,
            resize_pending_w: width,
            resize_pending_h: height,
            resize_at:        None,
        }
    }

    pub fn load(&mut self, path: &str) -> Result<(), String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read '{}': {}", path, e))?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e))?;

        let mut parser = Parser::new(tokens);
        let ast = parser.parse().map_err(|e| format!("{}", e))?;

        let validator = Validator::new();
        let errors    = validator.validate(&ast);
        if !errors.is_empty() {
            return Err(errors.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join("\n"));
        }

        let asset_base = Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        self.asset_base      = asset_base.clone();
        self.current         = path.to_string();
        self.scroll_y        = 0;
        self.scroll_velocity = 0.0;
        self.hover_link      = None;
        self.status_msg      = format!("Loaded: {}", path);

        let vw       = self.buf.width as i32;
        self.last_vw = vw;
        self.layout  = Some(Layout::build(&ast, vw, &asset_base));

        self.render_page();
        self.window.set_title(&format!("RetroX — {}", path));
        Ok(())
    }

    fn render_page(&mut self) {
        let layout = match &self.layout {
            Some(l) => l,
            None    => return,
        };

        let page_w = self.buf.width as i32;
        let page_h = (layout.content_h + NAV_H + STATUS_H + 32).max(self.buf.height as i32);

        let mut pb = PixelBuffer::new(page_w as u32, page_h as u32);
        pb.clear(Color::BG.r, Color::BG.g, Color::BG.b);

        for elem in &layout.elements {
            let ey     = elem.y() + NAV_H;
            let elem_h = elem.height(page_w);
            if ey + elem_h < 0 || ey > page_h { continue; }

            match elem {
                Element::Heading { text, level, .. } => {
                    let (color, scale) = match level {
                        1 => (Color::H1, H1_SCALE),
                        2 => (Color::H2, H2_SCALE),
                        _ => (Color::H3, H3_SCALE),
                    };
                    paint::draw_text(&mut pb, text, MARGIN_X, ey, color, scale);
                }
                Element::Paragraph { text, .. } => {
                    let max_w = page_w - MARGIN_X * 2 - 16;
                    paint::draw_text_wrapped(&mut pb, text, MARGIN_X, ey, max_w, Color::FG, BODY_SCALE);
                }
                Element::Image { image, alt, w: iw, h: ih, .. } => {
                    if let Some(img) = image {
                        paint::draw_image(&mut pb, img, MARGIN_X, ey, *iw, *ih);
                    } else {
                        paint::draw_image_placeholder(&mut pb, alt, MARGIN_X, ey, *iw, *ih);
                    }
                }
                Element::HRule { .. } => {
                    paint::draw_horizontal_rule(&mut pb, MARGIN_X, ey, page_w - MARGIN_X * 2, Color::DIVIDER);
                }
                Element::Link { text, x: lx, w: lw, .. } => {
                    paint::draw_text(&mut pb, text, *lx, ey, Color::LINK, BODY_SCALE);
                    paint::draw_link_underline(&mut pb, *lx, ey + 9, *lw, false);
                }
                Element::Spacer { .. } => {}
            }
        }

        self.page_buf = Some(pb);
    }

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

    pub fn run(&mut self) {
        let frame_time      = Duration::from_micros(16_667); // ~60fps
        let mut last_frame  = Instant::now();
        let mut needs_redraw = true;

        loop {
            // Process ALL pending events before rendering
            while let Some(event) = self.window.next_event() {
                match event {
                    Event::Close => return,

                    Event::Resize { width, height } => {
                        self.resize_pending   = true;
                        self.resize_pending_w = width;
                        self.resize_pending_h = height;
                        self.resize_at        = Some(Instant::now());
                        self.buf.resize(width, height);
                        needs_redraw = true;
                    }

                    Event::Scroll(delta) => {
                        // Add to velocity for momentum scrolling
                        self.scroll_velocity += delta as f32 * 1.5;
                        needs_redraw = true;
                    }

                    Event::KeyPress(key) => {
                        match key {
                            Key::PageUp => {
                                self.scroll_velocity = 0.0;
                                self.scroll_y = (self.scroll_y - self.buf.height as i32).max(0);
                                needs_redraw  = true;
                            }
                            Key::PageDown => {
                                self.scroll_velocity = 0.0;
                                self.scroll_y = (self.scroll_y + self.buf.height as i32)
                                    .min(self.max_scroll());
                                needs_redraw = true;
                            }
                            Key::Up => {
                                self.scroll_velocity = 0.0;
                                self.scroll_y = (self.scroll_y - 40).max(0);
                                needs_redraw  = true;
                            }
                            Key::Down => {
                                self.scroll_velocity = 0.0;
                                self.scroll_y = (self.scroll_y + 40).min(self.max_scroll());
                                needs_redraw  = true;
                            }
                            Key::Escape => return,
                            _ => {}
                        }
                    }

                    Event::MouseMove { x, y } => {
                        let prev = self.hover_link;
                        self.hover_link = self.link_at(x as i32, y as i32);
                        if self.hover_link != prev {
                            needs_redraw = true;
                        }
                    }

                    Event::MouseClick { x, y, button: MouseButton::Left } => {
                        if y < NAV_H as u32 {
                            if x < 80 && !self.history.is_empty() {
                                self.go_back();
                                needs_redraw = true;
                            }
                            continue;
                        }
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

            // Resize debounce
            if self.resize_pending {
                if let Some(at) = self.resize_at {
                    if at.elapsed() > Duration::from_millis(150) {
                        self.resize_pending = false;
                        self.resize_at      = None;
                        let w = self.resize_pending_w;
                        let h = self.resize_pending_h;
                        self.buf.resize(w, h);
                        if self.last_vw != w as i32 && !self.current.is_empty() {
                            self.last_vw = w as i32;
                            let path = self.current.clone();
                            let _ = self.load(&path);
                        } else {
                            self.render_page();
                        }
                        needs_redraw = true;
                    }
                }
            }

            // Apply scroll momentum every frame
            if self.scroll_velocity.abs() > 0.5 {
                let max = self.max_scroll();
                self.scroll_y = (self.scroll_y + self.scroll_velocity as i32).clamp(0, max);
                self.scroll_velocity *= 0.75; // friction
                // Stop at boundaries
                if self.scroll_y <= 0 || self.scroll_y >= max {
                    self.scroll_velocity = 0.0;
                }
                needs_redraw = true;
            } else if self.scroll_velocity != 0.0 {
                self.scroll_velocity = 0.0;
            }

            // Render at ~60fps
            let now = Instant::now();
            if needs_redraw && now.duration_since(last_frame) >= frame_time {
                let t0 = Instant::now();
                self.composite();
                let t1 = Instant::now();
                self.window.present(&self.buf);
                let t2 = Instant::now();
                eprintln!("composite: {}ms  present: {}ms  total: {}ms",
                    t1.duration_since(t0).as_millis(),
                    t2.duration_since(t1).as_millis(),
                    t2.duration_since(t0).as_millis());
                needs_redraw = false;
                last_frame   = now;
            }

            // Sleep until next frame
            let now = Instant::now();
            let elapsed = now.duration_since(last_frame);
            if elapsed < frame_time {
                std::thread::sleep(frame_time - elapsed);
            }
        }
    }

    fn composite(&mut self) {
        let w = self.buf.width;
        let h = self.buf.height;

//        self.buf.clear(Color::BG.r, Color::BG.g, Color::BG.b);

        if let Some(ref pb) = self.page_buf {
            let content_top = NAV_H as u32;
            let content_h   = h.saturating_sub(NAV_H as u32 + STATUS_H as u32);
            let row_bytes   = (w.min(pb.width) * 4) as usize;
            let src_stride  = (pb.width * 4) as usize;
            let dst_stride  = (w * 4) as usize;

            for dy in 0..content_h {
                let src_y = self.scroll_y.max(0) as u32 + dy;
                if src_y >= pb.height { break; }
                let src_off = src_y as usize * src_stride;
                let dst_off = (content_top + dy) as usize * dst_stride;
                if src_off + row_bytes > pb.data.len() { break; }
                if dst_off + row_bytes > self.buf.data.len() { break; }
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        pb.data.as_ptr().add(src_off),
                        self.buf.data.as_mut_ptr().add(dst_off),
                        row_bytes,
                    );
                }
            }
        } else {
            let msg = "No document loaded. Use: retrox gui <file.rnmdl>";
            paint::draw_text(&mut self.buf, msg, MARGIN_X, NAV_H + 32, Color::FG, 1);
        }

        self.draw_nav(w as i32);
        self.draw_status(w as i32, h as i32);

        let content_h  = self.layout.as_ref().map(|l| l.content_h).unwrap_or(0);
        let viewport_h = h as i32 - NAV_H - STATUS_H;
        paint::draw_scrollbar(&mut self.buf, self.scroll_y, content_h, viewport_h);
    }

    fn draw_nav(&mut self, w: i32) {
        paint::fill_rect(&mut self.buf, 0, 0, w, NAV_H, Color::NAV_BG);
        paint::draw_horizontal_rule(&mut self.buf, 0, NAV_H - 1, w, Color::DIVIDER);

        let btn_color = if !self.history.is_empty() { Color::NAV_BTN_HOVER } else { Color::NAV_BTN };
        paint::fill_rect(&mut self.buf, 8, 6, 64, NAV_H - 12, btn_color);
        paint::draw_text(&mut self.buf, "< Back", 14, 11, Color::NAV_TEXT, 1);

        let name = Path::new(&self.current)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let tx = (w / 2 - (name.len() as i32 * 8) / 2).max(0);
        paint::draw_text(&mut self.buf, &name, tx, 11, Color::NAV_TEXT, 1);

        let label = "RetroX GN-Z11";
        let lx = w - label.len() as i32 * 8 - 16;
        paint::draw_text(&mut self.buf, label, lx, 11, Color::DIVIDER, 1);
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

    fn link_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let vh     = self.buf.height as i32 - NAV_H - STATUS_H;
        for (i, elem) in layout.elements.iter().enumerate() {
            if let Element::Link { y: ey, x: ex, w, .. } = elem {
                let screen_y = ey - self.scroll_y + NAV_H;
                if screen_y < NAV_H || screen_y > NAV_H + vh { continue; }
                if x >= *ex && x <= ex + w && y >= screen_y && y <= screen_y + 16 {
                    return Some(i);
                }
            }
        }
        None
    }

    fn max_scroll(&self) -> i32 {
        let content_h  = self.layout.as_ref().map(|l| l.content_h).unwrap_or(0);
        let viewport_h = self.buf.height as i32 - NAV_H - STATUS_H;
        (content_h - viewport_h).max(0)
    }
}