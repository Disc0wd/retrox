// ============================================================
// RetroX macOS Platform (Cocoa)
// Stub - implement when targeting macOS
// Rust 1.95.0 | Edition 2021
// ============================================================

use super::{PlatformWindow, PixelBuffer, Event};

pub struct MacosWindow {
    width:  u32,
    height: u32,
}

impl PlatformWindow for MacosWindow {
    fn new(_title: &str, width: u32, height: u32) -> Self {
        MacosWindow { width, height }
    }
    fn present(&mut self, _buffer: &PixelBuffer) {
        todo!("macOS platform not yet implemented")
    }
    fn next_event(&mut self) -> Option<Event> {
        todo!("macOS platform not yet implemented")
    }
    fn width(&self)  -> u32 { self.width }
    fn height(&self) -> u32 { self.height }
    fn set_title(&mut self, _title: &str) {
        todo!("macOS platform not yet implemented")
    }
}
