//! What lies on a table as it is drawn now: everything posed where it lies, and the last
//! change's animation (a gun coming apart or going together, an attachment going on or off,
//! a magazine going in or out, rounds pushed into a magazine) as far as it has got.

use super::layout::strip_targets;
use super::pieces::{Piece, first_piece, lying_pieces, lying_root, part_bones, rig_of, strip_length};
use super::table::Table;
use crate::entity::{BenchEvent, BenchItem, GunBench, bench_event};
use crate::item::*;
use crate::model::guns::gun::{MAGAZINE, PARTS};
use crate::model::guns::gun_view;
use crate::model::guns::pistol_view::bench::self as rig;
use crate::model::guns::pistol_view::self as pv;
use crate::model::guns::pistol_view::stack_dirt;
use crate::model::rig::viewmodel::{BonePose, find_bone};
use crate::util::smoothstep;
use glam::{IVec3, Mat4, Vec3};
use std::f32::consts::PI;
use std::rc::Rc;

/// Seconds for a piece to fly across the table (to the gun it goes on, or off it), and how
/// much faster than it comes apart a gun goes together.
pub(super) const FLY: f32 = 0.45;
const ASSEMBLE_SPEED: f32 = 1.6;
/// Seconds for each round to go into a magazine: brought over its lips, then pushed down in.
const ROUND_TIME: f32 = 0.34;
/// When the magazine is out in the pistol's strip animation (the table skips that part).
pub(super) const MAG_OUT: f32 = 0.5;

/// Ease in and out, 0..1.
fn ease(t: f32) -> f32 {
    smoothstep(0.0, 1.0, t)
}

/// Between two rigid (uniformly scaled) transforms, rising by `arc` halfway.
pub(super) fn blend(a: Mat4, b: Mat4, k: f32, arc: f32) -> Mat4 {
    let k = k.clamp(0.0, 1.0);
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(
        sa.lerp(sb, k),
        ra.slerp(rb, k),
        ta.lerp(tb, k) + Vec3::Y * arc * (k * PI).sin(),
    )
}

/// Where putting a gun together starts in its strip animation (played backwards): the
/// magazine goes in on its own, afterwards.
fn assemble_from(kind: GunKind) -> f32 {
    if kind.uses_magazine() {
        MAG_OUT
    } else {
        0.0
    }
}

/// Where the strip animation starts for a gun: with its magazine coming out, or after that
/// part of it when there is none (a revolver: from the start).
pub(super) fn strip_start(gun: &Stack) -> f32 {
    if cylinder_gun(gun.item) || gun_has_mag(gun) {
        0.0
    } else {
        MAG_OUT
    }
}

/// The pose of a gun with only its magazine `at` seconds into the strip animation (sliding
/// out of the grip, and laid beside it by the end of its part).
pub(super) fn magazine_pose(gun: &Stack, at: f32) -> Vec<BonePose> {
    let kind = GunKind::of(gun.item).unwrap_or(GunKind::Pistol);
    let mut when = [0.0; PARTS];
    when[MAGAZINE] = at.min(MAG_OUT);
    let has = gun_has_mag(gun);
    let mut pose = rig::pose(pv::rig(kind), when, gun_mods(gun), has && gun_rounds(gun) > 0, gun_chambered(gun));
    if let (Some(b), false) = (find_bone(gun_view::bones(kind), "magazine"), has) {
        pose[b].scale = Vec3::ZERO;
    }
    pose
}

/// How long an animation on the table takes.
pub(super) fn event_length(e: &BenchEvent) -> f32 {
    let fit = |bit: u8| rig::fit_anim(&pv::PISTOL, bit).map_or(0.4, |a| a.length);
    match e.kind {
        bench_event::STRIP => {
            let kind = e.gone.first().and_then(|g| GunKind::of(g.stack.item)).unwrap_or(GunKind::Pistol);
            strip_length(kind) - e.gone.first().map_or(MAG_OUT, |g| strip_start(&g.stack)) + 0.1
        }
        bench_event::ASSEMBLE => {
            let kind = e.gone.iter().find_map(|g| match bench_role(g.stack.item) {
                BenchRole::Part(k, _) => Some(k),
                _ => None,
            });
            let kind = kind.unwrap_or(GunKind::Pistol);
            FLY + (strip_length(kind) - assemble_from(kind)) / ASSEMBLE_SPEED
        }
        bench_event::MAG_OUT => MAG_OUT + 0.05,
        bench_event::MAG_IN => FLY + MAG_OUT,
        bench_event::FIT | bench_event::UNFIT => FLY + fit(e.bit),
        bench_event::LOAD => e.bit as f32 * ROUND_TIME,
        _ => 0.0,
    }
}

/// What lies on the table as it is drawn: the pieces of the guns, and the things drawn flat.
pub(super) type Scene = (Vec<Piece>, Vec<BenchItem>);

/// `scene` for a station seen every frame (drawn, or under the mouse): while nothing on it
/// is moving it is posed once and kept until what lies on it changes (each gun's pieces are
/// posed from its whole model, which is not cheap for several stations in sight).
pub(super) fn scene_at(p: IVec3, table: &Table, bench: &GunBench, t: Option<f32>) -> Rc<Scene> {
    use std::cell::RefCell;
    type Made = crate::world::FastMap<IVec3, (Vec<BenchItem>, Vec3, Rc<Scene>)>;
    thread_local! {
        static MADE: RefCell<Made> = RefCell::new(Default::default());
    }
    let e = &bench.event;
    let playing = t.is_some_and(|t| e.kind != bench_event::NONE && t < event_length(e));
    if playing {
        return Rc::new(scene(table, bench, t));
    }
    MADE.with_borrow_mut(|made| {
        if let Some((items, toward, s)) = made.get(&p) {
            if *items == bench.items && *toward == table.toward {
                return s.clone();
            }
        }
        if made.len() > 64 {
            // (stations of another world, or far away)
            made.clear();
        }
        let s = Rc::new(scene(table, bench, t));
        made.insert(p, (bench.items.clone(), table.toward, s.clone()));
        s
    })
}

/// What lies on the table as it is drawn now, with the last change `t` seconds into its
/// animation.
pub(super) fn scene(table: &Table, bench: &GunBench, t: Option<f32>) -> Scene {
    let e = &bench.event;
    let playing = t.filter(|&t| e.kind != bench_event::NONE && t < event_length(e));
    let hidden = |id: u16| {
        playing.is_some()
            && match e.kind {
                bench_event::STRIP | bench_event::UNFIT | bench_event::MAG_OUT => e.made.contains(&id),
                bench_event::ASSEMBLE => e.gun == id,
                _ => false,
            }
    };
    let mut pieces = Vec::new();
    let mut flats = Vec::new();
    for it in bench.items.iter().filter(|it| !hidden(it.id)) {
        let mut it = *it;
        if let (Some(t), true) = (playing, it.id == e.gun) {
            match e.kind {
                // The gun an attachment is going on (or coming off) is drawn without it
                // meanwhile.
                bench_event::FIT | bench_event::UNFIT => {
                    let m = gun_mods(&it.stack) & !e.bit;
                    set_gun_mods(&mut it.stack, m);
                }
                // A gun a magazine is going into is drawn without it meanwhile.
                bench_event::MAG_IN => it.stack = without_magazine(&it.stack),
                // A magazine being loaded shows the rounds in it so far.
                bench_event::LOAD => {
                    let done = (t / ROUND_TIME).floor() as u8;
                    let r = gun_rounds(&it.stack).saturating_sub(e.bit) + done.min(e.bit);
                    set_gun_rounds(&mut it.stack, r);
                }
                _ => {}
            }
        }
        match lying_pieces(table, &it) {
            Some(p) => pieces.extend(p),
            None => flats.push(it),
        }
    }
    if let Some(t) = playing {
        pieces.extend(animation(table, bench, t));
    }
    (pieces, flats)
}

/// The pieces moving in the last change's animation, `t` seconds into it.
pub(super) fn animation(table: &Table, bench: &GunBench, t: f32) -> Vec<Piece> {
    let e = &bench.event;
    let piece = match e.kind {
        bench_event::STRIP => strip(table, e, t),
        bench_event::ASSEMBLE => return assemble(table, bench, t),
        bench_event::FIT | bench_event::UNFIT => fit(table, bench, t),
        bench_event::MAG_OUT | bench_event::MAG_IN => magazine(table, bench, t),
        bench_event::LOAD => load(table, bench, t),
        _ => None,
    };
    piece.into_iter().collect()
}

/// A gun coming apart where it lay, and moving as a whole to where its parts will be on the
/// table.
fn strip(table: &Table, e: &BenchEvent, t: f32) -> Option<Piece> {
    let gun = e.gone.first()?;
    let st = gun.stack;
    let kind = GunKind::of(st.item)?;
    let (start, end) = (strip_start(&st), strip_length(kind));
    let at = (start + t).min(end);
    let ((_, bones, rest), (_, own, pose)) = (rig_of(&st, 0.0)?, rig_of(&emptied(&st), at)?);
    let (shift, _, _) = strip_targets(table, gun);
    let root = Mat4::from_translation(shift * ease(t / (end - start))) * lying_root(table, kind, bones, &rest, gun.x, gun.z, gun.turn);
    let mut piece = Piece::new(kind, own, &pose, root, stack_dirt(&st), None);
    piece.mag = (kind.uses_magazine() && gun_has_mag(&st))
        .then(|| magazine_out_of(&st).0)
        .map(|m| (gun_rounds(&m), magazine_capacity(m.item).unwrap_or(12)));
    Some(piece)
}

/// The parts flying from where they lay to the middle, and going together into the gun
/// (its strip animation backwards).
fn assemble(table: &Table, bench: &GunBench, t: f32) -> Vec<Piece> {
    let e = &bench.event;
    let mut out = Vec::new();
    let Some(gun) = bench.get(e.gun) else { return out };
    let Some(kind) = GunKind::of(gun.stack.item) else { return out };
    let (start, end) = (assemble_from(kind), strip_length(kind));
    let at = (end - (t - FLY).max(0.0) * ASSEMBLE_SPEED).clamp(start, end);
    // The gun they make, where it will lie, its parts as far apart as `at`.
    let (Some((_, bones, rest)), Some((_, _, pose))) = (rig_of(&gun.stack, 0.0), rig_of(&gun.stack, at)) else {
        return out;
    };
    let root = lying_root(table, kind, bones, &rest, gun.x, gun.z, gun.turn);
    for part in &e.gone {
        let BenchRole::Part(_, p) = bench_role(part.stack.item) else { continue };
        let there = Piece::new(kind, part_bones(kind, p, 0xff), &pose, root, stack_dirt(&part.stack), None);
        let mut piece = if t < FLY {
            // Flying from where it lay to the gun, and turning as it goes.
            match first_piece(table, part) {
                Some(from) => from.toward(&there.only(from.bones, None), ease(t / FLY), 0.15),
                None => there,
            }
        } else {
            there
        };
        piece.dirt = stack_dirt(&part.stack);
        out.push(piece);
    }
    out
}

/// An attachment flying to the gun and going on it, or coming off it and flying to where it
/// is laid.
fn fit(table: &Table, bench: &GunBench, t: f32) -> Option<Piece> {
    let e = &bench.event;
    let gun = bench.get(e.gun)?;
    let len = event_length(e) - FLY;
    let mut with = gun.stack;
    let m = gun_mods(&with) | e.bit;
    set_gun_mods(&mut with, m);
    let (kind, bones, mut pose) = rig_of(&with, 0.0)?;
    let root = lying_root(table, kind, bones, &pose, gun.x, gun.z, gun.turn);
    // On the gun: into place (backwards to come off), after flying there.
    let fitting = e.kind == bench_event::FIT;
    let u = if fitting { t - FLY } else { len - t };
    rig::add_fit(pv::rig(kind), &mut pose, e.bit, u.clamp(0.0, len));
    let on = Piece::new(kind, rig::attachment(pv::rig(kind), e.bit), &pose, root, 0, None);
    let lies = if fitting { e.gone.first().copied() } else { e.made.first().and_then(|&id| bench.get(id)).copied() };
    Some(match lies.and_then(|it| first_piece(table, &it)) {
        Some(off) if fitting && t < FLY => off.toward(&on, ease(t / FLY), 0.12),
        Some(off) if !fitting && t > len => on.toward(&off, ease((t - len) / FLY), 0.12),
        _ => on,
    })
}

/// The magazine sliding out of the grip and laid beside the gun, or flying to it and pushed
/// up into it.
fn magazine(table: &Table, bench: &GunBench, t: f32) -> Option<Piece> {
    let e = &bench.event;
    let now = bench.get(e.gun)?;
    let out = e.kind == bench_event::MAG_OUT;
    let (gun, mag) = if out {
        let g = e.gone.first()?;
        (g.stack, magazine_out_of(&g.stack).0)
    } else {
        (now.stack, e.gone.first()?.stack)
    };
    let (kind, bones, rest) = rig_of(&gun, 0.0)?;
    let root = lying_root(table, kind, bones, &rest, now.x, now.z, now.turn);
    let at = if out { t } else { MAG_OUT - (t - FLY).max(0.0) };
    let mut there = Piece::new(kind, rig::part(pv::rig(kind), MAGAZINE, gun_mods(&gun)), &magazine_pose(&gun, at), root, stack_dirt(&mag), None);
    there.mag = magazine_of(&mag);
    Some(match e.gone.first().and_then(|m| first_piece(table, m)) {
        Some(from) if !out && t < FLY => from.toward(&there.only(from.bones & there.bones, None), ease(t / FLY), 0.1),
        _ => there,
    })
}

/// The round going into a magazine now: from where the rounds were let go, over the
/// magazine's lips, then pushed down into it.
fn load(table: &Table, bench: &GunBench, t: f32) -> Option<Piece> {
    let e = &bench.event;
    let (m, from) = (bench.get(e.gun)?, e.gone.first()?);
    let done = (t / ROUND_TIME).floor() as u8;
    if done >= e.bit {
        return None;
    }
    let k = (t - done as f32 * ROUND_TIME) / ROUND_TIME;
    let mut shown = m.stack;
    set_gun_rounds(&mut shown, gun_rounds(&m.stack).saturating_sub(e.bit) + done + 1);
    let mut round = first_piece(table, &BenchItem { stack: shown, ..*m })?;
    round.top_round = true;
    round.pick = None;
    let bone = find_bone(gun_view::bones(round.kind), "magazine")?;
    let up = round.mats[bone].transform_vector3(Vec3::Y).normalize_or_zero();
    let (lo, hi) = round.reach();
    let at = (lo + hi) * 0.5;
    let over = at + up * 0.09;
    let start = table.at(from.x, from.z) + Vec3::Y * 0.02;
    let pos = if k < 0.6 {
        let q = ease(k / 0.6);
        start.lerp(over, q) + Vec3::Y * 0.08 * (q * PI).sin()
    } else {
        over.lerp(at, ease((k - 0.6) / 0.4))
    };
    let by = Mat4::from_translation(pos - at);
    for mat in &mut round.mats {
        *mat = by * *mat;
    }
    Some(round)
}
