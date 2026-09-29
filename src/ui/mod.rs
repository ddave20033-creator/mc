//! Immediate-mode UI: rounded SDF rectangles, a pixel bitmap font, buttons and sliders; the
//! menus built with it (`screens`) and the chat (`chat`).

pub mod chat;
pub mod screens;

use font8x8::{UnicodeFonts, BASIC_FONTS, LATIN_FONTS};
use glam::Vec2;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub const FONT_TEX_W: u32 = 160;
pub const FONT_TEX_H: u32 = 130;

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
pub const VERSION: &str = "RustCraft 0.4.0 (Vulkan)";

pub const WHITE: Color = rgba(255, 255, 255, 255);
/// The menus' colours: rust orange (the logo's) on dark glass.
pub const ACCENT: Color = rgba(236, 128, 66, 255);
pub const ACCENT_LIGHT: Color = rgba(255, 178, 116, 255);
pub const DANGER: Color = rgba(222, 72, 64, 255);
pub const HOVER_TEXT: Color = rgba(255, 238, 224, 255);
pub const GLASS_TOP: Color = rgba(30, 32, 42, 205);
pub const GLASS_BOTTOM: Color = rgba(18, 19, 26, 215);

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
                });
            }
        }
        Self {
            glyphs,
            atlas,
            index,
        }
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
    /// A button's flash when it is clicked (1 .. 0).
    flashes: HashMap<u64, f32>,
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
}

impl Ui {
    pub fn new() -> Self {
        Self {
            verts: Vec::with_capacity(32_768),
            font: Font::new(),
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
            flashes: HashMap::new(),
            active: None,
            tooltip: None,
            fade: 1.0,
            offset: Vec2::ZERO,
            age: 0.0,
            screen_key: 0,
            stagger: 0,
        }
    }

    /// Starts the screen's entrance animations over when `key` (which screen is open)
    /// changes.
    pub fn screen(&mut self, key: u64) {
        if key != self.screen_key {
            self.screen_key = key;
            self.age = 0.0;
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
        self.fade = 1.0;
        self.offset = Vec2::ZERO;
        for f in self.flashes.values_mut() {
            *f = (*f - dt * 3.5).max(0.0);
        }
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
            self.rect(x, y, bw, bh, with_alpha(ACCENT, 0.7), 4.0 * s);
            self.rect_full(
                x + 1.0,
                y + 1.0,
                bw - 2.0,
                bh - 2.0,
                rgba(34, 32, 40, 248),
                rgba(20, 19, 26, 248),
                4.0 * s - 1.0,
                0.0,
            );
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
                let lift = if g.top != 0 { size } else { 0.0 };
                let (x0, y0, x1, y1) = (pen, y - lift, pen + g.w * size, y + 8.0 * size);
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
            self.draw_text(s, x + size, y + size, size, Self::shadow_color(c), None);
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

    pub fn text_rotated(&mut self, s: &str, center: Vec2, size: f32, angle: f32, c: Color) {
        let rot = Vec2::from_angle(angle);
        let w = self.text_width(s, size);
        let (x, y) = (center.x - w * 0.5, center.y - 3.5 * size);
        let sh = Vec2::splat(size);
        self.draw_text(
            s,
            x + size,
            y + size,
            size,
            Self::shadow_color(c),
            Some((center + sh, rot)),
        );
        self.draw_text(s, x, y, size, c, Some((center, rot)));
    }

    // ---------- widgets ----------

    fn anim(&mut self, id: u64, on: bool) -> f32 {
        let k = 1.0 - (-self.dt * 16.0).exp();
        let v = self.anims.entry(id).or_insert(0.0);
        *v += ((on as i32 as f32) - *v) * k;
        *v
    }

    /// A control's box: dark glass with a hairline border, lit up (`t`) with the accent and a
    /// glow when hovered; `press` sinks it a little.
    #[allow(clippy::too_many_arguments)]
    fn frame_box(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, enabled: bool, press: f32, kind: ButtonKind) {
        let s = self.s;
        let r = 4.0 * s;
        let inset = press * s;
        let (x, y, w, h) = (x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset);
        // Shadow, and the glow of the hovered one.
        self.rect_full(x, y + 2.0 * s, w, h, rgba(0, 0, 0, 70), rgba(0, 0, 0, 90), r, 6.0 * s);
        let tone = match kind {
            ButtonKind::Danger => DANGER,
            _ => ACCENT,
        };
        if enabled && t > 0.01 {
            self.rect_full(x, y, w, h, with_alpha(tone, 0.28 * t), with_alpha(tone, 0.18 * t), r, 9.0 * s);
        }
        let (border, top, bot) = if !enabled {
            (rgba(255, 255, 255, 14), rgba(28, 29, 36, 150), rgba(20, 21, 26, 150))
        } else {
            match kind {
                ButtonKind::Normal => (
                    lerp_color(rgba(255, 255, 255, 30), with_alpha(ACCENT_LIGHT, 0.85), t),
                    lerp_color(GLASS_TOP, rgba(50, 44, 46, 230), t),
                    lerp_color(GLASS_BOTTOM, rgba(34, 28, 30, 235), t),
                ),
                ButtonKind::Primary => (
                    lerp_color(rgba(255, 196, 150, 200), rgba(255, 224, 196, 255), t),
                    lerp_color(rgba(226, 118, 58, 240), rgba(246, 146, 82, 250), t),
                    lerp_color(rgba(176, 78, 34, 240), rgba(204, 98, 50, 250), t),
                ),
                ButtonKind::Danger => (
                    lerp_color(rgba(255, 150, 140, 120), rgba(255, 190, 180, 230), t),
                    lerp_color(rgba(120, 36, 36, 225), rgba(176, 52, 48, 240), t),
                    lerp_color(rgba(76, 22, 24, 225), rgba(120, 32, 32, 240), t),
                ),
            }
        };
        self.rect(x, y, w, h, border, r);
        self.rect_full(x + 1.0, y + 1.0, w - 2.0, h - 2.0, top, bot, r - 1.0, 0.0);
        // A thin highlight along the top.
        let sheen = if kind == ButtonKind::Normal { 16.0 + 16.0 * t } else { 50.0 + 30.0 * t };
        self.rect_full(
            x + 1.0,
            y + 1.0,
            w - 2.0,
            (h * 0.5).round(),
            rgba(255, 255, 255, sheen as u8),
            rgba(255, 255, 255, 0),
            r - 1.0,
            0.0,
        );
    }

    pub fn button(&mut self, label: &str, x: f32, y: f32, w: f32, h: f32, enabled: bool) -> bool {
        self.button_ex(label, x, y, w, h, enabled, ButtonKind::Normal)
    }

    /// The screen's main button, filled with the accent.
    pub fn button_primary(&mut self, label: &str, x: f32, y: f32, w: f32, h: f32, enabled: bool) -> bool {
        self.button_ex(label, x, y, w, h, enabled, ButtonKind::Primary)
    }

    /// A button that comes in with the screen (after the ones before it), lights up when
    /// hovered (a bar of the accent grows at its left, the label moves over to it), sinks
    /// when pressed and flashes when clicked.
    #[allow(clippy::too_many_arguments)]
    pub fn button_ex(&mut self, label: &str, x: f32, y: f32, w: f32, h: f32, enabled: bool, kind: ButtonKind) -> bool {
        let s = self.s;
        let a = self.appear();
        let old = self.style(a, Vec2::new(0.0, ((1.0 - a) * 8.0 * s).round()));
        let id = hash_id(label, x, y);
        let hovered = self.hit(x, y, w, h) && self.active.is_none();
        let t = self.anim(id, hovered && enabled);
        let down = hovered && enabled && self.mouse_down;
        let press = self.anim(id ^ 0x5eed, down);
        self.frame_box(x, y, w, h, t, enabled, press, kind);

        if enabled && kind == ButtonKind::Normal && t > 0.01 {
            let bh = ((h - 10.0 * s) * t).max(0.0);
            self.rect(x + 3.0 * s, y + (h - bh) * 0.5, 2.0 * s, bh, ACCENT, s);
        }
        let flash = self.flashes.get(&id).copied().unwrap_or(0.0);
        if flash > 0.0 {
            self.rect(x, y, w, h, rgba(255, 255, 255, (flash * 90.0) as u8), 4.0 * s);
        }
        let tc = if !enabled {
            rgba(120, 122, 132, 255)
        } else if kind == ButtonKind::Normal {
            lerp_color(rgba(222, 224, 232, 255), HOVER_TEXT, t)
        } else {
            WHITE
        };
        let tw = self.text_width(label, s);
        let shift = if kind == ButtonKind::Normal { (t * 2.0 * s).round() } else { 0.0 };
        self.text(label, x + (w - tw) * 0.5 + shift, y + (h - 7.0 * s) * 0.5, s, tc, true);
        let clicked = hovered && enabled && self.pressed;
        if clicked {
            self.flashes.insert(id, 1.0);
        }
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
        let old = self.style(a, Vec2::new(0.0, ((1.0 - a) * 8.0 * s).round()));
        let hid = hash_id(id, 0.0, 0.0);
        let hovered = self.hit(x, y, w, h);
        if hovered && self.pressed && self.active.is_none() {
            self.active = Some(hid);
        }
        let active = self.active == Some(hid);
        let t = self.anim(hid, (hovered && self.active.is_none()) || active);
        let knob = 10.0 * s;
        let mut changed = false;
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
        self.rect_full(
            x + 1.0,
            y + 1.0,
            fill_w,
            h - 2.0,
            with_alpha(ACCENT, 0.42 + 0.14 * t),
            with_alpha(rgba(176, 78, 34, 255), 0.36 + 0.14 * t),
            r - 1.0,
            0.0,
        );
        // The knob: a pill, glowing when held.
        let (ky, kh) = (y + 3.0 * s, h - 6.0 * s);
        let held = if active { 1.0 } else { t };
        self.rect_full(kx + s, ky, knob - 2.0 * s, kh, with_alpha(ACCENT, 0.5 * held), with_alpha(ACCENT, 0.3 * held), 3.0 * s, 6.0 * s);
        self.rect_full(
            kx + s,
            ky,
            knob - 2.0 * s,
            kh,
            lerp_color(rgba(236, 238, 244, 255), WHITE, t),
            rgba(190, 192, 202, 255),
            3.0 * s,
            0.0,
        );
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
        let old = self.style(a, Vec2::new(0.0, ((1.0 - a) * 8.0 * s).round()));
        let hid = hash_id(id, 0.0, 0.0);
        let hovered = self.hit(x, y, w, h);
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
        if t > 0.01 {
            self.rect_full(x, y, w, h, with_alpha(ACCENT, 0.3 * t), with_alpha(ACCENT, 0.2 * t), r, 8.0 * s);
        }
        let border = lerp_color(
            lerp_color(rgba(255, 255, 255, 34), rgba(255, 255, 255, 80), hover),
            ACCENT_LIGHT,
            t,
        );
        self.rect(x, y, w, h, border, r);
        self.rect_full(x + 1.0, y + 1.0, w - 2.0, h - 2.0, rgba(10, 11, 16, 235), rgba(16, 17, 24, 235), r - 1.0, 0.0);
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
        self.rect_full(x, y + 6.0 * s, w, h, rgba(0, 0, 0, 120), rgba(0, 0, 0, 150), r, 16.0 * s);
        self.rect_full(x, y, w, h, rgba(255, 255, 255, 46), rgba(255, 255, 255, 18), r, 0.0);
        self.rect_full(
            x + 1.0,
            y + 1.0,
            w - 2.0,
            h - 2.0,
            rgba(26, 28, 38, 228),
            rgba(13, 14, 20, 238),
            r - 1.0,
            0.0,
        );
        self.rect_full(
            x + r,
            y + 1.0,
            w - 2.0 * r,
            s,
            rgba(255, 255, 255, 40),
            rgba(255, 255, 255, 40),
            0.0,
            0.0,
        );
    }

    /// A small rounded label: `text` on a tinted pill (a world's mode, a setting's state).
    pub fn chip(&mut self, text: &str, x: f32, y: f32, color: Color) -> f32 {
        let s = self.s;
        let size = s;
        let tw = self.text_width(text, size);
        let (w, h) = ((tw + 8.0 * s).round(), (11.0 * s).round());
        self.rect(x, y, w, h, with_alpha(color, 0.45), h * 0.5);
        self.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, lerp_color(rgba(14, 15, 20, 235), color, 0.18), h * 0.5 - 1.0);
        let light = lerp_color(color, WHITE, 0.55);
        self.text(text, x + 4.0 * s, y + (h - 7.0 * size) * 0.5, size, light, false);
        w
    }
}
