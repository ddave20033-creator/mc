//! Blocky player character (head, body, arms, legs) with Minecraft's walk, run, sneak
//! and attack animations.
//! Model space is in pixels (1 px = 1/16 of the model height unit), Y up, facing -Z.

use crate::model::emit_box;
use crate::item::{icon, tool_of, Icon, ItemId, NONE, STICK};
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::textures::tex;
use glam::{Mat4, Vec3};
use std::f32::consts::{PI, TAU};

/// World units per model pixel (the model is 32 px tall).
pub const PX: f32 = 1.8 / 32.0;
pub const LIMB_SWING_SCALE: f32 = 0.6662;

pub struct PlayerPose {
    /// Feet position.
    pub pos: Vec3,
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    /// Attack swing progress 0..1 (0 = none).
    pub attack: f32,
    /// Sneak amount 0..1.
    pub crouch: f32,
    /// Running (0..1: the gun is carried across the chest).
    pub sprint: f32,
    pub held: ItemId,
    /// The held stack's `data` (a magazine's rounds).
    pub held_data: u16,
    pub skin: u8,
    pub time: f32,
    /// Red flash when hurt.
    pub hurt: bool,
    /// Seen from the player's own eyes (like the First Person Model mod): no head, the camera
    /// is inside it.
    pub first_person: bool,
    /// On fire (in lava or still burning after leaving it).
    pub burning: bool,
    /// Blocking with a sword.
    pub blocking: bool,
    /// Leave out the arms and the held item (First Person Model's dynamic hands: the regular
    /// first-person hand shows them while you are not looking down).
    pub hide_arms: bool,
    /// Leave out only the right arm and the held item.
    pub hide_right_arm: bool,
    /// Held lantern: direction from the hand down its chain (from its swing).
    pub lantern: Option<Vec3>,
    /// The held pistol's attachments, and what it is doing (its slide, trigger and magazine
    /// move like in the first-person view).
    pub gun_mods: u8,
    /// How dirty the held gun looks (`pistol_view::dirt_level`).
    pub gun_dirt: u8,
    pub gun: crate::model::guns::pistol_view::GunAnim,
    /// What is worn (`item::armor_code`).
    pub armor: u16,
    /// Holding the guide book open: its pages (see `book::BookView`).
    pub book: Option<crate::model::items::book::BookView>,
    /// Readying the held grenade: seconds since the button went down (raised, the pin pulled
    /// by the left hand, drawn back higher the harder it will be thrown).
    pub grenade: Option<f32>,
    /// Holding a fishing rod: what it is doing (cast, line out, fighting a fish...).
    pub rod: Option<crate::model::items::angler::RodAnim>,
    /// Chopping a tree: the axe's swing (`chop_rig`); the whole player is posed by it then.
    pub chop: Option<crate::model::players::chop_rig::Swing>,
}

// Face order for layers: +X, -X, +Y, -Y, +Z (back), -Z (front)
pub const HEAD: [u32; 6] = [
    tex::HEAD_SIDE,
    tex::HEAD_SIDE,
    tex::HAIR,
    tex::SKIN,
    tex::HEAD_BACK,
    tex::FACE,
];
pub const BODY: [u32; 6] = [
    tex::SHIRT,
    tex::SHIRT,
    tex::SHIRT,
    tex::LEG,
    tex::SHIRT_BACK,
    tex::SHIRT_FRONT,
];
pub const ARM: [u32; 6] = [
    tex::ARM,
    tex::ARM,
    tex::SLEEVE,
    tex::SKIN,
    tex::ARM,
    tex::ARM,
];
pub const LEG: [u32; 6] = [tex::LEG; 6];

fn t(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

/// A limb's frame (`upper`, from its pivot) bent `bend` radians forward at `joint` pixels down
/// it: the lower half's frame, the same as the limb's when straight.
fn bent(upper: Mat4, joint: f32, bend: f32) -> Mat4 {
    upper * t(0.0, -joint, 0.0) * Mat4::from_rotation_x(bend) * t(0.0, joint, 0.0)
}

/// Rotations (x, y, z in radians) of the arms and legs, applied Z·Y·X like Minecraft's ModelPart,
/// and how far the elbows and knees bend (radians: a forearm forward, a shin back).
#[derive(Clone, Copy, Default)]
pub struct Limbs {
    pub right_arm: Vec3,
    pub left_arm: Vec3,
    pub right_leg: Vec3,
    pub left_leg: Vec3,
    pub right_elbow: f32,
    pub left_elbow: f32,
    pub right_knee: f32,
    pub left_knee: f32,
    /// How far a shoulder slides toward what its hand holds when it would not reach it
    /// (model pixels, in the body's frame).
    pub right_shift: Vec3,
    pub left_shift: Vec3,
}

/// The arms and legs bend halfway: shoulder to elbow, hip to knee (model pixels).
const UPPER_ARM: f32 = 4.0;
const THIGH: f32 = 6.0;
/// Elbow to the middle of the fist, and knee to the sole.
const FOREARM: f32 = 5.5;
const SHIN: f32 = 6.0;
/// How far above and below a joint an arm or a leg bends (half its thickness): the mesh turns
/// from one bone to the next through there, like a skinned one.
const JOINT_BLEND: f32 = 2.0;
/// How far the body leans forward while sneaking (about the neck).
const SNEAK_LEAN: f32 = 0.3;
/// How far the shoulders and the neck sink while sneaking (the head a pixel more).
pub const SNEAK_DROP: f32 = 2.2;

/// Where the hips are (the legs' pivots, x left out): behind the body as it leans.
fn hips(p: &PlayerPose) -> Vec3 {
    let lean = gait(p).lean;
    let neck = 24.0 - SNEAK_DROP * p.crouch;
    Vec3::new(0.0, neck - 12.0 * lean.cos(), 12.0 * lean.sin())
}

/// How far a thigh swings (radians) walking, at this limb amount and running.
fn stride(la: f32, s: f32) -> f32 {
    la * (0.62 + 0.26 * s)
}

/// How the whole body moves with the steps.
struct Gait {
    /// The whole model sinks a little as the legs spread (model pixels, down).
    bob: f32,
    /// The shoulders turn against the hips (radians about the vertical), and the torso rocks
    /// from side to side over the legs.
    twist: f32,
    roll: f32,
    /// How far the body leans forward (sneaking, running).
    lean: f32,
}

fn gait(p: &PlayerPose) -> Gait {
    let ls = p.limb_swing * LIMB_SWING_SCALE;
    let (la, s, c) = (p.limb_amount.min(1.0), p.sprint.clamp(0.0, 1.0), p.crouch.clamp(0.0, 1.0));
    // A third of the drop straight legs would make at their widest.
    let spread = stride(la, s) * ls.cos();
    let bob = 0.35 * 12.0 * (1.0 - spread.cos());
    // Not with both hands on something held: they would come off it. A gun's stance turns
    // the body instead.
    let gun = crate::item::GunKind::of(p.held).filter(|_| p.attack <= 0.0 && !p.blocking);
    let both_hands = p.book.is_some() || gun.is_some() || readying(p) || rod_anim(p).is_some();
    let free = if both_hands { 0.0 } else { 1.0 };
    Gait {
        bob,
        twist: -(0.07 + 0.05 * s) * la * ls.cos() * free,
        roll: 0.03 * la * (1.0 + s) * ls.sin() * free,
        lean: SNEAK_LEAN * c + 0.12 * s * (1.0 - c),
    }
}

/// How far a held gun's stance turns the whole body, feet and all (a rifle's puts the left
/// shoulder forward; less while sneaking), the head still looking ahead.
fn gun_turn(p: &PlayerPose) -> f32 {
    match crate::item::GunKind::of(p.held).filter(|_| p.attack <= 0.0 && !p.blocking) {
        Some(kind) => crate::model::players::tp_rig::held(kind, p).turn * (1.0 - 0.35 * p.crouch.clamp(0.0, 1.0)),
        None => 0.0,
    }
}

/// Where the head looks against the body (radians about the vertical, as `look_turn`).
fn look_yaw(p: &PlayerPose) -> f32 {
    -(p.head_yaw - p.body_yaw)
}

/// The model's root: the feet on the ground where the player is, facing its body's way, sunk
/// with the steps; model pixels to the world.
fn model_root(p: &PlayerPose) -> Mat4 {
    Mat4::from_translation(p.pos)
        * Mat4::from_rotation_y(-p.body_yaw - PI / 2.0)
        * Mat4::from_scale(Vec3::splat(PX))
        * t(0.0, -gait(p).bob, 0.0)
        * Mat4::from_rotation_y(gun_turn(p))
}

/// What carries the body and the arms: turned by an attack's swing and by the steps (about the
/// hips).
fn torso_of(p: &PlayerPose, root: Mat4) -> Mat4 {
    let g = gait(p);
    root * t(0.0, 12.0, 0.0)
        * Mat4::from_rotation_y(attack_twist(p.attack) + g.twist)
        * Mat4::from_rotation_z(g.roll)
        * t(0.0, -12.0, 0.0)
}

/// A leg reaching from the hip for a foot on the ground (both in the side view: y up, z back):
/// the thigh's swing (+ forward) and how far the knee bends.
fn leg_to(hip: Vec3, foot: Vec3) -> (f32, f32) {
    let d = foot - hip;
    let dist = (d.y * d.y + d.z * d.z).sqrt().clamp(0.01, THIGH + SHIN - 0.01);
    let along = (-d.z).atan2(-d.y);
    let half = (dist / (THIGH + SHIN)).clamp(-1.0, 1.0).acos();
    (along + half, 2.0 * half)
}

/// Shoulders slide at most this far toward what they hold (model pixels).
const MAX_SHIFT: f32 = 3.0;

/// An arm (hanging from `shoulder`, on a body turned `turn` about the vertical) bent at the
/// elbow to put the middle of its fist at `target`, the elbow toward `pole`: its rotation (in
/// the body's frame), the bend, and how far its shoulder slides (in the body's frame) if the
/// arm alone would not reach.
fn reach_bent(shoulder: Vec3, target: Vec3, pole: Vec3, turn: f32) -> (Vec3, f32, Vec3) {
    let far = (target - shoulder).length() - (UPPER_ARM + FOREARM - 0.05);
    let slide = if far > 0.0 { (target - shoulder).normalize_or_zero() * far.min(MAX_SHIFT) } else { Vec3::ZERO };
    let shoulder = shoulder + slide;
    let d = target - shoulder;
    let dist = d.length().clamp(0.5, UPPER_ARM + FOREARM - 0.01);
    let w = d.normalize_or(Vec3::NEG_Y);
    // The elbow's inner angle, from the three sides.
    let inner = ((UPPER_ARM * UPPER_ARM + FOREARM * FOREARM - dist * dist) / (2.0 * UPPER_ARM * FOREARM))
        .clamp(-1.0, 1.0)
        .acos();
    let bend = PI - inner;
    // The fist with the arm hanging and bent forward, turned onto the target.
    let fist = Vec3::new(0.0, -UPPER_ARM, 0.0) + Mat4::from_rotation_x(bend).transform_vector3(Vec3::new(0.0, -FOREARM, 0.0));
    let aim = glam::Quat::from_rotation_arc(fist.normalize(), w);
    // Rolled about the line to the fist so the elbow points out and down.
    let elbow = aim * Vec3::new(0.0, -UPPER_ARM, 0.0);
    let flat = |v: Vec3| v - w * v.dot(w);
    let (e, q) = (flat(elbow), flat(pole));
    let roll = if e.length_squared() > 1e-6 && q.length_squared() > 1e-6 {
        let (e, q) = (e.normalize(), q.normalize());
        e.cross(q).dot(w).atan2(e.dot(q))
    } else {
        0.0
    };
    let r = glam::Quat::from_rotation_y(-turn) * glam::Quat::from_axis_angle(w, roll) * aim;
    let (z, y, x) = r.to_euler(glam::EulerRot::ZYX);
    (Vec3::new(x, y, z), bend, glam::Quat::from_rotation_y(-turn) * slide)
}

fn rot(r: Vec3) -> Mat4 {
    Mat4::from_rotation_z(r.z) * Mat4::from_rotation_y(r.y) * Mat4::from_rotation_x(r.x)
}

/// Attack: the torso twists so the right shoulder comes forward (HumanoidModel.setupAttackAnimation).
fn attack_twist(a: f32) -> f32 {
    if a > 0.0 {
        (a.sqrt() * TAU).sin() * 0.2
    } else {
        0.0
    }
}

/// Burning flail phase: changes every game tick (1/20 s), like `sin(tickCount)`.
fn flail(time: f32) -> f32 {
    (time * 20.0).floor().sin() * 0.1
}

/// Limb rotations for this frame: Minecraft's walk, sneak, hold and attack poses, then the
/// Not Enough Animations poses on top (burning flail, torch held up to the eyes). The swinging
/// arm always keeps the attack animation.
/// Minecraft's model space has X and Y flipped, so its X and Y rotations appear negated here.
pub fn limb_targets(p: &PlayerPose) -> Limbs {
    let ls = p.limb_swing * LIMB_SWING_SCALE;
    let la = p.limb_amount;
    let c = p.crouch;
    let a = p.attack;
    let swinging = a > 0.0;

    let s = p.sprint.clamp(0.0, 1.0);

    // Arms: swing opposite to the legs, breathe while idle, a little forward when sneaking;
    // the elbows bend a little as they swing forward, and a bit more while running.
    let idle_z = (p.time * 1.8).cos() * 0.05 + 0.05;
    let idle_x = (p.time * 1.34).sin() * 0.05;
    let arm_swing = la * (1.0 - 0.25 * s);
    let (swing_r, swing_l) = ((ls + PI).cos(), ls.cos());
    let mut right_x = swing_r * arm_swing + idle_x + 0.12 * c;
    let left_x = swing_l * arm_swing - idle_x + 0.12 * c;
    let elbow = |swing: f32| 0.1 + la * (0.15 + 0.2 * swing.max(0.0)) + s * (0.3 + 0.1 * swing.max(0.0)) + 0.1 * c;
    let mut right_elbow = elbow(swing_r);
    let mut right_z = idle_z;
    let mut right_y = attack_twist(a);
    if p.blocking {
        // Minecraft 1.8's blocking pose: the arm brings the sword across the chest.
        right_x = right_x * 0.5 + PI * 0.3;
        right_y += std::f32::consts::FRAC_PI_6;
        right_elbow = 0.4;
    } else if p.held != NONE {
        right_x = right_x * 0.5 + PI / 10.0;
        right_elbow = right_elbow.min(0.25 + 0.3 * s);
    }
    if swinging {
        let f = 1.0 - (1.0 - a).powi(4);
        right_x += (f * PI).sin() * 1.2 + (a * PI).sin() * (0.7 + p.pitch) * 0.75;
        right_z -= (a * PI).sin() * 0.4;
        // The arm straightens to strike.
        right_elbow *= 1.0 - (a * PI).sin();
    }

    // Legs: the thigh swings as in Minecraft (a little less, the knee does the rest); the leg
    // coming forward bends at the knee, the one on the ground stays straight. Sneaking, the hips
    // sink behind the leaning body and the knees bend under them, the feet stepping short.
    let stride = stride(la, s);
    let knee_swing = la * (0.7 + 0.8 * s);
    let step = |phase: f32| (phase.cos() * stride, 0.08 * la + knee_swing * (-phase.sin()).max(0.0));
    let (mut right_leg, mut right_knee) = step(ls);
    let (mut left_leg, mut left_knee) = step(ls + PI);
    if c > 0.0 {
        let hip = hips(p);
        let short = (la / 0.3).min(1.0);
        let crouched = |phase: f32| {
            let foot = Vec3::new(0.0, 1.0 * short * (-phase.sin()).max(0.0), hip.z * 0.5 - 2.2 * short * phase.cos());
            leg_to(hip, foot)
        };
        let ((rt, rk), (lt, lk)) = (crouched(ls), crouched(ls + PI));
        let mix = |a: f32, b: f32| a + (b - a) * c;
        (right_leg, right_knee) = (mix(right_leg, rt), mix(right_knee, rk));
        (left_leg, left_knee) = (mix(left_leg, lt), mix(left_knee, lk));
    }
    let mut l = Limbs {
        right_arm: Vec3::new(right_x, right_y, right_z),
        left_arm: Vec3::new(left_x, 0.0, -idle_z),
        right_leg: Vec3::new(right_leg, 0.0, 0.0),
        left_leg: Vec3::new(left_leg, 0.0, 0.0),
        right_elbow,
        left_elbow: elbow(swing_l),
        right_knee,
        left_knee,
        right_shift: Vec3::ZERO,
        left_shift: Vec3::ZERO,
    };

    if p.burning {
        // BurningAnimation: both arms thrown up above the head, flailing.
        let h = flail(p.time);
        if !swinging {
            l.right_arm = Vec3::new(2.6 - h, 0.2, -0.3);
            l.right_elbow = 0.2;
        }
        l.left_arm = Vec3::new(2.6 + h, -0.2, 0.3);
        l.left_elbow = 0.2;
    } else if held_up(p.held) && !swinging {
        // LookAtItemAnimation (camera target) for the items Not Enough Animations holds up by
        // default (here the torch): held up in front of the eyes, following where the head
        // looks. Other items are held like in plain Minecraft.
        let head_yaw = p.head_yaw - p.body_yaw;
        // The torch follows vertical look subtly; a full 1:1 pitch made the hand
        // sweep across most of the screen. Keep the lantern's established pose.
        let pitch_follow = if p.held == crate::world::TORCH as ItemId {
            0.48
        } else {
            1.0
        };
        l.right_arm = Vec3::new(
            (PI / 2.0 + p.pitch * pitch_follow).clamp(0.0, 2.5),
            -(0.1 + head_yaw).clamp(-0.2, 0.2),
            0.1,
        );
        l.right_elbow = 0.0;
    } else if let (Some(_), false) = (p.book, swinging) {
        // Holding the open book: both hands hold it by its sides in front of the chest (each
        // hand the side on its own side, also when it is turned around to show it).
        let shoulder_y = 22.0 - SNEAK_DROP * c;
        let at = book_on_model(p);
        let (a, b) = (
            at.transform_point3(Vec3::new(5.5, 0.0, 1.5)),
            at.transform_point3(Vec3::new(-5.5, 0.0, 1.5)),
        );
        let (right, left) = if a.x >= b.x { (a, b) } else { (b, a) };
        (l.right_arm, l.right_elbow, l.right_shift) = reach_bent(Vec3::new(5.0, shoulder_y, 0.0), right, Vec3::new(1.0, -1.0, 0.35), 0.0);
        (l.left_arm, l.left_elbow, l.left_shift) = reach_bent(Vec3::new(-5.0, shoulder_y, 0.0), left, Vec3::new(-1.0, -1.0, 0.35), 0.0);
    } else if let (Some(t), false) = (p.grenade.filter(|_| readying(p)), swinging) {
        // Readying a grenade, as the first-person hand does (its poses brought over to the
        // model: where the hand is before the eyes): brought up in front of the face, the
        // left hand comes up by it and takes the pin off down to the side, then it goes a
        // little to the right and down, and up again in front, higher the harder it will be
        // thrown.
        use crate::model::guns::grenade::{power, PULL_TIME, RAISE_TIME};
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let shoulder_y = 22.0 - SNEAK_DROP * c;
        let look = look_turn(p);
        let pull = t - RAISE_TIME;
        let (rest, front) = (Vec3::new(5.0, 13.0, -4.0), Vec3::new(2.0, 21.0, -8.5));
        let cock = smooth((pull - 0.55) / 0.3);
        let right = rest.lerp(front, smooth(t / RAISE_TIME)).lerp(cocked(power(t)), cock);
        // (the ring is up at the left of the grenade in the fist)
        let ring = front + Vec3::new(-3.0, 4.0, -0.5);
        let (by_side, away) = (Vec3::new(-6.0, 11.0, -1.0), Vec3::new(-7.0, 9.0, 1.5));
        let left = if pull < 0.35 {
            by_side.lerp(ring, smooth((t - 0.05) / (RAISE_TIME + 0.03)))
        } else {
            ring.lerp(away, smooth((pull - 0.35) / (PULL_TIME - 0.35)))
        };
        let (sr, sl) = (Vec3::new(5.0, shoulder_y, 0.0), Vec3::new(-5.0, shoulder_y, 0.0));
        (l.right_arm, l.right_elbow, l.right_shift) =
            reach_bent(sr, look.transform_point3(right), Vec3::new(1.0, -1.0, 0.35), 0.0);
        (l.left_arm, l.left_elbow, l.left_shift) =
            reach_bent(sl, look.transform_point3(left), Vec3::new(-1.0, -1.0, 0.35), 0.0);
    } else if let (true, true) = (throwing(p), swinging) {
        // A grenade just thrown (the swing): the arm goes on forward from where it held it
        // and down, the hand empty.
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let over = Vec3::new(4.5, 26.0, -9.5);
        let down = Vec3::new(3.5, 14.0, -7.5);
        let hand = if a < 0.4 { cocked(0.7).lerp(over, smooth(a / 0.4)) } else { over.lerp(down, smooth((a - 0.4) / 0.6)) };
        let sr = Vec3::new(5.0, 22.0 - SNEAK_DROP * c, 0.0);
        (l.right_arm, l.right_elbow, l.right_shift) =
            reach_bent(sr, look_turn(p).transform_point3(hand), Vec3::new(1.0, -1.0, 0.35), 0.0);
    } else if let Some((rod, pose)) = rod_on_torso(p) {
        // Holding a fishing rod: the right hand on its grip, the left on the reel's handle
        // (off it to the side while the rod is swung).
        let pts = crate::model::items::angler::points(rod, &pose);
        let a = rod_anim(p).unwrap_or_default();
        let shoulder_y = 22.0 - SNEAK_DROP * c;
        let free = crate::model::items::angler::model_free_hand() - Vec3::Y * SNEAK_DROP * c;
        let left = pts.crank.lerp(free, crate::model::items::angler::hand_off_crank(&a));
        let (sr, sl) = (Vec3::new(5.0, shoulder_y, 0.0), Vec3::new(-5.0, shoulder_y, 0.0));
        (l.right_arm, l.right_elbow, l.right_shift) = reach_bent(sr, pts.grip, Vec3::new(1.0, -1.0, 0.35), 0.0);
        (l.left_arm, l.left_elbow, l.left_shift) = reach_bent(sl, left, Vec3::new(-1.0, -1.0, 0.35), 0.0);
    } else if let (Some(kind), false, false) =
        (crate::item::GunKind::of(p.held), swinging, p.blocking)
    {
        // Holding a gun: both arms reach for it where its rig has it (`tp_rig`: at rest,
        // walking, running, sneaking, aimed, reloading; it turns with the head), the right
        // hand to the grip, the left where the rig puts it, the elbows bent out.
        let held = crate::model::players::tp_rig::held(kind, p);
        // (The rig holds the gun on a body turned into its stance; the model's root is turned
        // so already.)
        let look = Mat4::from_rotation_y(-gun_turn(p)) * look_turn(p);
        let shoulder_y = 22.0 - SNEAK_DROP * c;
        let right_hand = (look * held.gun).transform_point3(Vec3::ZERO);
        let left_hand = look.transform_point3(held.left_hand);
        // A rifle's right elbow out to the side, its left one down under the handguard (the hand
        // holding it up from below).
        // (The body is not twisted with a gun: it turns all of it, legs and all.)
        let stance = 0.0;
        let turn = Mat4::from_rotation_y(stance);
        let (pole_r, pole_l) = if kind.long() {
            (Vec3::new(1.0, -0.4, 0.3), Vec3::new(-0.25, -1.0, 0.1))
        } else {
            (Vec3::new(1.0, -1.0, 0.35), Vec3::new(-1.0, -1.0, 0.35))
        };
        let (sr, sl) = (turn.transform_point3(Vec3::new(5.0, shoulder_y, 0.0)), turn.transform_point3(Vec3::new(-5.0, shoulder_y, 0.0)));
        (l.right_arm, l.right_elbow, l.right_shift) = reach_bent(sr, right_hand, pole_r, stance);
        (l.left_arm, l.left_elbow, l.left_shift) = reach_bent(sl, left_hand, pole_l, stance);
    }
    if p.first_person && !held_up(p.held) {
        // First Person Model's dynamic hands: just past the angle where the body's arms take
        // over from the first-person hand they are pulled back a little (up to 0.7 rad),
        // straightening out as you look further down.
        let back = (2.0 + p.pitch.to_degrees() / 20.0).clamp(0.0, 0.7);
        l.right_arm.x -= back;
        l.left_arm.x -= back;
    }
    l
}

/// Not Enough Animations' animation smoothing: every game tick (1/20 s) the limbs move 90% of
/// the way to their target pose, so changes between poses blend instead of snapping.
#[derive(Default)]
pub struct LimbSmoother {
    cur: Option<Limbs>,
}

impl LimbSmoother {
    pub fn update(&mut self, target: Limbs, dt: f32) -> Limbs {
        let keep = 0.1f32.powf(dt * 20.0);
        let blend = |t: Vec3, c: Vec3| t + (c - t) * keep;
        let bend = |t: f32, c: f32| t + (c - t) * keep;
        let next = match self.cur {
            Some(c) => Limbs {
                right_arm: blend(target.right_arm, c.right_arm),
                left_arm: blend(target.left_arm, c.left_arm),
                right_leg: blend(target.right_leg, c.right_leg),
                left_leg: blend(target.left_leg, c.left_leg),
                right_elbow: bend(target.right_elbow, c.right_elbow),
                left_elbow: bend(target.left_elbow, c.left_elbow),
                right_knee: bend(target.right_knee, c.right_knee),
                left_knee: bend(target.left_knee, c.left_knee),
                right_shift: blend(target.right_shift, c.right_shift),
                left_shift: blend(target.left_shift, c.left_shift),
            },
            None => target,
        };
        self.cur = Some(next);
        next
    }
}

/// 0 above `from`, 1 below `to` (`from` > `to`, going down a limb), smooth in between.
fn blend_down(y: f32, from: f32, to: f32) -> f32 {
    let x = ((from - y) / (from - to)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Where along a limb (model pixels down it, from its pivot) it bends from one bone into the
/// next: through the elbow, from the body into the thigh, and through the knee.
const ELBOW_ZONE: (f32, f32) = (-UPPER_ARM + JOINT_BLEND, -UPPER_ARM - JOINT_BLEND);
const HIP_ZONE: (f32, f32) = (0.0, -2.0 * JOINT_BLEND);
const KNEE_ZONE: (f32, f32) = (-THIGH + JOINT_BLEND, -THIGH - JOINT_BLEND);

/// The rings of an arm (from 2 above its pivot, the shoulder, to the hand) and of a leg (from
/// the hips to the foot): the same for every player, worked out once.
static ARM_RINGS: std::sync::LazyLock<Vec<f32>> = std::sync::LazyLock::new(|| limb_rings(2.0, -10.0, &[ELBOW_ZONE]));
static LEG_RINGS: std::sync::LazyLock<Vec<f32>> = std::sync::LazyLock::new(|| limb_rings(0.0, -12.0, &[HIP_ZONE, KNEE_ZONE]));

/// Heights (model pixels down a limb, from its pivot) where a limb's mesh has a ring of
/// vertices: every half pixel through the bends, far apart elsewhere.
fn limb_rings(top: f32, bottom: f32, bends: &[(f32, f32)]) -> Vec<f32> {
    let mut ys = vec![top, bottom];
    for &(from, to) in bends {
        let mut y = from;
        while y > to - 1e-3 {
            ys.push(y);
            y -= 0.5;
        }
    }
    ys.retain(|&y| y <= top && y >= bottom);
    ys.sort_by(|a, b| b.total_cmp(a));
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    ys
}

/// An arm or a leg as one mesh that bends smoothly, like a skinned one: a box (x and z from
/// `min`/`max`, from `top` down to `bottom`) cut into rings (`limb_rings`), each ring placed by
/// `frame` (the limb's transform at that height: blending from one bone to the next through a
/// joint). Its sides show the rows of their textures for their heights.
#[allow(clippy::too_many_arguments)]
fn emit_bent_limb(
    out: &mut Vec<Vertex>,
    frame: impl Fn(f32) -> Mat4,
    min: Vec3,
    max: Vec3,
    rings: &[f32],
    layers: [u32; 6],
    tints: [[u8; 3]; 6],
    light: [u8; 4],
    fl: u8,
) {
    use crate::model::prim::{quad_at, Paint, Sides};
    use crate::world::mesh::{corner_pos, corner_uv, CORNERS};
    let (top, bottom) = (rings[0], rings[rings.len() - 1]);
    let row = |y: f32| (top - y) / (top - bottom);
    let quad = |out: &mut Vec<Vertex>, face: usize, ya: f32, ma: &Mat4, yb: f32, mb: &Mat4, v: [f32; 2]| {
        let mut pos = [Vec3::ZERO; 4];
        let mut uvs = [[0.0; 2]; 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let c = Vec3::from(corner_pos(face, su, sv));
            let (y, m) = if face == 2 || face == 3 {
                (ya, ma)
            } else if sv > 0 {
                (ya, ma)
            } else {
                (yb, mb)
            };
            let local = Vec3::new(min.x + (max.x - min.x) * c.x, y, min.z + (max.z - min.z) * c.z);
            let mut uv = corner_uv(su, sv);
            if face != 2 && face != 3 {
                uv[1] = v[0] + (v[1] - v[0]) * uv[1];
            }
            (pos[i], uvs[i]) = (m.transform_point3(local), uv);
        }
        let paint = Paint { layer: layers[face], light, face: face as u8, tint: tints[face], fl };
        quad_at(out, pos, uvs, &paint, Sides::Front);
    };
    // Each ring's frame worked out once, going down the limb.
    let first = frame(top);
    let mut upper = first;
    for k in 0..rings.len() - 1 {
        let (ya, yb) = (rings[k], rings[k + 1]);
        let lower = frame(yb);
        for face in [0, 1, 4, 5] {
            quad(out, face, ya, &upper, yb, &lower, [row(ya), row(yb)]);
        }
        upper = lower;
    }
    quad(out, 2, top, &first, top, &first, [0.0, 1.0]);
    quad(out, 3, bottom, &upper, bottom, &upper, [0.0, 1.0]);
}

/// Where the armor is worn, each frame at its joint as `build_player` poses the body: the
/// head and the body at the neck, the arms at the shoulders, the legs and the shins at the
/// hips (a part not drawn: None).
pub struct ArmorFrames {
    pub head: Option<Mat4>,
    pub body: Mat4,
    pub right_arm: Option<Mat4>,
    pub left_arm: Option<Mat4>,
    pub legs: [Mat4; 2],
    pub shins: [Mat4; 2],
}

/// Armor over the body: a helmet (the face left free), a chestplate with shoulder pads,
/// leggings from the hips, boots, and the vest over the chest with its pouches (`armor` as
/// `item::unpack_armor` reads it).
pub fn emit_armor(out: &mut Vec<Vertex>, f: &ArmorFrames, armor: u16, tint: [u8; 3], light: [u8; 4], fl: u8) {
    let (worn, vest) = crate::item::unpack_armor(armor);
    let piece = |out: &mut Vec<Vertex>, m: Mat4, min: [f32; 3], max: [f32; 3], material: usize| {
        let (layer, color) = match material {
            0 => (tex::ARMOR_WOOL, [196, 184, 160]),
            1 => (tex::ARMOR_METAL, [226, 146, 96]),
            2 => (tex::ARMOR_METAL, [176, 184, 198]),
            _ => (tex::ARMOR_METAL, [120, 228, 232]),
        };
        let c: [u8; 3] = std::array::from_fn(|i| (color[i] as u32 * tint[i] as u32 / 255) as u8);
        emit_box(out, m, Vec3::from(min), Vec3::from(max), [layer; 6], [c; 6], light, fl);
    };
    if let (Some(m), Some(head)) = (worn[0], f.head) {
        piece(out, head, [-4.6, 4.6, -4.6], [4.6, 8.7, 4.6], m);
        piece(out, head, [-4.6, 0.5, 1.2], [4.6, 4.6, 4.6], m);
        piece(out, head, [-4.6, 1.5, -4.6], [-3.6, 4.6, 1.2], m);
        piece(out, head, [3.6, 1.5, -4.6], [4.6, 4.6, 1.2], m);
    }
    if let Some(m) = worn[1] {
        piece(out, f.body, [-4.6, -10.8, -2.6], [4.6, 0.5, 2.6], m);
        if let Some(right) = f.right_arm {
            piece(out, right, [-1.6, -4.0, -2.6], [3.6, 2.6, 2.6], m);
        }
        if let Some(left) = f.left_arm {
            piece(out, left, [-3.6, -4.0, -2.6], [1.6, 2.6, 2.6], m);
        }
    }
    if let Some(m) = worn[2] {
        piece(out, f.body, [-4.5, -12.4, -2.5], [4.5, -9.8, 2.5], m);
        for leg in f.legs {
            piece(out, leg, [-2.5, -8.5, -2.5], [2.5, 0.3, 2.5], m);
        }
    }
    if let Some(m) = worn[3] {
        for shin in f.shins {
            piece(out, shin, [-2.6, -12.4, -2.6], [2.6, -8.3, 2.6], m);
        }
    }
    if vest {
        let olive: [u8; 3] = std::array::from_fn(|i| ([118u32, 124, 92][i] * tint[i] as u32 / 255) as u8);
        let dark: [u8; 3] = olive.map(|c| (c as u32 * 4 / 5) as u8);
        let v = |out: &mut Vec<Vertex>, min: [f32; 3], max: [f32; 3], c: [u8; 3]| {
            emit_box(out, f.body, Vec3::from(min), Vec3::from(max), [tex::VEST; 6], [c; 6], light, fl);
        };
        v(out, [-4.9, -10.2, -3.0], [4.9, 0.6, 3.0], olive);
        for (x0, x1) in [(-3.8, -1.5), (-1.1, 1.1), (1.5, 3.8)] {
            v(out, [x0, -9.6, -3.7], [x1, -6.6, -3.0], dark);
        }
    }
}

/// A body part's texture layers (`HEAD`, `BODY`, `ARM`, `LEG`) in the player's skin `skin`.
pub fn skinned(layers: [u32; 6], skin: u8) -> [u32; 6] {
    layers.map(|layer| crate::textures::skin_layer(layer, skin))
}

/// `glass`: where the held gun's see-through glass goes (drawn blended).
pub fn build_player(out: &mut Vec<Vertex>, glass: &mut Vec<Vertex>, p: &PlayerPose, limbs: &Limbs, sky: u8, blk: u8) {
    let light = vertex_light(sky, blk);
    let tint = if p.hurt {
        [255, 120, 120]
    } else {
        [255, 255, 255]
    };
    let tints = [tint; 6];
    let fl = flags::ENTITY;
    if let Some(swing) = p.chop {
        // Chopping: the whole player as the chop's rig has it (seen from its own eyes, the
        // body under the arms: the first-person view draws the arms and the axe from the
        // same rig, where they are in the world).
        use crate::model::players::chop_rig::{emit, to_world, Aim, Parts};
        let parts = if p.first_person { Parts::Body } else { Parts::All };
        let shake = if p.burning { flail(p.time) } else { 0.0 };
        let aim = Aim::new(p.pitch, p.head_yaw, p.body_yaw);
        let pose = swing.pose().aimed(aim);
        emit(out, to_world(p.pos, p.head_yaw), &pose, parts, p.held, p.skin, tint, p.armor, shake, light, fl);
        return;
    }
    let root = model_root(p);
    let skin = |layers: [u32; 6]| skinned(layers, p.skin);
    // A part of the body.
    let box_ = |out: &mut Vec<Vertex>, m: Mat4, min: [f32; 3], max: [f32; 3], layers: [u32; 6]| {
        emit_box(out, m, Vec3::from(min), Vec3::from(max), skin(layers), tints, light, fl);
    };

    let c = p.crouch;
    let torso = torso_of(p, root);
    let g = gait(p);

    // Head: sinks with the shoulders while sneaking, shakes while burning.
    let shake = if p.burning { flail(p.time) } else { 0.0 };
    // Aiming, the head tips toward the gun: a rifle's cheek down on its stock.
    let aim = p.gun.aim.clamp(0.0, 1.0);
    let tilt = match crate::item::GunKind::of(p.held) {
        Some(kind) if kind.long() => -0.2 * aim,
        Some(_) => -0.08 * aim,
        None => 0.0,
    };
    let head = root
        * t(0.0, 24.0 - (SNEAK_DROP + 1.0) * c, 0.0)
        * Mat4::from_rotation_y(look_yaw(p) - gun_turn(p) + shake)
        * Mat4::from_rotation_x(p.pitch)
        * Mat4::from_rotation_z(tilt);
    if !p.first_person {
        box_(out, head, [-4.0, 0.0, -4.0], [4.0, 8.0, 4.0], HEAD);
    }

    // Body: hangs from the neck and leans forward while sneaking (hips go back).
    // Leaning straight ahead, and turned about its own length (the steps, a gun's stance, an
    // attack): not tipped over sideways when a turned body leans.
    let body = root
        * t(0.0, 12.0, 0.0)
        * Mat4::from_rotation_z(g.roll)
        * t(0.0, 12.0 - SNEAK_DROP * c, 0.0)
        * Mat4::from_rotation_x(-g.lean)
        * Mat4::from_rotation_y(attack_twist(p.attack) + g.twist);
    box_(out, body, [-4.0, -12.0, -2.0], [4.0, 0.0, 2.0], BODY);

    // Arms: the upper arm from the shoulder, the forearm bent at the elbow (`right_hand` is the
    // arm's frame carried by the forearm: what is held goes with it).
    let shoulder_y = 22.0 - SNEAK_DROP * c;
    let right = torso * Mat4::from_translation(Vec3::new(5.0, shoulder_y, 0.0) + limbs.right_shift) * rot(limbs.right_arm);
    let left = torso * Mat4::from_translation(Vec3::new(-5.0, shoulder_y, 0.0) + limbs.left_shift) * rot(limbs.left_arm);
    let right_hand = bent(right, UPPER_ARM, limbs.right_elbow);
    // The right arm (and what it holds) can be left out alone: the first-person hand shows
    // it instead while the left arm stays on the body.
    let show_right = !p.hide_arms && !p.hide_right_arm;
    // Each arm one mesh from the shoulder (2 above its pivot) to the hand, bending smoothly
    // through the elbow.
    let arm_rings: &[f32] = &ARM_RINGS;
    let arm_frame = |arm: Mat4, bend: f32| move |y: f32| bent(arm, UPPER_ARM, bend * blend_down(y, ELBOW_ZONE.0, ELBOW_ZONE.1));
    if show_right {
        let (lo, hi) = (Vec3::new(-1.0, 0.0, -2.0), Vec3::new(3.0, 0.0, 2.0));
        emit_bent_limb(out, arm_frame(right, limbs.right_elbow), lo, hi, arm_rings, skin(ARM), tints, light, fl);
    }
    if !p.hide_arms {
        let (lo, hi) = (Vec3::new(-3.0, 0.0, -2.0), Vec3::new(1.0, 0.0, 2.0));
        emit_bent_limb(out, arm_frame(left, limbs.left_elbow), lo, hi, arm_rings, skin(ARM), tints, light, fl);
    }

    // Legs: from the hips (sunk behind the body while sneaking), bent at the knees.
    let hip = hips(p);
    let rl = root * t(1.9, hip.y, hip.z) * rot(limbs.right_leg);
    let ll = root * t(-1.9, hip.y, hip.z) * rot(limbs.left_leg);
    let right_shin = bent(rl, THIGH, -limbs.right_knee);
    let left_shin = bent(ll, THIGH, -limbs.left_knee);
    // Each leg one mesh with the body: its top stays square to the body's bottom (the hips don't
    // come apart when it swings), turning into the thigh just below, and it bends smoothly
    // through the knee.
    let leg_rings: &[f32] = &LEG_RINGS;
    let body_turn = glam::Quat::from_rotation_x(-g.lean) * glam::Quat::from_rotation_y(g.twist);
    let leg_frame = |x: f32, turn: Vec3, knee: f32| {
        let at = root * t(x, hip.y, hip.z);
        let leg = glam::Quat::from_mat4(&rot(turn));
        move |y: f32| {
            let q = body_turn.slerp(leg, blend_down(y, HIP_ZONE.0, HIP_ZONE.1));
            bent(at * Mat4::from_quat(q), THIGH, -knee * blend_down(y, KNEE_ZONE.0, KNEE_ZONE.1))
        }
    };
    let (lo, hi) = (Vec3::new(-2.0, 0.0, -2.0), Vec3::new(2.0, 0.0, 2.0));
    emit_bent_limb(out, leg_frame(1.9, limbs.right_leg, limbs.right_knee), lo, hi, leg_rings, skin(LEG), tints, light, fl);
    emit_bent_limb(out, leg_frame(-1.9, limbs.left_leg, limbs.left_knee), lo, hi, leg_rings, skin(LEG), tints, light, fl);

    let frames = ArmorFrames {
        head: (!p.first_person).then_some(head),
        body,
        right_arm: show_right.then_some(right),
        left_arm: (!p.hide_arms).then_some(left),
        legs: [rl, ll],
        shins: [right_shin, left_shin],
    };
    emit_armor(out, &frames, p.armor, tint, light, fl);

    // Held item, placed like Minecraft's ItemInHandLayer followed by the item model's
    // `thirdperson_righthand` display transform (handheld tools, flat items, blocks).
    if p.held == crate::world::LANTERN as ItemId && show_right {
        // Hanging from the hand by its chain, swinging with its pendulum.
        let pivot = right_hand.transform_point3(Vec3::new(1.0, -11.0, 0.0));
        let dir = p.lantern.unwrap_or(Vec3::NEG_Y);
        let style = crate::model::items::lantern::ON_MODEL;
        crate::model::items::lantern::emit_held_lantern(out, style, pivot, dir, p.body_yaw, light, fl);
    } else if let (Some(fill), true) = (crate::model::items::bucket::Fill::of(p.held), show_right) {
        // Hanging from the hand by its handle like the lantern, swinging with the same
        // pendulum, its ears to the sides.
        use crate::model::items::bucket;
        // (the grip in the fist)
        let pivot = right_hand.transform_point3(Vec3::new(1.0, -10.2, 0.0));
        let dir = p.lantern.unwrap_or(Vec3::NEG_Y);
        let tilt = glam::Quat::from_rotation_arc(Vec3::NEG_Y, dir.try_normalize().unwrap_or(Vec3::NEG_Y));
        let size = 0.5;
        let m = Mat4::from_translation(pivot)
            * Mat4::from_quat(tilt)
            * Mat4::from_rotation_y(-p.body_yaw + std::f32::consts::FRAC_PI_2)
            * Mat4::from_scale(Vec3::splat(size))
            * Mat4::from_translation(Vec3::new(0.0, -bucket::handle_top(), 0.0));
        bucket::emit(out, m, fill, &bucket::Surface::still(false), 0.0, light, fl);
    } else if let (Some(view), true) = (&p.book, show_right) {
        crate::model::items::book::emit_open_book(out, root * book_on_model(p), view, light, fl);
    } else if let (Some(kind), true) = (crate::item::GunKind::of(p.held), show_right) {
        // The Blockbench gun, its parts moving like in the first-person view.
        let (mats, shown) = gun_matrices(p, kind, root * gun_on_model(p, kind));
        let lamp = p.gun_mods & crate::item::gun_mod::LIGHT != 0 && p.gun_mods & crate::item::gun_mod::LIGHT_ON != 0;
        crate::model::guns::gun_view::emit(kind, out, Some(glass), &mats, &shown, false, p.gun_dirt, lamp, &p.gun, light, fl);
    } else if let (Some((rod, pose)), true) = (rod_on_torso(p), show_right) {
        // The fishing rod, where the hands hold it.
        crate::model::items::angler::emit_rod(out, torso * rod, &pose, light, fl);
    } else if throwing(p) && p.attack > 0.0 {
        // (it has just left the hand)
    } else if let (Some(t), true) = (p.grenade.filter(|_| readying(p)), show_right) {
        // A grenade being readied: its pin coming out, then gone.
        use crate::model::guns::grenade::{emit, sized, Look};
        let smoke = p.held == crate::item::SMOKE_GRENADE;
        emit(out, smoke, sized(smoke, held_item(p, right_hand), 0.62), Look::readied(t), light, fl);
    } else if p.held != NONE && show_right {
        let st = crate::item::Stack { data: p.held_data, ..crate::item::Stack::one(p.held) };
        crate::model::emit_held_data(out, held_item(p, right_hand), &st, light, fl);
    }
}

/// Where the open guide book is on the player model (model pixels from the feet, facing -Z):
/// held in front of the chest, its far edge tipped up toward the eyes; looking down lifts it
/// up to read.
fn book_on_model(p: &PlayerPose) -> Mat4 {
    let read = ((-p.pitch - 0.2) / 0.6).clamp(0.0, 1.0);
    // Shown: held out further and higher, turned around to face whoever is in front, and
    // stood up so they can read it.
    let show = p.book.map_or(0.0, |b| b.show);
    let e = show * show * (3.0 - 2.0 * show);
    let read = read * (1.0 - e);
    t(0.0, 14.0 + 3.0 * read + 4.0 * e - SNEAK_DROP * p.crouch, -6.0 - 1.0 * read - 2.5 * e)
        * Mat4::from_rotation_y(PI * e)
        * Mat4::from_rotation_x(0.5 + 0.6 * read + 0.8 * e)
        * Mat4::from_scale(Vec3::splat(0.8))
}

/// Where the head looks, turning what is held in front of it about the shoulders (the rig's
/// gun is posed looking straight ahead).
fn look_turn(p: &PlayerPose) -> Mat4 {
    let pivot = Vec3::new(0.0, 22.0 - SNEAK_DROP * p.crouch, 0.0);
    Mat4::from_translation(pivot)
        * Mat4::from_rotation_y(-(p.head_yaw - p.body_yaw))
        * Mat4::from_rotation_x(p.pitch)
        * Mat4::from_translation(-pivot)
}

/// Where a held gun is on the player model (model pixels from the feet, facing -Z): where its
/// third-person rig has it for what the player is doing (`tp_rig`), turned with the head,
/// drawn thicker across than it is (`gun::Spec::thick`).
pub fn gun_on_model(p: &PlayerPose, kind: crate::item::GunKind) -> Mat4 {
    let spec = crate::model::guns::gun::spec(kind);
    let held = crate::model::players::tp_rig::held(kind, p);
    // Gun space to model space: the muzzle forward (-Z), its right side to the right (+X).
    let basis = Mat4::from_cols(
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::Y,
        glam::Vec4::X,
        glam::Vec4::W,
    );
    Mat4::from_rotation_y(-gun_turn(p))
        * look_turn(p)
        * held.gun
        * basis
        * Mat4::from_scale(Vec3::splat(spec.arm_scale * crate::model::guns::gun_view::MODEL_SCALE))
        * Mat4::from_scale(Vec3::new(1.0, 1.0, spec.thick))
        * Mat4::from_translation(-spec.hand)
}

/// The Blockbench gun's bones held by the model: `gun` is the old gun space's transform
/// (`gun_on_model`, with the model's own root in front for the world).
fn gun_matrices(p: &PlayerPose, kind: crate::item::GunKind, gun: Mat4) -> (Vec<Mat4>, Vec<bool>) {
    use crate::model::guns::gun_view;
    gun_view::matrices(kind, &p.gun, p.gun_mods, true, gun * gun_view::to_gun_space(kind))
}

/// A point of the held gun (gun space) in the world.
pub fn gun_point(p: &PlayerPose, kind: crate::item::GunKind, point: Vec3) -> Vec3 {
    let root = model_root(p);
    (root * gun_on_model(p, kind)).transform_point3(point)
}

/// The held item's transform (the unit item of `emit_held`) from the right arm's, placed like
/// Minecraft's ItemInHandLayer followed by the item model's `thirdperson_righthand` display
/// transform (handheld tools, flat items, blocks).
fn held_item(p: &PlayerPose, right: Mat4) -> Mat4 {
    let deg = f32::to_radians;
    let (tr, r, sc) = if tool_of(p.held).is_some() || p.held == STICK {
        ([0.0, 4.0, 0.5], [0.0, -90.0, 55.0], 0.85)
    } else if matches!(icon(p.held), Icon::Block(_)) {
        ([0.0, 2.5, 0.0], [75.0, 45.0, 0.0], 0.375)
    } else if crate::model::is_model_item(p.held) {
        // A gun's part, a magazine, a grenade...: in the fist, its side outward.
        ([0.0, 2.0, 0.5], [0.0, -90.0, 0.0], 0.42)
    } else {
        ([0.0, 3.0, 1.0], [0.0, 0.0, 0.0], 0.55)
    };
    right
        // Into Minecraft's model space (X and Y flipped), then its hand offset.
        * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        * Mat4::from_rotation_x(deg(-90.0))
        * Mat4::from_rotation_y(deg(180.0))
        * t(1.0, 2.0, -10.0)
        * t(tr[0], tr[1], tr[2])
        * Mat4::from_rotation_x(deg(r[0]))
        * Mat4::from_rotation_y(deg(r[1]))
        * Mat4::from_rotation_z(deg(r[2]))
        * Mat4::from_scale(Vec3::splat(16.0 * sc))
}

/// What the held fishing rod is doing (None: no rod in the hand).
fn rod_anim(p: &PlayerPose) -> Option<crate::model::items::angler::RodAnim> {
    (p.held == crate::item::FISHING_ROD).then(|| p.rod.unwrap_or_default())
}

/// The held fishing rod on the model: its model (blocks) to the torso's frame (model pixels),
/// and how it bends toward the line.
fn rod_on_torso(p: &PlayerPose) -> Option<(Mat4, crate::model::items::fishing_rod::RodPose)> {
    let a = rod_anim(p)?;
    // Lowered with the shoulders while sneaking.
    let sink = Mat4::from_translation(Vec3::new(0.0, -SNEAK_DROP * p.crouch, 0.0));
    let torso = torso_of(p, model_root(p)) * sink;
    let bobber = a.bobber.map(|b| torso.inverse().transform_point3(b));
    let (m, pose) = crate::model::items::angler::on_model(&a, p.time, (look_yaw(p), p.pitch), bobber, PX);
    Some((sink * m, pose))
}

/// Where the tip of the held fishing rod is on the model (the line leaves from there), in the
/// world.
pub fn rod_tip(p: &PlayerPose) -> Option<Vec3> {
    let (rod, pose) = rod_on_torso(p)?;
    let torso = torso_of(p, model_root(p));
    Some(crate::model::items::angler::points(torso * rod, &pose).tip)
}

/// Whether a grenade is being readied in the hand.
fn readying(p: &PlayerPose) -> bool {
    p.grenade.is_some() && crate::model::grenade_item(p.held)
}

/// A grenade in the hand and not being readied: a swing now is its throw.
fn throwing(p: &PlayerPose) -> bool {
    p.grenade.is_none() && crate::model::grenade_item(p.held)
}

/// Where the right hand holds a readied grenade once the pin is out (model pixels, the
/// torso's frame), as the first-person hand does: in front, to the right, from under the
/// eyes up over them with the throw's `power`.
fn cocked(power: f32) -> Vec3 {
    let k = power * power * (3.0 - 2.0 * power);
    Vec3::new(4.5, 21.5, -8.5).lerp(Vec3::new(5.5, 28.5, -7.0), k)
}

/// The middle of the held item (a readied grenade: where it is thrown from), in the world.
pub fn held_center(p: &PlayerPose, limbs: &Limbs) -> Vec3 {
    held_item(p, right_arm(p, limbs)).transform_point3(Vec3::ZERO)
}

/// Where a held torch's fire is (the tip of its glowing head), in the world.
pub fn held_torch_tip(p: &PlayerPose, limbs: &Limbs) -> Vec3 {
    held_item(p, right_arm(p, limbs)).transform_point3(TORCH_TIP)
}

/// The top of a torch's glowing head in the torch model (`emit_torch`).
pub const TORCH_TIP: Vec3 = Vec3::new(0.0, 0.17, 0.0);

/// Lights held up in front of the eyes (Not Enough Animations' pose for torches; lanterns too).
pub fn held_up(item: ItemId) -> bool {
    item == crate::world::TORCH as ItemId || hangs(item)
}

/// Whether a held item hangs from the hand and swings (a lantern by its chain, a bucket by
/// its handle).
pub fn hangs(item: ItemId) -> bool {
    item == crate::world::LANTERN as ItemId || crate::model::items::bucket::is_bucket(item)
}

/// Whether a held item lights up the world around its holder (a torch, a lantern, a bucket
/// of lava).
pub fn gives_light(item: ItemId) -> bool {
    item == crate::world::TORCH as ItemId || item == crate::world::LANTERN as ItemId || item == crate::item::LAVA_BUCKET
}

/// A player lying on their back in a bed. `bed_top` is the middle of the top of the bed's
/// head half and `head` the way the bed points (foot to head). Returns where the standing
/// model's feet go, its body yaw (facing the foot end) and the turn that lays it down: the
/// head reaches into the pillow and the feet stay on the foot half.
pub fn lying(bed_top: Vec3, head: Vec3) -> (Vec3, f32, glam::Quat) {
    let feet = bed_top - head * 1.35 + Vec3::Y * 2.0 * PX;
    let yaw = (-head.z).atan2(-head.x);
    (feet, yaw, glam::Quat::from_rotation_arc(Vec3::Y, head))
}

/// Turns model vertices (built standing with their feet at `feet`) by `turn` around the feet.
pub fn lay_down(verts: &mut [Vertex], feet: Vec3, turn: glam::Quat) {
    use crate::world::mesh::FACE_N;
    for v in verts {
        v.pos = (feet + turn * (Vec3::from(v.pos) - feet)).to_array();
        // The face direction used for shading turns too.
        if let Some(n) = FACE_N.get(v.light[3] as usize) {
            let n = turn * Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32);
            if let Some(i) = FACE_N
                .iter()
                .position(|m| Vec3::new(m[0] as f32, m[1] as f32, m[2] as f32).dot(n) > 0.9)
            {
                v.light[3] = i as u8;
            }
        }
    }
}

/// The right arm's transform (model pixels to the world), as `build_player` draws it.
fn right_arm(p: &PlayerPose, limbs: &Limbs) -> Mat4 {
    let torso = torso_of(p, model_root(p));
    let shoulder = Vec3::new(5.0, 22.0 - SNEAK_DROP * p.crouch, 0.0) + limbs.right_shift;
    bent(torso * Mat4::from_translation(shoulder) * rot(limbs.right_arm), UPPER_ARM, limbs.right_elbow)
}

/// Where the right hand holds things (just below the fist), in the world.
pub fn hand_pivot(p: &PlayerPose, limbs: &Limbs) -> Vec3 {
    right_arm(p, limbs).transform_point3(Vec3::new(1.0, -11.0, 0.0))
}

#[cfg(test)]
mod gun_hold_tests {
    use super::*;
    use crate::item::GUN_KINDS;

    fn pose(held: ItemId, pitch: f32, turn: f32) -> PlayerPose {
        PlayerPose {
            pos: Vec3::ZERO,
            body_yaw: 0.0,
            head_yaw: turn,
            pitch,
            limb_swing: 0.0,
            limb_amount: 0.0,
            attack: 0.0,
            crouch: 0.0,
            sprint: 0.0,
            held,
            skin: 0,
            time: 0.0,
            hurt: false,
            first_person: false,
            burning: false,
            blocking: false,
            hide_arms: false,
            hide_right_arm: false,
            lantern: None,
            gun_mods: 0,
            gun_dirt: 0,
            held_data: 0,
            gun: Default::default(),
            armor: 0,
            book: None,
            grenade: None,
            rod: None,
            chop: None,
        }
    }

    #[test]
    fn hands_hold_the_gun_that_points_where_the_head_looks() {
        for kind in GUN_KINDS {
            for (pitch, turn) in [(0.0, 0.0), (0.5, 0.3), (-0.6, -0.4)] {
                let p = pose(kind.item(), pitch, turn);
                // (on the model's root, which a gun's stance turns)
                let g = Mat4::from_rotation_y(gun_turn(&p)) * gun_on_model(&p, kind);
                let spec = crate::model::guns::gun::spec(kind);
                // The muzzle is ahead of the grip, the way the head faces (held ready: a little
                // down, a rifle's a little more).
                let look = Mat4::from_rotation_y(-turn)
                    * Mat4::from_rotation_x(pitch)
                    * glam::Vec4::new(0.0, 0.0, -1.0, 0.0);
                use crate::model::guns::gun_view::{muzzle, rest_point_in_gun_space};
                let (bone, front) = muzzle(kind, 0);
                // Down the barrel: from ten pixels behind its end to its end.
                let back = rest_point_in_gun_space(kind, (bone, front + Vec3::Z * 10.0));
                let along = g.transform_point3(rest_point_in_gun_space(kind, (bone, front))) - g.transform_point3(back);
                assert!(along.normalize().dot(look.truncate()) > 0.85, "{kind:?}");
                // The right arm reaches for the grip, bent at the elbow; its fist closes on it.
                let l = limb_targets(&p);
                let fist = right_arm(&p, &l).transform_point3(Vec3::new(0.0, -UPPER_ARM - FOREARM, 0.0));
                let grip = gun_point(&p, kind, spec.hand);
                assert!(fist.distance(grip) < 0.6 * PX, "{kind:?}: fist {fist}, grip {grip}");
            }
        }
    }
}
#[cfg(test)]
mod bend_tests {
    use super::*;

    fn pose(crouch: f32, limb_swing: f32, limb_amount: f32, sprint: f32) -> PlayerPose {
        PlayerPose {
            pos: Vec3::ZERO,
            body_yaw: 0.0,
            head_yaw: 0.0,
            pitch: 0.0,
            limb_swing,
            limb_amount,
            attack: 0.0,
            crouch,
            sprint,
            held: NONE,
            skin: 0,
            time: 0.0,
            hurt: false,
            first_person: false,
            burning: false,
            blocking: false,
            hide_arms: false,
            hide_right_arm: false,
            lantern: None,
            gun_mods: 0,
            gun_dirt: 0,
            held_data: 0,
            gun: Default::default(),
            armor: 0,
            book: None,
            grenade: None,
            rod: None,
            chop: None,
        }
    }

    /// The sole of the right foot (model pixels), as `build_player` bends the leg.
    fn sole(p: &PlayerPose, l: &Limbs) -> Vec3 {
        let hip = hips(p);
        let leg = t(1.9, hip.y, hip.z) * rot(l.right_leg);
        bent(leg, THIGH, -l.right_knee).transform_point3(Vec3::new(0.0, -THIGH - SHIN, 0.0))
    }

    #[test]
    fn sneaking_bends_the_knees_with_the_feet_on_the_ground() {
        for swing in [0.0, 1.0, 2.5, 4.0] {
            let p = pose(1.0, swing / LIMB_SWING_SCALE, 0.0, 0.0);
            let l = limb_targets(&p);
            assert!(l.right_knee > 0.5, "the knee bends: {}", l.right_knee);
            assert!(sole(&p, &l).y.abs() < 0.05, "the foot stands: {}", sole(&p, &l));
        }
    }

    #[test]
    fn the_stepping_knee_bends_forward_and_the_standing_one_stays_straight() {
        for sprint in [0.0, 1.0] {
            // The right leg comes forward (its thigh swinging on toward the front).
            let forward = limb_targets(&pose(0.0, -0.5 / LIMB_SWING_SCALE, 1.0, sprint));
            let back = limb_targets(&pose(0.0, 0.5 / LIMB_SWING_SCALE, 1.0, sprint));
            assert!(forward.right_knee > 0.3 && back.right_knee < 0.15, "{} {}", forward.right_knee, back.right_knee);
            // A knee never bends the wrong way, nor an elbow.
            for l in [forward, back] {
                assert!(l.right_knee >= 0.0 && l.left_knee >= 0.0 && l.right_elbow >= 0.0 && l.left_elbow >= 0.0);
            }
        }
        // Running, the arms pump bent.
        let run = limb_targets(&pose(0.0, 0.0, 1.0, 1.0));
        assert!(run.left_elbow > 0.4 && run.right_elbow > 0.4);
    }
}
