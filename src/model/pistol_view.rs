//! The Blockbench pistol (`pistol_vm`) wherever it is seen: its own moving parts (the slide
//! flying back, the trigger, the round in the chamber, the magazine change) from what the
//! gun is doing, and drawing it with its attachments. The first-person hand (`hand`) adds the
//! arms' animations on top; the player model (`player`) holds it in its hands.

use super::pistol_vm as vm;
use super::viewmodel::{add_anim, cube_matrix, emit_cube, find_anim, find_bone, sample, Anim, BonePose};
use crate::item::gun_mod;
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{Mat4, Vec3};

/// Moments of the reload animation (seconds): the old magazine drops out, the new one is in,
/// the slide slams shut.
pub const RELOAD_MAG_OUT: f32 = 0.35;
pub const RELOAD_MAG_IN: f32 = 1.2;
pub const RELOAD_SLIDE: f32 = 1.6;
/// The left hand is back on the grip with the new magazine in (a reload without the slide
/// ends here); the old magazine has fallen (taking it out without a new one ends here); the
/// left hand reaches for the new one (putting one into an empty gun starts here); the left
/// hand goes for the slide (just chambering a round starts here).
const RELOAD_IN_END: f32 = 1.35;
const RELOAD_OUT_END: f32 = 0.6;
const RELOAD_FETCH: f32 = 0.62;
const RELOAD_RACK: f32 = 1.32;
/// The left hand starts pulling the slide back, and has it all the way back (the animation
/// has the slide already back before, held there by an empty magazine).
const RELOAD_PULL: f32 = 1.45;
const RELOAD_PULLED: f32 = 1.55;

/// The bones that are the arms and what holds the pistol, not the pistol's own parts.
const HOLDING: [&str; 5] = ["viewmodel", "right_arm", "right_arm_mesh", "left_arm", "left_arm_mesh"];

/// What the R key does with the pistol.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ReloadKind {
    /// The magazine in it drops out and a new one goes in.
    #[default]
    Swap,
    /// There is no other: the magazine in it only drops out.
    Eject,
    /// There is none in it: a new one goes in.
    Insert,
    /// The slide is pulled back and let go (a round from the magazine into the chamber).
    Rack,
}

/// The part of the reload animation a reload plays (seconds from, to): the whole of it, or
/// only the magazine going out, or only coming in, or only the slide; `rack`: with the slide
/// pulled and let go at the end.
fn reload_span(kind: ReloadKind, rack: bool) -> (f32, f32) {
    let length = anim("reload").map_or(2.0, |a| a.length);
    let end = if rack { length } else { RELOAD_IN_END.min(length) };
    match kind {
        ReloadKind::Swap => (0.0, end),
        ReloadKind::Eject => (0.0, RELOAD_OUT_END),
        ReloadKind::Insert => (RELOAD_FETCH, end),
        ReloadKind::Rack => (RELOAD_RACK, length),
    }
}

/// Seconds a reload goes on after its part of the animation, while the arms ease back to
/// holding the gun: one that ends before the animation does (no slide pulled, or only the
/// magazine dropped) would otherwise jump there.
fn settle_seconds(kind: ReloadKind, rack: bool) -> f32 {
    let (_, b) = reload_span(kind, rack);
    let length = anim("reload").map_or(2.0, |a| a.length);
    if b < length - 1e-3 {
        0.3
    } else {
        0.0
    }
}

/// How long a reload takes (seconds): its part of the animation, and the arms settling back
/// when it ends early.
pub fn reload_seconds(kind: ReloadKind, rack: bool) -> f32 {
    let (a, b) = reload_span(kind, rack);
    (b - a) + settle_seconds(kind, rack)
}

/// Where the reload animation is (seconds) this far (0..1) into a reload.
pub fn reload_anim_time(p: f32, kind: ReloadKind, rack: bool) -> f32 {
    let (a, b) = reload_span(kind, rack);
    let p = p.clamp(0.0, 1.0);
    let k = (p * reload_seconds(kind, rack) / (b - a)).min(1.0);
    a + (b - a) * k
}

/// How much of the reload animation shows (0..1): a reload starting in the middle of it eases
/// in, one ending before its end eases back out while it settles.
fn reload_weight(p: f32, kind: ReloadKind, rack: bool) -> f32 {
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let total = reload_seconds(kind, rack);
    let settle = settle_seconds(kind, rack) / total;
    let fade_in = match kind {
        ReloadKind::Swap | ReloadKind::Eject => 1.0,
        ReloadKind::Insert | ReloadKind::Rack => smooth(p / 0.18),
    };
    let fade_out = if settle > 0.0 { 1.0 - smooth((p - (1.0 - settle)) / settle) } else { 1.0 };
    fade_in * fade_out
}

/// What the gun is doing.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct GunAnim {
    /// Seconds since it last fired (while that is still moving), and since the trigger was
    /// pulled on an empty chamber (only the trigger moves).
    pub shot: Option<f32>,
    pub dry: Option<f32>,
    /// How far a reload is (0..1), what it does, whether the slide is pulled at its end, and
    /// whether the magazine taken out is empty.
    pub reload: Option<f32>,
    pub kind: ReloadKind,
    pub rack: bool,
    pub old_empty: bool,
    /// The slide is held back (an empty magazine, after its last round); there is no
    /// magazine in it; a round is in the chamber.
    pub locked: bool,
    pub no_mag: bool,
    pub chambered: bool,
    /// Aimed (0 from the hip .. 1 aimed down the sights or the scope).
    pub aim: f32,
}

impl GunAnim {
    /// Packed for the others (`net::Pose::gun_state`): the reload's progress (1..=63; 0 not
    /// reloading), its kind, the slide pulled, the old magazine empty, the slide held back,
    /// no magazine, nothing in the chamber, how far aimed (0..7). The shot comes with its own
    /// message.
    pub fn pack(&self) -> u16 {
        let progress = self.reload.map_or(0, |p| 1 + (p.clamp(0.0, 1.0) * 62.0).round() as u16);
        let kind = match self.kind {
            ReloadKind::Swap => 0,
            ReloadKind::Eject => 1,
            ReloadKind::Insert => 2,
            ReloadKind::Rack => 3,
        };
        progress
            | kind << 6
            | (self.rack as u16) << 8
            | (self.old_empty as u16) << 9
            | (self.locked as u16) << 10
            | (self.no_mag as u16) << 11
            | (!self.chambered as u16) << 12
            | ((self.aim.clamp(0.0, 1.0) * 7.0).round() as u16) << 13
    }

    pub fn unpack(b: u16, shot: Option<f32>) -> Self {
        let progress = b & 0x3f;
        GunAnim {
            shot,
            dry: None,
            reload: (progress > 0).then(|| (progress - 1) as f32 / 62.0),
            kind: [ReloadKind::Swap, ReloadKind::Eject, ReloadKind::Insert, ReloadKind::Rack][(b >> 6 & 3) as usize],
            rack: b & 1 << 8 != 0,
            old_empty: b & 1 << 9 != 0,
            locked: b & 1 << 10 != 0,
            no_mag: b & 1 << 11 != 0,
            chambered: b & 1 << 12 == 0,
            aim: (b >> 13) as f32 / 7.0,
        }
    }

    /// Where the reload animation is now (seconds), while reloading.
    pub fn reload_time(&self) -> Option<f32> {
        self.reload.map(|p| reload_anim_time(p, self.kind, self.rack))
    }
}

fn anim(name: &str) -> Option<&'static Anim> {
    find_anim(vm::ANIMS, name)
}

fn bone(name: &str) -> Option<usize> {
    find_bone(vm::BONES, name)
}

/// A channel's value (0 position, 1 rotation, 2 scale) of one bone in an animation at `t`.
fn channel_at(anim: &Anim, bone: usize, kind: u8, t: f32) -> Vec3 {
    anim.channels
        .iter()
        .find(|c| c.bone == bone && c.kind == kind)
        .map_or(Vec3::ZERO, |c| sample(c, t, false))
}

/// When a bone's position is furthest back (+Z) in an animation: the slide thrown back.
fn slide_back_time(anim: &Anim, bone: usize) -> f32 {
    anim.channels
        .iter()
        .find(|c| c.bone == bone && c.kind == 0)
        .and_then(|c| c.keys.iter().max_by(|a, b| a.v[2].total_cmp(&b.v[2])))
        .map_or(0.0, |k| k.t)
}

/// A pose with nothing moved.
pub fn rest_pose() -> Vec<BonePose> {
    vec![BonePose::default(); vm::BONES.len()]
}

/// Adds the shot and the reload to the pose; the slide held back, the chamber empty and the
/// magazine missing as the gun is. With `parts_only` the arms and what holds the pistol are
/// left still (the player model holds it with its own arms).
pub fn add_gun_anims(pose: &mut [BonePose], g: &GunAnim, parts_only: bool) {
    let holding: Vec<usize> = if parts_only { HOLDING.iter().filter_map(|n| bone(n)).collect() } else { Vec::new() };
    let slide = bone("slide");
    let chamber = bone("chambered_round");
    let rounds = bone("magazine_rounds");
    let reloading = g.reload.is_some();
    if let (Some(p), Some(an)) = (g.reload, anim("reload")) {
        // Without the slide being pulled, the slide and the chamber stay as they are.
        let t = reload_anim_time(p, g.kind, g.rack);
        let own = |b: usize| {
            holding.contains(&b) || Some(b) == rounds || (!g.rack && (Some(b) == slide || Some(b) == chamber))
        };
        add_anim(pose, an, t, reload_weight(p, g.kind, g.rack), own);
        // A slide that is forward (not held back) stays forward until the hand pulls it.
        if let (Some(s), true, false) = (slide, g.rack, g.locked) {
            if t < RELOAD_PULLED {
                let k = ((t - RELOAD_PULL) / (RELOAD_PULLED - RELOAD_PULL)).clamp(0.0, 1.0);
                pose[s].pos *= k * k * (3.0 - 2.0 * k);
            }
        }
        // The rounds on top of the magazine: the old one's until it is out (none if it was
        // empty), then the new one's.
        if let (Some(r), true) = (rounds, g.old_empty && t < RELOAD_FETCH) {
            pose[r].scale = Vec3::ZERO;
        }
    }
    if let (Some(st), Some(an)) = (g.shot, anim("shoot")) {
        if st < an.length {
            add_anim(pose, an, st, 1.0, |b| holding.contains(&b));
        }
    }
    if let (Some(dt), Some(an), Some(trigger)) = (g.dry, anim("shoot"), bone("trigger")) {
        if dt < an.length {
            add_anim(pose, an, dt, 1.0, |b| b != trigger);
        }
    }
    if !(reloading && g.rack) {
        // Held back on an empty magazine (once the last shot has thrown it back).
        if let (Some(s), true) = (slide, g.locked) {
            let back = anim("shoot").map_or(0.0, |an| slide_back_time(an, s));
            if g.shot.map_or(true, |st| st >= back) {
                pose[s].pos = anim("reload").map_or(Vec3::ZERO, |an| channel_at(an, s, 0, 0.0));
            }
        }
        if let (Some(c), false) = (chamber, g.chambered) {
            pose[c].scale = Vec3::ZERO;
        }
    }
    if let (Some(m), true, false) = (bone("magazine_mesh"), g.no_mag, reloading) {
        pose[m].scale = Vec3::ZERO;
    }
}

/// The attachments are groups of the model: shown only when fitted (the extended magazine
/// in place of the standard one's base plate).
pub fn apply_mods(pose: &mut [BonePose], mods: u8) {
    let ext = mods & gun_mod::EXTENDED_MAGAZINE != 0;
    for (name, shown) in [
        ("silencer", mods & gun_mod::SILENCER != 0),
        ("laser", mods & gun_mod::LASER != 0),
        ("flashlight", mods & gun_mod::LIGHT != 0),
        ("scope", mods & gun_mod::SCOPE != 0),
        // A scope takes the rear sight's place.
        ("rear_sight", mods & gun_mod::SCOPE == 0),
        ("mag_extended", ext),
        ("mag_standard", !ext),
    ] {
        if let (Some(b), false) = (bone(name), shown) {
            pose[b].scale = Vec3::ZERO;
        }
    }
}

/// How dirty a gun looks (0 clean .. `tex::PISTOL_DIRT_LEVELS - 1`) with this much dirt
/// (`Stack::damage`) of `max`.
pub fn dirt_level(damage: u16, max: u16) -> u8 {
    if damage == 0 || max == 0 {
        return 0;
    }
    let top = tex::PISTOL_DIRT_LEVELS - 1;
    ((damage as f32 / max as f32 * top as f32).ceil() as u32).clamp(1, top) as u8
}

/// The first texture layer of the pistol's pages as dirty as `dirt` (`dirt_level`).
pub fn layers(dirt: u8) -> u32 {
    tex::PISTOL_VIEW + (dirt as u32).min(tex::PISTOL_DIRT_LEVELS - 1) * vm::PAGES
}

/// The pistol's cubes (with the attachments `apply_mods` left shown). The see-through glass
/// (the scope's lenses) goes to `glass` to be drawn blended after the rest, when given; the
/// eyepiece's glass is left out with `eyepiece_view` (the scope's view shows there instead).
pub fn emit_pistol(
    out: &mut Vec<Vertex>,
    mut glass: Option<&mut Vec<Vertex>>,
    mats: &[Mat4],
    shown: &[bool],
    eyepiece_view: bool,
    dirt: u8,
    lamp: bool,
    light: [u8; 4],
    fl: u8,
) {
    let first = layers(dirt);
    for c in vm::CUBES {
        // The weapon light's lens glows while it is on.
        let fl = if lamp && c.name == "flashlight_lens" { fl | crate::world::mesh::flags::EMISSIVE } else { fl };
        if !shown[c.bone] {
            continue;
        }
        let m = mats[c.bone] * cube_matrix(c);
        if c.name.starts_with("scope_lens_") {
            if eyepiece_view && c.name.starts_with("scope_lens_back") {
                continue;
            }
            if let Some(g) = glass.as_deref_mut() {
                emit_cube(g, c, m, first, light, fl);
                continue;
            }
        }
        emit_cube(out, c, m, first, light, fl);
    }
}

/// The scope's eyepiece (its back lens) where `mats` put it: its middle, its right and up
/// directions (unit) and its radius, when a scope is shown.
pub fn eyepiece(mats: &[Mat4], shown: &[bool]) -> Option<(Vec3, Vec3, Vec3, f32)> {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    let mut bone = None;
    for c in vm::CUBES.iter().filter(|c| c.name.starts_with("scope_lens_back")) {
        lo = lo.min(Vec3::from(c.from)).min(Vec3::from(c.to));
        hi = hi.max(Vec3::from(c.from)).max(Vec3::from(c.to));
        bone = Some(c.bone);
    }
    let b = bone.filter(|&b| shown[b])?;
    let m = mats[b];
    let right = m.transform_vector3(Vec3::X);
    let scale = right.length();
    Some((
        m.transform_point3((lo + hi) * 0.5),
        right / scale,
        m.transform_vector3(Vec3::Y).normalize(),
        (hi.x - lo.x) * 0.5 * scale,
    ))
}

/// The pistol on the gun station's table, taken apart ("strip") and with attachments going on
/// ("fit_*"): which bones each part is, and the part's window of the strip animation.
pub mod bench {
    use super::super::gun::{BARREL, FRAME, MAGAZINE, PARTS, SLIDE, SPRING};
    use super::super::viewmodel::{add_anim, find_anim, BonePose};
    use super::{bone, vm};
    use crate::item::gun_mod;

    /// A set of bones (bit i: bone i).
    pub type Bones = u64;

    fn subtree(name: &str) -> Bones {
        let Some(root) = bone(name) else { return 0 };
        let mut set: Bones = 1 << root;
        // Parents come before their children.
        for (i, b) in vm::BONES.iter().enumerate() {
            if b.parent >= 0 && set & (1 << b.parent) != 0 {
                set |= 1 << i;
            }
        }
        set
    }

    /// The bones an attachment is (`gun_mod` bit; several bits: all of them).
    pub fn attachment(bits: u8) -> Bones {
        [
            (gun_mod::SCOPE, "scope"),
            (gun_mod::SILENCER, "silencer"),
            (gun_mod::LASER, "laser"),
            (gun_mod::LIGHT, "flashlight"),
        ]
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .fold(0, |b, (_, name)| b | subtree(name))
    }

    /// The attachments that sit on a part.
    fn attachments_on(part: usize) -> u8 {
        match part {
            FRAME => gun_mod::RAIL,
            BARREL => gun_mod::SILENCER,
            SLIDE => gun_mod::SCOPE,
            _ => 0,
        }
    }

    /// The bones a part is (`gun::FRAME` ..), without the attachments on it; with `mods`,
    /// those fitted on it too.
    pub fn part(part: usize, mods: u8) -> Bones {
        let own = match part {
            FRAME => subtree("frame") | subtree("grip"),
            BARREL => subtree("barrel") | subtree("chambered_round"),
            SPRING => subtree("recoil_spring"),
            SLIDE => subtree("slide"),
            MAGAZINE => subtree("magazine"),
            _ => 0,
        };
        let on = attachments_on(part);
        let all_on = attachment(on);
        (own & !all_on) | if mods & on != 0 { all_on } else { 0 }
    }

    /// Whether a cube of a magazine with `rounds` of `cap` in it shows: the brass in its
    /// witness holes and the rounds on top only as far as there are rounds.
    pub fn mag_cube_shown(name: &str, rounds: u8, cap: u8) -> bool {
        if let Some(k) = name.strip_prefix("witness_brass_").and_then(|r| r.get(1..)?.parse::<u32>().ok()) {
            // Hole k (from the top) shows brass once the rounds reach down to it.
            return (rounds as u32) * 7 >= (k + 1) * cap.max(1) as u32;
        }
        if name.starts_with("mag_round_top") {
            return rounds >= 1;
        }
        if name.starts_with("mag_round_2") {
            return rounds >= 2;
        }
        true
    }

    /// The pistol's bones an item is when it is on its own (a part, an attachment, a
    /// magazine, a round), posed as it lies, with a magazine's rounds (in it, of how many it
    /// holds), and how to turn the model for the item: the muzzle end to +X, its right side
    /// toward +Z (a magazine straightened from the grip's slant, stood up).
    pub fn item_rig(st: &crate::item::Stack) -> Option<(Bones, Vec<BonePose>, Option<(u8, u8)>, glam::Mat4)> {
        use crate::item::*;
        use glam::Mat4;
        let side = Mat4::from_rotation_y((-90f32).to_radians());
        let part = |p: usize, mods: u8| (self::part(p, mods), pose([0.0; PARTS], mods, false, false), None, side);
        Some(match st.item {
            PISTOL_FRAME => part(FRAME, gun_mods(st) & gun_mod::RAIL),
            PISTOL_BARREL => part(BARREL, gun_mods(st) & gun_mod::SILENCER),
            PISTOL_SPRING => part(SPRING, 0),
            PISTOL_SLIDE => part(SLIDE, gun_mods(st) & gun_mod::SCOPE),
            PISTOL_MAGAZINE | EXTENDED_MAGAZINE => {
                let cap = magazine_capacity(st.item).unwrap_or(12);
                let rounds = gun_rounds(st);
                let ext = if st.item == EXTENDED_MAGAZINE { gun_mod::EXTENDED_MAGAZINE } else { 0 };
                let upright = side * Mat4::from_rotation_x(18f32.to_radians());
                (self::part(MAGAZINE, 0), pose([0.0; PARTS], ext, rounds > 0, false), Some((rounds, cap)), upright)
            }
            SCOPE => (attachment(gun_mod::SCOPE), pose([0.0; PARTS], gun_mod::SCOPE, false, false), None, side),
            SILENCER => (attachment(gun_mod::SILENCER), pose([0.0; PARTS], gun_mod::SILENCER, false, false), None, side),
            LASER_SIGHT => (attachment(gun_mod::LASER), pose([0.0; PARTS], gun_mod::LASER, false, false), None, side),
            FLASHLIGHT => (attachment(gun_mod::LIGHT), pose([0.0; PARTS], gun_mod::LIGHT, false, false), None, side),
            BULLET => (round(), pose([0.0; PARTS], 0, false, true), None, side),
            _ => return None,
        })
    }

    /// The round in the chamber (a cartridge on its own).
    pub fn round() -> Bones {
        subtree("chambered_round")
    }

    /// The whole gun without its attachments, or with `mods` fitted.
    pub fn gun(mods: u8) -> Bones {
        (0..PARTS).fold(0, |b, p| b | part(p, mods))
    }

    /// The strip animation's length: every part off.
    pub fn strip_length() -> f32 {
        find_anim(vm::ANIMS, "strip").map_or(2.4, |a| a.length)
    }

    /// The bones a part's own animation moves.
    fn moved(part: usize) -> Bones {
        match part {
            BARREL => subtree("barrel") | subtree("chambered_round"),
            FRAME => 0,
            p => self::part(p, 0xff),
        }
    }

    /// The pose with each part as far into the strip animation as `at` says (seconds), the
    /// attachments in `mods` shown (and the extended magazine with its bit), the magazine's
    /// top rounds and the chambered round shown or not.
    pub fn pose(at: [f32; PARTS], mods: u8, rounds: bool, chambered: bool) -> Vec<BonePose> {
        let mut pose = super::rest_pose();
        if let Some(an) = find_anim(vm::ANIMS, "strip") {
            for (p, &t) in at.iter().enumerate() {
                let m = moved(p);
                if m != 0 && t > 0.0 {
                    add_anim(&mut pose, an, t, 1.0, |b| m & (1 << b) == 0);
                }
            }
        }
        super::apply_mods(&mut pose, mods);
        for (name, show) in [("magazine_rounds", rounds), ("chambered_round", chambered)] {
            if let (Some(b), false) = (bone(name), show) {
                pose[b].scale = glam::Vec3::ZERO;
            }
        }
        pose
    }

    /// An attachment's own animation onto the gun ("fit_scope" ..) and its length.
    pub fn fit_anim(bit: u8) -> Option<&'static super::super::viewmodel::Anim> {
        let name = match bit {
            gun_mod::SCOPE => "fit_scope",
            gun_mod::SILENCER => "fit_silencer",
            gun_mod::LASER => "fit_laser",
            gun_mod::LIGHT => "fit_flashlight",
            _ => return None,
        };
        find_anim(vm::ANIMS, name)
    }

    /// Adds an attachment going on, `t` seconds into its animation.
    pub fn add_fit(pose: &mut [BonePose], bit: u8, t: f32) {
        if let Some(an) = fit_anim(bit) {
            let m = attachment(bit);
            add_anim(pose, an, t, 1.0, |b| m & (1 << b) == 0);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_parts_are_the_whole_gun_once_each() {
            let mut seen: Bones = 0;
            for p in 0..PARTS {
                let b = part(p, 0);
                assert!(b != 0, "part {p}");
                assert_eq!(seen & b, 0, "part {p} shares bones");
                seen |= b;
            }
            for bit in [gun_mod::SCOPE, gun_mod::SILENCER, gun_mod::LASER, gun_mod::LIGHT] {
                assert_eq!(seen & attachment(bit), 0);
                assert!(gun(bit) & attachment(bit) != 0);
            }
            // Every drawn cube is in a part or an attachment.
            let all = gun(0xff);
            for c in vm::CUBES {
                assert!(all & (1 << c.bone) != 0, "{} is in no part", c.name);
            }
        }
    }
}

/// Where the bullet leaves (the silencer's end when there is one): bone and model point.
pub fn muzzle(mods: u8) -> (usize, Vec3) {
    let (b, p) = if mods & gun_mod::SILENCER != 0 { vm::SILENCED } else { vm::MUZZLE };
    (b, Vec3::from(p))
}

/// Where the spent case comes out (the pistol's own bone: the chambered round's is hidden
/// while the chamber is empty).
pub fn eject() -> (usize, Vec3) {
    (bone("pistol").unwrap_or(vm::EJECT.0), Vec3::from(vm::EJECT.1))
}

/// Where the weapon light's light comes from (its lens).
pub fn light() -> (usize, Vec3) {
    let (b, p) = vm::LIGHT;
    (b, Vec3::from(p))
}

/// Where the laser sight's beam starts.
pub fn laser() -> (usize, Vec3) {
    let (b, p) = vm::LASER;
    (b, Vec3::from(p))
}

/// The bottom of the magazine's base plate, where a hand holds it (world, with `mats`).
pub fn magazine_bottom(mats: &[Mat4]) -> Option<Vec3> {
    let c = vm::CUBES.iter().find(|c| c.name == "mag_base")?;
    let c = if mats[c.bone].x_axis.length() < 1e-4 {
        vm::CUBES.iter().find(|c| c.name == "ext_base")?
    } else {
        c
    };
    let p = Vec3::new((c.from[0] + c.to[0]) * 0.5, c.from[1].min(c.to[1]), (c.from[2] + c.to[2]) * 0.5);
    Some((mats[c.bone] * cube_matrix(c)).transform_point3(p))
}

/// From the Blockbench model's space to the old gun space (`gun::Spec`: the muzzle +X, the
/// right side +Z, about a centimetre a unit), so the pistol sits where the old one did: the
/// right fist's middle on the grip at the spec's `hand`, the same length.
pub fn to_gun_space() -> Mat4 {
    let spec = super::gun::spec(crate::item::GunKind::Pistol);
    let fist = bone("right_arm_mesh").map_or(Vec3::ZERO, |b| Vec3::from(vm::BONES[b].origin));
    // The old pistol is 18.2 gun units long, the Blockbench one 21.8 pixels.
    let scale = 18.2 / 21.8;
    Mat4::from_translation(spec.hand)
        * Mat4::from_rotation_y((-90.0f32).to_radians())
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-fist)
}

/// The line of sight (the scope's axis with one, else over the iron sights) above the right
/// fist, in the old gun space.
pub fn sight_above_hand(mods: u8) -> Vec3 {
    let y = if mods & gun_mod::SCOPE != 0 { -8.5 } else { -9.7 };
    let fist = bone("right_arm_mesh").map_or(Vec3::ZERO, |b| Vec3::from(vm::BONES[b].origin));
    let pistol = bone("pistol").unwrap_or(0);
    rest_point_in_gun_space((pistol, Vec3::new(fist.x, y, fist.z))) - rest_point_in_gun_space((pistol, fist))
}

/// A model point of the pistol at rest, in the old gun space (for the third-person muzzle,
/// ejection port and laser, see `player::gun_point`).
pub fn rest_point_in_gun_space((b, p): (usize, Vec3)) -> Vec3 {
    let (mats, _) = super::viewmodel::bone_matrices(vm::BONES, &rest_pose(), to_gun_space());
    mats[b].transform_point3(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gun_state_goes_through_the_network() {
        let g = GunAnim {
            shot: None,
            dry: None,
            reload: Some(0.5),
            kind: ReloadKind::Insert,
            rack: true,
            old_empty: true,
            locked: true,
            no_mag: false,
            chambered: false,
            aim: 0.0,
        };
        let back = GunAnim::unpack(g.pack(), None);
        assert!((back.reload.unwrap() - 0.5).abs() < 0.02);
        assert_eq!(GunAnim { reload: g.reload, ..back }, g);
        let aimed = GunAnim { aim: 1.0, ..g };
        assert_eq!(GunAnim::unpack(aimed.pack(), None).aim, 1.0);
        let idle = GunAnim { chambered: true, ..GunAnim::default() };
        assert_eq!(GunAnim::unpack(idle.pack(), None), idle);
    }

    #[test]
    fn every_reload_plays_its_moments() {
        use ReloadKind::*;
        let crosses = |kind, rack, at: f32| {
            reload_anim_time(0.0, kind, rack) <= at && reload_anim_time(1.0, kind, rack) >= at
        };
        assert!(crosses(Swap, true, RELOAD_MAG_OUT) && crosses(Swap, true, RELOAD_MAG_IN) && crosses(Swap, true, RELOAD_SLIDE));
        assert!(crosses(Swap, false, RELOAD_MAG_IN) && !crosses(Swap, false, RELOAD_SLIDE));
        assert!(crosses(Eject, false, RELOAD_MAG_OUT) && !crosses(Eject, false, RELOAD_MAG_IN));
        assert!(!crosses(Insert, true, RELOAD_MAG_OUT) && crosses(Insert, true, RELOAD_MAG_IN) && crosses(Insert, true, RELOAD_SLIDE));
        assert!(crosses(Rack, true, RELOAD_SLIDE) && !crosses(Rack, true, RELOAD_MAG_IN));
    }

    #[test]
    fn the_pistol_sits_where_the_old_one_did() {
        // The muzzle out in front (+X) of the grip, about where the old one's was.
        let m = rest_point_in_gun_space(muzzle(0));
        let old = Vec3::new(8.6, 3.0, 0.0);
        assert!((m - old).length() < 4.0, "{m} vs {old}");
        let e = rest_point_in_gun_space(eject());
        assert!(e.x < m.x && e.y > 0.0, "{e}");
    }
}
