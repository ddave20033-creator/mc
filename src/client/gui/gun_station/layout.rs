//! Where things lie on the table: each all on it and none in another (what is laid where
//! something already lies goes beside it), a gun's parts as it comes apart, a magazine beside
//! its gun.

use super::anim::{MAG_OUT, magazine_pose};
use super::pieces::{Piece, bounds, lying_pieces, lying_root, mag_rig, pivot_bones, rig_of, strip_length};
use super::table::{HALF_D, Table};
use crate::entity::{BenchItem, GunBench};
use crate::item::*;
use crate::model::gun::MAGAZINE;
use crate::model::pistol_view::bench::self as rig;
use crate::model::pistol_view::self as pv;
use crate::model::revolver_view::bench as rrig;
use glam::{Vec2, Vec3};

/// Across the table, how wide the strips are that a thing's shape is made of (`shape`).
const STRIP: f32 = 0.1;
/// Longer than this across, a thing lies on the table as its strips (a long gun and its long
/// parts); anything shorter as its whole outline.
const LONG: f32 = 0.8;

/// Where a box of rounds lying flat reaches from its middle (across, toward the front), and
/// anything else lying flat.
fn flat_half(stack: &Stack) -> Vec2 {
    if stack.item == AMMO_BOX {
        let size = crate::model::gun_station::ammo_box_size() / 16.0;
        Vec2::new(size.x, size.z) * 0.5
    } else {
        Vec2::splat(0.1)
    }
}

/// Where pieces reach on the table together (across, toward the front).
fn extent_of<'a>(table: &Table, pieces: impl IntoIterator<Item = &'a Piece>) -> (Vec2, Vec2) {
    pieces
        .into_iter()
        .map(|pc| pc.extent(table))
        .fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)))
}

/// How far (across, toward the front) something reaching from `lo` to `hi` must move to be
/// on the table (to its middle, if it is bigger than the table).
fn onto_table(table: &Table, lo: Vec2, hi: Vec2) -> Vec2 {
    let fit = |lo: f32, hi: f32, half: f32| {
        if hi - lo > 2.0 * half {
            -(lo + hi) * 0.5
        } else {
            (-half - lo).max(0.0) + (half - hi).min(0.0)
        }
    };
    Vec2::new(fit(lo.x, hi.x, table.half_w), fit(lo.y, hi.y, HALF_D))
}

/// Where a stack laid at (x, z) ends up so that all of it is on the table.
pub(super) fn fit_spot(table: &Table, stack: Stack, x: f32, z: f32, turn: f32) -> (f32, f32) {
    let it = BenchItem { id: 0, stack, x, z, turn };
    let Some(pieces) = lying_pieces(table, &it) else {
        let half = flat_half(&stack);
        return (x.clamp(-table.half_w + half.x, table.half_w - half.x), z.clamp(-HALF_D + half.y, HALF_D - half.y));
    };
    let (lo, hi) = extent_of(table, &pieces);
    let d = onto_table(table, lo, hi);
    (x + d.x, z + d.y)
}

/// Where a thing lying at (0, 0) reaches on the table (across, toward the front).
pub(super) fn footprint(table: &Table, stack: Stack, turn: f32) -> (Vec2, Vec2) {
    let it = BenchItem { id: 0, stack, x: 0.0, z: 0.0, turn };
    match lying_pieces(table, &it) {
        Some(pieces) => extent_of(table, &pieces),
        None => {
            let half = flat_half(&stack);
            (-half, half)
        }
    }
}

/// The shape of a thing lying at (0, 0) on the table: for a long one, strips across it, each as
/// deep as the thing is there (a long gun is deep only where its stock, grip or magazine is,
/// not along its barrel), so other things may lie beside its thin parts.
pub(super) fn shape(table: &Table, stack: Stack, turn: f32) -> Vec<(Vec2, Vec2)> {
    let it = BenchItem { id: 0, stack, x: 0.0, z: 0.0, turn };
    let Some(pieces) = lying_pieces(table, &it) else {
        return vec![footprint(table, stack, turn)];
    };
    // Every cube's reach on the table.
    let mut cubes = Vec::new();
    for pc in &pieces {
        for c in pc.visible() {
            let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
            for q in super::pieces::corners(c, pc.mats[c.bone]) {
                let (x, z) = table.local(q);
                lo = lo.min(Vec2::new(x, z));
                hi = hi.max(Vec2::new(x, z));
            }
            cubes.push((lo, hi));
        }
    }
    let (lo, hi) = cubes.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(a, b), (c, d)| (a.min(*c), b.max(*d)));
    if cubes.is_empty() {
        return vec![(Vec2::splat(-0.1), Vec2::splat(0.1))];
    }
    // Something short keeps its whole outline (things slide round it smoothly).
    if hi.x - lo.x < LONG {
        return vec![(lo, hi)];
    }
    let n = ((hi.x - lo.x) / STRIP).ceil().clamp(1.0, 32.0) as usize;
    let w = (hi.x - lo.x) / n as f32;
    (0..n)
        .filter_map(|k| {
            let (x0, x1) = (lo.x + w * k as f32, lo.x + w * (k + 1) as f32);
            let (z0, z1) = cubes
                .iter()
                .filter(|(c, d)| c.x < x1 && d.x > x0)
                .fold((f32::MAX, f32::MIN), |(a, b), (c, d)| (a.min(c.y), b.max(d.y)));
            (z0 <= z1).then(|| (Vec2::new(x0, z0), Vec2::new(x1, z1)))
        })
        .collect()
}

/// Where everything lying on the table reaches (across, toward the front): the strips of each
/// thing's shape.
pub(super) fn occupied(table: &Table, bench: &GunBench) -> Vec<(Vec2, Vec2)> {
    let mut out = Vec::new();
    for it in &bench.items {
        let at = Vec2::new(it.x, it.z);
        out.extend(shape(table, it.stack, it.turn).into_iter().map(|(lo, hi)| (lo + at, hi + at)));
    }
    out
}

/// The first of `own` (moved by `at`) that is in one of `taken`, and that one.
fn overlap(own: &[(Vec2, Vec2)], at: Vec2, taken: &[(Vec2, Vec2)], gap: f32) -> Option<((Vec2, Vec2), (Vec2, Vec2))> {
    own.iter().find_map(|&(lo, hi)| {
        let (a, b) = (lo + at, hi + at);
        taken
            .iter()
            .find(|(c, d)| !(b.x + gap <= c.x || d.x + gap <= a.x || b.y + gap <= c.y || d.y + gap <= a.y))
            .map(|&t| ((a, b), t))
    })
}

/// Whether something reaching from `a` to `b` is on the table (a hair over its edge too).
fn inside(table: &Table, a: Vec2, b: Vec2) -> bool {
    a.x >= -table.half_w - 1e-3 && b.x <= table.half_w + 1e-3 && a.y >= -HALF_D - 1e-3 && b.y <= HALF_D + 1e-3
}

/// Whether `stack` lying at (x, z) is on the table and in nothing already lying there.
pub(super) fn is_free(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32) -> bool {
    let (lo, hi) = footprint(table, stack, turn);
    let at = Vec2::new(x, z);
    let gap = 0.01 - 1e-3;
    inside(table, lo + at, hi + at) && overlap(&shape(table, stack, turn), at, &occupied(table, bench), gap).is_none()
}

/// The nearest place to (x, z) where `stack` lies on the table without being in anything
/// already lying there (it stays where it was asked for when there is no such place).
pub(super) fn free_spot(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32) -> (f32, f32) {
    free_spot_near(table, bench, stack, x, z, turn, None)
}

/// `free_spot` for something held on the mouse: pushed out of what is in its way on the same
/// side as it was a moment ago (`prev`) while that side is not much further, so it slides on
/// round it with the mouse instead of flipping between two ways round.
pub(super) fn free_spot_near(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32, prev: Option<(f32, f32)>) -> (f32, f32) {
    let (lo, hi) = footprint(table, stack, turn);
    let own = shape(table, stack, turn);
    let taken = occupied(table, bench);
    let gap = 0.01;
    let fits = |x: f32, z: f32| {
        let at = Vec2::new(x, z);
        inside(table, lo + at, hi + at) && overlap(&own, at, &taken, gap).is_none()
    };
    let (mut x, mut z) = fit_spot(table, stack, x, z, turn);
    if fits(x, z) {
        return (x, z);
    }
    // Pushed out of whatever it is in, the shortest way (so it slides along the edges of
    // things as the mouse moves, without jumping about), and kept on the table.
    for _ in 0..12 {
        // (the part of it that is in something, pushed out of that; kept on the table whole)
        let Some(((a, b), (c, d))) = overlap(&own, Vec2::new(x, z), &taken, gap) else {
            break;
        };
        let (whole_a, whole_b) = (lo + Vec2::new(x, z), hi + Vec2::new(x, z));
        let pushes = [
            Vec2::new(c.x - gap - b.x, 0.0),
            Vec2::new(d.x + gap - a.x, 0.0),
            Vec2::new(0.0, c.y - gap - b.y),
            Vec2::new(0.0, d.y + gap - a.y),
        ];
        // The shortest way out; or, near enough to it, the one toward where it was.
        let cost = |v: &Vec2| {
            let side = prev.map_or(0.0, |q| {
                let d = Vec2::new(q.0 - x, q.1 - z);
                if d.dot(*v) > 0.0 { -0.12 } else { 0.0 }
            });
            v.length() + side
        };
        let Some(v) = pushes
            .iter()
            .filter(|v| inside(table, whole_a + **v, whole_b + **v))
            .min_by(|u, v| cost(u).total_cmp(&cost(v)))
        else {
            break;
        };
        x += v.x + v.x.signum() * 1e-4;
        z += v.y + v.y.signum() * 1e-4;
    }
    if fits(x, z) {
        return (x, z);
    }
    // Rings further and further out, the nearest free place first.
    let mut r = 0.02;
    while r < 2.0 {
        let n = ((r * 60.0) as usize).max(8);
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let (cx, cz) = (x + a.cos() * r, z + a.sin() * r);
            if fits(cx, cz) {
                return (cx, cz);
            }
        }
        r += 0.02;
    }
    (x, z)
}

/// A gun taken apart at `gun` (as it lay): where its parts end up, as they will lie, each
/// with what it is made into (its stack, with the attachment on it and the gun's dirt); how
/// far the whole spread moves to be on the table (world); and the live rounds that go back to
/// the player (from the pistol's chamber when it did not fit back into its magazine, the
/// revolver's cylinder).
pub(super) fn strip_targets(table: &Table, gun: &BenchItem) -> (Vec3, Vec<(Stack, f32, f32, f32)>, u8) {
    let st = gun.stack;
    let mods = gun_mods(&st);
    let Some((kind, bones, rest)) = rig_of(&st, 0.0) else {
        return (Vec3::ZERO, Vec::new(), 0);
    };
    let root = lying_root(table, kind, bones, &rest, gun.x, gun.z, gun.turn);
    let (_, _, end) = rig_of(&emptied(&st), strip_length(kind)).unwrap_or((kind, bones, rest));
    let mut parts: Vec<(Stack, Piece)> = table_parts(kind)
        .iter()
        .map(|&p| {
            let mut stack = Stack { damage: st.damage, ..Stack::one(kind.parts()[p]) };
            // The attachments stay on their parts (kept in the parts' data, like a gun's).
            set_gun_mods(&mut stack, mods & attachments_on(p));
            let own = match mag_rig(kind) {
                Some(r) => rig::part(r, p, mods) & !rig::round(r),
                None => rrig::part(p),
            };
            (stack, Piece::new(kind, own, &end, root, 0, None))
        })
        .collect();
    // The magazine too, the round from the chamber back in it if there is room (else it goes
    // back to the player).
    let mut loose = if kind.uses_magazine() { gun_chambered(&st) as u8 } else { gun_rounds(&st) };
    if kind.uses_magazine() && gun_has_mag(&st) {
        let (mag, round_in) = magazine_out_of(&st);
        if round_in {
            loose = 0;
        }
        parts.push((mag, Piece::new(kind, rig::part(pv::rig(kind), MAGAZINE, mods), &end, root, 0, None)));
    }
    // Where each will lie: the middle it is placed by (`lying_root`), as the gun's pose has it.
    let spot = |pc: &Piece| {
        let (lo, hi) = bounds(kind, pivot_bones(kind, pc.bones), &end);
        root.transform_point3((lo + hi) * 0.5)
    };
    // Moved as a whole to be on the table.
    let (lo, hi) = extent_of(table, parts.iter().map(|(_, pc)| pc));
    let d = onto_table(table, lo, hi);
    let shift = table.right * d.x + table.toward * d.y;
    let out = parts
        .into_iter()
        .map(|(stack, pc)| {
            let (x, z) = table.local(spot(&pc) + shift);
            (stack, x, z, gun.turn)
        })
        .collect();
    (shift, out, loose)
}

/// Where a gun's magazine lies once it is out (beside the gun, on the table).
pub(super) fn magazine_spot(table: &Table, gun: &BenchItem) -> (f32, f32) {
    let Some((kind, bones, rest)) = rig_of(&gun.stack, 0.0) else { return (gun.x, gun.z) };
    let root = lying_root(table, kind, bones, &rest, gun.x, gun.z, gun.turn);
    let pose = magazine_pose(&gun.stack, MAG_OUT);
    let (lo, hi) = bounds(kind, rig::part(pv::rig(kind), MAGAZINE, gun_mods(&gun.stack)), &pose);
    let (x, z) = table.local(root.transform_point3((lo + hi) * 0.5));
    let (mag, _) = magazine_out_of(&gun.stack);
    fit_spot(table, mag, x, z, gun.turn)
}
