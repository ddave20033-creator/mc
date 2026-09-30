//! Every block, in one table (`blocks!` below). A line gives a block its constant, its key
//! (commands, save files), its names, how it looks (its model, faces, tint and light), what it
//! does in the world (solid, sunlight, support, gravity, fluids), how it is mined and what it
//! drops, the item it is, where it is in the creative inventory, and how it is placed.
//!
//! Ids are not written anywhere: they follow from the order of the table. A block with a
//! state (a facing, a door's halves...) takes `states` ids after its constant, the state
//! being `b - BASE`. Save files keep the blocks' keys (`save`), so the order can change.
//!
//! Adding a plain block is adding a line here (and its texture: `tex`, the painter in
//! `textures::procedural` and a resource pack name in `textures::pack`).

use crate::item::{ItemId, Stack, ToolKind, CLAY_BALL, COAL, LAVA_BUCKET, SHEARS, STICK, WATER_BUCKET};
use crate::world::block::*;
use crate::world::textures::tex;

/// A block id: a block and its state.
pub type Block = u16;

/// How a block is drawn (and which family of blocks it is: `is_leaves`, `is_torch`...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    /// Nothing drawn.
    Air,
    /// A full cube with a texture on each face.
    Cube,
    /// A cube whose faces toward other leaves stay (a full looking canopy).
    Leaves,
    /// Two crossed quads (flowers, grass, saplings).
    Plant,
    Fluid,
    Torch,
    Lantern,
    Chest,
    Furnace,
    Chimney,
    Door,
    /// A gun or rifle station (drawn every frame from its left block).
    GunBench,
    Stairs,
    /// A round log or branch.
    Log,
    Bed,
}

/// The texture layer of each face (faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z).
#[derive(Clone, Copy)]
pub enum Faces {
    All(u32),
    /// The same on the four sides, `end` on top and at the bottom.
    Column { side: u32, end: u32 },
    Sides { top: u32, side: u32, bottom: u32 },
    /// Worked out from the state (a facing, a lit furnace...).
    Custom(fn(Block, usize) -> u32),
}

/// The color a block's texture is multiplied with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TintKind {
    None,
    /// The biome's grass color (not on the bottom face).
    Grass,
    Foliage,
    Spruce,
    Birch,
}

/// How a block is mined: Minecraft's hardness (bare hands, harvestable: hardness * 1.5 s),
/// the tool that mines it faster, and the tool's harvest level it needs to drop anything
/// (`Tier::level`: 0 wood, 1 stone, 2 copper, 3 iron, 4 diamond).
#[derive(Clone, Copy)]
pub struct Mine {
    pub hardness: f32,
    pub tool: Option<ToolKind>,
    pub needs: Option<u8>,
}

/// What a mined block drops (when it may drop: see `Mine::needs`).
#[derive(Clone, Copy)]
pub enum Drops {
    /// The item it counts as (`Item`).
    Itself,
    Nothing,
    Item(ItemId, u8),
    /// (block, held item, a random number in 0..1)
    Custom(fn(Block, ItemId, f32) -> Vec<Stack>),
}

/// The item a block is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlockItem {
    None,
    /// Its own item (id = the block's constant), stacking up to `stack`.
    Own { stack: u8 },
    /// Counts as another block's item (a lit furnace, the other half of a bed...).
    As(Block),
    /// Counts as an item that is not a block (water: the water bucket).
    Other(ItemId),
}

/// Where the block's item is in the creative inventory: the tab and the group (each group
/// starts on a new row; in a group, in the order of the table).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Creative {
    None,
    Blocks(u8),
    Functional(u8),
}

/// How the block's item is placed (`game::player::items`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// As it is.
    Plain,
    /// Its front toward the player.
    Facing,
    /// Standing on the floor, or on a wall.
    Torch,
    /// Standing, or hanging from a ceiling.
    Lantern,
    /// Single, or joined into a double chest.
    Chest,
    Stairs,
    /// Along the clicked face's axis (a branch out of wood).
    Log,
    Door,
    Bed,
    /// A furnace of several blocks (blast furnace, advanced furnace).
    BigFurnace,
    GunBench,
    RifleBench,
}

/// A block: one line of the table.
#[derive(Clone, Copy)]
pub struct BlockDef {
    /// Its first id (its constant) and how many it takes.
    pub id: Block,
    pub states: u16,
    pub key: &'static str,
    pub en: &'static str,
    pub hu: &'static str,
    pub model: Model,
    /// Fully hides the faces next to it and stops light.
    pub opaque: bool,
    /// Blocks movement.
    pub solid: bool,
    /// Stops full sunlight (the heightmap).
    pub stops_sky: bool,
    /// Light it gives off (0..=15).
    pub light: u8,
    pub faces: Faces,
    pub tint: TintKind,
    /// Hides its faces toward the same block (glass).
    pub cull_same: bool,
    /// Placing a block, or flowing fluid, overwrites it.
    pub replaceable: bool,
    /// Breaks without a solid block under it.
    pub needs_support: bool,
    /// Falls when nothing holds it up.
    pub gravity: bool,
    /// Flowing fluid washes it away.
    pub washes_away: bool,
    /// None: it cannot be mined.
    pub mine: Option<Mine>,
    pub drops: Drops,
    /// Sheared, it drops itself.
    pub shears: bool,
    pub item: BlockItem,
    /// Its item's icon, when not the block drawn as a cube.
    pub icon: Option<u32>,
    pub creative: Creative,
    pub place: Place,
}

/// A solid, opaque full cube, mined by hand in 1.5 s, dropping itself: most blocks.
const CUBE: BlockDef = BlockDef {
    id: 0,
    states: 1,
    key: "",
    en: "",
    hu: "",
    model: Model::Cube,
    opaque: true,
    solid: true,
    stops_sky: true,
    light: 0,
    faces: Faces::All(tex::STONE),
    tint: TintKind::None,
    cull_same: false,
    replaceable: false,
    needs_support: false,
    gravity: false,
    washes_away: false,
    mine: mine(1.0, None, None),
    drops: Drops::Itself,
    shears: false,
    item: BlockItem::Own { stack: 64 },
    icon: None,
    creative: Creative::None,
    place: Place::Plain,
};

/// Something small in a block: not solid, lets light and sunlight through.
const THIN: BlockDef = BlockDef { opaque: false, solid: false, stops_sky: false, ..CUBE };

/// Walk-through plants on the ground, broken at once and washed away.
const PLANT: BlockDef = BlockDef {
    model: Model::Plant,
    needs_support: true,
    washes_away: true,
    mine: mine(0.0, None, None),
    ..THIN
};

const LEAVES: BlockDef = BlockDef {
    model: Model::Leaves,
    opaque: false,
    mine: mine(0.2, Some(ToolKind::Sword), None),
    drops: Drops::Custom(leaf_drops),
    shears: true,
    creative: Creative::Blocks(3),
    ..CUBE
};

/// A round log, mined with an axe.
const LOG: BlockDef = BlockDef {
    model: Model::Log,
    opaque: false,
    mine: mine(2.0, Some(ToolKind::Axe), None),
    place: Place::Log,
    ..CUBE
};

const fn mine(hardness: f32, tool: Option<ToolKind>, needs: Option<u8>) -> Option<Mine> {
    Some(Mine { hardness, tool, needs })
}
/// Mined with a pickaxe of at least harvest level `needs`.
const fn pick(hardness: f32, needs: u8) -> Option<Mine> {
    mine(hardness, Some(ToolKind::Pickaxe), Some(needs))
}
const fn shovel(hardness: f32) -> Option<Mine> {
    mine(hardness, Some(ToolKind::Shovel), None)
}
const fn axe(hardness: f32) -> Option<Mine> {
    mine(hardness, Some(ToolKind::Axe), None)
}

/// Declares the blocks: `NAME [* states] = BlockDef { .. };`. Each gets the constant `NAME`
/// (the id after the previous block's ids) and its line in `BLOCKS`.
macro_rules! blocks {
    ($( $(#[$m:meta])* $name:ident $(* $n:literal)? = $def:expr; )*) => {
        blocks!(@ids 0; $( $(#[$m])* $name $(* $n)?; )*);
        /// Every block, in the order of their ids.
        pub const BLOCKS: &[BlockDef] = &[$( BlockDef { id: $name, states: blocks!(@n $($n)?), ..$def } ),*];
    };
    (@ids $at:expr; ) => {
        /// How many block ids there are.
        pub const BLOCK_IDS: usize = ($at) as usize;
    };
    (@ids $at:expr; $(#[$m:meta])* $name:ident $(* $n:literal)?; $($rest:tt)*) => {
        $(#[$m])*
        pub const $name: Block = $at;
        blocks!(@ids $name + blocks!(@n $($n)?); $($rest)*);
    };
    (@n) => { 1 };
    (@n $n:literal) => { $n };
}

blocks! {
    AIR = BlockDef {
        key: "air", en: "-", hu: "-", model: Model::Air, replaceable: true, mine: None,
        drops: Drops::Nothing, item: BlockItem::None, ..THIN
    };

    // Earth.
    GRASS = BlockDef {
        key: "grass_block", en: "Grass Block", hu: "Füves blokk",
        faces: Faces::Sides { top: tex::GRASS_TOP, side: tex::GRASS_SIDE, bottom: tex::DIRT },
        tint: TintKind::Grass, mine: shovel(0.6), drops: Drops::Item(DIRT, 1),
        creative: Creative::Blocks(0), ..CUBE
    };
    SNOWY_GRASS = BlockDef {
        key: "snowy_grass_block", en: "Snowy Grass Block", hu: "Havas füves blokk",
        faces: Faces::Sides { top: tex::SNOW, side: tex::SNOWY_GRASS_SIDE, bottom: tex::DIRT },
        mine: shovel(0.6), drops: Drops::Item(DIRT, 1), creative: Creative::Blocks(0), ..CUBE
    };
    DIRT = BlockDef {
        key: "dirt", en: "Dirt", hu: "Föld", faces: Faces::All(tex::DIRT), mine: shovel(0.5),
        creative: Creative::Blocks(0), ..CUBE
    };
    SAND = BlockDef {
        key: "sand", en: "Sand", hu: "Homok", faces: Faces::All(tex::SAND), mine: shovel(0.5),
        gravity: true, creative: Creative::Blocks(0), ..CUBE
    };
    GRAVEL = BlockDef {
        key: "gravel", en: "Gravel", hu: "Kavics", faces: Faces::All(tex::GRAVEL),
        mine: shovel(0.6), gravity: true, creative: Creative::Blocks(0), ..CUBE
    };
    CLAY = BlockDef {
        key: "clay", en: "Clay", hu: "Agyag", faces: Faces::All(tex::CLAY), mine: shovel(0.6),
        drops: Drops::Item(CLAY_BALL, 4), creative: Creative::Blocks(0), ..CUBE
    };
    SNOW = BlockDef {
        key: "snow_block", en: "Snow Block", hu: "Hóblokk", faces: Faces::All(tex::SNOW),
        mine: mine(0.2, Some(ToolKind::Shovel), Some(0)), creative: Creative::Blocks(0), ..CUBE
    };
    ICE = BlockDef {
        key: "ice", en: "Ice", hu: "Jég", faces: Faces::All(tex::ICE),
        mine: mine(0.5, Some(ToolKind::Pickaxe), None), drops: Drops::Nothing, cull_same: true,
        creative: Creative::Blocks(0), ..CUBE
    };
    /// Grass with the mark of a cut-down trunk in the middle of its top (bare soil in a
    /// circle, smaller at each stage as the grass grows back over it): `+ stage`.
    STUMP_MARK * 3 = BlockDef {
        key: "stump_mark", en: "Grass Block", hu: "Füves blokk", tint: TintKind::Grass,
        faces: Faces::Sides { top: tex::GRASS_TOP, side: tex::GRASS_SIDE, bottom: tex::DIRT },
        mine: shovel(0.6), drops: Drops::Item(DIRT, 1), item: BlockItem::As(GRASS), ..CUBE
    };
    /// The same on snowy grass.
    STUMP_MARK_SNOWY * 3 = BlockDef {
        key: "snowy_stump_mark", en: "Snowy Grass Block", hu: "Havas füves blokk",
        faces: Faces::Sides { top: tex::SNOW, side: tex::SNOWY_GRASS_SIDE, bottom: tex::DIRT },
        mine: shovel(0.6), drops: Drops::Item(DIRT, 1), item: BlockItem::As(SNOWY_GRASS), ..CUBE
    };

    // Stone.
    STONE = BlockDef {
        key: "stone", en: "Stone", hu: "Kő", faces: Faces::All(tex::STONE), mine: pick(1.5, 0),
        drops: Drops::Item(COBBLE, 1), creative: Creative::Blocks(1), ..CUBE
    };
    COBBLE = BlockDef {
        key: "cobblestone", en: "Cobblestone", hu: "Zúzottkő", faces: Faces::All(tex::COBBLE),
        mine: pick(2.0, 0), creative: Creative::Blocks(1), ..CUBE
    };
    STONE_BRICKS = BlockDef {
        key: "stone_bricks", en: "Stone Bricks", hu: "Kőtégla",
        faces: Faces::All(tex::STONE_BRICKS), mine: pick(1.5, 0), creative: Creative::Blocks(1),
        ..CUBE
    };
    SANDSTONE = BlockDef {
        key: "sandstone", en: "Sandstone", hu: "Homokkő",
        faces: Faces::Column { side: tex::SANDSTONE, end: tex::SANDSTONE_TOP },
        mine: pick(0.8, 0), creative: Creative::Blocks(1), ..CUBE
    };
    BRICKS = BlockDef {
        key: "bricks", en: "Bricks", hu: "Téglák", faces: Faces::All(tex::BRICKS),
        mine: pick(2.0, 0), creative: Creative::Blocks(1), ..CUBE
    };
    OBSIDIAN = BlockDef {
        key: "obsidian", en: "Obsidian", hu: "Obszidián", faces: Faces::All(tex::OBSIDIAN),
        mine: pick(50.0, 4), creative: Creative::Blocks(1), ..CUBE
    };
    BEDROCK = BlockDef {
        key: "bedrock", en: "Bedrock", hu: "Alapkő", faces: Faces::All(tex::BEDROCK), mine: None,
        creative: Creative::Blocks(1), ..CUBE
    };

    // Wood and building blocks. Logs stand upright; `_X` and `_Z` lie along those axes.
    OAK_LOG = BlockDef {
        key: "oak_log", en: "Oak Log", hu: "Tölgyfarönk",
        faces: Faces::Column { side: tex::OAK_LOG, end: tex::OAK_LOG_TOP },
        creative: Creative::Blocks(2), ..LOG
    };
    BIRCH_LOG = BlockDef {
        key: "birch_log", en: "Birch Log", hu: "Nyírfarönk",
        faces: Faces::Column { side: tex::BIRCH_LOG, end: tex::BIRCH_LOG_TOP },
        creative: Creative::Blocks(2), ..LOG
    };
    SPRUCE_LOG = BlockDef {
        key: "spruce_log", en: "Spruce Log", hu: "Lucfenyőrönk",
        faces: Faces::Column { side: tex::SPRUCE_LOG, end: tex::SPRUCE_LOG_TOP },
        creative: Creative::Blocks(2), ..LOG
    };
    OAK_LOG_X = BlockDef { key: "oak_log_x", item: BlockItem::As(OAK_LOG), ..LOG };
    OAK_LOG_Z = BlockDef { key: "oak_log_z", item: BlockItem::As(OAK_LOG), ..LOG };
    SPRUCE_LOG_X = BlockDef { key: "spruce_log_x", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    SPRUCE_LOG_Z = BlockDef { key: "spruce_log_z", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    BIRCH_LOG_X = BlockDef { key: "birch_log_x", item: BlockItem::As(BIRCH_LOG), ..LOG };
    BIRCH_LOG_Z = BlockDef { key: "birch_log_z", item: BlockItem::As(BIRCH_LOG), ..LOG };
    /// Branches: thin round logs growing out of the trees' trunks, upright or lying along X
    /// or Z (spruce branches only lie). Mined, they give back their tree's log.
    OAK_BRANCH = BlockDef { key: "oak_branch", item: BlockItem::As(OAK_LOG), ..LOG };
    OAK_BRANCH_X = BlockDef { key: "oak_branch_x", item: BlockItem::As(OAK_LOG), ..LOG };
    OAK_BRANCH_Z = BlockDef { key: "oak_branch_z", item: BlockItem::As(OAK_LOG), ..LOG };
    BIRCH_BRANCH = BlockDef { key: "birch_branch", item: BlockItem::As(BIRCH_LOG), ..LOG };
    BIRCH_BRANCH_X = BlockDef { key: "birch_branch_x", item: BlockItem::As(BIRCH_LOG), ..LOG };
    BIRCH_BRANCH_Z = BlockDef { key: "birch_branch_z", item: BlockItem::As(BIRCH_LOG), ..LOG };
    SPRUCE_BRANCH_X = BlockDef { key: "spruce_branch_x", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    SPRUCE_BRANCH_Z = BlockDef { key: "spruce_branch_z", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    PLANKS = BlockDef {
        key: "oak_planks", en: "Oak Planks", hu: "Tölgyfa deszka", faces: Faces::All(tex::PLANKS),
        mine: axe(2.0), creative: Creative::Blocks(2), ..CUBE
    };
    /// Oak stairs: + facing (bits 0-1, toward the tall back) + upside down (bit 2).
    OAK_STAIRS * 8 = BlockDef {
        key: "oak_stairs", en: "Oak Stairs", hu: "Tölgyfa lépcső", model: Model::Stairs,
        opaque: false, faces: Faces::All(tex::PLANKS), mine: axe(2.0), place: Place::Stairs,
        creative: Creative::Blocks(2), ..CUBE
    };
    GLASS = BlockDef {
        key: "glass", en: "Glass", hu: "Üveg", faces: Faces::All(tex::GLASS),
        mine: mine(0.3, None, None), drops: Drops::Nothing, cull_same: true, solid: true,
        creative: Creative::Blocks(2), ..THIN
    };
    GLOWSTONE = BlockDef {
        key: "glowstone", en: "Glowstone", hu: "Izzókő", faces: Faces::All(tex::GLOWSTONE),
        light: 15, mine: mine(0.3, None, None), creative: Creative::Blocks(2), ..CUBE
    };
    WOOL = BlockDef {
        key: "white_wool", en: "White Wool", hu: "Fehér gyapjú", faces: Faces::All(tex::WOOL),
        mine: mine(0.8, None, None), creative: Creative::Blocks(2), ..CUBE
    };

    // Plants.
    OAK_LEAVES = BlockDef {
        key: "oak_leaves", en: "Oak Leaves", hu: "Tölgylevelek", faces: Faces::All(tex::OAK_LEAVES),
        tint: TintKind::Foliage, ..LEAVES
    };
    BIRCH_LEAVES = BlockDef {
        key: "birch_leaves", en: "Birch Leaves", hu: "Nyírfalevelek",
        faces: Faces::All(tex::BIRCH_LEAVES), tint: TintKind::Birch, ..LEAVES
    };
    SPRUCE_LEAVES = BlockDef {
        key: "spruce_leaves", en: "Spruce Leaves", hu: "Lucfenyőlevelek",
        faces: Faces::All(tex::SPRUCE_LEAVES), tint: TintKind::Spruce, ..LEAVES
    };
    OAK_SAPLING = BlockDef {
        key: "oak_sapling", en: "Oak Sapling", hu: "Tölgycsemete",
        faces: Faces::All(tex::OAK_SAPLING), icon: Some(tex::OAK_SAPLING),
        creative: Creative::Blocks(3), ..PLANT
    };
    BIRCH_SAPLING = BlockDef {
        key: "birch_sapling", en: "Birch Sapling", hu: "Nyírfacsemete",
        faces: Faces::All(tex::BIRCH_SAPLING), icon: Some(tex::BIRCH_SAPLING),
        creative: Creative::Blocks(3), ..PLANT
    };
    SPRUCE_SAPLING = BlockDef {
        key: "spruce_sapling", en: "Spruce Sapling", hu: "Lucfenyőcsemete",
        faces: Faces::All(tex::SPRUCE_SAPLING), icon: Some(tex::SPRUCE_SAPLING),
        creative: Creative::Blocks(3), ..PLANT
    };
    TALL_GRASS = BlockDef {
        key: "grass", en: "Grass", hu: "Fű", faces: Faces::All(tex::TALL_GRASS),
        tint: TintKind::Grass, replaceable: true, drops: Drops::Nothing, shears: true,
        icon: Some(tex::TALL_GRASS), creative: Creative::Blocks(3), ..PLANT
    };
    POPPY = BlockDef {
        key: "poppy", en: "Poppy", hu: "Pipacs", faces: Faces::All(tex::POPPY),
        icon: Some(tex::POPPY), creative: Creative::Blocks(3), ..PLANT
    };
    DANDELION = BlockDef {
        key: "dandelion", en: "Dandelion", hu: "Pitypang", faces: Faces::All(tex::DANDELION),
        icon: Some(tex::DANDELION), creative: Creative::Blocks(3), ..PLANT
    };
    DEAD_BUSH = BlockDef {
        key: "dead_bush", en: "Dead Bush", hu: "Elszáradt bokor", faces: Faces::All(tex::DEAD_BUSH),
        drops: Drops::Custom(dead_bush_drops), shears: true, icon: Some(tex::DEAD_BUSH),
        creative: Creative::Blocks(3), ..PLANT
    };
    CACTUS = BlockDef {
        key: "cactus", en: "Cactus", hu: "Kaktusz",
        faces: Faces::Column { side: tex::CACTUS, end: tex::CACTUS_TOP },
        needs_support: true, mine: mine(0.4, None, None), creative: Creative::Blocks(3), ..CUBE
    };

    // Ores and what they give.
    COAL_ORE = BlockDef {
        key: "coal_ore", en: "Coal Ore", hu: "Szénérc", faces: Faces::All(tex::COAL_ORE),
        mine: pick(3.0, 0), drops: Drops::Item(COAL, 1), creative: Creative::Blocks(4), ..CUBE
    };
    COPPER_ORE = BlockDef {
        key: "copper_ore", en: "Copper Ore", hu: "Rézérc", faces: Faces::All(tex::COPPER_ORE),
        mine: pick(3.0, 1), creative: Creative::Blocks(4), ..CUBE
    };
    IRON_ORE = BlockDef {
        key: "iron_ore", en: "Iron Ore", hu: "Vasérc", faces: Faces::All(tex::IRON_ORE),
        mine: pick(3.0, 2), creative: Creative::Blocks(4), ..CUBE
    };
    GOLD_ORE = BlockDef {
        key: "gold_ore", en: "Gold Ore", hu: "Aranyérc", faces: Faces::All(tex::GOLD_ORE),
        mine: pick(3.0, 3), creative: Creative::Blocks(4), ..CUBE
    };
    DIAMOND_ORE = BlockDef {
        key: "diamond_ore", en: "Diamond Ore", hu: "Gyémántérc",
        faces: Faces::All(tex::DIAMOND_ORE), mine: pick(3.0, 3), creative: Creative::Blocks(4),
        ..CUBE
    };
    COAL_BLOCK = BlockDef {
        key: "coal_block", en: "Block of Coal", hu: "Szénblokk", faces: Faces::All(tex::COAL_BLOCK),
        mine: pick(5.0, 0), creative: Creative::Blocks(5), ..CUBE
    };
    COPPER_BLOCK = BlockDef {
        key: "copper_block", en: "Block of Copper", hu: "Rézblokk",
        faces: Faces::All(tex::COPPER_BLOCK), mine: pick(5.0, 1), creative: Creative::Blocks(5),
        ..CUBE
    };
    IRON_BLOCK = BlockDef {
        key: "iron_block", en: "Block of Iron", hu: "Vasblokk", faces: Faces::All(tex::IRON_BLOCK),
        mine: pick(5.0, 2), creative: Creative::Blocks(5), ..CUBE
    };
    GOLD_BLOCK = BlockDef {
        key: "gold_block", en: "Block of Gold", hu: "Aranyblokk", faces: Faces::All(tex::GOLD_BLOCK),
        mine: pick(3.0, 3), creative: Creative::Blocks(5), ..CUBE
    };
    DIAMOND_BLOCK = BlockDef {
        key: "diamond_block", en: "Block of Diamond", hu: "Gyémántblokk",
        faces: Faces::All(tex::DIAMOND_BLOCK), mine: pick(5.0, 3), creative: Creative::Blocks(5),
        ..CUBE
    };

    // Fluids: + level. Level 0 is a source, 1..7 flowing, 8 falling (`FALLING`).
    WATER * 9 = BlockDef {
        key: "water", en: "Water", hu: "Víz", model: Model::Fluid, faces: Faces::All(tex::WATER),
        stops_sky: true, replaceable: true, mine: None, drops: Drops::Nothing,
        item: BlockItem::Other(WATER_BUCKET), ..THIN
    };
    LAVA * 9 = BlockDef {
        key: "lava", en: "Lava", hu: "Láva", model: Model::Fluid, faces: Faces::All(tex::LAVA),
        stops_sky: true, light: 15, replaceable: true, mine: None, drops: Drops::Nothing,
        item: BlockItem::Other(LAVA_BUCKET), ..THIN
    };

    // Workstations. Furnaces and chests: + facing (0 north/-Z, 1 east/+X, 2 south/+Z,
    // 3 west/-X). A burning furnace's ids come right after the furnace's (`furnace_id`).
    CRAFTING_TABLE = BlockDef {
        key: "crafting_table", en: "Crafting Table", hu: "Barkácsasztal",
        faces: Faces::Custom(crafting_table_faces), mine: axe(2.5),
        creative: Creative::Functional(0), ..CUBE
    };
    FURNACE * 4 = BlockDef {
        key: "furnace", en: "Furnace", hu: "Kemence", ..FURNACE_DEF
    };
    FURNACE_LIT * 4 = BlockDef {
        key: "lit_furnace", light: 13, item: BlockItem::As(FURNACE), creative: Creative::None,
        ..FURNACE_DEF
    };
    /// Blast furnace (smelts iron too), and the chimney standing on it (+ facing).
    BLAST_FURNACE * 4 = BlockDef {
        key: "blast_furnace", en: "Blast Furnace", hu: "Kohó", place: Place::BigFurnace,
        ..FURNACE_DEF
    };
    BLAST_FURNACE_LIT * 4 = BlockDef {
        key: "lit_blast_furnace", light: 13, item: BlockItem::As(BLAST_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    CHIMNEY * 4 = BlockDef {
        key: "chimney", model: Model::Chimney, opaque: false, item: BlockItem::As(BLAST_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    /// Advanced furnace (smelts gold and diamond too), two wide and two tall: the furnace
    /// itself (lower left, seen from the front) + facing, lit + facing, and its other parts,
    /// `ADV_PART + (part - 1) * 4 + facing` (part 1 lower right, 2 upper left, 3 upper right),
    /// glowing ones from `ADV_PART_LIT`.
    ADV_FURNACE * 4 = BlockDef {
        key: "advanced_furnace", en: "Advanced Furnace", hu: "Fejlett kohó",
        place: Place::BigFurnace, ..FURNACE_DEF
    };
    ADV_FURNACE_LIT * 4 = BlockDef {
        key: "lit_advanced_furnace", light: 13, item: BlockItem::As(ADV_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    ADV_PART * 12 = BlockDef {
        key: "advanced_furnace_part", model: Model::Cube, item: BlockItem::As(ADV_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    ADV_PART_LIT * 12 = BlockDef {
        key: "lit_advanced_furnace_part", model: Model::Cube, item: BlockItem::As(ADV_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    /// Metal workbench for assembling and cleaning guns: the item. Placed, it is a
    /// `GUN_BENCH` (this block itself is the old one-block station).
    GUN_STATION = BlockDef {
        key: "gun_station", en: "Gun Station", hu: "Fegyverasztal", faces: GUN_STATION_FACES,
        mine: pick(3.5, 0), place: Place::GunBench, creative: Creative::Functional(0), ..CUBE
    };
    /// The gun station, two blocks wide: + facing (its front, where its drawer slides out,
    /// toward the player who placed it) + right half (bit 2; the left half, seen from the
    /// front, holds what lies on it). The cells in front of it are kept free for the drawer.
    GUN_BENCH * 8 = BlockDef {
        key: "gun_bench", model: Model::GunBench, opaque: false, faces: GUN_STATION_FACES,
        mine: pick(3.5, 0), item: BlockItem::As(GUN_STATION), ..CUBE
    };
    /// The rifle station, three blocks wide: its left block (seen from the front) + facing,
    /// which holds what lies on it (and is the item), and its other two blocks (without a
    /// facing: `bench_main` finds the left block they belong to).
    RIFLE_BENCH * 4 = BlockDef {
        key: "rifle_station", en: "Rifle Station", hu: "Puskaasztal", model: Model::GunBench,
        opaque: false, faces: GUN_STATION_FACES, mine: pick(3.5, 0), place: Place::RifleBench,
        creative: Creative::Functional(0), ..CUBE
    };
    RIFLE_BENCH_PART = BlockDef {
        key: "rifle_station_part", model: Model::GunBench, opaque: false,
        faces: GUN_STATION_FACES, mine: pick(3.5, 0), item: BlockItem::As(RIFLE_BENCH), ..CUBE
    };

    // Furniture and lights.
    CHEST * 4 = BlockDef {
        key: "chest", en: "Chest", hu: "Láda", place: Place::Chest,
        creative: Creative::Functional(1), ..CHEST_DEF
    };
    /// Double chest halves: + facing. The other half is on the chest's local +X side (the
    /// viewer's right, seen from the front) for `CHEST_LEFT`, local -X for `CHEST_RIGHT`.
    CHEST_LEFT * 4 = BlockDef { key: "chest_left", item: BlockItem::As(CHEST), ..CHEST_DEF };
    CHEST_RIGHT * 4 = BlockDef { key: "chest_right", item: BlockItem::As(CHEST), ..CHEST_DEF };
    /// Red bed halves: + facing (bits 0-1, from the foot toward the head: the way the player
    /// looked when placing it) + head half (bit 2).
    BED * 8 = BlockDef {
        key: "red_bed", en: "Red Bed", hu: "Piros ágy", model: Model::Bed, opaque: false,
        faces: Faces::Custom(bed_faces), mine: mine(0.2, None, None),
        item: BlockItem::Own { stack: 1 }, icon: Some(tex::BED_ITEM), place: Place::Bed,
        creative: Creative::Functional(1), ..CUBE
    };
    /// Oak door halves: + facing (bits 0-1, the way the player looked when placing it)
    /// + open (bit 2) + upper half (bit 3) + hinge on the right (bit 4) + swings out (bit 5:
    /// toward the side it closes on, into the next block, instead of into its own block).
    OAK_DOOR * 64 = BlockDef {
        key: "oak_door", en: "Oak Door", hu: "Tölgyfa ajtó", model: Model::Door, solid: true,
        faces: Faces::Custom(door_faces), needs_support: true, mine: axe(3.0),
        icon: Some(tex::DOOR_ITEM), place: Place::Door, creative: Creative::Functional(1),
        ..THIN
    };
    TORCH = BlockDef {
        key: "torch", en: "Torch", hu: "Fáklya", icon: Some(tex::TORCH), place: Place::Torch,
        creative: Creative::Functional(1), ..TORCH_DEF
    };
    /// Wall torches: + the side of the block they hang on (north, east, south, west).
    WALL_TORCH * 4 = BlockDef { key: "wall_torch", item: BlockItem::As(TORCH), ..TORCH_DEF };
    /// A lantern standing on a block, and one hanging from the block above.
    LANTERN = BlockDef {
        key: "lantern", en: "Lantern", hu: "Lámpás", icon: Some(tex::LANTERN_ITEM),
        place: Place::Lantern, creative: Creative::Functional(1), ..LANTERN_DEF
    };
    LANTERN_HANGING = BlockDef {
        key: "hanging_lantern", item: BlockItem::As(LANTERN), ..LANTERN_DEF
    };
}

const FURNACE_DEF: BlockDef = BlockDef {
    model: Model::Furnace,
    faces: Faces::Custom(furnace_faces),
    mine: pick(3.5, 0),
    place: Place::Facing,
    creative: Creative::Functional(0),
    ..CUBE
};

const CHEST_DEF: BlockDef = BlockDef {
    model: Model::Chest,
    opaque: false,
    faces: Faces::Custom(chest_faces),
    mine: axe(2.5),
    ..CUBE
};

const TORCH_DEF: BlockDef = BlockDef {
    model: Model::Torch,
    faces: Faces::All(tex::TORCH),
    light: 14,
    ..PLANT
};

const LANTERN_DEF: BlockDef = BlockDef {
    model: Model::Lantern,
    faces: Faces::All(tex::LANTERN),
    light: 15,
    needs_support: true,
    mine: pick(3.5, 0),
    ..THIN
};

const GUN_STATION_FACES: Faces = Faces::Sides {
    top: tex::GUN_STATION_TOP,
    side: tex::GUN_STATION_SIDE,
    bottom: tex::GUN_STATION_BOTTOM,
};

/// Fluid level of a falling fluid.
pub const FALLING: u8 = 8;
/// The stages of a stump mark.
pub const STUMP_STAGES: Block = 3;

// The ids that are worked out from others (`furnace_id`, `adv_part`, `stump_mark`).
const _: () = {
    assert!(FURNACE_LIT == FURNACE + 4);
    assert!(BLAST_FURNACE_LIT == BLAST_FURNACE + 4);
    assert!(ADV_FURNACE_LIT == ADV_FURNACE + 4);
    assert!(ADV_PART_LIT == ADV_PART + 12);
    assert!(STUMP_MARK_SNOWY == STUMP_MARK + STUMP_STAGES);
    assert!(AIR == 0);
    // Block items share the block's id; the other items come after them.
    assert!(BLOCK_IDS <= crate::item::FIRST_ITEM as usize);
};

/// The line of the table for each id.
static TYPE_OF: [u8; BLOCK_IDS] = {
    let mut t = [0u8; BLOCK_IDS];
    let mut i = 0;
    while i < BLOCKS.len() {
        assert!(i < 256);
        let d = &BLOCKS[i];
        let mut s = 0;
        while s < d.states {
            t[(d.id + s) as usize] = i as u8;
            s += 1;
        }
        i += 1;
    }
    t
};

/// A block's line of the table (an unknown id: air's).
#[inline]
pub fn def(b: Block) -> &'static BlockDef {
    &BLOCKS[TYPE_OF.get(b as usize).copied().unwrap_or(0) as usize]
}

/// The first id of a block (its constant): the block without its state.
#[inline]
pub fn base(b: Block) -> Block {
    def(b).id
}

/// A known id (anything else, from a damaged file or a peer: air).
#[inline]
pub fn valid(b: Block) -> Block {
    if (b as usize) < BLOCK_IDS {
        b
    } else {
        AIR
    }
}

/// The block with this key (its first id).
pub fn by_key(key: &str) -> Option<Block> {
    BLOCKS.iter().find(|d| d.key == key).map(|d| d.id)
}

/// A block's name in the current language (the block it counts as, for a part of one).
pub fn block_name(b: Block) -> String {
    let d = match def(b).item {
        BlockItem::As(other) => def(other),
        _ => def(b),
    };
    (if crate::lang::is_hungarian() { d.hu } else { d.en }).to_string()
}

// Hot properties, asked for every block and its neighbours in the mesher's and the lighting's
// inner loops: bits in one table.
const OPAQUE: u8 = 1;
const SOLID: u8 = 2;
const STOPS_SKY: u8 = 4;
static FLAGS: [u8; BLOCK_IDS] = {
    let mut t = [0u8; BLOCK_IDS];
    let mut i = 0;
    while i < BLOCKS.len() {
        let d = &BLOCKS[i];
        let f = if d.opaque { OPAQUE } else { 0 }
            | if d.solid { SOLID } else { 0 }
            | if d.stops_sky { STOPS_SKY } else { 0 };
        let mut s = 0;
        while s < d.states {
            t[(d.id + s) as usize] = f;
            s += 1;
        }
        i += 1;
    }
    t
};

#[inline]
pub fn flag(b: Block, f: u8) -> bool {
    FLAGS.get(b as usize).is_some_and(|&x| x & f != 0)
}
/// Fully hides neighbouring faces and blocks light.
#[inline]
pub fn is_opaque(b: Block) -> bool {
    flag(b, OPAQUE)
}
/// Blocks player movement.
#[inline]
pub fn is_solid(b: Block) -> bool {
    flag(b, SOLID)
}
/// Stops full-strength sunlight (used for the heightmap).
#[inline]
pub fn attenuates_sky(b: Block) -> bool {
    flag(b, STOPS_SKY)
}

// The faces worked out from a block's state.

fn crafting_table_faces(_: Block, face: usize) -> u32 {
    match face {
        2 => tex::CRAFTING_TOP,
        3 => tex::PLANKS,
        0 | 1 => tex::CRAFTING_SIDE,
        _ => tex::CRAFTING_FRONT,
    }
}

fn furnace_faces(b: Block, face: usize) -> u32 {
    let ends = face == 2 || face == 3;
    let f = facing(b).unwrap_or(0);
    let front = face == front_face(f);
    match def(b).model {
        Model::Chimney => match face {
            2 => tex::CHIMNEY_TOP,
            3 => tex::BLAST_TOP,
            _ => tex::CHIMNEY_SIDE,
        },
        // The advanced furnace's other parts.
        Model::Cube => {
            let (part, lit) = adv_part(b).unwrap_or((1, false));
            if face == 2 && part >= 2 {
                tex::ADV_VENT_TOP
            } else if ends {
                tex::ADV_TOP
            } else if front {
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
        _ => {
            let (top, front_tex, side) = match furnace_base(b) {
                Some(BLAST_FURNACE) => (tex::BLAST_TOP, tex::BLAST_FRONT, tex::BLAST_SIDE),
                Some(ADV_FURNACE) => (tex::ADV_TOP, tex::ADV_FRONT, tex::ADV_SIDE),
                _ if is_lit_furnace(b) => (tex::FURNACE_TOP, tex::FURNACE_FRONT_LIT, tex::FURNACE_SIDE),
                _ => (tex::FURNACE_TOP, tex::FURNACE_FRONT, tex::FURNACE_SIDE),
            };
            if ends {
                top
            } else if front {
                front_tex
            } else {
                side
            }
        }
    }
}

fn chest_faces(b: Block, face: usize) -> u32 {
    if face == 2 || face == 3 {
        tex::CHEST_TOP
    } else if Some(face) == facing(b).map(front_face) {
        tex::CHEST_FRONT
    } else {
        tex::CHEST_SIDE
    }
}

/// The bed is meshed on its own; this is for particles.
fn bed_faces(b: Block, face: usize) -> u32 {
    if face == 3 {
        tex::BED_BOTTOM
    } else if bed_head(b) {
        tex::BED_HEAD_TOP
    } else {
        tex::BED_FOOT_TOP
    }
}

fn door_faces(b: Block, _: usize) -> u32 {
    if door_upper(b) {
        tex::DOOR_TOP
    } else {
        tex::DOOR_BOTTOM
    }
}

// What some blocks drop.

/// Now and then a sapling of the tree, or (oak) a stick.
fn leaf_drops(b: Block, _: ItemId, r: f32) -> Vec<Stack> {
    if r < 0.05 {
        let sapling = match b {
            BIRCH_LEAVES => BIRCH_SAPLING,
            SPRUCE_LEAVES => SPRUCE_SAPLING,
            _ => OAK_SAPLING,
        };
        vec![Stack::one(sapling as ItemId)]
    } else if b == OAK_LEAVES && r > 0.98 {
        vec![Stack::one(STICK)]
    } else {
        Vec::new()
    }
}

/// Up to two sticks.
fn dead_bush_drops(_: Block, _: ItemId, r: f32) -> Vec<Stack> {
    let n = (r * 3.0) as u8;
    if n > 0 {
        vec![Stack::new(STICK, n)]
    } else {
        Vec::new()
    }
}

/// Whether shears are what is held (sheared leaves and plants drop themselves).
pub fn sheared(b: Block, held: ItemId) -> bool {
    held == SHEARS && def(b).shears
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_consistent() {
        let mut keys = std::collections::HashSet::new();
        let mut next = 0;
        for d in BLOCKS {
            assert!(!d.key.is_empty() && keys.insert(d.key), "key {:?}", d.key);
            assert_eq!(d.id, next, "{}", d.key);
            next += d.states;
            if let BlockItem::As(other) = d.item {
                assert!(matches!(def(other).item, BlockItem::Own { .. }), "{} counts as {}", d.key, def(other).key);
            }
            if matches!(d.item, BlockItem::Own { .. }) {
                assert!(!d.en.is_empty() && !d.hu.is_empty(), "{} has no name", d.key);
            }
            assert_eq!(by_key(d.key), Some(d.id));
        }
        assert_eq!(next as usize, BLOCK_IDS);
        for b in 0..BLOCK_IDS as Block {
            assert!(b >= base(b) && b < base(b) + def(b).states);
        }
    }
}
