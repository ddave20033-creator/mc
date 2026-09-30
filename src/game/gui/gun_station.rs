//! The gun station: a bench two blocks wide. Opened, the camera glides over its table (as over
//! a crafting table) and sways a little left and right with the mouse; its drawer slides out,
//! the cleaning brush in it. There is nothing on the screen but the inventory along the
//! bottom: everything happens on the table, in 3D.
//!
//! - Anything from the inventory is laid on the table where the mouse points, and picked up
//!   again (or dragged somewhere else) with the mouse.
//! - A right click on a gun takes it apart there: it comes apart the way a real one does
//!   (the pistol's "strip" animation) and its parts lie beside each other.
//! - A right click on a part puts a gun together from the parts on the table (a frame, a
//!   barrel, a recoil spring and a slide; a magazine if there is one): they fly to the middle
//!   and go together.
//! - An attachment dragged onto a gun goes on it ("fit_*" animations); clicking one on a gun
//!   takes it off and lays it beside the gun.
//! - The brush from the drawer scrubs whatever it is held down on: a part clean quickly, a
//!   whole gun slowly. Dirt shows on the guns and parts themselves (`pistol_view::dirt_level`).
//!
//! What lies on the table is the station's block entity (`entity::GunBench`): saved with the
//! world and the same for every player; each change goes to the others (`Msg::Bench`), with
//! the animation everyone plays (`BenchEvent`).

use super::super::station::{hit_plane, Screen2};
use super::*;
use crate::entity::{bench_event, BenchEvent, BenchItem, GunBench};
use crate::model::gun::{BARREL, FRAME, MAGAZINE, PARTS, SLIDE, SPRING};
use crate::model::gun_view;
use crate::model::pistol_view::bench::{self as rig, Bones};
use crate::model::pistol_view::{self as pv, dirt_level};
use crate::model::revolver_view::bench as rrig;
use crate::model::viewmodel::{bone_matrices, cube_matrix, emit_cube, find_bone, BonePose, Cube};
use crate::util::{ray_box, vertex_light};
use crate::world::mesh::flags;
use glam::{Mat3, Quat};

/// Blocks per model pixel of the pistol lying on the table.
const PX: f32 = 0.026;
/// How far from the table's middle things may lie (across, and toward the front and back).
const HALF_W: f32 = 0.9;
const HALF_D: f32 = 0.4;
/// Seconds for a piece to fly across the table (to the gun it goes on, or off it), and how
/// much faster than it comes apart a gun goes together.
const FLY: f32 = 0.45;
const ASSEMBLE_SPEED: f32 = 1.6;
/// Seconds for each round to go into a magazine: brought over its lips, then pushed down in.
const ROUND_TIME: f32 = 0.34;
/// Seconds the rifle station's loader takes to push each round into the magazine on it (its
/// "feed" animation's length).
const LOADER_ROUND: f32 = 0.35;
/// Gun model units to the station model's pixels (the table's `PX`, a pixel a sixteenth).
const MODEL_TO_STATION: f32 = PX * 16.0;

/// The box the loader takes its next round from (for the magazine on it): which, if any.
fn loader_source(bench: &GunBench) -> Option<usize> {
    let mag = bench.loader_mag.filter(|_| bench.loader)?;
    let kind = magazine_gun(mag.item)?;
    if gun_rounds(&mag) >= magazine_capacity(mag.item).unwrap_or(0) {
        return None;
    }
    bench.boxes.iter().position(|b| b.and_then(box_ammo) == Some(kind.ammo()))
}

/// The magazine on the loader as it is drawn, lying in its cradle on its side, its feed lips
/// toward the feed block (`mount`: the loader's frame there, `loader_mount`).
fn loader_piece(mount: Mat4, mag: &Stack) -> Option<Piece> {
    let (kind, bones, pose) = rig_of(mag, 0.0)?;
    let (lo, hi) = bounds(kind, bones, &pose);
    // The magazine's up (its lips) along the loader, its side down.
    let lay = Mat4::from_cols(glam::Vec4::Y, glam::Vec4::X, -glam::Vec4::Z, glam::Vec4::W);
    let root = mount * lay * Mat4::from_scale(Vec3::splat(MODEL_TO_STATION)) * Mat4::from_translation(-(lo + hi) * 0.5);
    let mut pc = Piece::new(kind, bones, &pose, root, dirt_of(mag), Some(Pick::Loader));
    pc.mag = magazine_of(mag);
    Some(pc)
}
/// When the magazine is out in the pistol's strip animation (the table skips that part).
const MAG_OUT: f32 = 0.5;
/// Seconds of scrubbing that clean a completely dirty part, and a whole gun.
const SCRUB_PART: f32 = 1.6;
const SCRUB_GUN: f32 = 6.0;
/// Seconds between sending the table to the others while scrubbing.
const SCRUB_SYNC: f32 = 0.25;
/// With something in the hand: where the mouse opens the drawer (down past this fraction of
/// the screen above the inventory, looking over the table) and shuts it again (looking into
/// the drawer, up past this one: to the table's edge at the top of the view), and how long
/// the camera takes to go down to it.
const DRAWER_OPEN: f32 = 0.84;
const DRAWER_CLOSE: f32 = 0.26;
const DRAWER_GLIDE: f32 = 0.35;
/// Seconds the mouse must stay there for the drawer to open (or close).
const DRAWER_DWELL: f32 = 0.35;
/// How far under the table's top the drawer's floor is (blocks).
const DRAWER_DEPTH: f32 = (16.0 - 10.95) / 16.0;
/// Moving the mouse this far (GUI pixels) with something picked up drags it.
const DRAG: f32 = 6.0;

/// Ease in and out, 0..1.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Between two rigid (uniformly scaled) transforms, rising by `arc` halfway.
fn blend(a: Mat4, b: Mat4, k: f32, arc: f32) -> Mat4 {
    let k = k.clamp(0.0, 1.0);
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(
        sa.lerp(sb, k),
        ra.slerp(rb, k),
        ta.lerp(tb, k) + Vec3::Y * arc * (k * PI).sin(),
    )
}

/// The station's table top: its middle (between the two halves), which way is right and
/// toward its front.
#[derive(Clone, Copy)]
pub(in crate::game) struct Table {
    pub(in crate::game) center: Vec3,
    pub(in crate::game) right: Vec3,
    pub(in crate::game) toward: Vec3,
    /// How many blocks wide the station is (the rifle station three), and how far across
    /// from its middle things may lie.
    pub(in crate::game) wide: f32,
    pub(in crate::game) half_w: f32,
}

impl Table {
    /// The table of the station whose left block `p` is (block `b`).
    pub(in crate::game) fn of(p: IVec3, b: u8) -> Option<Table> {
        let f = facing(b).filter(|_| is_gun_bench(b))?;
        let toward = facing_dir(f).as_vec3();
        let right = chest_right(f).as_vec3();
        let wide = bench_width(b) as f32;
        Some(Table {
            center: p.as_vec3() + Vec3::new(0.5, 1.0, 0.5) + right * (wide - 1.0) * 0.5,
            right,
            toward,
            wide,
            half_w: HALF_W + (wide - 2.0) * 0.5,
        })
    }

    /// The rifle station's (not the small one's).
    pub(in crate::game) fn rifle(&self) -> bool {
        self.wide > 2.5
    }

    /// A point on the table: `x` to the right of its middle, `z` toward its front.
    pub(in crate::game) fn at(&self, x: f32, z: f32) -> Vec3 {
        self.center + self.right * x + self.toward * z
    }

    /// Where on the table (x, z) a point over it is.
    fn local(&self, q: Vec3) -> (f32, f32) {
        let d = q - self.center;
        (d.dot(self.right), d.dot(self.toward))
    }

    /// Whether (x, z) is on the table, where things may lie.
    fn on(&self, x: f32, z: f32) -> bool {
        x.abs() <= self.half_w && z.abs() <= HALF_D
    }

    /// The pistol lying on its left side, turned `turn` about the up axis: at 0 the muzzle
    /// (the model's -Z) to the right, its top (+Y) away from the front, its right side (+X,
    /// the ejection port) up.
    fn lying(&self, turn: f32) -> Quat {
        Quat::from_rotation_y(turn) * Quat::from_mat3(&Mat3::from_cols(Vec3::Y, -self.toward, -self.right))
    }
}

/// What the mouse can point at on the table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Pick {
    /// Something lying there (by id).
    Item(u16),
    /// An attachment on a gun lying there (the gun's id, the `gun_mod` bit).
    Mod(u16, u8),
    /// The brush in the drawer.
    Brush,
    /// A box of rounds in the drawer.
    Ammo(u8),
    /// The magazine in a gun lying there (the gun's id).
    Mag(u16),
    /// The drawer's handle (opens and shuts it).
    Handle,
    /// The rifle station's magazine loader in its drawer (or the magazine on it).
    Loader,
}

/// How a thing lying on the table is drawn: as pieces of a Blockbench gun (the pistol, the
/// revolver or the AK), or as an item lying flat.
#[derive(Clone, Copy)]
enum Look {
    Gun(GunKind),
    /// A part of a gun (`gun::FRAME` ..; a magazine is its gun's MAGAZINE part).
    Part(GunKind, usize),
    Attachment(u8),
    /// A round of a magazine-fed gun (the pistol's, the AK's).
    Round(GunKind),
    Magnum,
    Speedloader,
    Flat,
}

fn look(item: ItemId) -> Look {
    for w in &WEAPONS {
        if w.item == item {
            return Look::Gun(w.kind);
        }
        if let Some(p) = w.parts.iter().position(|&i| i == item) {
            return Look::Part(w.kind, p);
        }
        // (an extended magazine is its gun's magazine)
        if magazine_gun(item) == Some(w.kind) {
            return Look::Part(w.kind, MAGAZINE);
        }
        if w.ammo == item {
            return if w.kind.uses_magazine() { Look::Round(w.kind) } else { Look::Magnum };
        }
    }
    match item {
        SPEEDLOADER => Look::Speedloader,
        _ => attachment_bit(item).map_or(Look::Flat, Look::Attachment),
    }
}

/// The model of a magazine-fed gun (None: the revolver's, `rrig`).
fn mag_rig(kind: GunKind) -> Option<&'static pv::Rig> {
    kind.magazine().map(|m| m.rig)
}

/// The parts a gun lies in taken apart on the table (a magazine is not one of them: it is
/// laid beside them on its own), and a part's item.
fn table_parts(kind: GunKind) -> &'static [usize] {
    if kind.uses_magazine() {
        &[FRAME, BARREL, SPRING, SLIDE]
    } else {
        &[0, 1, 2, 3, 4]
    }
}

fn part_item(kind: GunKind, part: usize) -> ItemId {
    kind.parts()[part]
}

/// The bones of a gun's part (with the attachments in `mods` on it, the pistol's), and how
/// long its strip animation is.
fn part_bones(kind: GunKind, part: usize, mods: u8) -> Bones {
    match mag_rig(kind) {
        Some(r) => rig::part(r, part, mods),
        None => rrig::part(part),
    }
}

fn strip_length(kind: GunKind) -> f32 {
    mag_rig(kind).map_or_else(rrig::strip_length, rig::strip_length)
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

/// A magazine's rounds and how many it holds (a gun's: the one in it).
fn magazine_of(st: &Stack) -> Option<(u8, u8)> {
    if let Some(cap) = magazine_capacity(st.item) {
        return Some((gun_rounds(st), cap));
    }
    let kind = GunKind::of(st.item)?;
    gun_has_mag(st).then(|| (gun_rounds(st), kind.magazine_size(gun_mods(st))))
}

/// Whether something may be laid on the table: only what belongs to the guns (a gun, its
/// parts, magazines, attachments, rounds and boxes of them); a long gun's things only on the
/// rifle station's (`rifle`).
pub(in crate::game) fn belongs_on_bench(item: ItemId, rifle: bool) -> bool {
    (!matches!(look(item), Look::Flat) || item == AMMO_BOX) && (rifle || !needs_rifle_station(item))
}

/// A long gun's thing (the gun, a part, its magazine or rounds): worked on only at the rifle
/// station, the small one is for the handguns.
pub(in crate::game) fn needs_rifle_station(item: ItemId) -> bool {
    match look(item) {
        Look::Gun(k) | Look::Part(k, _) | Look::Round(k) => k.long(),
        _ => false,
    }
}

/// The attachment an item is (`gun_mod` bit), and back.
fn attachment_bit(item: ItemId) -> Option<u8> {
    ATTACHMENTS.iter().find(|a| a.1 == item).map(|a| a.0)
}

fn attachment_item(bit: u8) -> ItemId {
    ATTACHMENTS.iter().find(|a| a.0 == bit).map_or(SCOPE, |a| a.1)
}

/// The attachment that sits on a part (and is kept in the part's `data`, as on a gun).
fn attachment_of(part: usize) -> u8 {
    match part {
        FRAME => gun_mod::RAIL,
        BARREL => gun_mod::SILENCER,
        SLIDE => gun_mod::SCOPE,
        _ => 0,
    }
}

/// How dirty a gun or part looks.
fn dirt_of(st: &Stack) -> u8 {
    let max = max_damage(st.item);
    dirt_level(st.damage, if max > 0 { max } else { GunKind::Pistol.stats().dirt_max })
}

/// Which gun's model something is, its bones in it, posed as it lies (the strip animation
/// `at` for a gun coming apart).
fn rig_of(st: &Stack, at: f32) -> Option<(GunKind, Bones, Vec<BonePose>)> {
    let with = |pose: &mut Vec<BonePose>, kind: GunKind, name: &str, show: bool| {
        if let (Some(b), false) = (find_bone(gun_view::bones(kind), name), show) {
            pose[b].scale = Vec3::ZERO;
        }
    };
    match look(st.item) {
        Look::Gun(kind) if !kind.uses_magazine() => Some((kind, rrig::gun(), rrig::pose([at; PARTS], st.data))),
        Look::Gun(kind) => {
            let r = pv::rig(kind);
            let mods = gun_mods(st);
            let has_mag = gun_has_mag(st);
            let mut pose = rig::pose(r, [at; PARTS], mods, has_mag && gun_rounds(st) > 0, gun_chambered(st));
            with(&mut pose, kind, "magazine", has_mag);
            Some((kind, rig::gun(r, mods), pose))
        }
        // A cylinder on its own is empty.
        Look::Part(kind, p) if !kind.uses_magazine() => Some((kind, rrig::part(p), rrig::pose([0.0; PARTS], 0))),
        Look::Part(kind, p) => {
            let r = pv::rig(kind);
            let (ext, mods) = if p == MAGAZINE {
                ((st.item == EXTENDED_MAGAZINE) as u8 * gun_mod::EXTENDED_MAGAZINE, 0)
            } else {
                (0, gun_mods(st) & attachment_of(p))
            };
            let rounds = p == MAGAZINE && gun_rounds(st) > 0;
            Some((kind, rig::part(r, p, mods), rig::pose(r, [0.0; PARTS], mods | ext, rounds, false)))
        }
        Look::Attachment(bit) => {
            let r = &pv::PISTOL;
            Some((GunKind::Pistol, rig::attachment(r, bit), rig::pose(r, [0.0; PARTS], bit, false, false)))
        }
        Look::Round(kind) => {
            let r = pv::rig(kind);
            Some((kind, rig::round(r), rig::pose(r, [0.0; PARTS], 0, false, true)))
        }
        Look::Speedloader => Some((GunKind::Revolver, rrig::speedloader(), rrig::loader_pose(gun_rounds(st)))),
        Look::Magnum => Some((GunKind::Revolver, rrig::round(), rrig::round_pose())),
        Look::Flat => None,
    }
}

/// The cubes of these bones (of this gun's model) that are shown.
fn cubes<'a>(kind: GunKind, bones: Bones, shown: &'a [bool]) -> impl Iterator<Item = &'static Cube> + 'a {
    gun_view::cubes(kind).iter().filter(move |c| bones & (1 << c.bone) != 0 && shown[c.bone])
}

/// A cube's eight corners where its bone's matrix puts them.
fn corners(c: &Cube, bone: Mat4) -> [Vec3; 8] {
    let m = bone * cube_matrix(c);
    let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
    std::array::from_fn(|i| {
        m.transform_point3(Vec3::new(
            if i & 1 == 0 { a.x } else { b.x },
            if i & 2 == 0 { a.y } else { b.y },
            if i & 4 == 0 { a.z } else { b.z },
        ))
    })
}

/// Where the cubes of these bones reach (model space) in this pose.
fn bounds(kind: GunKind, bones: Bones, pose: &[BonePose]) -> (Vec3, Vec3) {
    let (mats, shown) = bone_matrices(gun_view::bones(kind), pose, Mat4::IDENTITY);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for c in cubes(kind, bones, &shown) {
        for p in corners(c, mats[c.bone]) {
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    (lo, hi)
}

/// Something drawn on the table: bones of a gun's model posed where, how dirty, tinted how,
/// and what it is to the mouse.
struct Piece {
    kind: GunKind,
    pick: Option<Pick>,
    bones: Bones,
    mats: Vec<Mat4>,
    shown: Vec<bool>,
    dirt: u8,
    tint: [u8; 3],
    /// A magazine's rounds (in it, of how many it holds): the brass in its witness holes and
    /// the rounds on top show as many as there are.
    mag: Option<(u8, u8)>,
    /// Only its top round (the one being pushed in).
    top_round: bool,
}

impl Piece {
    fn new(kind: GunKind, bones: Bones, pose: &[BonePose], root: Mat4, dirt: u8, pick: Option<Pick>) -> Self {
        let (mats, shown) = bone_matrices(gun_view::bones(kind), pose, root);
        Piece { kind, pick, bones, mats, shown, dirt, tint: [255; 3], mag: None, top_round: false }
    }

    /// Its cubes that are drawn.
    fn visible(&self) -> impl Iterator<Item = &'static Cube> + '_ {
        cubes(self.kind, self.bones, &self.shown).filter(move |c| {
            let name = c.name;
            if !self.kind.uses_magazine() {
                return true;
            }
            if self.top_round {
                return name.starts_with("mag_round_top");
            }
            let Some((n, cap)) = self.mag else { return true };
            rig::mag_cube_shown(pv::rig(self.kind), name, n, cap)
        })
    }

    /// Its lowest point (world).
    fn bottom(&self) -> f32 {
        self.visible()
            .flat_map(|c| corners(c, self.mats[c.bone]))
            .fold(f32::MAX, |y, p| y.min(p.y))
    }

    /// The middle of its cubes (world).
    #[cfg(test)]
    fn middle(&self) -> Vec3 {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for c in self.visible() {
            for p in corners(c, self.mats[c.bone]) {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        (lo + hi) * 0.5
    }

    /// Only some of its bones (the same pose).
    fn only(&self, bones: Bones, pick: Option<Pick>) -> Piece {
        Piece {
            kind: self.kind,
            pick,
            bones: self.bones & bones,
            mats: self.mats.clone(),
            shown: self.shown.clone(),
            dirt: self.dirt,
            tint: self.tint,
            mag: self.mag,
            top_round: self.top_round,
        }
    }

    /// Part of the way (`k`) to where `to` has the same bones, rising by `arc` halfway.
    fn toward(&self, to: &Piece, k: f32, arc: f32) -> Piece {
        let mats = self.mats.iter().zip(&to.mats).map(|(&a, &b)| blend(a, b, k, arc)).collect();
        Piece { mats, shown: to.shown.clone(), ..to.only(to.bones, to.pick) }
    }
}

/// The transform putting these bones (posed) lying on the table at (x, z), turned `turn`,
/// resting on it: their middle over the spot. A gun's (or part's) attachments do not count,
/// so it stays where it is as they go on and off.
fn lying_root(table: &Table, kind: GunKind, bones: Bones, pose: &[BonePose], x: f32, z: f32, turn: f32) -> Mat4 {
    let own = pivot_bones(kind, bones);
    let (lo, hi) = bounds(kind, own, pose);
    let root = Mat4::from_scale_rotation_translation(Vec3::splat(PX), table.lying(turn), table.at(x, z))
        * Mat4::from_translation(-(lo + hi) * 0.5);
    let bottom = Piece::new(kind, own, pose, root, 0, None).bottom();
    Mat4::from_translation(Vec3::Y * (table.center.y + 0.002 - bottom)) * root
}

/// The bones something lying on the table is placed by: without the attachments on it (they
/// do not move it as they go on and off).
fn pivot_bones(kind: GunKind, bones: Bones) -> Bones {
    if !kind.uses_magazine() {
        return bones;
    }
    let r = pv::rig(kind);
    let attachments = ATTACHMENTS.iter().fold(0, |m, a| m | rig::attachment(r, a.0));
    // Nor a gun's magazine, going in and out.
    let loose = attachments | rig::part(r, MAGAZINE, 0);
    match bones & !loose {
        0 => bones,
        b => b,
    }
}

/// Something lying on the table as it is drawn: pieces of a gun (a magazine-fed gun's
/// magazine and attachments each their own, to be taken off), or None for an item drawn flat.
fn lying_pieces(table: &Table, it: &BenchItem) -> Option<Vec<Piece>> {
    let (kind, bones, pose) = rig_of(&it.stack, 0.0)?;
    let root = lying_root(table, kind, bones, &pose, it.x, it.z, it.turn);
    let dirt = dirt_of(&it.stack);
    let mut whole = Piece::new(kind, bones, &pose, root, dirt, Some(Pick::Item(it.id)));
    whole.mag = magazine_of(&it.stack);
    if !matches!(look(it.stack.item), Look::Gun(k) if k.uses_magazine()) {
        return Some(vec![whole]);
    }
    let r = pv::rig(kind);
    let mods = gun_mods(&it.stack);
    let mag = rig::part(r, MAGAZINE, 0);
    let mut out = vec![whole.only(rig::gun(r, 0) & !mag, Some(Pick::Item(it.id)))];
    if gun_has_mag(&it.stack) {
        out.push(whole.only(mag, Some(Pick::Mag(it.id))));
    }
    for &(bit, _) in &ATTACHMENTS {
        if mods & bit != 0 {
            out.push(whole.only(rig::attachment(r, bit), Some(Pick::Mod(it.id, bit))));
        }
    }
    Some(out)
}

/// Where a piece reaches on the table (across, toward the front).
fn table_extent(table: &Table, pc: &Piece) -> (Vec2, Vec2) {
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for c in pc.visible() {
        for q in corners(c, pc.mats[c.bone]) {
            let (x, z) = table.local(q);
            lo = lo.min(Vec2::new(x, z));
            hi = hi.max(Vec2::new(x, z));
        }
    }
    (lo, hi)
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
fn fit_spot(table: &Table, stack: Stack, x: f32, z: f32, turn: f32) -> (f32, f32) {
    let it = BenchItem { id: 0, stack, x, z, turn };
    let Some(pieces) = lying_pieces(table, &it) else {
        let half = if stack.item == AMMO_BOX {
            let size = crate::model::gun_station::ammo_box_size() / 16.0;
            Vec2::new(size.x, size.z) * 0.5
        } else {
            Vec2::splat(0.1)
        };
        return ((x).clamp(-table.half_w + half.x, table.half_w - half.x), z.clamp(-HALF_D + half.y, HALF_D - half.y));
    };
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for pc in &pieces {
        let (a, b) = table_extent(table, pc);
        lo = lo.min(a);
        hi = hi.max(b);
    }
    let d = onto_table(table, lo, hi);
    (x + d.x, z + d.y)
}

/// Where a thing lying at (0, 0) reaches on the table (across, toward the front).
fn footprint(table: &Table, stack: Stack, turn: f32) -> (Vec2, Vec2) {
    let it = BenchItem { id: 0, stack, x: 0.0, z: 0.0, turn };
    match lying_pieces(table, &it) {
        Some(pieces) => pieces.iter().map(|pc| table_extent(table, pc)).fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(lo, hi), (a, b)| (lo.min(a), hi.max(b)),
        ),
        None if stack.item == AMMO_BOX => {
            let size = crate::model::gun_station::ammo_box_size() / 16.0;
            let half = Vec2::new(size.x, size.z) * 0.5;
            (-half, half)
        }
        None => (Vec2::splat(-0.1), Vec2::splat(0.1)),
    }
}

/// Across the table, how wide the strips are that a thing's shape is made of (`shape`).
const STRIP: f32 = 0.1;
/// Longer than this across, a thing lies on the table as its strips (a long gun and its long
/// parts); anything shorter as its whole outline.
const LONG: f32 = 0.8;

/// The shape of a thing lying at (0, 0) on the table: for a long one, strips across it, each as
/// deep as the thing is there (a long gun is deep only where its stock, grip or magazine is,
/// not along its barrel), so other things may lie beside its thin parts.
fn shape(table: &Table, stack: Stack, turn: f32) -> Vec<(Vec2, Vec2)> {
    let it = BenchItem { id: 0, stack, x: 0.0, z: 0.0, turn };
    let Some(pieces) = lying_pieces(table, &it) else {
        return vec![footprint(table, stack, turn)];
    };
    // Every cube's reach on the table.
    let mut cubes = Vec::new();
    for pc in &pieces {
        for c in pc.visible() {
            let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
            for q in corners(c, pc.mats[c.bone]) {
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
fn occupied(table: &Table, bench: &GunBench) -> Vec<(Vec2, Vec2)> {
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

/// Whether `stack` lying at (x, z) is on the table and in nothing already lying there.
fn is_free(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32) -> bool {
    let (lo, hi) = footprint(table, stack, turn);
    let (a, b) = (lo + Vec2::new(x, z), hi + Vec2::new(x, z));
    let gap = 0.01 - 1e-3;
    a.x >= -table.half_w - 1e-3
        && b.x <= table.half_w + 1e-3
        && a.y >= -HALF_D - 1e-3
        && b.y <= HALF_D + 1e-3
        && overlap(&shape(table, stack, turn), Vec2::new(x, z), &occupied(table, bench), gap).is_none()
}

/// `free_spot` for something held on the mouse: pushed out of what is in its way on the same
/// side as it was a moment ago (`prev`) while that side is not much further, so it slides on
/// round it with the mouse instead of flipping between two ways round.
fn free_spot_near(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32, prev: Option<(f32, f32)>) -> (f32, f32) {
    free_spot_from(table, bench, stack, x, z, turn, prev)
}

/// The nearest place to (x, z) where `stack` lies on the table without being in anything
/// already lying there (it stays where it was asked for when there is no such place).
fn free_spot(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32) -> (f32, f32) {
    free_spot_from(table, bench, stack, x, z, turn, None)
}

fn free_spot_from(table: &Table, bench: &GunBench, stack: Stack, x: f32, z: f32, turn: f32, prev: Option<(f32, f32)>) -> (f32, f32) {
    let (lo, hi) = footprint(table, stack, turn);
    let own = shape(table, stack, turn);
    let taken = occupied(table, bench);
    let gap = 0.01;
    let fits = |x: f32, z: f32| {
        let (a, b) = (lo + Vec2::new(x, z), hi + Vec2::new(x, z));
        a.x >= -table.half_w - 1e-3
            && b.x <= table.half_w + 1e-3
            && a.y >= -HALF_D - 1e-3
            && b.y <= HALF_D + 1e-3
            && overlap(&own, Vec2::new(x, z), &taken, gap).is_none()
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
        let inside = |v: &Vec2| {
            let (a, b) = (whole_a + *v, whole_b + *v);
            a.x >= -table.half_w - 1e-3 && b.x <= table.half_w + 1e-3 && a.y >= -HALF_D - 1e-3 && b.y <= HALF_D + 1e-3
        };
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
            .filter(|v| inside(v))
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
        let mut best: Option<(f32, f32)> = None;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let (cx, cz) = (x + a.cos() * r, z + a.sin() * r);
            if fits(cx, cz) {
                best = Some((cx, cz));
                break;
            }
        }
        if let Some(b) = best {
            return b;
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
fn strip_targets(table: &Table, gun: &BenchItem) -> (Vec3, Vec<(Stack, f32, f32, f32)>, u8) {
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
            let mut stack = Stack { damage: st.damage, ..Stack::one(part_item(kind, p)) };
            // The attachments stay on their parts (kept in the parts' data, like a gun's).
            set_gun_mods(&mut stack, mods & attachment_of(p));
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
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for (_, pc) in &parts {
        let (a, b) = table_extent(table, pc);
        lo = lo.min(a);
        hi = hi.max(b);
    }
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

/// The magazine in a gun, taken out: as its own item (with the gun's dirt), and whether the
/// round in the chamber went back into it.
fn magazine_out_of(gun: &Stack) -> (Stack, bool) {
    let standard = GunKind::of(gun.item).and_then(|k| k.magazine_item()).unwrap_or(PISTOL_MAGAZINE);
    let item = if gun_mods(gun) & gun_mod::EXTENDED_MAGAZINE != 0 { EXTENDED_MAGAZINE } else { standard };
    let cap = magazine_capacity(item).unwrap_or(0);
    let round = gun_chambered(gun) && gun_rounds(gun) < cap;
    let mut mag = Stack { damage: gun.damage, ..Stack::one(item) };
    set_gun_rounds(&mut mag, gun_rounds(gun) + round as u8);
    (mag, round)
}

/// A gun with its magazine out.
fn without_magazine(gun: &Stack) -> Stack {
    let mut g = *gun;
    set_gun_state(&mut g, gun_state::NO_MAG, true);
    set_gun_rounds(&mut g, 0);
    let m = gun_mods(gun) & !gun_mod::EXTENDED_MAGAZINE;
    set_gun_mods(&mut g, m);
    g
}

/// A gun with `mag` put in.
fn with_magazine(gun: &Stack, mag: &Stack) -> Stack {
    let mut g = *gun;
    set_gun_state(&mut g, gun_state::NO_MAG, false);
    set_gun_rounds(&mut g, gun_rounds(mag));
    let ext = if mag.item == EXTENDED_MAGAZINE { gun_mod::EXTENDED_MAGAZINE } else { 0 };
    let m = (gun_mods(gun) & !gun_mod::EXTENDED_MAGAZINE) | ext;
    set_gun_mods(&mut g, m);
    g
}

/// The pose of a gun with only its magazine `at` seconds into the strip animation (sliding
/// out of the grip, and laid beside it by the end of its part).
fn magazine_pose(gun: &Stack, at: f32) -> Vec<BonePose> {
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

/// Where a gun's magazine lies once it is out (beside the gun, on the table).
fn magazine_spot(table: &Table, gun: &BenchItem) -> (f32, f32) {
    let Some((kind, bones, rest)) = rig_of(&gun.stack, 0.0) else { return (gun.x, gun.z) };
    let root = lying_root(table, kind, bones, &rest, gun.x, gun.z, gun.turn);
    let pose = magazine_pose(&gun.stack, MAG_OUT);
    let (lo, hi) = bounds(kind, rig::part(pv::rig(kind), MAGAZINE, gun_mods(&gun.stack)), &pose);
    let (x, z) = table.local(root.transform_point3((lo + hi) * 0.5));
    let (mag, _) = magazine_out_of(&gun.stack);
    fit_spot(table, mag, x, z, gun.turn)
}

/// The gun a set of parts makes (in `gone`): as dirty as they are on average; a pistol with
/// the attachments that were on them, no magazine in it (that is put in the usual way) and
/// nothing in the chamber; a revolver with its cylinder empty.
fn assembled(kind: GunKind, gone: &[BenchItem]) -> Stack {
    let mut gun = Stack::one(kind.item());
    let mut mods = 0;
    for it in gone {
        if let Look::Part(_, p) = look(it.stack.item) {
            mods |= gun_mods(&it.stack) & attachment_of(p);
        }
    }
    set_gun_mods(&mut gun, mods);
    set_gun_state(&mut gun, gun_state::NO_MAG, true);
    set_gun_state(&mut gun, gun_state::CHAMBER_EMPTY, true);
    let n = gone.len().max(1) as u32;
    gun.damage = ((gone.iter().map(|i| i.stack.damage as u32).sum::<u32>() + n / 2) / n) as u16;
    gun
}

/// A gun as it comes apart on the table: the round in its chamber is already out (into its
/// magazine, or back to the player); a revolver's cylinder is emptied.
fn emptied(st: &Stack) -> Stack {
    let mut g = *st;
    if g.item == REVOLVER {
        for k in 0..6 {
            set_revolver_chamber(&mut g, k, chamber::EMPTY);
        }
        return g;
    }
    set_gun_state(&mut g, gun_state::CHAMBER_EMPTY, true);
    g
}

/// Where the strip animation starts for a gun: with its magazine coming out, or after that
/// part of it when there is none (a revolver: from the start).
fn strip_start(gun: &Stack) -> f32 {
    if gun.item == REVOLVER || gun_has_mag(gun) {
        0.0
    } else {
        MAG_OUT
    }
}

/// How long an animation on the table takes.
fn event_length(e: &BenchEvent) -> f32 {
    let fit = |bit: u8| rig::fit_anim(&pv::PISTOL, bit).map_or(0.4, |a| a.length);
    match e.kind {
        bench_event::STRIP => {
            let kind = e.gone.first().and_then(|g| GunKind::of(g.stack.item)).unwrap_or(GunKind::Pistol);
            strip_length(kind) - e.gone.first().map_or(MAG_OUT, |g| strip_start(&g.stack)) + 0.1
        }
        bench_event::ASSEMBLE => {
            let kind = e.gone.iter().find_map(|g| match look(g.stack.item) {
                Look::Part(k, _) => Some(k),
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

/// What lies on the table as it is drawn now: the pieces of the pistol, the other things (to
/// be drawn flat), with the last change `t` seconds into its animation.
fn scene(table: &Table, bench: &GunBench, t: Option<f32>) -> (Vec<Piece>, Vec<BenchItem>) {
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
        // The gun an attachment is going on (or coming off) is drawn without it meanwhile.
        if playing.is_some() && matches!(e.kind, bench_event::FIT | bench_event::UNFIT) && it.id == e.gun {
            { let m = gun_mods(&it.stack) & !e.bit; set_gun_mods(&mut it.stack, m); }
        }
        // A gun a magazine is going into is drawn without it meanwhile.
        if playing.is_some() && e.kind == bench_event::MAG_IN && it.id == e.gun {
            it.stack = without_magazine(&it.stack);
        }
        // A magazine being loaded shows the rounds in it so far.
        if playing.is_some() && e.kind == bench_event::LOAD && it.id == e.gun {
            let done = (playing.unwrap_or(0.0) / ROUND_TIME).floor() as u8;
            let r = gun_rounds(&it.stack).saturating_sub(e.bit) + done.min(e.bit);
            set_gun_rounds(&mut it.stack, r);
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
fn animation(table: &Table, bench: &GunBench, t: f32) -> Vec<Piece> {
    let e = &bench.event;
    let mut out = Vec::new();
    match e.kind {
        bench_event::STRIP => {
            let Some(gun) = e.gone.first() else { return out };
            let st = gun.stack;
            let Some(kind) = GunKind::of(st.item) else { return out };
            let (start, end) = (strip_start(&st), strip_length(kind));
            let at = (start + t).min(end);
            let (Some((_, bones, rest)), Some((_, own, pose))) = (rig_of(&st, 0.0), rig_of(&emptied(&st), at)) else {
                return out;
            };
            let (shift, _, _) = strip_targets(table, gun);
            let root = Mat4::from_translation(shift * ease(t / (end - start)))
                * lying_root(table, kind, bones, &rest, gun.x, gun.z, gun.turn);
            let mut piece = Piece::new(kind, own, &pose, root, dirt_of(&st), None);
            piece.mag = (kind.uses_magazine() && gun_has_mag(&st))
                .then(|| magazine_out_of(&st).0)
                .map(|m| (gun_rounds(&m), magazine_capacity(m.item).unwrap_or(12)));
            out.push(piece);
        }
        bench_event::ASSEMBLE => {
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
                let Look::Part(_, p) = look(part.stack.item) else { continue };
                let there = Piece::new(kind, part_bones(kind, p, 0xff), &pose, root, dirt_of(&part.stack), None);
                let piece = if t < FLY {
                    // Flying from where it lay to the gun, and turning as it goes.
                    match lying_pieces(table, part).and_then(|mut v| v.drain(..).next()) {
                        Some(from) => from.toward(&there.only(from.bones, None), ease(t / FLY), 0.15),
                        None => there,
                    }
                } else {
                    there
                };
                out.push(Piece { dirt: dirt_of(&part.stack), ..piece });
            }
        }
        bench_event::FIT | bench_event::UNFIT => {
            let Some(gun) = bench.get(e.gun) else { return out };
            let len = event_length(e) - FLY;
            let mut with = gun.stack;
            { let m = gun_mods(&with) | e.bit; set_gun_mods(&mut with, m); }
            let Some((kind, bones, mut pose)) = rig_of(&with, 0.0) else { return out };
            let root = lying_root(table, kind, bones, &pose, gun.x, gun.z, gun.turn);
            // On the gun: into place (backwards to come off), after flying there.
            let u = if e.kind == bench_event::FIT { t - FLY } else { len - t };
            rig::add_fit(pv::rig(kind), &mut pose, e.bit, u.clamp(0.0, len));
            let on = Piece::new(kind, rig::attachment(pv::rig(kind), e.bit), &pose, root, 0, None);
            let lies = if e.kind == bench_event::FIT { e.gone.first().copied() } else { e.made.first().and_then(|&id| bench.get(id)).copied() };
            let piece = match lies.and_then(|it| lying_pieces(table, &it)).and_then(|mut v| v.drain(..).next()) {
                Some(off) if e.kind == bench_event::FIT && t < FLY => off.toward(&on, ease(t / FLY), 0.12),
                Some(off) if e.kind == bench_event::UNFIT && t > len => on.toward(&off, ease((t - len) / FLY), 0.12),
                _ => on,
            };
            out.push(piece);
        }
        bench_event::MAG_OUT | bench_event::MAG_IN => {
            // The magazine sliding out of the grip and laid beside the gun, or flying to it and
            // pushed up into it.
            let Some(now) = bench.get(e.gun) else { return out };
            let (gun, mag) = if e.kind == bench_event::MAG_OUT {
                let Some(g) = e.gone.first() else { return out };
                (g.stack, magazine_out_of(&g.stack).0)
            } else {
                let Some(m) = e.gone.first() else { return out };
                (now.stack, m.stack)
            };
            let Some((kind, bones, rest)) = rig_of(&gun, 0.0) else { return out };
            let root = lying_root(table, kind, bones, &rest, now.x, now.z, now.turn);
            let at = if e.kind == bench_event::MAG_OUT { t } else { MAG_OUT - (t - FLY).max(0.0) };
            let mut there = Piece::new(kind, rig::part(pv::rig(kind), MAGAZINE, gun_mods(&gun)), &magazine_pose(&gun, at), root, dirt_of(&mag), None);
            there.mag = magazine_of(&mag);
            let piece = match (e.kind, e.gone.first().and_then(|m| lying_pieces(table, m)).and_then(|mut v| v.drain(..).next())) {
                (bench_event::MAG_IN, Some(from)) if t < FLY => from.toward(&there.only(from.bones & there.bones, None), ease(t / FLY), 0.1),
                _ => there,
            };
            out.push(piece);
        }
        bench_event::LOAD => {
            // The round going in now: from where the rounds were let go, over the magazine's
            // lips, then pushed down into it.
            let (Some(m), Some(from)) = (bench.get(e.gun), e.gone.first()) else { return out };
            let done = (t / ROUND_TIME).floor() as u8;
            if done >= e.bit {
                return out;
            }
            let k = (t - done as f32 * ROUND_TIME) / ROUND_TIME;
            let mut shown = m.stack;
            set_gun_rounds(&mut shown, gun_rounds(&m.stack).saturating_sub(e.bit) + done + 1);
            let Some(mut pcs) = lying_pieces(table, &BenchItem { stack: shown, ..*m }) else { return out };
            let mut round = pcs.remove(0);
            round.top_round = true;
            round.pick = None;
            let Some(bone) = find_bone(gun_view::bones(round.kind), "magazine") else { return out };
            let up = round.mats[bone].transform_vector3(Vec3::Y).normalize_or_zero();
            let seat = Piece { ..round.only(round.bones, None) };
            let (lo, hi) = seat.visible().flat_map(|c| corners(c, seat.mats[c.bone])).fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(lo, hi), q| (lo.min(q), hi.max(q)),
            );
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
            out.push(round);
        }
        _ => {}
    }
    out
}

/// A square on the table around (x, z).
fn square(table: &Table, x: f32, z: f32, r: f32) -> [Vec3; 4] {
    let y = Vec3::Y * 0.002;
    [table.at(x - r, z - r) + y, table.at(x + r, z - r) + y, table.at(x + r, z + r) + y, table.at(x - r, z + r) + y]
}

/// The patch of table under some pieces.
fn glow_of<'a>(table: &Table, pieces: impl Iterator<Item = &'a Piece>) -> Option<[Vec3; 4]> {
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for pc in pieces {
        let (a, b) = table_extent(table, pc);
        lo = lo.min(a);
        hi = hi.max(b);
    }
    if lo.x > hi.x {
        return None;
    }
    let (pad, y) = (0.025, Vec3::Y * 0.002);
    let (x0, x1, z0, z1) = (lo.x - pad, hi.x + pad, lo.y - pad, hi.y + pad);
    Some([table.at(x0, z0) + y, table.at(x1, z0) + y, table.at(x1, z1) + y, table.at(x0, z1) + y])
}

/// The patch of table under what the mouse is on (the whole thing lying there).
fn glow_under(table: &Table, pieces: &[Piece], flats: &[BenchItem], h: Pick) -> Option<[Vec3; 4]> {
    let id = match h {
        Pick::Item(id) | Pick::Mod(id, _) | Pick::Mag(id) => id,
        _ => return None,
    };
    let of = |p: Option<Pick>| matches!(p, Some(Pick::Item(i) | Pick::Mod(i, _) | Pick::Mag(i)) if i == id);
    glow_of(table, pieces.iter().filter(|pc| of(pc.pick))).or_else(|| {
        flats.iter().find(|f| f.id == id).map(|f| square(table, f.x, f.z, if f.stack.item == AMMO_BOX { 0.28 } else { 0.12 }))
    })
}

/// The nearest of the pieces (and the things drawn flat) the mouse ray (`o` + t `d`) goes
/// through, and how far along it.
fn pick(table: &Table, pieces: &[Piece], flats: &[BenchItem], o: Vec3, d: Vec3) -> Option<(Pick, f32)> {
    let mut best: Option<(Pick, f32)> = None;
    let mut take = |p: Pick, t: f32| {
        if best.is_none_or(|(_, bt)| t < bt) {
            best = Some((p, t));
        }
    };
    for pc in pieces {
        let Some(p) = pc.pick else { continue };
        for c in pc.visible() {
            let inv = (pc.mats[c.bone] * cube_matrix(c)).inverse();
            let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
            if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), a.min(b), a.max(b), 64.0) {
                take(p, t);
            }
        }
    }
    for it in flats {
        if it.stack.item == AMMO_BOX {
            let size = crate::model::gun_station::ammo_box_size();
            let inv = box_matrix(table, it).inverse();
            let (lo, hi) = (Vec3::new(-size.x * 0.5, 0.0, -size.z * 0.5), Vec3::new(size.x * 0.5, size.y, size.z * 0.5));
            if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), lo, hi, 64.0) {
                take(Pick::Item(it.id), t);
            }
            continue;
        }
        let c = table.at(it.x, it.z);
        let r = Vec3::new(0.1, 0.03, 0.1);
        if let Some(t) = ray_box(o, d, c - r, c + r, 64.0) {
            take(Pick::Item(it.id), t);
        }
    }
    best
}

/// How far along the ray `o`, `d` it meets the level plane at height `y` (ahead of it).
fn hit_plane_t(o: Vec3, d: Vec3, y: f32) -> Option<f32> {
    (d.y.abs() > 1e-6).then(|| (y - o.y) / d.y).filter(|&t| t > 0.0)
}

/// Whether something `t` along the ray `o`, `d` is behind the table's top (its slab, top and
/// front edge): what lies in the drawer under it. A ray that is below the slab's underside
/// inside the table's outline went through the slab.
fn hidden_by_top(table: &Table, o: Vec3, d: Vec3, t: f32) -> bool {
    /// How thick the table's top is (blocks).
    const SLAB: f32 = 0.12;
    if d.y.abs() < 1e-6 {
        return false;
    }
    let under = (table.center.y - SLAB - o.y) / d.y;
    if under <= 0.0 || t <= under + 1e-3 {
        return false;
    }
    let (x, z) = table.local(o + d * under);
    x.abs() <= table.wide * 0.5 && z.abs() <= 0.5
}

/// A box of rounds standing on the table: from the box's pixels (its bottom's middle at the
/// origin, its front toward +Z) to the world; its front toward the table's front.
fn box_matrix(table: &Table, it: &BenchItem) -> Mat4 {
    let yaw = table.toward.x.atan2(table.toward.z) + it.turn;
    Mat4::from_translation(table.at(it.x, it.z) + Vec3::Y * 0.001)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
}

/// Draws the pieces (the one under the mouse, `hover`, lit up: green where what is held on
/// the mouse goes on or into it) and the things lying flat.
fn emit(out: &mut Vec<Vertex>, table: &Table, pieces: &[Piece], flats: &[BenchItem], hover: Option<(Pick, bool)>, light: [u8; 4]) {
    let hover_ok = hover.is_some_and(|h| h.1);
    let hover = hover.map(|h| h.0);
    for pc in pieces {
        let lit = pc.pick.is_some() && pc.pick == hover;
        let tint = if lit && hover_ok { [150, 255, 150] } else { pc.tint };
        let from = out.len();
        let first = gun_view::layers(pc.kind, pc.dirt);
        for c in pc.visible() {
            emit_cube(out, c, pc.mats[c.bone] * cube_matrix(c), first, light, flags::ENTITY);
        }
        if lit {
            // Brighter than white can make it: the light's own channel.
            for v in &mut out[from..] {
                v.tint[..3].copy_from_slice(&tint);
                v.light[0] = v.light[0].saturating_add(90);
                v.light[1] = v.light[1].saturating_add(90);
            }
        }
    }
    for it in flats {
        let lit = hover == Some(Pick::Item(it.id));
        if it.stack.item == AMMO_BOX {
            let from = out.len();
            crate::model::gun_station::emit_ammo_box(out, box_matrix(table, it), it.stack.data, light, flags::ENTITY);
            if lit {
                for v in &mut out[from..] {
                    if hover_ok {
                        v.tint[..3].copy_from_slice(&[150, 255, 150]);
                    }
                    v.light[0] = v.light[0].saturating_add(90);
                    v.light[1] = v.light[1].saturating_add(90);
                }
            }
            continue;
        }
        let size = 0.28;
        let m = Mat4::from_translation(table.at(it.x, it.z) + Vec3::Y * (size / 32.0 + 0.004))
            * Mat4::from_quat(Quat::from_rotation_y(it.turn) * Quat::from_mat3(&Mat3::from_cols(table.right, Vec3::Y, table.toward)))
            * Mat4::from_rotation_x(-FRAC_PI_2)
            * Mat4::from_scale(Vec3::splat(size));
        let from = out.len();
        crate::model::emit_lying(out, m, &it.stack, light, flags::ENTITY);
        if lit {
            for v in &mut out[from..] {
                v.light[0] = v.light[0].saturating_add(90);
                v.light[1] = v.light[1].saturating_add(90);
            }
        }
    }
}

/// Something held on the mouse over a gun station, drawn in 3D where it shows (`at`: over
/// the spot it would be laid on, lifted).
fn emit_hold(out: &mut Vec<Vertex>, table: &Table, st: Stack, at: Vec3, light: [u8; 4]) {
    let st = if rig_of(&st, 0.0).is_some() { Stack { count: 1, ..st } } else { st };
    let (x, z) = table.local(at);
    let it = BenchItem { id: 0, stack: st, x, z, turn: 0.0 };
    let lift = Vec3::Y * (at.y - table.center.y);
    match lying_pieces(table, &it) {
        Some(pieces) => {
            let pieces: Vec<Piece> = pieces
                .into_iter()
                .map(|mut pc| {
                    pc.pick = None;
                    for m in &mut pc.mats {
                        *m = Mat4::from_translation(lift) * *m;
                    }
                    pc
                })
                .collect();
            emit(out, table, &pieces, &[], None, light);
        }
        None => {
            let from = out.len();
            emit(out, table, &[], &[it], None, light);
            for v in &mut out[from..] {
                v.pos[1] += lift.y;
            }
        }
    }
}

impl Game {
    /// The table of the gun station whose left half is `p`.
    pub(in crate::game) fn bench_table(&self, p: IVec3) -> Option<Table> {
        Table::of(p, self.terrain.world.geti(p))
    }

    /// Seconds into the animation of the last change on the table at `p` (when one was seen).
    fn bench_time(&self, p: IVec3) -> Option<f32> {
        let serial = self.level.block_entities.benches.get(&p)?.event.serial;
        self.level.bench_anims.get(&p).filter(|a| a.0 == serial).map(|a| self.time - a.1)
    }

    /// Whether something on the table at `p` is still moving (its things wait until then).
    fn bench_busy(&self, p: IVec3) -> bool {
        let Some(b) = self.level.block_entities.benches.get(&p) else { return false };
        self.bench_time(p).is_some_and(|t| t < event_length(&b.event))
    }

    /// A table's contents came from the others (or were loaded): a new change there plays
    /// from now.
    pub(in crate::game) fn set_bench(&mut self, p: IVec3, bench: GunBench) {
        let serial = bench.event.serial;
        if serial != 0 && self.level.bench_anims.get(&p).is_none_or(|a| a.0 != serial) {
            self.level.bench_anims.insert(p, (serial, self.time));
        }
        self.level.block_entities.benches.insert(p, bench);
    }

    /// The half of a rifle station's grenade crate the crosshair is on (the station's left
    /// block, 0 frag grenades / 1 smoke grenades), within reach.
    pub(in crate::game) fn crate_under_crosshair(&self) -> Option<(IVec3, usize)> {
        let (hit, _) = self.target?;
        let w = &self.terrain.world;
        let main = bench_main(hit, w.geti(hit), |q| w.geti(q))?;
        let table = Table::of(main, w.geti(main)).filter(|t| t.rifle())?;
        let (eye, dir) = (self.player.eye(), look_dir(self.yaw, self.pitch));
        let mut found: Option<(usize, f32)> = None;
        for (half, lo, hi, m) in crate::model::gun_station::crate_halves(main, table.toward) {
            let inv = m.inverse();
            let (o, d) = (inv.transform_point3(eye), inv.transform_vector3(dir));
            // (the ray in model pixels: its length scales with them)
            let reach = 5.0 * d.length();
            if let Some(t) = ray_box(o, d.normalize(), lo, hi, reach) {
                if found.is_none_or(|(_, bt)| t < bt) {
                    found = Some((half, t));
                }
            }
        }
        found.map(|(half, _)| (main, half))
    }

    /// Right click on a rifle station's grenade crate: holding grenades, one goes in (into the
    /// half for its kind; sneaking, as many as there is room for); otherwise one is taken out
    /// of the half clicked (sneaking, all of it). False if the crosshair is not on the crate.
    pub(in crate::game) fn crate_click(&mut self) -> bool {
        use crate::item::inventory::take;
        use crate::item::{FRAG_GRENADE, SMOKE_GRENADE};
        use crate::model::gun_station::CRATE_MAX;
        let Some((main, half)) = self.crate_under_crosshair() else { return false };
        let kinds = [FRAG_GRENADE, SMOKE_GRENADE];
        let held = self.held();
        let slot = self.hotbar_slot;
        let creative = self.creative();
        let sneaking = self.sneaking();
        let bench = self.level.block_entities.benches.entry(main).or_default();
        if let Some(i) = kinds.iter().position(|&k| k == held) {
            let count = self.inventory.slots[slot].map_or(0, |s| s.count);
            let k = (if sneaking { count } else { 1 }).min(CRATE_MAX - bench.grenades[i].min(CRATE_MAX));
            if k == 0 {
                return true;
            }
            bench.grenades[i] += k;
            if !creative {
                take(&mut self.inventory.slots[slot], k);
            }
        } else if bench.grenades[half] > 0 {
            let n = if sneaking { bench.grenades[half] } else { 1 };
            bench.grenades[half] -= n;
            self.give(Stack::new(kinds[half], n));
        } else {
            return true;
        }
        self.audio.play(crate::audio::Sound::GrenadeBounce, Some(self.player.eye()), 0.35);
        self.hand.swing();
        self.bench_changed(main, None);
        true
    }

    /// Something on the table at `p` changed here: with `event`, what happened (everyone
    /// plays it); it goes to the others.
    fn bench_changed(&mut self, p: IVec3, event: Option<BenchEvent>) {
        let now = self.time;
        let b = self.level.block_entities.benches.entry(p).or_default();
        if let Some(mut e) = event {
            e.serial = b.event.serial.wrapping_add(1).max(1);
            b.event = e;
            self.level.bench_anims.insert(p, (b.event.serial, now));
        }
        let msg = crate::net::Msg::Bench { p, bench: b.clone() };
        self.bench_ui.sent = now;
        if self.is_client() {
            self.send(msg);
        } else {
            self.broadcast(&msg, None);
        }
    }

    /// Everything lying on every gun station near the camera, its drawer, the brush in it or
    /// in someone's hand.
    pub(in crate::game) fn build_benches(&mut self, out: &mut Vec<Vertex>, dt: f32) {
        let world = &self.terrain.world;
        let near = |p: &IVec3| (p.as_vec3() - self.player.pos).length_squared() < 48.0 * 48.0;
        let open_here = match self.screen {
            Screen::Container(Container::GunStation(q)) => Some(q),
            _ => None,
        };
        let remote = self.remote_drawers();
        let remote_brushes = self.remote_brushes();
        let remote_holds = self.remote_bench_holds();
        let step = dt / crate::model::gun_station::open_seconds();
        let stations: Vec<IVec3> = self.terrain.gun_stations.values().flatten().copied().filter(|p| near(p)).collect();
        for p in stations {
            let b = world.geti(p);
            let Some(table) = Table::of(p, b) else { continue };
            // The drawer is out while someone looks into it.
            let used = (open_here == Some(p) && self.bench_ui.in_drawer) || remote.contains(&p);
            let s = self.level.bench_drawer.entry(p).or_insert(0.0);
            *s = if used { (*s + step).min(1.0) } else { (*s - step).max(0.0) };
            let drawer = *s;
            let (sky, blk) = world.light_estimate(table.center + Vec3::Y * 0.2);
            let light = vertex_light(sky, blk);
            let brush_out = (open_here == Some(p) && self.bench_ui.brush) || remote_brushes.iter().any(|(q, _)| *q == p);
            let ammo = self.level.block_entities.benches.get(&p).map_or([Some(0); 3], |b| b.boxes);
            let handle_lit = open_here == Some(p) && self.bench_ui.hover == Some(Pick::Handle);
            let loader = self.level.block_entities.benches.get(&p).map_or(Default::default(), |b| crate::model::gun_station::Loader {
                there: b.loader && table.rifle(),
                feed: loader_source(b).map(|_| self.time),
            });
            crate::model::gun_station::emit_block(out, table.rifle(), p, table.toward, drawer, !brush_out, ammo, loader, handle_lit, light, flags::ENTITY);
            if table.rifle() {
                // The grenades in the crate on the shelf, and how many.
                let n = self.level.block_entities.benches.get(&p).map_or([0; 2], |b| b.grenades);
                let (sky, blk) = world.light_estimate(table.center - Vec3::Y * 0.7);
                crate::model::gun_station::emit_crate(out, p, table.toward, n, vertex_light(sky, blk), flags::ENTITY);
            }
            if let Some(bench) = self.level.block_entities.benches.get(&p) {
                let t = self.bench_time(p);
                let (mut pieces, flats) = scene(&table, bench, t);
                // The magazine on the loader (with the drawer, wherever it is).
                if let (true, Some(mag)) = (loader.there, bench.loader_mag) {
                    if let Some(pc) = crate::model::gun_station::loader_mount(p, table.toward, drawer).and_then(|m| loader_piece(m, &mag)) {
                        pieces.push(pc);
                    }
                }
                let hover = if open_here == Some(p) { self.bench_ui.hover.map(|h| (h, self.bench_ui.hover_ok)) } else { None };
                emit(out, &table, &pieces, &flats, hover, light);
            }
            for (_, at) in remote_brushes.iter().filter(|(q, _)| *q == p) {
                crate::model::gun_station::emit_brush(out, *at, table.toward.x.atan2(table.toward.z), 0.0, light, flags::ENTITY);
            }
            // What is held on the mouse, over the table or in the drawer: in 3D, lifted over
            // where it would go.
            let held_at = match (self.bench_ui.spot, self.bench_ui.drawer_spot) {
                (Some(_), _) => self.bench_ui.held_spot.map(|(x, z)| (x, z, 0.0)),
                (None, Some(q)) => {
                    let (x, z) = table.local(q);
                    Some((x, z, q.y - table.center.y))
                }
                _ => None,
            };
            let held = self.cursor.filter(|st| belongs_on_bench(st.item, table.rifle()));
            if open_here == Some(p) {
                self.bench_ui.hold_at = None;
            }
            if let (true, Some(st), Some((x, z, below))) = (open_here == Some(p), held, held_at) {
                let at = table.at(x, z) + Vec3::Y * (0.06 + below);
                self.bench_ui.hold_at = Some(at);
                emit_hold(out, &table, st, at, light);
            }
            // And what the others hold there.
            for (_, st, at) in remote_holds.iter().filter(|(q, _, _)| *q == p) {
                emit_hold(out, &table, *st, *at, light);
            }
            if open_here == Some(p) && self.bench_ui.brush {
                if let Some(at) = self.bench_ui.brush_at {
                    let tilt = if self.bench_ui.scrubbing { (self.time * 26.0).sin() * 0.12 } else { 0.0 };
                    let wiggle = if self.bench_ui.scrubbing { table.right * (self.time * 26.0).sin() * 0.015 } else { Vec3::ZERO };
                    crate::model::gun_station::emit_brush(out, at + wiggle, table.toward.x.atan2(table.toward.z), tilt, light, flags::ENTITY);
                }
            }
        }
        self.level.bench_drawer.retain(|p, s| *s > 0.0 && is_gun_bench(world.geti(*p)));
        self.level.bench_anims.retain(|p, _| is_gun_bench(world.geti(*p)));
    }

    /// Where the brush is in this player's hand, for the others to see.
    pub(in crate::game) fn bench_brush_pose(&self) -> Option<Vec3> {
        match self.screen {
            Screen::Container(Container::GunStation(_)) if self.bench_ui.brush => self.bench_ui.brush_at,
            _ => None,
        }
    }

    /// Opens the gun station whose left half is `p`: the camera glides over its table.
    pub(in crate::game) fn open_gun_station(&mut self, p: IVec3) {
        self.bench_ui.brush = false;
        self.bench_ui.brush_at = None;
        self.bench_ui.drag = None;
        self.bench_ui.pan = 0.0;
        self.bench_ui.in_drawer = false;
        self.bench_ui.focus = 0.0;
        self.open_container(Container::GunStation(p));
    }

    /// Closing the gun station: the brush goes back into the drawer.
    pub(in crate::game) fn close_gun_station(&mut self) {
        self.bench_ui.hold_at = None;
        // A box of rounds still on the mouse goes back into the drawer (or onto the table).
        if let (Screen::Container(Container::GunStation(p)), Some(st)) = (self.screen, self.cursor) {
            if st.item == AMMO_BOX {
                self.cursor = None;
                let table = self.bench_table(p);
                let bench = self.level.block_entities.benches.entry(p).or_default();
                match bench.boxes.iter().position(|b| b.is_none()) {
                    Some(i) => bench.boxes[i] = Some(st.data),
                    None => {
                        if let Some(t) = table {
                            let (x, z) = free_spot(&t, bench, st, 0.0, 0.0, 0.0);
                            bench.add(st, x, z, 0.0);
                        }
                    }
                }
                self.bench_changed(p, None);
            }
        }
        self.bench_ui.spot = None;
        self.bench_ui.drawer_spot = None;
        self.bench_ui.in_drawer = false;
        self.bench_ui.brush = false;
        self.bench_ui.brush_at = None;
        self.bench_ui.drag = None;
        self.bench_ui.hover = None;
    }

    /// The open gun station: the inventory along the bottom, and whatever the mouse does on
    /// the table. Returns the inventory slot under the mouse.
    pub(super) fn gun_station_screen(&mut self, p: IVec3) -> Option<SlotRef> {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let mut hovered = None;
        // The inventory, on a dark strip along the bottom.
        let (pw, ph) = (176.0 * s, 86.0 * s);
        let (px, py) = (((w - pw) * 0.5).round(), (h - ph - 4.0 * s).round());
        self.ui.rect_full(px, py, pw, ph, rgba(10, 11, 16, 150), rgba(10, 11, 16, 190), 5.0 * s, 3.0 * s);
        self.inventory_slots(px, py, 5.0, &mut hovered);
        let over_inventory = self.ui.hit(px, py, pw, ph);
        // A box of rounds belongs to the station: it does not go into the inventory.
        let holding_box = self.cursor.is_some_and(|st| st.item == AMMO_BOX);
        if holding_box {
            hovered = None;
        }
        // The camera sways a little with the mouse, to see along the table; the mouse going
        // down toward the inventory looks into the drawer (it slides out, the camera goes down
        // to it), and back up over the table (it closes).
        let want = ((self.ui.mouse.x / w.max(1.0)) * 2.0 - 1.0).clamp(-1.0, 1.0);
        self.bench_ui.pan += (want - self.bench_ui.pan) * (1.0 - (-4.0 * self.ui.dt).exp());
        let low = self.ui.mouse.y / py.max(1.0);

        let Some(table) = self.bench_table(p) else { return hovered };
        let ready = self.station.as_ref().is_some_and(|st| st.blend > 0.9 && !st.closing);
        let view = Screen2 { view_proj: self.view_proj, w, h };
        let (o, d) = view.ray(self.ui.mouse);
        let bench = self.level.block_entities.benches.get(&p).cloned().unwrap_or_default();
        let (pieces, flats) = scene(&table, &bench, self.bench_time(p));
        let busy = self.bench_busy(p);
        let mut found = None;
        let mut spot = None;
        let mut drawer_spot = None;
        let drawer = self.level.bench_drawer.get(&p).copied().unwrap_or(0.0);
        if ready && !over_inventory {
            found = self.bench_pick(p, &table, &bench, &pieces, &flats, drawer, o, d);
            spot = hit_plane(o, d, table.center.y)
                .map(|q| table.local(q))
                .filter(|&(x, z)| table.on(x, z));
            // Or in the drawer, out in front of the table: on its floor.
            drawer_spot = hit_plane(o, d, table.center.y - DRAWER_DEPTH).filter(|&q| {
                let (x, z) = table.local(q);
                drawer > 0.8 && x.abs() < table.half_w && (0.46..0.95).contains(&z)
            });
            // The table's top hides what is under it: over it, the mouse is on the table; the
            // drawer only where it is out in front of the table.
            if spot.is_some() {
                drawer_spot = None;
            }
        }
        self.bench_ui.spot = spot;
        self.bench_ui.drawer_spot = drawer_spot;

        // The drawer opens and shuts with its handle (a click, below). Only with something in
        // the hand does it follow the mouse (once it has stayed there a moment): taken out of
        // the drawer and brought up over the table (the mouse on its top, or up at the top of
        // the view), it shuts and the camera goes back up; brought down toward it from the
        // table with what goes into it (the brush, rounds, a box of them), it opens again.
        let settled = self.bench_ui.focus > 0.99 || self.bench_ui.focus < 0.01;
        let holding = self.cursor.is_some() || self.bench_ui.brush;
        let for_drawer = self.bench_ui.brush || self.cursor.is_some_and(|st| BOX_AMMO.contains(&st.item) || st.item == AMMO_BOX);
        let over_table = spot.is_some() || low < DRAWER_CLOSE;
        let wants = if self.bench_ui.in_drawer {
            holding && over_table && drawer_spot.is_none()
        } else {
            for_drawer && low > DRAWER_OPEN && !over_inventory
        };
        self.bench_ui.dwell = if settled && wants && !over_inventory { self.bench_ui.dwell + self.ui.dt } else { 0.0 };
        if self.bench_ui.dwell > DRAWER_DWELL {
            self.bench_ui.in_drawer = !self.bench_ui.in_drawer;
            self.bench_ui.dwell = 0.0;
        }
        let target = if self.bench_ui.in_drawer { 1.0 } else { 0.0 };
        let step = self.ui.dt / DRAWER_GLIDE;
        // (toward it; staying put once there)
        self.bench_ui.focus = if self.bench_ui.focus < target {
            (self.bench_ui.focus + step).min(target)
        } else {
            (self.bench_ui.focus - step).max(target)
        };
        let pick = found.map(|(k, _)| k);
        let point = found.map(|(_, t)| o + d * t).or(spot.map(|(x, z)| table.at(x, z))).or(drawer_spot);
        // Where what is held on the mouse would lie (kept from the last frame while it can be).
        self.bench_ui.held_spot = match (self.cursor, spot) {
            (Some(st), Some((x, z))) if belongs_on_bench(st.item, table.rifle()) => {
                let st = if rig_of(&st, 0.0).is_some() { Stack { count: 1, ..st } } else { st };
                Some(free_spot_near(&table, &bench, st, x, z, 0.0, self.bench_ui.held_spot))
            }
            _ => None,
        };
        // What the mouse is on lights up: what a click takes; with something held, only where
        // it goes on or into (green).
        let ok = self.bench_target_ok(&bench, pick);
        self.bench_ui.hover = pick.filter(|_| self.cursor.is_none() || ok);
        self.bench_ui.hover_ok = self.cursor.is_some() && ok;

        let (left, right) = (self.ui.pressed, self.ui.right_pressed);
        self.bench_ui.scrubbing = false;
        if self.bench_ui.brush {
            // The brush: where the mouse points, scrubbing what it is held down on.
            self.bench_ui.brush_at = point.map(|q| q + Vec3::Y * 0.005);
            self.bench_ui.hover = None;
            if right || (left && drawer_spot.is_some() && pick.is_none()) {
                // Put back (a right click anywhere, or a click in the drawer).
                self.bench_ui.brush = false;
                self.bench_ui.brush_at = None;
            } else if self.input.left_down {
                if let (Some(Pick::Item(id) | Pick::Mod(id, _)), false) = (pick, busy) {
                    self.scrub(p, id, point);
                }
            }
        } else if let Some(from) = self.bench_ui.drag.filter(|_| !self.input.left_down) {
            // Let go of something picked up: dropped where the mouse is (on the table, or into
            // an inventory slot), once it was dragged.
            self.bench_ui.drag = None;
            if (self.ui.mouse - from).length() > DRAG * s && !busy {
                match (hovered, pick) {
                    (Some(r), _) if over_inventory => self.click_slot(Container::GunStation(p), r, false, false),
                    (_, Some(Pick::Ammo(i))) => self.bench_box_slot(p, i as usize, false),
                    (_, Some(Pick::Loader)) => self.bench_loader_click(p, &table),
                    _ => self.bench_put(p, &table, pick, spot, false),
                }
            }
        } else if (left || right) && ready && !busy && !over_inventory {
            match (self.cursor, pick) {
                (_, Some(Pick::Ammo(i))) => self.bench_box_slot(p, i as usize, right),
                (_, Some(Pick::Loader)) => self.bench_loader_click(p, &table),
                (_, Some(Pick::Handle)) => {
                    self.bench_ui.in_drawer = !self.bench_ui.in_drawer;
                    self.bench_ui.dwell = 0.0;
                }
                // Looking into the drawer, a click beside it (on nothing) shuts it.
                (None, None) if self.bench_ui.in_drawer && drawer_spot.is_none() && spot.is_none() => {
                    self.bench_ui.in_drawer = false;
                    self.bench_ui.dwell = 0.0;
                }
                (Some(_), _) => self.bench_put(p, &table, pick, spot, right),
                (None, Some(Pick::Brush)) if left => self.bench_ui.brush = true,
                (None, Some(Pick::Item(id))) if left => {
                    if let Some(it) = self.level.block_entities.benches.get_mut(&p).and_then(|b| b.take(id)) {
                        self.cursor = Some(it.stack);
                        self.bench_ui.drag = Some(self.ui.mouse);
                        self.bench_changed(p, None);
                    }
                }
                (None, Some(Pick::Mod(id, bit))) if left => self.bench_unfit(p, &table, id, bit),
                (None, Some(Pick::Mag(id))) if left => self.bench_mag_out(p, &table, id),
                (None, Some(Pick::Mag(id))) => self.bench_right_click(p, &table, id),
                (None, Some(Pick::Item(id))) => self.bench_right_click(p, &table, id),
                _ => {}
            }
        }
        // Not scrubbing any more: what is left goes to the others.
        if !self.bench_ui.scrubbing && self.bench_ui.scrub_dirty {
            self.bench_ui.scrub_dirty = false;
            self.bench_changed(p, None);
        }

        let far = table.right * (table.wide - 1.0);
        let min = p.as_vec3().min(p.as_vec3() + far);
        let over_block = ray_box(o, d, min, min + Vec3::new(1.0, 1.0, 1.0) + far.abs(), 64.0).is_some();
        self.inv_ui.station_inside = over_inventory || over_block || pick.is_some() || spot.is_some() || holding_box;
        self.inv_ui.station_hover = None;
        // A glow on the table under what the mouse is on, or where what is held would go.
        self.inv_ui.station_frame = match (self.bench_ui.hover, self.cursor, spot) {
            (Some(h), _, _) => glow_under(&table, &pieces, &flats, h),
            (None, Some(st), Some(_)) if !self.bench_ui.brush && belongs_on_bench(st.item, table.rifle()) => {
                let st = if rig_of(&st, 0.0).is_some() { Stack { count: 1, ..st } } else { st };
                let (x, z) = self.bench_ui.held_spot.unwrap_or((0.0, 0.0));
                let it = BenchItem { id: 0, stack: st, x, z, turn: 0.0 };
                match lying_pieces(&table, &it) {
                    Some(pcs) => glow_of(&table, pcs.iter()),
                    None => Some(square(&table, x, z, 0.12)),
                }
            }
            _ => None,
        };
        hovered
    }

    /// Whether what is held on the mouse goes on or into what it is on: an attachment onto a
    /// gun without one, a magazine into a gun without one, rounds into a magazine or a box
    /// with room, a box into an empty place in the drawer.
    fn bench_target_ok(&self, bench: &GunBench, pick: Option<Pick>) -> bool {
        let Some(st) = self.cursor else { return pick.is_some() };
        let item = |id: u16| bench.get(id).map(|i| i.stack);
        match pick {
            // A magazine onto the loader when there is none on it; the loader into its bay.
            Some(Pick::Loader) => self.loader_can(bench),
            Some(Pick::Item(id) | Pick::Mod(id, _) | Pick::Mag(id)) => {
                let Some(t) = item(id) else { return false };
                if let Some(bit) = attachment_bit(st.item) {
                    return GunKind::of(t.item).is_some_and(|k| k.fits(bit)) && attachment_fits(gun_mods(&t), bit);
                }
                if let Some(g) = magazine_gun(st.item) {
                    return GunKind::of(t.item) == Some(g) && !gun_has_mag(&t);
                }
                if let Some(g) = GUN_KINDS.into_iter().find(|k| k.uses_magazine() && k.ammo() == st.item) {
                    return match (magazine_capacity(t.item), t.item) {
                        (Some(cap), m) if magazine_gun(m) == Some(g) => gun_rounds(&t) < cap,
                        (None, AMMO_BOX) => box_room(t.data, st.item) > 0,
                        _ => false,
                    };
                }
                if st.item == MAGNUM_ROUND {
                    return match t.item {
                        SPEEDLOADER => gun_rounds(&t) < magazine_capacity(SPEEDLOADER).unwrap_or(6),
                        AMMO_BOX => box_room(t.data, MAGNUM_ROUND) > 0,
                        _ => false,
                    };
                }
                false
            }
            Some(Pick::Ammo(i)) => match bench.boxes[i as usize] {
                Some(v) => box_room(v, st.item) > 0,
                None => st.item == AMMO_BOX,
            },
            _ => false,
        }
    }

    /// Something held on the mouse cursor put on the table: an attachment onto the gun under
    /// the mouse (if it fits and has none such), otherwise laid where the mouse points (all
    /// of it, or one with the right button; the pistol's parts one at a time).
    fn bench_put(&mut self, p: IVec3, table: &Table, pick: Option<Pick>, spot: Option<(f32, f32)>, one: bool) {
        let Some(st) = self.cursor else { return };
        if let Some(Pick::Item(id) | Pick::Mod(id, _) | Pick::Mag(id)) = pick {
            if self.bench_mag_in(p, id, spot) {
                return;
            }
        }
        let bench = self.level.block_entities.benches.entry(p).or_default();
        if let (Some(bit), Some(Pick::Item(id) | Pick::Mod(id, _) | Pick::Mag(id))) = (attachment_bit(st.item), pick) {
            let fits = bench.get(id).is_some_and(|g| {
                GunKind::of(g.stack.item).is_some_and(|k| k.fits(bit)) && attachment_fits(gun_mods(&g.stack), bit)
            });
            if fits {
                // It goes on from where the mouse let go of it.
                let (x, z) = spot.unwrap_or((0.0, 0.0));
                let gone = BenchItem { id: 0, stack: Stack::one(st.item), x, z, turn: 0.0 };
                if let Some(g) = bench.items.iter_mut().find(|g| g.id == id) {
                    { let m = gun_mods(&g.stack) | bit; set_gun_mods(&mut g.stack, m); }
                }
                take(&mut self.cursor, 1);
                let e = BenchEvent { kind: bench_event::FIT, gun: id, bit, gone: vec![gone], ..Default::default() };
                self.bench_changed(p, Some(e));
                return;
            }
        }
        // Rounds onto a box of them lying on the table: into it (only the kind it holds).
        if let (BULLET | MAGNUM_ROUND | RIFLE_ROUND, Some(Pick::Item(id))) = (st.item, pick) {
            if let Some(b) = bench.items.iter_mut().find(|b| b.id == id && b.stack.item == AMMO_BOX) {
                let room = box_room(b.stack.data, st.item);
                let n = (if one { 1 } else { st.count as u16 }).min(room);
                if n > 0 {
                    b.stack.data = box_with(b.stack.data, st.item, n);
                    take(&mut self.cursor, n as u8);
                    self.bench_changed(p, None);
                }
                return;
            }
        }
        // Magnum rounds onto a speedloader lying on the table: into it.
        if let (MAGNUM_ROUND, Some(Pick::Item(id))) = (st.item, pick) {
            if let Some(l) = bench.items.iter_mut().find(|l| l.id == id && l.stack.item == SPEEDLOADER) {
                let cap = magazine_capacity(SPEEDLOADER).unwrap_or(6);
                let n = (if one { 1 } else { st.count }).min(cap.saturating_sub(gun_rounds(&l.stack)));
                if n > 0 {
                    let r = gun_rounds(&l.stack) + n;
                    set_gun_rounds(&mut l.stack, r);
                    take(&mut self.cursor, n);
                    self.bench_changed(p, None);
                }
                return;
            }
        }
        // Rounds onto a magazine lying on the table: pushed into it, one after another.
        if let (BULLET | RIFLE_ROUND, Some(Pick::Item(id))) = (st.item, pick) {
            let mag = bench.get(id).copied().filter(|m| magazine_gun(m.stack.item).is_some_and(|k| k.ammo() == st.item));
            if let Some(m) = mag {
                let cap = magazine_capacity(m.stack.item).unwrap_or(0);
                let n = (if one { 1 } else { st.count }).min(cap.saturating_sub(gun_rounds(&m.stack)));
                if n > 0 {
                    if let Some(g) = bench.items.iter_mut().find(|g| g.id == id) {
                        let r = gun_rounds(&g.stack) + n;
                        set_gun_rounds(&mut g.stack, r);
                    }
                    let (x, z) = spot.unwrap_or((m.x, m.z + 0.15));
                    let gone = BenchItem { id: 0, stack: Stack::new(st.item, n), x, z, turn: 0.0 };
                    take(&mut self.cursor, n);
                    let e = BenchEvent { kind: bench_event::LOAD, gun: id, bit: n, gone: vec![gone], ..Default::default() };
                    self.bench_changed(p, Some(e));
                }
                return;
            }
        }
        let Some((x, z)) = spot else { return };
        if !belongs_on_bench(st.item, table.rifle()) {
            if needs_rifle_station(st.item) {
                self.gun_message(t("gun.rifle_station_only"));
            }
            return;
        }
        let single = one || rig_of(&st, 0.0).is_some();
        let lay = if single { Stack { count: 1, ..st } } else { st };
        let turn = if rig_of(&st, 0.0).is_some() { 0.0 } else { (self.random() - 0.5) * 0.6 };
        let bench = self.level.block_entities.benches.entry(p).or_default();
        let (x, z) = match self.bench_ui.held_spot {
            Some(q) if single && is_free(table, bench, lay, q.0, q.1, turn) => q,
            _ => free_spot(table, bench, lay, x, z, turn),
        };
        bench.add(lay, x, z, turn);
        take(&mut self.cursor, lay.count);
        self.bench_changed(p, None);
    }

    /// A right click on something on the table: a gun comes apart there, a part puts a gun
    /// together from the parts on the table (when they are all there).
    pub(in crate::game) fn bench_right_click(&mut self, p: IVec3, table: &Table, id: u16) {
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return };
        let Some(it) = bench.get(id).copied() else { return };
        match look(it.stack.item) {
            Look::Gun(_) => {
                bench.take(id);
                let (_, targets, loose) = strip_targets(table, &it);
                // Where something else already lies, a part goes beside it.
                let made = targets
                    .into_iter()
                    .map(|(st, x, z, turn)| {
                        let (x, z) = free_spot(table, bench, st, x, z, turn);
                        bench.add(st, x, z, turn)
                    })
                    .collect();
                let e = BenchEvent { kind: bench_event::STRIP, gun: id, gone: vec![it], made, ..Default::default() };
                self.bench_changed(p, Some(e));
                // The live rounds that were in it (the pistol's chamber, when it did not fit back
                // into the magazine; the revolver's cylinder).
                if let (true, Some(k)) = (loose > 0, GunKind::of(it.stack.item)) {
                    self.give(Stack::new(k.ammo(), loose));
                }
            }
            Look::Flat if it.stack.item == AMMO_BOX => {
                // A round out of it, onto the mouse.
                if let (true, Some(kind)) = (self.cursor.is_none(), box_ammo(it.stack.data)) {
                    if let Some(b) = bench.items.iter_mut().find(|b| b.id == id) {
                        b.stack.data = box_without(b.stack.data, 1);
                    }
                    self.cursor = Some(Stack::one(kind));
                    self.bench_ui.drag = Some(self.ui.mouse);
                    self.bench_changed(p, None);
                }
            }
            Look::Part(kind, q) if !(kind.uses_magazine() && q == MAGAZINE) => {
                // One of each part (the clicked one first).
                let mut chosen: Vec<u16> = Vec::new();
                for &want in table_parts(kind) {
                    let is = |i: &&BenchItem| matches!(look(i.stack.item), Look::Part(k, q) if k == kind && q == want);
                    match bench.items.iter().filter(is).min_by_key(|i| (i.id != id) as u8) {
                        Some(c) => chosen.push(c.id),
                        None => return,
                    }
                }
                let mut gone = Vec::new();
                for c in chosen {
                    let Some(i) = bench.items.iter().position(|i| i.id == c) else { continue };
                    let it = &mut bench.items[i];
                    gone.push(BenchItem { stack: Stack { count: 1, ..it.stack }, ..*it });
                    if it.stack.count > 1 {
                        it.stack.count -= 1;
                    } else {
                        bench.items.remove(i);
                    }
                }
                let gun = assembled(kind, &gone);
                let (x, z) = free_spot(table, bench, gun, 0.0, 0.0, 0.0);
                let gid = bench.add(gun, x, z, 0.0);
                let e = BenchEvent { kind: bench_event::ASSEMBLE, gun: gid, gone, ..Default::default() };
                self.bench_changed(p, Some(e));
            }
            _ => {}
        }
    }

    /// A click (or something let go) on a place for a box of rounds in the drawer: the box
    /// taken out (a round out of it with the right button), rounds dropped into it (all, or
    /// one with the right button), a box put back where there is none.
    fn bench_box_slot(&mut self, p: IVec3, i: usize, right: bool) {
        let bench = self.level.block_entities.benches.entry(p).or_default();
        match (self.cursor, bench.boxes[i]) {
            (None, Some(v)) if right => match box_ammo(v) {
                Some(kind) => {
                    bench.boxes[i] = Some(box_without(v, 1));
                    self.cursor = Some(Stack::one(kind));
                }
                None => return,
            },
            (None, Some(v)) => {
                bench.boxes[i] = None;
                self.cursor = Some(Stack { data: v, ..Stack::one(AMMO_BOX) });
            }
            (Some(st), Some(v)) if BOX_AMMO.contains(&st.item) => {
                // Only the kind it holds (either, when it is empty).
                let k = (if right { 1 } else { st.count as u16 }).min(box_room(v, st.item));
                if k == 0 {
                    return;
                }
                bench.boxes[i] = Some(box_with(v, st.item, k));
                take(&mut self.cursor, k as u8);
            }
            (Some(st), None) if st.item == AMMO_BOX => {
                bench.boxes[i] = Some(st.data);
                take(&mut self.cursor, 1);
            }
            _ => return,
        }
        if self.cursor.is_some() {
            self.bench_ui.drag = Some(self.ui.mouse);
        }
        self.bench_changed(p, None);
    }

    /// What the mouse is on at the station whose left block is `p` (the ray `o`, `d`): the
    /// nearest thing there is to click, and how far along the ray. What lies in the drawer is
    /// only found with the drawer out; what the table's top hides is not found.
    #[allow(clippy::too_many_arguments)]
    fn bench_pick(&self, p: IVec3, table: &Table, bench: &GunBench, pieces: &[Piece], flats: &[BenchItem], drawer: f32, o: Vec3, d: Vec3) -> Option<(Pick, f32)> {
    let mut found = pick(table, pieces, flats, o, d);
    if !self.bench_ui.brush && drawer > 0.8 {
        for (i, c, m) in crate::model::gun_station::ammo_boxes(table.rifle(), p, table.toward, drawer) {
            let inv = m.inverse();
            let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
            if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), a.min(b), a.max(b), 64.0) {
                if found.is_none_or(|(_, bt)| t < bt) {
                    found = Some((Pick::Ammo(i as u8), t));
                }
            }
        }
    }
    // The loader's bay, the middle of the rifle station's drawer: the mouse on anything
    // in it (the loader, the magazine on it, the bay's floor) is on the loader, when
    // there is something to do with it; nowhere else is.
    if !self.bench_ui.brush && drawer > 0.8 && table.rifle() && self.loader_can(bench) {
        let mut near = hit_plane_t(o, d, table.center.y - DRAWER_DEPTH);
        if bench.loader {
            for (c, m) in crate::model::gun_station::loader_cubes(p, table.toward, drawer) {
                let inv = m.inverse();
                let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
                if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), a.min(b), a.max(b), 64.0) {
                    near = Some(near.map_or(t, |n: f32| n.min(t)));
                }
            }
        }
        if let Some(t) = near.filter(|&t| crate::model::gun_station::in_loader_bay(p, table.toward, drawer, o + d * t)) {
            if found.is_none_or(|(k, bt)| k == Pick::Loader || t < bt) {
                found = Some((Pick::Loader, t));
            }
        }
    }
    if !self.bench_ui.brush && self.cursor.is_none() && drawer > 0.8 {
        for (c, m) in crate::model::gun_station::brush_in_drawer(table.rifle(), p, table.toward, drawer) {
            let inv = m.inverse();
            let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
            if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), a.min(b), a.max(b), 64.0) {
                if found.is_none_or(|(_, bt)| t < bt) {
                    found = Some((Pick::Brush, t));
                }
            }
        }
    }
    // The handle shuts the drawer (with something held too): the mouse on it is always
    // on it, whatever lies in the drawer behind it.
    if !self.bench_ui.brush {
        for (c, m) in crate::model::gun_station::drawer_handle(table.rifle(), p, table.toward, drawer) {
            let inv = m.inverse();
            let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
            if let Some(t) = ray_box(inv.transform_point3(o), inv.transform_vector3(d), a.min(b), a.max(b), 64.0) {
                if !matches!(found, Some((Pick::Handle, bt)) if bt <= t) {
                    found = Some((Pick::Handle, t));
                }
            }
        }
    }
    // The table's top hides what is behind it: what is in the drawer (under the top) is
    // not there to click where the mouse is on the top.
    if found.is_some_and(|(_, t)| hidden_by_top(table, o, d, t)) {
        found = None;
    }
    found
    }

    /// The test's `pickmap`: a map of what a click would do at every point of the view (the
    /// station at `p`, with what is on the mouse now), written as a picture beside the view's
    /// own: the loader's bay green, the boxes of rounds orange, the brush blue, the handle
    /// white, what lies on the table red, nothing black.
    pub(in crate::game) fn bench_pick_map(&self, p: IVec3, path: &std::path::Path) {
        let Some(table) = self.bench_table(p) else { return };
        let (w, h) = (self.ui.w, self.ui.h);
        let view = Screen2 { view_proj: self.view_proj, w, h };
        let bench = self.level.block_entities.benches.get(&p).cloned().unwrap_or_default();
        let (pieces, flats) = scene(&table, &bench, self.bench_time(p));
        let drawer = self.level.bench_drawer.get(&p).copied().unwrap_or(0.0);
        let (mw, mh) = ((w / 4.0) as u32, (h / 4.0) as u32);
        let mut px = Vec::with_capacity((mw * mh * 3) as usize);
        for y in 0..mh {
            for x in 0..mw {
                let (o, d) = view.ray(Vec2::new(x as f32 * 4.0 + 2.0, y as f32 * 4.0 + 2.0));
                let c = match self.bench_pick(p, &table, &bench, &pieces, &flats, drawer, o, d).map(|(k, _)| k) {
                    Some(Pick::Loader) => [40, 220, 60],
                    Some(Pick::Ammo(_)) => [240, 150, 30],
                    Some(Pick::Brush) => [60, 120, 250],
                    Some(Pick::Handle) => [240, 240, 240],
                    Some(_) => [220, 40, 40],
                    None => [0, 0, 0],
                };
                px.extend_from_slice(&c);
            }
        }
        if let Ok(f) = std::fs::File::create(path) {
            let mut e = png::Encoder::new(std::io::BufWriter::new(f), mw, mh);
            e.set_color(png::ColorType::Rgb);
            if let Ok(mut wr) = e.write_header() {
                let _ = wr.write_image_data(&px);
            }
        }
    }

    /// Whether a click in the loader's bay does something, with what is held: the loader put
    /// in (held, none there), a magazine laid on it (held, the loader there and bare), or taken
    /// out (empty-handed: the magazine on it, or the loader).
    fn loader_can(&self, bench: &GunBench) -> bool {
        match self.cursor {
            None => bench.loader,
            Some(st) if st.item == MAG_LOADER => !bench.loader,
            Some(st) => is_gun_magazine(st.item) && bench.loader && bench.loader_mag.is_none(),
        }
    }

    /// A click on the rifle station's magazine loader (or with one held, in its drawer): the
    /// loader put in the middle of the drawer, a magazine laid on it (it fills it from the
    /// boxes beside it), the magazine taken off it, or the loader itself taken out when it is
    /// bare.
    fn bench_loader_click(&mut self, p: IVec3, table: &Table) {
        let drawer = self.level.bench_drawer.get(&p).copied().unwrap_or(0.0);
        let bench = self.level.block_entities.benches.entry(p).or_default();
        match self.cursor {
            Some(st) if st.item == MAG_LOADER => {
                if !table.rifle() {
                    self.gun_message(t("gun.loader_rifle_station"));
                    return;
                }
                if bench.loader || drawer < 0.8 {
                    return;
                }
                bench.loader = true;
                take(&mut self.cursor, 1);
            }
            Some(st) if is_gun_magazine(st.item) && bench.loader && bench.loader_mag.is_none() => {
                bench.loader_mag = Some(Stack { count: 1, ..st });
                take(&mut self.cursor, 1);
            }
            None if bench.loader_mag.is_some() => {
                self.cursor = bench.loader_mag.take();
                self.bench_ui.drag = Some(self.ui.mouse);
            }
            None if bench.loader => {
                bench.loader = false;
                self.cursor = Some(Stack::one(MAG_LOADER));
                self.bench_ui.drag = Some(self.ui.mouse);
            }
            _ => return,
        }
        self.bench_changed(p, None);
    }

    /// Host, every frame: each loader with a magazine on it that is not full pushes a round
    /// into it from a box of the rounds it takes, one after another.
    pub(in crate::game) fn update_loaders(&mut self, dt: f32) {
        let busy: Vec<(IVec3, usize)> = self
            .level.block_entities
            .benches
            .iter()
            .filter_map(|(p, b)| loader_source(b).map(|i| (*p, i)))
            .collect();
        self.level.loader_feed.retain(|p, _| busy.iter().any(|(q, _)| q == p));
        for (p, i) in busy {
            let t = self.level.loader_feed.entry(p).or_insert(0.0);
            *t += dt;
            if *t < LOADER_ROUND {
                continue;
            }
            *t -= LOADER_ROUND;
            let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { continue };
            let (Some(v), Some(mag)) = (bench.boxes[i], bench.loader_mag.as_mut()) else { continue };
            bench.boxes[i] = Some(box_without(v, 1));
            let r = gun_rounds(mag) + 1;
            set_gun_rounds(mag, r);
            self.bench_changed(p, None);
        }
    }

    /// The magazine of a gun on the table clicked: it slides out and is laid beside the gun.
    fn bench_mag_out(&mut self, p: IVec3, table: &Table, id: u16) {
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return };
        let Some(before) = bench.get(id).copied().filter(|g| gun_has_mag(&g.stack)) else { return };
        // Its rounds come with it (the round in the chamber stays there).
        let (mut mag, _) = magazine_out_of(&before.stack);
        set_gun_rounds(&mut mag, gun_rounds(&before.stack));
        let (x, z) = magazine_spot(table, &before);
        if let Some(g) = bench.items.iter_mut().find(|g| g.id == id) {
            g.stack = without_magazine(&g.stack);
        }
        let (x, z) = free_spot(table, bench, mag, x, z, before.turn);
        let mid = bench.add(mag, x, z, before.turn);
        let e = BenchEvent { kind: bench_event::MAG_OUT, gun: id, gone: vec![before], made: vec![mid], ..Default::default() };
        self.bench_changed(p, Some(e));
    }

    /// A magazine held on the mouse let go on a gun without one: it goes in.
    fn bench_mag_in(&mut self, p: IVec3, id: u16, spot: Option<(f32, f32)>) -> bool {
        let Some(st) = self.cursor.filter(|s| is_gun_magazine(s.item)) else { return false };
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return false };
        let fits = magazine_gun(st.item);
        let Some(g) = bench.items.iter_mut().find(|g| g.id == id && GunKind::of(g.stack.item) == fits && !gun_has_mag(&g.stack)) else {
            return false;
        };
        g.stack = with_magazine(&g.stack, &st);
        let (x, z) = spot.unwrap_or((g.x, g.z + 0.2));
        let gone = BenchItem { id: 0, stack: Stack { count: 1, ..st }, x, z, turn: g.turn };
        take(&mut self.cursor, 1);
        let e = BenchEvent { kind: bench_event::MAG_IN, gun: id, gone: vec![gone], ..Default::default() };
        self.bench_changed(p, Some(e));
        true
    }

    /// An attachment on a gun clicked: it comes off and is laid beside the gun.
    fn bench_unfit(&mut self, p: IVec3, table: &Table, id: u16, bit: u8) {
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return };
        let Some(g) = bench.items.iter_mut().find(|g| g.id == id) else { return };
        { let m = gun_mods(&g.stack) & !bit; set_gun_mods(&mut g.stack, m); }
        let (x, z) = (g.x, g.z + 0.22);
        let att = Stack::one(attachment_item(bit));
        let (x, z) = free_spot(table, bench, att, x, z, 0.0);
        let aid = bench.add(att, x, z, 0.0);
        let e = BenchEvent { kind: bench_event::UNFIT, gun: id, bit, made: vec![aid], ..Default::default() };
        self.bench_changed(p, Some(e));
    }

    /// The brush held down on something on the table: its dirt comes off (a part quickly, a
    /// whole gun slowly), with foam where it works.
    fn scrub(&mut self, p: IVec3, id: u16, at: Option<Vec3>) {
        let dt = self.ui.dt;
        let Some(it) = self.level.block_entities.benches.get_mut(&p).and_then(|b| b.items.iter_mut().find(|i| i.id == id)) else {
            return;
        };
        let (item, damage) = (it.stack.item, it.stack.damage);
        let max = max_damage(item);
        if max == 0 || damage == 0 {
            return;
        }
        self.bench_ui.scrubbing = true;
        let time = if GunKind::of(item).is_some() { SCRUB_GUN } else { SCRUB_PART };
        self.bench_ui.scrub += dt * max as f32 / time;
        let off = self.bench_ui.scrub.floor();
        if off >= 1.0 {
            self.bench_ui.scrub -= off;
            it.stack.damage = it.stack.damage.saturating_sub(off as u16);
            self.bench_ui.scrub_dirty = true;
        }
        if self.bench_ui.scrub_dirty && self.time - self.bench_ui.sent > SCRUB_SYNC {
            self.bench_ui.scrub_dirty = false;
            self.bench_changed(p, None);
        }
        if let Some(at) = at {
            if self.rng.next() < dt * 25.0 {
                let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y * 0.2);
                let jitter = Vec3::new(self.rng.next() - 0.5, 0.0, self.rng.next() - 0.5) * 0.06;
                self.particles.smoke_shaded(at + jitter + Vec3::Y * 0.02, 245, sky, blk);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table { center: Vec3::new(1.0, 65.0, 0.5), right: Vec3::X, toward: Vec3::Z, wide: 2.0, half_w: HALF_W }
    }

    fn gun(mods: u8) -> Stack {
        let mut g = Stack::one(PISTOL);
        set_gun_mods(&mut g, mods);
        set_gun_rounds(&mut g, 7);
        g.damage = 20;
        g
    }

    /// Where the pieces reach, on the table: across, up, toward the front.
    fn extent(t: &Table, pieces: &[Piece]) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for pc in pieces {
            for c in pc.visible() {
                for q in corners(c, pc.mats[c.bone]) {
                    let w = q - t.center;
                    let q = Vec3::new(w.dot(t.right), w.y, w.dot(t.toward));
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
            }
        }
        (lo, hi)
    }

    #[test]
    fn the_top_hides_the_drawer_under_it() {
        let t = table();
        // Looking down onto the top's middle: something under the top's slab there is hidden,
        // what lies on the top is not.
        let o = t.center + Vec3::new(0.0, 1.0, 0.8);
        let d = (t.center - o).normalize();
        let top = (o - t.center).length();
        assert!(hidden_by_top(&t, o, d, top + 0.3));
        assert!(!hidden_by_top(&t, o, d, top - 0.02));
        // Out in front of the table, looking into the open drawer below the top's level: seen.
        let drawer = t.center + t.toward * 0.7 - Vec3::Y * 0.2;
        let o = drawer + Vec3::new(0.0, 0.6, 0.6);
        let d = (drawer - o).normalize();
        assert!(!hidden_by_top(&t, o, d, (drawer - o).length()));
    }

    #[test]
    fn a_gun_lies_on_the_table_muzzle_to_the_right() {
        let t = table();
        let it = BenchItem { id: 1, stack: gun(gun_mod::SCOPE | gun_mod::SILENCER | gun_mod::LASER), x: 0.0, z: 0.0, turn: 0.0 };
        let pieces = lying_pieces(&t, &it).unwrap();
        let (lo, hi) = extent(&t, &pieces);
        assert!(lo.y.abs() < 0.01 && hi.y > 0.02, "{lo} {hi}");
        assert!(lo.x > -HALF_W && hi.x < HALF_W && lo.z > -HALF_D && hi.z < HALF_D, "{lo} {hi}");
        let (b, m) = crate::model::pistol_view::muzzle(&pv::PISTOL, gun_mod::SILENCER);
        let muzzle = pieces[0].mats[b].transform_point3(m) - t.center;
        assert!(muzzle.dot(t.right) > 0.1, "{muzzle}");
    }

    #[test]
    fn a_gun_comes_apart_on_the_table_and_goes_back_together() {
        let t = table();
        let st = gun(gun_mod::SCOPE | gun_mod::LASER);
        let it = BenchItem { id: 1, stack: st, x: 0.5, z: 0.2, turn: 0.0 };
        // Near the table's corner: it all still ends up on it.
        let it = BenchItem { x: 0.8, z: 0.35, ..it };
        let (_, parts, _) = strip_targets(&t, &it);
        // Frame, barrel, spring, slide, and the magazine that was in it.
        assert_eq!(parts.len(), 5);
        let mut bench = GunBench::default();
        for (s, x, z, turn) in &parts {
            bench.add(*s, *x, *z, *turn);
        }
        // Lying there, every part rests on the table, all of it on it, none on another.
        let (pieces, _) = scene(&t, &bench, None);
        let boxes: Vec<(Vec3, Vec3)> = pieces.iter().map(|pc| extent(&t, std::slice::from_ref(pc))).collect();

        for (i, (lo, hi)) in boxes.iter().enumerate() {
            assert!(lo.y.abs() < 0.01, "{lo}");
            assert!(lo.x >= -HALF_W - 1e-3 && hi.x <= HALF_W + 1e-3 && lo.z >= -HALF_D - 1e-3 && hi.z <= HALF_D + 1e-3, "{i}: {lo} {hi}");
        }
        // Put together again: the same gun (its attachments back from its parts), as dirty,
        // with no magazine in it and nothing in the chamber.
        let back = assembled(GunKind::Pistol, &bench.items);
        assert_eq!(gun_mods(&back), gun_mods(&st));
        assert_eq!(back.damage, st.damage);
        assert!(!gun_has_mag(&back) && !gun_chambered(&back));
    }

    #[test]
    fn a_revolver_comes_apart_into_its_five_parts_and_goes_back_together() {
        let t = table();
        let mut st = Stack::one(REVOLVER);
        set_gun_rounds(&mut st, 4);
        st.damage = 30;
        let it = BenchItem { id: 1, stack: st, x: 0.0, z: 0.0, turn: 0.0 };
        let pieces = lying_pieces(&t, &it).unwrap();
        let (lo, hi) = extent(&t, &pieces);
        assert!(lo.y.abs() < 0.01 && hi.y > 0.02, "{lo} {hi}");
        let (parts, loose) = {
            let (_, parts, loose) = strip_targets(&t, &it);
            (parts, loose)
        };
        // Frame, barrel, mainspring, cylinder, hammer; its four live rounds back to the player.
        assert_eq!(parts.len(), 5);
        assert_eq!(loose, 4);
        let mut bench = GunBench::default();
        for (s, x, z, turn) in &parts {
            assert_eq!(s.damage, 30);
            bench.add(*s, *x, *z, *turn);
        }
        let (pieces, _) = scene(&t, &bench, None);
        for pc in &pieces {
            let (lo, hi) = extent(&t, std::slice::from_ref(pc));
            assert!(lo.y.abs() < 0.01, "{lo}");
            assert!(lo.x >= -HALF_W - 1e-3 && hi.x <= HALF_W + 1e-3 && lo.z >= -HALF_D - 1e-3 && hi.z <= HALF_D + 1e-3, "{lo} {hi}");
        }
        let back = assembled(GunKind::Revolver, &bench.items);
        assert_eq!(back.item, REVOLVER);
        assert_eq!(back.damage, 30);
        assert_eq!(gun_rounds(&back), 0);
    }

    #[test]
    fn nothing_is_laid_into_something_else() {
        let t = table();
        let mut bench = GunBench::default();
        let g = gun(0);
        bench.add(g, 0.0, 0.0, 0.0);
        // Another gun laid right on it goes beside it instead, on the table.
        let (x, z) = free_spot(&t, &bench, g, 0.05, 0.02, 0.0);
        bench.add(g, x, z, 0.0);
        let boxes = occupied(&t, &bench);
        let (a, b) = (boxes[0], boxes[1]);
        let apart = a.1.x <= b.0.x || b.1.x <= a.0.x || a.1.y <= b.0.y || b.1.y <= a.0.y;
        assert!(apart, "{a:?} {b:?}");
        assert!(t.on(x, z), "{x} {z}");
        // And a round laid where both are finds room too.
        let (rx, rz) = free_spot(&t, &bench, Stack::one(BULLET), 0.0, 0.0, 0.0);
        let r = footprint(&t, Stack::one(BULLET), 0.0);
        let r = (r.0 + Vec2::new(rx, rz), r.1 + Vec2::new(rx, rz));
        for (c, d) in occupied(&t, &bench) {
            assert!(r.1.x <= c.x || d.x <= r.0.x || r.1.y <= c.y || d.y <= r.0.y, "{r:?} in {c:?} {d:?}");
        }
    }

    #[test]
    fn something_held_slides_along_what_is_in_its_way() {
        let t = table();
        let mut bench = GunBench::default();
        bench.add(gun(0), 0.0, 0.0, 0.0);
        let round = Stack::one(BULLET);
        for z in [-0.1, 0.0, 0.08] {
            // Moved across the gun in small steps: it goes round it (a jump or two, from one
            // side to the other), never back and forth.
            let mut last = free_spot(&t, &bench, round, -0.6, z, 0.0);
            let mut x = -0.6;
            let mut jumps = 0;
            while x < 0.6 {
                x += 0.004;
                let now = free_spot_near(&t, &bench, round, x, z, 0.0, Some(last));
                if Vec2::new(now.0 - last.0, now.1 - last.1).length() > 0.06 {
                    jumps += 1;
                }
                last = now;
            }
            assert!(jumps <= 2, "{z}: {jumps} jumps");
        }
    }

    #[test]
    fn a_magazine_shows_its_rounds_as_they_go_in() {
        let t = table();
        let mut bench = GunBench::default();
        let mut mag = Stack::one(PISTOL_MAGAZINE);
        set_gun_rounds(&mut mag, 5);
        let id = bench.add(mag, 0.0, 0.0, 0.0);
        let brass = |bench: &GunBench, time: Option<f32>| {
            let (pieces, _) = scene(&t, bench, time);
            pieces
                .iter()
                .filter(|pc| pc.pick == Some(Pick::Item(id)))
                .flat_map(|pc| pc.visible())
                .filter(|c| c.name.starts_with("witness_brass_"))
                .count()
        };
        let five = brass(&bench, None);
        set_gun_rounds(&mut bench.items[0].stack, 12);
        let full = brass(&bench, None);
        assert!(five > 0 && full > five, "{five} {full}");
        // Seven more going in: at first it still shows five, at the end twelve.
        bench.event = BenchEvent {
            serial: 1,
            kind: bench_event::LOAD,
            gun: id,
            bit: 7,
            gone: vec![BenchItem { id: 0, stack: Stack::new(BULLET, 7), x: 0.5, z: 0.2, turn: 0.0 }],
            made: Vec::new(),
        };
        assert_eq!(brass(&bench, Some(0.01)), five);
        assert_eq!(brass(&bench, Some(event_length(&bench.event) + 0.1)), full);
        // A round is on its way in the meantime.
        assert_eq!(animation(&t, &bench, 0.5).len(), 1);
    }

    #[test]
    fn every_animation_ends_where_things_lie() {
        let t = table();
        let mut bench = GunBench::default();
        let st = gun(0);
        let id = bench.add(st, -0.3, 0.0, 0.0);
        // An attachment going on: at the end it is where it sits on the gun.
        let mut with = st;
        set_gun_mods(&mut with, gun_mod::SCOPE);
        bench.items[0].stack = with;
        bench.event = BenchEvent {
            serial: 1,
            kind: bench_event::FIT,
            gun: id,
            bit: gun_mod::SCOPE,
            gone: vec![BenchItem { id: 0, stack: Stack::one(SCOPE), x: 0.5, z: 0.3, turn: 0.0 }],
            made: Vec::new(),
        };
        let end = event_length(&bench.event) - 1e-3;
        let moving = animation(&t, &bench, end);
        let lying = lying_pieces(&t, &bench.items[0]).unwrap();
        let on = lying.iter().find(|pc| pc.pick == Some(Pick::Mod(id, gun_mod::SCOPE))).unwrap();
        assert!(moving[0].middle().distance(on.middle()) < 0.01);
    }
}
