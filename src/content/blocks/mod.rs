//! Every block: a line each in the files of this folder, by kind (earth, stone, building
//! blocks, plants, ores, fluids, machines, furniture). A line gives a block its constant, its
//! key (commands, save files), its names, how it looks (its model, faces, tint and light), what
//! it does in the world (solid, sunlight, support, gravity, fluids), how it is mined and what
//! it drops, the item it is (and how its item burns or smelts in a furnace), where it is in
//! the creative inventory, and how it is placed.
//!
//! Ids are not written anywhere: they follow from the order of the lines (and of the files,
//! see `PARTS`). A block with a state (a facing, a door's halves...) takes `states` ids after
//! its constant, the state being `b - BASE`. Save files keep the blocks' keys (`save`), so the
//! order can change.
//!
//! Adding a plain block is adding a line to its file (and its texture: `tex`, the painter in
//! `textures::procedural` and a resource pack name in `textures::pack`).

use crate::item::{
    smelts, ItemId, Smelt, Stack, ToolKind, CHARCOAL, CLAY_BALL, COAL, COPPER_INGOT, DIAMOND, GOLD_INGOT, IRON_INGOT,
    LAVA_BUCKET, SHEARS, STICK, WATER_BUCKET,
};
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

pub use crate::content::Creative;

/// How the block's item is placed (`client::player::items`).
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
    /// Seconds its item burns in a furnace.
    pub fuel: Option<f32>,
    /// What its item smelts into (`item::smelt`).
    pub smelt: Option<Smelt>,
    pub creative: Creative,
    pub place: Place,
}

/// A solid, opaque full cube, mined by hand in 1.5 s, dropping itself: most blocks.
pub(super) const CUBE: BlockDef = BlockDef {
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
    fuel: None,
    smelt: None,
    creative: Creative::None,
    place: Place::Plain,
};

/// Something small in a block: not solid, lets light and sunlight through.
pub(super) const THIN: BlockDef = BlockDef { opaque: false, solid: false, stops_sky: false, ..CUBE };

/// Walk-through plants on the ground, broken at once and washed away.
pub(super) const PLANT: BlockDef = BlockDef {
    model: Model::Plant,
    needs_support: true,
    washes_away: true,
    mine: mine(0.0, None, None),
    ..THIN
};



pub(super) const fn mine(hardness: f32, tool: Option<ToolKind>, needs: Option<u8>) -> Option<Mine> {
    Some(Mine { hardness, tool, needs })
}
/// Mined with a pickaxe of at least harvest level `needs`.
pub(super) const fn pick(hardness: f32, needs: u8) -> Option<Mine> {
    mine(hardness, Some(ToolKind::Pickaxe), Some(needs))
}
pub(super) const fn shovel(hardness: f32) -> Option<Mine> {
    mine(hardness, Some(ToolKind::Shovel), None)
}
pub(super) const fn axe(hardness: f32) -> Option<Mine> {
    mine(hardness, Some(ToolKind::Axe), None)
}

/// Declares a file's blocks: `after PREVIOUS_END; NAME [* states] = BlockDef { .. }; ...`.
/// Each gets the constant `NAME` (the id after the previous block's ids, the first after the
/// last of the file before), and its line in the file's `DEFS`; `END` is the id after its last.
macro_rules! blocks {
    (after $prev:expr; $( $(#[$m:meta])* $name:ident $(* $n:literal)? = $def:expr; )*) => {
        blocks!(@ids $prev; $( $(#[$m])* $name $(* $n)?; )*);
        pub(super) const DEFS: &[BlockDef] = &[$( BlockDef { id: $name, states: blocks!(@n $($n)?), ..$def } ),*];
    };
    (@ids $at:expr; ) => {
        pub(super) const END: Block = $at;
    };
    (@ids $at:expr; $(#[$m:meta])* $name:ident $(* $n:literal)?; $($rest:tt)*) => {
        $(#[$m])*
        pub const $name: Block = $at;
        blocks!(@ids $name + blocks!(@n $($n)?); $($rest)*);
    };
    (@n) => { 1 };
    (@n $n:literal) => { $n };
}

mod earth;
mod stone;
mod building;
mod plants;
mod ores;
mod fluids;
mod machines;
mod furniture;

pub use building::*;
pub use earth::*;
pub use fluids::*;
pub use furniture::*;
pub use machines::*;
pub use ores::*;
pub use plants::*;
pub use stone::*;

/// The files' blocks, in the order of their ids.
const PARTS: [&[BlockDef]; 8] =
    [earth::DEFS, stone::DEFS, building::DEFS, plants::DEFS, ores::DEFS, fluids::DEFS, machines::DEFS, furniture::DEFS];
const BLOCK_TYPES: usize = {
    let (mut n, mut i) = (0, 0);
    while i < PARTS.len() {
        n += PARTS[i].len();
        i += 1;
    }
    n
};
const ALL: [BlockDef; BLOCK_TYPES] = {
    let mut out = [CUBE; BLOCK_TYPES];
    let (mut n, mut i) = (0, 0);
    while i < PARTS.len() {
        let mut j = 0;
        while j < PARTS[i].len() {
            out[n] = PARTS[i][j];
            n += 1;
            j += 1;
        }
        i += 1;
    }
    out
};
/// Every block, in the order of their ids.
pub const BLOCKS: &[BlockDef] = &ALL;
/// How many block ids there are.
pub const BLOCK_IDS: usize = furniture::END as usize;







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
    (if crate::app::lang::is_hungarian() { d.hu } else { d.en }).to_string()
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






// What some blocks drop.



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
