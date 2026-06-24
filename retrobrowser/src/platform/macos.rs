// ============================================================
// RetroBrowser macOS Platform (Cocoa)
// Software framebuffer presented via NSBitmapImageRep.
// No GPU required. Stable Cocoa API since macOS 10.0.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use objc2::runtime::AnyObject;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{
    NSApplication,
    NSBackingStoreType,  // lives behind the "NSGraphics" feature
    NSBitmapImageRep,    // lives behind "NSBitmapImageRep" + "NSImageRep" features
    NSEvent,
    NSEventMask,
    NSEventType,
    NSGraphicsContext,   // lives behind "NSGraphicsContext" feature
    NSScreen,
    NSView,
    NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{ns_string, NSPoint, NSRect, NSSize, NSString};

use super::{Event, Key, MouseButton, PixelBuffer, PlatformWindow};
use std::sync::Mutex;

static EVENT_QUEUE: Mutex<Vec<Event>> = Mutex::new(Vec::new());

pub struct MacosWindow {
    app:      *mut AnyObject,
    window:   *mut AnyObject,
    view:     *mut AnyObject,
    width:    u32,
    height:   u32,
    rgba_buf: Vec<u8>,
}

impl PlatformWindow for MacosWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        unsafe {
            // ── NSApplication ────────────────────────────────────────────
            // sharedApplication is a class method; use msg_send! on the class.
            let app: *mut AnyObject =
                msg_send![NSApplication::class(), sharedApplication];
            // NSApplicationActivationPolicyRegular == 0
            let _: () = msg_send![app, setActivationPolicy: 0i64];

            // ── NSWindow ─────────────────────────────────────────────────
            let rect = NSRect {
                origin: NSPoint { x: 200.0, y: 200.0 },
                size:   NSSize  { width: width as f64, height: height as f64 },
            };

            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::Miniaturizable;

            // NSWindow::alloc() returns an Allocated<NSWindow>; cast to *mut AnyObject
            // so we can keep using raw msg_send! for init (which has a non-standard
            // selector that the typed API doesn't expose in 0.2).
            let alloc: *mut AnyObject =
                msg_send![NSWindow::class(), alloc];
            let window: *mut AnyObject = msg_send![
                alloc,
                initWithContentRect: rect,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ];

            let ns_title = NSString::from_str(title);
            let _: () = msg_send![window, setTitle: &*ns_title];
            let _: () = msg_send![
                window,
                makeKeyAndOrderFront: std::ptr::null::<AnyObject>()
            ];

            let view: *mut AnyObject = msg_send![window, contentView];

            let _: () = msg_send![app, activateIgnoringOtherApps: true];

            eprintln!("[RetroX] Cocoa window initialized ({}x{})", width, height);

            MacosWindow {
                app,
                window,
                view,
                width,
                height,
                rgba_buf: vec![0u8; (width * height * 4) as usize],
            }
        }
    }

    fn present(&mut self, buffer: &PixelBuffer) {
        let w = buffer.width;
        let h = buffer.height;
        let size = (w * h * 4) as usize;

        if self.rgba_buf.len() < size {
            self.rgba_buf.resize(size, 0);
        }
        self.rgba_buf[..size].copy_from_slice(&buffer.data[..size]);

        unsafe {
            // ── NSBitmapImageRep ─────────────────────────────────────────
            // alloc via the class, then call the long init selector.
            let alloc: *mut AnyObject =
                msg_send![NSBitmapImageRep::class(), alloc];
            let bmp: *mut AnyObject = msg_send![
                alloc,
                initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>(),
                pixelsWide:      w as i64,
                pixelsHigh:      h as i64,
                bitsPerSample:   8i64,
                samplesPerPixel: 4i64,
                hasAlpha:        true,
                isPlanar:        false,
                colorSpaceName:  ns_string!("NSDeviceRGBColorSpace"),
                bytesPerRow:     (w * 4) as i64,
                bitsPerPixel:    32i64
            ];

            let bitmap_data: *mut u8 = msg_send![bmp, bitmapData];
            std::ptr::copy_nonoverlapping(
                self.rgba_buf.as_ptr(),
                bitmap_data,
                size,
            );

            // ── Draw into view ───────────────────────────────────────────
            // NSGraphicsContext::currentContext is gated behind "NSGraphicsContext"
            let ctx: *mut AnyObject =
                msg_send![NSGraphicsContext::class(), currentContext];
            if !ctx.is_null() {
                let rect = NSRect {
                    origin: NSPoint { x: 0.0, y: 0.0 },
                    size:   NSSize  { width: w as f64, height: h as f64 },
                };
                let _: () = msg_send![bmp, drawInRect: rect];
            }

            let _: () = msg_send![self.window, flushWindow];
        }
    }

    fn next_event(&mut self) -> Option<Event> {
        unsafe {
            loop {
                let event: *mut AnyObject = msg_send![
                    self.app,
                    nextEventMatchingMask: NSEventMask::Any,
                    untilDate: std::ptr::null::<AnyObject>(),
                    inMode: ns_string!("kCFRunLoopDefaultMode"),
                    dequeue: true
                ];
                if event.is_null() {
                    break;
                }

                // NSEventType is now behind the "NSEvent" feature — it resolves
                // correctly because we imported NSEvent above.
                let event_type: NSEventType = msg_send![event, type];
                match event_type {
                    NSEventType::KeyDown => {
                        let keycode: u16 = msg_send![event, keyCode];
                        let key = match keycode {
                            126 => Key::Up,
                            125 => Key::Down,
                            123 => Key::Left,
                            124 => Key::Right,
                            116 => Key::PageUp,
                            121 => Key::PageDown,
                            36  => Key::Enter,
                            53  => Key::Escape,
                            51  => Key::Backspace,
                            _   => Key::Char(' '),
                        };
                        if let Ok(mut q) = EVENT_QUEUE.lock() {
                            q.push(Event::KeyPress(key));
                        }
                    }
                    NSEventType::LeftMouseDown => {
                        let pt: NSPoint = msg_send![event, locationInWindow];
                        if let Ok(mut q) = EVENT_QUEUE.lock() {
                            q.push(Event::MouseClick {
                                x: pt.x as u32,
                                y: (self.height as f64 - pt.y) as u32,
                                button: MouseButton::Left,
                            });
                        }
                    }
                    NSEventType::RightMouseDown => {
                        let pt: NSPoint = msg_send![event, locationInWindow];
                        if let Ok(mut q) = EVENT_QUEUE.lock() {
                            q.push(Event::MouseClick {
                                x: pt.x as u32,
                                y: (self.height as f64 - pt.y) as u32,
                                button: MouseButton::Right,
                            });
                        }
                    }
                    NSEventType::MouseMoved | NSEventType::LeftMouseDragged => {
                        let pt: NSPoint = msg_send![event, locationInWindow];
                        if let Ok(mut q) = EVENT_QUEUE.lock() {
                            q.push(Event::MouseMove {
                                x: pt.x as u32,
                                y: (self.height as f64 - pt.y) as u32,
                            });
                        }
                    }
                    NSEventType::ScrollWheel => {
                        let dy: f64 = msg_send![event, scrollingDeltaY];
                        let delta = -(dy.round() as i32);
                        if delta != 0 {
                            if let Ok(mut q) = EVENT_QUEUE.lock() {
                                q.push(Event::Scroll(delta));
                            }
                        }
                    }
                    _ => {
                        let _: () = msg_send![self.app, sendEvent: event];
                    }
                }
            }

            // ── Resize detection ─────────────────────────────────────────
            let frame: NSRect = msg_send![self.view, frame];
            let w = frame.size.width  as u32;
            let h = frame.size.height as u32;
            if w != self.width || h != self.height {
                self.width  = w;
                self.height = h;
                if let Ok(mut q) = EVENT_QUEUE.lock() {
                    q.push(Event::Resize { width: w, height: h });
                }
            }
        }

        if let Ok(mut q) = EVENT_QUEUE.lock() {
            if !q.is_empty() {
                return Some(q.remove(0));
            }
        }
        None
    }

    fn width(&self)  -> u32 { self.width }
    fn height(&self) -> u32 { self.height }

    fn set_title(&mut self, title: &str) {
        unsafe {
            let ns_title = NSString::from_str(title);
            let _: () = msg_send![self.window, setTitle: &*ns_title];
        }
    }
}