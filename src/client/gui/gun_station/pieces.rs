//! Things lying on the table as pieces of the guns' Blockbench models (the pistol, the
//! revolver or the AK): which model and bones each is, posed as it lies, and where its cubes
//! reach. Anything else is an item lying flat.

use super::pick::Pick;
use super::table::{PX, Table};
use crate::entity::BenchItem;
use crate::item::*;
use crate::model::gun::{MAGAZINE, PARTS};
use crate::model::gun_view;
use crate::model::pistol_view::bench::Bones;
use crate::model::pistol_view::bench::self as rig;
use crate::model::pistol_view::stack_dirt;
use crate::model::pistol_view::self as pv;
use crate::model::revolver_view::bench as rrig;
use crate::model::viewmodel::{BonePose, Cube, bone_matrices, cube_matrix, find_bone};
use glam::{Mat4, Vec2, Vec3};

/// Gun model units to the station model's pixels (the table's `PX`, a pixel a sixteenth).
const MODEL_TO_STATION: f32 = PX * 16.0;

/// The model of a magazine-fed gun (None: the revolver's, `rrig`).
pub(super) fn mag_rig(kind: GunKind) -> Option<&'static pv::Rig> {
    kind.magazine().map(|m| m.rig)
}

/// The bones of a gun's part (with the attachments in `mods` on it, the pistol's), and how
/// long its strip animation is.
pub(super) fn part_bones(kind: GunKind, part: usize, mods: u8) -> Bones {
    match mag_rig(kind) {
        Some(r) => rig::part(r, part, mods),
        None => rrig::part(part),
    }
}

pub(super) fn strip_length(kind: GunKind) -> f32 {
    mag_rig(kind).map_or_else(rrig::strip_length, rig::strip_length)
}

/// Whether something is drawn as a gun's model (as `rig_of` poses it), not lying flat.
pub(super) fn modelled(item: ItemId) -> bool {
    bench_role(item) != BenchRole::Other
}

/// What of a stack is laid on the table at once: a thing drawn as a gun's model one at a
/// time, anything else all of it.
pub(super) fn as_laid(st: Stack) -> Stack {
    if modelled(st.item) { Stack { count: 1, ..st } } else { st }
}

/// Which gun's model something is, its bones in it, posed as it lies (the strip animation
/// `at` for a gun coming apart).
pub(super) fn rig_of(st: &Stack, at: f32) -> Option<(GunKind, Bones, Vec<BonePose>)> {
    let with = |pose: &mut Vec<BonePose>, kind: GunKind, name: &str, show: bool| {
        if let (Some(b), false) = (find_bone(gun_view::bones(kind), name), show) {
            pose[b].scale = Vec3::ZERO;
        }
    };
    match bench_role(st.item) {
        BenchRole::Gun(kind) if !kind.uses_magazine() => Some((kind, rrig::gun(), rrig::pose([at; PARTS], st.data))),
        BenchRole::Gun(kind) => {
            let r = pv::rig(kind);
            let mods = gun_mods(st);
            let has_mag = gun_has_mag(st);
            let mut pose = rig::pose(r, [at; PARTS], mods, has_mag && gun_rounds(st) > 0, gun_chambered(st));
            with(&mut pose, kind, "magazine", has_mag);
            Some((kind, rig::gun(r, mods), pose))
        }
        // A cylinder on its own is empty.
        BenchRole::Part(kind, p) if !kind.uses_magazine() => Some((kind, rrig::part(p), rrig::pose([0.0; PARTS], 0))),
        BenchRole::Part(kind, p) => {
            let r = pv::rig(kind);
            let (ext, mods) = if p == MAGAZINE {
                ((st.item == EXTENDED_MAGAZINE) as u8 * gun_mod::EXTENDED_MAGAZINE, 0)
            } else {
                (0, gun_mods(st) & attachments_on(p))
            };
            let rounds = p == MAGAZINE && gun_rounds(st) > 0;
            Some((kind, rig::part(r, p, mods), rig::pose(r, [0.0; PARTS], mods | ext, rounds, false)))
        }
        BenchRole::Attachment(bit) => {
            let r = &pv::PISTOL;
            Some((GunKind::Pistol, rig::attachment(r, bit), rig::pose(r, [0.0; PARTS], bit, false, false)))
        }
        BenchRole::Round(kind) => {
            let r = pv::rig(kind);
            Some((kind, rig::round(r), rig::pose(r, [0.0; PARTS], 0, false, true)))
        }
        BenchRole::Speedloader => Some((GunKind::Revolver, rrig::speedloader(), rrig::loader_pose(gun_rounds(st)))),
        BenchRole::Magnum => Some((GunKind::Revolver, rrig::round(), rrig::round_pose())),
        BenchRole::Other => None,
    }
}

/// The cubes of these bones (of this gun's model) that are shown.
fn cubes<'a>(kind: GunKind, bones: Bones, shown: &'a [bool]) -> impl Iterator<Item = &'static Cube> + 'a {
    gun_view::cubes(kind).iter().filter(move |c| bones & (1 << c.bone) != 0 && shown[c.bone])
}

/// A cube's eight corners where its bone's matrix puts them.
pub(super) fn corners(c: &Cube, bone: Mat4) -> [Vec3; 8] {
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
pub(super) fn bounds(kind: GunKind, bones: Bones, pose: &[BonePose]) -> (Vec3, Vec3) {
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
#[derive(Clone)]
pub(super) struct Piece {
    pub(super) kind: GunKind,
    pub(super) pick: Option<Pick>,
    pub(super) bones: Bones,
    pub(super) mats: Vec<Mat4>,
    shown: Vec<bool>,
    pub(super) dirt: u8,
    pub(super) tint: [u8; 3],
    /// A magazine's rounds (in it, of how many it holds): the brass in its witness holes and
    /// the rounds on top show as many as there are.
    pub(super) mag: Option<(u8, u8)>,
    /// Only its top round (the one being pushed in).
    pub(super) top_round: bool,
}

impl Piece {
    pub(super) fn new(kind: GunKind, bones: Bones, pose: &[BonePose], root: Mat4, dirt: u8, pick: Option<Pick>) -> Self {
        let (mats, shown) = bone_matrices(gun_view::bones(kind), pose, root);
        Piece { kind, pick, bones, mats, shown, dirt, tint: [255; 3], mag: None, top_round: false }
    }

    /// Its cubes that are drawn.
    pub(super) fn visible(&self) -> impl Iterator<Item = &'static Cube> + '_ {
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

    /// Where its visible cubes reach (world).
    pub(super) fn reach(&self) -> (Vec3, Vec3) {
        self.visible()
            .flat_map(|c| corners(c, self.mats[c.bone]))
            .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), q| (lo.min(q), hi.max(q)))
    }

    /// Its lowest point (world).
    fn bottom(&self) -> f32 {
        self.reach().0.y
    }

    /// Only some of its bones (the same pose).
    pub(super) fn only(&self, bones: Bones, pick: Option<Pick>) -> Piece {
        Piece { bones: self.bones & bones, pick, ..self.clone() }
    }

    /// Part of the way (`k`) to where `to` has the same bones, rising by `arc` halfway.
    pub(super) fn toward(&self, to: &Piece, k: f32, arc: f32) -> Piece {
        let mats = self.mats.iter().zip(&to.mats).map(|(&a, &b)| super::anim::blend(a, b, k, arc)).collect();
        Piece { mats, ..to.clone() }
    }

    /// Where it reaches on the table (across, toward the front).
    pub(super) fn extent(&self, table: &Table) -> (Vec2, Vec2) {
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for c in self.visible() {
            for q in corners(c, self.mats[c.bone]) {
                let (x, z) = table.local(q);
                lo = lo.min(Vec2::new(x, z));
                hi = hi.max(Vec2::new(x, z));
            }
        }
        (lo, hi)
    }
}

/// The transform putting these bones (posed) lying on the table at (x, z), turned `turn`,
/// resting on it: their middle over the spot. A gun's (or part's) attachments do not count,
/// so it stays where it is as they go on and off.
pub(super) fn lying_root(table: &Table, kind: GunKind, bones: Bones, pose: &[BonePose], x: f32, z: f32, turn: f32) -> Mat4 {
    let own = pivot_bones(kind, bones);
    let (lo, hi) = bounds(kind, own, pose);
    let root = Mat4::from_scale_rotation_translation(Vec3::splat(PX), table.lying(turn), table.at(x, z))
        * Mat4::from_translation(-(lo + hi) * 0.5);
    let bottom = Piece::new(kind, own, pose, root, 0, None).bottom();
    Mat4::from_translation(Vec3::Y * (table.center.y + 0.002 - bottom)) * root
}

/// The bones something lying on the table is placed by: without the attachments on it (they
/// do not move it as they go on and off).
pub(super) fn pivot_bones(kind: GunKind, bones: Bones) -> Bones {
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
pub(super) fn lying_pieces(table: &Table, it: &BenchItem) -> Option<Vec<Piece>> {
    let (kind, bones, pose) = rig_of(&it.stack, 0.0)?;
    let root = lying_root(table, kind, bones, &pose, it.x, it.z, it.turn);
    let dirt = stack_dirt(&it.stack);
    let mut whole = Piece::new(kind, bones, &pose, root, dirt, Some(Pick::Item(it.id)));
    whole.mag = magazine_of(&it.stack);
    if !matches!(bench_role(it.stack.item), BenchRole::Gun(k) if k.uses_magazine()) {
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

/// The first of a thing's pieces lying on the table (None: it lies flat).
pub(super) fn first_piece(table: &Table, it: &BenchItem) -> Option<Piece> {
    lying_pieces(table, it).and_then(|mut v| v.drain(..).next())
}

/// The magazine on the rifle station's loader as it is drawn, lying in its cradle on its
/// side, its feed lips toward the feed block (`mount`: the loader's frame there,
/// `loader_mount`).
pub(super) fn loader_piece(mount: Mat4, mag: &Stack) -> Option<Piece> {
    let (kind, bones, pose) = rig_of(mag, 0.0)?;
    let (lo, hi) = bounds(kind, bones, &pose);
    // The magazine's up (its lips) along the loader, its side down.
    let lay = Mat4::from_cols(glam::Vec4::Y, glam::Vec4::X, -glam::Vec4::Z, glam::Vec4::W);
    let root = mount * lay * Mat4::from_scale(Vec3::splat(MODEL_TO_STATION)) * Mat4::from_translation(-(lo + hi) * 0.5);
    let mut pc = Piece::new(kind, bones, &pose, root, stack_dirt(mag), Some(Pick::Loader));
    pc.mag = magazine_of(mag);
    Some(pc)
}
