// ============================================================
// RetroX Wayland Platform
// Native Wayland client using wl_shm shared memory buffers.
// No XWayland. Direct compositor communication.
// Page buffer stored pre-converted to XRGB8888 to avoid
// per-frame pixel conversion during present.
// wayland-client =0.31.14 | Rust 1.95.0 | Edition 2021
// ============================================================

use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_pointer::{self, WlPointer},
        wl_registry::WlRegistry,
        wl_seat::WlSeat,
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

use super::{PlatformWindow, PixelBuffer, Event, Key, MouseButton};
use std::os::unix::io::RawFd;
use libc;

// ─── Shared Memory Buffer ──────────────────────────────────

struct ShmBuffer {
    fd:     RawFd,
    ptr:    *mut u8,
    size:   usize,
    pool:   WlShmPool,
    buffer: WlBuffer,
    width:  u32,
    height: u32,
}

impl ShmBuffer {
    fn new(shm: &WlShm, width: u32, height: u32, qh: &QueueHandle<WaylandState>) -> Option<Self> {
        let size = (width * height * 4) as usize;

        let name = c"retrox-shm";
        let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 { return None; }

        if unsafe { libc::ftruncate(fd, size as libc::off_t) } < 0 {
            unsafe { libc::close(fd); }
            return None;
        }

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            unsafe { libc::close(fd); }
            return None;
        }

        let pool   = shm.create_pool(
            unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) },
            size as i32, qh, ()
        );
        let buffer = pool.create_buffer(
            0, width as i32, height as i32, (width * 4) as i32,
            wl_shm::Format::Xrgb8888, qh, (),
        );

        Some(ShmBuffer { fd, ptr: ptr as *mut u8, size, pool, buffer, width, height })
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
    }

    fn destroy(self) {
        self.buffer.destroy();
        self.pool.destroy();
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.size);
            libc::close(self.fd);
        }
    }
}

// ─── Wayland State ─────────────────────────────────────────

struct WaylandState {
    compositor:     Option<WlCompositor>,
    shm:            Option<WlShm>,
    wm_base:        Option<XdgWmBase>,
    seat:           Option<WlSeat>,
    surface:        Option<WlSurface>,
    xdg_surface:    Option<XdgSurface>,
    toplevel:       Option<XdgToplevel>,
    events:         Vec<Event>,
    pending_scroll: f64,
    pointer_x:      f64,
    pointer_y:      f64,
    width:          u32,
    height:         u32,
    configured:     bool,
    closed:         bool,
}

impl WaylandState {
    fn new(width: u32, height: u32) -> Self {
        WaylandState {
            compositor: None, shm: None, wm_base: None, seat: None,
            surface: None, xdg_surface: None, toplevel: None,
            events:         Vec::new(),
            pending_scroll: 0.0,
            pointer_x: 0.0, pointer_y: 0.0,
            width, height,
            configured: false,
            closed: false,
        }
    }
}

// ─── Registry ──────────────────────────────────────────────

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(state: &mut Self, registry: &WlRegistry,
             event: wayland_client::protocol::wl_registry::Event,
             _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        use wayland_client::protocol::wl_registry::Event;
        if let Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => { state.compositor = Some(registry.bind(name, version.min(4), qh, ())); }
                "wl_shm"        => { state.shm        = Some(registry.bind(name, version.min(1), qh, ())); }
                "xdg_wm_base"   => { state.wm_base    = Some(registry.bind(name, version.min(2), qh, ())); }
                "wl_seat"       => { state.seat        = Some(registry.bind(name, version.min(7), qh, ())); }
                _ => {}
            }
        }
    }
}

// ─── No-op dispatches ──────────────────────────────────────

impl Dispatch<WlCompositor, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlCompositor, _: wayland_client::protocol::wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShm, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShmPool, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlShmPool, _: wayland_client::protocol::wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlBuffer, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlBuffer, _: wayland_client::protocol::wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSurface, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlSurface, _: wayland_client::protocol::wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

// ─── XDG Shell ─────────────────────────────────────────────

impl Dispatch<XdgWmBase, ()> for WaylandState {
    fn event(_: &mut Self, wm_base: &XdgWmBase, event: xdg_wm_base::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event { wm_base.pong(serial); }
    }
}

impl Dispatch<XdgSurface, ()> for WaylandState {
    fn event(state: &mut Self, xdg_surface: &XdgSurface, event: xdg_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<XdgToplevel, ()> for WaylandState {
    fn event(state: &mut Self, _: &XdgToplevel, event: xdg_toplevel::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0 && height > 0 {
                    let (w, h) = (width as u32, height as u32);
                    if w != state.width || h != state.height {
                        state.width  = w;
                        state.height = h;
                        state.events.push(Event::Resize { width: w, height: h });
                    }
                }
            }
            xdg_toplevel::Event::Close => {
                state.closed = true;
                state.events.push(Event::Close);
            }
            _ => {}
        }
    }
}

// ─── Seat / Input ──────────────────────────────────────────

impl Dispatch<WlSeat, ()> for WaylandState {
    fn event(_state: &mut Self, seat: &WlSeat, event: wayland_client::protocol::wl_seat::Event,
             _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        use wayland_client::protocol::wl_seat::{Capability, Event};
        if let Event::Capabilities { capabilities } = event {
            let caps = Capability::from_bits_truncate(capabilities.into());
            if caps.contains(Capability::Pointer)  { seat.get_pointer(qh, ()); }
            if caps.contains(Capability::Keyboard) { seat.get_keyboard(qh, ()); }
        }
    }
}

impl Dispatch<WlPointer, ()> for WaylandState {
    fn event(state: &mut Self, _: &WlPointer, event: wl_pointer::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                state.pointer_x = surface_x;
                state.pointer_y = surface_y;
                state.events.push(Event::MouseMove {
                    x: surface_x as u32,
                    y: surface_y as u32,
                });
            }
            wl_pointer::Event::Button { button, state: btn_state, .. } => {
                use wayland_client::protocol::wl_pointer::ButtonState;
                if btn_state == wayland_client::WEnum::Value(ButtonState::Pressed) {
                    let mb = match button {
                        0x110 => MouseButton::Left,
                        0x111 => MouseButton::Right,
                        0x112 => MouseButton::Middle,
                        _     => MouseButton::Left,
                    };
                    state.events.push(Event::MouseClick {
                        x: state.pointer_x as u32,
                        y: state.pointer_y as u32,
                        button: mb,
                    });
                }
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                use wayland_client::protocol::wl_pointer::Axis;
                if axis == wayland_client::WEnum::Value(Axis::VerticalScroll) {
                    state.pending_scroll += value;
                }
            }
            wl_pointer::Event::Frame => {
                if state.pending_scroll != 0.0 {
                    let delta = state.pending_scroll.round() as i32;
                    if delta != 0 {
                        state.events.push(Event::Scroll(delta));
                    }
                    state.pending_scroll = 0.0;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for WaylandState {
    fn event(state: &mut Self, _: &WlKeyboard, event: wl_keyboard::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_keyboard::Event::Key { key, state: key_state, .. } = event {
            use wayland_client::protocol::wl_keyboard::KeyState;
            if key_state == wayland_client::WEnum::Value(KeyState::Pressed) {
                let mapped = match key {
                    103 => Key::Up,
                    108 => Key::Down,
                    105 => Key::Left,
                    106 => Key::Right,
                    104 => Key::PageUp,
                    109 => Key::PageDown,
                    28  => Key::Enter,
                    1   => Key::Escape,
                    14  => Key::Backspace,
                    _   => Key::Char(' '),
                };
                state.events.push(Event::KeyPress(mapped));
            }
        }
    }
}

// ─── WaylandWindow ─────────────────────────────────────────

pub struct WaylandWindow {
    conn:    Connection,
    queue:   EventQueue<WaylandState>,
    state:   WaylandState,
    qh:      QueueHandle<WaylandState>,
    shm_buf: Option<ShmBuffer>,
    // Pre-converted XRGB buffer — avoids per-frame RGBA→XRGB conversion
    xrgb_buf: Vec<u8>,
}

impl PlatformWindow for WaylandWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        let conn      = Connection::connect_to_env()
            .expect("RetroX: failed to connect to Wayland compositor");
        let mut queue = conn.new_event_queue::<WaylandState>();
        let qh        = queue.handle();
        let mut state = WaylandState::new(width, height);

        let display  = conn.display();
        let _registry = display.get_registry(&qh, ());
        queue.roundtrip(&mut state).expect("RetroX: registry roundtrip");

        let compositor = state.compositor.as_ref().expect("RetroX: no wl_compositor");
        let wm_base    = state.wm_base.as_ref().expect("RetroX: no xdg_wm_base");

        let surface     = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel    = xdg_surface.get_toplevel(&qh, ());

        toplevel.set_title(title.to_string());
        toplevel.set_app_id("retrox".to_string());
        surface.commit();

        state.surface     = Some(surface);
        state.xdg_surface = Some(xdg_surface);
        state.toplevel    = Some(toplevel);

        queue.roundtrip(&mut state).expect("RetroX: configure roundtrip");

        eprintln!("[RetroX] Wayland native ({}x{})", state.width, state.height);

        WaylandWindow { conn, queue, state, qh, shm_buf: None, xrgb_buf: Vec::new() }
    }

    fn present(&mut self, buffer: &PixelBuffer) {
        let w = buffer.width;
        let h = buffer.height;
        let pixels = (w * h) as usize;
        let size   = pixels * 4;

        // Reallocate SHM buffer if size changed
        let needs_realloc = self.shm_buf.as_ref()
            .map(|b| b.width != w || b.height != h)
            .unwrap_or(true);

        if needs_realloc {
            if let Some(old) = self.shm_buf.take() { old.destroy(); }
            let shm = self.state.shm.as_ref().expect("RetroX: no wl_shm");
            self.shm_buf  = ShmBuffer::new(shm, w, h, &self.qh);
            self.xrgb_buf = vec![0u8; size];
        }

        // Convert RGBA → XRGB8888 into xrgb_buf
        // XRGB8888 little-endian in memory: B G R X
        let src = &buffer.data;
        let dst = &mut self.xrgb_buf;
        unsafe {
            let s = src.as_ptr();
            let d = dst.as_mut_ptr();
            for i in 0..pixels {
                let si = i * 4;
                let di = i * 4;
                *d.add(di)     = *s.add(si + 2); // B
                *d.add(di + 1) = *s.add(si + 1); // G
                *d.add(di + 2) = *s.add(si);     // R
                *d.add(di + 3) = 0xFF;            // X
            }
        }

        // Memcpy xrgb_buf into SHM
        let shm_buf = match self.shm_buf.as_mut() {
            Some(b) => b,
            None    => { eprintln!("[RetroX] SHM buffer allocation failed"); return; }
        };
        let shm_slice = shm_buf.as_slice_mut();
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.xrgb_buf.as_ptr(),
                shm_slice.as_mut_ptr(),
                size,
            );
        }

        // Commit to compositor
        let surface = self.state.surface.as_ref().unwrap();
        surface.attach(Some(&shm_buf.buffer), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
        self.conn.flush().ok();
    }

    fn next_event(&mut self) -> Option<Event> {
        self.queue.dispatch_pending(&mut self.state).ok();
        if let Some(guard) = self.queue.prepare_read() {
            guard.read().ok();
            self.queue.dispatch_pending(&mut self.state).ok();
        }

        if self.state.events.is_empty() {
            None
        } else {
            Some(self.state.events.remove(0))
        }
    }

    fn width(&self)  -> u32 { self.state.width }
    fn height(&self) -> u32 { self.state.height }

    fn set_title(&mut self, title: &str) {
        if let Some(ref toplevel) = self.state.toplevel {
            toplevel.set_title(title.to_string());
        }
        self.conn.flush().ok();
    }
}