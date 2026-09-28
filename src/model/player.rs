//! Blocky player character (head, body, arms, legs) with Minecraft's walk, run, sneak
//! and attack animations.
//! Model space is in pixels (1 px = 1/16 of the model height unit), Y up, facing -Z.

use super::emit_box;
use crate::item::{icon, tool_of, Icon, ItemId, NONE, STICK};
use crate::util::vertex_light;
use crate::world::mesh::{flags, Vertex};
use crate::world::textures::tex;
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
    pub gun: super::pistol_view::GunAnim,
    /// What is worn (`item::armor_code`).
    pub armor: u16,
    /// Holding the guide book open: its pages (see `book::BookView`).
    pub book: Option<super::book::BookView>,
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

/// Rotations (x, y, z in radians) of the arms and legs, applied Z·Y·X like Minecraft's ModelPart.
#[derive(Clone, Copy, Default)]
pub struct Limbs {
    pub right_arm: Vec3,
    pub left_arm: Vec3,
    pub right_leg: Vec3,
    pub left_leg: Vec3,
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

    // Arms: swing opposite to the legs, breathe while idle, reach forward when sneaking.
    let idle_z = (p.time * 1.8).cos() * 0.05 + 0.05;
    let idle_x = (p.time * 1.34).sin() * 0.05;
    let mut right_x = (ls + PI).cos() * la + idle_x + 0.4 * c;
    let left_x = ls.cos() * la - idle_x + 0.4 * c;
    let mut right_z = idle_z;
    let mut right_y = attack_twist(a);
    if p.blocking {
        // Minecraft 1.8's blocking pose: the arm brings the sword across the chest.
        right_x = right_x * 0.5 + PI * 0.3;
        right_y += std::f32::consts::FRAC_PI_6;
    } else if p.held != NONE {
        right_x = right_x * 0.5 + PI / 10.0;
    }
    if swinging {
        let f = 1.0 - (1.0 - a).powi(4);
        right_x += (f * PI).sin() * 1.2 + (a * PI).sin() * (0.7 + p.pitch) * 0.75;
        right_z -= (a * PI).sin() * 0.4;
    }
    let mut l = Limbs {
        right_arm: Vec3::new(right_x, right_y, right_z),
        left_arm: Vec3::new(left_x, 0.0, -idle_z),
        right_leg: Vec3::new(ls.cos() * 1.4 * la, 0.0, 0.0),
        left_leg: Vec3::new((ls + PI).cos() * 1.4 * la, 0.0, 0.0),
    };

    if p.burning {
        // BurningAnimation: both arms thrown up above the head, flailing.
        let h = flail(p.time);
        if !swinging {
            l.right_arm = Vec3::new(2.6 - h, 0.2, -0.3);
        }
        l.left_arm = Vec3::new(2.6 + h, -0.2, 0.3);
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
    } else if let (Some(_), false) = (p.book, swinging) {
        // Holding the open book: both hands hold it by its sides in front of the chest (each
        // hand the side on its own side, also when it is turned around to show it).
        let shoulder_y = 22.0 - 3.2 * c;
        let at = book_on_model(p);
        let (a, b) = (
            at.transform_point3(Vec3::new(5.5, 0.0, 1.5)),
            at.transform_point3(Vec3::new(-5.5, 0.0, 1.5)),
        );
        let (right, left) = if a.x >= b.x { (a, b) } else { (b, a) };
        l.right_arm = reach(Vec3::new(5.0, shoulder_y, 0.0), right);
        l.left_arm = reach(Vec3::new(-5.0, shoulder_y, 0.0), left);
    } else if let (Some(kind), false, false) =
        (crate::item::GunKind::of(p.held), swinging, p.blocking)
    {
        // Holding a gun: both arms reach for it where it is (it turns with the head), the
        // right hand to the grip, the left to the handguard (or the grip, a pistol).
        let spec = super::gun::spec(kind);
        let g = gun_on_model(p, kind);
        let shoulder_y = 22.0 - 3.2 * c;
        let right_hand = g.transform_point3(spec.hand);
        // Reloading, the left hand takes the old magazine out and brings the new one.
        let reloading = p.gun.reload_time();
        let mag = reloading
            .filter(|&t| (super::pistol_view::RELOAD_MAG_OUT - 0.1..super::pistol_view::RELOAD_MAG_IN + 0.1).contains(&t))
            .and_then(|_| {
                let (mats, _) = pistol_matrices(p, g);
                super::pistol_view::magazine_bottom(&mats)
            });
        let left_hand = mag.unwrap_or_else(|| {
            g.transform_point3(spec.support.unwrap_or(spec.hand + Vec3::new(0.6, -1.2, -2.2)))
        });
        l.right_arm = reach(Vec3::new(5.0, shoulder_y, 0.0), right_hand);
        l.left_arm = reach(Vec3::new(-5.0, shoulder_y, 0.0), left_hand);
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
        let next = match self.cur {
            Some(c) => Limbs {
                right_arm: blend(target.right_arm, c.right_arm),
                left_arm: blend(target.left_arm, c.left_arm),
                right_leg: blend(target.right_leg, c.right_leg),
                left_leg: blend(target.left_leg, c.left_leg),
            },
            None => target,
        };
        self.cur = Some(next);
        next
    }
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
    let root = Mat4::from_translation(p.pos)
        * Mat4::from_rotation_y(-p.body_yaw - PI / 2.0)
        * Mat4::from_scale(Vec3::splat(PX));
    let box_ = |out: &mut Vec<Vertex>, m: Mat4, min: [f32; 3], max: [f32; 3], layers: [u32; 6]| {
        emit_box(
            out,
            m,
            Vec3::from(min),
            Vec3::from(max),
            layers.map(|layer| crate::world::textures::skin_layer(layer, p.skin)),
            tints,
            light,
            fl,
        );
    };

    let c = p.crouch;
    let torso = root * Mat4::from_rotation_y(attack_twist(p.attack));

    // Head: sinks with the shoulders while sneaking, shakes while burning.
    let shake = if p.burning { flail(p.time) } else { 0.0 };
    let head = root
        * t(0.0, 24.0 - 4.2 * c, 0.0)
        * Mat4::from_rotation_y(-(p.head_yaw - p.body_yaw) + shake)
        * Mat4::from_rotation_x(p.pitch);
    if !p.first_person {
        box_(out, head, [-4.0, 0.0, -4.0], [4.0, 8.0, 4.0], HEAD);
    }

    // Body: hangs from the neck and leans forward while sneaking (hips go back).
    let body = torso * t(0.0, 24.0 - 3.2 * c, 0.0) * Mat4::from_rotation_x(-0.5 * c);
    box_(out, body, [-4.0, -12.0, -2.0], [4.0, 0.0, 2.0], BODY);

    let shoulder_y = 22.0 - 3.2 * c;
    let right = torso * t(5.0, shoulder_y, 0.0) * rot(limbs.right_arm);
    let left = torso * t(-5.0, shoulder_y, 0.0) * rot(limbs.left_arm);
    // The right arm (and what it holds) can be left out alone: the first-person hand shows
    // it instead while the left arm stays on the body.
    let show_right = !p.hide_arms && !p.hide_right_arm;
    if show_right {
        box_(out, right, [-1.0, -10.0, -2.0], [3.0, 2.0, 2.0], ARM);
    }
    if !p.hide_arms {
        box_(out, left, [-3.0, -10.0, -2.0], [1.0, 2.0, 2.0], ARM);
    }

    // Legs: pushed back under the hips while sneaking.
    let hip = Vec3::new(1.9, 12.0 - 0.2 * c, 4.0 * c);
    let rl = root * t(hip.x, hip.y, hip.z) * rot(limbs.right_leg);
    box_(out, rl, [-2.0, -12.0, -2.0], [2.0, 0.0, 2.0], LEG);
    let ll = root * t(-hip.x, hip.y, hip.z) * rot(limbs.left_leg);
    box_(out, ll, [-2.0, -12.0, -2.0], [2.0, 0.0, 2.0], LEG);

    // Armor over the body: a helmet (the face left free), a chestplate with shoulder pads,
    // leggings from the hips, boots, and the vest over the chest with its pouches.
    let (worn, vest) = crate::item::unpack_armor(p.armor);
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
    if let (Some(m), false) = (worn[0], p.first_person) {
        piece(out, head, [-4.6, 4.6, -4.6], [4.6, 8.7, 4.6], m);
        piece(out, head, [-4.6, 0.5, 1.2], [4.6, 4.6, 4.6], m);
        piece(out, head, [-4.6, 1.5, -4.6], [-3.6, 4.6, 1.2], m);
        piece(out, head, [3.6, 1.5, -4.6], [4.6, 4.6, 1.2], m);
    }
    if let Some(m) = worn[1] {
        piece(out, body, [-4.6, -10.8, -2.6], [4.6, 0.5, 2.6], m);
        if show_right {
            piece(out, right, [-1.6, -4.0, -2.6], [3.6, 2.6, 2.6], m);
        }
        if !p.hide_arms {
            piece(out, left, [-3.6, -4.0, -2.6], [1.6, 2.6, 2.6], m);
        }
    }
    if let Some(m) = worn[2] {
        piece(out, body, [-4.5, -12.4, -2.5], [4.5, -9.8, 2.5], m);
        piece(out, rl, [-2.5, -8.5, -2.5], [2.5, 0.3, 2.5], m);
        piece(out, ll, [-2.5, -8.5, -2.5], [2.5, 0.3, 2.5], m);
    }
    if let Some(m) = worn[3] {
        piece(out, rl, [-2.6, -12.4, -2.6], [2.6, -8.3, 2.6], m);
        piece(out, ll, [-2.6, -12.4, -2.6], [2.6, -8.3, 2.6], m);
    }
    if vest {
        let olive: [u8; 3] = std::array::from_fn(|i| ([118u32, 124, 92][i] * tint[i] as u32 / 255) as u8);
        let dark: [u8; 3] = olive.map(|c| (c as u32 * 4 / 5) as u8);
        let v = |out: &mut Vec<Vertex>, min: [f32; 3], max: [f32; 3], c: [u8; 3]| {
            emit_box(out, body, Vec3::from(min), Vec3::from(max), [tex::VEST; 6], [c; 6], light, fl);
        };
        v(out, [-4.9, -10.2, -3.0], [4.9, 0.6, 3.0], olive);
        for (x0, x1) in [(-3.8, -1.5), (-1.1, 1.1), (1.5, 3.8)] {
            v(out, [x0, -9.6, -3.7], [x1, -6.6, -3.0], dark);
        }
    }

    // Held item, placed like Minecraft's ItemInHandLayer followed by the item model's
    // `thirdperson_righthand` display transform (handheld tools, flat items, blocks).
    if p.held == crate::world::LANTERN as ItemId && show_right {
        // Hanging from the hand by its chain, swinging with its pendulum.
        let pivot = right.transform_point3(Vec3::new(1.0, -11.0, 0.0));
        let dir = p.lantern.unwrap_or(Vec3::NEG_Y);
        let style = crate::model::lantern::ON_MODEL;
        crate::model::lantern::emit_held_lantern(out, style, pivot, dir, p.body_yaw, light, fl);
    } else if let (Some(view), true) = (&p.book, show_right) {
        super::book::emit_open_book(out, root * book_on_model(p), view, light, fl);
    } else if let (Some(kind), true) = (crate::item::GunKind::of(p.held), show_right) {
        // The Blockbench pistol, its parts moving like in the first-person view.
        let (mats, shown) = pistol_matrices(p, root * gun_on_model(p, kind));
        let lamp = p.gun_mods & crate::item::gun_mod::LIGHT != 0 && p.gun_mods & crate::item::gun_mod::LIGHT_ON != 0;
        super::pistol_view::emit_pistol(out, Some(glass), &mats, &shown, false, p.gun_dirt, lamp, light, fl);
    } else if p.held != NONE && show_right {
        let st = crate::item::Stack { data: p.held_data, ..crate::item::Stack::one(p.held) };
        super::emit_held_data(out, held_item(p, right), &st, light, fl);
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
    t(0.0, 14.0 + 3.0 * read + 4.0 * e - 3.2 * p.crouch, -6.0 - 1.0 * read - 2.5 * e)
        * Mat4::from_rotation_y(PI * e)
        * Mat4::from_rotation_x(0.5 + 0.6 * read + 0.8 * e)
        * Mat4::from_scale(Vec3::splat(0.8))
}

/// Where a held gun is on the player model (model pixels from the feet, facing -Z), turned
/// with the head: held out in front at arm's length, a long gun on the right with its stock
/// back at the shoulder and its sights under the eye, a pistol in the middle in both hands.
pub fn gun_on_model(p: &PlayerPose, kind: crate::item::GunKind) -> Mat4 {
    let spec = super::gun::spec(kind);
    // The right fist on the grip, from the neck in the head's frame.
    let hip = if spec.support.is_some() {
        Vec3::new(3.0, -3.0, -9.5)
    } else {
        Vec3::new(0.8, -2.2, -9.6)
    };
    // Aimed, the arms stretch out and bring the sights (or the scope) up in front of the
    // right eye.
    let a = p.gun.aim.clamp(0.0, 1.0);
    let a = a * a * (3.0 - 2.0 * a);
    let grip = if a > 0.0 {
        let eye = Vec3::new(1.6, 3.5, -11.2);
        let sight = super::pistol_view::sight_above_hand(p.gun_mods).y * spec.arm_scale;
        hip.lerp(eye - Vec3::Y * sight, a)
    } else {
        hip
    };
    // Gun space to model space: the muzzle forward (-Z), its right side to the right (+X).
    let basis = Mat4::from_cols(
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::Y,
        glam::Vec4::X,
        glam::Vec4::W,
    );
    t(0.0, 24.0 - 4.2 * p.crouch, 0.0)
        * Mat4::from_rotation_y(-(p.head_yaw - p.body_yaw))
        * Mat4::from_rotation_x(p.pitch)
        * Mat4::from_translation(grip)
        * basis
        * Mat4::from_scale(Vec3::splat(spec.arm_scale))
        * Mat4::from_translation(-spec.hand)
}

/// The Blockbench pistol's bones held by the model: `gun` is the old gun space's transform
/// (`gun_on_model`, with the model's own root in front for the world).
fn pistol_matrices(p: &PlayerPose, gun: Mat4) -> (Vec<Mat4>, Vec<bool>) {
    let mut pose = super::pistol_view::rest_pose();
    super::pistol_view::add_gun_anims(&mut pose, &p.gun, true);
    super::pistol_view::apply_mods(&mut pose, p.gun_mods);
    super::viewmodel::bone_matrices(super::pistol_vm::BONES, &pose, gun * super::pistol_view::to_gun_space())
}

/// A point of the held gun (gun space) in the world.
pub fn gun_point(p: &PlayerPose, kind: crate::item::GunKind, point: Vec3) -> Vec3 {
    let root = Mat4::from_translation(p.pos)
        * Mat4::from_rotation_y(-p.body_yaw - PI / 2.0)
        * Mat4::from_scale(Vec3::splat(PX));
    (root * gun_on_model(p, kind)).transform_point3(point)
}

/// Arm rotation (as in `Limbs`) that points an arm hanging from `shoulder` at `target`.
fn reach(shoulder: Vec3, target: Vec3) -> Vec3 {
    let d = (target - shoulder).normalize_or(Vec3::NEG_Y);
    Vec3::new((-d.y).clamp(-1.0, 1.0).acos(), (-d.x).atan2(-d.z), 0.0)
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
    } else if super::is_model_item(p.held) {
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

/// Where a held torch's fire is (the tip of its glowing head), in the world.
pub fn held_torch_tip(p: &PlayerPose, limbs: &Limbs) -> Vec3 {
    held_item(p, right_arm(p, limbs)).transform_point3(TORCH_TIP)
}

/// The top of a torch's glowing head in the torch model (`emit_torch`).
pub const TORCH_TIP: Vec3 = Vec3::new(0.0, 0.17, 0.0);

/// Lights held up in front of the eyes (Not Enough Animations' pose for torches; lanterns too).
pub fn held_up(item: ItemId) -> bool {
    item == crate::world::TORCH as ItemId || item == crate::world::LANTERN as ItemId
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
    let root = Mat4::from_translation(p.pos)
        * Mat4::from_rotation_y(-p.body_yaw - PI / 2.0)
        * Mat4::from_scale(Vec3::splat(PX));
    let torso = root * Mat4::from_rotation_y(attack_twist(p.attack));
    torso * t(5.0, 22.0 - 3.2 * p.crouch, 0.0) * rot(limbs.right_arm)
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
        }
    }

    #[test]
    fn hands_hold_the_gun_that_points_where_the_head_looks() {
        for kind in GUN_KINDS {
            for (pitch, turn) in [(0.0, 0.0), (0.5, 0.3), (-0.6, -0.4)] {
                let p = pose(kind.item(), pitch, turn);
                let g = gun_on_model(&p, kind);
                let spec = crate::model::gun::spec(kind);
                // The muzzle is ahead of the grip, the way the head faces.
                let look = Mat4::from_rotation_y(-turn)
                    * Mat4::from_rotation_x(pitch)
                    * glam::Vec4::new(0.0, 0.0, -1.0, 0.0);
                use crate::model::pistol_view::{muzzle, rest_point_in_gun_space};
                let (bone, front) = muzzle(0);
                // Down the barrel: from ten pixels behind its end to its end.
                let back = rest_point_in_gun_space((bone, front + Vec3::Z * 10.0));
                let along = g.transform_point3(rest_point_in_gun_space((bone, front))) - g.transform_point3(back);
                assert!(along.normalize().dot(look.truncate()) > 0.97, "{kind:?}");
                // The right arm points at the grip; the fist ends within reach of it.
                let l = limb_targets(&p);
                let fist = Vec3::new(5.0, 22.0, 0.0) + rot(l.right_arm).transform_vector3(Vec3::new(0.0, -10.0, 0.0));
                let grip = g.transform_point3(spec.hand);
                assert!(fist.distance(grip) < 3.0, "{kind:?}: fist {fist}, grip {grip}");
            }
        }
    }
}
