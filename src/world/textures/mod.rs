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
    /// Gun station faces.
    pub const GUN_STATION_TOP: u32 = LAVA_ANIM + FLUID_FRAMES;
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
    pub const LAYERS: usize = (GUN_GLASS + 1) as usize;
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

/// Smoke particle sprites, like Minecraft's generic_0..7.
pub const SMOKE_FRAMES: u32 = 8;

fn is_item_icon(l: u32) -> bool {
    (tex::STICK..tex::CHEST_INSIDE).contains(&l)
        || (tex::PIG_SPAWN_EGG..=tex::IRON_NUGGET).contains(&l)
        || l == tex::DOOR_ITEM
        || l == tex::BED_ITEM
        || (tex::MUTTON..=tex::SHEEP_SPAWN_EGG).contains(&l)
        || (tex::PISTOL..tex::GUN_GLASS).contains(&l)
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
        || (tex::SMOKE..tex::SMOKE + SMOKE_FRAMES).contains(&l)
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
    synth_doors(&mut base);
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
