//! Fishing with the rod (`fishing_rod`'s model): how it is held and moves (in the
//! first-person view and on the player model) while casting, waiting, fighting a fish and
//! landing it; the line from its tip to the bobber; and a fish on the end of the line.

use crate::model::items::fishing_rod::{self, RodPoints, RodPose, LENGTH};
use crate::world::mesh::{flags, Vertex};
use crate::textures::tex;
use glam::{Mat4, Vec3};

/// Seconds the whip of a cast takes (back over the shoulder, forward and down, then up again
/// to wait), and the lift of a landed fish.
pub const CAST_TIME: f32 = 0.55;
pub const LIFT_TIME: f32 = 0.8;
/// How far through the whip the rod is furthest forward (when the bobber leaves).
pub const WHIP_FORWARD: f32 = 0.28;

/// What the held rod is doing (set by the game each frame; sent to the others).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct RodAnim {
    /// How far the rod is drawn back to cast (0..1: the right button held).
    pub charge: f32,
    /// Seconds since the cast was let go (the whip), while it plays.
    pub cast: Option<f32>,
    /// The line is out (the rod held up a little higher, waiting).
    pub out: bool,
    /// Fighting a hooked fish (0..1, eased): the rod up high, braced against it.
    pub fight: f32,
    /// How hard the line pulls (0 slack .. 1 at the limit, more past it): bends the rod.
    pub tension: f32,
    /// The reel's handle (radians).
    pub crank: f32,
    /// Seconds since a fish was landed (the rod swung up), while it plays.
    pub lift: Option<f32>,
    /// Where the bobber is (world), when it is out.
    pub bobber: Option<Vec3>,
}

/// Where the grip is held and which way the rod points (in the frame of a view).
#[derive(Clone, Copy)]
struct Hold {
    grip: Vec3,
    dir: Vec3,
}

const fn hold(grip: [f32; 3], dir: [f32; 3]) -> Hold {
    Hold { grip: Vec3::from_array(grip), dir: Vec3::from_array(dir) }
}

/// The holds of a view: at rest, waiting with the line out, drawn back to cast, furthest
/// forward in the whip, fighting a fish, and swung up with a landed fish; where the free
/// (left) hand goes while the rod is swung.
struct Keys {
    rest: Hold,
    out: Hold,
    back: Hold,
    whip: Hold,
    fight: Hold,
    lift: Hold,
    free_hand: Vec3,
}

/// Camera space (blocks; x right, y up, -z ahead).
const FIRST_PERSON: Keys = Keys {
    rest: hold([0.2, -0.24, -0.62], [-0.2, 0.36, -1.0]),
    out: hold([0.18, -0.22, -0.64], [-0.14, 0.46, -1.0]),
    back: hold([0.34, -0.04, -0.46], [0.22, 1.0, 0.35]),
    whip: hold([0.14, -0.22, -0.8], [-0.08, 0.0, -1.0]),
    fight: hold([0.14, -0.18, -0.62], [-0.08, 0.95, -1.0]),
    lift: hold([0.18, -0.04, -0.56], [0.04, 1.0, 0.1]),
    free_hand: Vec3::new(-0.42, -0.85, -0.5),
};

/// The player model's torso (model pixels from the feet; facing -z).
const ON_MODEL: Keys = Keys {
    rest: hold([3.5, 13.0, -6.0], [-0.12, 0.5, -1.0]),
    out: hold([3.5, 14.0, -6.5], [-0.1, 0.62, -1.0]),
    back: hold([6.0, 25.5, -1.0], [0.12, 1.0, 0.8]),
    whip: hold([3.0, 18.0, -9.0], [-0.05, 0.05, -1.0]),
    fight: hold([2.5, 16.5, -7.5], [-0.08, 1.0, -0.8]),
    lift: hold([4.0, 23.0, -5.0], [0.05, 1.0, 0.25]),
    free_hand: Vec3::new(-6.0, 11.5, -1.5),
};

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn mix(a: Hold, b: Hold, k: f32) -> Hold {
    Hold { grip: a.grip.lerp(b.grip, k), dir: a.dir.normalize().lerp(b.dir.normalize(), k).normalize_or(a.dir) }
}

/// The whip's progress (0..1) with the swing forward quick and the settling slow.
fn whip(a: &RodAnim, keys: &Keys) -> Option<Hold> {
    let k = (a.cast? / CAST_TIME).clamp(0.0, 1.0);
    Some(if k < WHIP_FORWARD {
        let x = k / WHIP_FORWARD;
        // Easing in: it starts slow from the back and is fastest when it lets go.
        mix(keys.back, keys.whip, x * x)
    } else {
        mix(keys.whip, keys.out, smooth((k - WHIP_FORWARD) / (1.0 - WHIP_FORWARD)))
    })
}

/// How much of a landed fish's lift is on (0..1: up quickly, down slowly).
fn lifted(a: &RodAnim) -> f32 {
    let Some(t) = a.lift else { return 0.0 };
    let k = t / LIFT_TIME;
    if k < 0.3 {
        smooth(k / 0.3)
    } else {
        1.0 - smooth((k - 0.3) / 0.7)
    }
}

/// The hold of this moment. `line`: the line's direction in the view's frame (the fight turns
/// the rod toward it); `time` shakes it under strain.
fn hold_now(a: &RodAnim, keys: &Keys, line: Option<Vec3>, time: f32) -> Hold {
    let mut h = if a.out { keys.out } else { keys.rest };
    h = mix(h, keys.fight, smooth(a.fight));
    h = mix(h, keys.back, smooth(a.charge));
    if let Some(w) = whip(a, keys) {
        h = w;
    }
    h = mix(h, keys.lift, lifted(a));
    // Fighting: turned a little toward where the fish is, shaking with the strain.
    if let (Some(d), true) = (line, a.fight > 0.0) {
        let yaw = d.x.atan2(-d.z).clamp(-1.2, 1.2) * 0.45 * a.fight;
        h.dir = Mat4::from_rotation_y(-yaw).transform_vector3(h.dir);
    }
    let strain = (a.tension.clamp(0.0, 1.3) * a.fight).max(0.25 * smooth((a.charge - 0.9) * 10.0));
    let shake = Vec3::new((time * 37.0).sin() + 0.5 * (time * 61.0).sin(), (time * 29.0 + 1.0).sin(), 0.0) * 0.012 * strain;
    h.dir = (h.dir.normalize() + shake).normalize();
    h
}

/// The rod's frame (its model, blocks, to the view's frame) with the grip at the hold, `scale`
/// view units per block. (The model's +X, the crank's side, comes out on the left: held
/// right-handed, the left hand turns the crank.)
fn frame(h: Hold, scale: f32, grip: Vec3) -> Mat4 {
    let z = h.dir.normalize_or(Vec3::NEG_Z);
    let x = Vec3::Y.cross(z).normalize_or(Vec3::X);
    let y = z.cross(x);
    Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), h.grip.extend(1.0))
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-grip)
}

/// The rod drawn where `m` puts it (`fishing_rod::emit`).
pub fn emit_rod(out: &mut Vec<Vertex>, m: Mat4, pose: &RodPose, light: [u8; 4], fl: u8) -> RodPoints {
    fishing_rod::emit(out, m, pose, light, fl)
}

/// The points of the rod in its own model space as it looks in `pose` (the model is built
/// once aside to find them).
pub fn points_local(pose: &RodPose) -> RodPoints {
    thread_local! {
        static SCRATCH: std::cell::RefCell<Vec<Vertex>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    SCRATCH.with(|s| {
        let mut v = s.borrow_mut();
        v.clear();
        fishing_rod::emit(&mut v, Mat4::IDENTITY, pose, [0; 4], 0)
    })
}

/// `points_local` put where `m` puts the model.
pub fn points(m: Mat4, pose: &RodPose) -> RodPoints {
    let p = points_local(pose);
    RodPoints { tip: m.transform_point3(p.tip), grip: m.transform_point3(p.grip), crank: m.transform_point3(p.crank) }
}

/// How the rod bends and its crank stands, placed by `m` (model to the view's frame), the
/// bobber at `bobber` (the view's frame).
fn pose_for(a: &RodAnim, m: Mat4, bobber: Option<Vec3>) -> RodPose {
    let mut bend = a.tension.clamp(0.0, 1.2) * (0.35 + 0.75 * a.fight) + 0.08 * a.fight;
    let mut dir = Vec3::NEG_Y;
    if let Some(b) = bobber {
        let tip = m.transform_point3(Vec3::new(0.0, 0.0, LENGTH));
        let local = m.inverse().transform_vector3(b - tip);
        let across = Vec3::new(local.x, local.y, 0.0);
        if across.length_squared() > 1e-8 {
            dir = across.normalize();
        }
    }
    // The whip: the tip lags behind as the rod comes forward (bent back, up the rod's top).
    if let Some(t) = a.cast {
        let k = t / CAST_TIME;
        let lag = if k < WHIP_FORWARD + 0.15 { (k / (WHIP_FORWARD + 0.15) * std::f32::consts::PI).sin() } else { 0.0 };
        if lag > bend {
            bend = lag * 0.55;
            dir = Vec3::Y;
        }
    }
    RodPose { crank: a.crank, bend: bend.min(1.0), bend_dir: dir }
}

/// The rod in the first-person view: its frame (camera space) and pose. `bobber`: where the
/// bobber is in camera space; `eq`: how far it is brought up (being taken out).
pub fn first_person(a: &RodAnim, time: f32, bobber: Option<Vec3>, eq: f32) -> (Mat4, RodPose) {
    let grip = points_local(&RodPose::default()).grip;
    let mut h = hold_now(a, &FIRST_PERSON, bobber, time);
    h.grip.y -= (1.0 - eq) * 0.6;
    let m = frame(h, 1.0, grip);
    (m, pose_for(a, m, bobber))
}

/// Where the free (left) hand goes in the first-person view while the rod is being swung
/// (cast, drawn back): 0 on the crank .. 1 off it.
pub fn hand_off_crank(a: &RodAnim) -> f32 {
    let whip = a.cast.map_or(0.0, |t| 1.0 - smooth((t / CAST_TIME - 0.55) / 0.45));
    smooth(a.charge * 1.6).max(whip).max(lifted(a) * 0.6)
}

pub fn first_person_free_hand() -> Vec3 {
    FIRST_PERSON.free_hand
}

/// The rod on the player model: its frame in the torso's frame (model pixels), the head's
/// look turning it (`look`: the head's turn from the body, and its pitch), and its pose.
/// `bobber`: where the bobber is in the torso's frame.
pub fn on_model(a: &RodAnim, time: f32, look: (f32, f32), bobber: Option<Vec3>, px: f32) -> (Mat4, RodPose) {
    let grip = points_local(&RodPose::default()).grip;
    let shoulder = Vec3::new(0.0, 22.0, 0.0);
    // Turned with the head, and a little up and down with its look.
    let turn = Mat4::from_translation(shoulder)
        * Mat4::from_rotation_y(look.0)
        * Mat4::from_rotation_x(look.1 * 0.4)
        * Mat4::from_translation(-shoulder);
    let line = bobber.map(|b| turn.inverse().transform_vector3(b - turn.transform_point3(ON_MODEL.fight.grip)));
    let h = hold_now(a, &ON_MODEL, line, time);
    let m = turn * frame(h, 1.0 / px, grip);
    (m, pose_for(a, m, bobber))
}

/// Where the free hand goes on the model while the rod is swung (torso frame, model pixels).
pub fn model_free_hand() -> Vec3 {
    ON_MODEL.free_hand
}

/// The fishing line from the rod's tip to the bobber, seen from `cam`: a thin strip sagging
/// `sag` blocks in the middle (a slack line hangs, a taut one is straight).
pub fn emit_line(out: &mut Vec<Vertex>, from: Vec3, to: Vec3, cam: Vec3, sag: f32, light: [u8; 4]) {
    let len = from.distance(to);
    if len < 1e-3 {
        return;
    }
    let n = ((len * 1.5) as usize).clamp(6, 40);
    let at = |i: usize| {
        let s = i as f32 / n as f32;
        from.lerp(to, s) - Vec3::Y * sag * 4.0 * s * (1.0 - s)
    };
    let pts: Vec<Vec3> = (0..=n).map(at).collect();
    let side = |i: usize| {
        let along = pts[(i + 1).min(n)] - pts[i.saturating_sub(1)];
        let p = pts[i];
        // About a pixel wide wherever it is (thinner close up, never vanishing far off).
        let w = (p.distance(cam) * 0.0026).max(0.004);
        along.cross(cam - p).normalize_or_zero() * w * 0.5
    };
    let mut light = light;
    light[3] = 6;
    let v = |p: Vec3| Vertex { pos: p.to_array(), uv: [0.45, 0.45], layer: tex::WOOL as f32, light, tint: [236, 236, 228, 0] };
    for i in 0..n {
        let (a, b) = (pts[i], pts[i + 1]);
        let (sa, sb) = (side(i), side(i + 1));
        let q = [v(a - sa), v(b - sb), v(b + sb), v(a + sa)];
        // Both ways round (it is seen from either side).
        out.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3], q[0], q[2], q[1], q[0], q[3], q[2]]);
    }
}

/// The bobber at `pos`, tipped over by `tilt` radians toward `toward` (the line pulling it),
/// in the world.
pub fn emit_bobber(out: &mut Vec<Vertex>, pos: Vec3, toward: Vec3, tilt: f32, size: f32, light: [u8; 4]) {
    let axis = Vec3::Y.cross(toward).normalize_or(Vec3::X);
    let m = Mat4::from_translation(pos) * Mat4::from_axis_angle(axis, tilt) * Mat4::from_scale(Vec3::splat(size));
    fishing_rod::emit_bobber(out, m, light, flags::ENTITY);
}

/// A fish: its middle at `pos`, heading `dir`, `size` times a small one's size, its tail
/// swinging `wiggle` (radians), in the colours of its kind (`tint`: back, belly).
pub fn emit_fish(out: &mut Vec<Vertex>, pos: Vec3, dir: Vec3, size: f32, wiggle: f32, tint: [[u8; 3]; 2], light: [u8; 4]) {
    let dir = dir.normalize_or(Vec3::X);
    let q = glam::Quat::from_rotation_arc(Vec3::Z, dir);
    let m = Mat4::from_rotation_translation(q, pos) * Mat4::from_scale(Vec3::splat(size));
    let fl = flags::ENTITY;
    let [back, belly] = tint;
    let sides = [back, back, back, belly, back, back];
    let w = [tex::WOOL; 6];
    use crate::model::emit_box;
    // Body, the head a little narrower, the back fin, the tail swinging.
    emit_box(out, m, Vec3::new(-0.045, -0.06, -0.12), Vec3::new(0.045, 0.065, 0.1), w, sides, light, fl);
    emit_box(out, m, Vec3::new(-0.035, -0.045, 0.1), Vec3::new(0.035, 0.05, 0.17), w, sides, light, fl);
    emit_box(out, m, Vec3::new(-0.008, 0.065, -0.06), Vec3::new(0.008, 0.1, 0.05), w, [back; 6], light, fl);
    let tail = m * Mat4::from_translation(Vec3::new(0.0, 0.0, -0.12)) * Mat4::from_rotation_y(wiggle);
    emit_box(out, tail, Vec3::new(-0.01, -0.07, -0.1), Vec3::new(0.01, 0.07, 0.0), w, [back; 6], light, fl);
    // The eyes.
    let dark = [[30, 30, 30]; 6];
    for x in [-0.036, 0.028] {
        emit_box(out, m, Vec3::new(x, 0.01, 0.13), Vec3::new(x + 0.008, 0.025, 0.145), w, dark, light, fl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rod_is_held_by_its_grip_and_points_ahead() {
        for a in [
            RodAnim::default(),
            RodAnim { charge: 1.0, ..Default::default() },
            RodAnim { out: true, fight: 1.0, tension: 0.8, bobber: Some(Vec3::new(0.0, -2.0, -20.0)), ..Default::default() },
        ] {
            let (m, pose) = first_person(&a, 0.0, a.bobber, 1.0);
            let p = points(m, &pose);
            let rest = points_local(&RodPose::default()).grip;
            assert!(p.grip.distance(m.transform_point3(rest)) < 1e-3);
            assert!(p.tip.is_finite() && p.crank.is_finite());
            if a.charge == 0.0 {
                // Ahead of the eye, higher than the hand.
                assert!(p.tip.z < p.grip.z && p.tip.y > p.grip.y, "{:?}", p);
            } else {
                // Drawn back over the shoulder: up high.
                assert!(p.tip.y > 1.0, "{:?}", p);
            }
        }
    }

    #[test]
    fn a_line_is_drawn_both_ways_round() {
        let mut v = Vec::new();
        emit_line(&mut v, Vec3::ZERO, Vec3::new(10.0, -2.0, 0.0), Vec3::new(0.0, 0.0, 5.0), 0.5, [255; 4]);
        assert!(!v.is_empty() && v.len() % 12 == 0);
        assert!(v.iter().all(|v| v.pos.iter().all(|c| c.is_finite())));
    }
}
