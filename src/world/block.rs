use super::textures::tex;
use glam::IVec3;

pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const SAND: u8 = 4;
pub const OAK_LOG: u8 = 5;
pub const OAK_LEAVES: u8 = 6;
pub const SNOW: u8 = 7;
pub const PLANKS: u8 = 8;
pub const COBBLE: u8 = 9;
pub const BEDROCK: u8 = 10;
pub const GLASS: u8 = 11;
pub const BRICKS: u8 = 12;
pub const GRAVEL: u8 = 13;
pub const SNOWY_GRASS: u8 = 14;
pub const SANDSTONE: u8 = 15;
pub const SPRUCE_LOG: u8 = 16;
pub const SPRUCE_LEAVES: u8 = 17;
pub const BIRCH_LOG: u8 = 18;
pub const BIRCH_LEAVES: u8 = 19;
pub const CACTUS: u8 = 20;
pub const TALL_GRASS: u8 = 21;
pub const POPPY: u8 = 22;
pub const DANDELION: u8 = 23;
pub const DEAD_BUSH: u8 = 24;
pub const COAL_ORE: u8 = 25;
pub const IRON_ORE: u8 = 26;
pub const GOLD_ORE: u8 = 27;
pub const DIAMOND_ORE: u8 = 28;
pub const OBSIDIAN: u8 = 29;
pub const ICE: u8 = 30;
pub const CLAY: u8 = 31;
pub const GLOWSTONE: u8 = 32;
pub const CRAFTING_TABLE: u8 = 33;
/// Furnace, lit furnace and chest: base id + facing (0 north/-Z, 1 east/+X, 2 south/+Z, 3 west/-X).
pub const FURNACE: u8 = 34;
pub const FURNACE_LIT: u8 = 38;
pub const CHEST: u8 = 42;
pub const TORCH: u8 = 46;
pub const OAK_SAPLING: u8 = 47;
pub const BIRCH_SAPLING: u8 = 48;
pub const SPRUCE_SAPLING: u8 = 49;
pub const IRON_BLOCK: u8 = 50;
pub const GOLD_BLOCK: u8 = 51;
pub const DIAMOND_BLOCK: u8 = 52;
pub const COAL_BLOCK: u8 = 53;
pub const STONE_BRICKS: u8 = 54;
/// Wall torches encode which adjacent block they are attached to:
/// north, east, south, west. Floor torches keep the original TORCH id.
pub const WALL_TORCH: u8 = 55;
/// Lantern standing on a block, and hanging from the block above.
pub const LANTERN: u8 = 59;
pub const LANTERN_HANGING: u8 = 60;
pub const WOOL: u8 = 61;
/// Metal workbench for assembling and cleaning guns.
pub const GUN_STATION: u8 = 62;
/// Copper ore (smelts into copper ingots) and the copper storage block.
pub const COPPER_ORE: u8 = 63;

/// Fluids: base id + level. Level 0 = source, 1..7 = flowing, 8 = falling.
pub const WATER: u8 = 64;
pub const LAVA: u8 = 80;
pub const FALLING: u8 = 8;

/// Double chest halves: base id + facing. The other half is on the chest's local +X side
/// (the viewer's right, seen from the front) for `CHEST_LEFT`, local -X for `CHEST_RIGHT`.
pub const CHEST_LEFT: u8 = 96;
pub const CHEST_RIGHT: u8 = 100;

/// Oak door halves: base id + facing (bits 0-1, the way the player looked when placing it)
/// + open (bit 2) + upper half (bit 3) + hinge on the right (bit 4) + swings out (bit 5:
/// toward the side it closes on, into the next block, instead of into its own block).
pub const OAK_DOOR: u8 = 104;
/// Oak stairs: base id + facing (bits 0-1, toward the tall back) + upside down (bit 2).
pub const OAK_STAIRS: u8 = 168;
/// Logs lying along X or Z (the plain ids stand upright).
pub const OAK_LOG_X: u8 = 176;
pub const OAK_LOG_Z: u8 = 177;
pub const SPRUCE_LOG_X: u8 = 178;
pub const SPRUCE_LOG_Z: u8 = 179;
pub const BIRCH_LOG_X: u8 = 180;
pub const BIRCH_LOG_Z: u8 = 181;
/// Branches: thin round logs growing out of the trees' trunks, upright or lying along X or
/// Z (spruce branches only lie).
pub const OAK_BRANCH: u8 = 182;
pub const OAK_BRANCH_X: u8 = 183;
pub const OAK_BRANCH_Z: u8 = 193;
pub const BIRCH_BRANCH: u8 = 251;
pub const BIRCH_BRANCH_X: u8 = 252;
pub const BIRCH_BRANCH_Z: u8 = 253;
pub const SPRUCE_BRANCH_X: u8 = 254;
pub const SPRUCE_BRANCH_Z: u8 = 255;
/// Red bed halves: base id + facing (bits 0-1, from the foot toward the head: the way the
/// player looked when placing it) + head half (bit 2).
pub const BED: u8 = 184;
pub const COPPER_BLOCK: u8 = 192;
/// Blast furnace (smelts iron too): base id + facing, lit + facing, and the chimney standing
/// on it (+ facing).
pub const BLAST_FURNACE: u8 = 194;
pub const BLAST_FURNACE_LIT: u8 = 198;
pub const CHIMNEY: u8 = 202;
/// Advanced furnace (smelts gold and diamond too), two wide and two tall: the furnace itself
/// (lower left, seen from the front) + facing, lit + facing, and its other parts,
/// `ADV_PART + (part - 1) * 4 + facing` (part 1 lower right, 2 upper left, 3 upper right),
/// glowing ones from `ADV_PART_LIT`.
pub const ADV_FURNACE: u8 = 206;
pub const ADV_FURNACE_LIT: u8 = 210;
pub const ADV_PART: u8 = 214;
pub const ADV_PART_LIT: u8 = 226;
/// The gun station, two blocks wide: base id + facing (its front, where its drawer slides
/// out, toward the player who placed it) + right half (bit 2; the left half, seen from the
/// front, holds what lies on it). The cells in front of it are kept free for the drawer.
/// (`GUN_STATION`, 62, is the old one-block station: a plain block now.)
pub const GUN_BENCH: u8 = 238;
/// The rifle station, the big gun station for the long guns, three blocks wide: its left
/// block (seen from the front) + facing, which holds what lies on it (and is the item), and
/// its other two blocks (`RIFLE_BENCH_PART`, without a facing: `bench_main` finds the left
/// block they belong to). The cells in front of it are kept free for its drawer too.
pub const RIFLE_BENCH: u8 = 246;
pub const RIFLE_BENCH_PART: u8 = 250;
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
pub fn is_door(b: u8) -> bool {
    (OAK_DOOR..OAK_DOOR + 64).contains(&b)
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
pub fn is_stairs(b: u8) -> bool {
    (OAK_STAIRS..OAK_STAIRS + 8).contains(&b)
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
pub fn is_bed(b: u8) -> bool {
    (BED..BED + 8).contains(&b)
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
pub fn is_gun_bench(b: u8) -> bool {
    (GUN_BENCH..GUN_BENCH + 8).contains(&b) || is_rifle_bench(b)
}
/// Any block of a rifle station.
pub fn is_rifle_bench(b: u8) -> bool {
    (RIFLE_BENCH..=RIFLE_BENCH_PART).contains(&b)
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
pub fn is_log(b: u8) -> bool {
    matches!(b, OAK_LOG | SPRUCE_LOG | BIRCH_LOG) || (OAK_LOG_X..=BIRCH_LOG_Z).contains(&b) || is_branch(b)
}
#[inline]
pub fn is_branch(b: u8) -> bool {
    matches!(b, OAK_BRANCH | OAK_BRANCH_X | OAK_BRANCH_Z) || (BIRCH_BRANCH..=SPRUCE_BRANCH_Z).contains(&b)
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

#[inline]
pub fn is_water(b: u8) -> bool {
    (WATER..WATER + 16).contains(&b)
}
#[inline]
pub fn is_lava(b: u8) -> bool {
    (LAVA..LAVA + 16).contains(&b)
}
#[inline]
pub fn is_fluid(b: u8) -> bool {
    is_water(b) || is_lava(b)
}
#[inline]
pub fn fluid_level(b: u8) -> u8 {
    if is_water(b) {
        b - WATER
    } else {
        b - LAVA
    }
}
#[inline]
pub fn is_leaves(b: u8) -> bool {
    matches!(b, OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES)
}
#[inline]
pub fn is_sapling(b: u8) -> bool {
    matches!(b, OAK_SAPLING | BIRCH_SAPLING | SPRUCE_SAPLING)
}
#[inline]
pub fn is_torch(b: u8) -> bool {
    b == TORCH || (WALL_TORCH..WALL_TORCH + 4).contains(&b)
}

#[inline]
pub fn is_lantern(b: u8) -> bool {
    b == LANTERN || b == LANTERN_HANGING
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
/// Cross-shaped decorations (rendered as two crossed quads, walk-through).
#[inline]
pub fn is_plant(b: u8) -> bool {
    matches!(b, TALL_GRASS | POPPY | DANDELION | DEAD_BUSH) || is_sapling(b)
}
/// A furnace of any kind (the block with the openings, not the other parts of a big one).
#[inline]
pub fn is_furnace(b: u8) -> bool {
    (FURNACE..FURNACE_LIT + 4).contains(&b)
        || (BLAST_FURNACE..BLAST_FURNACE_LIT + 4).contains(&b)
        || (ADV_FURNACE..ADV_FURNACE_LIT + 4).contains(&b)
}
#[inline]
pub fn is_chimney(b: u8) -> bool {
    (CHIMNEY..CHIMNEY + 4).contains(&b)
}
#[inline]
pub fn is_adv_part(b: u8) -> bool {
    (ADV_PART..ADV_PART_LIT + 12).contains(&b)
}
/// The kind of furnace (FURNACE, BLAST_FURNACE or ADV_FURNACE) a block belongs to, its
/// other parts included.
pub fn furnace_base(b: u8) -> Option<u8> {
    match b {
        _ if (FURNACE..FURNACE_LIT + 4).contains(&b) => Some(FURNACE),
        _ if (BLAST_FURNACE..BLAST_FURNACE_LIT + 4).contains(&b) || is_chimney(b) => {
            Some(BLAST_FURNACE)
        }
        _ if (ADV_FURNACE..ADV_FURNACE_LIT + 4).contains(&b) || is_adv_part(b) => {
            Some(ADV_FURNACE)
        }
        _ => None,
    }
}
/// What a furnace can smelt (`item::smelt_tier`): 1 the furnace, 2 the blast furnace, 3 the
/// advanced furnace.
pub fn furnace_tier(b: u8) -> u8 {
    match furnace_base(b) {
        Some(BLAST_FURNACE) => 2,
        Some(ADV_FURNACE) => 3,
        _ => 1,
    }
}
/// A burning furnace.
pub fn is_lit_furnace(b: u8) -> bool {
    is_furnace(b) && furnace_base(b).is_some_and(|base| b - base >= 4)
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
/// The front of a furnace with its openings cut out (the model has hollows behind them).
pub fn furnace_front_cut(b: u8) -> u32 {
    match furnace_base(b) {
        Some(BLAST_FURNACE) => tex::BLAST_FRONT_CUT,
        Some(ADV_FURNACE) => tex::ADV_FRONT_CUT,
        _ => tex::FURNACE_FRONT_CUT,
    }
}
#[inline]
pub fn is_chest(b: u8) -> bool {
    (CHEST..CHEST + 4).contains(&b) || (CHEST_LEFT..CHEST_RIGHT + 4).contains(&b)
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
/// Needs a solid block below it (breaks otherwise).
pub fn needs_support(b: u8) -> bool {
    is_plant(b) || is_torch(b) || is_lantern(b) || b == CACTUS || is_door(b)
}
/// Falls like sand when unsupported.
pub fn has_gravity(b: u8) -> bool {
    matches!(b, SAND | GRAVEL)
}
/// Fully hides neighbouring faces and blocks light.
#[inline]
pub fn is_opaque(b: u8) -> bool {
    !(b == AIR
        || b == GLASS
        || is_torch(b)
        || is_lantern(b)
        || is_leaves(b)
        || is_plant(b)
        || is_fluid(b)
        || is_chest(b)
        || is_door(b)
        || is_stairs(b)
        || is_bed(b)
        || is_chimney(b)
        || is_gun_bench(b)
        || is_log(b))
}
/// Blocks player movement.
#[inline]
pub fn is_solid(b: u8) -> bool {
    !(b == AIR || is_torch(b) || is_lantern(b) || is_plant(b) || is_fluid(b))
}
/// Can be overwritten by placing a block or by flowing fluid.
#[inline]
pub fn is_replaceable(b: u8) -> bool {
    b == AIR || b == TALL_GRASS || is_fluid(b)
}
/// Flowing fluid washes these away.
#[inline]
pub fn fluid_breaks(b: u8) -> bool {
    is_plant(b) || is_torch(b)
}
/// Stops full-strength sunlight (used for the heightmap).
#[inline]
pub fn attenuates_sky(b: u8) -> bool {
    !(b == AIR || b == GLASS || is_torch(b) || is_lantern(b) || is_plant(b) || is_door(b))
}
#[inline]
pub fn emission(b: u8) -> u8 {
    match b {
        _ if is_lava(b) => 15,
        GLOWSTONE => 15,
        _ if is_torch(b) => 14,
        _ if is_lantern(b) => 15,
        _ if is_lit_furnace(b) => 13,
        _ => 0,
    }
}

/// Axis (0 x, 1 y, 2 z) of each face's normal.
const FACE_AXIS: [usize; 6] = [0, 0, 1, 1, 2, 2];

/// Texture array layer for a block face. Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z.
pub fn face_texture(b: u8, face: usize) -> u32 {
    let top = face == 2;
    let bottom = face == 3;
    let ends = top || bottom;
    if log_axis(b) != 1 || is_branch(b) {
        let end = FACE_AXIS[face] == log_axis(b);
        return face_texture(log_base(b), if end { 2 } else { 0 });
    }
    match b {
        _ if is_door(b) => {
            if door_upper(b) {
                tex::DOOR_TOP
            } else {
                tex::DOOR_BOTTOM
            }
        }
        _ if is_stairs(b) => tex::PLANKS,
        // The bed is meshed on its own; this is for particles.
        _ if is_bed(b) => {
            if bottom {
                tex::BED_BOTTOM
            } else if bed_head(b) {
                tex::BED_HEAD_TOP
            } else {
                tex::BED_FOOT_TOP
            }
        }
        GRASS => {
            if top {
                tex::GRASS_TOP
            } else if bottom {
                tex::DIRT
            } else {
                tex::GRASS_SIDE
            }
        }
        SNOWY_GRASS => {
            if top {
                tex::SNOW
            } else if bottom {
                tex::DIRT
            } else {
                tex::SNOWY_GRASS_SIDE
            }
        }
        DIRT => tex::DIRT,
        STONE => tex::STONE,
        SAND => tex::SAND,
        OAK_LOG => {
            if ends {
                tex::OAK_LOG_TOP
            } else {
                tex::OAK_LOG
            }
        }
        OAK_LEAVES => tex::OAK_LEAVES,
        SNOW => tex::SNOW,
        PLANKS => tex::PLANKS,
        COBBLE => tex::COBBLE,
        BEDROCK => tex::BEDROCK,
        GLASS => tex::GLASS,
        BRICKS => tex::BRICKS,
        GRAVEL => tex::GRAVEL,
        SANDSTONE => {
            if ends {
                tex::SANDSTONE_TOP
            } else {
                tex::SANDSTONE
            }
        }
        SPRUCE_LOG => {
            if ends {
                tex::SPRUCE_LOG_TOP
            } else {
                tex::SPRUCE_LOG
            }
        }
        SPRUCE_LEAVES => tex::SPRUCE_LEAVES,
        BIRCH_LOG => {
            if ends {
                tex::BIRCH_LOG_TOP
            } else {
                tex::BIRCH_LOG
            }
        }
        BIRCH_LEAVES => tex::BIRCH_LEAVES,
        CACTUS => {
            if ends {
                tex::CACTUS_TOP
            } else {
                tex::CACTUS
            }
        }
        TALL_GRASS => tex::TALL_GRASS,
        POPPY => tex::POPPY,
        DANDELION => tex::DANDELION,
        DEAD_BUSH => tex::DEAD_BUSH,
        COAL_ORE => tex::COAL_ORE,
        IRON_ORE => tex::IRON_ORE,
        COPPER_ORE => tex::COPPER_ORE,
        GOLD_ORE => tex::GOLD_ORE,
        DIAMOND_ORE => tex::DIAMOND_ORE,
        OBSIDIAN => tex::OBSIDIAN,
        ICE => tex::ICE,
        CLAY => tex::CLAY,
        GLOWSTONE => tex::GLOWSTONE,
        CRAFTING_TABLE => match face {
            2 => tex::CRAFTING_TOP,
            3 => tex::PLANKS,
            0 | 1 => tex::CRAFTING_SIDE,
            _ => tex::CRAFTING_FRONT,
        },
        _ if is_furnace(b) => {
            let f = facing(b).unwrap();
            let (top, front, side) = match furnace_base(b) {
                Some(BLAST_FURNACE) => (tex::BLAST_TOP, tex::BLAST_FRONT, tex::BLAST_SIDE),
                Some(ADV_FURNACE) => (tex::ADV_TOP, tex::ADV_FRONT, tex::ADV_SIDE),
                _ if is_lit_furnace(b) => (
                    tex::FURNACE_TOP,
                    tex::FURNACE_FRONT_LIT,
                    tex::FURNACE_SIDE,
                ),
                _ => (tex::FURNACE_TOP, tex::FURNACE_FRONT, tex::FURNACE_SIDE),
            };
            if ends {
                top
            } else if face == front_face(f) {
                front
            } else {
                side
            }
        }
        _ if is_chimney(b) => match face {
            2 => tex::CHIMNEY_TOP,
            3 => tex::BLAST_TOP,
            _ => tex::CHIMNEY_SIDE,
        },
        _ if is_adv_part(b) => {
            let (part, lit) = adv_part(b).unwrap();
            let f = facing(b).unwrap();
            if top && part >= 2 {
                tex::ADV_VENT_TOP
            } else if ends {
                tex::ADV_TOP
            } else if face == front_face(f) {
                match (part, lit) {
                    (1, _) => tex::ADV_PANEL,
                    (2, false) => tex::ADV_HOOD_L,
                    (2, true) => tex::ADV_HOOD_L_LIT,
                    (_, false) => tex::ADV_HOOD_R,
                    _ => tex::ADV_HOOD_R_LIT,
                }
            } else {
                tex::ADV_SIDE
            }
        }
        _ if is_chest(b) => {
            if ends {
                tex::CHEST_TOP
            } else if Some(face) == facing(b).map(front_face) {
                tex::CHEST_FRONT
            } else {
                tex::CHEST_SIDE
            }
        }
        _ if is_torch(b) => tex::TORCH,
        _ if is_lantern(b) => tex::LANTERN,
        OAK_SAPLING => tex::OAK_SAPLING,
        BIRCH_SAPLING => tex::BIRCH_SAPLING,
        SPRUCE_SAPLING => tex::SPRUCE_SAPLING,
        IRON_BLOCK => tex::IRON_BLOCK,
        COPPER_BLOCK => tex::COPPER_BLOCK,
        GOLD_BLOCK => tex::GOLD_BLOCK,
        DIAMOND_BLOCK => tex::DIAMOND_BLOCK,
        COAL_BLOCK => tex::COAL_BLOCK,
        STONE_BRICKS => tex::STONE_BRICKS,
        WOOL => tex::WOOL,
        _ if is_gun_bench(b) => match face {
            2 => tex::GUN_STATION_TOP,
            3 => tex::GUN_STATION_BOTTOM,
            _ => tex::GUN_STATION_SIDE,
        },
        GUN_STATION => match face {
            2 => tex::GUN_STATION_TOP,
            3 => tex::GUN_STATION_BOTTOM,
            _ => tex::GUN_STATION_SIDE,
        },
        _ if is_water(b) => tex::WATER,
        _ if is_lava(b) => tex::LAVA,
        _ => tex::STONE,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TintKind {
    None,
    Grass,
    Foliage,
    Spruce,
    Birch,
}

pub fn tint_kind(b: u8, face: usize) -> TintKind {
    match b {
        GRASS if face != 3 => TintKind::Grass,
        TALL_GRASS => TintKind::Grass,
        OAK_LEAVES => TintKind::Foliage,
        SPRUCE_LEAVES => TintKind::Spruce,
        BIRCH_LEAVES => TintKind::Birch,
        _ => TintKind::None,
    }
}

pub const SPRUCE_TINT: [u8; 3] = [97, 153, 97];
pub const BIRCH_TINT: [u8; 3] = [128, 167, 85];

/// Tint used for inventory icons (no biome context).
pub fn icon_tint(b: u8) -> [u8; 3] {
    match b {
        GRASS | TALL_GRASS => [112, 170, 72],
        OAK_LEAVES => [90, 146, 56],
        SPRUCE_LEAVES => SPRUCE_TINT,
        BIRCH_LEAVES => BIRCH_TINT,
        _ if is_water(b) => [70, 125, 230],
        _ => [255, 255, 255],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_torch_orientation_and_block_rules() {
        for support in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let b = wall_torch_for_support(support).unwrap();
            assert_eq!(torch_support_offset(b), Some(support));
            assert!(is_torch(b) && needs_support(b) && fluid_breaks(b));
            assert!(!is_solid(b) && !is_opaque(b));
            assert_eq!(emission(b), emission(TORCH));
            assert_eq!(
                crate::item::item_of_block(b),
                Some(TORCH as crate::item::ItemId)
            );
        }
        assert_eq!(torch_support_offset(TORCH), Some(IVec3::NEG_Y));
        assert_eq!(wall_torch_for_support(IVec3::Y), None);
    }

    #[test]
    fn door_shapes_match_minecraft() {
        // Placed looking east: closed on the west side; open along the hinge side.
        assert_eq!(door_side(door_id(1, false, false, false)), IVec3::NEG_X);
        assert_eq!(door_side(door_id(1, true, false, false)), IVec3::NEG_Z);
        assert_eq!(door_side(door_id(1, true, false, true)), IVec3::Z);
        // Swung out, the panel lies in the block west of it.
        let out = door_set_open(door_id(1, false, false, false), true, true);
        let (lo, hi) = block_boxes(out, |_| AIR).b[0];
        assert!(lo[0] < 0.0 && hi[0] <= 3.0 / 16.0 + 1e-6 && lo[0] > -1.0, "{lo:?} {hi:?}");
        assert!(lo[2] == 0.0 && hi[2] == 3.0 / 16.0);
        for f in 0..4 {
            for bits in 0..8u8 {
                let b = door_id(f, bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
                assert!(is_door(b) && !is_opaque(b) && is_solid(b));
                assert_eq!(door_facing(b), f);
                for out in [false, true] {
                    let opened = door_set_open(b, true, out);
                    assert!(door_open(opened) && door_out(opened) == out);
                    assert_eq!(door_facing(opened), f);
                    assert_eq!(door_upper(opened), door_upper(b));
                    let closed = door_set_open(opened, false, false);
                    assert!(!door_open(closed) && door_out(closed) == out);
                }
                assert_eq!(crate::item::item_of_block(b), Some(OAK_DOOR as crate::item::ItemId));
            }
        }
    }

    #[test]
    fn stairs_bend_into_corners() {
        let alone = |_: IVec3| AIR;
        // Facing east: the upper half fills the east quarters (x = 1).
        assert_eq!(stairs_octants(stairs_id(1, false), alone), 0b1010_1111);
        assert_eq!(stairs_octants(stairs_id(1, true), alone), 0b1111_1010);
        // An east-facing stair with a north-facing one in front (east of it): outer corner,
        // only the north-east quarter stays up.
        let front = |d: IVec3| if d == IVec3::X { stairs_id(0, false) } else { AIR };
        assert_eq!(stairs_octants(stairs_id(1, false), front), 0b0010_1111);
        // ...and with a north-facing one behind it: inner corner, three quarters up.
        let back = |d: IVec3| if d == IVec3::NEG_X { stairs_id(0, false) } else { AIR };
        assert_eq!(stairs_octants(stairs_id(1, false), back), 0b1011_1111);
    }

    #[test]
    fn bed_halves_point_at_each_other() {
        for f in 0..4 {
            let (foot, head) = (bed_id(f, false), bed_id(f, true));
            assert!(is_bed(foot) && is_bed(head) && !is_opaque(foot) && is_solid(foot));
            assert_eq!((bed_facing(foot), bed_facing(head)), (f, f));
            assert!(!bed_head(foot) && bed_head(head));
            // The head is the way the bed faces.
            assert_eq!(bed_other_half(foot), facing_dir(f));
            assert_eq!(bed_other_half(head), -facing_dir(f));
            assert_eq!(crate::item::item_of_block(head), Some(BED as crate::item::ItemId));
        }
        assert!(!is_bed(BED + 8) && !is_bed(BIRCH_LOG_Z));
    }

    #[test]
    fn logs_lie_along_the_clicked_axis() {
        for base in [OAK_LOG, SPRUCE_LOG, BIRCH_LOG] {
            for axis in 0..3 {
                let b = log_with_axis(base, axis);
                assert!(is_log(b));
                assert_eq!(log_axis(b), axis);
                assert_eq!(log_base(b), base);
                assert_eq!(crate::item::item_of_block(b), Some(base as crate::item::ItemId));
            }
            // The ends show the rings; the bark runs along the log.
            let x = log_with_axis(base, 0);
            assert_eq!(face_texture(x, 0), face_texture(base, 2));
            assert_eq!(face_texture(x, 2), face_texture(base, 0));
            assert!(face_rotated(x, 2) && !face_rotated(x, 0));
        }
    }

    #[test]
    fn double_chest_halves_point_at_each_other() {
        for f in 0..4 {
            assert_eq!(chest_partner_offset(chest_id(f, 0)), None);
            for side in [-1, 1] {
                let b = chest_id(f, side);
                assert!(is_chest(b) && !is_opaque(b));
                assert_eq!(facing(b), Some(f));
                let d = chest_partner_offset(b).unwrap();
                assert_eq!(d, chest_right(f) * side);
                let other = chest_other_half(b).unwrap();
                assert_eq!(chest_partner_offset(other), Some(-d));
                assert_eq!(facing(other), Some(f));
                assert_eq!(
                    crate::item::item_of_block(b),
                    Some(CHEST as crate::item::ItemId)
                );
            }
        }
    }
}
