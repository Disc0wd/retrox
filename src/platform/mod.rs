// ============================================================
// RetroX Platform Abstraction Layer
// Defines the common interface all platforms must implement.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Enter,
    Escape,
    Backspace,
    Char(char),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
pub enum Event {
    Close,
    KeyPress(Key),
    MouseClick { x: u32, y: u32, button: MouseButton },
    MouseMove { x: u32, y: u32 },
    Scroll(i32),           // positive = down, negative = up
    Resize { width: u32, height: u32 },
}

/// RGBA pixel buffer
pub struct PixelBuffer {
    pub width:  u32,
    pub height: u32,
    pub data:   Vec<u8>,   // RGBA, 4 bytes per pixel
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        PixelBuffer {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x >= self.width || y >= self.height { return; }
        let idx = ((y * self.width + x) * 4) as usize;
        self.data[idx]     = r;
        self.data[idx + 1] = g;
        self.data[idx + 2] = b;
        self.data[idx + 3] = a;
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        if x >= self.width || y >= self.height { return (0, 0, 0, 0); }
        let idx = ((y * self.width + x) * 4) as usize;
        (self.data[idx], self.data[idx+1], self.data[idx+2], self.data[idx+3])
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        for i in 0..(self.width * self.height) as usize {
            self.data[i * 4]     = r;
            self.data[i * 4 + 1] = g;
            self.data[i * 4 + 2] = b;
            self.data[i * 4 + 3] = 255;
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width  = width;
        self.height = height;
        self.data   = vec![0u8; (width * height * 4) as usize];
    }
}

/// Platform window trait - every platform implements this
pub trait PlatformWindow {
    fn new(title: &str, width: u32, height: u32) -> Self where Self: Sized;
    fn present(&mut self, buffer: &PixelBuffer);
    fn next_event(&mut self) -> Option<Event>;
    fn width(&self)  -> u32;
    fn height(&self) -> u32;
    fn set_title(&mut self, title: &str);
}

// Platform selection
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxWindow as NativeWindow;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsWindow as NativeWindow;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacosWindow as NativeWindow;
