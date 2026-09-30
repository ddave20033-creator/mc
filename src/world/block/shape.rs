//! The shapes of blocks with a facing or several parts: doors, stairs, beds, gun
//! stations, logs, torches, chests and the big furnaces (their orientation and halves),
//! and the boxes blocks are made of.

use super::*;
use glam::IVec3;

/// The chimney's boxes (block-local): a slab over the furnace, the stack and a rim round
/// its top.
pub const CHIMNEY_BOXES: [([f32; 3], [f32; 3]); 3] = [
    ([0.0, 0.0, 0.0], [1.0, 5.0 / 16.0, 1.0]),
    ([4.0 / 16.0, 5.0 / 16.0, 4.0 / 16.0], [12.0 / 16.0, 14.0 / 16.0, 12.0 / 16.0]),
    ([3.0 / 16.0, 14.0 / 16.0, 3.0 / 16.0], [13.0 / 16.0, 1.0, 13.0 / 16.0]),
];
/// Height of a bed's top (Minecraft: 9 pixels).
pub const BED_HEIGHT: f32 = 9.0 / 16.0;

/// Horizontal unit vectors of the facings (0 north/-Z, 1 east/+X, 2 south/+Z, 3 west/-X).
pub fn facing_dir(f: u8) -> IVec3 {
    [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X][f as usize & 3]
}

/// Facing toward a horizontal direction (the larger of its x and z).
pub fn facing_of(dx: f32, dz: f32) -> u8 {
    if dx.abs() > dz.abs() {
        if dx > 0.0 {
            1
        } else {
            3
        }
    } else if dz > 0.0 {
        2
    } else {
        0
    }
}

#[inline]
pub const fn is_door(b: u8) -> bool {
    b >= OAK_DOOR && b < OAK_DOOR + 64
}
pub fn door_id(facing: u8, open: bool, upper: bool, hinge_right: bool) -> u8 {
    OAK_DOOR
        + (facing & 3)
        + ((open as u8) << 2)
        + ((upper as u8) << 3)
        + ((hinge_right as u8) << 4)
}
pub fn door_facing(b: u8) -> u8 {
    (b - OAK_DOOR) & 3
}
pub fn door_open(b: u8) -> bool {
    (b - OAK_DOOR) & 4 != 0
}
pub fn door_upper(b: u8) -> bool {
    (b - OAK_DOOR) & 8 != 0
}
pub fn door_hinge_right(b: u8) -> bool {
    (b - OAK_DOOR) & 16 != 0
}
pub fn door_out(b: u8) -> bool {
    (b - OAK_DOOR) & 32 != 0
}
/// The same door half opened (swinging out or in) or closed. A closed door keeps the way it
/// last swung, so it swings back the same way.
pub fn door_set_open(b: u8, open: bool, out: bool) -> u8 {
    let keep = (b - OAK_DOOR) & !(4 | 32);
    let out = if open { out } else { door_out(b) };
    OAK_DOOR + keep + ((open as u8) << 2) + ((out as u8) << 5)
}
/// Offset from a door half to its other half.
pub fn door_other_half(b: u8) -> IVec3 {
    if door_upper(b) {
        IVec3::NEG_Y
    } else {
        IVec3::Y
    }
}
/// The block side the door panel lies against, like Minecraft's door shapes: closed on the
/// side the player placed it from, open along the hinge side.
pub fn door_side_facing(facing: u8, open: bool, hinge_right: bool) -> u8 {
    (if !open {
        facing + 2
    } else if hinge_right {
        facing + 1
    } else {
        facing + 3
    }) & 3
}
pub fn door_side(b: u8) -> IVec3 {
    facing_dir(door_side_facing(
        door_facing(b),
        door_open(b),
        door_hinge_right(b),
    ))
}

#[inline]
pub const fn is_stairs(b: u8) -> bool {
    b >= OAK_STAIRS && b < OAK_STAIRS + 8
}
pub fn stairs_id(facing: u8, upside_down: bool) -> u8 {
    OAK_STAIRS + (facing & 3) + ((upside_down as u8) << 2)
}
pub fn stairs_facing(b: u8) -> u8 {
    (b - OAK_STAIRS) & 3
}
pub fn stairs_upside_down(b: u8) -> bool {
    (b - OAK_STAIRS) & 4 != 0
}

/// Which eighths of the block a stair fills: bit `x + 2 * z + 4 * y` for the half-block
/// cube at (x, y, z) in 0..2. Straight, or an inner/outer corner when it meets another stair
/// at its front or back (Minecraft's stair shapes). `get` reads a block at an offset.
pub fn stairs_octants(b: u8, get: impl Fn(IVec3) -> u8) -> u8 {
    let f = stairs_facing(b);
    let up = stairs_upside_down(b);
    let d = facing_dir(f);
    let left = facing_dir(f + 3);
    let same_half = |o: u8| is_stairs(o) && stairs_upside_down(o) == up;
    // A neighbour at `dir` that would not let this one bend toward it.
    let can_take = |dir: IVec3| {
        let o = get(dir);
        !(same_half(o) && stairs_facing(o) == f)
    };
    // 0 straight, 1 outer left, 2 outer right, 3 inner left, 4 inner right.
    let mut shape = 0;
    let front = get(d);
    if same_half(front) {
        let f2 = stairs_facing(front);
        if (f2 & 1) != (f & 1) && can_take(-facing_dir(f2)) {
            shape = if f2 == (f + 3) & 3 { 1 } else { 2 };
        }
    }
    if shape == 0 {
        let back = get(-d);
        if same_half(back) {
            let f3 = stairs_facing(back);
            if (f3 & 1) != (f & 1) && can_take(facing_dir(f3)) {
                shape = if f3 == (f + 3) & 3 { 3 } else { 4 };
            }
        }
    }
    let (low, high) = if up { (1, 0) } else { (0, 1) };
    let mut bits = 0u8;
    for z in 0..2 {
        for x in 0..2 {
            bits |= 1 << (x + 2 * z + 4 * low);
            // Quarter center relative to the block center.
            let c = IVec3::new(2 * x - 1, 0, 2 * z - 1);
            let back = c.dot(d) > 0;
            let on_left = c.dot(left) > 0;
            let fill = match shape {
                0 => back,
                1 => back && on_left,
                2 => back && !on_left,
                3 => back || on_left,
                _ => back || !on_left,
            };
            if fill {
                bits |= 1 << (x + 2 * z + 4 * high);
            }
        }
    }
    bits
}

#[inline]
pub const fn is_bed(b: u8) -> bool {
    b >= BED && b < BED + 8
}
pub fn bed_id(facing: u8, head: bool) -> u8 {
    BED + (facing & 3) + ((head as u8) << 2)
}
pub fn bed_facing(b: u8) -> u8 {
    (b - BED) & 3
}
pub fn bed_head(b: u8) -> bool {
    (b - BED) & 4 != 0
}
/// Any block of a gun station (the small one or the rifle station).
#[inline]
pub const fn is_gun_bench(b: u8) -> bool {
    (b >= GUN_BENCH && b < GUN_BENCH + 8) || is_rifle_bench(b)
}
/// Any block of a rifle station.
pub const fn is_rifle_bench(b: u8) -> bool {
    b >= RIFLE_BENCH && b <= RIFLE_BENCH_PART
}
pub fn rifle_bench_id(facing: u8) -> u8 {
    RIFLE_BENCH + (facing & 3)
}
/// How many blocks wide the station a block is part of is.
pub fn bench_width(b: u8) -> i32 {
    if is_rifle_bench(b) {
        3
    } else {
        2
    }
}
/// The block of a station that holds what lies on it (its left one, seen from the front).
pub fn is_bench_main(b: u8) -> bool {
    if is_rifle_bench(b) {
        b != RIFLE_BENCH_PART
    } else {
        is_gun_bench(b) && !gun_bench_right(b)
    }
}
/// The left block of the station that the block `b` at `p` is part of (`get`: the block at a
/// place; a rifle station's other blocks look for it beside them).
pub fn bench_main(p: IVec3, b: u8, get: impl Fn(IVec3) -> u8) -> Option<IVec3> {
    if b == RIFLE_BENCH_PART {
        for k in 1..=2 {
            for f in 0..4u8 {
                let q = p - chest_right(f) * k;
                if get(q) == rifle_bench_id(f) {
                    return Some(q);
                }
            }
        }
        return None;
    }
    is_gun_bench(b).then(|| if is_rifle_bench(b) { p } else { gun_bench_main(p, b) })
}
/// The blocks of the station whose left block (`main`, the block `b`) is there, left to right.
pub fn bench_cells(main: IVec3, b: u8) -> Vec<IVec3> {
    let Some(f) = facing(b) else { return vec![main] };
    (0..bench_width(b)).map(|i| main + chest_right(f) * i).collect()
}
pub fn gun_bench_id(facing: u8, right: bool) -> u8 {
    GUN_BENCH + (facing & 3) + ((right as u8) << 2)
}
pub fn gun_bench_right(b: u8) -> bool {
    (b - GUN_BENCH) & 4 != 0
}
/// Offset from a gun station half to its other half.
pub fn gun_bench_other_half(b: u8) -> IVec3 {
    let r = chest_right((b - GUN_BENCH) & 3);
    if gun_bench_right(b) {
        -r
    } else {
        r
    }
}
/// The left half of the gun station a half belongs to (it keeps what lies on the table).
pub fn gun_bench_main(p: IVec3, b: u8) -> IVec3 {
    if gun_bench_right(b) {
        p + gun_bench_other_half(b)
    } else {
        p
    }
}
/// Offset from a bed half to its other half.
pub fn bed_other_half(b: u8) -> IVec3 {
    let d = facing_dir(bed_facing(b));
    if bed_head(b) {
        -d
    } else {
        d
    }
}

/// A log or a branch (both round, wood).
#[inline]
pub const fn is_log(b: u8) -> bool {
    matches!(b, OAK_LOG | SPRUCE_LOG | BIRCH_LOG) || (b >= OAK_LOG_X && b <= BIRCH_LOG_Z) || is_branch(b)
}
#[inline]
pub const fn is_branch(b: u8) -> bool {
    matches!(b, OAK_BRANCH | OAK_BRANCH_X | OAK_BRANCH_Z) || (b >= BIRCH_BRANCH && b <= SPRUCE_BRANCH_Z)
}
/// The upright log of a log block (of a branch: of its tree).
pub fn log_base(b: u8) -> u8 {
    match b {
        OAK_LOG_X | OAK_LOG_Z | OAK_BRANCH | OAK_BRANCH_X | OAK_BRANCH_Z => OAK_LOG,
        SPRUCE_LOG_X | SPRUCE_LOG_Z | SPRUCE_BRANCH_X | SPRUCE_BRANCH_Z => SPRUCE_LOG,
        BIRCH_LOG_X | BIRCH_LOG_Z | BIRCH_BRANCH | BIRCH_BRANCH_X | BIRCH_BRANCH_Z => BIRCH_LOG,
        _ => b,
    }
}
/// A branch of the tree of `log` along `axis` (0 x, 1 y, 2 z; spruce ones only lie).
pub fn branch_with_axis(log: u8, axis: usize) -> u8 {
    match (log, axis) {
        (SPRUCE_LOG, 2) => SPRUCE_BRANCH_Z,
        (SPRUCE_LOG, _) => SPRUCE_BRANCH_X,
        (BIRCH_LOG, 0) => BIRCH_BRANCH_X,
        (BIRCH_LOG, 2) => BIRCH_BRANCH_Z,
        (BIRCH_LOG, _) => BIRCH_BRANCH,
        (_, 0) => OAK_BRANCH_X,
        (_, 2) => OAK_BRANCH_Z,
        _ => OAK_BRANCH,
    }
}
/// How thick a round log is: its radius (blocks).
pub fn log_radius(b: u8) -> f32 {
    if is_branch(b) {
        0.19
    } else {
        0.44
    }
}
/// Log lying along `axis` (0 x, 1 y, 2 z).
pub fn log_with_axis(base: u8, axis: usize) -> u8 {
    let (x, z) = match base {
        SPRUCE_LOG => (SPRUCE_LOG_X, SPRUCE_LOG_Z),
        BIRCH_LOG => (BIRCH_LOG_X, BIRCH_LOG_Z),
        _ => (OAK_LOG_X, OAK_LOG_Z),
    };
    match axis {
        0 => x,
        2 => z,
        _ => base,
    }
}
/// Axis a log runs along (0 x, 1 y, 2 z).
pub fn log_axis(b: u8) -> usize {
    match b {
        OAK_LOG_X | SPRUCE_LOG_X | BIRCH_LOG_X | OAK_BRANCH_X | BIRCH_BRANCH_X | SPRUCE_BRANCH_X => 0,
        OAK_LOG_Z | SPRUCE_LOG_Z | BIRCH_LOG_Z | OAK_BRANCH_Z | BIRCH_BRANCH_Z | SPRUCE_BRANCH_Z => 2,
        _ => 1,
    }
}
/// The face's texture is turned a quarter (the bark of a log lying on its side).
pub fn face_rotated(b: u8, face: usize) -> bool {
    match log_axis(b) {
        0 => face >= 2,
        2 => face < 2,
        _ => false,
    }
}

/// Up to 8 boxes (block-local corners in 0..1) a block is made of, for collisions and aiming.
#[derive(Clone, Copy)]
pub struct Boxes {
    pub n: usize,
    pub b: [([f32; 3], [f32; 3]); 8],
}

impl Boxes {
    pub fn one(lo: [f32; 3], hi: [f32; 3]) -> Self {
        let mut b = [([0.0; 3], [0.0; 3]); 8];
        b[0] = (lo, hi);
        Self { n: 1, b }
    }
    pub fn iter(&self) -> impl Iterator<Item = &([f32; 3], [f32; 3])> {
        self.b[..self.n].iter()
    }
    /// The smallest box around all of them.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut lo = [1.0f32; 3];
        let mut hi = [0.0f32; 3];
        for (a, b) in self.iter() {
            for k in 0..3 {
                lo[k] = lo[k].min(a[k]);
                hi[k] = hi[k].max(b[k]);
            }
        }
        (lo, hi)
    }
}

/// Box of a door panel against side `s` of the block.
pub fn door_panel(s: IVec3) -> ([f32; 3], [f32; 3]) {
    let t = 3.0 / 16.0;
    let mut lo = [0.0; 3];
    let mut hi = [1.0; 3];
    for k in [0, 2] {
        match s[k] {
            1 => lo[k] = 1.0 - t,
            -1 => hi[k] = t,
            _ => {}
        }
    }
    (lo, hi)
}

/// The side a door panel closes on (unit vector from the block center).
pub fn door_closed_side(b: u8) -> IVec3 {
    facing_dir(door_side_facing(door_facing(b), false, door_hinge_right(b)))
}

/// The shape of a solid block (a full cube for most). `get` reads a block at an offset.
/// A door swung out reaches into the next block.
pub fn block_boxes(b: u8, get: impl Fn(IVec3) -> u8) -> Boxes {
    if is_door(b) {
        let (mut lo, mut hi) = door_panel(door_side(b));
        if door_open(b) && door_out(b) {
            let c = door_closed_side(b);
            let shift = 1.0 - 3.0 / 16.0;
            for k in [0, 2] {
                lo[k] += c[k] as f32 * shift;
                hi[k] += c[k] as f32 * shift;
            }
        }
        return Boxes::one(lo, hi);
    }
    if is_stairs(b) {
        let bits = stairs_octants(b, get);
        let mut out = Boxes {
            n: 0,
            b: [([0.0; 3], [0.0; 3]); 8],
        };
        for i in 0..8 {
            if bits & (1 << i) != 0 {
                let o = [
                    (i & 1) as f32 * 0.5,
                    (i >> 2) as f32 * 0.5,
                    ((i >> 1) & 1) as f32 * 0.5,
                ];
                out.b[out.n] = (o, [o[0] + 0.5, o[1] + 0.5, o[2] + 0.5]);
                out.n += 1;
            }
        }
        return out;
    }
    if is_bed(b) {
        return Boxes::one([0.0; 3], [1.0, BED_HEIGHT, 1.0]);
    }
    if is_log(b) {
        // Round: the square inside its circle, along its axis.
        let r = log_radius(b) * 0.92;
        let (mut lo, mut hi) = ([0.5 - r; 3], [0.5 + r; 3]);
        let a = log_axis(b);
        (lo[a], hi[a]) = (0.0, 1.0);
        return Boxes::one(lo, hi);
    }
    if is_chimney(b) {
        let mut out = Boxes::one(CHIMNEY_BOXES[0].0, CHIMNEY_BOXES[0].1);
        for &bx in &CHIMNEY_BOXES[1..] {
            out.b[out.n] = bx;
            out.n += 1;
        }
        return out;
    }
    Boxes::one([0.0; 3], [1.0; 3])
}

/// Offset from a torch block to the block holding it up.
pub fn torch_support_offset(b: u8) -> Option<IVec3> {
    match b {
        TORCH | LANTERN => Some(IVec3::NEG_Y),
        LANTERN_HANGING => Some(IVec3::Y),
        WALL_TORCH => Some(IVec3::NEG_Z),
        x if x == WALL_TORCH + 1 => Some(IVec3::X),
        x if x == WALL_TORCH + 2 => Some(IVec3::Z),
        x if x == WALL_TORCH + 3 => Some(IVec3::NEG_X),
        _ => None,
    }
}

pub fn wall_torch_for_support(offset: IVec3) -> Option<u8> {
    match offset {
        IVec3::NEG_Z => Some(WALL_TORCH),
        IVec3::X => Some(WALL_TORCH + 1),
        IVec3::Z => Some(WALL_TORCH + 2),
        IVec3::NEG_X => Some(WALL_TORCH + 3),
        _ => None,
    }
}

/// A furnace of kind `base` facing `facing`, burning or not.
pub fn furnace_id(base: u8, facing: u8, lit: bool) -> u8 {
    base + if lit { 4 } else { 0 } + (facing & 3)
}
/// Which part of an advanced furnace a block is (1 lower right, 2 upper left, 3 upper
/// right) and whether it glows.
pub fn adv_part(b: u8) -> Option<(u8, bool)> {
    is_adv_part(b).then(|| {
        let i = b - ADV_PART;
        ((i % 12) / 4 + 1, i >= 12)
    })
}
pub fn adv_part_id(part: u8, facing: u8, lit: bool) -> u8 {
    (if lit { ADV_PART_LIT } else { ADV_PART }) + (part - 1) * 4 + (facing & 3)
}
/// To the right of a furnace facing `facing`, as seen from in front of it.
pub fn furnace_right(facing: u8) -> IVec3 {
    let n = facing_dir(facing);
    IVec3::new(n.z, 0, -n.x)
}
/// The blocks of a furnace of kind `base` with its furnace block at the origin: (offset,
/// block). A blast furnace has its chimney on top; an advanced furnace is two wide (to its
/// right) and two tall.
pub fn furnace_cells(base: u8, facing: u8, lit: bool) -> Vec<(IVec3, u8)> {
    let f = furnace_id(base, facing, lit);
    match base {
        BLAST_FURNACE => vec![(IVec3::ZERO, f), (IVec3::Y, CHIMNEY + (facing & 3))],
        ADV_FURNACE => {
            let r = furnace_right(facing);
            vec![
                (IVec3::ZERO, f),
                (r, adv_part_id(1, facing, lit)),
                (IVec3::Y, adv_part_id(2, facing, lit)),
                (r + IVec3::Y, adv_part_id(3, facing, lit)),
            ]
        }
        _ => vec![(IVec3::ZERO, f)],
    }
}
/// Where the furnace block of the big furnace that the block `b` at `p` is part of is.
pub fn furnace_origin(p: IVec3, b: u8) -> IVec3 {
    if is_chimney(b) {
        return p - IVec3::Y;
    }
    if let (Some((part, _)), Some(f)) = (adv_part(b), facing(b)) {
        let r = furnace_right(f);
        return p - match part {
            1 => r,
            2 => IVec3::Y,
            _ => r + IVec3::Y,
        };
    }
    p
}

/// Facing of a directional block.
pub fn facing(b: u8) -> Option<u8> {
    match b {
        _ if is_furnace(b) => furnace_base(b).map(|base| (b - base) & 3),
        _ if is_chimney(b) => Some(b - CHIMNEY),
        _ if is_adv_part(b) => Some((b - ADV_PART) & 3),
        _ if (CHEST..CHEST + 4).contains(&b) => Some(b - CHEST),
        _ if (CHEST_LEFT..CHEST_RIGHT + 4).contains(&b) => Some((b - CHEST_LEFT) & 3),
        _ if is_rifle_bench(b) => (b != RIFLE_BENCH_PART).then_some((b - RIFLE_BENCH) & 3),
        _ if is_gun_bench(b) => Some((b - GUN_BENCH) & 3),
        _ => None,
    }
}
/// A chest's local +X axis in the world (its front is local +Z): the viewer's right.
pub fn chest_right(facing: u8) -> IVec3 {
    [IVec3::NEG_X, IVec3::NEG_Z, IVec3::X, IVec3::Z][facing as usize & 3]
}
/// Offset from a double chest half to its other half.
pub fn chest_partner_offset(b: u8) -> Option<IVec3> {
    let f = facing(b)?;
    match b {
        _ if (CHEST_LEFT..CHEST_LEFT + 4).contains(&b) => Some(chest_right(f)),
        _ if (CHEST_RIGHT..CHEST_RIGHT + 4).contains(&b) => Some(-chest_right(f)),
        _ => None,
    }
}
/// The id of a double chest half's other half.
pub fn chest_other_half(b: u8) -> Option<u8> {
    let f = facing(b)?;
    match b {
        _ if (CHEST_LEFT..CHEST_LEFT + 4).contains(&b) => Some(CHEST_RIGHT + f),
        _ if (CHEST_RIGHT..CHEST_RIGHT + 4).contains(&b) => Some(CHEST_LEFT + f),
        _ => None,
    }
}
/// Chest id for a facing: single (`side` 0), or the half whose partner is at
/// `side * chest_right(facing)`.
pub fn chest_id(facing: u8, side: i32) -> u8 {
    let f = facing & 3;
    match side.signum() {
        1 => CHEST_LEFT + f,
        -1 => CHEST_RIGHT + f,
        _ => CHEST + f,
    }
}
/// Face index (0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z) that a facing points to.
pub fn front_face(facing: u8) -> usize {
    [5, 0, 4, 1][facing as usize & 3]
}
