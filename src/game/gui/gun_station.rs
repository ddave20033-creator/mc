//! The gun station, in 3D: the camera glides over its table as over a crafting table. A gun
//! is put together floating over the table from the parts lying on it, taken apart on it to
//! be cleaned, and tuned lying on it with the attachments beside it, which the hand picks up
//! and fits on. The modes and what each needs are on a panel on the right, the inventory
//! along the bottom.

use super::super::guns::{Bench, BenchMode, FitAnim, Pick};
use crate::item::inventory::Inventory;
use super::super::station::{hit_plane, Screen2};
use super::*;
use crate::model::emit_box;
use crate::model::gun::{self, GunBox, PARTS};
use crate::model::player::ARM as ARM_LAYERS;
use crate::util::{ray_box, vertex_light};
use crate::world::mesh::flags;
use crate::world::textures::{skin_layer, tex};
use glam::{Mat3, Quat};
use std::f32::consts::FRAC_PI_2;

/// Seconds for a part to fly from the table into its place on the gun.
const INSTALL_TIME: f32 = 0.75;
/// Seconds of scrubbing that clean a completely dirty part.
const SCRUB_TIME: f32 = 1.2;
/// Seconds for the hand to lay the gun on the table, to let go of it, and for the gun to
/// come apart to be cleaned.
const PUT_TIME: f32 = 0.55;
const LET_GO: f32 = 0.3;
const SPREAD_TIME: f32 = 0.35;
/// Seconds for the hand to fit an attachment (or take one off), and when each step ends:
/// reaching for it, lifting it, carrying it to the gun, sliding it on; then the hand goes back.
pub(in crate::game) const FIT_TIME: f32 = 1.45;
const FIT_STEPS: [f32; 4] = [0.3, 0.4, 0.85, 1.1];
/// The right-hand panel's width (GUI pixels).
const PANEL_W: f32 = 104.0;
/// How high over the table the gun being put together floats, and how much bigger it is on
/// show there than lying on the table.
const FLOAT_HEIGHT: f32 = 0.4;
const FLOAT_SCALE: f32 = 1.2;

/// The brush sticks out this far past the fist.
const BRUSH_AHEAD: f32 = 0.09;
/// The fist is this far above what it holds.
const GRAB: Vec3 = Vec3::new(0.0, 0.035, 0.0);

/// Ease in and out, 0..1.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], k: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * k.clamp(0.0, 1.0)) as u8)
}

fn mul_rgb(a: [u8; 3], b: [u8; 3]) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as u32 * b[i] as u32 / 255) as u8)
}

/// Where a gun (or a part or attachment of it) is: its middle, how it is turned, how big.
#[derive(Clone, Copy, Debug)]
struct Place {
    pos: Vec3,
    rot: Quat,
    scale: f32,
}

impl Place {
    /// From gun space (with `pivot` at `pos`) to the world.
    fn matrix(self, pivot: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::splat(self.scale), self.rot, self.pos)
            * Mat4::from_translation(-pivot)
    }

    /// Where the point `p` of gun space is, with `pivot` at `pos`.
    fn point(self, pivot: Vec3, p: Vec3) -> Vec3 {
        self.matrix(pivot).transform_point3(p)
    }

    /// Part of the way (`k`) to `to`, rising by `arc` halfway.
    fn toward(self, to: Place, k: f32, arc: f32) -> Place {
        let k = k.clamp(0.0, 1.0);
        Place {
            pos: self.pos.lerp(to.pos, k) + Vec3::Y * arc * (k * PI).sin(),
            rot: self.rot.slerp(to.rot, k),
            scale: self.scale + (to.scale - self.scale) * k,
        }
    }

    fn moved(self, by: Vec3) -> Place {
        Place {
            pos: self.pos + by,
            ..self
        }
    }
}

/// The station's table top: its middle, and which way is right and toward the player.
struct Table {
    top: Vec3,
    right: Vec3,
    toward: Vec3,
    rot: Quat,
}

impl Table {
    fn new(p: IVec3, side: u8) -> Self {
        let toward = facing_dir(side).as_vec3();
        let right = (-toward).cross(Vec3::Y);
        Table {
            top: p.as_vec3() + Vec3::new(0.5, 1.0, 0.5),
            right,
            toward,
            rot: Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, toward)),
        }
    }

    /// A point over the table: `x` to the right, `y` up, `z` toward the player.
    fn at(&self, x: f32, y: f32, z: f32) -> Vec3 {
        self.top + self.right * x + Vec3::Y * y + self.toward * z
    }

    /// A gun lying on its side, the muzzle to the right and its top away from the player.
    fn lying(&self) -> Quat {
        self.rot * Quat::from_rotation_x(-FRAC_PI_2)
    }
}

/// How big a gun is on the table, its middle (gun space), the middle of its parts laid apart
/// for cleaning, and how high each of those lies over the table on its side.
struct Layout {
    scale: f32,
    center: Vec3,
    apart: Vec3,
    lift: f32,
    apart_lift: f32,
}

fn layout(kind: GunKind) -> Layout {
    let mods = kind.shown_mods(0);
    let (lo, hi) = gun::bounds_where(kind, |b| b.shown(mods));
    let (mut alo, mut ahi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for part in 0..PARTS {
        let (a, b) = gun::part_bounds(kind, part, mods);
        if a.x <= b.x {
            let e = gun::exploded_offset(kind, part);
            alo = alo.min(a + e);
            ahi = ahi.max(b + e);
        }
    }
    let scale = (0.8 / (hi - lo).x)
        .min(0.8 / (ahi - alo).x)
        .min(0.62 / (ahi - alo).y);
    let (center, apart) = ((lo + hi) * 0.5, (alo + ahi) * 0.5);
    Layout {
        scale,
        center,
        apart,
        lift: (center.z - lo.z) * scale + 0.003,
        apart_lift: (apart.z - alo.z) * scale + 0.003,
    }
}

/// Which boxes of a gun a piece is: of these parts (bit mask), shown with `mods`, leaving out
/// the attachments in `skip`, or only those of the attachment `only`.
#[derive(Clone, Copy)]
struct Sel {
    parts: u8,
    mods: u8,
    skip: u8,
    only: u8,
}

impl Sel {
    fn parts(parts: u8, mods: u8) -> Self {
        Sel {
            parts,
            mods,
            skip: 0,
            only: 0,
        }
    }

    fn attachment(bit: u8) -> Self {
        Sel {
            parts: 0xff,
            mods: bit,
            skip: 0,
            only: bit,
        }
    }

    fn picks(self, b: &GunBox) -> bool {
        self.parts & (1 << b.part) != 0
            && b.shown(self.mods)
            && b.with & self.skip == 0
            && (self.only == 0 || b.with == self.only)
    }
}

/// Something drawn on the table: which boxes, where, tinted how, and what it is to the mouse.
struct Piece {
    pick: Option<Pick>,
    sel: Sel,
    m: Mat4,
    tint: [u8; 3],
}

/// The camera: where it is and its axes.
#[derive(Clone, Copy)]
struct Cam {
    pos: Vec3,
    fwd: Vec3,
    right: Vec3,
    up: Vec3,
}

impl Cam {
    /// Where the arm comes from: just past the lower right corner of the view, so its end is
    /// never seen; and where the hand waits, out of view there.
    fn shoulder(&self) -> Vec3 {
        self.pos + self.fwd * 0.8 + self.right * 0.95 - self.up * 0.58
    }

    fn hand_rest(&self) -> Vec3 {
        self.pos + self.fwd * 0.8 + self.right * 0.85 - self.up * 0.52
    }

    /// A gun held in front of the camera, pointing ahead.
    fn holding(&self, scale: f32) -> Place {
        Place {
            pos: self.pos + self.fwd * 0.6 + self.right * 0.2 - self.up * 0.22,
            rot: Quat::from_mat3(&Mat3::from_cols(self.fwd, self.up, self.right)),
            scale,
        }
    }
}

/// The hand at work: its fist, and whether it holds the brush.
struct Hand {
    fist: Vec3,
    brush: bool,
}

/// The way an attachment goes on: down onto the rail, along the barrel, up into the well.
fn approach(kind: GunKind, bit: u8) -> Vec3 {
    match bit {
        gun_mod::SCOPE => Vec3::Y,
        gun_mod::EXTENDED_MAGAZINE => gun::spec(kind).mag_axis.normalize_or(Vec3::NEG_Y),
        _ => Vec3::X,
    }
}

/// Where the attachment in the hand is `u` seconds into fitting it (the same backwards takes
/// one off), and where the fist is: it reaches for it on the table, lifts it, carries it over
/// and slides it on, and goes back.
fn fit_pose(u: f32, bit: u8, rest: Place, near: Place, on: Place, hand: Vec3) -> (Place, Vec3) {
    let [a, b, c, d] = FIT_STEPS;
    let lifted = rest.moved(Vec3::Y * 0.1);
    let held = |p: Place| (p, p.pos + GRAB);
    if u < a {
        (rest, hand.lerp(rest.pos + GRAB, ease(u / a)))
    } else if u < b {
        held(rest.toward(lifted, ease((u - a) / (b - a)), 0.0))
    } else if u < c {
        held(lifted.toward(near, ease((u - b) / (c - b)), 0.08))
    } else if u < d {
        let k = ease((u - c) / (d - c));
        let mut p = near.toward(on, k, 0.0);
        if bit == gun_mod::SILENCER {
            // Screwed on.
            p.rot *= Quat::from_rotation_x((1.0 - k) * TAU * 1.5);
        }
        held(p)
    } else {
        (on, (on.pos + GRAB).lerp(hand, ease((u - d) / (FIT_TIME - d))))
    }
}

/// The patch of table under a piece lying on it (None while it is up in the air).
fn footprint(kind: GunKind, pc: &Piece, table: &Table) -> Option<[Vec3; 4]> {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for b in gun::boxes(kind).iter().filter(|b| pc.sel.picks(b)) {
        let m = pc.m * b.transform();
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            );
            let w = m.transform_point3(c) - table.top;
            let q = Vec3::new(w.dot(table.right), w.y, w.dot(table.toward));
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    if lo.x > hi.x || lo.y > 0.08 {
        return None;
    }
    let (pad, y) = (0.02, 0.002);
    let (x0, x1, z0, z1) = (lo.x - pad, hi.x + pad, lo.z - pad, hi.z + pad);
    Some([table.at(x0, y, z0), table.at(x1, y, z0), table.at(x1, y, z1), table.at(x0, y, z1)])
}

/// `v` along the ground, one long.
fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z).normalize_or_zero()
}

/// The nearest box of the pieces the mouse ray (`o` + t `d`) goes through, and where.
fn pick_piece(kind: GunKind, pieces: &[Piece], o: Vec3, d: Vec3) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (i, pc) in pieces.iter().enumerate().filter(|(_, pc)| pc.pick.is_some()) {
        for b in gun::boxes(kind).iter().filter(|b| pc.sel.picks(b)) {
            let inv = (pc.m * b.transform()).inverse();
            let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
            if let Some(t) = ray_box(lo, ld, b.min, b.max, 64.0) {
                if best.is_none_or(|(_, bt)| t < bt) {
                    best = Some((i, t));
                }
            }
        }
    }
    best
}

impl Game {
    /// What lies on the open gun station's table (see `bench_pieces`).
    fn bench_pieces(&self, p: IVec3, cam: Cam) -> Option<(GunKind, Vec<Piece>, Option<Hand>)> {
        let table = Table::new(p, self.table_side(p));
        bench_pieces(&self.guns.bench, self.time, &table, self.creative(), &self.inventory, cam)
    }

    /// The open gun station's table: the gun, its parts and attachments, and the hand at work.
    pub(in crate::game) fn build_gun_station(
        &self,
        out: &mut Vec<Vertex>,
        cam: Vec3,
        right: Vec3,
        up: Vec3,
    ) {
        let Screen::Container(Container::GunStation(p)) = self.screen else {
            return;
        };
        let cam = Cam {
            pos: cam,
            fwd: up.cross(right),
            right,
            up,
        };
        let Some((kind, pieces, hand)) = self.bench_pieces(p, cam) else {
            return;
        };
        let top = Table::new(p, self.table_side(p)).top;
        let (sky, blk) = self.terrain.world.light_estimate(top + Vec3::Y * 0.3);
        let light = vertex_light(sky, blk);
        emit_pieces(out, kind, &pieces, self.guns.bench.hover, light);
        if let Some(h) = hand {
            emit_arm(out, cam.shoulder(), &h, self.effective_skin(), light, self.time);
        }
    }

    /// The gun station: the panel on the right, the inventory along the bottom, what the mouse
    /// points at on the table, and what clicking does there. Returns the slot under the mouse
    /// (the gun on the table and its attachments count as slots).
    pub(super) fn gun_station_screen(&mut self, p: IVec3) -> Option<SlotRef> {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let mut hovered = None;
        // The inventory, on a dark strip along the bottom.
        let (pw, ph) = (176.0 * s, 86.0 * s);
        let (px, py) = (((w - pw) * 0.5).round(), (h - ph - 4.0 * s).round());
        self.ui.rect_full(
            px,
            py,
            pw,
            ph,
            rgba(10, 11, 16, 150),
            rgba(10, 11, 16, 190),
            5.0 * s,
            3.0 * s,
        );
        self.inventory_slots(px, py, 5.0, &mut hovered);
        let over_inventory = self.ui.hit(px, py, pw, ph);
        let over_panel = self.bench_panel(p);

        // A gun put on the table with the mouse is laid down by the hand too.
        let time = self.time;
        let bench = &mut self.guns.bench;
        if bench.gun.is_some() != bench.had_gun {
            bench.had_gun = bench.gun.is_some();
            bench.placed_at = time;
            bench.from_slot = None;
            bench.fit = None;
        }
        if bench.fit.is_some_and(|f| time - f.start >= FIT_TIME) {
            bench.fit = None;
        }
        let busy = bench.fit.is_some() || time - bench.placed_at < PUT_TIME + SPREAD_TIME;

        // What the mouse points at, once the camera is there.
        let ready = self
            .station
            .as_ref()
            .is_some_and(|st| st.blend > 0.9 && !st.closing);
        let view = Screen2 {
            view_proj: self.view_proj,
            w,
            h,
        };
        let (o, d) = view.ray(self.ui.mouse);
        let table = Table::new(p, self.table_side(p));
        let cam = Cam {
            pos: table.at(0.0, 1.0, 1.2),
            fwd: -table.toward,
            right: table.right,
            up: Vec3::Y,
        };
        let mut pick = None;
        let mut point = None;
        let found = self.bench_pieces(p, cam);
        if ready && !over_inventory && !over_panel {
            if let Some((kind, pieces, _)) = &found {
                if let Some((i, t)) = pick_piece(*kind, pieces, o, d) {
                    pick = pieces[i].pick;
                    point = Some(o + d * t);
                }
            }
            if pick.is_none() {
                let on_top = hit_plane(o, d, table.top.y).filter(|q| {
                    let r = *q - table.top;
                    r.x.abs() < 0.5 && r.z.abs() < 0.5
                });
                if on_top.is_some() {
                    pick = Some(Pick::Table);
                }
            }
        }
        self.guns.bench.hover = pick;

        let mode = self.guns.bench.mode;
        match mode {
            BenchMode::Assemble => {
                // Turning the floating gun: drag with the right mouse button.
                let mouse = self.ui.mouse;
                let bench = &mut self.guns.bench;
                match (self.right_down, bench.drag_from) {
                    (true, Some(from)) => {
                        bench.yaw += (mouse - from).x / s * 0.02;
                        bench.drag_from = Some(mouse);
                        bench.turned = true;
                    }
                    (true, None) if ready && !over_panel && !over_inventory => {
                        bench.drag_from = Some(mouse)
                    }
                    (false, _) => bench.drag_from = None,
                    _ => {}
                }
                if self.ui.pressed {
                    let bench = &self.guns.bench;
                    if bench.finished && pick == Some(Pick::Gun) {
                        self.guns.bench.reset_assembly();
                    } else if pick == Some(Pick::Part(bench.installed)) {
                        self.install_next_part(p);
                    }
                }
                if let Some(Pick::Part(i)) = pick {
                    self.ui.set_tooltip(t(self.guns.bench.kind.parts()[i].name));
                }
            }
            BenchMode::Clean => {
                let part = match pick {
                    Some(Pick::Part(i)) if !busy => Some(i),
                    _ => None,
                };
                self.scrub(part, point);
                if self.guns.bench.gun.is_none() && pick == Some(Pick::Table) {
                    hovered = Some(SlotRef::GunSlot);
                }
            }
            BenchMode::Tune => {
                let names = ["gun.mod.scope", "gun.mod.silencer", "gun.mod.extended", "gun.mod.laser"];
                match pick {
                    Some(Pick::Attachment(i)) if !busy => {
                        hovered = Some(SlotRef::GunMod(i));
                        self.ui.set_tooltip(t(names[i]));
                    }
                    Some(Pick::Gun) if !busy => hovered = Some(SlotRef::GunSlot),
                    Some(Pick::Table) if self.guns.bench.gun.is_none() => {
                        hovered = Some(SlotRef::GunSlot)
                    }
                    _ => {}
                }
            }
        }

        // What to do, over the inventory.
        let (msg, color) = self.bench_hint();
        let fs = (s * 0.75).round().max(1.0);
        let lines = self.ui.wrap(&msg, 280.0 * s, fs);
        let line_h = 9.0 * fs;
        let mut y = py - 6.0 * s - lines.len() as f32 * line_h;
        for line in &lines {
            self.ui.text_centered(line, w * 0.5, y, fs, color, true);
            y += line_h;
        }

        let min = p.as_vec3();
        let over_block = ray_box(o, d, min, min + Vec3::ONE, 64.0).is_some();
        self.station_inside = over_inventory || over_panel || over_block || pick.is_some();
        self.station_hover = None;
        // A glow on the table under what the mouse points at, or else under the next part.
        let next = match self.guns.bench.mode {
            BenchMode::Assemble if !self.guns.bench.finished => Some(Pick::Part(self.guns.bench.installed)),
            _ => None,
        };
        self.station_frame = found.and_then(|(kind, pieces, _)| {
            let under = |want: Option<Pick>| {
                pieces
                    .iter()
                    .filter(|pc| want.is_some() && pc.pick == want)
                    .find_map(|pc| footprint(kind, pc, &table))
            };
            under(pick).or_else(|| under(next))
        });
        hovered
    }

    /// The line telling what to do now, and its color.
    fn bench_hint(&self) -> (String, Color) {
        let bench = &self.guns.bench;
        let plain = rgba(235, 235, 240, 255);
        let good = rgba(120, 230, 140, 255);
        match bench.mode {
            BenchMode::Assemble if bench.finished => {
                (format!("{} {}", t("gun.done"), t("gun.again")), good)
            }
            BenchMode::Assemble => {
                let n = bench.installed.min(PARTS - 1);
                let p = &bench.kind.parts()[n];
                let part = t(p.name);
                if self.can_make(p) {
                    (format!("{}   ({})", tf("gun.next", &[&part]), t("gun.rotate")), plain)
                } else {
                    (tf("gun.missing", &[&part]), rgba(255, 130, 110, 255))
                }
            }
            BenchMode::Clean => match bench.gun {
                None => (t("gun.put_gun").to_string(), plain),
                Some(g) if g.damage == 0 => (t("gun.clean").to_string(), good),
                Some(_) => (t("gun.scrub").to_string(), plain),
            },
            BenchMode::Tune => match bench.gun {
                None => (t("gun.put_gun").to_string(), plain),
                Some(g) => {
                    let kind = GunKind::of(g.item);
                    let any = self.creative()
                        || ATTACHMENTS.iter().any(|&(bit, item)| {
                            kind.is_some_and(|k| k.fits(bit)) && self.inventory.count(item) > 0
                        });
                    if any {
                        (t("gun.tune_hint").to_string(), plain)
                    } else if gun_mods(&g) != 0 {
                        (t("gun.tune_ready").to_string(), plain)
                    } else {
                        (t("gun.no_attachments").to_string(), rgba(255, 190, 120, 255))
                    }
                }
            },
        }
    }

    /// The panel on the right: the three modes, and what the chosen one needs. Returns
    /// whether the mouse is over it.
    fn bench_panel(&mut self, p: IVec3) -> bool {
        let (w, s) = (self.ui.w, self.ui.s);
        let pw = PANEL_W * s;
        let (x0, y0) = ((w - pw - 6.0 * s).round(), (6.0 * s).round());
        let rows = match self.guns.bench.mode {
            BenchMode::Assemble => 5.0 * 10.0 + 15.0,
            BenchMode::Clean => 34.0,
            BenchMode::Tune => 12.0 + 4.0 * 10.0,
        };
        let ph = (18.0 + 3.0 * 22.0 + 6.0 + rows) * s;
        self.ui.rect_full(
            x0,
            y0,
            pw,
            ph,
            rgba(10, 11, 16, 170),
            rgba(10, 11, 16, 200),
            5.0 * s,
            3.0 * s,
        );
        let fs = (s * 0.75).round().max(1.0);
        let (x, inner) = (x0 + 6.0 * s, pw - 12.0 * s);
        self.ui.text_centered(
            t("gui.gun_station"),
            x0 + pw * 0.5,
            y0 + 5.0 * s,
            fs,
            rgba(255, 220, 150, 255),
            true,
        );

        // The three modes; the chosen one has a green frame.
        let modes = [
            (BenchMode::Assemble, "gun.assemble_1", "gun.assemble_2"),
            (BenchMode::Clean, "gun.clean_1", "gun.clean_2"),
            (BenchMode::Tune, "gun.tuning_1", "gun.tuning_2"),
        ];
        for (i, (mode, line1, line2)) in modes.into_iter().enumerate() {
            let (bx, by) = (x, y0 + (18.0 + i as f32 * 22.0) * s);
            let (bw, bh) = (inner, 20.0 * s);
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

        let y = y0 + (18.0 + 3.0 * 22.0 + 4.0) * s;
        match self.guns.bench.mode {
            BenchMode::Assemble => {
                self.gun_selector(x, y, inner);
                self.parts_list(x, y + 15.0 * s, inner, p);
            }
            BenchMode::Clean => {
                if let Some(g) = self.guns.bench.gun {
                    self.ui.text(&name(g.item), x, y, fs, WHITE, true);
                    let clean = 1.0 - g.damage as f32 / max_damage(g.item).max(1) as f32;
                    let by = y + 11.0 * s;
                    self.ui.solid(x, by, inner, 6.0 * s, rgba(0, 0, 0, 255));
                    let color = [(1.0 - clean) * 2.0, clean * 2.0, 0.2, 1.0].map(|v: f32| v.min(1.0));
                    self.ui
                        .solid(x + s, by + s, ((inner - 2.0 * s) * clean).max(0.0), 4.0 * s, color);
                    let pct = (clean * 100.0).round() as u32;
                    self.ui.text(
                        &tf("gun.cleanliness", &[&pct]),
                        x,
                        by + 9.0 * s,
                        fs,
                        rgba(210, 210, 215, 255),
                        true,
                    );
                }
            }
            BenchMode::Tune => {
                let gun = self.guns.bench.gun;
                let title = gun.map_or_else(|| t("gun.attachments").to_string(), |g| name(g.item));
                self.ui.text(&title, x, y, fs, WHITE, true);
                let mods = gun.map_or(0, |g| gun_mods(&g));
                let kind = gun.and_then(|g| GunKind::of(g.item));
                for (i, &(bit, item)) in ATTACHMENTS.iter().enumerate() {
                    let ly = y + (12.0 + i as f32 * 10.0) * s;
                    draw_stack(&mut self.ui, x, ly - 0.5 * s, 8.0 * s, &Stack::one(item));
                    let (text, color) = if kind.is_some_and(|k| !k.fits(bit)) {
                        ("-".to_string(), rgba(150, 150, 158, 255))
                    } else if mods & bit != 0 {
                        ("OK".to_string(), rgba(120, 230, 140, 255))
                    } else if self.creative() {
                        (String::new(), WHITE)
                    } else {
                        let n = self.inventory.count(item);
                        let c = if n > 0 { WHITE } else { rgba(255, 120, 100, 255) };
                        (format!("x{n}"), c)
                    };
                    let label = name(item);
                    self.ui.text(&label, x + 10.0 * s, ly + s, fs, rgba(210, 210, 215, 255), true);
                    let tw = self.ui.text_width(&text, fs);
                    self.ui.text(&text, x + inner - tw, ly + s, fs, color, true);
                }
            }
        }
        self.ui.hit(x0, y0, pw, ph)
    }

    fn set_bench_mode(&mut self, mode: BenchMode) {
        // Parts already put in go back when switching away from assembling; the gun on the
        // table stays for cleaning and tuning, and goes back to the inventory to assemble.
        let parts = self.guns.bench.parts();
        self.guns.bench.reset_assembly();
        for st in parts {
            self.give(st);
        }
        self.guns.bench.mode = mode;
        if mode == BenchMode::Assemble {
            self.take_gun_off_bench();
        } else {
            self.lay_gun_on_bench(0.0);
        }
    }

    /// A click on an attachment (on the gun, or lying beside it): the hand takes it off into
    /// the inventory, or fits it on (from the cursor or the inventory).
    pub(super) fn click_gun_mod(&mut self, i: usize, _shift: bool) {
        let (bit, item) = ATTACHMENTS[i];
        if self.guns.bench.fit.is_some() {
            return;
        }
        let Some(mut gun) = self.guns.bench.gun else {
            return;
        };
        let Some(kind) = GunKind::of(gun.item).filter(|k| k.fits(bit)) else {
            return;
        };
        let mods = gun_mods(&gun);
        let removing = mods & bit != 0;
        if removing {
            set_gun_mods(&mut gun, mods & !bit);
            // Off with the extended magazine: the rounds that no longer fit come out.
            let size = kind.magazine_size(mods & !bit);
            let extra = gun_rounds(&gun).saturating_sub(size);
            if extra > 0 {
                set_gun_rounds(&mut gun, size);
                self.give(Stack::new(kind.ammo(), extra));
            }
            if !self.creative() {
                self.give(Stack::one(item));
            }
        } else if self.cursor.is_some_and(|c| c.item == item) {
            take(&mut self.cursor, 1);
            set_gun_mods(&mut gun, mods | bit);
        } else if self.creative() || self.inventory.remove_one(item) {
            set_gun_mods(&mut gun, mods | bit);
        } else {
            return;
        }
        let bench = &mut self.guns.bench;
        bench.gun = Some(gun);
        bench.fit = Some(FitAnim {
            index: i,
            removing,
            start: self.time,
        });
    }

    /// Whether the player has what a part is made of (its item, or its materials).
    fn can_make(&self, part: &crate::item::Part) -> bool {
        self.creative()
            || match part.item {
                Some(item) => self.inventory.count(item) > 0,
                None => part
                    .cost
                    .iter()
                    .all(|&(item, n)| self.inventory.count(item) >= n as u32),
            }
    }

    /// Puts the next part in: the pistol's from its part item, the others made from their
    /// materials (free in creative). The last one finishes the gun, which goes into the
    /// inventory.
    fn install_next_part(&mut self, p: IVec3) {
        let n = self.guns.bench.installed;
        let kind = self.guns.bench.kind;
        let since = self.time - self.guns.bench.installed_at;
        if n >= PARTS || since < INSTALL_TIME {
            return;
        }
        let part = &kind.parts()[n];
        if !self.can_make(part) {
            self.guns.bench.missing_at = self.time;
            return;
        }
        let taken = !self.creative();
        if taken {
            match part.item {
                Some(item) => {
                    self.inventory.remove_one(item);
                }
                None => {
                    for &(item, count) in part.cost {
                        for _ in 0..count {
                            self.inventory.remove_one(item);
                        }
                    }
                }
            }
        }
        let bench = &mut self.guns.bench;
        bench.taken[n] = taken;
        bench.installed = n + 1;
        bench.installed_at = self.time;
        if bench.installed == PARTS {
            bench.finished = true;
            bench.taken = [false; PARTS];
            self.give(Stack::one(kind.item()));
            // Done: sparks fly off the finished gun.
            let at = Table::new(p, self.table_side(p)).at(0.0, FLOAT_HEIGHT, -0.14);
            self.particles.sparks(at, Vec3::Y, 16);
        }
    }

    /// Holding the left mouse button on a part scrubs it clean (at `at`, where the brush is).
    fn scrub(&mut self, part: Option<usize>, at: Option<Vec3>) {
        let dt = self.ui.dt;
        let bench = &mut self.guns.bench;
        let Some(gun) = bench.gun else {
            bench.dirt_of = None;
            bench.brush = 0.0;
            return;
        };
        let dirt_max = max_damage(gun.item).max(1);
        if bench.dirt_of != Some((gun.item, gun.damage)) {
            // A gun was put in: all its parts are as dirty as it is.
            let d = gun.damage as f32 / dirt_max as f32;
            bench.dirt = [d; PARTS];
            bench.dirt_of = Some((gun.item, gun.damage));
        }
        let working = part
            .zip(at)
            .filter(|&(p, _)| self.left_down && bench.dirt[p] > 0.0);
        bench.brush = (bench.brush + if working.is_some() { dt / 0.15 } else { -dt / 0.3 }).clamp(0.0, 1.0);
        let Some((p, at)) = working else {
            return;
        };
        bench.scrub_point = at;
        bench.dirt[p] = (bench.dirt[p] - dt / SCRUB_TIME).max(0.0);
        let mean = bench.dirt.iter().sum::<f32>() / PARTS as f32;
        let damage = (mean * dirt_max as f32).ceil() as u16;
        if let Some(g) = &mut bench.gun {
            g.damage = damage;
        }
        bench.dirt_of = Some((gun.item, damage));
        // Foam where the brush works.
        if self.rng.next() < dt * 25.0 && bench.brush > 0.8 {
            let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y * 0.2);
            let jitter = Vec3::new(self.rng.next() - 0.5, 0.0, self.rng.next() - 0.5) * 0.06;
            self.particles
                .smoke_shaded(at + jitter + Vec3::Y * 0.02, 245, sky, blk);
        }
    }

    /// Assembling: which gun, with arrows to pick another (what was put in goes back).
    fn gun_selector(&mut self, x0: f32, y0: f32, width: f32) {
        let s = self.ui.s;
        let (bw, bh) = (10.0 * s, 10.0 * s);
        let current = GUN_KINDS.iter().position(|&k| k == self.guns.bench.kind).unwrap_or(0);
        let mut next = None;
        if self.ui.button("<", x0, y0, bw, bh, true) {
            next = Some((current + GUN_KINDS.len() - 1) % GUN_KINDS.len());
        }
        if self.ui.button(">", x0 + width - bw, y0, bw, bh, true) {
            next = Some((current + 1) % GUN_KINDS.len());
        }
        let fs = (s * 0.75).round().max(1.0);
        let title = name(self.guns.bench.kind.item());
        self.ui
            .text_centered(&title, x0 + width * 0.5, y0 + 2.5 * s, fs, WHITE, true);
        if let Some(i) = next {
            let parts = self.guns.bench.parts();
            self.guns.bench.reset_assembly();
            for st in parts {
                self.give(st);
            }
            self.guns.bench.kind = GUN_KINDS[i];
        }
    }

    /// Assembling: the parts in order, with what each takes (the pistol's parts are items of
    /// their own, the others are made from materials).
    fn parts_list(&mut self, x0: f32, y0: f32, width: f32, p: IVec3) {
        let s = self.ui.s;
        let fs = (s * 0.75).round().max(1.0);
        let bench = &self.guns.bench;
        let (kind, installed, finished, missing_at) =
            (bench.kind, bench.installed, bench.finished, bench.missing_at);
        let creative = self.creative();
        for (i, part) in kind.parts().iter().enumerate() {
            let y = y0 + i as f32 * 10.0 * s;
            let done = i < installed || finished;
            let current = i == installed && !finished;
            let ready = self.can_make(part);
            if current {
                self.ui
                    .solid(x0 - s, y - s, width + 2.0 * s, 10.0 * s, rgba(255, 255, 255, 40));
            }
            let flash = current && self.time - missing_at < 0.6;
            let color = if done {
                rgba(120, 230, 140, 255)
            } else if flash || (current && !ready) {
                rgba(255, 120, 100, 255)
            } else {
                rgba(215, 215, 220, 255)
            };
            self.ui.text(t(part.name), x0, y + 1.0 * s, fs, color, true);
            // What it takes, right-aligned: the part item, or the materials with their counts.
            let mut x = x0 + width;
            if done {
                let w = self.ui.text_width("OK", fs);
                self.ui.text("OK", x - w, y + 1.0 * s, fs, color, true);
            } else if !creative {
                let needs: Vec<(ItemId, u8)> = match part.item {
                    Some(item) => vec![(item, 1)],
                    None => part.cost.to_vec(),
                };
                for &(item, n) in needs.iter().rev() {
                    let have = self.inventory.count(item) >= n as u32;
                    let num = format!("{n}");
                    let w = self.ui.text_width(&num, fs);
                    x -= w;
                    let c = if have { color } else { rgba(255, 120, 100, 255) };
                    self.ui.text(&num, x, y + 1.0 * s, fs, c, true);
                    x -= 9.0 * s;
                    draw_stack(&mut self.ui, x, y - 0.5 * s, 8.0 * s, &Stack::one(item));
                    if self.ui.hit(x, y, 8.0 * s, 8.0 * s) {
                        self.ui.set_tooltip(&name(item));
                    }
                    x -= 2.0 * s;
                }
            }
            // Clicking the current part's line puts it in too.
            if current && self.ui.pressed && self.ui.hit(x0 - s, y - s, width + 2.0 * s, 10.0 * s) {
                self.install_next_part(p);
            }
        }
    }
}

/// What lies on the open gun station's table (the gun's kind and the pieces), and the
/// hand at work. `cam` is needed for what the hand holds while it comes into view.
fn bench_pieces(
bench: &Bench,
time: f32,
table: &Table,
creative: bool,
inventory: &Inventory,
cam: Cam,
) -> Option<(GunKind, Vec<Piece>, Option<Hand>)> {
    let mut pieces = Vec::new();
    let mut hand = None;
    if bench.mode == BenchMode::Assemble {
        let kind = bench.kind;
        let lay = layout(kind);
        let mods = kind.shown_mods(0);
        // The gun floats over the back of the table, turning; a finished one spins.
        let sway = if bench.turned { 0.0 } else { (time * 0.6).sin() * 0.5 };
        let spin = if bench.finished { (time - bench.installed_at) * 1.6 } else { 0.0 };
        let float = Place {
            pos: table.at(0.0, FLOAT_HEIGHT + (time * 2.0).sin() * 0.012, -0.14),
            rot: table.rot * Quat::from_rotation_y(bench.yaw + sway + spin),
            scale: lay.scale * FLOAT_SCALE,
        };
        // The parts not in yet lie apart on the front of the table.
        let apart = Place {
            pos: table.at(0.0, lay.apart_lift, 0.12),
            rot: table.lying(),
            scale: lay.scale,
        };
        let n = bench.installed;
        for part in 0..PARTS {
            let (lo, hi) = gun::part_bounds(kind, part, mods);
            if lo.x > hi.x {
                continue;
            }
            let pc = (lo + hi) * 0.5;
            let rest = Place {
                pos: apart.point(lay.apart, pc + gun::exploded_offset(kind, part)),
                ..apart
            };
            let fixed = Place {
                pos: float.point(lay.center, pc),
                ..float
            };
            let near = Place {
                pos: float.point(lay.center, pc + gun::assembly_offset(kind, part)),
                ..float
            };
            let place = if part + 1 == n && time - bench.installed_at < INSTALL_TIME {
                // Flying up from the table, then sliding into its place.
                let k = (time - bench.installed_at) / INSTALL_TIME;
                if k < 0.6 {
                    rest.toward(near, ease(k / 0.6), 0.12)
                } else {
                    near.toward(fixed, ease((k - 0.6) / 0.4), 0.0)
                }
            } else if part < n {
                fixed
            } else if part == n && !bench.finished {
                // The next part hovers a little over the table.
                rest.moved(Vec3::Y * (0.02 + 0.012 * (time * 3.0).sin()))
            } else {
                rest
            };
            let (pick, tint) = if bench.finished {
                (Some(Pick::Gun), [255; 3])
            } else if part == n {
                let p = &kind.parts()[part];
                let have = creative
                    || match p.item {
                        Some(item) => inventory.count(item) > 0,
                        None => p.cost.iter().all(|&(i, c)| inventory.count(i) >= c as u32),
                    };
                let tint = if time - bench.missing_at < 0.6 {
                    [255, 110, 100]
                } else if have {
                    lerp_rgb([225, 225, 225], [255, 255, 190], 0.5 + 0.5 * (time * 5.0).sin())
                } else {
                    [95, 95, 105]
                };
                (Some(Pick::Part(part)), tint)
            } else if part > n {
                (None, [150, 150, 158])
            } else {
                (None, [255; 3])
            };
            pieces.push(Piece {
                pick,
                sel: Sel::parts(1 << part, mods),
                m: place.matrix(pc),
                tint,
            });
        }
        return Some((kind, pieces, None));
    }

    let gun = bench.gun?;
    let kind = GunKind::of(gun.item)?;
    let lay = layout(kind);
    let mods = gun_mods(&gun);
    let put = time - bench.placed_at;
    let spec = gun::spec(kind);
    let rest_hand = cam.hand_rest();
    // The hand lays the gun down from in front of the camera, then lets go of it.
    let lying = Place {
        pos: table.at(0.0, lay.lift, if bench.mode == BenchMode::Tune { -0.12 } else { 0.0 }),
        rot: table.lying(),
        scale: lay.scale,
    };
    let placed = cam.holding(lay.scale).toward(lying, ease(put / PUT_TIME), 0.12);
    if put < PUT_TIME + LET_GO {
        let grip = placed.point(lay.center, spec.grip);
        let k = ease((put - PUT_TIME) / LET_GO);
        hand = Some(Hand {
            fist: grip.lerp(rest_hand, k),
            brush: false,
        });
    }

    if bench.mode == BenchMode::Clean {
        // Taken apart once it is down: the parts slide apart, tinted by their dirt.
        let apart = Place {
            pos: table.at(0.0, lay.apart_lift, 0.0),
            rot: table.lying(),
            scale: lay.scale,
        };
        let spread = ease((put - PUT_TIME) / SPREAD_TIME);
        let shown = kind.shown_mods(mods);
        for part in 0..PARTS {
            let (lo, hi) = gun::part_bounds(kind, part, shown);
            if lo.x > hi.x {
                continue;
            }
            let pc = (lo + hi) * 0.5;
            let together = Place {
                pos: placed.point(lay.center, pc),
                ..placed
            };
            let laid = Place {
                pos: apart.point(lay.apart, pc + gun::exploded_offset(kind, part)),
                ..apart
            };
            let dirt = bench.dirt[part];
            pieces.push(Piece {
                pick: (spread >= 1.0).then_some(Pick::Part(part)),
                sel: Sel::parts(1 << part, shown),
                m: together.toward(laid, spread, 0.0).matrix(pc),
                tint: lerp_rgb([255; 3], [175, 135, 90], dirt),
            });
        }
        if bench.brush > 0.0 {
            let wiggle = table.right * (time * 28.0).sin() * 0.02 * bench.brush;
            let at = bench.scrub_point + Vec3::Y * 0.11 + flat(cam.shoulder() - bench.scrub_point) * BRUSH_AHEAD + wiggle;
            hand = Some(Hand {
                fist: rest_hand.lerp(at, ease(bench.brush)),
                brush: true,
            });
        }
        return Some((kind, pieces, hand));
    }

    // Tuning: the gun lies at the back, the attachments the player has in a row in front.
    let down = put >= PUT_TIME;
    let moving = bench.fit.map(|f| ATTACHMENTS[f.index].0);
    let fitted = kind.shown_mods(mods) & !moving.unwrap_or(0);
    let removable = ATTACHMENTS
        .iter()
        .map(|a| a.0)
        .filter(|&bit| kind.fits(bit))
        .fold(0, |m, bit| m | bit);
    pieces.push(Piece {
        pick: down.then_some(Pick::Gun),
        sel: Sel {
            skip: removable,
            ..Sel::parts(0xff, fitted)
        },
        m: placed.matrix(lay.center),
        tint: [255; 3],
    });
    for (i, &(bit, item)) in ATTACHMENTS.iter().enumerate() {
        if !kind.fits(bit) {
            continue;
        }
        let (alo, ahi) = gun::attachment_bounds(kind, bit);
        let ac = (alo + ahi) * 0.5;
        let small = lay.scale.min(0.19 / (ahi - alo).x.max(0.01));
        let rest = Place {
            pos: table.at(-0.33 + 0.22 * i as f32, (ac.z - alo.z) * small + 0.003, 0.3),
            rot: table.lying(),
            scale: small,
        };
        let on = Place {
            pos: placed.point(lay.center, ac),
            ..placed
        };
        let near = on.moved(placed.rot * approach(kind, bit) * 0.14);
        let anim = bench.fit.filter(|f| f.index == i);
        let mut spare = if creative { 1 } else { inventory.count(item) };
        if let Some(f) = anim {
            let t = (time - f.start).min(FIT_TIME);
            let u = if f.removing { FIT_TIME - t } else { t };
            let (at, fist) = fit_pose(u, bit, rest, near, on, rest_hand);
            pieces.push(Piece {
                pick: None,
                sel: Sel::attachment(bit),
                m: at.matrix(ac),
                tint: [255; 3],
            });
            hand = Some(Hand { fist, brush: false });
            if f.removing && !creative {
                // It is in the inventory already, but still in the hand.
                spare = spare.saturating_sub(1);
            }
        } else if mods & bit != 0 {
            pieces.push(Piece {
                pick: down.then_some(Pick::Attachment(i)),
                sel: Sel::attachment(bit),
                m: on.matrix(ac),
                tint: [255; 3],
            });
        }
        if spare > 0 {
            pieces.push(Piece {
                pick: (down && bench.fit.is_none() && mods & bit == 0).then_some(Pick::Attachment(i)),
                sel: Sel::attachment(bit),
                m: rest.matrix(ac),
                tint: if mods & bit != 0 { [150, 150, 158] } else { [255; 3] },
            });
        }
    }
    Some((kind, pieces, hand))
}


/// Draws the pieces (the one under the mouse, `hover`, lit up).
fn emit_pieces(out: &mut Vec<Vertex>, kind: GunKind, pieces: &[Piece], hover: Option<Pick>, light: [u8; 4]) {
    for pc in pieces {
        let lit = pc.pick.is_some() && pc.pick == hover;
        let tint = if lit {
            lerp_rgb(pc.tint, [255, 255, 255], 0.45)
        } else {
            pc.tint
        };
        for b in gun::boxes(kind).iter().filter(|b| pc.sel.picks(b)) {
            let c = mul_rgb(b.tint, tint);
            let c = if lit { lerp_rgb(c, [255; 3], 0.25) } else { c };
            emit_box(
                out,
                pc.m * b.transform(),
                b.min,
                b.max,
                [b.layer; 6],
                [c; 6],
                light,
                flags::ENTITY,
            );
        }
    }
}

/// The arm reaching in to the fist along `reach` (holding the brush while scrubbing).
fn emit_arm(out: &mut Vec<Vertex>, shoulder: Vec3, hand: &Hand, skin: u8, light: [u8; 4], time: f32) {
    let fist = hand.fist;
    // World size of a model pixel of the arm; the arm runs from the fist (y = -1) to the
    // shoulder out of view, the sleeve there.
    let px = 0.026;
    let reach = (shoulder - fist).normalize_or(Vec3::Y);
    let len = (fist.distance(shoulder) + 0.1) / px;
    let rot = Quat::from_rotation_arc(Vec3::Y, reach);
    let m = Mat4::from_scale_rotation_translation(Vec3::splat(px), rot, fist);
    emit_box(
        out,
        m,
        Vec3::new(-2.0, -1.0, -2.0),
        Vec3::new(2.0, len, 2.0),
        ARM_LAYERS.map(|layer| skin_layer(layer, skin)),
        [[255; 3]; 6],
        light,
        flags::ENTITY,
    );
    if hand.brush {
        // A scrubbing brush: a wooden handle and white bristles under it.
        let turn = Quat::from_rotation_y((time * 28.0).sin() * 0.25);
        let b = Mat4::from_rotation_translation(turn, fist - flat(reach) * BRUSH_AHEAD);
        let wood = [tex::GUN_WOOD; 6];
        emit_box(out, b, Vec3::new(-0.07, -0.08, -0.03), Vec3::new(0.07, -0.045, 0.03), wood, [[200, 150, 100]; 6], light, flags::ENTITY);
        emit_box(out, b, Vec3::new(-0.065, -0.11, -0.026), Vec3::new(0.065, -0.08, 0.026), [tex::WOOL; 6], [[240, 236, 220]; 6], light, flags::ENTITY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_fits_on_the_table() {
        for kind in GUN_KINDS {
            let lay = layout(kind);
            let table = Table::new(IVec3::ZERO, 2);
            let mods = kind.shown_mods(0);
            let (lo, hi) = gun::bounds_where(kind, |b| b.shown(mods));
            let lying = Place {
                pos: table.at(0.0, lay.lift, 0.0),
                rot: table.lying(),
                scale: lay.scale,
            };
            for c in [lo, hi, Vec3::new(lo.x, hi.y, lo.z), Vec3::new(hi.x, lo.y, hi.z)] {
                let q = lying.point(lay.center, c) - table.top;
                assert!(q.x.abs() < 0.5 && q.z.abs() < 0.5, "{kind:?}: {q}");
                assert!(q.y > -1e-3, "{kind:?} sinks into the table: {q}");
            }
            // Lying on its side: the muzzle to the right, the top away from the player.
            let muzzle = lying.point(lay.center, gun::spec(kind).muzzle) - table.top;
            assert!(muzzle.dot(table.right) > 0.1, "{kind:?}: {muzzle}");
        }
    }

    #[test]
    fn a_fitted_attachment_ends_where_it_sits_on_the_gun() {
        let rest = Place {
            pos: Vec3::new(0.0, 1.0, 0.3),
            rot: Quat::IDENTITY,
            scale: 0.01,
        };
        let on = Place {
            pos: Vec3::new(0.1, 1.05, -0.1),
            rot: Quat::from_rotation_y(0.4),
            scale: 0.02,
        };
        let near = on.moved(Vec3::Y * 0.14);
        let hand = Vec3::new(1.0, 2.0, 1.0);
        let (at, fist) = fit_pose(FIT_TIME, gun_mod::SCOPE, rest, near, on, hand);
        assert!(at.pos.distance(on.pos) < 1e-4 && fist.distance(hand) < 1e-4);
        let (at, fist) = fit_pose(0.0, gun_mod::SCOPE, rest, near, on, hand);
        assert!(at.pos.distance(rest.pos) < 1e-4 && fist.distance(hand) < 1e-4);
        // In the middle the fist holds it.
        let (at, fist) = fit_pose(0.6, gun_mod::SCOPE, rest, near, on, hand);
        assert!(fist.distance(at.pos + GRAB) < 1e-4);
    }
}
