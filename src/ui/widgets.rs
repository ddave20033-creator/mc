//! The controls: buttons, sliders, text fields, panels, the logo and chips, with their hover
//! and press animations.

use super::*;

impl Ui {
    fn anim(&mut self, id: u64, on: bool) -> f32 {
        let k = crate::util::damp(self.dt, 16.0);
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

    /// The game's logo (`textures/logo.png`: `tex::LOGO_TILES` square texture layers from
    /// `first_layer`), `width` wide, its top middle at (`cx`, `y`), its edges smooth.
    pub fn logo(&mut self, first_layer: u32, cx: f32, y: f32, width: f32) {
        let tiles = crate::textures::tex::LOGO_TILES;
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
