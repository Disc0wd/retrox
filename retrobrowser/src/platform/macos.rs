// ============================================================
// RetroBrowser macOS Platform (Cocoa)
// Software framebuffer → CGImage → CALayer.contents
// No GPU required. Works on macOS 10.14+ (Mojave and later).
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================
//
// Rendering strategy:
//   graphicsContextWithWindow: was removed in macOS 10.14 (always nil).
//   lockFocusIfCanDraw returns NO on layer-backed views (default post-10.14).
//   The correct modern approach for a software render loop:
//     1. Build a CGImage from raw RGBA bytes via CoreGraphics C API.
//     2. Set it as the `contents` of the view's backing CALayer.
//     3. Call display on the layer to push it to screen immediately.
//   This is zero-copy (CGDataProviderCreateWithData takes a pointer),
//   stable since macOS 10.0, and is exactly what game/emulator renderers use.
// ============================================================

use objc2::runtime::AnyObject;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{
    NSApplication,
    NSEvent,
    NSEventMask,
    NSEventType,
    NSScreen,
    NSView,
    NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{ns_string, NSPoint, NSRect, NSSize, NSString};

use super::{Event, Key, MouseButton, PixelBuffer, PlatformWindow};
use std::sync::Mutex;

// ── CoreGraphics types/functions (available as a plain C lib on macOS) ───────
// We link via the CoreGraphics framework that is always present.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> *mut std::ffi::c_void;
    fn CGColorSpaceRelease(cs: *mut std::ffi::c_void);
    fn CGDataProviderCreateWithData(
        info: *mut std::ffi::c_void,
        data: *const std::ffi::c_void,
        size: usize,
        release_data: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_void, usize)>,
    ) -> *mut std::ffi::c_void;
    fn CGDataProviderRelease(dp: *mut std::ffi::c_void);
    fn CGImageCreate(
        width: usize,
        height: usize,
        bits_per_component: usize,
        bits_per_pixel: usize,
        bytes_per_row: usize,
        color_space: *mut std::ffi::c_void,
        bitmap_info: u32,
        provider: *mut std::ffi::c_void,
        decode: *const std::ffi::c_void,
        should_interpolate: bool,
        intent: u32,
    ) -> *mut std::ffi::c_void;
    fn CGImageRelease(img: *mut std::ffi::c_void);
}

// kCGBitmapByteOrderDefault | kCGImageAlphaNoneSkipLast  = 0 | 4 = 4
// Our buffer is RGBA: R G B A where A is ignored (opaque).
// kCGBitmapByteOrder32Big = 4<<12 = 0x4000, kCGImageAlphaNoneSkipLast = 5 → 0x4005


const BITMAP_INFO_RGBA: u32 = 0x4005;
// kCGRenderingIntentDefault
const RENDERING_INTENT_DEFAULT: u32 = 0;

static EVENT_QUEUE: Mutex<Vec<Event>> = Mutex::new(Vec::new());

pub struct MacosWindow {
    app:      *mut AnyObject,
    window:   *mut AnyObject,
    view:     *mut AnyObject,
    layer:    *mut AnyObject,
    width:    u32,
    height:   u32,
    rgba_buf: Vec<u8>,
}

unsafe impl Send for MacosWindow {}

impl PlatformWindow for MacosWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        unsafe {
            // ── NSApplication ────────────────────────────────────────────
            let app: *mut AnyObject =
                msg_send![NSApplication::class(), sharedApplication];
            let _: bool = msg_send![app, setActivationPolicy: 0i64];

            // ── NSWindow ─────────────────────────────────────────────────
            let rect = NSRect {
                origin: NSPoint { x: 200.0, y: 200.0 },
                size:   NSSize  { width: width as f64, height: height as f64 },
            };
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::Miniaturizable;

            let alloc: *mut AnyObject = msg_send![NSWindow::class(), alloc];
            let window: *mut AnyObject = msg_send![
                alloc,
                initWithContentRect: rect,
                styleMask: style,
                backing: 2u64,  // NSBackingStoreBuffered
                defer: false
            ];

            let ns_title = NSString::from_str(title);
            let _: () = msg_send![window, setTitle: &*ns_title];

            // ── Enable layer-backing on the content view ─────────────────
            // Required for the CALayer contents approach to work.
            let view: *mut AnyObject = msg_send![window, contentView];
            let _: () = msg_send![view, setWantsLayer: true];

            // Grab the backing CALayer.
            let layer: *mut AnyObject = msg_send![view, layer];

            // Flip the layer coordinate system to match our top-left origin.
            // CALayer default is bottom-left; NSView is also bottom-left but
            // NSBitmapImageRep data comes top-left from our renderer.
            let _: () = msg_send![layer, setGeometryFlipped: true];

            let _: () = msg_send![
                window,
                makeKeyAndOrderFront: std::ptr::null::<AnyObject>()
            ];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];

            eprintln!("[RetroX] Cocoa window initialized ({}x{})", width, height);

            MacosWindow {
                app, window, view, layer,
                width, height,
                rgba_buf: vec![0u8; (width * height * 4) as usize],
            }
        }
    }

    fn present(&mut self, buffer: &PixelBuffer) {
        let w = buffer.width as usize;
        let h = buffer.height as usize;
        let size = w * h * 4;

        if self.rgba_buf.len() < size {
            self.rgba_buf.resize(size, 0);
        }
        self.rgba_buf[..size].copy_from_slice(&buffer.data[..size]);

        unsafe {
            // ── Build CGImage from raw RGBA bytes ─────────────────────────
            let cs = CGColorSpaceCreateDeviceRGB();
            let provider = CGDataProviderCreateWithData(
                std::ptr::null_mut(),
                self.rgba_buf.as_ptr() as *const std::ffi::c_void,
                size,
                None, // data is owned by rgba_buf; no release callback needed
            );
            let cg_image = CGImageCreate(
                w, h,
                8,      // bits per component
                32,     // bits per pixel
                w * 4,  // bytes per row
                cs,
                BITMAP_INFO_RGBA,
                provider,
                std::ptr::null(),
                false,
                RENDERING_INTENT_DEFAULT,
            );
            CGDataProviderRelease(provider);
            CGColorSpaceRelease(cs);

            // ── Push to screen via CALayer.contents ───────────────────────
            // `contents` accepts a CGImageRef (toll-free bridged to id).
            // After setting it, call display to commit immediately rather
            // than waiting for the next run-loop cycle.
            let _: () = msg_send![self.layer, setContents: cg_image as *mut AnyObject];
            let _: () = msg_send![self.layer, display];

            CGImageRelease(cg_image);
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
