//! The start-up splash: just the logo (`ui/logo.png`) floating in the middle of the screen in
//! a window of its own with no frame and no background, while the game gets ready behind it.
//! The logo is the progress bar: its letters fill with their colour from the left as the game
//! loads, the rest still grey.
//!
//! On Windows it is a layered window (per-pixel transparency, drawn with
//! `UpdateLayeredWindow`); elsewhere there is none.

/// How far along the logo is filled (0..1), eased toward what is set so it moves smoothly.
pub struct Splash {
    #[cfg(windows)]
    win: Option<imp::Window>,
    shown: f32,
    target: f32,
    last: std::time::Instant,
}

impl Splash {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            win: imp::Window::new(),
            shown: 0.0,
            target: 0.0,
            last: std::time::Instant::now(),
        }
    }

    /// Where loading has got to (0..1); redraws the logo filled that far (eased).
    pub fn set_progress(&mut self, p: f32) {
        self.target = self.target.max(p.clamp(0.0, 1.0));
        let now = std::time::Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let before = self.shown;
        self.shown += (self.target - self.shown) * (1.0 - (-dt * 6.0).exp());
        if self.target >= 1.0 && self.target - self.shown < 0.01 {
            self.shown = 1.0;
        }
        #[cfg(windows)]
        if let Some(w) = &mut self.win {
            if (self.shown - before).abs() > 1e-4 || !w.drawn {
                w.draw(self.shown);
            }
            w.pump();
        }
        let _ = before;
    }

    /// Whether the logo is (nearly) all filled in.
    pub fn full(&self) -> bool {
        self.shown >= 0.995
    }
}

/// The logo's pixels at `progress`: coloured up to that far from the left of its letters, a
/// light edge where the colour has got to, dim grey after it. RGBA, `w` x `h`.
fn paint(logo: &[u8], w: usize, h: usize, progress: f32) -> Vec<u8> {
    // The letters' extent, so the fill runs across them (not the empty margins).
    let (mut x0, mut x1) = (w, 0);
    for y in 0..h {
        for x in 0..w {
            if logo[(y * w + x) * 4 + 3] > 0 {
                x0 = x0.min(x);
                x1 = x1.max(x);
            }
        }
    }
    let edge = x0 as f32 + (x1.saturating_sub(x0) + 1) as f32 * progress;
    let mut out = logo.to_vec();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = logo[i + 3];
            if a == 0 {
                continue;
            }
            let fx = x as f32;
            let (r, g, b) = (logo[i] as f32, logo[i + 1] as f32, logo[i + 2] as f32);
            let (r, g, b, a) = if fx < edge {
                // Filled; brighter just behind the edge.
                let glow = 1.0 + 0.45 * (1.0 - ((edge - fx) / 14.0).min(1.0)) * (progress < 1.0) as i32 as f32;
                (r * glow, g * glow, b * glow, a as f32)
            } else {
                let l = 0.3 * r + 0.59 * g + 0.11 * b;
                let v = 26.0 + l * 0.32;
                (v, v, v * 1.05, a as f32 * 0.8)
            };
            out[i] = r.min(255.0) as u8;
            out[i + 1] = g.min(255.0) as u8;
            out[i + 2] = b.min(255.0) as u8;
            out[i + 3] = a as u8;
        }
    }
    out
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    #[repr(C)]
    struct WndClassW {
        style: u32,
        wnd_proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_ppm: i32,
        y_ppm: i32,
        clr_used: u32,
        clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 1],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Size {
        cx: i32,
        cy: i32,
    }

    #[repr(C)]
    struct BlendFunction {
        op: u8,
        flags: u8,
        constant_alpha: u8,
        alpha_format: u8,
    }

    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassW(class: *const WndClassW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *const c_void,
        ) -> isize;
        fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
        fn UpdateLayeredWindow(
            hwnd: isize,
            dst: isize,
            dst_pt: *const Point,
            size: *const Size,
            src: isize,
            src_pt: *const Point,
            key: u32,
            blend: *const BlendFunction,
            flags: u32,
        ) -> i32;
        fn PeekMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(dc: isize) -> isize;
        fn DeleteDC(dc: isize) -> i32;
        fn CreateDIBSection(
            dc: isize,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut c_void,
            section: isize,
            offset: u32,
        ) -> isize;
        fn SelectObject(dc: isize, obj: isize) -> isize;
        fn DeleteObject(obj: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    unsafe extern "system" fn wnd_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    pub struct Window {
        hwnd: isize,
        dc: isize,
        bitmap: isize,
        old: isize,
        bits: *mut u8,
        /// The logo scaled to its size on the screen (RGBA), and that size.
        logo: Vec<u8>,
        w: usize,
        h: usize,
        pos: Point,
        pub drawn: bool,
    }

    impl Window {
        pub fn new() -> Option<Window> {
            let img = crate::pack::decode_png(include_bytes!("ui/logo.png"))?;
            let (sw, sh) = unsafe { (GetSystemMetrics(0), GetSystemMetrics(1)) };
            if sw <= 0 || sh <= 0 {
                return None;
            }
            // About half the screen wide, in whole logo pixels where it can (crisp).
            let k = (sw as f32 * 0.5 / img.w as f32).clamp(0.3, 2.0);
            let (w, h) = ((img.w as f32 * k) as usize, (img.h as f32 * k) as usize);
            let mut logo = vec![0u8; w * h * 4];
            for y in 0..h {
                for x in 0..w {
                    let (sx, sy) = ((x as f32 / k) as usize, (y as f32 / k) as usize);
                    let s = (sy.min(img.h as usize - 1) * img.w as usize + sx.min(img.w as usize - 1)) * 4;
                    logo[(y * w + x) * 4..][..4].copy_from_slice(&img.rgba[s..s + 4]);
                }
            }
            unsafe {
                let instance = GetModuleHandleW(null());
                let class = wide("YourWorldsSplash");
                let wc = WndClassW {
                    style: 0,
                    wnd_proc,
                    cls_extra: 0,
                    wnd_extra: 0,
                    instance,
                    icon: 0,
                    cursor: 0,
                    background: 0,
                    menu_name: null(),
                    class_name: class.as_ptr(),
                };
                RegisterClassW(&wc);
                const WS_POPUP: u32 = 0x8000_0000;
                const WS_EX_LAYERED: u32 = 0x0008_0000;
                const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
                const WS_EX_TOPMOST: u32 = 0x0000_0008;
                const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
                let pos = Point { x: (sw - w as i32) / 2, y: (sh - h as i32) / 2 };
                let title = wide("Your Worlds");
                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP,
                    pos.x,
                    pos.y,
                    w as i32,
                    h as i32,
                    0,
                    0,
                    instance,
                    null(),
                );
                if hwnd == 0 {
                    return None;
                }
                let screen = GetDC(0);
                let dc = CreateCompatibleDC(screen);
                ReleaseDC(0, screen);
                let info = BitmapInfo {
                    header: BitmapInfoHeader {
                        size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                        width: w as i32,
                        // (negative: the rows from the top down)
                        height: -(h as i32),
                        planes: 1,
                        bit_count: 32,
                        compression: 0,
                        size_image: 0,
                        x_ppm: 0,
                        y_ppm: 0,
                        clr_used: 0,
                        clr_important: 0,
                    },
                    colors: [0],
                };
                let mut bits: *mut c_void = null_mut();
                let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
                if bitmap == 0 || bits.is_null() {
                    DeleteDC(dc);
                    DestroyWindow(hwnd);
                    return None;
                }
                let old = SelectObject(dc, bitmap);
                let mut win = Window { hwnd, dc, bitmap, old, bits: bits as *mut u8, logo, w, h, pos, drawn: false };
                win.draw(0.0);
                const SW_SHOWNOACTIVATE: i32 = 4;
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                win.pump();
                Some(win)
            }
        }

        /// Paints the logo filled `progress` far into the window.
        pub fn draw(&mut self, progress: f32) {
            let rgba = super::paint(&self.logo, self.w, self.h, progress);
            // Premultiplied BGRA.
            let out = unsafe { std::slice::from_raw_parts_mut(self.bits, self.w * self.h * 4) };
            for (o, p) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                let a = p[3] as u32;
                o[0] = (p[2] as u32 * a / 255) as u8;
                o[1] = (p[1] as u32 * a / 255) as u8;
                o[2] = (p[0] as u32 * a / 255) as u8;
                o[3] = p[3];
            }
            let size = Size { cx: self.w as i32, cy: self.h as i32 };
            let src = Point { x: 0, y: 0 };
            let blend = BlendFunction { op: 0, flags: 0, constant_alpha: 255, alpha_format: 1 };
            unsafe {
                let screen = GetDC(0);
                UpdateLayeredWindow(self.hwnd, screen, &self.pos, &size, self.dc, &src, 0, &blend, 2);
                ReleaseDC(0, screen);
            }
            self.drawn = true;
        }

        /// Handles the window's messages (so it is not thought to hang).
        pub fn pump(&self) {
            let mut msg: Msg = unsafe { std::mem::zeroed() };
            unsafe {
                while PeekMessageW(&mut msg, self.hwnd, 0, 0, 1) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.old);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
                DestroyWindow(self.hwnd);
            }
        }
    }
}
