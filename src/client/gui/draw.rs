//! Drawing the item screens: the slots and the stacks in them, tooltips, the panels, and the
//! light and dark themes.

use crate::client::Game;
use crate::client::gui::{SLOT, SlotRef};
use crate::item::*;
use crate::lang::tf;
use crate::ui::{Color, Ui, WHITE, rgba, with_alpha};
use crate::world::{
    OAK_LOG, TintKind, face_texture, icon_tint, is_log, is_stairs, log_radius, tint_kind,
};
use glam::Vec2;

/// Colors of the item screens; light is classic Minecraft, dark is the optional dark mode.
pub(super) struct Theme {
    pub(super) border: Color,
    pub(super) bevel_hi: Color,
    bevel_lo: Color,
    pub(super) fill_top: Color,
    fill_bottom: Color,
    pub(super) slot: Color,
    pub(super) slot_shadow: Color,
    pub(super) slot_light: Color,
    slot_inner: Color,
    hover: Color,
    label: Color,
    label_shadow: bool,
    pub(super) idle: Color,
    progress: Color,
    pub(super) preview_top: Color,
    pub(super) preview_bottom: Color,
    pub(super) scroll_track: Color,
    pub(super) scroll_thumb: Color,
    /// Creative tabs that are not open.
    pub(super) tab_idle: Color,
}

pub(super) const LIGHT: Theme = Theme {
    border: rgba(16, 16, 20, 255),
    bevel_hi: rgba(255, 255, 255, 255),
    bevel_lo: rgba(86, 86, 92, 255),
    fill_top: rgba(198, 198, 204, 255),
    fill_bottom: rgba(198, 198, 204, 255),
    slot: rgba(55, 55, 60, 255),
    slot_shadow: rgba(35, 35, 38, 255),
    slot_light: rgba(120, 120, 128, 255),
    slot_inner: rgba(78, 78, 84, 255),
    hover: rgba(255, 255, 255, 70),
    label: rgba(64, 64, 70, 255),
    label_shadow: false,
    idle: rgba(139, 139, 145, 255),
    progress: rgba(255, 255, 255, 255),
    preview_top: rgba(28, 30, 38, 255),
    preview_bottom: rgba(12, 12, 16, 255),
    scroll_track: rgba(90, 90, 96, 255),
    scroll_thumb: rgba(220, 220, 226, 255),
    tab_idle: rgba(160, 160, 167, 255),
};

pub(super) const DARK: Theme = Theme {
    border: rgba(6, 6, 9, 255),
    bevel_hi: rgba(74, 78, 96, 255),
    bevel_lo: rgba(12, 13, 18, 255),
    fill_top: rgba(42, 44, 56, 255),
    fill_bottom: rgba(32, 34, 44, 255),
    slot: rgba(22, 23, 30, 255),
    slot_shadow: rgba(12, 12, 17, 255),
    slot_light: rgba(62, 65, 80, 255),
    slot_inner: rgba(30, 32, 41, 255),
    hover: rgba(98, 214, 120, 60),
    label: rgba(214, 218, 232, 255),
    label_shadow: true,
    idle: rgba(76, 80, 98, 255),
    progress: rgba(120, 226, 140, 255),
    preview_top: rgba(18, 19, 26, 255),
    preview_bottom: rgba(6, 6, 9, 255),
    scroll_track: rgba(22, 23, 30, 255),
    scroll_thumb: rgba(112, 118, 140, 255),
    tab_idle: rgba(26, 27, 36, 255),
};

/// Draws an item icon with its stack count and durability bar. `size` is the icon size in pixels.
pub fn draw_stack(ui: &mut Ui, x: f32, y: f32, size: f32, st: &Stack) {
    let c = Vec2::new(x + size * 0.5, y + size * 0.5);
    // A gun, magazine or part as it is (its rounds, attachments, dirt), once drawn.
    let state = super::icons::state_icon(st);
    match icon(st.item) {
        _ if state.is_some() => ui.block_sprite(c, size * 0.5, state.unwrap_or(0), [255; 3]),
        Icon::Block(b) if is_stairs(b) => {
            // Two boxes seen from the same corner as the cube icons: the step in front,
            // the tall part behind it.
            let r = size * 0.47;
            let k = 0.866 * r;
            let at = |x: f32, y: f32, z: f32| {
                Vec2::new(
                    c.x + k * (x + z - 1.0),
                    c.y + r * (1.0 - y) - r * 0.5 * (1.0 + z - x),
                )
            };
            let layer = face_texture(b, 2);
            for (lo, hi) in [([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]), ([0.0, 0.5, 0.5], [1.0, 1.0, 1.0])] {
                let [x0, y0, z0] = lo;
                let [x1, y1, z1] = hi;
                let top = [at(x0, y1, z1), at(x1, y1, z1), at(x1, y1, z0), at(x0, y1, z0)];
                let front = [at(x0, y1, z0), at(x1, y1, z0), at(x1, y0, z0), at(x0, y0, z0)];
                let side = [at(x1, y1, z0), at(x1, y1, z1), at(x1, y0, z1), at(x1, y0, z0)];
                ui.tex_quad(top, layer, 1.0);
                ui.tex_quad(front, layer, 0.8);
                ui.tex_quad(side, layer, 0.62);
            }
        }
        Icon::Block(b) if is_log(b) => {
            // Round, upright, seen from the same corner as the cube icons: the bark toward
            // us, the rings on top.
            let r = size * 0.47;
            let k = 0.866 * r;
            let at = |x: f32, y: f32, z: f32| {
                Vec2::new(
                    c.x + k * (x + z - 1.0),
                    c.y + r * (1.0 - y) - r * 0.5 * (1.0 + z - x),
                )
            };
            const SIDES: usize = 12;
            let (rad, rim) = (log_radius(OAK_LOG), crate::world::mesh::LOG_END_RIM);
            let ang = |i: usize| i as f32 / SIDES as f32 * std::f32::consts::TAU;
            let rim_at = |i: usize| (0.5 + ang(i).cos() * rad, 0.5 + ang(i).sin() * rad);
            let (bark, rings) = (face_texture(b, 0), face_texture(b, 2));
            for i in 0..SIDES {
                let mid = (ang(i) + ang(i + 1)) * 0.5;
                let (nc, ns) = (mid.cos(), mid.sin());
                // Only the sides facing us (toward +x and -z).
                if nc - ns <= 0.0 {
                    continue;
                }
                let ((x0, z0), (x1, z1)) = (rim_at(i), rim_at(i + 1));
                let (u0, u1) = ((i % 4) as f32 / 4.0, (i % 4 + 1) as f32 / 4.0);
                let shade = 0.71 + 0.09 * (-ns - nc);
                ui.tex_quad_uv(
                    [at(x0, 1.0, z0), at(x1, 1.0, z1), at(x1, 0.0, z1), at(x0, 0.0, z0)],
                    [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]],
                    bark,
                    shade,
                );
            }
            let f = rim / rad;
            let uv = |x: f32, z: f32| [0.5 + (x - 0.5) * f, 0.5 + (z - 0.5) * f];
            for i in 0..SIDES {
                let ((x0, z0), (x1, z1)) = (rim_at(i), rim_at(i + 1));
                let p = [at(0.5, 1.0, 0.5), at(x0, 1.0, z0), at(x1, 1.0, z1), at(x1, 1.0, z1)];
                ui.tex_quad_uv(p, [uv(0.5, 0.5), uv(x0, z0), uv(x1, z1), uv(x1, z1)], rings, 1.0);
            }
        }
        Icon::Block(b) => {
            let tint = icon_tint(b);
            let top = if tint_kind(b, 2) != TintKind::None {
                tint
            } else {
                [255; 3]
            };
            let side = if tint_kind(b, 0) != TintKind::None {
                tint
            } else {
                [255; 3]
            };
            ui.block_icon_faces(
                c,
                size * 0.47,
                face_texture(b, 2),
                face_texture(b, 5),
                face_texture(b, 0),
                top,
                side,
            );
        }
        Icon::Flat(layer) => {
            let tint = block_of(st.item).map(icon_tint).unwrap_or([255; 3]);
            ui.block_sprite(c, size * 0.5, layer, tint);
        }
    }
    let px = (size / 16.0).max(1.0);
    let max = max_damage(st.item);
    // A gun's dirt shows on the gun itself, not as a bar.
    let dirt = gets_dirty(st.item);
    if max > 0 && st.damage > 0 && !dirt {
        let f = 1.0 - st.damage as f32 / max as f32;
        let (bx, by, bw) = (x + 2.0 * px, y + size - 3.0 * px, size - 4.0 * px);
        ui.solid(bx, by, bw, 2.0 * px, rgba(0, 0, 0, 255));
        let col = [(1.0 - f).min(1.0) * 2.0, f * 2.0, 0.0, 1.0].map(|v: f32| v.min(1.0));
        ui.solid(bx, by, (bw * f).round().max(px), px, col);
    }
    if st.count > 1 {
        let text = st.count.to_string();
        let fs = (px * 0.75).round().max(1.0);
        let tw = ui.text_width(&text, fs);
        ui.text(
            &text,
            x + size - tw + px * 0.5,
            y + size - 7.0 * fs + px * 0.5,
            fs,
            WHITE,
            true,
        );
    }
}

impl Game {
    pub(super) fn theme(&self) -> &'static Theme {
        if self.settings.dark_ui {
            &DARK
        } else {
            &LIGHT
        }
    }

    /// Draws one slot and returns whether the mouse is over it.
    pub(super) fn draw_slot(&mut self, x: f32, y: f32, content: Option<Stack>) -> bool {
        self.draw_slot_sized(x, y, SLOT, content)
    }

    /// Slot of `gui` GUI pixels (18 normal, 26 for result slots); the item is centered.
    pub(super) fn draw_slot_sized(&mut self, x: f32, y: f32, gui: f32, content: Option<Stack>) -> bool {
        let s = self.ui.s;
        let size = gui * s;
        let hovered = self.ui.hit(x, y, size, size);
        let th = self.theme();
        self.ui.solid(x, y, size, size, th.slot);
        self.ui.solid(x, y, size - s, s, th.slot_shadow);
        self.ui.solid(x, y, s, size - s, th.slot_shadow);
        self.ui
            .solid(x + s, y + size - s, size - s, s, th.slot_light);
        self.ui
            .solid(x + size - s, y + s, s, size - s, th.slot_light);
        self.ui
            .solid(x + s, y + s, size - 2.0 * s, size - 2.0 * s, th.slot_inner);
        if hovered {
            self.ui
                .solid(x + s, y + s, size - 2.0 * s, size - 2.0 * s, th.hover);
        }
        if let Some(st) = content {
            let o = (gui - 16.0) * 0.5 * s;
            draw_stack(&mut self.ui, x + o, y + o, 16.0 * s, &st);
        }
        hovered
    }

    pub(super) fn tooltip_for(&mut self, st: &Stack) {
        let mut text = name(st.item);
        let max = max_damage(st.item);
        if let Some(kind) = GunKind::of(st.item) {
            let size = kind.magazine_size(gun_mods(st));
            let rounds = if gun_has_mag(st) {
                tf("gun.magazine", &[&gun_ready_rounds(st), &size])
            } else {
                crate::lang::t("gun.no_mag").split('!').next().unwrap_or("").to_string()
            };
            text = format!("{text}  ({rounds})");
        } else if st.item == AMMO_BOX {
            text = match box_ammo(st.data) {
                Some(kind) => format!("{text}  ({}/{} {})", box_rounds(st), AMMO_BOX_ROUNDS, name(kind)),
                None => format!("{text}  (0/{AMMO_BOX_ROUNDS})"),
            };
        } else if let Some(cap) = magazine_capacity(st.item) {
            text = format!(
                "{text}  ({}, {})",
                tf("gun.magazine", &[&gun_rounds(st), &cap]),
                crate::lang::t("gun.mag_hint")
            );
        } else if max > 0 && st.damage > 0 && !gets_dirty(st.item) {
            text = format!(
                "{text}  ({})",
                tf("gui.durability", &[&(max - st.damage), &max])
            );
        }
        self.ui.set_tooltip(&text);
    }

    /// Main inventory (3 rows) and hotbar at GUI offset (ox, oy) = top-left of the main rows.
    pub(super) fn inventory_slots(
        &mut self,
        px: f32,
        py: f32,
        oy: f32,
        hovered: &mut Option<SlotRef>,
    ) {
        let s = self.ui.s;
        for i in 9..36 {
            let (cx, cy) = ((i - 9) % 9, (i - 9) / 9);
            let (x, y) = (
                px + (8.0 + cx as f32 * SLOT) * s,
                py + (oy + cy as f32 * SLOT) * s,
            );
            if self.draw_slot(x, y, self.me.items.inventory.slots[i]) {
                *hovered = Some(SlotRef::Inv(i));
            }
        }
        for i in 0..9 {
            let (x, y) = (px + (8.0 + i as f32 * SLOT) * s, py + (oy + 58.0) * s);
            if self.draw_slot(x, y, self.me.items.inventory.slots[i]) {
                *hovered = Some(SlotRef::Inv(i));
            }
        }
    }

    /// The armor slots at these places (helmet, chestplate, leggings, boots, vest), each
    /// showing a faint picture of what goes there while empty.
    pub(super) fn armor_slots(&mut self, spots: [(f32, f32); ARMOR_SLOTS], hovered: &mut Option<SlotRef>) {
        let s = self.ui.s;
        for (i, (x, y)) in spots.into_iter().enumerate() {
            if self.draw_slot(x, y, self.me.items.inventory.armor[i]) {
                *hovered = Some(SlotRef::Armor(i));
            }
            if self.me.items.inventory.armor[i].is_none() {
                let hint = if i == VEST_SLOT { BULLETPROOF_VEST } else { armor_id(2, i) };
                draw_stack(&mut self.ui, x + s, y + s, 16.0 * s, &Stack::one(hint));
                let th = self.theme();
                self.ui
                    .solid(x + s, y + s, 16.0 * s, 16.0 * s, with_alpha(th.slot_inner, 0.8));
            }
        }
    }

    pub(super) fn panel(&mut self, pw: f32, ph: f32) -> (f32, f32) {
        self.panel_below(pw, ph, 0.0)
    }

    /// A window centered together with `top` GUI pixels above it (the creative tabs).
    pub(super) fn panel_below(&mut self, pw: f32, ph: f32, top: f32) -> (f32, f32) {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        self.ui
            .gradient(0.0, 0.0, w, h, rgba(0, 0, 0, 150), rgba(0, 0, 0, 110));
        let (px, py) = (
            ((w - pw * s) * 0.5).round(),
            ((h - (ph + top) * s) * 0.5 + top * s).round(),
        );
        let (bw, bh) = (pw * s, ph * s);
        self.ui.rect_full(
            px - s,
            py + s,
            bw + 2.0 * s,
            bh + 2.0 * s,
            rgba(0, 0, 0, 90),
            rgba(0, 0, 0, 90),
            5.0 * s,
            6.0 * s,
        );
        let th = self.theme();
        self.ui.rect(
            px - s,
            py - s,
            bw + 2.0 * s,
            bh + 2.0 * s,
            th.border,
            4.0 * s,
        );
        self.ui.rect(px, py, bw, bh, th.bevel_hi, 3.0 * s);
        self.ui.rect(
            px + 2.0 * s,
            py + 2.0 * s,
            bw - 2.0 * s,
            bh - 2.0 * s,
            th.bevel_lo,
            3.0 * s,
        );
        self.ui.rect_full(
            px + 2.0 * s,
            py + 2.0 * s,
            bw - 4.0 * s,
            bh - 4.0 * s,
            th.fill_top,
            th.fill_bottom,
            2.0 * s,
            0.0,
        );
        (px, py)
    }

    /// Minecraft-style progress arrow `w` GUI pixels long (15 tall), filled left to right by `k`.
    pub(super) fn arrow(&mut self, x: f32, y: f32, w: f32, k: f32) {
        let s = self.ui.s;
        let head = 8.0;
        let fill = (w * k.clamp(0.0, 1.0)).round();
        let th = self.theme();
        for col in 0..w as i32 {
            let c = col as f32;
            let (y0, h) = if c < w - head {
                (5.0, 5.0)
            } else {
                let d = w - 1.0 - c;
                (7.0 - d, 1.0 + 2.0 * d)
            };
            let color = if c < fill { th.progress } else { th.idle };
            self.ui.solid(x + c * s, y + y0 * s, s, h * s, color);
        }
    }

    pub(super) fn label(&mut self, text: &str, x: f32, y: f32) {
        let size = (self.ui.s * 0.75).round().max(1.0);
        let th = self.theme();
        self.ui.text(text, x, y, size, th.label, th.label_shadow);
    }
}
