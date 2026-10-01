//! Procedurally generated 128x128 textures (no external assets).
//!
//! Shapes are designed on a 32-unit grid (`d(x)` converts a pixel to design units) and
//! rendered at 128 px, so edges stay crisp while shading and detail get the full resolution.
//!
//! For opaque textures the alpha channel doubles as a *mask*: 255 = fully biome tinted,
//! 153 = untinted. Fire is generated continuously in the fragment shader.
//! Values below 128 are cut out (leaves, plants, glass).
//!
//! This module only picks the drawing for a layer (`pixel`); the drawings are in `terrain`,
//! `blocks`, `items`, `guns` and `entities`, the helpers in `noise` and `draw`.

mod blocks;
mod draw;
mod entities;
mod guns;
mod items;
pub(super) mod noise;
mod terrain;

use super::mips::is_item_icon;
use super::{tex, SMOKE_FRAMES, TILE, UNTINTED};
use blocks::*;
use draw::*;
use entities::*;
use guns::*;
use items::*;
use noise::*;
use terrain::*;

pub(super) use blocks::crack_pattern;
pub(super) use noise::hash;

pub(super) fn pixel(layer: u32, x: i32, y: i32, crack: &[u16]) -> [u8; 4] {
    let l = layer;
    match layer {
        tex::GRASS_TOP => grass_gray(l, x, y),
        tex::GRASS_SIDE | tex::SNOWY_GRASS_SIDE => grass_side(l, x, y),
        tex::DIRT => dirt(l, x, y),
        tex::STONE => stone_base(l, x, y),
        tex::SAND => sand(l, x, y),
        tex::OAK_LOG => log_side(l, x, y, [104.0, 80.0, 50.0]),
        tex::OAK_LOG_TOP => log_top(
            l,
            x,
            y,
            [104.0, 80.0, 50.0],
            [186.0, 150.0, 96.0],
            [148.0, 114.0, 68.0],
        ),
        tex::OAK_LEAVES => leaves(l, x, y, false),
        tex::SPRUCE_LEAVES => leaves(l, x, y, true),
        tex::BIRCH_LEAVES => leaves(l, x, y, false),
        tex::WATER => water(l, x, y),
        tex::SNOW => snow(l, x, y),
        tex::PLANKS => planks(l, x, y),
        tex::BUCKET_METAL => bucket_metal(l, x, y),
        tex::COBBLE => cobble(l, x, y),
        tex::BEDROCK => bedrock(l, x, y),
        tex::GLASS => glass(x, y),
        tex::BRICKS => bricks(l, x, y),
        tex::GRAVEL => gravel(l, x, y),
        tex::SANDSTONE => sandstone(l, x, y),
        tex::SANDSTONE_TOP => sandstone_top(l, x, y),
        tex::SPRUCE_LOG => log_side(l, x, y, [64.0, 45.0, 28.0]),
        tex::SPRUCE_LOG_TOP => log_top(
            l,
            x,
            y,
            [64.0, 45.0, 28.0],
            [134.0, 100.0, 62.0],
            [104.0, 76.0, 46.0],
        ),
        tex::BIRCH_LOG => birch_log(l, x, y),
        tex::BIRCH_LOG_TOP => log_top(
            l,
            x,
            y,
            [216.0, 214.0, 206.0],
            [206.0, 190.0, 150.0],
            [180.0, 160.0, 120.0],
        ),
        tex::CACTUS => cactus(l, x, y),
        tex::CACTUS_TOP => cactus_top(x, y),
        tex::TALL_GRASS => tall_grass(l, x, y),
        tex::POPPY => flower(l, x, y, [206.0, 32.0, 30.0], [40.0, 24.0, 16.0], 5.5),
        tex::DANDELION => flower(l, x, y, [246.0, 214.0, 40.0], [214.0, 150.0, 20.0], 4.5),
        tex::DEAD_BUSH => dead_bush(l, x, y),
        tex::COAL_ORE => ore(l, x, y, [40.0, 40.0, 44.0]),
        tex::IRON_ORE => ore(l, x, y, [216.0, 176.0, 146.0]),
        tex::COPPER_ORE => ore(l, x, y, [226.0, 124.0, 74.0]),
        tex::COPPER_BLOCK => metal_block(l, x, y, [200.0, 112.0, 70.0]),
        tex::GOLD_ORE => ore(l, x, y, [250.0, 214.0, 64.0]),
        tex::DIAMOND_ORE => ore(l, x, y, [98.0, 234.0, 226.0]),
        tex::OBSIDIAN => obsidian(l, x, y),
        tex::ICE => ice(l, x, y),
        tex::CLAY => clay(l, x, y),
        tex::LAVA => lava(l, x, y),
        tex::GLOWSTONE => glowstone(l, x, y),
        tex::SKIN => skin(l, x, y),
        tex::SLEEVE => col(SHIRT_C, 0.9 + 0.1 * fbm(l, x, y, 280), UNTINTED),
        tex::FACE..=tex::LEG => character(l, x, y),
        tex::CRAFTING_TOP..=tex::STONE_BRICKS => new_block(l, x, y),
        tex::CHEST_INSIDE | tex::CHEST_LATCH => chest(l, x, y),
        tex::TORCH_WOOD => torch_wood(l, x, y),
        tex::TORCH_CAP => torch_cap(l, x, y),
        // The flame's shape and colors are computed continuously by the shader.
        tex::TORCH_FLAME => [255, 255, 255, 255],
        tex::TORCH_CHAR => torch_char(l, x, y),
        tex::HEAD_BACK => character(tex::HAIR, x, y),
        // The new furnaces' looks come from the built-in pack; these stand in without it.
        tex::BLAST_FRONT | tex::ADV_FRONT => pixel(tex::FURNACE_FRONT, x, y, crack),
        tex::BLAST_FRONT_CUT | tex::ADV_FRONT_CUT => [0, 0, 0, 0],
        tex::BLAST_SIDE
        | tex::ADV_SIDE
        | tex::ADV_PANEL
        | tex::ADV_HOOD_L
        | tex::ADV_HOOD_R
        | tex::ADV_HOOD_L_LIT
        | tex::ADV_HOOD_R_LIT => pixel(tex::FURNACE_SIDE, x, y, crack),
        tex::BLAST_TOP | tex::ADV_TOP | tex::ADV_VENT_TOP | tex::CHIMNEY_TOP => {
            pixel(tex::FURNACE_TOP, x, y, crack)
        }
        tex::CHIMNEY_SIDE => pixel(tex::BRICKS, x, y, crack),
        _ if (tex::FURNACE_ANIM..tex::FLAME_PARTICLE).contains(&l) => {
            pixel(tex::FURNACE_FRONT_LIT, x, y, crack)
        }
        tex::FLAME_PARTICLE => flame_particle(x, y),
        _ if (tex::SMOKE..tex::SMOKE + SMOKE_FRAMES).contains(&l) => smoke_puff(l, x, y),
        tex::CLOUD => cloud_puff(l, x, y),
        // Filled from the Blockbench view model's texture in `synth_pistol_view`.
        _ if (tex::PISTOL_VIEW..tex::WOLF).contains(&l) => [0, 0, 0, 0],
        tex::SHIRT_BACK => character(tex::SHIRT, x, y),
        // The pig's and the sheep's skin pages: built-in atlases (`pig_skin`, plain colors) cut up
        // like a pack's.
        _ if (tex::PIG..tex::SHEEP).contains(&l) => pig_page(l, x, y),
        _ if (tex::SHEEP..tex::SHEEP_WOOL).contains(&l) => sheep_page(l, x, y),
        _ if (tex::SHEEP_WOOL..tex::FISHING_ROD_MODEL).contains(&l) => sheep_wool_page(l, x, y),
        tex::SPARE_PIG | tex::SPARE_SHEEP | tex::SPARE_SHEEP_WOOL => [0, 0, 0, 0],
        // The fishing rod's model pages and its icon, drawn from the model later
        // (`synth_model_pages`, `render_item_icons`).
        tex::FISHING_ROD_MODEL..=tex::FISHING_ROD => [0, 0, 0, 0],
        tex::LANTERN => lantern(x, y),
        tex::CHAIN => chain(x, y),
        // Made from the final planks in `synth_doors` unless a pack has them.
        tex::DOOR_TOP..=tex::DOOR_ITEM => [0, 0, 0, 0],
        tex::BED_HEAD_TOP..=tex::BED_ITEM => bed(l, x, y),
        tex::GUN_STATION_TOP | tex::GUN_STATION_SIDE | tex::GUN_STATION_BOTTOM => {
            gun_station(l, x, y)
        }
        tex::GUN_BLUED | tex::GUN_STEEL | tex::GUN_POLYMER | tex::GUN_GLASS | tex::GUN_WOOD => {
            gun_surface(l, x, y)
        }
        tex::MUZZLE_FLASH | tex::MUZZLE_FLASH_SIDE => muzzle_flash(l, x, y),
        tex::BULLET_HOLE => bullet_hole(l, x, y),
        tex::ARMOR_WOOL | tex::ARMOR_METAL | tex::VEST => armor_surface(l, x, y),
        tex::BOOK_COVER | tex::BOOK_EDGE | tex::BOOK_PAGE => book_surface(l, x, y),
        // Blank until a page is drawn onto them.
        // (the guide book's pages and what follows them up to the wolf, filled in later)
        _ if (tex::BOOK_SHEETS..tex::WOLF).contains(&l) => col(BOOK_PAPER, 1.0, 255),
        // Filled from the packs' animations, or copies of the still texture.
        _ if (tex::WATER_ANIM..tex::GUN_STATION_TOP).contains(&l) => [0, 0, 0, 0],
        // Wool: soft white fibres. The sheep atlases are plain: skin and a white coat.
        tex::WOOL => wool(l, x, y),
        // The wolf: grey fur (the packs draw it); its collar only in a band round the neck.
        _ if (tex::WOLF..tex::WOLF_COLLAR).contains(&l) => col([196.0, 192.0, 186.0], 0.9 + 0.1 * grain(l, x, y, 563), 255),
        _ if (tex::WOLF_COLLAR..tex::BONE).contains(&l) => [0, 0, 0, 0],
        // Made in `synth_grilled`.
        tex::HALF_BURNT_PORKCHOP..=tex::RAW_BURNT_MUTTON => [0, 0, 0, 0],
        _ if is_item_icon(l) => item_icon(l, x, y),
        _ => crack_pixel(layer, x, y, crack),
    }
}
