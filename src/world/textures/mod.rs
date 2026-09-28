//! Block, item and entity textures: one 128x128 layer each in a texture array. Every layer
//! is drawn procedurally first (`procedural`), then replaced by the resource packs' texture
//! where they have one (`pack`); the mipmaps and the item sprite masks are made from the result.

mod pack;
mod procedural;

use crate::pack::{Image, Packs};
use pack::apply_pack;
use procedural::{crack_pattern, pixel};
pub const TILE: usize = 128;
/// Alpha of opaque texels that are not biome tinted (255 = fully tinted).
const UNTINTED: u8 = 153;

pub mod tex {
    pub const GRASS_TOP: u32 = 0;
    pub const GRASS_SIDE: u32 = 1;
    pub const DIRT: u32 = 2;
    pub const STONE: u32 = 3;
    pub const SAND: u32 = 4;
    pub const OAK_LOG: u32 = 5;
    pub const OAK_LOG_TOP: u32 = 6;
    pub const OAK_LEAVES: u32 = 7;
    pub const WATER: u32 = 8;
    pub const SNOW: u32 = 9;
    pub const PLANKS: u32 = 10;
    pub const COBBLE: u32 = 11;
    pub const BEDROCK: u32 = 12;
    pub const GLASS: u32 = 13;
    pub const BRICKS: u32 = 14;
    pub const GRAVEL: u32 = 15;
    pub const SNOWY_GRASS_SIDE: u32 = 16;
    pub const SANDSTONE: u32 = 17;
    pub const SANDSTONE_TOP: u32 = 18;
    pub const SPRUCE_LOG: u32 = 19;
    pub const SPRUCE_LOG_TOP: u32 = 20;
    pub const SPRUCE_LEAVES: u32 = 21;
    pub const BIRCH_LOG: u32 = 22;
    pub const BIRCH_LOG_TOP: u32 = 23;
    pub const BIRCH_LEAVES: u32 = 24;
    pub const CACTUS: u32 = 25;
    pub const CACTUS_TOP: u32 = 26;
    pub const TALL_GRASS: u32 = 27;
    pub const POPPY: u32 = 28;
    pub const DANDELION: u32 = 29;
    pub const DEAD_BUSH: u32 = 30;
    pub const COAL_ORE: u32 = 31;
    pub const IRON_ORE: u32 = 32;
    pub const GOLD_ORE: u32 = 33;
    pub const DIAMOND_ORE: u32 = 34;
    pub const OBSIDIAN: u32 = 35;
    pub const ICE: u32 = 36;
    pub const CLAY: u32 = 37;
    pub const LAVA: u32 = 38;
    pub const GLOWSTONE: u32 = 39;
    pub const SKIN: u32 = 40;
    pub const SLEEVE: u32 = 41;
    pub const CRACK: u32 = 42; // 10 stages (42..=51)
    pub const FACE: u32 = 52;
    pub const HEAD_SIDE: u32 = 53;
    pub const HAIR: u32 = 54;
    pub const SHIRT_FRONT: u32 = 55;
    pub const SHIRT: u32 = 56;
    pub const ARM: u32 = 57;
    pub const LEG: u32 = 58;
    pub const CRAFTING_TOP: u32 = 59;
    pub const CRAFTING_SIDE: u32 = 60;
    pub const CRAFTING_FRONT: u32 = 61;
    pub const FURNACE_FRONT: u32 = 62;
    pub const FURNACE_FRONT_LIT: u32 = 63;
    pub const FURNACE_SIDE: u32 = 64;
    pub const FURNACE_TOP: u32 = 65;
    pub const CHEST_FRONT: u32 = 66;
    pub const CHEST_SIDE: u32 = 67;
    pub const CHEST_TOP: u32 = 68;
    pub const TORCH: u32 = 69;
    pub const OAK_SAPLING: u32 = 70;
    pub const BIRCH_SAPLING: u32 = 71;
    pub const SPRUCE_SAPLING: u32 = 72;
    pub const IRON_BLOCK: u32 = 73;
    pub const GOLD_BLOCK: u32 = 74;
    pub const DIAMOND_BLOCK: u32 = 75;
    pub const COAL_BLOCK: u32 = 76;
    pub const STONE_BRICKS: u32 = 77;
    // Item icons
    pub const STICK: u32 = 78;
    pub const COAL: u32 = 79;
    pub const CHARCOAL: u32 = 80;
    pub const IRON_INGOT: u32 = 81;
    pub const GOLD_INGOT: u32 = 82;
    pub const DIAMOND: u32 = 83;
    pub const CLAY_BALL: u32 = 84;
    pub const BRICK: u32 = 85;
    pub const BUCKET: u32 = 86;
    pub const WATER_BUCKET: u32 = 87;
    pub const LAVA_BUCKET: u32 = 88;
    /// 20 tools: TOOLS + tier * 4 + kind (pickaxe, axe, shovel, sword).
    pub const TOOLS: u32 = 89;
    pub const CHEST_INSIDE: u32 = 109;
    pub const CHEST_LATCH: u32 = 110;
    pub const TORCH_WOOD: u32 = 111;
    pub const TORCH_CAP: u32 = 112;
    pub const TORCH_FLAME: u32 = 113;
    pub const TORCH_CHAR: u32 = 114;
    pub const HEAD_BACK: u32 = 115;
    pub const SHIRT_BACK: u32 = 116;
    /// Lit furnace animation frames (resource packs); must match world.frag.
    pub const FURNACE_ANIM: u32 = 117;
    pub const FURNACE_FRAMES: u32 = 12;
    /// Torch flame and smoke particle sprites (smoke: small to large).
    pub const FLAME_PARTICLE: u32 = FURNACE_ANIM + FURNACE_FRAMES;
    pub const SMOKE: u32 = FLAME_PARTICLE + 1;
    /// Pig skin: a whole Minecraft entity atlas (64x64 texels) in one layer; the model picks
    /// its faces with atlas UVs.
    pub const PIG: u32 = SMOKE + super::SMOKE_FRAMES;
    pub const PIG_SPAWN_EGG: u32 = PIG + 1;
    pub const PORKCHOP: u32 = PIG + 2;
    pub const COOKED_PORKCHOP: u32 = PIG + 3;
    pub const GLASS_BOTTLE: u32 = PIG + 4;
    pub const WATER_BOTTLE: u32 = PIG + 5;
    pub const PURIFIED_WATER: u32 = PIG + 6;
    pub const LANTERN_ITEM: u32 = PIG + 7;
    pub const IRON_NUGGET: u32 = PIG + 8;
    /// Lantern block texture (Minecraft layout: body, cap, handle), and a chain.
    pub const LANTERN: u32 = PIG + 9;
    pub const CHAIN: u32 = PIG + 10;
    pub const SKIN_VARIANTS: u32 = CHAIN + 1;
    pub const SKIN_VARIANT_LAYERS: u32 = 5;
    pub const PRESET_SKINS: u8 = 4;
    pub const SKIN_COUNT: u8 = 5; // four outfits and one uploaded skin
    pub const CUSTOM_SKIN_START: u32 = SKIN_VARIANTS + SKIN_VARIANT_LAYERS * 3;
    pub const CUSTOM_SKIN_LAYERS: u32 = 11;
    pub const CUSTOM_SKIN_SLOTS: u8 = 16; // host and up to 15 LAN guests
    /// Double chest faces: CHEST_FRONT, CHEST_SIDE (the back), CHEST_TOP and CHEST_INSIDE
    /// without the frame on the edge where the two halves meet (see
    /// `crate::world::mesh::chest_open_layer`), 4 edges each (right, left, top, bottom of the
    /// texture).
    pub const CHEST_OPEN: u32 = CUSTOM_SKIN_START + CUSTOM_SKIN_LAYERS * CUSTOM_SKIN_SLOTS as u32;
    pub const DOOR_TOP: u32 = CHEST_OPEN + 16;
    pub const DOOR_BOTTOM: u32 = DOOR_TOP + 1;
    pub const DOOR_ITEM: u32 = DOOR_TOP + 2;
    /// Red bed faces as seen on a bed whose head points north (the pack's layout): the tops,
    /// the long sides (the legs cut out below), the head and foot ends, the bottom.
    pub const BED_HEAD_TOP: u32 = DOOR_ITEM + 1;
    pub const BED_FOOT_TOP: u32 = BED_HEAD_TOP + 1;
    pub const BED_HEAD_EAST: u32 = BED_HEAD_TOP + 2;
    pub const BED_HEAD_WEST: u32 = BED_HEAD_TOP + 3;
    pub const BED_FOOT_EAST: u32 = BED_HEAD_TOP + 4;
    pub const BED_FOOT_WEST: u32 = BED_HEAD_TOP + 5;
    pub const BED_HEAD_END: u32 = BED_HEAD_TOP + 6;
    pub const BED_FOOT_END: u32 = BED_HEAD_TOP + 7;
    pub const BED_BOTTOM: u32 = BED_HEAD_TOP + 8;
    pub const BED_ITEM: u32 = BED_HEAD_TOP + 9;
    /// Sheep skin and its wool coat: whole Minecraft entity atlases (64 texels wide) like
    /// `PIG`.
    pub const SHEEP: u32 = BED_ITEM + 1;
    pub const SHEEP_WOOL: u32 = SHEEP + 1;
    pub const WOOL: u32 = SHEEP + 2;
    pub const MUTTON: u32 = SHEEP + 3;
    pub const COOKED_MUTTON: u32 = SHEEP + 4;
    pub const SHEARS: u32 = SHEEP + 5;
    pub const SHEEP_SPAWN_EGG: u32 = SHEEP + 6;
    /// Water and lava animation frames (from the packs' animated strips; must match
    /// world.frag).
    pub const WATER_ANIM: u32 = SHEEP_SPAWN_EGG + 1;
    pub const LAVA_ANIM: u32 = WATER_ANIM + FLUID_FRAMES;
    pub const FLUID_FRAMES: u32 = 32;
    /// Grilled meat (made from the raw and cooked textures by `synth_grilled`).
    pub const HALF_COOKED_PORKCHOP: u32 = LAVA_ANIM + FLUID_FRAMES;
    pub const HALF_COOKED_MUTTON: u32 = HALF_COOKED_PORKCHOP + 1;
    pub const BURNT_PORKCHOP: u32 = HALF_COOKED_PORKCHOP + 2;
    pub const BURNT_MUTTON: u32 = HALF_COOKED_PORKCHOP + 3;
    /// A soft rounded highlight (drawn multiplied): the slot under the mouse, the spot of a
    /// furnace to put meat on.
    pub const SLOT_GLOW: u32 = BURNT_MUTTON + 1;
    /// The furnace front with its openings cut out, and the sooty stone inside them.
    pub const FURNACE_FRONT_CUT: u32 = SLOT_GLOW + 1;
    pub const FURNACE_INSIDE: u32 = SLOT_GLOW + 2;
    /// Gun station faces.
    pub const GUN_STATION_TOP: u32 = FURNACE_INSIDE + 1;
    pub const GUN_STATION_SIDE: u32 = GUN_STATION_TOP + 1;
    pub const GUN_STATION_BOTTOM: u32 = GUN_STATION_TOP + 2;
    /// Surfaces of the 3D pistol model: blued steel (slide), bare steel (barrel, spring),
    /// polymer (frame, grip, magazine).
    pub const GUN_BLUED: u32 = GUN_STATION_TOP + 3;
    pub const GUN_STEEL: u32 = GUN_STATION_TOP + 4;
    pub const GUN_POLYMER: u32 = GUN_STATION_TOP + 5;
    /// Item icons: the pistol, its five parts (frame, barrel, spring, slide, magazine) and
    /// the bullet.
    pub const PISTOL: u32 = GUN_STATION_TOP + 6;
    pub const PISTOL_PARTS: u32 = PISTOL + 1;
    pub const BULLET: u32 = PISTOL_PARTS + 5;
    /// Attachment icons: scope, silencer, extended magazine, laser sight.
    pub const GUN_ATTACHMENTS: u32 = BULLET + 1;
    /// The scope's glass (the model's lenses).
    pub const GUN_GLASS: u32 = GUN_ATTACHMENTS + 4;
    /// Meat burnt on one side (item icons, made in `synth_grilled`).
    pub const HALF_BURNT_PORKCHOP: u32 = GUN_GLASS + 1;
    pub const HALF_BURNT_MUTTON: u32 = GUN_GLASS + 2;
    /// Meat burnt on one side and raw on the other.
    pub const RAW_BURNT_PORKCHOP: u32 = GUN_GLASS + 3;
    pub const RAW_BURNT_MUTTON: u32 = GUN_GLASS + 4;
    /// Wood of the gun stocks.
    pub const GUN_WOOD: u32 = RAW_BURNT_MUTTON + 1;
    /// Copper: the ore, the storage block and the ingot.
    pub const COPPER_ORE: u32 = GUN_WOOD + 1;
    pub const COPPER_BLOCK: u32 = COPPER_ORE + 1;
    pub const COPPER_INGOT: u32 = COPPER_ORE + 2;
    /// The tools of the tiers added after the first five (copper): 4 each, like `TOOLS`
    /// (see `tool_layer`).
    pub const MORE_TOOLS: u32 = COPPER_INGOT + 1;
    /// Blast furnace: front (and with its openings cut out), sides, top; its chimney.
    pub const BLAST_FRONT: u32 = MORE_TOOLS + 4;
    pub const BLAST_FRONT_CUT: u32 = BLAST_FRONT + 1;
    pub const BLAST_SIDE: u32 = BLAST_FRONT + 2;
    pub const BLAST_TOP: u32 = BLAST_FRONT + 3;
    pub const CHIMNEY_SIDE: u32 = BLAST_FRONT + 4;
    pub const CHIMNEY_TOP: u32 = BLAST_FRONT + 5;
    /// Advanced furnace: the furnace's front (and cut), sides and top; the fronts of its
    /// other parts (the panel beside the furnace, the hood above it, glowing while it burns)
    /// and the vents on top.
    pub const ADV_FRONT: u32 = BLAST_FRONT + 6;
    pub const ADV_FRONT_CUT: u32 = ADV_FRONT + 1;
    pub const ADV_SIDE: u32 = ADV_FRONT + 2;
    pub const ADV_TOP: u32 = ADV_FRONT + 3;
    pub const ADV_PANEL: u32 = ADV_FRONT + 4;
    pub const ADV_HOOD_L: u32 = ADV_FRONT + 5;
    pub const ADV_HOOD_R: u32 = ADV_FRONT + 6;
    pub const ADV_HOOD_L_LIT: u32 = ADV_FRONT + 7;
    pub const ADV_HOOD_R_LIT: u32 = ADV_FRONT + 8;
    pub const ADV_VENT_TOP: u32 = ADV_FRONT + 9;
    /// Muzzle flash: the star seen from the front, and a flame tongue seen from the side.
    pub const MUZZLE_FLASH: u32 = ADV_VENT_TOP + 1;
    pub const MUZZLE_FLASH_SIDE: u32 = MUZZLE_FLASH + 1;
    /// A bullet hole, multiplied onto the block it is in.
    pub const BULLET_HOLE: u32 = MUZZLE_FLASH_SIDE + 1;
    /// Steel and ceramic from the blast and advanced furnaces, and the grenades.
    pub const STEEL_INGOT: u32 = BULLET_HOLE + 1;
    pub const CERAMIC_PLATE: u32 = STEEL_INGOT + 1;
    pub const FRAG_GRENADE: u32 = CERAMIC_PLATE + 1;
    pub const SMOKE_GRENADE: u32 = FRAG_GRENADE + 1;
    /// Armor: the icons (sixteen pieces, then the vest) and what the pieces look like worn.
    /// A box of rounds (an ammo can), as an item.
    pub const AMMO_BOX: u32 = SMOKE_GRENADE + 1;
    /// The weapon light, as an item (drawn from its model, `render_item_icons`).
    pub const FLASHLIGHT: u32 = AMMO_BOX + 1;
    /// The revolver, its five parts and the speedloader, as items (drawn from its model,
    /// `render_item_icons`).
    pub const REVOLVER: u32 = FLASHLIGHT + 1;
    pub const REVOLVER_PARTS: u32 = REVOLVER + 1;
    pub const SPEEDLOADER: u32 = REVOLVER_PARTS + 5;
    pub const MAGNUM_ROUND: u32 = SPEEDLOADER + 1;
    /// The AK-47, its five parts (the magazine last) and its rounds, as items (drawn from its
    /// model, `render_item_icons`).
    pub const AK47: u32 = MAGNUM_ROUND + 1;
    pub const AK_PARTS: u32 = AK47 + 1;
    pub const RIFLE_ROUND: u32 = AK_PARTS + 5;
    /// The rifle station's magazine loader, as an item (drawn from its model).
    pub const MAG_LOADER: u32 = RIFLE_ROUND + 1;
    /// The target dummy, as an item (drawn from its model, `render_item_icons`).
    pub const TARGET_DUMMY: u32 = MAG_LOADER + 1;
    pub const ARMOR_ICONS: u32 = TARGET_DUMMY + 1;
    pub const ARMOR_WOOL: u32 = ARMOR_ICONS + 17;
    pub const ARMOR_METAL: u32 = ARMOR_WOOL + 1;
    pub const VEST: u32 = ARMOR_METAL + 1;
    /// The guide book: its icon, and the leather, the page edges and a written page of the
    /// open book in a player's hands.
    pub const BOOK: u32 = VEST + 1;
    pub const BOOK_COVER: u32 = BOOK + 1;
    pub const BOOK_EDGE: u32 = BOOK + 2;
    pub const BOOK_PAGE: u32 = BOOK + 3;
    /// Pages of the guide book open in players' hands, drawn while the game runs
    /// (`game::book`): `BOOK_SHEET_COUNT` pages of `model::book::SHEET_LAYERS` layers each.
    pub const BOOK_SHEETS: u32 = BOOK_PAGE + 1;
    pub const BOOK_SHEET_COUNT: u32 = 12;
    /// The chapter tabs along the top of this player's guide book (`model::book::TAB_LAYERS`).
    pub const BOOK_TABS: u32 = BOOK_SHEETS + BOOK_SHEET_COUNT * 6;
    /// A round puff for big smoke clouds (smoke grenades, explosions): Minecraft's smoke
    /// sprites are small pixel blotches that turn into squares when drawn a block wide.
    pub const CLOUD: u32 = BOOK_TABS + 4;
    /// The pistol made in Blockbench: its texture pages (`model::pistol_vm`), clean, then
    /// the same with more and more grime on them (`PISTOL_DIRT_LEVELS` sets in all).
    pub const PISTOL_VIEW: u32 = CLOUD + 1;
    pub const PISTOL_DIRT_LEVELS: u32 = 4;
    /// The gun station block made in Blockbench: its texture pages (`model::gun_station`).
    pub const GUN_STATION_MODEL: u32 = PISTOL_VIEW + crate::model::pistol_vm::PAGES * PISTOL_DIRT_LEVELS;
    /// The grenades made in Blockbench: their texture pages (`model::grenade`).
    pub const GRENADE_MODEL: u32 = GUN_STATION_MODEL + crate::model::gun_station::PAGES;
    /// The revolver made in Blockbench: its texture pages (`model::revolver_vm`), clean and
    /// dirtier, like the pistol's (`PISTOL_DIRT_LEVELS` sets).
    pub const REVOLVER_VIEW: u32 = GRENADE_MODEL + crate::model::grenade::PAGES;
    /// The target dummy made in Blockbench: its texture pages (`model::dummy`).
    pub const DUMMY_MODEL: u32 = REVOLVER_VIEW + crate::model::revolver_vm::PAGES * PISTOL_DIRT_LEVELS;
    /// Item icons drawn while the game runs for things as they are (a magazine as full as it
    /// is, a gun with its attachments and dirt...: `game::icons`).
    /// The AK-47 made in Blockbench: its texture pages (`model::ak_vm`), clean and dirtier,
    /// like the pistol's (`PISTOL_DIRT_LEVELS` sets).
    pub const AK_VIEW: u32 = DUMMY_MODEL + crate::model::dummy::PAGES;
    /// The rifle station made in Blockbench: its texture pages (`model::gun_station`).
    pub const RIFLE_STATION_MODEL: u32 = AK_VIEW + crate::model::ak_vm::PAGES * PISTOL_DIRT_LEVELS;
    pub const STATE_ICONS: u32 = RIFLE_STATION_MODEL + crate::model::gun_station::RIFLE_PAGES;
    pub const STATE_ICON_COUNT: u32 = 64;
    pub const LAYERS: usize = (STATE_ICONS + STATE_ICON_COUNT) as usize;
}

/// Texture layer of a tool: `tier` and `kind` as `Tier as usize` and `ToolKind as usize`.
pub fn tool_layer(tier: usize, kind: usize) -> u32 {
    let i = (tier * 4 + kind) as u32;
    if i < 20 {
        tex::TOOLS + i
    } else {
        tex::MORE_TOOLS + i - 20
    }
}

/// Clothing layers shared by the world model, the hand and the menu preview.
pub fn skin_layer(layer: u32, skin: u8) -> u32 {
    if skin == 0 {
        return layer;
    }
    if skin >= tex::SKIN_COUNT - 1 {
        let slot = skin - (tex::SKIN_COUNT - 1);
        if slot >= tex::CUSTOM_SKIN_SLOTS {
            return layer;
        }
        let offset = match layer {
            tex::FACE => 0,
            tex::HEAD_SIDE => 1,
            tex::HAIR => 2,
            tex::HEAD_BACK => 3,
            tex::SKIN => 4,
            tex::SHIRT_FRONT => 5,
            tex::SHIRT => 6,
            tex::SHIRT_BACK => 7,
            tex::ARM => 8,
            tex::SLEEVE => 9,
            tex::LEG => 10,
            _ => return layer,
        };
        return tex::CUSTOM_SKIN_START + slot as u32 * tex::CUSTOM_SKIN_LAYERS + offset;
    }
    let offset = match layer {
        tex::SHIRT_FRONT => 0,
        tex::SHIRT => 1,
        tex::SHIRT_BACK => 2,
        tex::ARM => 3,
        tex::LEG => 4,
        _ => return layer,
    };
    tex::SKIN_VARIANTS + (skin as u32 - 1) * tex::SKIN_VARIANT_LAYERS + offset
}

const SKIN_PARTS: [(u32, [u32; 4]); 11] = [
    (tex::FACE, [8, 8, 8, 8]),
    (tex::HEAD_SIDE, [0, 8, 8, 8]),
    (tex::HAIR, [8, 0, 8, 8]),
    (tex::HEAD_BACK, [24, 8, 8, 8]),
    (tex::SKIN, [16, 0, 8, 8]),
    (tex::SHIRT_FRONT, [20, 20, 8, 12]),
    (tex::SHIRT, [16, 20, 4, 12]),
    (tex::SHIRT_BACK, [32, 20, 8, 12]),
    (tex::ARM, [44, 20, 4, 12]),
    (tex::SLEEVE, [44, 16, 4, 4]),
    (tex::LEG, [4, 20, 4, 12]),
];

/// A standard 64x64 or legacy 64x32 Minecraft skin, optionally at higher resolution.
pub fn decode_skin_png(data: &[u8]) -> Result<Image, &'static str> {
    if data.len() > 1_000_000 || data.len() < 24 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("A skin PNG fájl túl nagy vagy hibás.");
    }
    let w = u32::from_be_bytes(data[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(data[20..24].try_into().unwrap());
    if !(64..=1024).contains(&w) || w % 64 != 0 || (h != w && h * 2 != w) {
        return Err("64x64 vagy 64x32 arányú Minecraft skin PNG kell.");
    }
    crate::pack::decode_png(data).ok_or("Nem sikerült beolvasni a skin PNG-t.")
}

/// The Blockbench models' texture pages, made by `tools/blockbench/bbmodel_to_rust.py`:
/// 128x128 pages one under the other, from layer `first`.
fn synth_model_pages(base: &mut [u8], png: &[u8], pages: u32, first: u32) {
    let Some(img) = crate::pack::decode_png(png) else {
        return;
    };
    let layer_bytes = TILE * TILE * 4;
    for page in 0..pages as usize {
        let dst = (first as usize + page) * layer_bytes;
        for y in 0..TILE {
            let sy = page * TILE + y;
            if sy >= img.h as usize || img.w as usize != TILE {
                return;
            }
            let src = sy * TILE * 4;
            base[dst + y * TILE * 4..dst + (y + 1) * TILE * 4]
                .copy_from_slice(&img.rgba[src..src + TILE * 4]);
        }
    }
}

/// The guns' and grenades' item icons, drawn from their 3D models (the Blockbench pistol and
/// grenades) as they are held: turned a little to show them in 3D, lit from the upper left,
/// drawn at twice the size and scaled down for smooth edges.
fn render_item_icons(base: &mut [u8]) {
    use crate::item::*;
    let mut loaded = Stack::one(PISTOL);
    set_gun_rounds(&mut loaded, 12);
    let mut full = Stack::one(PISTOL_MAGAZINE);
    set_gun_rounds(&mut full, 12);
    let mut ext = Stack::one(EXTENDED_MAGAZINE);
    set_gun_rounds(&mut ext, 20);
    let mut revolver = Stack::one(REVOLVER);
    set_gun_rounds(&mut revolver, 6);
    let mut loader = Stack::one(SPEEDLOADER);
    set_gun_rounds(&mut loader, 6);
    let mut ak = Stack::one(AK47);
    set_gun_rounds(&mut ak, 30);
    let mut ak_mag = Stack::one(AK_MAGAZINE);
    set_gun_rounds(&mut ak_mag, 30);
    let icons = [
        (tex::PISTOL, loaded),
        (tex::PISTOL_PARTS, Stack::one(PISTOL_FRAME)),
        (tex::PISTOL_PARTS + 1, Stack::one(PISTOL_BARREL)),
        (tex::PISTOL_PARTS + 2, Stack::one(PISTOL_SPRING)),
        (tex::PISTOL_PARTS + 3, Stack::one(PISTOL_SLIDE)),
        (tex::PISTOL_PARTS + 4, full),
        (tex::BULLET, Stack::one(BULLET)),
        (tex::GUN_ATTACHMENTS, Stack::one(SCOPE)),
        (tex::GUN_ATTACHMENTS + 1, Stack::one(SILENCER)),
        (tex::GUN_ATTACHMENTS + 2, ext),
        (tex::GUN_ATTACHMENTS + 3, Stack::one(LASER_SIGHT)),
        (tex::FLASHLIGHT, Stack::one(FLASHLIGHT)),
        (tex::REVOLVER, revolver),
        (tex::REVOLVER_PARTS, Stack::one(REVOLVER_FRAME)),
        (tex::REVOLVER_PARTS + 1, Stack::one(REVOLVER_BARREL)),
        (tex::REVOLVER_PARTS + 2, Stack::one(REVOLVER_SPRING)),
        (tex::REVOLVER_PARTS + 3, Stack::one(REVOLVER_CYLINDER)),
        (tex::REVOLVER_PARTS + 4, Stack::one(REVOLVER_HAMMER)),
        (tex::SPEEDLOADER, loader),
        (tex::MAGNUM_ROUND, Stack::one(MAGNUM_ROUND)),
        (tex::FRAG_GRENADE, Stack::one(FRAG_GRENADE)),
        (tex::SMOKE_GRENADE, Stack::one(SMOKE_GRENADE)),
        (tex::TARGET_DUMMY, Stack::one(TARGET_DUMMY)),
        (tex::AK47, ak),
        (tex::AK_PARTS, Stack::one(AK_RECEIVER)),
        (tex::AK_PARTS + 1, Stack::one(AK_GAS_TUBE)),
        (tex::AK_PARTS + 2, Stack::one(AK_BOLT)),
        (tex::AK_PARTS + 3, Stack::one(AK_COVER)),
        (tex::AK_PARTS + 4, ak_mag),
        (tex::RIFLE_ROUND, Stack::one(RIFLE_ROUND)),
        (tex::MAG_LOADER, Stack::one(MAG_LOADER)),
    ];
    for (layer, st) in icons {
        let img = render_icon(base, &st);
        let dst = layer as usize * TILE * TILE * 4;
        base[dst..dst + TILE * TILE * 4].copy_from_slice(&img);
    }
}

/// An item's icon drawn from its 3D model as it is (its state: rounds, attachments, dirt), a
/// `TILE` square, from the texture layers `base` (the items' model pages in it).
pub fn render_icon(base: &[u8], st: &crate::item::Stack) -> Vec<u8> {
    let mut verts = Vec::new();
    // A three-quarter view: turned toward the viewer's left, looked at a little from above.
    let turn = -0.4;
    let view = glam::Mat4::from_rotation_x(0.35) * glam::Mat4::from_rotation_y(turn);
    crate::model::emit_held_data(&mut verts, view, st, [255, 255, 255, 0], 0);
    // A long gun and its long parts lie across the icon corner to corner, the muzzle end up, to
    // fill it (as Minecraft draws its long items).
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for v in &verts {
        lo = lo.min(glam::Vec2::new(v.pos[0], v.pos[1]));
        hi = hi.max(glam::Vec2::new(v.pos[0], v.pos[1]));
    }
    let size = hi - lo;
    let long = crate::item::GunKind::of(st.item).is_some_and(|k| k.long()) || crate::item::AK_PARTS[..4].contains(&st.item);
    if long && size.x > size.y * 2.0 {
        let c = (lo + hi) * 0.5;
        let tip = glam::Mat4::from_translation(c.extend(0.0))
            * glam::Mat4::from_rotation_z(35f32.to_radians())
            * glam::Mat4::from_translation(-c.extend(0.0));
        for v in &mut verts {
            v.pos = tip.transform_point3(glam::Vec3::from(v.pos)).to_array();
        }
    }
    rasterize(base, &verts, TILE)
}

/// Draws triangles (x right, y up, z toward the viewer) looking straight at them, fitted into
/// a `size` square with a small margin: each pixel the nearest triangle's texel from the
/// texture layers, shaded by which way its face looks. Rendered twice as big and averaged
/// down, so edges are smooth; what is not covered is clear.
fn rasterize(base: &[u8], verts: &[crate::world::mesh::Vertex], size: usize) -> Vec<u8> {
    use glam::{Vec2, Vec3};
    let big = size * 2;
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for v in verts {
        let p = Vec2::new(v.pos[0], v.pos[1]);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let mut out = vec![0u8; size * size * 4];
    if lo.x > hi.x {
        return out;
    }
    let margin = big as f32 * 0.06;
    let scale = (big as f32 - 2.0 * margin) / (hi - lo).max_element().max(1e-6);
    let center = (lo + hi) * 0.5;
    let to_px = |p: Vec3| {
        Vec3::new(
            big as f32 * 0.5 + (p.x - center.x) * scale,
            big as f32 * 0.5 - (p.y - center.y) * scale,
            p.z,
        )
    };
    let mut color = vec![[0f32; 4]; big * big];
    let mut depth = vec![f32::MIN; big * big];
    let light = Vec3::new(-0.45, 0.75, 0.5).normalize();
    let layer_bytes = TILE * TILE * 4;
    for tri in verts.chunks_exact(3) {
        let p: Vec<Vec3> = tri.iter().map(|v| to_px(Vec3::from(v.pos))).collect();
        let world: Vec<Vec3> = tri.iter().map(|v| Vec3::from(v.pos)).collect();
        let n = (world[1] - world[0]).cross(world[2] - world[0]).normalize_or_zero();
        // Faces seen from behind are not drawn (the cubes are closed).
        let n = if n.z < 0.0 { continue } else { n };
        let shade = 0.5 + 0.5 * n.dot(light).max(0.0);
        let area = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[2].x - p[0].x) * (p[1].y - p[0].y);
        if area.abs() < 1e-8 {
            continue;
        }
        let x0 = p.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let x1 = (p.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as usize).min(big - 1);
        let y0 = p.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let y1 = (p.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as usize).min(big - 1);
        let layer = tri[0].layer as usize;
        let tint = tri[0].tint;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((p[1].x - fx) * (p[2].y - fy) - (p[2].x - fx) * (p[1].y - fy)) / area;
                let w1 = ((p[2].x - fx) * (p[0].y - fy) - (p[0].x - fx) * (p[2].y - fy)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * p[0].z + w1 * p[1].z + w2 * p[2].z;
                let i = y * big + x;
                if z <= depth[i] {
                    continue;
                }
                let u = w0 * tri[0].uv[0] + w1 * tri[1].uv[0] + w2 * tri[2].uv[0];
                let v = w0 * tri[0].uv[1] + w1 * tri[1].uv[1] + w2 * tri[2].uv[1];
                let tx = ((u * TILE as f32) as usize).min(TILE - 1);
                let ty = ((v * TILE as f32) as usize).min(TILE - 1);
                let t = layer * layer_bytes + (ty * TILE + tx) * 4;
                if t + 3 >= base.len() || base[t + 3] < 128 {
                    continue;
                }
                depth[i] = z;
                color[i] = [
                    base[t] as f32 * tint[0] as f32 / 255.0 * shade,
                    base[t + 1] as f32 * tint[1] as f32 / 255.0 * shade,
                    base[t + 2] as f32 * tint[2] as f32 / 255.0 * shade,
                    255.0,
                ];
            }
        }
    }
    // Averaged down (the colour weighted by coverage).
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0f32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let c = color[(y * 2 + dy) * big + x * 2 + dx];
                for k in 0..3 {
                    sum[k] += c[k] * c[3] / 255.0;
                }
                sum[3] += c[3];
            }
            let a = sum[3] / 4.0;
            let o = (y * size + x) * 4;
            if a > 0.0 {
                for k in 0..3 {
                    out[o + k] = (sum[k] / (sum[3] / 255.0)).clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = a.round() as u8;
            }
        }
    }
    out
}

/// A dirty copy of the pistol's pages (`level` of `tex::PISTOL_DIRT_LEVELS - 1`): carbon and
/// old oil in blotches over everything, heavier the dirtier, the metal duller. The glass stays
/// clear.
fn synth_grime(base: &mut [u8], first: u32, pages: u32, level: u32) {
    let layer_bytes = TILE * TILE * 4;
    let amount = level as f32 / (tex::PISTOL_DIRT_LEVELS - 1) as f32;
    // Smooth blotches: value noise on a coarse grid, and a finer one on top.
    let hash = |x: i32, y: i32, s: u32| -> f32 {
        let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ s.wrapping_mul(0xC2B2_AE3D);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        (h & 0xffff) as f32 / 65535.0
    };
    let noise = |x: f32, y: f32, cell: f32, s: u32| -> f32 {
        let (gx, gy) = (x / cell, y / cell);
        let (ix, iy) = (gx.floor() as i32, gy.floor() as i32);
        let (fx, fy) = (gx - ix as f32, gy - iy as f32);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let a = hash(ix, iy, s) + (hash(ix + 1, iy, s) - hash(ix, iy, s)) * sx;
        let b = hash(ix, iy + 1, s) + (hash(ix + 1, iy + 1, s) - hash(ix, iy + 1, s)) * sx;
        a + (b - a) * sy
    };
    let grime = [52.0, 44.0, 34.0];
    for page in 0..pages as usize {
        let src = (first as usize + page) * layer_bytes;
        let dst = (first as usize + (level * pages) as usize + page) * layer_bytes;
        for y in 0..TILE {
            for x in 0..TILE {
                let i = (y * TILE + x) * 4;
                let mut px = [base[src + i], base[src + i + 1], base[src + i + 2], base[src + i + 3]];
                if px[3] == 255 {
                    let (fx, fy) = (x as f32, y as f32 + page as f32 * TILE as f32);
                    let n = noise(fx, fy, 11.0, 1) * 0.65 + noise(fx, fy, 3.0, 2) * 0.35;
                    let speck = hash(x as i32, y as i32 + page as i32 * 1000, 3);
                    let k = ((n - 0.62 + 0.55 * amount) * 1.8).clamp(0.0, 0.85)
                        + if speck < 0.05 * amount { 0.35 } else { 0.0 };
                    let dull = 1.0 - 0.22 * amount;
                    for c in 0..3 {
                        let v = px[c] as f32 * dull;
                        px[c] = (v + (grime[c] - v) * k.min(0.9)).clamp(0.0, 255.0) as u8;
                    }
                }
                base[dst + i..dst + i + 4].copy_from_slice(&px);
            }
        }
    }
}

/// Smoke particle sprites, like Minecraft's generic_0..7.
pub const SMOKE_FRAMES: u32 = 8;

fn is_item_icon(l: u32) -> bool {
    (tex::STICK..tex::CHEST_INSIDE).contains(&l)
        || (tex::PIG_SPAWN_EGG..=tex::IRON_NUGGET).contains(&l)
        || l == tex::DOOR_ITEM
        || l == tex::BED_ITEM
        || (tex::MUTTON..=tex::SHEEP_SPAWN_EGG).contains(&l)
        || (tex::HALF_COOKED_PORKCHOP..=tex::BURNT_MUTTON).contains(&l)
        || (tex::HALF_BURNT_PORKCHOP..=tex::RAW_BURNT_MUTTON).contains(&l)
        || (tex::PISTOL..tex::GUN_GLASS).contains(&l)
        || l == tex::COPPER_INGOT
        || (tex::STEEL_INGOT..=tex::TARGET_DUMMY).contains(&l)
        || (tex::ARMOR_ICONS..tex::ARMOR_ICONS + 17).contains(&l)
        || l == tex::BOOK
        || (tex::MORE_TOOLS..tex::MORE_TOOLS + 4).contains(&l)
}

fn is_crack(l: u32) -> bool {
    (tex::CRACK..tex::CRACK + 10).contains(&l)
}

const CUTOUT: &[u32] = &[
    tex::OAK_LEAVES,
    tex::SPRUCE_LEAVES,
    tex::BIRCH_LEAVES,
    tex::GLASS,
    tex::TALL_GRASS,
    tex::POPPY,
    tex::DANDELION,
    tex::DEAD_BUSH,
    tex::OAK_SAPLING,
    tex::BIRCH_SAPLING,
    tex::SPRUCE_SAPLING,
    tex::TORCH,
];

/// Keep the fraction of leaf gaps steady as a texture is minified. Otherwise alpha
/// testing can turn a canopy from see-through up close into a solid wall at distance.
fn preserve_leaf_coverage(base: &[u8], next: &mut [u8], size: usize, layer: usize) {
    let stride = TILE / size;
    let base_start = layer * TILE * TILE * 4;
    let next_start = layer * size * size * 4;
    let base_holes = (0..TILE * TILE)
        .filter(|&i| base[base_start + i * 4 + 3] < 128)
        .count();
    let target = (base_holes * size * size + TILE * TILE / 2) / (TILE * TILE);
    let mut coverage = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            let mut opaque = 0u32;
            for sy in y * stride..(y + 1) * stride {
                for sx in x * stride..(x + 1) * stride {
                    opaque += (base[base_start + (sy * TILE + sx) * 4 + 3] >= 128) as u32;
                }
            }
            // Stable spatial tie-breaker for equal coverage at the smallest mips.
            let tie = ((x * 73) ^ (y * 151) ^ (layer * 31)) as u32;
            coverage.push((opaque, tie, y * size + x));
        }
    }
    coverage.sort_unstable();
    for (rank, &(_, _, i)) in coverage.iter().enumerate() {
        next[next_start + i * 4 + 3] = if rank < target { 0 } else { 255 };
    }
}

fn is_cutout(l: u32) -> bool {
    CUTOUT.contains(&l)
        || is_crack(l)
        || is_item_icon(l)
        || l == tex::LANTERN
        || l == tex::CHAIN
        || l == tex::DOOR_TOP
        || l == tex::DOOR_BOTTOM
        || (tex::BED_HEAD_EAST..=tex::BED_FOOT_END).contains(&l)
        || l == tex::FLAME_PARTICLE
        || l == tex::MUZZLE_FLASH
        || l == tex::MUZZLE_FLASH_SIDE
        || l == tex::BULLET_HOLE
        || l == tex::SLOT_GLOW
        || l == tex::FURNACE_FRONT_CUT
        || l == tex::BLAST_FRONT_CUT
        || l == tex::ADV_FRONT_CUT
        || (tex::SMOKE..tex::SMOKE + SMOKE_FRAMES).contains(&l)
        || l == tex::CLOUD
}

/// Resolution of the opaque-pixel masks used to extrude flat item sprites into 3D models: the
/// texture's own, so every side wall samples the middle of exactly one texel.
pub const MASK: usize = TILE;

/// Opaque pixels of every layer at MASK x MASK (one row per u128, bit x = column x, row 0 at the
/// top of the texture). Filled by `generate`, so it follows the active resource pack.
pub static ITEM_MASKS: std::sync::RwLock<Vec<[u128; MASK]>> = std::sync::RwLock::new(Vec::new());

fn opaque_masks(base: &[u8]) -> Vec<[u128; MASK]> {
    let step = TILE / MASK;
    base.chunks_exact(TILE * TILE * 4)
        .map(|layer| {
            std::array::from_fn(|y| {
                (0..MASK).fold(0u128, |row, x| {
                    let (px, py) = (x * step + step / 2, y * step + step / 2);
                    let a = layer[(py * TILE + px) * 4 + 3];
                    row | (u128::from(a > 127) << x)
                })
            })
        })
        .collect()
}

/// The full mip chain without uploaded skins.
#[cfg(test)]
pub fn generate(packs: &Packs) -> Vec<Vec<u8>> {
    with_skins(&generate_base(packs), &std::collections::HashMap::new())
}

/// Every layer at full size except the uploaded skins: the procedural textures, replaced by
/// the resource packs' where they have them. This is the slow part; `with_skins` finishes it,
/// so a new skin does not have to redo it.
pub fn generate_base(packs: &Packs) -> Vec<u8> {
    let crack = crack_pattern();
    let layers = tex::LAYERS;
    let layer_bytes = TILE * TILE * 4;
    let mut base = vec![0u8; layer_bytes * layers];
    // Layers are independent: generate them in parallel.
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);
    std::thread::scope(|scope| {
        let crack = &crack;
        let mut chunks: Vec<(usize, &mut [u8])> =
            base.chunks_mut(layer_bytes).enumerate().collect();
        let per = chunks.len().div_ceil(threads);
        while !chunks.is_empty() {
            let batch: Vec<(usize, &mut [u8])> = chunks.drain(..per.min(chunks.len())).collect();
            scope.spawn(move || {
                for (l, out) in batch {
                    // Uploaded skins and the double chest faces are filled in later.
                    if (tex::CUSTOM_SKIN_START..tex::DOOR_TOP).contains(&(l as u32)) {
                        continue;
                    }
                    for y in 0..TILE {
                        for x in 0..TILE {
                            let i = (y * TILE + x) * 4;
                            out[i..i + 4]
                                .copy_from_slice(&pixel(l as u32, x as i32, y as i32, crack));
                        }
                    }
                }
            });
        }
    });
    apply_pack(packs, &mut base);
    use crate::model::{ak_vm, gun_station, pistol_vm, revolver_vm};
    synth_model_pages(&mut base, pistol_vm::PNG, pistol_vm::PAGES, tex::PISTOL_VIEW);
    synth_model_pages(&mut base, revolver_vm::PNG, revolver_vm::PAGES, tex::REVOLVER_VIEW);
    synth_model_pages(&mut base, ak_vm::PNG, ak_vm::PAGES, tex::AK_VIEW);
    for level in 1..tex::PISTOL_DIRT_LEVELS {
        synth_grime(&mut base, tex::PISTOL_VIEW, pistol_vm::PAGES, level);
        synth_grime(&mut base, tex::REVOLVER_VIEW, revolver_vm::PAGES, level);
        synth_grime(&mut base, tex::AK_VIEW, ak_vm::PAGES, level);
    }
    synth_model_pages(&mut base, gun_station::PNG, gun_station::PAGES, tex::GUN_STATION_MODEL);
    synth_model_pages(&mut base, gun_station::RIFLE_PNG, gun_station::RIFLE_PAGES, tex::RIFLE_STATION_MODEL);
    synth_model_pages(&mut base, crate::model::grenade::PNG, crate::model::grenade::PAGES, tex::GRENADE_MODEL);
    synth_model_pages(&mut base, crate::model::dummy::PNG, crate::model::dummy::PAGES, tex::DUMMY_MODEL);
    render_item_icons(&mut base);
    synth_doors(&mut base);
    synth_grilled(&mut base);
    synth_glow(&mut base);
    for (front, cut) in [
        (tex::FURNACE_FRONT, tex::FURNACE_FRONT_CUT),
        (tex::BLAST_FRONT, tex::BLAST_FRONT_CUT),
        (tex::ADV_FRONT, tex::ADV_FRONT_CUT),
    ] {
        synth_furnace_cut(&mut base, front, cut);
    }
    synth_furnace_inside(&mut base);
    // Fluids without animation frames: every frame is the still texture (it still scrolls).
    for (still, anim) in [(tex::WATER, tex::WATER_ANIM), (tex::LAVA, tex::LAVA_ANIM)] {
        let src = still as usize * layer_bytes;
        for f in 0..tex::FLUID_FRAMES {
            let dst = (anim + f) as usize * layer_bytes;
            if base[dst..dst + layer_bytes].iter().all(|&v| v == 0) {
                base.copy_within(src..src + layer_bytes, dst);
            }
        }
    }
    // Make the extra outfits from the final texture set, including resource packs. Keep
    // luminance/detail while changing cloth colour; face and exposed hands remain intact.
    const SOURCE: [u32; 5] = [
        tex::SHIRT_FRONT,
        tex::SHIRT,
        tex::SHIRT_BACK,
        tex::ARM,
        tex::LEG,
    ];
    const SHIRTS: [[f32; 3]; 3] = [
        [62.0, 144.0, 85.0],
        [178.0, 66.0, 55.0],
        [77.0, 80.0, 130.0],
    ];
    const PANTS: [[f32; 3]; 3] = [[75.0, 67.0, 53.0], [58.0, 60.0, 65.0], [37.0, 42.0, 55.0]];
    for variant in 1..tex::PRESET_SKINS as usize {
        for (part, &src) in SOURCE.iter().enumerate() {
            let dst = skin_layer(src, variant as u8) as usize * layer_bytes;
            let src = src as usize * layer_bytes;
            let original = base[src..src + layer_bytes].to_vec();
            let target = &mut base[dst..dst + layer_bytes];
            target.copy_from_slice(&original);
            for y in 0..TILE {
                for x in 0..TILE {
                    let i = (y * TILE + x) * 4;
                    let exposed =
                        (part == 0 && y < TILE / 8 && (3 * TILE / 8..5 * TILE / 8).contains(&x))
                            || (part == 3 && y >= 44)
                            || (part == 4 && y >= 104);
                    if exposed || target[i + 3] < 128 {
                        continue;
                    }
                    let color = if part == 4 {
                        PANTS[variant - 1]
                    } else {
                        SHIRTS[variant - 1]
                    };
                    let luma = original[i] as f32 * 0.299
                        + original[i + 1] as f32 * 0.587
                        + original[i + 2] as f32 * 0.114;
                    let reference = if part == 4 { 75.0 } else { 110.0 };
                    let shade = (luma / reference).clamp(0.45, 1.55);
                    for c in 0..3 {
                        target[i + c] = (color[c] * shade).min(255.0) as u8;
                    }
                }
            }
        }
    }
    // Double chest faces. Without the pack's double chest textures, the half of the single
    // chest texture toward the open edge is stretched over the frame there. The top and
    // bottom edges are the left and right ones transposed.
    let half = TILE / 2;
    let stretch = |a: usize| if a < half { a } else { half + (a - half) / 2 };
    let mirror = |a: usize| TILE - 1 - a;
    for (i, src) in [
        tex::CHEST_FRONT,
        tex::CHEST_SIDE,
        tex::CHEST_TOP,
        tex::CHEST_INSIDE,
    ]
    .into_iter()
    .enumerate()
    {
        let src = src as usize * layer_bytes;
        let open = |edge: usize| (tex::CHEST_OPEN as usize + i * 4 + edge) * layer_bytes;
        for edge in 0..2 {
            let dst = open(edge);
            if base[dst..dst + layer_bytes].iter().any(|&v| v != 0) {
                continue; // from the pack
            }
            for y in 0..TILE {
                for x in 0..TILE {
                    let sx = if edge == 0 {
                        stretch(x)
                    } else {
                        mirror(stretch(mirror(x)))
                    };
                    let (s, d) = (src + (y * TILE + sx) * 4, dst + (y * TILE + x) * 4);
                    base.copy_within(s..s + 4, d);
                }
            }
        }
        for (edge, from) in [(2, 1), (3, 0)] {
            let (src, dst) = (open(from), open(edge));
            for y in 0..TILE {
                for x in 0..TILE {
                    let (s, d) = (src + (x * TILE + y) * 4, dst + (y * TILE + x) * 4);
                    base.copy_within(s..s + 4, d);
                }
            }
        }
    }
    base
}

/// Oak door textures (both halves and the item) made from the final oak planks when no pack
/// has them: vertical boards in a darker frame, a recessed panel with a handle below and a
/// four-pane window above.
fn synth_doors(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let layer = |l: u32| l as usize * layer_bytes;
    let empty = |base: &[u8], l: u32| base[layer(l)..layer(l) + layer_bytes].iter().all(|&v| v == 0);
    let planks = base[layer(tex::PLANKS)..layer(tex::PLANKS) + layer_bytes].to_vec();
    // Boards run up and down: the planks texture turned a quarter.
    let wood = |x: usize, y: usize, k: f32| -> [u8; 4] {
        let i = (x * TILE + (TILE - 1 - y)) * 4;
        let c = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
        [c(planks[i]), c(planks[i + 1]), c(planks[i + 2]), 255]
    };
    // One Minecraft pixel is 8 texels.
    const P: usize = TILE / 16;
    let door_pixel = |x: usize, y: usize, upper: bool| -> [u8; 4] {
        let (mx, my) = (x / P, y / P);
        let frame = mx < 2 || mx >= 14 || (upper && my < 2) || (!upper && my >= 14);
        if frame {
            // Bevel: light on the outer top/left edge, dark at the bottom/right.
            let k = if x < 2 || (upper && y < 2) {
                0.95
            } else if x >= TILE - 2 || (!upper && y >= TILE - 2) {
                0.55
            } else {
                0.78
            };
            return wood(x, y, k);
        }
        if upper {
            // Window: 2x2 panes between 3..13 across and 3..11 down.
            if (3..13).contains(&mx) && (3..11).contains(&my) {
                let mullion = mx == 7 || mx == 8 || my == 6 || my == 7;
                if mullion {
                    return wood(x, y, 0.72);
                }
                let shine = if (x % (5 * P)) + (y % (4 * P)) < 3 * P { 1.25 } else { 1.0 };
                let v = |c: f32| (c * shine).min(255.0) as u8;
                return [v(58.0), v(66.0), v(78.0), 255];
            }
            let rim = (2..14).contains(&mx) && (2..12).contains(&my);
            return wood(x, y, if rim { 0.82 } else { 1.0 });
        }
        // Lower half: a recessed panel, and the handle near the top on the free side (the
        // hinges are on the left, as in Minecraft's texture).
        if (12..14).contains(&mx) && (1..4).contains(&my) {
            let (hx, hy) = (x - 12 * P, y - P);
            let rim = hx < 2 || hy < 2 || hx >= 2 * P - 2 || hy >= 3 * P - 2;
            return if rim {
                [38, 38, 42, 255]
            } else if hx < 5 && hy < 5 {
                [150, 150, 158, 255]
            } else {
                [92, 92, 100, 255]
            };
        }
        let panel = (5..11).contains(&mx) && (4..12).contains(&my);
        let k = if panel {
            if x == 5 * P || y == 4 * P {
                0.6
            } else if x == 11 * P - 1 || y == 12 * P - 1 {
                1.1
            } else {
                0.88
            }
        } else {
            1.0
        };
        wood(x, y, k)
    };
    for (l, upper) in [(tex::DOOR_TOP, true), (tex::DOOR_BOTTOM, false)] {
        if !empty(base, l) {
            continue;
        }
        let o = layer(l);
        for y in 0..TILE {
            for x in 0..TILE {
                let i = o + (y * TILE + x) * 4;
                base[i..i + 4].copy_from_slice(&door_pixel(x, y, upper));
            }
        }
    }
    if empty(base, tex::DOOR_ITEM) {
        // The whole door squeezed into the middle of the sprite, with a dark outline.
        let (top, bottom) = (layer(tex::DOOR_TOP), layer(tex::DOOR_BOTTOM));
        let (x0, x1) = (4 * P, 12 * P);
        let o = layer(tex::DOOR_ITEM);
        for y in 0..TILE {
            for x in x0..x1 {
                let sx = (x - x0) * TILE / (x1 - x0);
                let (src, sy) = if y < TILE / 2 { (top, y * 2) } else { (bottom, y * 2 - TILE) };
                let s = src + (sy * TILE + sx) * 4;
                let mut px = [base[s], base[s + 1], base[s + 2], base[s + 3]];
                if x < x0 + 4 || x >= x1 - 4 || y < 4 || y >= TILE - 4 {
                    px = [px[0] / 3, px[1] / 3, px[2] / 3, 255];
                }
                let i = o + (y * TILE + x) * 4;
                base[i..i + 4].copy_from_slice(&px);
            }
        }
    }
}

/// The highlight of a slot or spot: a soft rounded patch that slightly brightens what is under
/// it (drawn multiplied: mid-gray changes nothing), with a faintly brighter rim. Clear outside.
fn synth_glow(base: &mut [u8]) {
    let o = tex::SLOT_GLOW as usize * TILE * TILE * 4;
    let t = TILE as f32;
    let radius = t * 0.2;
    let feather = t * 0.08;
    for y in 0..TILE {
        for x in 0..TILE {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let q = p.map(|v| (v - t * 0.5).abs() - (t * 0.5 - radius));
            let outside =
                (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).min(0.0);
            // Distance inward from the rounded edge.
            let depth = radius - outside;
            let i = o + (y * TILE + x) * 4;
            if depth <= 0.0 {
                continue;
            }
            let fade = (depth / feather).min(1.0);
            let rim = (1.0 - ((depth - feather * 0.9) / (t * 0.05)).abs()).clamp(0.0, 1.0);
            let v = (150.0 + 30.0 * rim) as u8;
            let a = (26.0 + 229.0 * fade) as u8;
            base[i..i + 4].copy_from_slice(&[v, v, v, a]);
        }
    }
}

/// A furnace front (`front`) with its two openings cut out (the mouth above, the firebox
/// below) into `cut`, for the furnace model with real hollows behind them: in each row,
/// everything between the dark outline of an opening is made clear.
fn synth_furnace_cut(base: &mut [u8], front: u32, cut: u32) {
    let layer_bytes = TILE * TILE * 4;
    let src = base[front as usize * layer_bytes..][..layer_bytes].to_vec();
    let o = cut as usize * layer_bytes;
    let dark = |x: usize, y: usize| {
        let i = (y * TILE + x) * 4;
        (src[i] as u32 + src[i + 1] as u32 + src[i + 2] as u32) < 60
    };
    for y in 0..TILE {
        let row = &mut base[o + y * TILE * 4..][..TILE * 4];
        row.copy_from_slice(&src[y * TILE * 4..][..TILE * 4]);
        for px in row.chunks_exact_mut(4) {
            px[3] = 255;
        }
        if !(TILE / 10..TILE - 2).contains(&y) {
            continue;
        }
        let first = (TILE / 16..TILE - TILE / 16).find(|&x| dark(x, y));
        let last = (TILE / 16..TILE - TILE / 16).rev().find(|&x| dark(x, y));
        if let (Some(a), Some(b)) = (first, last) {
            if b - a >= TILE / 8 {
                for x in a..=b {
                    row[x * 4 + 3] = 0;
                }
            }
        }
    }
}

/// Inside a furnace: charred, sooty black all over, with soft smudges of soot and ash and
/// a fine grain (smooth like the other textures, not blocky).
fn synth_furnace_inside(base: &mut [u8]) {
    let o = tex::FURNACE_INSIDE as usize * TILE * TILE * 4;
    // Smooth value noise with cells of `size` texels, tiling across the texture's edges.
    let smooth = |x: usize, y: usize, size: usize, salt: u32| {
        let n = TILE / size;
        let (fx, fy) = (x as f32 / size as f32, y as f32 / size as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let at = |cx: usize, cy: usize| texel_noise(cx % n, cy % n, salt);
        let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * tx;
        let bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * tx;
        top + (bottom - top) * ty
    };
    for y in 0..TILE {
        for x in 0..TILE {
            let smudge = smooth(x, y, TILE / 4, 11) * 0.65 + smooth(x, y, TILE / 16, 13) * 0.35;
            let grain = texel_noise(x, y, 12);
            let v = 22.0 + 28.0 * smudge * smudge + 6.0 * grain;
            let i = o + (y * TILE + x) * 4;
            base[i..i + 4].copy_from_slice(&[(v * 1.06) as u8, v as u8, (v * 0.94) as u8, 255]);
        }
    }
}

/// A hash of a texel for noise, 0..1.
fn texel_noise(x: usize, y: usize, salt: u32) -> f32 {
    let h = (x as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((y as u32).wrapping_mul(0x85EB_CA6B))
        .wrapping_add(salt.wrapping_mul(0xC2B2_AE35));
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    ((h >> 8) & 0xFF) as f32 / 255.0
}

/// `a` over the upper right half of `b` (along a slightly ragged diagonal).
fn half_over(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = b.to_vec();
    for y in 0..TILE {
        for x in 0..TILE {
            // Across the meat (which lies from the lower left to the upper right).
            let wobble = ((x * 7 + y * 13) % 5) as i32 - 2;
            if x as i32 - y as i32 + wobble * 2 > 0 {
                let i = (y * TILE + x) * 4;
                if a[i + 3] > 127 || b[i + 3] <= 127 {
                    out[i..i + 4].copy_from_slice(&a[i..i + 4]);
                }
            }
        }
    }
    out
}

/// Charred: cooked meat blackened almost all over, with a few glowing embers.
fn char_meat(roasted: &[u8]) -> Vec<u8> {
    let mut out = roasted.to_vec();
    for y in 0..TILE {
        for x in 0..TILE {
            let i = (y * TILE + x) * 4;
            let noise = texel_noise(x, y, 3);
            let c = &roasted[i..i + 4];
            let luma = (c[0] as f32 * 0.3 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11) / 255.0;
            let k = 0.16 + 0.22 * luma + 0.06 * noise;
            out[i] = (c[0] as f32 * k + 10.0) as u8;
            out[i + 1] = (c[1] as f32 * k * 0.8 + 6.0) as u8;
            out[i + 2] = (c[2] as f32 * k * 0.7 + 4.0) as u8;
            if noise > 0.985 && c[3] > 127 {
                out[i..i + 3].copy_from_slice(&[168, 58, 18]);
            }
        }
    }
    out
}

/// Grilled meat, made from the final raw and cooked textures (unless a pack has them): burnt
/// (the cooked meat charred), and the pieces with two different sides, shown as the one over
/// the upper right half of the other: one side cooked (cooked over raw), one side burnt
/// (burnt over cooked), and burnt on one side, raw on the other (burnt over raw). On the
/// grill each side shows the raw, cooked or burnt meat's own texture (same shape).
fn synth_grilled(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let get = |base: &[u8], l: u32| base[l as usize * layer_bytes..][..layer_bytes].to_vec();
    let empty = |base: &[u8], l: u32| get(base, l).iter().all(|&v| v == 0);
    let put = |base: &mut [u8], l: u32, px: &[u8]| {
        if empty(base, l) {
            base[l as usize * layer_bytes..][..layer_bytes].copy_from_slice(px);
        }
    };
    for (raw, cooked, half, half_burnt, raw_burnt, burnt) in [
        (
            tex::PORKCHOP,
            tex::COOKED_PORKCHOP,
            tex::HALF_COOKED_PORKCHOP,
            tex::HALF_BURNT_PORKCHOP,
            tex::RAW_BURNT_PORKCHOP,
            tex::BURNT_PORKCHOP,
        ),
        (
            tex::MUTTON,
            tex::COOKED_MUTTON,
            tex::HALF_COOKED_MUTTON,
            tex::HALF_BURNT_MUTTON,
            tex::RAW_BURNT_MUTTON,
            tex::BURNT_MUTTON,
        ),
    ] {
        let (r, c) = (get(base, raw), get(base, cooked));
        put(base, burnt, &char_meat(&c));
        let b = get(base, burnt);
        put(base, half, &half_over(&c, &r));
        put(base, half_burnt, &half_over(&b, &c));
        put(base, raw_burnt, &half_over(&b, &r));
    }
}

/// The full mip chain (each level holds all layers back to back): `generate_base` with the
/// uploaded skins (by player slot) filled in.
pub fn with_skins(base: &[u8], skins: &std::collections::HashMap<u8, Image>) -> Vec<Vec<u8>> {
    let layers = tex::LAYERS;
    let layer_bytes = TILE * TILE * 4;
    let mut base = base.to_vec();
    for slot in 0..tex::CUSTOM_SKIN_SLOTS {
        for (part, &(source, region)) in SKIN_PARTS.iter().enumerate() {
            let dst = (tex::CUSTOM_SKIN_START + slot as u32 * tex::CUSTOM_SKIN_LAYERS + part as u32)
                as usize
                * layer_bytes;
            if let Some(atlas) = skins.get(&slot) {
                let mut pixels = atlas
                    .region(64, region[0], region[1], region[2], region[3])
                    .resized(TILE);
                for p in pixels.chunks_exact_mut(4) {
                    p[3] = 255;
                }
                base[dst..dst + layer_bytes].copy_from_slice(&pixels);
            } else {
                let src = source as usize * layer_bytes;
                base.copy_within(src..src + layer_bytes, dst);
            }
        }
    }
    if let Ok(mut masks) = ITEM_MASKS.write() {
        *masks = opaque_masks(&base);
    }

    // Mipmaps average in linear light; the sRGB decode of every byte value is looked up.
    let linear: [f32; 256] = std::array::from_fn(|v| (v as f32 / 255.0).powf(2.2));
    let mut levels = vec![base];
    let mut size = TILE;
    while size > 1 {
        let prev = levels.last().unwrap();
        let ns = size / 2;
        let mut next = vec![0u8; ns * ns * 4 * layers];
        for l in 0..layers {
            let cutout = is_cutout(l as u32);
            for y in 0..ns {
                for x in 0..ns {
                    let mut rgb = [0.0f32; 3];
                    let mut wsum = 0.0;
                    let mut asum = 0.0;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let i = ((l * size + y * 2 + dy) * size + x * 2 + dx) * 4;
                        let a = prev[i + 3] as f32 / 255.0;
                        let w = if cutout { a } else { 1.0 };
                        for c in 0..3 {
                            rgb[c] += linear[prev[i + c] as usize] * w;
                        }
                        wsum += w;
                        asum += a;
                    }
                    let mut a = asum / 4.0;
                    if cutout && l as u32 != tex::GLASS && !is_crack(l as u32) {
                        a = if a > 0.3 { 1.0 } else { 0.0 };
                    } else if l as u32 == tex::GLASS {
                        a = if a > 0.45 { 1.0 } else { 0.0 };
                    }
                    let o = ((l * ns + y) * ns + x) * 4;
                    for c in 0..3 {
                        let v = if wsum > 0.0 { rgb[c] / wsum } else { 0.0 };
                        next[o + c] = (v.powf(1.0 / 2.2) * 255.0).round() as u8;
                    }
                    // Keep the tint masks exact instead of averaging them.
                    let first = prev[((l * size + y * 2) * size + x * 2) * 4 + 3];
                    next[o + 3] = if !cutout && l as u32 != tex::GRASS_SIDE && !is_crack(l as u32) {
                        first
                    } else {
                        (a * 255.0).round() as u8
                    };
                }
            }
        }
        for layer in [tex::OAK_LEAVES, tex::SPRUCE_LEAVES, tex::BIRCH_LEAVES] {
            preserve_leaf_coverage(&levels[0], &mut next, ns, layer as usize);
        }
        levels.push(next);
        size = ns;
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploaded_skin_maps_its_face_and_clothes_to_own_slot() {
        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_data, 64, 64);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut pixels = vec![0u8; 64 * 64 * 4];
            for p in pixels.chunks_exact_mut(4) {
                p.copy_from_slice(&[32, 144, 220, 255]);
            }
            writer.write_image_data(&pixels).unwrap();
        }
        let skin = decode_skin_png(&png_data).unwrap();
        let mut skins = std::collections::HashMap::new();
        skins.insert(2, skin);
        let levels = with_skins(&generate_base(&Packs::none()), &skins);
        let offset = skin_layer(tex::FACE, 6) as usize * TILE * TILE * 4;
        assert_eq!(&levels[0][offset..offset + 4], &[32, 144, 220, 255]);
        assert_ne!(skin_layer(tex::FACE, 6), skin_layer(tex::FACE, 5));
    }

    #[test]
    fn outfits_have_distinct_cloth_and_shared_face() {
        let levels = generate(&Packs::none());
        let base = &levels[0];
        let bytes = TILE * TILE * 4;
        let pixel = |layer: u32, x: usize, y: usize| {
            let i = layer as usize * bytes + (y * TILE + x) * 4;
            &base[i..i + 4]
        };
        for skin in 1..tex::PRESET_SKINS {
            assert_ne!(
                pixel(tex::SHIRT, 30, 50),
                pixel(skin_layer(tex::SHIRT, skin), 30, 50)
            );
            assert_eq!(skin_layer(tex::FACE, skin), tex::FACE);
            assert_eq!(
                pixel(tex::ARM, 30, 100),
                pixel(skin_layer(tex::ARM, skin), 30, 100)
            );
        }
    }

    /// Every flat item sprite gets a real outline to extrude: some opaque pixels, not all.
    #[test]
    fn item_masks_have_outlines() {
        generate(&Packs::none());
        let masks = ITEM_MASKS.read().unwrap();
        for id in crate::item::all_items() {
            if let crate::item::Icon::Flat(layer) = crate::item::icon(id) {
                let n: u32 = masks[layer as usize].iter().map(|r| r.count_ones()).sum();
                assert!(
                    n > 0 && n < (MASK * MASK) as u32,
                    "item {id}: {n} opaque pixels"
                );
            }
        }
    }

    #[test]
    fn flame_placeholder_and_furnace_stone_stays_stable() {
        let levels = generate(&Packs::none());
        let base = &levels[0];
        let flame_start = tex::TORCH_FLAME as usize * TILE * TILE * 4;
        assert!(base[flame_start..flame_start + TILE * TILE * 4]
            .chunks_exact(4)
            .all(|p| p == [255, 255, 255, 255]));
        let layer_bytes = TILE * TILE * 4;
        let lit = tex::FURNACE_FRONT_LIT as usize * layer_bytes;
        let unlit = tex::FURNACE_FRONT as usize * layer_bytes;
        assert_eq!(
            &base[lit..lit + layer_bytes],
            &base[unlit..unlit + layer_bytes]
        );
    }

    #[test]
    fn leaf_gaps_have_consistent_coverage_across_species_and_mips() {
        let levels = generate(&Packs::none());
        for (mip, pixels) in levels.iter().take(5).enumerate() {
            let size = TILE >> mip;
            let holes: Vec<usize> = [tex::OAK_LEAVES, tex::SPRUCE_LEAVES, tex::BIRCH_LEAVES]
                .into_iter()
                .map(|layer| {
                    let start = layer as usize * size * size * 4;
                    (0..size * size)
                        .filter(|&i| pixels[start + i * 4 + 3] < 128)
                        .count()
                })
                .collect();
            assert!(
                holes.iter().max().unwrap() - holes.iter().min().unwrap() <= 1,
                "mip {mip}: {holes:?}"
            );
            if mip == 0 {
                assert!(
                    holes[0] > size * size / 20 && holes[0] < size * size / 4,
                    "leaf gaps: {holes:?}"
                );
            }
        }
    }

    /// Generates every layer; with TEX_DUMP=<file.bmp> also writes a contact sheet to look at.
    #[test]
    fn shader_layer_numbers_match() {
        let src = include_str!("../../../shaders/world.frag");
        let value = |name: &str| -> u32 {
            let line = src
                .lines()
                .find(|l| l.starts_with(&format!("const float {name} = ")))
                .unwrap_or_else(|| panic!("{name} missing"));
            let v = line.split('=').nth(1).unwrap().trim().trim_end_matches(';');
            v.parse::<f32>().unwrap() as u32
        };
        assert_eq!(value("GRASS_SIDE_LAYER"), tex::GRASS_SIDE);
        assert_eq!(value("GRASS_TOP_LAYER"), tex::GRASS_TOP);
        assert_eq!(value("SNOW_LAYER"), tex::SNOW);
        assert_eq!(value("SNOWY_GRASS_SIDE_LAYER"), tex::SNOWY_GRASS_SIDE);
        assert_eq!(value("GLASS_LAYER"), tex::GLASS);
        assert_eq!(value("FURNACE_LIT_LAYER"), tex::FURNACE_FRONT_LIT);
        assert_eq!(value("TORCH_FLAME_LAYER"), tex::TORCH_FLAME);
        assert_eq!(value("WATER_ANIM_LAYER"), tex::WATER_ANIM);
        assert_eq!(value("LAVA_ANIM_LAYER"), tex::LAVA_ANIM);
        assert_eq!(value("FLUID_FRAMES"), tex::FLUID_FRAMES);
    }

    #[test]
    fn generate_all_layers() {
        // TEX_PACK=<name in resourcepacks/> dumps a resource pack's version instead (over the
        // built-in one); TEX_PACK=builtin dumps the built-in pack's.
        let packs = match std::env::var("TEX_PACK") {
            Ok(n) if n == "builtin" => Packs::load(&[]),
            Ok(n) => Packs::load(&[n]),
            Err(_) => Packs::none(),
        };
        let levels = generate(&packs);
        assert_eq!(levels[0].len(), TILE * TILE * 4 * tex::LAYERS);
        let Ok(path) = std::env::var("TEX_DUMP") else {
            return;
        };
        let cols = 12;
        let rows = tex::LAYERS.div_ceil(cols);
        let (w, h) = (cols * TILE, rows * TILE);
        let mut img = vec![0u8; w * h * 3];
        for l in 0..tex::LAYERS {
            let (ox, oy) = ((l % cols) * TILE, (l / cols) * TILE);
            for y in 0..TILE {
                for x in 0..TILE {
                    let s = ((l * TILE + y) * TILE + x) * 4;
                    let p = &levels[0][s..s + 4];
                    // Checkerboard behind cut-out pixels; tint grayscale layers green for viewing.
                    let bg = if (x / 8 + y / 8) % 2 == 0 { 60 } else { 90 };
                    let tinted = p[3] == 255
                        && matches!(
                            l as u32,
                            tex::GRASS_TOP
                                | tex::GRASS_SIDE
                                | tex::TALL_GRASS
                                | tex::OAK_LEAVES
                                | tex::SPRUCE_LEAVES
                                | tex::BIRCH_LEAVES
                        );
                    // TEX_ALPHA=1 shows the alpha channel instead (tint masks, fire mask).
                    let show_alpha = std::env::var("TEX_ALPHA").is_ok();
                    let rgb = if show_alpha {
                        [p[3]; 3]
                    } else if p[3] < 128 {
                        [bg, bg, bg]
                    } else if tinted {
                        [
                            (p[0] as u32 * 112 / 255) as u8,
                            (p[1] as u32 * 170 / 255) as u8,
                            (p[2] as u32 * 72 / 255) as u8,
                        ]
                    } else {
                        [p[0], p[1], p[2]]
                    };
                    let d = ((oy + y) * w + ox + x) * 3;
                    img[d..d + 3].copy_from_slice(&rgb);
                }
            }
        }
        // 24-bit BMP, rows bottom-up, BGR.
        let row = (w * 3).div_ceil(4) * 4;
        let size = 54 + row * h;
        let mut f = Vec::with_capacity(size);
        f.extend_from_slice(b"BM");
        f.extend_from_slice(&(size as u32).to_le_bytes());
        f.extend_from_slice(&[0; 4]);
        f.extend_from_slice(&54u32.to_le_bytes());
        f.extend_from_slice(&40u32.to_le_bytes());
        f.extend_from_slice(&(w as i32).to_le_bytes());
        f.extend_from_slice(&(h as i32).to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&24u16.to_le_bytes());
        f.extend_from_slice(&[0; 24]);
        for y in (0..h).rev() {
            for x in 0..w {
                let s = (y * w + x) * 3;
                f.extend_from_slice(&[img[s + 2], img[s + 1], img[s]]);
            }
            f.resize(f.len() + row - w * 3, 0);
        }
        std::fs::write(path, f).unwrap();
    }
}
