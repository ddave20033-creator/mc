//! The gun station's screen: a 3D view of the pistol on the left, the three modes on the
//! right (putting a pistol together part by part, cleaning one and fitting attachments) and
//! the inventory below.

use super::super::guns::BenchMode;
use super::*;
use crate::model::gun::{self, PARTS};
use glam::Mat3;

/// Panel size in GUI pixels.
pub(super) const PANEL: (f32, f32) = (270.0, 250.0);
/// The 3D view (x, y, width, height) and the right column's left edge and width.
const VIEW: (f32, f32, f32, f32) = (8.0, 17.0, 160.0, 132.0);
const COLUMN: (f32, f32) = (176.0, 86.0);
/// Seconds for a part to slide into place.
const INSTALL_TIME: f32 = 0.45;
/// Seconds of scrubbing that clean a completely dirty part.
const SCRUB_TIME: f32 = 1.2;

const PART_NAMES: [&str; PARTS] = [
    "gun.part.frame",
    "gun.part.barrel",
    "gun.part.spring",
    "gun.part.slide",
    "gun.part.magazine",
];

/// A face of the pistol model as drawn in the view.
struct Face {
    quad: [Vec2; 4],
    depth: f32,
    layer: u32,
    shade: f32,
    tint: [u8; 3],
    part: usize,
}

/// Ease in and out, 0..1.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], k: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * k.clamp(0.0, 1.0)) as u8)
}

/// Is `p` inside the convex quad `q` (either winding)?
fn in_quad(p: Vec2, q: &[Vec2; 4]) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let c = (b - a).perp_dot(p - a);
        if c.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

/// The six faces of a box: outward normal and corners (top-left, top-right, bottom-right,
/// bottom-left as seen from outside).
fn box_faces(lo: Vec3, hi: Vec3) -> [(Vec3, [Vec3; 4]); 6] {
    let v = Vec3::new;
    let (x0, y0, z0, x1, y1, z1) = (lo.x, lo.y, lo.z, hi.x, hi.y, hi.z);
    [
        (Vec3::Z, [v(x0, y1, z1), v(x1, y1, z1), v(x1, y0, z1), v(x0, y0, z1)]),
        (Vec3::NEG_Z, [v(x1, y1, z0), v(x0, y1, z0), v(x0, y0, z0), v(x1, y0, z0)]),
        (Vec3::X, [v(x1, y1, z1), v(x1, y1, z0), v(x1, y0, z0), v(x1, y0, z1)]),
        (Vec3::NEG_X, [v(x0, y1, z0), v(x0, y1, z1), v(x0, y0, z1), v(x0, y0, z0)]),
        (Vec3::Y, [v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)]),
        (Vec3::NEG_Y, [v(x0, y0, z1), v(x1, y0, z1), v(x1, y0, z0), v(x0, y0, z0)]),
    ]
}

impl Game {
    /// Draws the gun station screen (the panel is at `px`, `py`) and handles its clicks; the
    /// slots are reported through `hovered` like the other item screens.
    pub(super) fn gun_station_panel(&mut self, px: f32, py: f32, hovered: &mut Option<SlotRef>) {
        let s = self.ui.s;
        let at = |gx: f32, gy: f32| (px + gx * s, py + gy * s);
        let (tx, ty) = at(8.0, 6.0);
        self.label(t("gui.gun_station"), tx, ty);

        // The three modes, as buttons on the right; the chosen one has a green frame.
        let modes = [
            (BenchMode::Assemble, "gun.assemble_1", "gun.assemble_2"),
            (BenchMode::Clean, "gun.clean_1", "gun.clean_2"),
            (BenchMode::Tune, "gun.tuning_1", "gun.tuning_2"),
        ];
        for (i, (mode, line1, line2)) in modes.into_iter().enumerate() {
            let (bx, by) = at(COLUMN.0, 17.0 + i as f32 * 23.0);
            let (bw, bh) = (COLUMN.1 * s, 20.0 * s);
            if self.guns.bench.mode == mode {
                self.ui
                    .rect(bx - s, by - s, bw + 2.0 * s, bh + 2.0 * s, rgba(98, 214, 120, 255), 4.0 * s);
            }
            if self.ui.button("", bx, by, bw, bh, true) && self.guns.bench.mode != mode {
                self.set_bench_mode(mode);
            }
            let cx = bx + bw * 0.5;
            self.ui.text_centered(t(line1), cx, by + 2.5 * s, s, WHITE, true);
            self.ui.text_centered(t(line2), cx, by + 10.5 * s, s, WHITE, true);
        }

        let hovered_part = self.gun_view(px, py);
        match self.guns.bench.mode {
            BenchMode::Assemble => {
                self.parts_list(px, py);
                if self.ui.pressed {
                    let bench = &self.guns.bench;
                    if bench.finished {
                        let (vx, vy) = at(VIEW.0, VIEW.1);
                        if self.ui.hit(vx, vy, VIEW.2 * s, VIEW.3 * s) {
                            self.guns.bench.finished = false;
                            self.guns.bench.installed = 0;
                        }
                    } else if hovered_part == Some(bench.installed) {
                        self.install_next_part();
                    }
                }
            }
            BenchMode::Clean => {
                self.clean_column(px, py, hovered);
                self.scrub(hovered_part);
            }
            BenchMode::Tune => self.tune_column(px, py, hovered),
        }

        let (lx, ly) = at(8.0, 156.0);
        self.label(t("gui.inventory"), lx, ly);
        // The inventory is centered under the wider panel.
        let inv_x = px + (PANEL.0 - 176.0) * 0.5 * s;
        self.inventory_slots(inv_x, py, 167.0, hovered);
    }

    fn set_bench_mode(&mut self, mode: BenchMode) {
        // Parts already put in go back when switching away from assembling (the pistol being
        // cleaned or tuned stays).
        let parts = self.guns.bench.parts();
        self.guns.bench.reset_assembly();
        for st in parts {
            self.give(st);
        }
        self.guns.bench.mode = mode;
        self.guns.bench.frame = None;
    }

    /// Tuning: the pistol's slot and a slot for each attachment.
    fn tune_column(&mut self, px: f32, py: f32, hovered: &mut Option<SlotRef>) {
        let s = self.ui.s;
        let (x0, y0) = (px + COLUMN.0 * s, py + 90.0 * s);
        if self.draw_slot(x0, y0, self.guns.bench.gun) {
            *hovered = Some(SlotRef::GunSlot);
        }
        self.label(t("gun.pistol"), x0 + 22.0 * s, y0 + 5.5 * s);
        self.label(t("gun.attachments"), x0, y0 + 24.0 * s);
        let mods = self.guns.bench.gun.map_or(0, |g| gun_mods(&g));
        let names = ["gun.mod.scope", "gun.mod.silencer", "gun.mod.extended", "gun.mod.laser"];
        for (i, (bit, item)) in ATTACHMENTS.into_iter().enumerate() {
            let (x, y) = (x0 + i as f32 * 21.0 * s, y0 + 34.0 * s);
            let fitted = mods & bit != 0;
            let content = fitted.then(|| Stack::one(item));
            if self.draw_slot(x, y, content) {
                *hovered = Some(SlotRef::GunMod(i));
                self.ui.set_tooltip(t(names[i]));
            }
            if !fitted {
                // A faint picture of what goes there.
                let o = s;
                draw_stack(&mut self.ui, x + o, y + o, 16.0 * s, &Stack::one(item));
                let th = self.theme();
                self.ui
                    .solid(x + s, y + s, 16.0 * s, 16.0 * s, with_alpha(th.slot_inner, 0.75));
            }
        }
    }

    /// A click on an attachment's slot: fits the attachment on the cursor, or takes the
    /// fitted one off (onto the cursor; with shift into the inventory).
    pub(super) fn click_gun_mod(&mut self, i: usize, shift: bool) {
        let (bit, item) = ATTACHMENTS[i];
        let Some(mut gun) = self.guns.bench.gun else {
            return;
        };
        let mods = gun_mods(&gun);
        if mods & bit != 0 {
            if self.cursor.is_some() && !shift {
                return;
            }
            set_gun_mods(&mut gun, mods & !bit);
            // Off with the extended magazine: the rounds that no longer fit come out.
            let size = magazine_size(mods & !bit);
            let extra = gun_rounds(&gun).saturating_sub(size);
            if extra > 0 {
                set_gun_rounds(&mut gun, size);
                self.give(Stack::new(BULLET, extra));
            }
            self.guns.bench.gun = Some(gun);
            if shift {
                self.give(Stack::one(item));
            } else {
                self.cursor = Some(Stack::one(item));
            }
        } else if self.cursor.is_some_and(|c| c.item == item) {
            set_gun_mods(&mut gun, mods | bit);
            take(&mut self.cursor, 1);
            self.guns.bench.gun = Some(gun);
        }
    }

    /// Puts the next part in (from the inventory; free in creative). The last one finishes
    /// the pistol, which goes into the inventory.
    fn install_next_part(&mut self) {
        let n = self.guns.bench.installed;
        let since = self.time - self.guns.bench.installed_at;
        if n >= PARTS || since < INSTALL_TIME {
            return;
        }
        let item = PISTOL_PARTS[n];
        let taken = if self.creative() {
            false
        } else if self.inventory.remove_one(item) {
            true
        } else {
            self.guns.bench.missing_at = self.time;
            return;
        };
        let bench = &mut self.guns.bench;
        bench.taken[n] = taken;
        bench.installed = n + 1;
        bench.installed_at = self.time;
        if bench.installed == PARTS {
            bench.finished = true;
            bench.taken = [false; PARTS];
            self.give(Stack::one(PISTOL));
        }
    }

    /// Holding the left mouse button on a part scrubs it clean.
    fn scrub(&mut self, part: Option<usize>) {
        let dt = self.ui.dt;
        let time = self.time;
        let bench = &mut self.guns.bench;
        bench.bubbles.retain(|b| time - b.1 < 0.6);
        let Some(gun) = bench.gun else {
            bench.dirt_of = None;
            return;
        };
        if bench.dirt_of != Some(gun.damage) {
            // A pistol was put in: all its parts are as dirty as it is.
            let d = gun.damage as f32 / PISTOL_DIRT_MAX as f32;
            bench.dirt = [d; PARTS];
            bench.dirt_of = Some(gun.damage);
        }
        let Some(p) = part.filter(|_| self.left_down) else {
            return;
        };
        if bench.dirt[p] <= 0.0 {
            return;
        }
        bench.dirt[p] = (bench.dirt[p] - dt / SCRUB_TIME).max(0.0);
        let mean = bench.dirt.iter().sum::<f32>() / PARTS as f32;
        let damage = (mean * PISTOL_DIRT_MAX as f32).ceil() as u16;
        if let Some(g) = &mut bench.gun {
            g.damage = damage;
        }
        bench.dirt_of = Some(damage);
        if bench.bubbles.last().is_none_or(|b| time - b.1 > 0.05) {
            let jitter = Vec2::new(self.rng.next() - 0.5, self.rng.next() - 0.5) * 10.0 * self.ui.s;
            bench.bubbles.push((self.ui.mouse + jitter, time));
        }
    }

    /// The 3D view of the pistol. Returns the part under the mouse.
    fn gun_view(&mut self, px: f32, py: f32) -> Option<usize> {
        let s = self.ui.s;
        let (vx, vy, vw, vh) = (px + VIEW.0 * s, py + VIEW.1 * s, VIEW.2 * s, VIEW.3 * s);
        let th = self.theme();
        self.ui.gradient(vx, vy, vw, vh, th.preview_top, th.preview_bottom);
        let inside = self.ui.hit(vx, vy, vw, vh);

        // Turning: drag with the right mouse button; until then it sways gently by itself.
        let time = self.time;
        let mouse = self.ui.mouse;
        let bench = &mut self.guns.bench;
        match (self.right_down, bench.drag_from) {
            (true, Some(from)) => {
                let d = (mouse - from) / s;
                bench.yaw += d.x * 0.02;
                bench.pitch = (bench.pitch + d.y * 0.02).clamp(-1.3, 1.3);
                bench.drag_from = Some(mouse);
                bench.turned = true;
            }
            (true, None) if inside => bench.drag_from = Some(mouse),
            (false, _) => bench.drag_from = None,
            _ => {}
        }
        let sway = if bench.turned { 0.0 } else { (time * 0.6).sin() * 0.45 };
        let rot = Mat3::from_rotation_x(bench.pitch) * Mat3::from_rotation_y(bench.yaw + sway);

        // Where each part is and how it is tinted (None: not shown).
        let mode = bench.mode;
        let creative = self.game_mode == GameMode::Creative;
        let placement = |part: usize| -> Option<(Vec3, [u8; 3])> {
            match mode {
                BenchMode::Assemble => {
                    let n = bench.installed;
                    if part < n {
                        let k = if part + 1 == n {
                            1.0 - ease((time - bench.installed_at) / INSTALL_TIME)
                        } else {
                            0.0
                        };
                        Some((gun::assembly_offset(part) * k, [255; 3]))
                    } else if part == n && !bench.finished {
                        let have = creative || self.inventory.count(PISTOL_PARTS[part]) > 0;
                        let bob = Vec3::Y * (time * 3.0).sin() * 0.3;
                        let tint = if have {
                            let glow = 0.5 + 0.5 * (time * 5.0).sin();
                            lerp_rgb([230, 230, 230], [255, 255, 200], glow)
                        } else {
                            [80, 80, 90]
                        };
                        Some((gun::assembly_offset(part) + bob, tint))
                    } else {
                        None
                    }
                }
                BenchMode::Clean => {
                    bench.gun?;
                    let dirt = bench.dirt[part];
                    Some((gun::exploded_offset(part), lerp_rgb([255; 3], [200, 160, 115], dirt)))
                }
                BenchMode::Tune => bench.gun.map(|_| (Vec3::ZERO, [255; 3])),
            }
        };
        // The pistol on the bench shows its attachments (a new one has none yet).
        let mods = match mode {
            BenchMode::Assemble => 0,
            _ => bench.gun.map_or(0, |g| gun_mods(&g)),
        };

        // Project every visible face; the view frames what is shown and eases toward it.
        let light = Vec3::new(-0.35, 0.6, 0.75).normalize();
        let mut faces: Vec<Face> = Vec::new();
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        let shown: [bool; PARTS] = std::array::from_fn(|p| placement(p).is_some());
        for b in gun::boxes().iter().filter(|b| b.shown(mods)) {
            let Some((offset, tint)) = placement(b.part) else {
                continue;
            };
            // Sorting faces cannot show a box inside another: draw those first, so the part
            // around them covers them (put together, the parts are only moving into place).
            let behind = mode == BenchMode::Assemble && b.inside.is_some_and(|q| shown[q]);
            let m = Mat4::from_translation(offset) * b.transform();
            for (n, corners) in box_faces(b.min, b.max) {
                let q = corners.map(|c| rot * m.transform_point3(c));
                for p in &q {
                    lo = lo.min(p.truncate());
                    hi = hi.max(p.truncate());
                }
                let n = rot * m.transform_vector3(n).normalize();
                if n.z <= 0.01 {
                    continue;
                }
                faces.push(Face {
                    quad: q.map(|p| Vec2::new(p.x, p.y)),
                    depth: q.iter().map(|p| p.z).sum::<f32>() / 4.0 - if behind { 1000.0 } else { 0.0 },
                    layer: b.layer,
                    shade: 0.5 + 0.5 * n.dot(light).max(0.0),
                    tint: [
                        (tint[0] as u32 * b.tint[0] as u32 / 255) as u8,
                        (tint[1] as u32 * b.tint[1] as u32 / 255) as u8,
                        (tint[2] as u32 * b.tint[2] as u32 / 255) as u8,
                    ],
                    part: b.part,
                });
            }
        }
        let mut hovered = None;
        if !faces.is_empty() {
            let size = (hi - lo).max(Vec2::splat(1.0));
            let target = ((lo + hi) * 0.5, (vw * 0.86 / size.x).min(vh * 0.8 / size.y));
            let (center, scale) = match bench.frame {
                Some((c, k)) => {
                    let e = 1.0 - (-6.0 * self.ui.dt).exp();
                    (c + (target.0 - c) * e, k + (target.1 - k) * e)
                }
                None => target,
            };
            bench.frame = Some((center, scale));
            let origin = Vec2::new(vx + vw * 0.5, vy + vh * 0.5);
            let to_screen = |p: Vec2| origin + Vec2::new(p.x - center.x, center.y - p.y) * scale;
            faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
            self.ui.set_clip(Some([vx, vy, vw, vh]));
            for f in &mut faces {
                f.quad = f.quad.map(to_screen);
                if inside && in_quad(mouse, &f.quad) {
                    hovered = Some(f.part);
                }
            }
            let bench = &self.guns.bench;
            let grab = |part: usize| match bench.mode {
                BenchMode::Assemble => part == bench.installed && !bench.finished,
                BenchMode::Clean => true,
                BenchMode::Tune => false,
            };
            let hovered_ok = hovered.filter(|&p| grab(p));
            for f in &faces {
                let lit = hovered_ok == Some(f.part);
                let tint = if lit {
                    lerp_rgb(f.tint, [255, 255, 255], 0.5)
                } else {
                    f.tint
                };
                let shade = if lit { (f.shade + 0.15).min(1.0) } else { f.shade };
                self.ui.tex_quad_tint(f.quad, f.layer, shade, tint);
                // Grime on the parts being cleaned: the crack pattern in dark brown.
                if bench.mode == BenchMode::Clean {
                    let dirt = bench.dirt[f.part];
                    if dirt > 0.02 {
                        let stage = ((dirt * 10.0) as u32).min(9);
                        let layer = crate::world::textures::tex::CRACK + stage;
                        self.ui.tex_quad_tint(f.quad, layer, 1.0, [150, 110, 70]);
                    }
                }
            }
            self.ui.set_clip(None);
            hovered = hovered_ok;
        } else {
            self.guns.bench.frame = None;
        }

        // Soap bubbles where the parts are being scrubbed.
        for &(p, born) in &self.guns.bench.bubbles.clone() {
            let age = (time - born) / 0.6;
            let c = with_alpha(rgba(235, 245, 255, 255), 1.0 - age);
            self.ui
                .ring(p - Vec2::Y * age * 14.0 * s, (1.5 + age * 2.5) * s, 0.8 * s, c);
        }

        // What to do, at the bottom of the view.
        let bench = &self.guns.bench;
        let fs = (s * 0.75).round().max(1.0);
        let (msg, color) = match bench.mode {
            BenchMode::Assemble if bench.finished => (
                format!("{} {}", t("gun.done"), t("gun.again")),
                rgba(120, 230, 140, 255),
            ),
            BenchMode::Assemble => {
                let n = bench.installed.min(PARTS - 1);
                let part = t(PART_NAMES[n]);
                if creative || self.inventory.count(PISTOL_PARTS[n]) > 0 {
                    (tf("gun.next", &[&part]), rgba(230, 230, 235, 255))
                } else {
                    (tf("gun.missing", &[&part]), rgba(255, 130, 110, 255))
                }
            }
            BenchMode::Clean => match bench.gun {
                None => (t("gun.put_gun").to_string(), rgba(230, 230, 235, 255)),
                Some(g) if g.damage == 0 => (t("gun.clean").to_string(), rgba(120, 230, 140, 255)),
                Some(_) => (t("gun.scrub").to_string(), rgba(230, 230, 235, 255)),
            },
            BenchMode::Tune => match bench.gun {
                None => (t("gun.put_gun").to_string(), rgba(230, 230, 235, 255)),
                Some(_) if mods != 0 => (t("gun.tune_ready").to_string(), rgba(230, 230, 235, 255)),
                Some(_) => (t("gun.tune_hint").to_string(), rgba(230, 230, 235, 255)),
            },
        };
        let lines = self.ui.wrap(&msg, vw - 8.0 * s, fs);
        let line_h = 9.0 * fs;
        let mut y = vy + vh - 4.0 * s - lines.len() as f32 * line_h;
        for line in &lines {
            self.ui.text_centered(line, vx + vw * 0.5, y, fs, color, true);
            y += line_h;
        }
        let hint = t("gun.rotate");
        let hw = self.ui.text_width(hint, fs);
        self.ui
            .text(hint, vx + vw - hw - 3.0 * s, vy + 3.0 * s, fs, rgba(150, 150, 160, 255), true);

        // A little brush on the mouse while scrubbing.
        if bench.mode == BenchMode::Clean && hovered.is_some() {
            let m = self.ui.mouse;
            let wiggle = if self.left_down { (time * 30.0).sin() * 1.5 * s } else { 0.0 };
            self.ui
                .solid(m.x + 2.0 * s + wiggle, m.y - 9.0 * s, 2.0 * s, 7.0 * s, rgba(140, 90, 50, 255));
            self.ui
                .solid(m.x + wiggle, m.y - 2.0 * s, 6.0 * s, 3.0 * s, rgba(230, 220, 190, 255));
        }
        hovered
    }

    /// Assembling: the parts in order, with how many of each you have.
    fn parts_list(&mut self, px: f32, py: f32) {
        let s = self.ui.s;
        let fs = (s * 0.75).round().max(1.0);
        let (x0, y0) = (px + COLUMN.0 * s, py + 88.0 * s);
        self.label(t("gun.parts"), x0, y0);
        let bench = &self.guns.bench;
        let (installed, finished, missing_at) = (bench.installed, bench.finished, bench.missing_at);
        let creative = self.creative();
        for (i, &item) in PISTOL_PARTS.iter().enumerate() {
            let y = y0 + (9.0 + i as f32 * 11.0) * s;
            let done = i < installed || finished;
            let current = i == installed && !finished;
            let have = self.inventory.count(item);
            if current {
                self.ui
                    .solid(x0 - s, y - s, COLUMN.1 * s, 11.0 * s, rgba(255, 255, 255, 40));
            }
            draw_stack(&mut self.ui, x0, y, 9.0 * s, &Stack::one(item));
            let flash = current && self.time - missing_at < 0.6;
            let color = if done {
                rgba(120, 230, 140, 255)
            } else if flash || (current && have == 0 && !creative) {
                rgba(255, 120, 100, 255)
            } else {
                self.theme().label
            };
            let name = t(PART_NAMES[i]);
            self.ui
                .text(name, x0 + 12.0 * s, y + 1.5 * s, fs, color, self.theme().label_shadow);
            let right = if done {
                "OK".to_string()
            } else if creative {
                String::new()
            } else {
                format!("x{have}")
            };
            let rw = self.ui.text_width(&right, fs);
            self.ui.text(
                &right,
                x0 + (COLUMN.1 - 3.0) * s - rw,
                y + 1.5 * s,
                fs,
                color,
                self.theme().label_shadow,
            );
            // Clicking the current part's line puts it in too.
            if current && self.ui.pressed && self.ui.hit(x0 - s, y - s, COLUMN.1 * s, 11.0 * s) {
                self.install_next_part();
            }
        }
    }

    /// Cleaning: the pistol's slot and how clean it is.
    fn clean_column(&mut self, px: f32, py: f32, hovered: &mut Option<SlotRef>) {
        let s = self.ui.s;
        let (x0, y0) = (px + COLUMN.0 * s, py + 90.0 * s);
        if self.draw_slot(x0, y0, self.guns.bench.gun) {
            *hovered = Some(SlotRef::GunSlot);
        }
        self.label(t("gun.pistol"), x0 + 22.0 * s, y0 + 5.5 * s);
        let Some(g) = self.guns.bench.gun else {
            return;
        };
        let clean = 1.0 - g.damage as f32 / PISTOL_DIRT_MAX as f32;
        let (bx, by, bw) = (x0, y0 + 26.0 * s, COLUMN.1 * s - 2.0 * s);
        self.ui.solid(bx, by, bw, 6.0 * s, rgba(0, 0, 0, 255));
        let color = [(1.0 - clean) * 2.0, clean * 2.0, 0.2, 1.0].map(|v: f32| v.min(1.0));
        self.ui
            .solid(bx + s, by + s, ((bw - 2.0 * s) * clean).max(0.0), 4.0 * s, color);
        let pct = (clean * 100.0).round() as u32;
        self.label(&tf("gun.cleanliness", &[&pct]), bx, by + 9.0 * s);
    }
}
