// ============================================================
// RetroX Windows Platform (Win32)
// Stub - implement when targeting Windows
// Rust 1.95.0 | Edition 2021
// ============================================================

use super::{PlatformWindow, PixelBuffer, Event};

pub struct WindowsWindow {
    width:  u32,
    height: u32,
}

impl PlatformWindow for WindowsWindow {
    fn new(_title: &str, width: u32, height: u32) -> Self {
        WindowsWindow { width, height }
    }
    fn present(&mut self, _buffer: &PixelBuffer) {
        todo!("Windows platform not yet implemented")
    }
    fn next_event(&mut self) -> Option<Event> {
        todo!("Windows platform not yet implemented")
    }
    fn width(&self)  -> u32 { self.width }
    fn height(&self) -> u32 { self.height }
    fn set_title(&mut self, _title: &str) {
        todo!("Windows platform not yet implemented")
    }
}
