// ============================================================
// RetroBrowser Windows Platform (Win32 GDI)
// Software framebuffer presented via StretchDIBits.
// No GPU required. Stable Win32 API since Windows 95.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT, POINT},
    Graphics::Gdi::{
        BeginPaint, EndPaint, PAINTSTRUCT, StretchDIBits,
        BITMAPINFOHEADER, BITMAPINFO, DIB_RGB_COLORS, SRCCOPY,
        GetDC, ReleaseDC, RGBQUAD,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
        LoadCursorW, MSG, PeekMessageW, PostQuitMessage, RegisterClassExW,
        SetWindowTextW, ShowWindow, TranslateMessage, WNDCLASSEXW,
        CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, PM_REMOVE,
        SW_SHOW, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN,
        WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_QUIT, WM_SIZE,
        WM_RBUTTONDOWN, WM_MBUTTONDOWN, WS_OVERLAPPEDWINDOW,
        VK_UP, VK_DOWN, VK_LEFT, VK_RIGHT, VK_PRIOR, VK_NEXT,
        VK_RETURN, VK_ESCAPE, VK_BACK, GET_WHEEL_DELTA_WPARAM,
        WHEEL_DELTA,
    },
};

use super::{PlatformWindow, PixelBuffer, Event, Key, MouseButton};
use std::sync::Mutex;

// ─── Global event queue (Win32 callback can't carry state easily) ──

static EVENT_QUEUE: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static mut WINDOW_W: u32 = 0;
static mut WINDOW_H: u32 = 0;

unsafe extern "system" fn wnd_proc(
    hwnd:   HWND,
    msg:    u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLOSE | WM_DESTROY => {
            if let Ok(mut q) = EVENT_QUEUE.lock() { q.push(Event::Close); }
            PostQuitMessage(0);
            0
        }

        WM_SIZE => {
            let w = (lparam & 0xFFFF) as u32;
            let h = ((lparam >> 16) & 0xFFFF) as u32;
            if w > 0 && h > 0 {
                WINDOW_W = w;
                WINDOW_H = h;
                if let Ok(mut q) = EVENT_QUEUE.lock() {
                    q.push(Event::Resize { width: w, height: h });
                }
            }
            0
        }

        WM_PAINT => {
            let mut ps = PAINTSTRUCT {
                hdc: std::ptr::null_mut(),
                fErase: 0,
                rcPaint: RECT { left: 0, top: 0, right: 0, bottom: 0 },
                fRestore: 0,
                fIncUpdate: 0,
                rgbReserved: [0u8; 32],
            };
            BeginPaint(hwnd, &mut ps);
            EndPaint(hwnd, &ps);
            0
        }

        WM_KEYDOWN => {
            let key = match wparam as i32 {
                k if k == VK_UP    as i32 => Key::Up,
                k if k == VK_DOWN  as i32 => Key::Down,
                k if k == VK_LEFT  as i32 => Key::Left,
                k if k == VK_RIGHT as i32 => Key::Right,
                k if k == VK_PRIOR as i32 => Key::PageUp,
                k if k == VK_NEXT  as i32 => Key::PageDown,
                k if k == VK_RETURN as i32 => Key::Enter,
                k if k == VK_ESCAPE as i32 => Key::Escape,
                k if k == VK_BACK  as i32 => Key::Backspace,
                _ => Key::Char(' '),
            };
            if let Ok(mut q) = EVENT_QUEUE.lock() { q.push(Event::KeyPress(key)); }
            0
        }

        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as u32;
            let y = ((lparam >> 16) & 0xFFFF) as u32;
            if let Ok(mut q) = EVENT_QUEUE.lock() {
                q.push(Event::MouseMove { x, y });
            }
            0
        }

        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as u32;
            let y = ((lparam >> 16) & 0xFFFF) as u32;
            if let Ok(mut q) = EVENT_QUEUE.lock() {
                q.push(Event::MouseClick { x, y, button: MouseButton::Left });
            }
            0
        }

        WM_RBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as u32;
            let y = ((lparam >> 16) & 0xFFFF) as u32;
            if let Ok(mut q) = EVENT_QUEUE.lock() {
                q.push(Event::MouseClick { x, y, button: MouseButton::Right });
            }
            0
        }

        WM_MBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as u32;
            let y = ((lparam >> 16) & 0xFFFF) as u32;
            if let Ok(mut q) = EVENT_QUEUE.lock() {
                q.push(Event::MouseClick { x, y, button: MouseButton::Middle });
            }
            0
        }

        WM_MOUSEWHEEL => {
            let delta = GET_WHEEL_DELTA_WPARAM(wparam) as i32;
            let scroll = -(delta / WHEEL_DELTA as i32) * 3;
            if let Ok(mut q) = EVENT_QUEUE.lock() {
                q.push(Event::Scroll(scroll));
            }
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ─── WindowsWindow ─────────────────────────────────────────

pub struct WindowsWindow {
    hwnd:   HWND,
    width:  u32,
    height: u32,
    // Pre-converted BGR0 buffer for StretchDIBits
    bgr_buf: Vec<u8>,
}

impl PlatformWindow for WindowsWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        unsafe {
            WINDOW_W = width;
            WINDOW_H = height;

            let hinstance = GetModuleHandleW(std::ptr::null());

            let class_name: Vec<u16> = "RetroBrowserWnd\0".encode_utf16().collect();

            let wc = WNDCLASSEXW {
                cbSize:        std::mem::size_of::<WNDCLASSEXW>() as u32,
                style:         CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc:   Some(wnd_proc),
                cbClsExtra:    0,
                cbWndExtra:    0,
                hInstance:     hinstance,
                hIcon:         std::ptr::null_mut(),
                hCursor:       LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName:  std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm:       std::ptr::null_mut(),
            };
            RegisterClassExW(&wc);

            let title_wide: Vec<u16> = format!("{}\0", title).encode_utf16().collect();

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title_wide.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT, CW_USEDEFAULT,
                width as i32, height as i32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            );

            ShowWindow(hwnd, SW_SHOW);

            eprintln!("[RetroX] Win32 GDI window initialized ({}x{})", width, height);

            WindowsWindow { hwnd, width, height, bgr_buf: vec![0u8; (width * height * 4) as usize] }
        }
    }

    fn present(&mut self, buffer: &PixelBuffer) {
        let w = buffer.width;
        let h = buffer.height;
        let pixels = (w * h) as usize;

        // Grow buffer if needed
        if self.bgr_buf.len() < pixels * 4 {
            self.bgr_buf.resize(pixels * 4, 0);
        }

        // Convert RGBA → BGR0 (Win32 DIB format)
        unsafe {
            let src = buffer.data.as_ptr();
            let dst = self.bgr_buf.as_mut_ptr();
            for i in 0..pixels {
                let s = i * 4;
                let d = i * 4;
                *dst.add(d)     = *src.add(s + 2); // B
                *dst.add(d + 1) = *src.add(s + 1); // G
                *dst.add(d + 2) = *src.add(s);     // R
                *dst.add(d + 3) = 0;               // reserved
            }
        }

        // Blit to window using StretchDIBits
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize:          std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth:         w as i32,
                biHeight:        -(h as i32), // negative = top-down
                biPlanes:        1,
                biBitCount:      32,
                biCompression:   0, // BI_RGB
                biSizeImage:     0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed:       0,
                biClrImportant:  0,
            },
            bmiColors: [RGBQUAD { rgbBlue: 0, rgbGreen: 0, rgbRed: 0, rgbReserved: 0 }],
        };

        unsafe {
            let hdc = GetDC(self.hwnd);
            StretchDIBits(
                hdc,
                0, 0, w as i32, h as i32,
                0, 0, w as i32, h as i32,
                self.bgr_buf.as_ptr() as *const _,
                &bmi,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            ReleaseDC(self.hwnd, hdc);
        }
    }

    fn next_event(&mut self) -> Option<Event> {
        // Pump Win32 message queue
        unsafe {
            let mut msg = MSG {
                hwnd:    std::ptr::null_mut(),
                message: 0,
                wParam:  0,
                lParam:  0,
                time:    0,
                pt:      POINT { x: 0, y: 0 },
            };
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT { return Some(Event::Close); }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Return first queued event
        if let Ok(mut q) = EVENT_QUEUE.lock() {
            if !q.is_empty() { return Some(q.remove(0)); }
        }
        None
    }

    fn width(&self)  -> u32 { self.width }
    fn height(&self) -> u32 { self.height }

    fn set_title(&mut self, title: &str) {
        let wide: Vec<u16> = format!("{}\0", title).encode_utf16().collect();
        unsafe { SetWindowTextW(self.hwnd, wide.as_ptr()); }
    }
}