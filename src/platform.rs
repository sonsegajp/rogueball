//! The few things that differ between the desktop exe and the browser build:
//! small key/value storage (saves), the diagnostics log, and gamepad state.
//! Desktop uses files under %APPDATA%\Rogueball and XInput; the web build calls into web/rogueball.js.

/// Gamepad buttons, in the W3C "standard gamepad" order (XInput is remapped to match).
pub mod btn {
    pub const A: u32 = 0;
    pub const B: u32 = 1;
    pub const X: u32 = 2;
    pub const Y: u32 = 3;
    pub const LB: u32 = 4;
    pub const RB: u32 = 5;
    pub const LT: u32 = 6;
    pub const RT: u32 = 7;
    pub const BACK: u32 = 8;
    pub const START: u32 = 9;
    pub const UP: u32 = 12;
    pub const DOWN: u32 = 13;
    pub const LEFT: u32 = 14;
    pub const RIGHT: u32 = 15;
}

#[derive(Default, Clone, Copy)]
pub struct PadState { pub connected: bool, pub buttons: u32, pub lx: f32, pub ly: f32 }

/// gamepad with edge detection; call `poll` once per frame
#[derive(Default)]
pub struct Pad { pub now: PadState, prev: u32, #[allow(dead_code)] retry: u32, #[allow(dead_code)] slot: Option<u32> }

impl Pad {
    pub fn held(&self, b: u32) -> bool { self.now.buttons & (1 << b) != 0 }
    pub fn pressed(&self, b: u32) -> bool { self.held(b) && self.prev & (1 << b) == 0 }
    pub fn any_pressed(&self) -> bool { self.now.buttons & !self.prev != 0 }
    pub fn poll(&mut self) {
        self.prev = self.now.buttons;
        self.now = self.read();
        // the stick doubles as a d-pad for menus
        let (x, y) = (self.now.lx, self.now.ly);
        if x < -0.6 { self.now.buttons |= 1 << btn::LEFT + 16; }
        if x > 0.6 { self.now.buttons |= 1 << btn::RIGHT + 16; }
        if y < -0.6 { self.now.buttons |= 1 << btn::UP + 16; }
        if y > 0.6 { self.now.buttons |= 1 << btn::DOWN + 16; }
    }
    /// d-pad or stick direction pressed this frame (for menu navigation)
    pub fn nav(&self, b: u32) -> bool {
        let both = |s: u32| (s >> b) & 1 != 0 || (s >> (b + 16)) & 1 != 0;
        both(self.now.buttons) && !both(self.prev)
    }

    #[cfg(windows)]
    fn read(&mut self) -> PadState {
        use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
        // polling an empty slot can stall, so only look for a new controller every couple of seconds
        let slots: Vec<u32> = match self.slot {
            Some(s) => vec![s],
            None => {
                if self.retry > 0 { self.retry -= 1; return PadState::default(); }
                self.retry = 120;
                (0..4).collect()
            }
        };
        for s in slots {
            let mut st: XINPUT_STATE = unsafe { std::mem::zeroed() };
            if unsafe { XInputGetState(s, &mut st) } == 0 {
                self.slot = Some(s);
                let g = st.Gamepad;
                let map = [(0x1000, btn::A), (0x2000, btn::B), (0x4000, btn::X), (0x8000, btn::Y), (0x100, btn::LB), (0x200, btn::RB),
                    (0x20, btn::BACK), (0x10, btn::START), (0x1, btn::UP), (0x2, btn::DOWN), (0x4, btn::LEFT), (0x8, btn::RIGHT)];
                let mut b = 0u32;
                for (mask, bit) in map { if g.wButtons & mask != 0 { b |= 1 << bit; } }
                if g.bLeftTrigger > 40 { b |= 1 << btn::LT; }
                if g.bRightTrigger > 40 { b |= 1 << btn::RT; }
                let axis = |v: i16| { let f = v as f32 / 32767.0; if f.abs() < 0.24 { 0.0 } else { f } };
                // XInput's Y axis points up; the standard mapping points down
                return PadState { connected: true, buttons: b, lx: axis(g.sThumbLX), ly: -axis(g.sThumbLY) };
            }
        }
        self.slot = None;
        PadState::default()
    }

    #[cfg(target_arch = "wasm32")]
    fn read(&mut self) -> PadState {
        let b = unsafe { web::rb_pad_buttons() };
        if b < 0 { return PadState::default(); }
        let axis = |i| { let f = unsafe { web::rb_pad_axis(i) }; if f.abs() < 0.24 { 0.0 } else { f } };
        PadState { connected: true, buttons: b as u32, lx: axis(0), ly: axis(1) }
    }

    #[cfg(not(any(windows, target_arch = "wasm32")))]
    fn read(&mut self) -> PadState { PadState::default() }
}

#[cfg(target_arch = "wasm32")]
mod web {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn rb_log(ptr: *const u8, len: usize);
        pub fn rb_store_set(kp: *const u8, kl: usize, vp: *const u8, vl: usize);
        pub fn rb_store_len(kp: *const u8, kl: usize) -> i32;
        pub fn rb_store_get(kp: *const u8, kl: usize, out: *mut u8);
        pub fn rb_pad_buttons() -> i32;
        pub fn rb_pad_axis(i: i32) -> f32;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn data_dir() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(|_| std::path::PathBuf::from("."));
    base.join("Rogueball")
}

/// read a stored value (a save file on desktop, localStorage in the browser)
pub fn load(key: &str) -> Option<Vec<u8>> {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        let n = web::rb_store_len(key.as_ptr(), key.len());
        if n < 0 { return None; }
        let mut buf = vec![0u8; n as usize];
        web::rb_store_get(key.as_ptr(), key.len(), buf.as_mut_ptr());
        Some(buf)
    }
    #[cfg(not(target_arch = "wasm32"))]
    { std::fs::read(data_dir().join(key)).ok() }
}

pub fn save(key: &str, val: &[u8]) {
    #[cfg(target_arch = "wasm32")]
    unsafe { web::rb_store_set(key.as_ptr(), key.len(), val.as_ptr(), val.len()); }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let d = data_dir();
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(key), val);
    }
}

/// append a line to the diagnostics log (the browser console on the web)
pub fn log(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    unsafe { web::rb_log(msg.as_ptr(), msg.len()); }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Write;
        let p = data_dir().join("log.txt");
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        // keep the log from growing without bound
        if std::fs::metadata(&p).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
            let _ = std::fs::remove_file(&p);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let _ = writeln!(f, "[{t}] {msg}");
        }
    }
}

pub const IS_WEB: bool = cfg!(target_arch = "wasm32");
