//! Chopping a tree with an axe, as made in Blockbench (`tools/blockbench/chop.bbmodel`, from
//! `gen_chop_anim.py`): the player's rig with the axe in both hands, and its `chop`
//! animation (drawn back, swung round level into what is in front, stuck a moment, pulled
//! out; and `stump`: raised over the head and brought straight down into a stump). This rig is where the player and the axe really are while chopping: the player
//! model is drawn from it (third person), the arms and the axe are drawn from it where they
//! are in the world (first person: the very same arms and axe, seen from the eye), and the
//! game follows the axe's edge through it to find where it bites into a trunk (`Swing`).
//!
//! A swing comes out of the axe held at rest (`hold_axe`, as the player model holds it) and
//! settles back into it; it is aimed up or down by the upper body bending at the hips (the
//! legs stay planted, turned the way the body faces: `Aim`).
//!
//! Model space: the player model's pixels, standing on the origin, facing -Z.

#[allow(unused_imports, dead_code)]
mod data {
    include!("tp_chop_data.rs");
}

use crate::model::viewmodel::{add_anim, bone_matrices, find_anim, find_bone, BonePose};
use crate::world::mesh::Vertex;
use glam::{Mat4, Vec3};

/// Which swing: the level chop into a standing trunk, or the one straight down into a stump.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Chop,
    Stump,
}

/// A swing's times (seconds into its animation): the stroke (where the edge can meet the
/// wood), the edge at its furthest (the stroke's end when it meets nothing), when the axe is
/// pulled out again, and the whole animation's length.
pub struct Times {
    pub stroke: f32,
    pub hit: f32,
    pub pull: f32,
    pub length: f32,
}

impl Kind {
    pub fn times(self) -> Times {
        match self {
            Kind::Chop => Times { stroke: 0.29, hit: 0.38, pull: 0.52, length: 0.8 },
            Kind::Stump => Times { stroke: 0.34, hit: 0.42, pull: 0.58, length: 0.9 },
        }
    }

    fn anim(self) -> &'static str {
        match self {
            Kind::Chop => "chop",
            Kind::Stump => "stump",
        }
    }
}
/// Seconds the axe stays stuck in the wood, and how long it takes to come back into the
/// animation's pull after it.
const STUCK: f32 = 0.12;
const REJOIN: f32 = 0.12;
/// Seconds a swing takes to come out of the axe held at rest, and to settle back into it
/// after its animation's end.
const FADE_IN: f32 = 0.1;
const SETTLE: f32 = 0.18;
/// How much of the look's pitch the upper body bends (the head looks the rest of the way).
const BEND: f32 = 0.4;

/// Where the axe is held at rest (the right fist's middle, on the handle's end): the axe's
/// handle goes up (+Y) from it, its edge toward -Z.
pub const GRIP: Vec3 = Vec3::new(6.0, 13.2, 0.0);
/// Points along the blade's edge (at rest), from its heel to its toe.
pub const EDGE: [Vec3; 3] = [
    Vec3::new(6.0, 23.7, -5.6),
    Vec3::new(6.0, 25.95, -5.6),
    Vec3::new(6.0, 28.2, -5.6),
];

/// Blocks per model pixel.
pub const PX: f32 = 1.8 / 32.0;

/// One swing: which, seconds since it began, and the animation's time when the edge met the
/// wood (it stops there, stuck, then is pulled out).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Swing {
    pub kind: Kind,
    pub clock: f32,
    pub hit: Option<f32>,
}

impl Swing {
    /// Where the animation is: its time, and (after the axe was stuck somewhere short of the
    /// stroke's end) the stuck pose's time with how far it has come back into the pull.
    fn times(&self) -> (f32, Option<(f32, f32)>) {
        let pull = self.kind.times().pull;
        match self.hit {
            Some(h) if self.clock >= h => {
                let after = self.clock - h;
                if after < STUCK {
                    (h, None)
                } else {
                    let t = pull + (after - STUCK);
                    let k = ((after - STUCK) / REJOIN).clamp(0.0, 1.0);
                    (t, (k < 1.0).then_some((h, k * k * (3.0 - 2.0 * k))))
                }
            }
            _ => (self.clock, None),
        }
    }

    /// The animation's time now (for the edge's path).
    pub fn anim_time(&self) -> f32 {
        self.times().0
    }

    /// Settled back into holding the axe: the swing is gone.
    pub fn done(&self) -> bool {
        self.anim_time() >= self.kind.times().length + SETTLE
    }

    /// Past the animation's end (only settling back): the next swing may begin.
    pub fn over(&self) -> bool {
        self.anim_time() >= self.kind.times().length
    }

    /// Whether the axe has been stuck in the wood long enough to be pulled out.
    pub fn pulling(&self) -> bool {
        self.hit.is_some_and(|h| self.clock >= h + STUCK)
    }

    /// The rig's pose now.
    pub fn pose(&self) -> ChopPose {
        let (t, from) = self.times();
        let now = ChopPose::at(self.kind, t);
        let now = match from {
            Some((h, k)) => ChopPose::at(self.kind, h).blend(&now, k),
            None => now,
        };
        let length = self.kind.times().length;
        if t < FADE_IN {
            HOLD.blend(&now, smooth(t / FADE_IN))
        } else if t > length {
            now.blend(&HOLD, smooth((t - length) / SETTLE))
        } else {
            now
        }
    }
}

fn smooth(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// The axe held at rest, as the player model holds it (where a swing begins and ends).
static HOLD: std::sync::LazyLock<ChopPose> = std::sync::LazyLock::new(|| ChopPose::of_anim("hold_axe", 0.0, 1.0));

/// Where the player aims the swing (from where they look): `pitch` (the look's, radians, up
/// positive) is taken by the upper body bending at the hips, and the head looks on along it;
/// the legs stay turned `twist` (radians) from the way the upper body faces, the way the body
/// faces (the head's turn from the body's: `head_yaw - body_yaw`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aim {
    pub pitch: f32,
    pub twist: f32,
}

impl Aim {
    pub fn new(pitch: f32, head_yaw: f32, body_yaw: f32) -> Aim {
        use std::f32::consts::{PI, TAU};
        Aim { pitch, twist: (head_yaw - body_yaw + PI).rem_euclid(TAU) - PI }
    }
}

/// The rig's bones where they are: each bone's rest place (model space) to where it is.
#[derive(Clone)]
pub struct ChopPose {
    mats: Vec<Mat4>,
}

impl ChopPose {
    pub fn at(kind: Kind, t: f32) -> ChopPose {
        ChopPose::of_anim(kind.anim(), t, kind.times().length)
    }

    fn of_anim(name: &str, t: f32, length: f32) -> ChopPose {
        let bones = data::BONES;
        let mut p = vec![BonePose::default(); bones.len()];
        if let Some(anim) = find_anim(data::ANIMS, name) {
            add_anim(&mut p, anim, t.clamp(0.0, length - 1e-4), 1.0, |_| false);
        }
        ChopPose { mats: bone_matrices(bones, &p, Mat4::IDENTITY).0 }
    }

    /// Aimed (`Aim`): the upper body (and the axe) bent at the hips, the head looking on, the
    /// legs turned back the way the body faces.
    pub fn aimed(mut self, aim: Aim) -> ChopPose {
        use bone::*;
        let pitch = aim.pitch.clamp(-1.2, 1.2);
        let hip = Vec3::new(0.0, 12.0, 0.0);
        let bend = Mat4::from_translation(hip) * Mat4::from_rotation_x(pitch * BEND) * Mat4::from_translation(-hip);
        let legs = Mat4::from_rotation_y(aim.twist.clamp(-1.0, 1.0));
        for b in [TORSO, bone::HEAD, RIGHT_ARM, RIGHT_HAND, LEFT_ARM, LEFT_HAND, AXE] {
            if let Some(i) = BONE_IDS[b] {
                self.mats[i] = bend * self.mats[i];
            }
        }
        // (the head about its neck, as it now is)
        if let Some(i) = BONE_IDS[bone::HEAD] {
            let neck = self.mats[i].transform_point3(Vec3::new(0.0, 24.0, 0.0));
            let look = Mat4::from_translation(neck) * Mat4::from_rotation_x(pitch * (1.0 - BEND) * 0.8) * Mat4::from_translation(-neck);
            self.mats[i] = look * self.mats[i];
        }
        for b in [RIGHT_LEG, RIGHT_FOOT, LEFT_LEG, LEFT_FOOT] {
            if let Some(i) = BONE_IDS[b] {
                self.mats[i] = legs * self.mats[i];
            }
        }
        self
    }

    fn blend(&self, other: &ChopPose, k: f32) -> ChopPose {
        let mats = self.mats.iter().zip(&other.mats).map(|(a, b)| {
            let (sa, ra, ta) = a.to_scale_rotation_translation();
            let (sb, rb, tb) = b.to_scale_rotation_translation();
            Mat4::from_scale_rotation_translation(sa.lerp(sb, k), ra.slerp(rb, k), ta.lerp(tb, k))
        });
        ChopPose { mats: mats.collect() }
    }

    /// Where one of the bones in `bone` is.
    fn bone(&self, b: usize) -> Mat4 {
        BONE_IDS[b].map_or(Mat4::IDENTITY, |i| self.mats[i])
    }

    pub fn axe(&self) -> Mat4 {
        self.bone(bone::AXE)
    }
}

/// The rig's bones the player and the axe are drawn on (indices into `BONE_NAMES`).
mod bone {
    pub const AXE: usize = 0;
    pub const RIGHT_ARM: usize = 1;
    pub const RIGHT_HAND: usize = 2;
    pub const LEFT_ARM: usize = 3;
    pub const LEFT_HAND: usize = 4;
    pub const TORSO: usize = 5;
    pub const HEAD: usize = 6;
    pub const RIGHT_LEG: usize = 7;
    pub const RIGHT_FOOT: usize = 8;
    pub const LEFT_LEG: usize = 9;
    pub const LEFT_FOOT: usize = 10;
}

const BONE_NAMES: [&str; 11] = [
    "axe",
    "right_arm",
    "right_hand",
    "left_arm",
    "left_hand",
    "torso",
    "head",
    "right_leg",
    "right_foot",
    "left_leg",
    "left_foot",
];

/// Those bones' places in the rig, looked up by name once.
static BONE_IDS: std::sync::LazyLock<[Option<usize>; 11]> =
    std::sync::LazyLock::new(|| BONE_NAMES.map(|name| find_bone(data::BONES, name)));

/// The rig in the world: standing at `feet`, turned the way the head looks (`yaw`, as the
/// player model's; the legs are turned back by `Aim`).
pub fn to_world(feet: Vec3, yaw: f32) -> Mat4 {
    Mat4::from_translation(feet)
        * Mat4::from_rotation_y(-yaw - std::f32::consts::FRAC_PI_2)
        * Mat4::from_scale(Vec3::splat(PX))
}

/// The held axe item (`emit_held`'s flat sprite, one block across, its handle corner to
/// corner) placed on the rig's axe at rest, in model pixels: stood up so the handle runs up
/// from the grip, its head at the top, the blade's edge toward -Z.
pub fn axe_item() -> Mat4 {
    Mat4::from_translation(GRIP)
        * Mat4::from_scale(Vec3::splat(1.0 / PX))
        * Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(0.0, 0.36, 0.0))
        * Mat4::from_scale(Vec3::splat(0.72))
        * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4)
}

/// Which parts of the player to draw: all of it (seen from outside), only the arms and the
/// axe (seen from its own eyes, the rest behind the camera or below it), or the body under
/// them (the first-person body: all but the head, the arms and the axe).
#[derive(Clone, Copy, PartialEq)]
pub enum Parts {
    All,
    Arms,
    Body,
}

/// The player (its skin's parts: the game's own boxes, the arms and legs in halves at the
/// elbows and knees as in the rig) and the held axe, posed by the rig, `world` placing the
/// rig in the world.
#[allow(clippy::too_many_arguments)]
/// `tint` (hurt: reddened), `armor` (as `PlayerPose::armor`) and `shake` (the head shaking
/// while burning, radians) as the player model has them.
pub fn emit(
    out: &mut Vec<Vertex>,
    world: Mat4,
    pose: &ChopPose,
    parts: Parts,
    held: crate::item::ItemId,
    skin: u8,
    tint: [u8; 3],
    armor: u16,
    shake: f32,
    light: [u8; 4],
    fl: u8,
) {
    use crate::model::player::{skinned, ARM, BODY, HEAD, LEG};
    use bone::*;
    let v = Vec3::new;
    // (bone, lower corner, upper corner, layers, rows of the texture on its sides); the
    // upper halves of the limbs reach 2 px past the joint, the lower ones a hair thinner.
    let boxes: [(usize, Vec3, Vec3, [u32; 6], [f32; 2]); 10] = [
        (RIGHT_ARM, v(4.0, 16.0, -2.0), v(8.0, 24.0, 2.0), ARM, [0.0, 0.67]),
        (RIGHT_HAND, v(4.04, 12.0, -1.96), v(7.96, 18.0, 1.96), ARM, [0.5, 1.0]),
        (LEFT_ARM, v(-8.0, 16.0, -2.0), v(-4.0, 24.0, 2.0), ARM, [0.0, 0.67]),
        (LEFT_HAND, v(-7.96, 12.0, -1.96), v(-4.04, 18.0, 1.96), ARM, [0.5, 1.0]),
        (TORSO, v(-4.0, 12.0, -2.0), v(4.0, 24.0, 2.0), BODY, [0.0, 1.0]),
        (bone::HEAD, v(-4.0, 24.0, -4.0), v(4.0, 32.0, 4.0), HEAD, [0.0, 1.0]),
        (RIGHT_LEG, v(-0.1, 4.0, -2.0), v(3.9, 12.0, 2.0), LEG, [0.0, 0.67]),
        (RIGHT_FOOT, v(-0.06, 0.0, -1.96), v(3.86, 6.0, 1.96), LEG, [0.5, 1.0]),
        (LEFT_LEG, v(-3.9, 4.0, -2.0), v(0.1, 12.0, 2.0), LEG, [0.0, 0.67]),
        (LEFT_FOOT, v(-3.86, 0.0, -1.96), v(0.06, 6.0, 1.96), LEG, [0.5, 1.0]),
    ];
    for (b, lo, hi, layers, rows) in boxes {
        let arm = matches!(b, RIGHT_ARM | RIGHT_HAND | LEFT_ARM | LEFT_HAND);
        let head = b == bone::HEAD;
        let shown = match parts {
            Parts::All => true,
            Parts::Arms => arm,
            Parts::Body => !arm && !head,
        };
        if shown {
            let m = if head { world * pose.bone(b) * head_shake(shake) } else { world * pose.bone(b) };
            crate::model::emit_box_rows(out, m, lo, hi, skinned(layers, skin), [tint; 6], light, fl, rows);
        }
    }
    // The armor on the same bones (each frame at its joint, as `build_player` has them; seen
    // from its own eyes only the body's).
    if parts != Parts::Arms {
        let at = |b: usize, x: f32, y: f32| world * pose.bone(b) * Mat4::from_translation(Vec3::new(x, y, 0.0));
        let all = parts == Parts::All;
        let frames = crate::model::player::ArmorFrames {
            head: all.then(|| at(bone::HEAD, 0.0, 24.0) * Mat4::from_rotation_y(shake)),
            body: at(TORSO, 0.0, 24.0),
            right_arm: all.then(|| at(RIGHT_ARM, 5.0, 22.0)),
            left_arm: all.then(|| at(LEFT_ARM, -5.0, 22.0)),
            legs: [at(RIGHT_LEG, 1.9, 12.0), at(LEFT_LEG, -1.9, 12.0)],
            shins: [at(RIGHT_FOOT, 1.9, 12.0), at(LEFT_FOOT, -1.9, 12.0)],
        };
        crate::model::player::emit_armor(out, &frames, armor, tint, light, fl);
    }
    if parts == Parts::Body {
        return;
    }
    let st = crate::item::Stack::one(held);
    crate::model::emit_held_data(out, world * pose.axe() * axe_item(), &st, light, fl);
}

/// The head shaking about the neck (see `emit`).
fn head_shake(shake: f32) -> Mat4 {
    let neck = Vec3::new(0.0, 24.0, 0.0);
    Mat4::from_translation(neck) * Mat4::from_rotation_y(shake) * Mat4::from_translation(-neck)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_animations_are_as_long_as_the_game_thinks() {
        for kind in [Kind::Chop, Kind::Stump] {
            let anim = find_anim(data::ANIMS, kind.anim()).expect("animation");
            assert!((anim.length - kind.times().length).abs() < 1e-3);
        }
    }

    #[test]
    fn the_axe_bites_in_ahead_of_the_player() {
        let edge = ChopPose::at(Kind::Chop, Kind::Chop.times().hit).axe().transform_point3(EDGE[1]);
        assert!(edge.z < -10.0, "{edge}");
    }

    #[test]
    fn the_stump_swing_comes_down_ahead_of_the_feet() {
        let t = Kind::Stump.times();
        let up = ChopPose::at(Kind::Stump, 0.3).axe().transform_point3(EDGE[1]);
        let down = ChopPose::at(Kind::Stump, t.hit).axe().transform_point3(EDGE[1]);
        assert!(up.y > 30.0, "raised: {up}");
        assert!(down.y < 4.0 && down.z < -12.0, "down: {down}");
    }

    #[test]
    fn a_swing_stuck_early_is_pulled_out_smoothly() {
        for (kind, at) in [(Kind::Chop, 0.34), (Kind::Stump, 0.4)] {
            // (from the moment it is stuck: the stroke itself may be fast)
            let mut s = Swing { kind, clock: at, hit: Some(at) };
            let mut last = s.pose().axe().transform_point3(EDGE[1]);
            while !s.done() {
                s.clock += 0.01;
                let now = s.pose().axe().transform_point3(EDGE[1]);
                assert!((now - last).length() < 6.0, "jump at {}: {last} -> {now}", s.clock);
                last = now;
            }
        }
    }
}
