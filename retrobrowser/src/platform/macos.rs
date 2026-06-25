// ============================================================
// RetroBrowser macOS Platform (Cocoa)
// Software framebuffer presented via NSBitmapImageRep.
// No GPU required. Stable Cocoa API since macOS 10.0.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================
//
// objc2 0.5 raw-pointer style: every ObjC call goes through
// msg_send! returning *mut AnyObject.  We deliberately avoid:
//   - msg_send_id!   (requires Retained<T> / MaybeUnwrap)
//   - T::alloc()     (requires IsAllocableAnyThread / MainThreadMarker)
//   - NSBackingStoreType::Buffered  (needs dual feature gates)
// Raw integer / pointer equivalents are used instead.
// ============================================================

use objc2::runtime::AnyObject;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{
    NSApplication,      // feature: NSApplication + NSResponder
    NSBitmapImageRep,   // feature: NSBitmapImageRep + NSImageRep
    NSEvent,            // feature: NSEvent  (pulls in NSEventMask, NSEventType)
    NSEventMask,
    NSEventType,
    NSGraphicsContext,  // feature: NSGraphicsContext
    NSScreen,           // feature: NSScreen
    NSView,             // feature: NSView   + NSResponder
    NSWindow,           // feature: NSWindow + NSResponder
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

// SAFETY: MacosWindow is only ever created and used on the main thread.
// The raw pointers are Cocoa objects whose lifetimes are managed by ObjC ARC.
unsafe impl Send for MacosWindow {}

impl PlatformWindow for MacosWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        unsafe {
            // ── NSApplication ────────────────────────────────────────────
            // msg_send! (not msg_send_id!) — returns *mut AnyObject directly.
            let app: *mut AnyObject =
                msg_send![NSApplication::class(), sharedApplication];
            // NSApplicationActivationPolicyRegular == 0
            let _: bool = msg_send![app, setActivationPolicy: 0i64]; // returns BOOL (type code 'c'), not void

            // ── NSWindow ─────────────────────────────────────────────────
            let rect = NSRect {
                origin: NSPoint { x: 200.0, y: 200.0 },
                size:   NSSize  { width: width as f64, height: height as f64 },
            };

            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::Miniaturizable;

            // alloc via msg_send! on the *Class* object — this avoids
            // NSWindow::alloc() which requires IsAllocableAnyThread
            // (not satisfied by MainThreadOnly in objc2 0.5).
            let alloc: *mut AnyObject = msg_send![NSWindow::class(), alloc];
            let window: *mut AnyObject = msg_send![
                alloc,
                initWithContentRect: rect,
                styleMask: style,
                // NSBackingStoreBuffered == 2. Using the integer avoids the
                // dual feature-gate (NSGraphics + NSWindow) that
                // NSBackingStoreType::Buffered requires.
                backing: 2u64,
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
            // ── NSBitmapImageRep ─────────────────────────────────────────
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
            // lockFocusIfCanDraw silently returns NO on non-opaque views in
            // newer macOS. The reliable render-loop approach is:
            //   1. Get a graphics context for the window
            //   2. saveGraphicsState / make it current
            //   3. draw the bitmap rep (returns BOOL per NSImageRep docs)
            //   4. restoreGraphicsState
            //   5. flushWindow → blit backing store to screen
            let gfx_ctx: *mut AnyObject = msg_send![
                NSGraphicsContext::class(),
                graphicsContextWithWindow: self.window
            ];
            if !gfx_ctx.is_null() {
                let _: () = msg_send![NSGraphicsContext::class(), saveGraphicsState];
                let _: () = msg_send![
                    NSGraphicsContext::class(),
                    setCurrentContext: gfx_ctx
                ];
                let rect = NSRect {
                    origin: NSPoint { x: 0.0, y: 0.0 },
                    size:   NSSize  { width: w as f64, height: h as f64 },
                };
                let _: bool = msg_send![bmp, drawInRect: rect];
                let _: () = msg_send![NSGraphicsContext::class(), restoreGraphicsState];
                let _: () = msg_send![self.window, flushWindow];
            }
        }
    }

    fn next_event(&mut self) -> Option<Event> {
        unsafe {
            // ── Pump the NSApplication event queue (non-blocking) ────────
            // untilDate: nil  →  return immediately if no event waiting.
            // Do NOT use msg_send_id! here; it returns *mut AnyObject fine
            // via msg_send! because NSEvent is not an init/copy/new method.
            loop {
                let event: *mut AnyObject = msg_send![
                    self.app,
                    nextEventMatchingMask: NSEventMask::Any,
                    untilDate: std::ptr::null::<AnyObject>(), // nil = non-blocking
                    inMode: ns_string!("kCFRunLoopDefaultMode"),
                    dequeue: true
                ];
                if event.is_null() {
                    break;
                }

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
