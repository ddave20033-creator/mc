//! The texture array's layer numbers, one per 128x128 texture (`TILE`); some name the first
//! of a run of layers (animation frames, a model's pages...).

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
/// A spare layer (the pig's whole atlas once; its skin is on the `PIG` pages now).
pub const SPARE_PIG: u32 = SMOKE + super::SMOKE_FRAMES;
pub const PIG_SPAWN_EGG: u32 = SPARE_PIG + 1;
pub const PORKCHOP: u32 = SPARE_PIG + 2;
pub const COOKED_PORKCHOP: u32 = SPARE_PIG + 3;
pub const GLASS_BOTTLE: u32 = SPARE_PIG + 4;
pub const WATER_BOTTLE: u32 = SPARE_PIG + 5;
pub const PURIFIED_WATER: u32 = SPARE_PIG + 6;
pub const LANTERN_ITEM: u32 = SPARE_PIG + 7;
pub const IRON_NUGGET: u32 = SPARE_PIG + 8;
/// Lantern block texture (Minecraft layout: body, cap, handle), and a chain.
pub const LANTERN: u32 = SPARE_PIG + 9;
pub const CHAIN: u32 = SPARE_PIG + 10;
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
/// Two spare layers (the sheep's and its wool's whole atlases once; they are on the
/// `SHEEP` and `SHEEP_WOOL` pages now).
pub const SPARE_SHEEP: u32 = BED_ITEM + 1;
pub const SPARE_SHEEP_WOOL: u32 = SPARE_SHEEP + 1;
pub const WOOL: u32 = SPARE_SHEEP + 2;
pub const MUTTON: u32 = SPARE_SHEEP + 3;
pub const COOKED_MUTTON: u32 = SPARE_SHEEP + 4;
pub const SHEARS: u32 = SPARE_SHEEP + 5;
pub const SHEEP_SPAWN_EGG: u32 = SPARE_SHEEP + 6;
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
/// (`client::book`): `BOOK_SHEET_COUNT` pages of `model::book::SHEET_LAYERS` layers each.
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
/// is, a gun with its attachments and dirt...: `client::gui::icons`).
/// The AK-47 made in Blockbench: its texture pages (`model::ak_vm`), clean and dirtier,
/// like the pistol's (`PISTOL_DIRT_LEVELS` sets).
pub const AK_VIEW: u32 = DUMMY_MODEL + crate::model::dummy::PAGES;
/// The rifle station made in Blockbench: its texture pages (`model::gun_station`).
pub const RIFLE_STATION_MODEL: u32 = AK_VIEW + crate::model::ak_vm::PAGES * PISTOL_DIRT_LEVELS;
pub const STATE_ICONS: u32 = RIFLE_STATION_MODEL + crate::model::gun_station::RIFLE_PAGES;
pub const STATE_ICON_COUNT: u32 = 64;
/// The wolf's atlases (wild, tame, angry, and its collar, tinted by the game), a bone and
/// the wolf spawn egg.
/// (each `entity::mob::wolf_skin::PAGES` layers, see `entity::skin_pages`)
pub const WOLF: u32 = STATE_ICONS + STATE_ICON_COUNT;
pub const WOLF_TAME: u32 = WOLF + crate::entity::mob::wolf_skin::PAGES;
pub const WOLF_ANGRY: u32 = WOLF_TAME + crate::entity::mob::wolf_skin::PAGES;
pub const WOLF_COLLAR: u32 = WOLF_ANGRY + crate::entity::mob::wolf_skin::PAGES;
pub const BONE: u32 = WOLF_COLLAR + crate::entity::mob::wolf_skin::PAGES;
pub const WOLF_SPAWN_EGG: u32 = BONE + 1;
/// The pig's skin, the sheep's and its wool coat, on their pages
/// (`entity::mob::pig_skin`, `entity::mob::sheep_skin`).
pub const PIG: u32 = WOLF_SPAWN_EGG + 1;
pub const SHEEP: u32 = PIG + crate::entity::mob::pig_skin::PAGES;
pub const SHEEP_WOOL: u32 = SHEEP + crate::entity::mob::sheep_skin::PAGES;
/// Fishing: the rod made in Blockbench (its texture pages, `model::fishing_rod`), the
/// rod's icon, and the fish, raw and cooked.
pub const FISHING_ROD_MODEL: u32 = SHEEP_WOOL + crate::entity::mob::sheep_skin::WOOL_PAGES;
pub const FISHING_ROD: u32 = FISHING_ROD_MODEL + crate::model::fishing_rod::PAGES;
pub const RAW_FISH: u32 = FISHING_ROD + 1;
pub const COOKED_FISH: u32 = FISHING_ROD + 2;
/// The bucket's galvanized steel (`model::bucket`).
pub const BUCKET_METAL: u32 = COOKED_FISH + 1;
/// The mark of a cut-down trunk on grass, a stage each (`world::STUMP_MARK`): bare soil
/// in a circle, the rest see-through.
pub const STUMP_MARK: u32 = BUCKET_METAL + 1;
/// The game's logo (`ui/logo.png`), in tiles from its left.
pub const LOGO: u32 = STUMP_MARK + crate::world::STUMP_STAGES as u32;
pub const LOGO_TILES: u32 = 8;
pub const LAYERS: usize = (LOGO + LOGO_TILES) as usize;

/// Texture layer of a tool: `tier` and `kind` as `Tier as usize` and `ToolKind as usize`.
pub const fn tool_layer(tier: usize, kind: usize) -> u32 {
    let i = (tier * 4 + kind) as u32;
    if i < 20 {
        TOOLS + i
    } else {
        MORE_TOOLS + i - 20
    }
}
