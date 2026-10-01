//! Immediate-mode UI: rounded SDF rectangles, a pixel bitmap font, buttons and sliders; the
//! menus built with it (`screens`) and the chat (`chat`).
//!
//! Here: the colours, the vertex format and `Ui` itself (a frame begun and finished, the
//! keyboard moving through a screen's controls, entrance animations, clipping). The drawing
//! is in `draw`, the controls in `widgets`, the font in `font`.

pub mod chat;
pub mod screens;

mod draw;
mod font;
mod widgets;

pub use font::Font;

use font8x8::{UnicodeFonts, BASIC_FONTS, LATIN_FONTS};
use glam::Vec2;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub const FONT_TEX_W: u32 = 1024;
pub const FONT_TEX_H: u32 = 512;

/// The menus' and the HUD's typeface (from the system's fonts; the pixel font where it is
/// missing).
const SMOOTH_FONT: &str = "C:/Windows/Fonts/seguisb.ttf";
/// Atlas pixels per font unit of the smooth font (text is 8 units tall at size 1).
const FONT_RES: f32 = 4.0;

pub type Color = [f32; 4];

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

/// What a key did to a one-line text box being typed in (`edit_line`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineEdit {
    /// The text changed (a character typed, or one taken back).
    Changed,
    /// Enter or Tab: done typing.
    Done,
    Escape,
    None,
}

/// A key pressed while a one-line text box has the keyboard: Backspace takes the last
/// character back, typed characters go on the end (up to `max`), Enter/Tab finish.
pub fn edit_line(text: &mut String, code: winit::keyboard::KeyCode, typed: Option<&str>, max: usize) -> LineEdit {
    use winit::keyboard::KeyCode;
    match code {
        KeyCode::Escape => LineEdit::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab => LineEdit::Done,
        KeyCode::Backspace => {
            text.pop();
            LineEdit::Changed
        }
        _ => {
            let mut changed = LineEdit::None;
            for c in typed.unwrap_or("").chars() {
                if !c.is_control() && text.chars().count() < max {
                    text.push(c);
                    changed = LineEdit::Changed;
                }
            }
            changed
        }
    }
}

pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    [c[0], c[1], c[2], c[3] * a]
}

/// Shown in the main menu and on the F3 screen.
pub const VERSION: &str = "Your Worlds 0.4.0";

pub const WHITE: Color = rgba(255, 255, 255, 255);
/// The menus' colours: dark neutral grey, indigo violet only to pick things out.
pub const ACCENT: Color = rgba(118, 108, 236, 255);
pub const ACCENT_LIGHT: Color = rgba(172, 164, 255, 255);
pub const DANGER: Color = rgba(226, 78, 98, 255);
pub const HOVER_TEXT: Color = rgba(255, 255, 255, 255);
pub const GLASS_TOP: Color = rgba(24, 24, 30, 214);
pub const GLASS_BOTTOM: Color = rgba(18, 18, 24, 222);

/// Eased 0..1: fast at first, gently settling.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// How a button looks: a plain one, the main one of a screen, or one that destroys something.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Normal,
    Primary,
    Danger,
}

const MODE_RECT: f32 = 0.0;
const MODE_TEXT: f32 = 1.0;
const MODE_VIGNETTE: f32 = 2.0;
const MODE_BLOCK: f32 = 3.0;
const MODE_RING: f32 = 4.0;
const MODE_IMAGE: f32 = 5.0;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub rect: [f32; 4],
    pub mode: f32,
}

fn hash_id(label: &str, x: f32, y: f32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    label.hash(&mut h);
    (x as i32).hash(&mut h);
    (y as i32).hash(&mut h);
    h.finish()
}

pub struct Ui {
    pub verts: Vec<UiVertex>,
    pub font: Font,
    /// The pixel font (the guide book is drawn in it).
    pub pixel_font: Font,
    pub w: f32,
    pub h: f32,
    /// GUI scale (size of one "GUI pixel" in screen pixels).
    pub s: f32,
    pub mouse: Vec2,
    pub mouse_down: bool,
    pub pressed: bool,
    pub input_enabled: bool,
    pub dt: f32,
    pub time: f32,
    /// Set when any button was clicked this frame (for click sounds).
    pub clicked: bool,
    /// Text typed this frame and backspace presses, consumed by the focused text field.
    pub typed: String,
    pub backspace: u32,
    pub right_pressed: bool,
    pub shift: bool,
    pub scroll: f32,
    /// Scissor regions: (first vertex, clip rectangle x, y, w, h in pixels; None = no clip).
    pub clips: Vec<(u32, Option<[f32; 4]>)>,
    clip: Option<[f32; 4]>,
    focus: Option<u64>,
    anims: HashMap<u64, f32>,
    active: Option<u64>,
    tooltip: Option<String>,
    /// Everything drawn is this see-through (0..1) and moved by `offset` (the screens'
    /// entrance animations, see `style`).
    pub fade: f32,
    pub offset: Vec2,
    /// Seconds since the open screen opened, and which it is (`screen`); how many elements
    /// have come in so far this frame (`appear`).
    pub age: f32,
    screen_key: u64,
    stagger: u32,
    /// Moving through the controls with the keyboard (see `nav_key`).
    nav: Nav,
}

/// Keyboard control of a screen: the focused control (by its order on the screen), and the
/// keys pressed since the last frame.
#[derive(Default)]
struct Nav {
    focus: Option<usize>,
    /// Controls this frame so far, and in the whole last frame.
    count: usize,
    last_count: usize,
    moves: i32,
    adjust: i32,
    activate: bool,
    tab: i32,
    /// This frame's (taken from the above in `begin`).
    now_adjust: i32,
    now_activate: bool,
    now_tab: i32,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            verts: Vec::with_capacity(32_768),
            font: Font::smooth(),
            pixel_font: Font::new(),
            w: 1.0,
            h: 1.0,
            s: 2.0,
            mouse: Vec2::ZERO,
            mouse_down: false,
            pressed: false,
            input_enabled: true,
            dt: 0.016,
            time: 0.0,
            clicked: false,
            typed: String::new(),
            backspace: 0,
            right_pressed: false,
            shift: false,
            scroll: 0.0,
            clips: vec![(0, None)],
            clip: None,
            focus: None,
            anims: HashMap::new(),
            active: None,
            tooltip: None,
            fade: 1.0,
            offset: Vec2::ZERO,
            age: 0.0,
            screen_key: 0,
            stagger: 0,
            nav: Nav::default(),
        }
    }

    /// A key pressed on a menu: W/S or the up and down arrows (and Tab) move through its
    /// controls, A/D or left and right turn a slider, Space or Enter uses the focused one, Q/E
    /// switch between its tabs. Returns whether it was taken (while typing in a text field,
    /// only the arrows, Tab and Enter are).
    pub fn nav_key(&mut self, code: winit::keyboard::KeyCode, shift: bool) -> bool {
        use winit::keyboard::KeyCode as K;
        let typing = self.focus.is_some();
        let n = &mut self.nav;
        match code {
            K::ArrowUp => n.moves -= 1,
            K::ArrowDown => n.moves += 1,
            K::Tab => n.moves += if shift { -1 } else { 1 },
            K::ArrowLeft => n.adjust -= 1,
            K::ArrowRight => n.adjust += 1,
            K::Enter | K::NumpadEnter => {
                if typing && n.focus.is_none() {
                    return false;
                }
                n.activate = true;
            }
            _ if typing => return false,
            K::KeyW => n.moves -= 1,
            K::KeyS => n.moves += 1,
            K::KeyA => n.adjust -= 1,
            K::KeyD => n.adjust += 1,
            K::Space => n.activate = true,
            K::KeyQ => n.tab -= 1,
            K::KeyE => n.tab += 1,
            _ => return false,
        }
        true
    }

    /// Q/E this frame: the tab to go to, as a step (-1 back, 1 on).
    pub fn nav_tab(&self) -> i32 {
        self.nav.now_tab
    }

    /// The mouse moved to `p`: it takes over from the keyboard.
    pub fn set_mouse(&mut self, p: Vec2) {
        if p.distance(self.mouse) > 2.0 {
            self.nav.focus = None;
        }
        self.mouse = p;
    }

    /// The next control that can be focused with the keyboard: whether it is.
    pub fn nav_item(&mut self) -> bool {
        let i = self.nav.count;
        self.nav.count += 1;
        self.nav.focus == Some(i)
    }

    /// Whether the focused control is used (Space or Enter) this frame; call after `nav_item`
    /// said it is the focused one.
    pub fn nav_activated(&self) -> bool {
        self.nav.now_activate
    }

    /// A/D this frame on the focused control (-1, 0 or 1 and more).
    pub fn nav_adjust(&self) -> i32 {
        self.nav.now_adjust
    }

    /// Starts the screen's entrance animations over when `key` (which screen is open)
    /// changes.
    pub fn screen(&mut self, key: u64) {
        if key != self.screen_key {
            self.screen_key = key;
            self.age = 0.0;
            self.nav.focus = None;
        }
    }

    /// The next element's entrance (0..1, eased): each comes in a little after the one before.
    pub fn appear(&mut self) -> f32 {
        let i = self.stagger.min(14);
        self.stagger += 1;
        ease_out((self.age - 0.04 - i as f32 * 0.035) / 0.3)
    }

    /// Draws what follows `fade` times as see-through and moved by `offset`, until
    /// `restore` with what this returns.
    pub fn style(&mut self, fade: f32, offset: Vec2) -> (f32, Vec2) {
        let old = (self.fade, self.offset);
        self.fade *= fade;
        self.offset += offset;
        old
    }

    pub fn restore(&mut self, old: (f32, Vec2)) {
        (self.fade, self.offset) = old;
    }

    pub fn begin(&mut self, w: f32, h: f32, s: f32, dt: f32, time: f32) {
        self.verts.clear();
        self.clips.clear();
        self.clips.push((0, None));
        self.clip = None;
        self.w = w;
        self.h = h;
        self.s = s;
        self.dt = dt;
        self.time = time;
        self.age += dt;
        self.stagger = 0;
        // Controls at rest are forgotten (their ids follow where they are: the map would
        // grow with every resize and scroll); one hovered again starts from rest anyway.
        self.anims.retain(|_, v| *v > 0.002);
        // The keys pressed since the last frame move the focus over the last frame's controls.
        let nav = &mut self.nav;
        nav.last_count = nav.count;
        nav.count = 0;
        if nav.moves != 0 && nav.last_count > 0 {
            let n = nav.last_count as i32;
            // (from nothing focused, the first press lands on the first or the last control)
            nav.focus = Some(match nav.focus {
                None if nav.moves > 0 => (nav.moves - 1).rem_euclid(n),
                None => (n + nav.moves).rem_euclid(n),
                Some(f) => (f as i32 + nav.moves).rem_euclid(n),
            } as usize);
            // (a text field being typed in is left)
            self.focus = None;
        }
        let nav = &mut self.nav;
        nav.moves = 0;
        nav.now_adjust = std::mem::take(&mut nav.adjust);
        nav.now_activate = std::mem::take(&mut nav.activate) && nav.focus.is_some();
        nav.now_tab = std::mem::take(&mut nav.tab);
        self.fade = 1.0;
        self.offset = Vec2::ZERO;
        self.tooltip = None;
        self.clicked = false;
        if !self.mouse_down {
            self.active = None;
        }
    }

    pub fn finish(&mut self) {
        self.set_clip(None);
        if let Some(text) = self.tooltip.take() {
            let s = self.s;
            let tw = self.text_width(&text, s);
            let (bw, bh) = (tw + 8.0 * s, 14.0 * s);
            let x = (self.mouse.x + 10.0 * s).min(self.w - bw - 2.0 * s).round();
            let y = (self.mouse.y - bh - 4.0 * s).max(2.0 * s).round();
            self.rect_full(
                x,
                y + 2.0 * s,
                bw,
                bh,
                rgba(0, 0, 0, 110),
                rgba(0, 0, 0, 110),
                4.0 * s,
                6.0 * s,
            );
            self.rect(x, y, bw, bh, rgba(255, 255, 255, 30), 4.0 * s);
            self.rect(x + 1.0, y + 1.0, bw - 2.0, bh - 2.0, rgba(22, 22, 28, 248), 4.0 * s - 1.0);
            self.text(&text, x + 4.0 * s, y + 3.5 * s, s, WHITE, true);
        }
    }

    pub fn hit(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let inside = |x: f32, y: f32, w: f32, h: f32| {
            self.mouse.x >= x && self.mouse.x < x + w && self.mouse.y >= y && self.mouse.y < y + h
        };
        self.input_enabled
            && inside(x, y, w, h)
            && self.clip.is_none_or(|c| inside(c[0], c[1], c[2], c[3]))
    }

    /// Restricts drawing (and mouse hits) to a rectangle until `set_clip(None)`.
    pub fn set_clip(&mut self, rect: Option<[f32; 4]>) {
        self.clip = rect;
        let start = self.verts.len() as u32;
        match self.clips.last_mut() {
            Some(last) if last.0 == start => last.1 = rect,
            _ => self.clips.push((start, rect)),
        }
    }

    pub fn set_tooltip(&mut self, text: &str) {
        self.tooltip = Some(text.to_string());
    }
}

#[cfg(test)]
mod nav_tests {
    use super::*;
    use winit::keyboard::KeyCode as K;

    /// One frame of a screen with two buttons and a slider: which buttons were used.
    fn frame(ui: &mut Ui, value: &mut f32) -> [bool; 2] {
        ui.begin(800.0, 600.0, 2.0, 0.016, 1.0);
        ui.age = 10.0;
        let a = ui.button("A", 0.0, 0.0, 40.0, 20.0, true);
        let b = ui.button("B", 0.0, 30.0, 40.0, 20.0, true);
        ui.slider("s", "S", value, 0.0, 100.0, 0.0, 60.0, 40.0, 20.0);
        ui.finish();
        [a, b]
    }

    #[test]
    fn the_keyboard_moves_through_a_screen_and_uses_its_controls() {
        let mut ui = Ui::new();
        ui.mouse = Vec2::new(500.0, 500.0);
        let mut v = 50.0;
        assert_eq!(frame(&mut ui, &mut v), [false, false]);
        // S twice: the second button; Space uses it.
        for k in [K::KeyS, K::KeyS, K::Space] {
            assert!(ui.nav_key(k, false));
        }
        assert_eq!(frame(&mut ui, &mut v), [false, true]);
        // W and Enter: the first.
        ui.nav_key(K::KeyW, false);
        ui.nav_key(K::Enter, false);
        assert_eq!(frame(&mut ui, &mut v), [true, false]);
        // Down twice more, past the second, to the slider: D turns it up.
        ui.nav_key(K::ArrowDown, false);
        ui.nav_key(K::ArrowDown, false);
        ui.nav_key(K::KeyD, false);
        frame(&mut ui, &mut v);
        assert!(v > 50.0, "{v}");
        // And on past the last back to the first.
        ui.nav_key(K::KeyS, false);
        ui.nav_key(K::Space, false);
        assert_eq!(frame(&mut ui, &mut v), [true, false]);
        // Q and E are the tabs.
        ui.nav_key(K::KeyE, false);
        frame(&mut ui, &mut v);
        assert_eq!(ui.nav_tab(), 1);
    }
}
