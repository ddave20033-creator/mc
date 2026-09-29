//! Immediate-mode UI: rounded SDF rectangles, a pixel bitmap font, buttons and sliders; the
//! menus built with it (`screens`) and the chat (`chat`).

pub mod chat;
pub mod screens;

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

#[derive(Clone, Copy, Default)]
pub struct Glyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    /// Visible width in font pixels.
    pub w: f32,
    /// Advance in font pixels (width + 1 spacing).
    pub adv: f32,
    pub bits: [u8; 8],
    pub minx: u32,
    /// Accent row drawn one pixel above the cell (capital letters with accents).
    pub top: u8,
    /// Its quad in font units from the pen and the top of the line (x, y, width, height).
    pub quad: [f32; 4],
}

/// ASCII + Latin-1 + Hungarian o/u with double acute.
fn font_chars() -> Vec<(char, [u8; 8])> {
    let mut v = Vec::new();
    for c in (32u32..128).chain(160..256) {
        let ch = char::from_u32(c).unwrap();
        v.push((
            ch,
            BASIC_FONTS
                .get(ch)
                .or_else(|| LATIN_FONTS.get(ch))
                .unwrap_or([0; 8]),
        ));
    }
    v.push(('\u{151}', [0x6C, 0x36, 0x00, 0x1E, 0x33, 0x33, 0x1E, 0x00]));
    v.push(('\u{171}', [0x6C, 0x36, 0x00, 0x33, 0x33, 0x33, 0x7E, 0x00]));
    // font8x8 squashes accented capitals to lowercase height; use the plain capital
    // and put the accent in an extra row above the cell instead (see accent_row).
    for (ch, base) in [
        ('\u{c1}', 'A'),
        ('\u{c9}', 'E'),
        ('\u{cd}', 'I'),
        ('\u{d3}', 'O'),
        ('\u{d6}', 'O'),
        ('\u{da}', 'U'),
        ('\u{dc}', 'U'),
        ('\u{150}', 'O'),
        ('\u{170}', 'U'),
    ] {
        let g = BASIC_FONTS.get(base).unwrap();
        v.retain(|e| e.0 != ch);
        v.push((ch, g));
    }
    v
}

/// Accent pixels for the extra row above accented capitals (bit 0 = leftmost pixel).
fn accent_row(ch: char) -> u8 {
    match ch {
        '\u{c1}' | '\u{c9}' | '\u{cd}' | '\u{d3}' | '\u{da}' => 0x18,
        '\u{d6}' | '\u{dc}' => 0x12,
        '\u{150}' | '\u{170}' => 0x36,
        _ => 0,
    }
}

/// 8x8 bitmap font baked into a 16-column atlas with 1px padding; glyphs are cropped to be proportional.
pub struct Font {
    pub glyphs: Vec<Glyph>,
    pub atlas: Vec<u8>,
    index: HashMap<char, usize>,
}

impl Font {
    pub fn new() -> Self {
        let mut atlas = vec![0u8; (FONT_TEX_W * FONT_TEX_H) as usize];
        let chars = font_chars();
        let mut glyphs = Vec::with_capacity(chars.len());
        let mut index = HashMap::new();
        for (i, &(ch, bits)) in chars.iter().enumerate() {
            let i = i as u32;
            index.insert(ch, i as usize);
            let (ox, oy) = ((i % 16) * 10 + 1, (i / 16) * 10 + 1);
            let (mut minx, mut maxx) = (8u32, 0u32);
            let top = accent_row(ch);
            for x in 0..8u32 {
                if top & (1 << x) != 0 {
                    atlas[((oy - 1) * FONT_TEX_W + ox + x) as usize] = 255;
                }
            }
            for (y, row) in bits.iter().enumerate() {
                for x in 0..8u32 {
                    if row & (1 << x) != 0 {
                        atlas[((oy + y as u32) * FONT_TEX_W + ox + x) as usize] = 255;
                        minx = minx.min(x);
                        maxx = maxx.max(x);
                    }
                }
            }
            if minx > maxx {
                glyphs.push(Glyph {
                    w: 0.0,
                    adv: 4.0,
                    bits,
                    ..Default::default()
                });
            } else {
                let w = (maxx - minx + 1) as f32;
                let lift = (top != 0) as u32 as f32;
                glyphs.push(Glyph {
                    u0: (ox + minx) as f32 / FONT_TEX_W as f32,
                    u1: (ox + maxx + 1) as f32 / FONT_TEX_W as f32,
                    v0: (oy - (top != 0) as u32) as f32 / FONT_TEX_H as f32,
                    v1: (oy + 8) as f32 / FONT_TEX_H as f32,
                    w,
                    adv: w + 1.0,
                    bits,
                    minx,
                    top,
                    quad: [0.0, -lift, w, 8.0 + lift],
                });
            }
        }
        Self {
            glyphs,
            atlas,
            index,
        }
    }

    /// The smooth typeface (`SMOOTH_FONT`), anti-aliased: capitals about as tall as the pixel
    /// font's, its baseline where the pixel font's is. The pixel font where it cannot be read.
    pub fn smooth() -> Self {
        use ab_glyph::{point, Font as _, FontVec, PxScale, ScaleFont as _};
        let Some(font) = std::fs::read(SMOOTH_FONT).ok().and_then(|d| FontVec::try_from_vec(d).ok()) else {
            return Self::new();
        };
        // Its size: a capital H 6.4 units tall.
        let probe = PxScale::from(100.0);
        let cap = font
            .outline_glyph(font.glyph_id('H').with_scale_and_position(probe, point(0.0, 0.0)))
            .map_or(70.0, |g| g.px_bounds().height());
        let scale = PxScale::from(100.0 * 6.4 * FONT_RES / cap);
        let scaled = font.as_scaled(scale);
        let mut atlas = vec![0u8; (FONT_TEX_W * FONT_TEX_H) as usize];
        let mut glyphs = Vec::new();
        let mut index = HashMap::new();
        let chars = (32u32..127)
            .chain(160..256)
            .filter_map(char::from_u32)
            .chain(['\u{150}', '\u{151}', '\u{170}', '\u{171}', '\u{2013}', '\u{2014}', '\u{2022}', '\u{2026}', '\u{2190}', '\u{2192}']);
        const PAD: u32 = 2;
        let (mut cx, mut cy, mut row_h) = (1u32, 1u32, 0u32);
        for ch in chars {
            let id = font.glyph_id(ch);
            let adv = scaled.h_advance(id) / FONT_RES;
            let mut g = Glyph { adv, bits: [0; 8], ..Default::default() };
            // The baseline 7 units down the line.
            let placed = id.with_scale_and_position(scale, point(0.0, 7.0 * FONT_RES));
            if let Some(outline) = font.outline_glyph(placed) {
                let b = outline.px_bounds();
                let (bw, bh) = (b.width() as u32 + 2 * PAD, b.height() as u32 + 2 * PAD);
                if cx + bw >= FONT_TEX_W {
                    cx = 1;
                    cy += row_h + 1;
                    row_h = 0;
                }
                if cy + bh >= FONT_TEX_H {
                    break;
                }
                outline.draw(|x, y, c| {
                    let (ax, ay) = (cx + PAD + x, cy + PAD + y);
                    if ax < FONT_TEX_W && ay < FONT_TEX_H {
                        let i = (ay * FONT_TEX_W + ax) as usize;
                        atlas[i] = atlas[i].max((c.clamp(0.0, 1.0) * 255.0) as u8);
                    }
                });
                g.u0 = cx as f32 / FONT_TEX_W as f32;
                g.v0 = cy as f32 / FONT_TEX_H as f32;
                g.u1 = (cx + bw) as f32 / FONT_TEX_W as f32;
                g.v1 = (cy + bh) as f32 / FONT_TEX_H as f32;
                g.quad = [
                    (b.min.x - PAD as f32) / FONT_RES,
                    (b.min.y - PAD as f32) / FONT_RES,
                    bw as f32 / FONT_RES,
                    bh as f32 / FONT_RES,
                ];
                g.w = b.width() / FONT_RES;
                cx += bw + 1;
                row_h = row_h.max(bh);
            }
            index.insert(ch, glyphs.len());
            glyphs.push(g);
        }
        Self { glyphs, atlas, index }
    }

    pub fn glyph(&self, c: char) -> &Glyph {
        &self.glyphs[*self.index.get(&c).unwrap_or(&self.index[&'?'])]
    }
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

    fn push(&mut self, p: [Vec2; 4], uv: [[f32; 2]; 4], c: [Color; 4], rect: [f32; 4], mode: f32) {
        if self.fade <= 0.001 {
            return;
        }
        for i in [0, 1, 2, 0, 2, 3] {
            self.verts.push(UiVertex {
                pos: (p[i] + self.offset).to_array(),
                uv: uv[i],
                color: with_alpha(c[i], self.fade),
                rect,
                mode,
            });
        }
    }

    /// Rounded rectangle with a vertical gradient and optional soft (blurred) edge.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_full(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        top: Color,
        bottom: Color,
        radius: f32,
        soft: f32,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let m = soft + 1.0;
        let (hw, hh) = (w * 0.5, h * 0.5);
        let c = Vec2::new(x + hw, y + hh);
        let r = radius.min(hw).min(hh).max(0.0);
        let p = [
            Vec2::new(x - m, y - m),
            Vec2::new(x + w + m, y - m),
            Vec2::new(x + w + m, y + h + m),
            Vec2::new(x - m, y + h + m),
        ];
        let uv = p.map(|q| (q - c).to_array());
        self.push(
            p,
            uv,
            [top, top, bottom, bottom],
            [hw, hh, r, soft],
            MODE_RECT,
        );
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color, radius: f32) {
        self.rect_full(x, y, w, h, c, c, radius, 0.0);
    }

    /// Hard-edged gradient quad (no anti-aliasing) - for backgrounds and pixel art.
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let p = [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x + w, y + h),
            Vec2::new(x, y + h),
        ];
        self.push(
            p,
            [[0.0; 2]; 4],
            [top, top, bottom, bottom],
            [1e6, 1e6, 0.0, 0.0],
            MODE_RECT,
        );
    }

    /// Hard-edged gradient quad from left to right.
    pub fn hgradient(&mut self, x: f32, y: f32, w: f32, h: f32, left: Color, right: Color) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let p = [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x + w, y + h),
            Vec2::new(x, y + h),
        ];
        self.push(p, [[0.0; 2]; 4], [left, right, right, left], [1e6, 1e6, 0.0, 0.0], MODE_RECT);
    }

    /// Hard-edged solid quad of any shape (a triangle when two corners are the same).
    pub fn quad(&mut self, p: [Vec2; 4], c: Color) {
        self.push(p, [[0.0; 2]; 4], [c; 4], [1e6, 1e6, 0.0, 0.0], MODE_RECT);
    }

    pub fn solid(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        self.gradient(x, y, w, h, c, c);
    }

    /// Thin circular outline with a transparent center.
    pub fn ring(&mut self, center: Vec2, radius: f32, thickness: f32, color: Color) {
        let extent = radius + thickness * 0.5 + 1.5;
        let p = [
            center + Vec2::new(-extent, -extent),
            center + Vec2::new(extent, -extent),
            center + Vec2::new(extent, extent),
            center + Vec2::new(-extent, extent),
        ];
        let uv = p.map(|point| (point - center).to_array());
        self.push(p, uv, [color; 4], [radius, thickness, 0.0, 0.0], MODE_RING);
    }

    pub fn vignette(&mut self, c: Color) {
        let (w, h) = (self.w, self.h);
        let p = [
            Vec2::new(0.0, 0.0),
            Vec2::new(w, 0.0),
            Vec2::new(w, h),
            Vec2::new(0.0, h),
        ];
        let uv = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        self.push(p, uv, [c; 4], [0.0; 4], MODE_VIGNETTE);
    }

    fn block_face(&mut self, p: [Vec2; 4], layer: u32, shade: f32, tint: [u8; 3]) {
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let c = [
            tint[0] as f32 / 255.0 * shade,
            tint[1] as f32 / 255.0 * shade,
            tint[2] as f32 / 255.0 * shade,
            1.0,
        ];
        self.push(p, uv, [c; 4], [layer as f32, shade, 0.0, 0.0], MODE_BLOCK);
    }

    /// Flat textured sprite (plants).
    pub fn block_sprite(&mut self, c: Vec2, r: f32, layer: u32, tint: [u8; 3]) {
        let p = [
            Vec2::new(c.x - r, c.y - r),
            Vec2::new(c.x + r, c.y - r),
            Vec2::new(c.x + r, c.y + r),
            Vec2::new(c.x - r, c.y + r),
        ];
        self.block_face(p, layer, 1.0, tint);
    }

    /// Isometric icon with different textures on the left (front) and right faces.
    #[allow(clippy::too_many_arguments)]
    pub fn block_icon_faces(
        &mut self,
        c: Vec2,
        r: f32,
        top: u32,
        left: u32,
        right: u32,
        top_tint: [u8; 3],
        side_tint: [u8; 3],
    ) {
        let k = 0.866 * r;
        let top_face = [
            Vec2::new(c.x, c.y - r),
            Vec2::new(c.x + k, c.y - r * 0.5),
            Vec2::new(c.x, c.y),
            Vec2::new(c.x - k, c.y - r * 0.5),
        ];
        let lf = [
            Vec2::new(c.x - k, c.y - r * 0.5),
            Vec2::new(c.x, c.y),
            Vec2::new(c.x, c.y + r),
            Vec2::new(c.x - k, c.y + r * 0.5),
        ];
        let rf = [
            Vec2::new(c.x, c.y),
            Vec2::new(c.x + k, c.y - r * 0.5),
            Vec2::new(c.x + k, c.y + r * 0.5),
            Vec2::new(c.x, c.y + r),
        ];
        self.block_face(top_face, top, 1.0, top_tint);
        self.block_face(lf, left, 0.8, side_tint);
        self.block_face(rf, right, 0.62, side_tint);
    }

    /// Textured quad (corners TL, TR, BR, BL of the texture) with a brightness factor.
    pub fn tex_quad(&mut self, p: [Vec2; 4], layer: u32, shade: f32) {
        self.block_face(p, layer, shade, [255; 3]);
    }

    /// `tex_quad` showing the part `uv` (per corner) of the texture.
    pub fn tex_quad_uv(&mut self, p: [Vec2; 4], uv: [[f32; 2]; 4], layer: u32, shade: f32) {
        let c = [shade, shade, shade, 1.0];
        self.push(p, uv, [c; 4], [layer as f32, shade, 0.0, 0.0], MODE_BLOCK);
    }

    /// `tex_quad` with a color tint.
    pub fn tex_quad_tint(&mut self, p: [Vec2; 4], layer: u32, shade: f32, tint: [u8; 3]) {
        self.block_face(p, layer, shade, tint);
    }

    // ---------- text ----------

    pub fn text_width(&self, s: &str, size: f32) -> f32 {
        let units: f32 = s.chars().map(|c| self.font.glyph(c).adv).sum();
        if units > 0.0 {
            (units - 1.0) * size
        } else {
            0.0
        }
    }

    fn draw_text(
        &mut self,
        s: &str,
        x: f32,
        y: f32,
        size: f32,
        c: Color,
        xf: Option<(Vec2, Vec2)>,
    ) {
        let mut pen = x;
        for ch in s.chars() {
            let g = *self.font.glyph(ch);
            if g.w > 0.0 {
                let [qx, qy, qw, qh] = g.quad;
                let (x0, y0) = (pen + qx * size, y + qy * size);
                let (x1, y1) = (x0 + qw * size, y0 + qh * size);
                let mut p = [
                    Vec2::new(x0, y0),
                    Vec2::new(x1, y0),
                    Vec2::new(x1, y1),
                    Vec2::new(x0, y1),
                ];
                if let Some((center, rot)) = xf {
                    for q in &mut p {
                        *q = center + rot.rotate(*q - center);
                    }
                }
                let uv = [[g.u0, g.v0], [g.u1, g.v0], [g.u1, g.v1], [g.u0, g.v1]];
                self.push(p, uv, [c; 4], [0.0; 4], MODE_TEXT);
            }
            pen += g.adv * size;
        }
    }

    /// A soft shadow under text (not the hard, full-strength block shadow).
    fn shadow_color(c: Color) -> Color {
        [c[0] * 0.08, c[1] * 0.08, c[2] * 0.1, c[3] * 0.5]
    }

    /// Draws text with its top-left at (x, y). Returns the width.
    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, c: Color, shadow: bool) -> f32 {
        let (x, y) = (x.round(), y.round());
        if shadow {
            let d = (size * 0.5).max(1.0);
            self.draw_text(s, x + d, y + d, size, Self::shadow_color(c), None);
        }
        self.draw_text(s, x, y, size, c, None);
        self.text_width(s, size)
    }

    /// Splits `text` into lines no wider than `max_w` pixels at `size` (at spaces).
    pub fn wrap(&self, text: &str, max_w: f32, size: f32) -> Vec<String> {
        let mut rows = Vec::new();
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let candidate = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if !cur.is_empty() && self.text_width(&candidate, size) > max_w {
                rows.push(std::mem::replace(&mut cur, word.to_string()));
            } else {
                cur = candidate;
            }
        }
        if !cur.is_empty() {
            rows.push(cur);
        }
        rows
    }

    pub fn text_centered(&mut self, s: &str, cx: f32, y: f32, size: f32, c: Color, shadow: bool) {
        let w = self.text_width(s, size);
        self.text(s, cx - w * 0.5, y, size, c, shadow);
    }

    // ---------- widgets ----------

    fn anim(&mut self, id: u64, on: bool) -> f32 {
        let k = 1.0 - (-self.dt * 16.0).exp();
        let v = self.anims.entry(id).or_insert(0.0);
        *v += ((on as i32 as f32) - *v) * k;
        *v
    }

    /// A control's box: flat dark grey with a hairline border, a shade lighter when hovered
    /// (`t`); the main one filled with the accent, a destructive one tinted red only when
    /// hovered. `press` sinks it a little.
    #[allow(clippy::too_many_arguments)]
    fn frame_box(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, enabled: bool, press: f32, kind: ButtonKind) {
        let s = self.s;
        let r = 4.0 * s;
        let inset = press * s;
        let (x, y, w, h) = (x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset);
        let (border, fill) = if !enabled {
            (rgba(255, 255, 255, 10), rgba(22, 22, 28, 150))
        } else {
            match kind {
                ButtonKind::Normal => (
                    lerp_color(rgba(255, 255, 255, 18), rgba(255, 255, 255, 46), t),
                    lerp_color(GLASS_TOP, rgba(38, 38, 48, 230), t),
                ),
                ButtonKind::Primary => (
                    lerp_color(ACCENT, ACCENT_LIGHT, t * 0.5),
                    lerp_color(ACCENT, rgba(140, 130, 250, 255), t),
                ),
                ButtonKind::Danger => (
                    lerp_color(rgba(255, 255, 255, 18), with_alpha(DANGER, 0.7), t),
                    lerp_color(GLASS_TOP, rgba(74, 28, 38, 235), t),
                ),
            }
        };
        self.rect(x, y, w, h, border, r);
        self.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, fill, r - 1.0);
    }

    pub fn button(&mut self, label: &str, x: f32, y: f32, w: f32, h: f32, enabled: bool) -> bool {
        self.button_ex(label, x, y, w, h, enabled, ButtonKind::Normal)
    }

    /// The screen's main button, filled with the accent.
    pub fn button_primary(&mut self, label: &str, x: f32, y: f32, w: f32, h: f32, enabled: bool) -> bool {
        self.button_ex(label, x, y, w, h, enabled, ButtonKind::Primary)
    }

    /// A button that comes in with the screen (after the ones before it), lights up when
    /// hovered (a bar of the accent grows at its left, the label moves over to it) and sinks
    /// when pressed.
    #[allow(clippy::too_many_arguments)]
    pub fn button_ex(&mut self, label: &str, x: f32, y: f32, w: f32, h: f32, enabled: bool, kind: ButtonKind) -> bool {
        let s = self.s;
        let a = self.appear();
        let old = self.style(a, Vec2::new(0.0, ((1.0 - a) * 5.0 * s).round()));
        let id = hash_id(label, x, y);
        let focused = enabled && self.nav_item();
        let hovered = self.hit(x, y, w, h) && self.active.is_none();
        let t = self.anim(id, (hovered || focused) && enabled);
        let down = hovered && enabled && self.mouse_down;
        let press = self.anim(id ^ 0x5eed, down);
        self.frame_box(x, y, w, h, t, enabled, press, kind);

        if enabled && kind == ButtonKind::Normal && t > 0.01 {
            let bh = ((h - 10.0 * s) * t).max(0.0);
            self.rect(x + 3.0 * s, y + (h - bh) * 0.5, 2.0 * s, bh, ACCENT, s);
        }
        let tc = match (enabled, kind) {
            (false, _) => rgba(112, 112, 122, 255),
            (true, ButtonKind::Normal) => lerp_color(rgba(214, 214, 222, 255), HOVER_TEXT, t),
            (true, ButtonKind::Danger) => lerp_color(rgba(236, 150, 160, 255), HOVER_TEXT, t),
            (true, ButtonKind::Primary) => WHITE,
        };
        let tw = self.text_width(label, s);
        let shift = if kind == ButtonKind::Normal { (t * 2.0 * s).round() } else { 0.0 };
        self.text(label, x + (w - tw) * 0.5 + shift, y + (h - 7.0 * s) * 0.5, s, tc, true);
        let clicked = (hovered && enabled && self.pressed) || (focused && self.nav_activated());
        self.clicked |= clicked;
        self.restore(old);
        clicked
    }

    /// A slider: a glass box with its label, filled with the accent up to its value, a round
    /// knob that glows while it is held or hovered.
    #[allow(clippy::too_many_arguments)]
    pub fn slider(
        &mut self,
        id: &str,
        label: &str,
        value: &mut f32,
        min: f32,
        max: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> bool {
        let s = self.s;
        let a = self.appear();
        let old = self.style(a, Vec2::new(0.0, ((1.0 - a) * 5.0 * s).round()));
        let hid = hash_id(id, 0.0, 0.0);
        let focused = self.nav_item();
        let hovered = self.hit(x, y, w, h);
        if hovered && self.pressed && self.active.is_none() {
            self.active = Some(hid);
        }
        let active = self.active == Some(hid);
        let t = self.anim(hid, (hovered && self.active.is_none()) || active || focused);
        let knob = 10.0 * s;
        let mut changed = false;
        if focused && self.nav_adjust() != 0 {
            // A/D: a twentieth of the way (at least a whole step where it counts whole ones).
            let step = ((max - min) / 20.0).max(if max - min >= 20.0 { 1.0 } else { 0.0 });
            *value = (*value + step * self.nav_adjust() as f32).clamp(min, max);
            changed = true;
        }
        if active {
            let f = ((self.mouse.x - x - knob * 0.5) / (w - knob)).clamp(0.0, 1.0);
            let nv = min + f * (max - min);
            if (nv - *value).abs() > f32::EPSILON {
                *value = nv;
                changed = true;
            }
        }
        let f = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        self.frame_box(x, y, w, h, t * 0.6, true, 0.0, ButtonKind::Normal);
        let r = 4.0 * s;
        let kx = (x + f * (w - knob)).round();
        // The filled part, brighter toward the knob.
        let fill_w = (kx + knob * 0.5 - x).max(0.0);
        self.rect(x + 1.0, y + 1.0, fill_w, h - 2.0, with_alpha(ACCENT, 0.38 + 0.12 * t), r - 1.0);
        // The knob: a pill, glowing when held.
        let (ky, kh) = (y + 3.0 * s, h - 6.0 * s);
        let held = if active { 1.0 } else { t };
        let _ = held;
        self.rect(kx + s, ky, knob - 2.0 * s, kh, lerp_color(rgba(214, 214, 222, 255), WHITE, t), 3.0 * s);
        let tc = lerp_color(rgba(232, 234, 240, 255), HOVER_TEXT, t);
        let tw = self.text_width(label, s);
        self.text(label, x + (w - tw) * 0.5, y + (h - 7.0 * s) * 0.5, s, tc, true);
        self.restore(old);
        changed
    }

    /// Single-line text input. Click to focus; typing goes to the focused field.
    #[allow(clippy::too_many_arguments)]
    pub fn text_field(
        &mut self,
        id: &str,
        value: &mut String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        placeholder: &str,
        max: usize,
    ) -> bool {
        let s = self.s;
        let a = self.appear();
        let old = self.style(a, Vec2::new(0.0, ((1.0 - a) * 5.0 * s).round()));
        let hid = hash_id(id, 0.0, 0.0);
        let hovered = self.hit(x, y, w, h);
        if self.nav_item() {
            // Reached with the keyboard: typed into.
            self.focus = Some(hid);
        }
        if self.pressed {
            if hovered {
                self.focus = Some(hid);
            } else if self.focus == Some(hid) {
                self.focus = None;
            }
        }
        let focused = self.focus == Some(hid);
        let mut changed = false;
        if focused {
            for _ in 0..self.backspace {
                changed |= value.pop().is_some();
            }
            for c in self.typed.chars() {
                if !c.is_control() && value.chars().count() < max {
                    value.push(c);
                    changed = true;
                }
            }
        }
        let t = self.anim(hid, focused);
        let hover = self.anim(hid ^ 0x40, hovered);
        let r = 4.0 * s;
        let border = lerp_color(
            lerp_color(rgba(255, 255, 255, 22), rgba(255, 255, 255, 60), hover),
            ACCENT_LIGHT,
            t,
        );
        self.rect(x, y, w, h, border, r);
        self.rect_full(x + 1.0, y + 1.0, w - 2.0, h - 2.0, rgba(12, 12, 16, 235), rgba(14, 14, 18, 235), r - 1.0, 0.0);
        // An accent line along the bottom, growing out from the middle when focused.
        let lw = (w - 8.0 * s) * t;
        if lw > 0.5 {
            self.rect(x + (w - lw) * 0.5, y + h - 2.0 * s, lw, s, ACCENT, s * 0.5);
        }
        let ty = (y + (h - 7.0 * s) * 0.5).round();
        if value.is_empty() && !focused {
            self.text(placeholder, x + 5.0 * s, ty, s, rgba(112, 114, 126, 255), false);
        } else {
            let tw = self.text(value, x + 5.0 * s, ty, s, WHITE, true);
            if focused {
                // A caret that fades in and out.
                let blink = 0.5 + 0.5 * (self.time * 5.0).cos();
                self.rect(x + 5.0 * s + tw + s, ty - s, s, 9.0 * s, with_alpha(ACCENT_LIGHT, blink), 0.0);
            }
        }
        self.restore(old);
        changed
    }

    /// Focuses a text field by id (e.g. when a screen opens).
    pub fn focus(&mut self, id: &str) {
        self.focus = Some(hash_id(id, 0.0, 0.0));
    }

    /// A card of dark frosted glass: a soft shadow, a hairline border, a faint light along
    /// its top.
    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let s = self.s;
        let r = 7.0 * s;
        self.rect_full(x, y + 4.0 * s, w, h, rgba(0, 0, 0, 90), rgba(0, 0, 0, 110), r, 14.0 * s);
        self.rect(x, y, w, h, rgba(255, 255, 255, 22), r);
        self.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, rgba(17, 17, 22, 236), r - 1.0);
    }

    /// The game's logo (`ui/logo.png`: `tex::LOGO_TILES` square texture layers from
    /// `first_layer`), `width` wide, its top middle at (`cx`, `y`), its edges smooth.
    pub fn logo(&mut self, first_layer: u32, cx: f32, y: f32, width: f32) {
        let tiles = crate::world::textures::tex::LOGO_TILES;
        let t = (width / tiles as f32).round();
        let x0 = (cx - t * tiles as f32 * 0.5).round();
        for i in 0..tiles {
            let x = x0 + i as f32 * t;
            let p = [Vec2::new(x, y), Vec2::new(x + t, y), Vec2::new(x + t, y + t), Vec2::new(x, y + t)];
            let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            self.push(p, uv, [WHITE; 4], [(first_layer + i) as f32, 0.0, 0.0, 0.0], MODE_IMAGE);
        }
    }

    /// A small rounded label: `text` on a tinted pill (a world's mode, a setting's state).
    pub fn chip(&mut self, text: &str, x: f32, y: f32, color: Color) -> f32 {
        let s = self.s;
        let size = s;
        let tw = self.text_width(text, size);
        let (w, h) = ((tw + 8.0 * s).round(), (11.0 * s).round());
        self.rect(x, y, w, h, with_alpha(color, 0.3), h * 0.5);
        self.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, lerp_color(rgba(20, 20, 26, 235), color, 0.12), h * 0.5 - 1.0);
        let light = lerp_color(color, WHITE, 0.55);
        self.text(text, x + 4.0 * s, y + (h - 7.0 * size) * 0.5, size, light, false);
        w
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
