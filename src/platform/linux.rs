// ============================================================
// RetroX Linux Platform (XCB)
// Native X11 window via xcb =1.7.0.
// Presents a PixelBuffer to the screen using PutImage (ZPixmap).
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use xcb::{self, Connection, x, Xid};
use super::{PlatformWindow, PixelBuffer, Event, Key, MouseButton};

pub struct LinuxWindow {
    conn:      Connection,
    window:    x::Window,
    gc:        x::Gcontext,
    width:     u32,
    height:    u32,
    wm_delete: x::Atom,
    // Reusable BGR0 conversion buffer — avoids allocation every frame
    blit_buf:  Vec<u8>,
}

impl PlatformWindow for LinuxWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        let (conn, screen_num) = Connection::connect(None)
            .expect("RetroX: failed to connect to X server. Is DISPLAY set?");

        let setup  = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize)
            .expect("RetroX: failed to get X screen");

        let window: x::Window   = conn.generate_id();
        let gc:     x::Gcontext = conn.generate_id();

        // Create window
        conn.send_request(&x::CreateWindow {
            depth:        x::COPY_FROM_PARENT as u8,
            wid:          window,
            parent:       screen.root(),
            x:            0,
            y:            0,
            width:        width as u16,
            height:       height as u16,
            border_width: 0,
            class:        x::WindowClass::InputOutput,
            visual:       screen.root_visual(),
            value_list: &[
                x::Cw::BackPixel(screen.black_pixel()),
                x::Cw::EventMask(
                    x::EventMask::EXPOSURE
                    | x::EventMask::KEY_PRESS
                    | x::EventMask::BUTTON_PRESS
                    | x::EventMask::BUTTON_RELEASE
                    | x::EventMask::POINTER_MOTION
                    | x::EventMask::STRUCTURE_NOTIFY,
                ),
            ],
        });

        // Create graphics context
        conn.send_request(&x::CreateGc {
            cid:        gc,
            drawable:   x::Drawable::Window(window),
            value_list: &[],
        });

        // Set window title
        conn.send_request(&x::ChangeProperty {
            mode:     x::PropMode::Replace,
            window,
            property: x::ATOM_WM_NAME,
            r#type:   x::ATOM_STRING,
            data:     title.as_bytes(),
        });

        // Register WM_DELETE_WINDOW so closing the window sends an event
        let wm_protocols_cookie = conn.send_request(&x::InternAtom {
            only_if_exists: false,
            name:           b"WM_PROTOCOLS",
        });
        let wm_delete_cookie = conn.send_request(&x::InternAtom {
            only_if_exists: false,
            name:           b"WM_DELETE_WINDOW",
        });

        let wm_protocols = conn.wait_for_reply(wm_protocols_cookie)
            .expect("RetroX: failed to intern WM_PROTOCOLS").atom();
        let wm_delete = conn.wait_for_reply(wm_delete_cookie)
            .expect("RetroX: failed to intern WM_DELETE_WINDOW").atom();

        conn.send_request(&x::ChangeProperty {
            mode:     x::PropMode::Replace,
            window,
            property: wm_protocols,
            r#type:   x::ATOM_ATOM,
            data:     &[wm_delete],
        });

        conn.send_request(&x::MapWindow { window });
        conn.flush().expect("RetroX: failed to flush X connection");

        let blit_buf = vec![0u8; (width * height * 4) as usize];

        LinuxWindow { conn, window, gc, width, height, wm_delete, blit_buf }
    }

    fn present(&mut self, buffer: &PixelBuffer) {
        let pixels = buffer.width * buffer.height;

        // Grow blit buffer if needed (window was resized)
        if self.blit_buf.len() < (pixels * 4) as usize {
            self.blit_buf.resize((pixels * 4) as usize, 0);
        }

        // Convert RGBA → BGR0 (X11 ZPixmap expects BGR byte order on LE systems)
        for i in 0..pixels as usize {
            let r = buffer.data[i * 4];
            let g = buffer.data[i * 4 + 1];
            let b = buffer.data[i * 4 + 2];
            // alpha is ignored by X11 PutImage; pad byte = 0
            self.blit_buf[i * 4]     = b;
            self.blit_buf[i * 4 + 1] = g;
            self.blit_buf[i * 4 + 2] = r;
            self.blit_buf[i * 4 + 3] = 0;
        }

        self.conn.send_request(&x::PutImage {
            format:   x::ImageFormat::ZPixmap,
            drawable: x::Drawable::Window(self.window),
            gc:       self.gc,
            width:    buffer.width  as u16,
            height:   buffer.height as u16,
            dst_x:    0,
            dst_y:    0,
            left_pad: 0,
            depth:    24,
            data:     &self.blit_buf[..(pixels * 4) as usize],
        });

        self.conn.flush().expect("RetroX: failed to flush after PutImage");
    }

    fn next_event(&mut self) -> Option<Event> {
        match self.conn.poll_for_event() {
            Ok(Some(ev)) => self.translate(ev),
            _            => None,
        }
    }

    fn width(&self)  -> u32 { self.width }
    fn height(&self) -> u32 { self.height }

    fn set_title(&mut self, title: &str) {
        self.conn.send_request(&x::ChangeProperty {
            mode:     x::PropMode::Replace,
            window:   self.window,
            property: x::ATOM_WM_NAME,
            r#type:   x::ATOM_STRING,
            data:     title.as_bytes(),
        });
        self.conn.flush().expect("RetroX: failed to flush set_title");
    }
}

impl LinuxWindow {
    fn translate(&mut self, event: xcb::Event) -> Option<Event> {
        match event {
            // ── Keyboard ──────────────────────────────────
            xcb::Event::X(x::Event::KeyPress(e)) => {
                Some(Event::KeyPress(self.map_key(e.detail())))
            }

            // ── Mouse buttons & scroll wheel ──────────────
            xcb::Event::X(x::Event::ButtonPress(e)) => {
                match e.detail() {
                    1 => Some(Event::MouseClick {
                        x: e.event_x() as u32,
                        y: e.event_y() as u32,
                        button: MouseButton::Left,
                    }),
                    2 => Some(Event::MouseClick {
                        x: e.event_x() as u32,
                        y: e.event_y() as u32,
                        button: MouseButton::Middle,
                    }),
                    3 => Some(Event::MouseClick {
                        x: e.event_x() as u32,
                        y: e.event_y() as u32,
                        button: MouseButton::Right,
                    }),
                    // X11 scroll wheel: button 4 = up, 5 = down
                    4 => Some(Event::Scroll(-3)),
                    5 => Some(Event::Scroll(3)),
                    _ => None,
                }
            }

            // ── Mouse movement ────────────────────────────
            xcb::Event::X(x::Event::MotionNotify(e)) => {
                Some(Event::MouseMove {
                    x: e.event_x() as u32,
                    y: e.event_y() as u32,
                })
            }

            // ── Resize / move ─────────────────────────────
            xcb::Event::X(x::Event::ConfigureNotify(e)) => {
                let w = e.width()  as u32;
                let h = e.height() as u32;
                if w != self.width || h != self.height {
                    self.width  = w;
                    self.height = h;
                    Some(Event::Resize { width: w, height: h })
                } else {
                    None
                }
            }

            // ── Expose: treat as resize to force redraw ───
            xcb::Event::X(x::Event::Expose(e)) => {
                if e.count() == 0 {
                    Some(Event::Resize { width: self.width, height: self.height })
                } else {
                    None
                }
            }

            // ── Window close button ───────────────────────
            xcb::Event::X(x::Event::ClientMessage(e)) => {
                if let x::ClientMessageData::Data32([atom, ..]) = e.data() {
                    if atom == self.wm_delete.resource_id() {
                        return Some(Event::Close);
                    }
                }
                None
            }

            _ => None,
        }
    }

    fn map_key(&self, keycode: u8) -> Key {
        // Standard US keyboard X11 keycodes (evdev layout)
        match keycode {
            111 => Key::Up,
            116 => Key::Down,
            113 => Key::Left,
            114 => Key::Right,
            112 => Key::PageUp,
            117 => Key::PageDown,
            36  => Key::Enter,
            9   => Key::Escape,
            22  => Key::Backspace,
            _   => Key::Char(' '),
        }
    }
}