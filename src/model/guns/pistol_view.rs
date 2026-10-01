//! The magazine-fed guns made in Blockbench, the pistol (`pistol_vm`) and the AK-47
//! (`ak_vm`), wherever they are seen: their own moving parts (the slide or bolt carrier flying
//! back, the trigger, the round in the chamber, the magazine change) from what the gun is
//! doing, and drawing them with their attachments. Both models name their bones and
//! animations alike and their reloads have the same moments, so everything here works on
//! either (`Rig`). The first-person hand (`hand`) adds the arms' animations on top; the player
//! model (`player`) holds them in its hands.

use crate::model::gun::PARTS;
use crate::model::viewmodel::{add_anim, cube_matrix, emit_cube, find_anim, find_bone, sample, Anim, Bone, BonePose, Cube};
use crate::model::{ak_vm, pistol_vm};
use crate::item::{gun_mod, GunKind};
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

/// A magazine-fed gun's Blockbench model: its bones, cubes, animations and texture pages, the
/// points the game needs, and how it is placed.
pub struct Rig {
    pub kind: GunKind,
    pub bones: &'static [Bone],
    pub cubes: &'static [Cube],
    pub anims: &'static [Anim],
    pub pages: u32,
    /// Its first texture layer (the clean pages; dirtier sets follow).
    pub view: u32,
    muzzle: (usize, [f32; 3]),
    silenced: Option<(usize, [f32; 3])>,
    eject: (usize, [f32; 3]),
    laser: Option<(usize, [f32; 3])>,
    light: Option<(usize, [f32; 3])>,
    /// The gun's own bone (the whole gun, without the arms).
    pub gun_bone: &'static str,
    /// The groups each part is (`gun::FRAME` ..), with what is under them.
    parts: [&'static [&'static str]; PARTS],
    /// Degrees a magazine lying on its own is turned to stand up (the slant it has in the gun).
    mag_upright: f32,
    /// The rear sight's notch, and the scope's eyepiece with one (model space): aiming brings
    /// it to the middle of the view.
    sight: [f32; 3],
    scope_sight: Option<[f32; 3]>,
}

/// The pistol: a slide, the barrel with the chambered round, the recoil spring; attachments.
pub static PISTOL: Rig = Rig {
    kind: GunKind::Pistol,
    bones: pistol_vm::BONES,
    cubes: pistol_vm::CUBES,
    anims: pistol_vm::ANIMS,
    pages: pistol_vm::PAGES,
    view: tex::PISTOL_VIEW,
    muzzle: pistol_vm::MUZZLE,
    silenced: Some(pistol_vm::SILENCED),
    eject: pistol_vm::EJECT,
    laser: Some(pistol_vm::LASER),
    light: Some(pistol_vm::LIGHT),
    gun_bone: "pistol",
    parts: [&["frame", "grip"], &["barrel", "chambered_round"], &["recoil_spring"], &["slide"], &["magazine"]],
    mag_upright: 18.0,
    sight: [7.0, 14.3 - 24.0, 6.5 - 34.0],
    scope_sight: Some([7.0, 15.5 - 24.0, 5.2 - 34.0]),
};

/// The AK-47: its parts at the gun station are the receiver (with the barrel, sights,
/// handguard, grip and stock), the gas tube with the upper handguard, the bolt carrier (its
/// `slide`), the dust cover with the recoil spring, and the curved magazine. No attachments.
pub static AK: Rig = Rig {
    kind: GunKind::Ak,
    bones: ak_vm::BONES,
    cubes: ak_vm::CUBES,
    anims: ak_vm::ANIMS,
    pages: ak_vm::PAGES,
    view: tex::AK_VIEW,
    muzzle: ak_vm::MUZZLE,
    silenced: None,
    eject: ak_vm::EJECT,
    laser: None,
    light: None,
    gun_bone: "rifle",
    parts: [&["frame", "grip", "chambered_round"], &["gas_tube"], &["slide"], &["dust_cover"], &["magazine"]],
    mag_upright: 0.0,
    // The rear sight's notch, far forward on the receiver.
    sight: [7.0, 15.1 - 24.0, -18.25 - 34.0],
    scope_sight: None,
};

/// The model of a magazine-fed gun (its `item::WEAPONS` row's; the pistol's for any other).
pub fn rig(kind: GunKind) -> &'static Rig {
    kind.magazine().map_or(&PISTOL, |m| m.rig)
}

impl Rig {
    fn anim(&self, name: &str) -> Option<&'static Anim> {
        find_anim(self.anims, name)
    }

    fn bone(&self, name: &str) -> Option<usize> {
        find_bone(self.bones, name)
    }
}

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
    let length = PISTOL.anim("reload").map_or(2.0, |a| a.length);
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
    let length = PISTOL.anim("reload").map_or(2.0, |a| a.length);
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
    /// A revolver: its cylinder (the gun's `data`: what is in each chamber, the one under the
    /// hammer); seconds into loading a round; whether its reload empties the cylinder; the
    /// chambers the speedloader fills (bits).
    pub cyl: u16,
    pub load: Option<f32>,
    pub ejects: bool,
    pub loader: u8,
    /// The pistol's magazine in it (rounds, how many it holds), and the one a reload puts in:
    /// the rounds on top of it and in its witness holes show as many as there are (None: not
    /// known, all of them).
    pub mag: Option<(u8, u8)>,
    pub new_mag: Option<(u8, u8)>,
}

impl GunAnim {
    /// What `pack` leaves out, for the others (`net::Pose::gun_extra`): the magazine a reload
    /// brings (bits 0-5 its rounds + 1, 0: none; bit 6 an extended one), a revolver's round
    /// being loaded (bits 7-12: how far, + 1), its reload emptying the cylinder (bit 13) and
    /// the chambers its speedloader fills (bits 16-21).
    pub fn pack_extra(&self) -> u32 {
        let mag = self.new_mag.map_or(0, |(n, cap)| (n as u32 + 1).min(63) | ((cap > 12) as u32) << 6);
        let load = self.load.map_or(0, |t| 1 + (t / crate::model::revolver_view::LOAD_END * 62.0).round().clamp(0.0, 62.0) as u32);
        mag | load << 7 | (self.ejects as u32) << 13 | (self.loader as u32 & 0x3f) << 16
    }

    /// Takes back what `pack_extra` sent.
    pub fn unpack_extra(&mut self, e: u32) {
        let mag = e & 0x3f;
        self.new_mag = (mag > 0).then(|| ((mag - 1) as u8, if e & 1 << 6 != 0 { 20 } else { 12 }));
        let load = e >> 7 & 0x3f;
        self.load = (load > 0).then(|| (load - 1) as f32 / 62.0 * crate::model::revolver_view::LOAD_END);
        self.ejects = e & 1 << 13 != 0;
        self.loader = (e >> 16 & 0x3f) as u8;
    }
}

/// The magazine seen now (see `GunAnim::mag`): a reload's new one once the hand has it.
pub fn shown_mag(g: &GunAnim) -> Option<(u8, u8)> {
    match g.reload_time() {
        Some(t) if t >= RELOAD_FETCH && g.new_mag.is_some() => g.new_mag,
        _ => g.mag,
    }
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
            // A revolver's chambers are not sent: full while it has rounds.
            cyl: if b & 1 << 12 == 0 { crate::model::revolver_view::FULL } else { 0 },
            ejects: true,
            ..Default::default()
        }
    }

    /// Where the reload animation is now (seconds), while reloading.
    pub fn reload_time(&self) -> Option<f32> {
        self.reload.map(|p| reload_anim_time(p, self.kind, self.rack))
    }
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
pub fn rest_pose(r: &Rig) -> Vec<BonePose> {
    vec![BonePose::default(); r.bones.len()]
}

/// Adds the shot and the reload to the pose; the slide held back, the chamber empty and the
/// magazine missing as the gun is. With `parts_only` the arms and what holds the pistol are
/// left still (the player model holds it with its own arms).
pub fn add_gun_anims(r: &Rig, pose: &mut [BonePose], g: &GunAnim, parts_only: bool) {
    let holding = crate::model::gun_view::holding(r.bones, parts_only);
    let slide = r.bone("slide");
    let chamber = r.bone("chambered_round");
    let rounds = r.bone("magazine_rounds");
    let reloading = g.reload.is_some();
    if let (Some(p), Some(an)) = (g.reload, r.anim("reload")) {
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
    if let (Some(st), Some(an)) = (g.shot, r.anim("shoot")) {
        if st < an.length {
            add_anim(pose, an, st, 1.0, |b| holding.contains(&b));
        }
    }
    if let (Some(dt), Some(an), Some(trigger)) = (g.dry, r.anim("shoot"), r.bone("trigger")) {
        if dt < an.length {
            add_anim(pose, an, dt, 1.0, |b| b != trigger);
        }
    }
    if !(reloading && g.rack) {
        // Held back on an empty magazine (once the last shot has thrown it back).
        if let (Some(s), true) = (slide, g.locked) {
            let back = r.anim("shoot").map_or(0.0, |an| slide_back_time(an, s));
            if g.shot.map_or(true, |st| st >= back) {
                pose[s].pos = r.anim("reload").map_or(Vec3::ZERO, |an| channel_at(an, s, 0, 0.0));
            }
        }
        if let (Some(c), false) = (chamber, g.chambered) {
            pose[c].scale = Vec3::ZERO;
        }
    }
    if let (Some(m), true, false) = (r.bone("magazine_mesh"), g.no_mag, reloading) {
        pose[m].scale = Vec3::ZERO;
    }
    // An empty magazine shows no rounds (its witness holes: `emit_pistol`).
    if let (Some(r), Some((0, _))) = (rounds, shown_mag(g)) {
        pose[r].scale = Vec3::ZERO;
    }
}

/// The attachments are groups of the model: shown only when fitted (the extended magazine
/// in place of the standard one's base plate).
pub fn apply_mods(r: &Rig, pose: &mut [BonePose], mods: u8) {
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
        if let (Some(b), false) = (r.bone(name), shown) {
            pose[b].scale = Vec3::ZERO;
        }
    }
}

/// How dirty a gun looks (0 clean .. `tex::PISTOL_DIRT_LEVELS - 1`) with this much dirt
/// (`Stack::damage`) of `max`. A few magazines' worth does not show yet.
pub fn dirt_level(damage: u16, max: u16) -> u8 {
    if damage == 0 || max == 0 {
        return 0;
    }
    let top = tex::PISTOL_DIRT_LEVELS - 1;
    ((damage as f32 / max as f32 * top as f32).round() as u32).min(top) as u8
}

/// How dirty a gun, a part of one or a magazine looks (`dirt_level`; anything else: clean).
pub fn stack_dirt(st: &crate::item::Stack) -> u8 {
    dirt_level(st.damage, crate::item::max_damage(st.item))
}

/// The first texture layer of the gun's pages as dirty as `dirt` (`dirt_level`).
pub fn layers(r: &Rig, dirt: u8) -> u32 {
    crate::model::gun_view::dirty_layer(r.view, r.pages, dirt)
}

/// The rear sight's notch (or the scope's eyepiece, with one), model space.
pub fn sight_point(r: &Rig, mods: u8) -> Vec3 {
    match r.scope_sight {
        Some(s) if mods & gun_mod::SCOPE != 0 => Vec3::from(s),
        _ => Vec3::from(r.sight),
    }
}

/// The pistol's cubes (with the attachments `apply_mods` left shown). The see-through glass
/// (the scope's lenses) goes to `glass` to be drawn blended after the rest, when given; the
/// eyepiece's glass is left out with `eyepiece_view` (the scope's view shows there instead).
#[allow(clippy::too_many_arguments)]
pub fn emit_pistol(
    r: &Rig,
    out: &mut Vec<Vertex>,
    mut glass: Option<&mut Vec<Vertex>>,
    mats: &[Mat4],
    shown: &[bool],
    eyepiece_view: bool,
    dirt: u8,
    lamp: bool,
    mag: Option<(u8, u8)>,
    light: [u8; 4],
    fl: u8,
) {
    let first = layers(r, dirt);
    for c in r.cubes {
        // The magazine's rounds: as many as are in it.
        if let Some((n, cap)) = mag {
            if !bench::mag_cube_shown(r, c.name, n, cap) {
                continue;
            }
        }
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
pub fn eyepiece(r: &Rig, mats: &[Mat4], shown: &[bool]) -> Option<(Vec3, Vec3, Vec3, f32)> {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    let mut bone = None;
    for c in r.cubes.iter().filter(|c| c.name.starts_with("scope_lens_back")) {
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
    use crate::model::gun::{FRAME, MAGAZINE, PARTS};
    use crate::model::viewmodel::{add_anim, find_anim, BonePose};
    use super::Rig;
    use crate::item::{attachments_on, gun_mod};

    /// A set of bones (bit i: bone i).
    pub type Bones = u64;

    fn subtree(r: &Rig, name: &str) -> Bones {
        crate::model::gun_view::subtree(r.bones, name)
    }

    /// The bones an attachment is (`gun_mod` bit; several bits: all of them).
    pub fn attachment(r: &Rig, bits: u8) -> Bones {
        [
            (gun_mod::SCOPE, "scope"),
            (gun_mod::SILENCER, "silencer"),
            (gun_mod::LASER, "laser"),
            (gun_mod::LIGHT, "flashlight"),
        ]
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .fold(0, |b, (_, name)| b | subtree(r, name))
    }

    /// The bones a part is (`gun::FRAME` ..), without the attachments on it; with `mods`,
    /// those fitted on it too.
    pub fn part(r: &Rig, part: usize, mods: u8) -> Bones {
        let own = r.parts.get(part).map_or(0, |names| names.iter().fold(0, |b, n| b | subtree(r, n)));
        let on = attachments_on(part);
        let all_on = attachment(r, on);
        (own & !all_on) | if mods & on != 0 { all_on } else { 0 }
    }

    /// Whether a cube of a magazine with `rounds` of `cap` in it shows: the brass in its
    /// witness holes and the rounds on top only as far as there are rounds.
    pub fn mag_cube_shown(r: &Rig, name: &str, rounds: u8, cap: u8) -> bool {
        if let Some(k) = name.strip_prefix("witness_brass_").and_then(|r| r.get(1..)?.parse::<u32>().ok()) {
            // Hole k (from the top) shows brass once the rounds reach down to it.
            let holes = r.cubes.iter().filter(|c| c.name.starts_with("witness_brass_r")).count() as u32;
            return (rounds as u32) * holes >= (k + 1) * cap.max(1) as u32;
        }
        if name.starts_with("mag_round_top") {
            return rounds >= 1;
        }
        if name.starts_with("mag_round_2") {
            return rounds >= 2;
        }
        true
    }

    /// The gun's bones an item is when it is on its own (a part, an attachment, a magazine, a
    /// round), posed as it lies, with a magazine's rounds (in it, of how many it holds), and how
    /// to turn the model for the item: the muzzle end to +X, its right side toward +Z (a
    /// magazine straightened from its slant in the gun, stood up).
    pub fn item_rig(r: &Rig, st: &crate::item::Stack) -> Option<(Bones, Vec<BonePose>, Option<(u8, u8)>, glam::Mat4)> {
        use crate::item::*;
        use glam::Mat4;
        let side = Mat4::from_rotation_y((-90f32).to_radians());
        let none = [0.0; PARTS];
        let is_mag = magazine_gun(st.item) == Some(r.kind);
        if is_mag {
            let cap = magazine_capacity(st.item).unwrap_or(12);
            let rounds = gun_rounds(st);
            let ext = if st.item == EXTENDED_MAGAZINE { gun_mod::EXTENDED_MAGAZINE } else { 0 };
            let upright = side * Mat4::from_rotation_x(r.mag_upright.to_radians());
            return Some((part(r, MAGAZINE, 0), pose(r, none, ext, rounds > 0, false), Some((rounds, cap)), upright));
        }
        if let Some(p) = r.kind.parts().iter().position(|&i| i == st.item) {
            let mods = gun_mods(st) & attachments_on(p);
            return Some((part(r, p, mods), pose(r, none, mods, false, false), None, side));
        }
        if st.item == r.kind.ammo() {
            return Some((round(r), pose(r, none, 0, false, true), None, side));
        }
        let bit = attachment_bit(st.item)?;
        let bones = attachment(r, bit);
        (bones != 0).then(|| (bones, pose(r, none, bit, false, false), None, side))
    }

    /// The round in the chamber (a cartridge on its own).
    pub fn round(r: &Rig) -> Bones {
        subtree(r, "chambered_round")
    }

    /// The whole gun without its attachments, or with `mods` fitted.
    pub fn gun(r: &Rig, mods: u8) -> Bones {
        (0..PARTS).fold(0, |b, p| b | part(r, p, mods))
    }

    /// The strip animation's length: every part off.
    pub fn strip_length(r: &Rig) -> f32 {
        find_anim(r.anims, "strip").map_or(2.4, |a| a.length)
    }

    /// The bones a part's own animation moves.
    fn moved(r: &Rig, part: usize) -> Bones {
        match part {
            FRAME => 0,
            p => self::part(r, p, 0xff),
        }
    }

    /// The pose with each part as far into the strip animation as `at` says (seconds), the
    /// attachments in `mods` shown (and the extended magazine with its bit), the magazine's
    /// top rounds and the chambered round shown or not.
    pub fn pose(r: &Rig, at: [f32; PARTS], mods: u8, rounds: bool, chambered: bool) -> Vec<BonePose> {
        let mut pose = super::rest_pose(r);
        if let Some(an) = find_anim(r.anims, "strip") {
            for (p, &t) in at.iter().enumerate() {
                let m = moved(r, p);
                if m != 0 && t > 0.0 {
                    add_anim(&mut pose, an, t, 1.0, |b| m & (1 << b) == 0);
                }
            }
        }
        super::apply_mods(r, &mut pose, mods);
        for (name, show) in [("magazine_rounds", rounds), ("chambered_round", chambered)] {
            if let (Some(b), false) = (r.bone(name), show) {
                pose[b].scale = glam::Vec3::ZERO;
            }
        }
        pose
    }

    /// An attachment's own animation onto the gun ("fit_scope" ..) and its length.
    pub fn fit_anim(r: &Rig, bit: u8) -> Option<&'static crate::model::viewmodel::Anim> {
        let name = match bit {
            gun_mod::SCOPE => "fit_scope",
            gun_mod::SILENCER => "fit_silencer",
            gun_mod::LASER => "fit_laser",
            gun_mod::LIGHT => "fit_flashlight",
            _ => return None,
        };
        find_anim(r.anims, name)
    }

    /// Adds an attachment going on, `t` seconds into its animation.
    pub fn add_fit(r: &Rig, pose: &mut [BonePose], bit: u8, t: f32) {
        if let Some(an) = fit_anim(r, bit) {
            let m = attachment(r, bit);
            add_anim(pose, an, t, 1.0, |b| m & (1 << b) == 0);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_parts_are_the_whole_gun_once_each() {
            for r in [&super::super::PISTOL, &super::super::AK] {
                let mut seen: Bones = 0;
                for p in 0..PARTS {
                    let b = part(r, p, 0);
                    assert!(b != 0, "part {p}");
                    assert_eq!(seen & b, 0, "part {p} shares bones");
                    seen |= b;
                }
                for bit in [gun_mod::SCOPE, gun_mod::SILENCER, gun_mod::LASER, gun_mod::LIGHT] {
                    assert_eq!(seen & attachment(r, bit), 0);
                    if r.kind.fits(bit) {
                        assert!(gun(r, bit) & attachment(r, bit) != 0);
                    }
                }
                // Every drawn cube is in a part or an attachment.
                let all = gun(r, 0xff);
                for c in r.cubes {
                    assert!(all & (1 << c.bone) != 0, "{} is in no part", c.name);
                }
            }
        }
    }
}

/// Where the bullet leaves (the silencer's end when there is one): bone and model point.
pub fn muzzle(r: &Rig, mods: u8) -> (usize, Vec3) {
    let (b, p) = match r.silenced {
        Some(s) if mods & gun_mod::SILENCER != 0 => s,
        _ => r.muzzle,
    };
    (b, Vec3::from(p))
}

/// Where the spent case comes out (the gun's own bone: the chambered round's is hidden
/// while the chamber is empty).
pub fn eject(r: &Rig) -> (usize, Vec3) {
    (r.bone(r.gun_bone).unwrap_or(r.eject.0), Vec3::from(r.eject.1))
}

/// Where the weapon light's light comes from (its lens).
pub fn light(r: &Rig) -> (usize, Vec3) {
    let (b, p) = r.light.unwrap_or(r.muzzle);
    (b, Vec3::from(p))
}

/// Where the laser sight's beam starts.
pub fn laser(r: &Rig) -> (usize, Vec3) {
    let (b, p) = r.laser.unwrap_or(r.muzzle);
    (b, Vec3::from(p))
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
            ..Default::default()
        };
        let back = GunAnim::unpack(g.pack(), None);
        assert!((back.reload.unwrap() - 0.5).abs() < 0.02);
        assert_eq!(GunAnim { reload: g.reload, cyl: 0, ejects: false, ..back }, g);
        let aimed = GunAnim { aim: 1.0, ..g };
        assert_eq!(GunAnim::unpack(aimed.pack(), None).aim, 1.0);
        let idle = GunAnim { chambered: true, ..GunAnim::default() };
        assert_eq!(GunAnim { cyl: 0, ejects: false, ..GunAnim::unpack(idle.pack(), None) }, idle);
    }

    #[test]
    fn what_the_gun_is_doing_reaches_the_others() {
        let g = GunAnim { new_mag: Some((17, 20)), load: Some(0.3), ejects: true, loader: 0b101101, ..Default::default() };
        let mut back = GunAnim::default();
        back.unpack_extra(g.pack_extra());
        assert_eq!(back.new_mag, g.new_mag);
        assert!((back.load.unwrap() - 0.3).abs() < 0.01);
        assert!(back.ejects && back.loader == g.loader);
        let mut none = GunAnim::default();
        none.unpack_extra(GunAnim::default().pack_extra());
        assert_eq!(none, GunAnim::default());
    }

    #[test]
    fn the_magazine_in_the_gun_shows_its_rounds() {
        let drawn = |mag: Option<(u8, u8)>| {
            let g = GunAnim { chambered: true, mag, ..Default::default() };
            let mut pose = rest_pose(&PISTOL);
            add_gun_anims(&PISTOL, &mut pose, &g, false);
            let (mats, shown) = crate::model::viewmodel::bone_matrices(PISTOL.bones, &pose, Mat4::IDENTITY);
            let mut out = Vec::new();
            emit_pistol(&PISTOL, &mut out, None, &mats, &shown, false, 0, false, shown_mag(&g), [255; 4], 0);
            out.len()
        };
        let (full, half, empty) = (drawn(Some((12, 12))), drawn(Some((6, 12))), drawn(Some((0, 12))));
        assert!(full > half && half > empty, "{full} {half} {empty}");
        assert_eq!(drawn(None), full);
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

    fn rest_point_in_gun_space(r: &Rig, point: (usize, Vec3)) -> Vec3 {
        crate::model::gun_view::rest_point_in_gun_space(r.kind, point)
    }

    #[test]
    fn the_pistol_sits_where_the_old_one_did() {
        // The muzzle out in front (+X) of the grip, about where the old one's was.
        let m = rest_point_in_gun_space(&PISTOL, muzzle(&PISTOL, 0));
        let old = Vec3::new(8.6, 3.0, 0.0);
        assert!((m - old).length() < 4.0, "{m} vs {old}");
        let e = rest_point_in_gun_space(&PISTOL, eject(&PISTOL));
        assert!(e.x < m.x && e.y > 0.0, "{e}");
    }

    #[test]
    fn the_ak_is_held_in_the_same_fist_and_reloads_at_the_same_moments() {
        // Its reload is timed by the pistol's (`reload_span`).
        let len = |r: &Rig| r.anim("reload").map(|a| a.length);
        assert_eq!(len(&AK), len(&PISTOL));
        for name in ["shoot", "reload", "walk", "sprint", "aim", "strip"] {
            assert!(AK.anim(name).is_some(), "{name}");
        }
        for name in ["slide", "trigger", "magazine", "magazine_mesh", "magazine_rounds", "chambered_round", "left_arm", "rifle"] {
            assert!(AK.bone(name).is_some(), "{name}");
        }
        // The muzzle far out in front of the grip, the ejection port over it.
        let m = rest_point_in_gun_space(&AK, muzzle(&AK, 0));
        let e = rest_point_in_gun_space(&AK, eject(&AK));
        assert!(m.x > 40.0 && e.x < m.x && e.y > 0.0, "{m} {e}");
    }
}
