// ============================================================
// RetroX Linux Platform (XCB)
// Locked to xcb =1.7.0
// Rust 1.95.0 | Edition 2021
// ============================================================

use xcb::{self, Connection, x, Xid};
use super::{PlatformWindow, PixelBuffer, Event, Key, MouseButton};

pub struct LinuxWindow {
    conn:   Connection,
    window: x::Window,
    gc:     x::Gcontext,
    width:  u32,
    height: u32,
    wm_delete: x::Atom,
}

impl PlatformWindow for LinuxWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        let (conn, screen_num) = Connection::connect(None)
            .expect("Failed to connect to X server");

        let setup  = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize)
            .expect("Failed to get screen");

        let window: x::Window   = conn.generate_id();
        let gc:     x::Gcontext = conn.generate_id();

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
                    | x::EventMask::POINTER_MOTION
                    | x::EventMask::STRUCTURE_NOTIFY
                ),
            ],
        });

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

        // Register WM_DELETE_WINDOW so we get notified on close
        let wm_protocols_cookie = conn.send_request(&x::InternAtom {
            only_if_exists: false,
            name:           b"WM_PROTOCOLS",
        });
        let wm_delete_cookie = conn.send_request(&x::InternAtom {
            only_if_exists: false,
            name:           b"WM_DELETE_WINDOW",
        });

        let wm_protocols = conn.wait_for_reply(wm_protocols_cookie)
            .expect("Failed to get WM_PROTOCOLS atom").atom();
        let wm_delete = conn.wait_for_reply(wm_delete_cookie)
            .expect("Failed to get WM_DELETE_WINDOW atom").atom();

        conn.send_request(&x::ChangeProperty {
            mode:     x::PropMode::Replace,
            window,
            property: wm_protocols,
            r#type:   x::ATOM_ATOM,
            data:     &[wm_delete],
        });

        conn.send_request(&x::MapWindow { window });
        conn.flush().expect("Failed to flush");

        LinuxWindow { conn, window, gc, width, height, wm_delete }
    }

    fn present(&mut self, buffer: &PixelBuffer) {
        // Convert RGBA to XCB BGR0 format
        let mut bgr: Vec<u8> = Vec::with_capacity(
            (buffer.width * buffer.height * 4) as usize
        );
        for i in 0..(buffer.width * buffer.height) as usize {
            let r = buffer.data[i * 4];
            let g = buffer.data[i * 4 + 1];
            let b = buffer.data[i * 4 + 2];
            bgr.push(b);
            bgr.push(g);
            bgr.push(r);
            bgr.push(0);
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
            data:     &bgr,
        });

        self.conn.flush().expect("Failed to flush");
    }

    fn next_event(&mut self) -> Option<Event> {
        match self.conn.poll_for_event() {
            Ok(Some(event)) => self.translate_event(event),
            _ => None,
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
        self.conn.flush().expect("Failed to flush");
    }
}

impl LinuxWindow {
    fn translate_event(&mut self, event: xcb::Event) -> Option<Event> {
        match event {
            xcb::Event::X(x::Event::KeyPress(e)) => {
                Some(Event::KeyPress(self.translate_key(e.detail())))
            }

            xcb::Event::X(x::Event::ButtonPress(e)) => {
                match e.detail() {
                    1 => Some(Event::MouseClick {
                        x:      e.event_x() as u32,
                        y:      e.event_y() as u32,
                        button: MouseButton::Left,
                    }),
                    2 => Some(Event::MouseClick {
                        x:      e.event_x() as u32,
                        y:      e.event_y() as u32,
                        button: MouseButton::Middle,
                    }),
                    3 => Some(Event::MouseClick {
                        x:      e.event_x() as u32,
                        y:      e.event_y() as u32,
                        button: MouseButton::Right,
                    }),
                    4 => Some(Event::Scroll(-3)),
                    5 => Some(Event::Scroll(3)),
                    _ => None,
                }
            }

            xcb::Event::X(x::Event::MotionNotify(e)) => {
                Some(Event::MouseMove {
                    x: e.event_x() as u32,
                    y: e.event_y() as u32,
                })
            }

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

            xcb::Event::X(x::Event::ClientMessage(e)) => {
                if let x::ClientMessageData::Data32([atom, ..]) = e.data() {
                    if atom == self.wm_delete.resource_id() {
                        return Some(Event::Close);
                    }
                }
                None
            }

            xcb::Event::X(x::Event::Expose(_)) => {
                Some(Event::Resize {
                    width:  self.width,
                    height: self.height,
                })
            }

            _ => None,
        }
    }

    fn translate_key(&self, keycode: u8) -> Key {
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
