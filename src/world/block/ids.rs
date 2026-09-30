//! Block ids. Blocks with a facing or other state take a range of ids: a base id plus
//! the state bits.

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
/// Grass with the mark of a cut-down trunk in the middle of its top (bare soil in a circle,
/// smaller at each stage as the grass grows back over it), in the fluids' unused levels:
/// `STUMP_MARK + stage` on grass, `+ STUMP_STAGES + stage` on snowy grass.
pub const STUMP_MARK: u8 = WATER + FALLING + 1;
pub const STUMP_STAGES: u8 = 3;

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
