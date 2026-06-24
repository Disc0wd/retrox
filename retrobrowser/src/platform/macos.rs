// ============================================================
// RetroBrowser macOS Platform (Cocoa)
// Software framebuffer presented via NSBitmapImageRep.
// No GPU required. Stable Cocoa API since macOS 10.0.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id, ClassType};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBitmapImageRep,
    NSEventMask, NSEventType, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSRect, NSPoint, NSSize, NSString};

use super::{PlatformWindow, PixelBuffer, Event, Key, MouseButton};
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
            // Initialize NSApplication
            let app: *mut AnyObject = msg_send_id![NSApplication::class(), sharedApplication];

            // setActivationPolicy: returns BOOL (i8 on macOS)
            let _: i8 = msg_send![app, setActivationPolicy: 0i64];

            let rect = NSRect {
                origin: NSPoint { x: 200.0, y: 200.0 },
                size:   NSSize  { width: width as f64, height: height as f64 },
            };

            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::Miniaturizable;

            let window: *mut AnyObject = msg_send_id![
                NSWindow::alloc(),
                initWithContentRect: rect,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ];

            let ns_title = NSString::from_str(title);
            let _: () = msg_send![window, setTitle: &*ns_title];
            let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];

            let view: *mut AnyObject = msg_send![window, contentView];

            let _: () = msg_send![app, activateIgnoringOtherApps: true];

            // Run one event loop pass to actually show the window
            let distant_past: *mut AnyObject = msg_send_id![
                objc2_foundation::NSDate::class(), distantPast
            ];
            loop {
                let event: *mut AnyObject = msg_send![
                    app,
                    nextEventMatchingMask: NSEventMask::Any,
                    untilDate: distant_past,
                    inMode: &*NSString::from_str("kCFRunLoopDefaultMode"),
                    dequeue: true
                ];
                if event.is_null() { break; }
                let _: () = msg_send![app, sendEvent: event];
            }
            let _: () = msg_send![app, updateWindows];

            eprintln!("[RetroX] Cocoa window initialized ({}x{})", width, height);

            MacosWindow {
                app, window, view,
                width, height,
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
            // Create NSBitmapImageRep from raw RGBA data
            let color_space = NSString::from_str("NSDeviceRGBColorSpace");
            let bmp: *mut AnyObject = msg_send_id![
                NSBitmapImageRep::alloc(),
                initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>(),
                pixelsWide: w as i64,
                pixelsHigh: h as i64,
                bitsPerSample: 8i64,
                samplesPerPixel: 4i64,
                hasAlpha: true,
                isPlanar: false,
                colorSpaceName: &*color_space,
                bytesPerRow: (w * 4) as i64,
                bitsPerPixel: 32i64
            ];

            // Copy pixels into bitmap
            let bitmap_data: *mut u8 = msg_send![bmp, bitmapData];
            std::ptr::copy_nonoverlapping(self.rgba_buf.as_ptr(), bitmap_data, size);

            // Lock focus on view and draw
            let _: () = msg_send![self.view, lockFocus];
            let rect = NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size:   NSSize  { width: w as f64, height: h as f64 },
            };
            let _: () = msg_send![bmp, drawInRect: rect];
            let _: () = msg_send![self.view, unlockFocus];
            let _: () = msg_send![self.window, flushWindow];
        }
    }

    fn next_event(&mut self) -> Option<Event> {
        unsafe {
            let distant_past: *mut AnyObject = msg_send_id![
                objc2_foundation::NSDate::class(), distantPast
            ];

            loop {
                let event: *mut AnyObject = msg_send![
                    self.app,
                    nextEventMatchingMask: NSEventMask::Any,
                    untilDate: distant_past,
                    inMode: &*NSString::from_str("kCFRunLoopDefaultMode"),
                    dequeue: true
                ];
                if event.is_null() { break; }

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

            // Check for resize
            let frame: NSRect = msg_send![self.view, frame];
            let w = frame.size.width  as u32;
            let h = frame.size.height as u32;
            if w > 0 && h > 0 && (w != self.width || h != self.height) {
                self.width  = w;
                self.height = h;
                if let Ok(mut q) = EVENT_QUEUE.lock() {
                    q.push(Event::Resize { width: w, height: h });
                }
            }
        }

        if let Ok(mut q) = EVENT_QUEUE.lock() {
            if !q.is_empty() { return Some(q.remove(0)); }
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