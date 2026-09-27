//! Resource pack textures: the layers taken straight from the pack, the ones cut out of its
//! entity atlases (chests, the torch), the lit furnace's animation and the double chest.

use super::procedural::hash;
use super::*;
// ------------------------------------------------------------------ resource packs

/// Layers taken directly from a resource pack texture (`block/...`, `item/...`).
/// Alternatives are separated by `|` (texture names changed between Minecraft versions).
const PACK_TEXTURES: &[(u32, &str)] = &[
    (tex::GRASS_TOP, "block/grass_block_top"),
    (tex::DIRT, "block/dirt"),
    (tex::STONE, "block/stone"),
    (tex::SAND, "block/sand"),
    (tex::OAK_LOG, "block/oak_log"),
    (tex::OAK_LOG_TOP, "block/oak_log_top"),
    (tex::OAK_LEAVES, "block/oak_leaves"),
    (tex::WATER, "block/water_still"),
    (tex::SNOW, "block/snow"),
    (tex::PLANKS, "block/oak_planks"),
    (tex::COBBLE, "block/cobblestone"),
    (tex::BEDROCK, "block/bedrock"),
    (tex::GLASS, "block/glass"),
    (tex::BRICKS, "block/bricks"),
    (tex::GRAVEL, "block/gravel"),
    (tex::SNOWY_GRASS_SIDE, "block/grass_block_snow"),
    (tex::SANDSTONE, "block/sandstone"),
    (tex::SANDSTONE_TOP, "block/sandstone_top"),
    (tex::SPRUCE_LOG, "block/spruce_log"),
    (tex::SPRUCE_LOG_TOP, "block/spruce_log_top"),
    (tex::SPRUCE_LEAVES, "block/spruce_leaves"),
    (tex::BIRCH_LOG, "block/birch_log"),
    (tex::BIRCH_LOG_TOP, "block/birch_log_top"),
    (tex::BIRCH_LEAVES, "block/birch_leaves"),
    (tex::CACTUS, "block/cactus_side"),
    (tex::CACTUS_TOP, "block/cactus_top"),
    (tex::TALL_GRASS, "block/short_grass|block/grass"),
    (tex::POPPY, "block/poppy"),
    (tex::DANDELION, "block/dandelion"),
    (tex::DEAD_BUSH, "block/dead_bush"),
    (tex::COAL_ORE, "block/coal_ore"),
    (tex::IRON_ORE, "block/iron_ore"),
    (tex::GOLD_ORE, "block/gold_ore"),
    (tex::DIAMOND_ORE, "block/diamond_ore"),
    (tex::OBSIDIAN, "block/obsidian"),
    (tex::ICE, "block/ice"),
    (tex::CLAY, "block/clay"),
    (tex::LAVA, "block/lava_still"),
    (tex::GLOWSTONE, "block/glowstone"),
    (tex::CRAFTING_TOP, "block/crafting_table_top"),
    (tex::CRAFTING_SIDE, "block/crafting_table_side"),
    (tex::CRAFTING_FRONT, "block/crafting_table_front"),
    (tex::FURNACE_FRONT, "block/furnace_front"),
    (tex::FURNACE_FRONT_LIT, "block/furnace_front_on"),
    (tex::FURNACE_SIDE, "block/furnace_side"),
    (tex::FURNACE_TOP, "block/furnace_top"),
    (tex::TORCH, "block/torch"),
    (tex::OAK_SAPLING, "block/oak_sapling"),
    (tex::BIRCH_SAPLING, "block/birch_sapling"),
    (tex::SPRUCE_SAPLING, "block/spruce_sapling"),
    (tex::IRON_BLOCK, "block/iron_block"),
    (tex::GOLD_BLOCK, "block/gold_block"),
    (tex::DIAMOND_BLOCK, "block/diamond_block"),
    (tex::COAL_BLOCK, "block/coal_block"),
    (tex::STONE_BRICKS, "block/stone_bricks"),
    (tex::STICK, "item/stick"),
    (tex::COAL, "item/coal"),
    (tex::CHARCOAL, "item/charcoal"),
    (tex::IRON_INGOT, "item/iron_ingot"),
    (tex::GOLD_INGOT, "item/gold_ingot"),
    (tex::DIAMOND, "item/diamond"),
    (tex::CLAY_BALL, "item/clay_ball"),
    (tex::BRICK, "item/brick"),
    (tex::BUCKET, "item/bucket"),
    (tex::WATER_BUCKET, "item/water_bucket"),
    (tex::LAVA_BUCKET, "item/lava_bucket"),
    (tex::PIG_SPAWN_EGG, "item/pig_spawn_egg"),
    (tex::PORKCHOP, "item/porkchop"),
    (tex::COOKED_PORKCHOP, "item/cooked_porkchop"),
    (tex::GLASS_BOTTLE, "item/glass_bottle"),
    (tex::LANTERN_ITEM, "item/lantern"),
    (tex::IRON_NUGGET, "item/iron_nugget"),
    (tex::LANTERN, "block/lantern"),
    (tex::CHAIN, "block/iron_chain|block/chain"),
    (tex::DOOR_TOP, "block/oak_door_top"),
    (tex::DOOR_BOTTOM, "block/oak_door_bottom"),
    (tex::DOOR_ITEM, "item/oak_door"),
    // Faithful's per-face bed textures (Minecraft itself draws beds from an entity atlas).
    (tex::BED_HEAD_TOP, "block/red_bed_head_up"),
    (tex::BED_FOOT_TOP, "block/red_bed_foot_up"),
    (tex::BED_HEAD_EAST, "block/red_bed_head_east"),
    (tex::BED_HEAD_WEST, "block/red_bed_head_west"),
    (tex::BED_FOOT_EAST, "block/red_bed_foot_east"),
    (tex::BED_FOOT_WEST, "block/red_bed_foot_west"),
    (tex::BED_HEAD_END, "block/bed_head_north"),
    (tex::BED_FOOT_END, "block/red_bed_foot_south"),
    (tex::BED_BOTTOM, "block/bed_down"),
    (tex::BED_ITEM, "item/red_bed|item/bed"),
    (tex::WOOL, "block/white_wool|block/wool_colored_white"),
    (tex::MUTTON, "item/mutton|item/mutton_raw"),
    (tex::COOKED_MUTTON, "item/cooked_mutton|item/mutton_cooked"),
    (tex::SHEARS, "item/shears"),
    (tex::SHEEP_SPAWN_EGG, "item/sheep_spawn_egg"),
];

/// Deterministic value noise in 0..1 along one axis (period of about 1 unit).
fn noise1(x: f32, seed: u32) -> f32 {
    let h = |i: i32| hash(seed, i, 0, 777);
    let i = x.floor() as i32;
    let f = x - x.floor();
    let f = f * f * (3.0 - 2.0 * f);
    h(i) * (1.0 - f) + h(i + 1) * f
}

/// Animation frames for a resource pack's lit furnace, drawn from its own art: the fire is
/// the part of the lit front that differs from the unlit one (lower half). Every frame
/// shifts it with a smooth displacement: each tongue of flame (about 1/8 of the texture
/// wide) stretches up and shrinks back with its own phase and sways a little, more at the
/// tips than at the base. Where no flame reaches, the unlit (dark) opening shows through.
fn furnace_frames(lit: &Image, unlit: &Image) -> Vec<Image> {
    let (w, h) = (lit.w as i32, lit.h as i32);
    let fire = |x: i32, y: i32| {
        (0..w).contains(&x) && (h / 2..h).contains(&y) && {
            let (a, b) = (
                lit.pixel(x as u32, y as u32),
                unlit.pixel(x as u32, y as u32),
            );
            (0..3)
                .map(|c| (a[c] as i32 - b[c] as i32).abs())
                .sum::<i32>()
                > 40
        }
    };
    let (mut top, mut bottom) = (h, 0);
    for y in h / 2..h {
        for x in 0..w {
            if fire(x, y) {
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    let unit = w as f32 / 64.0; // displacement in 64x pixels, scaled to the texture
    let tongue = w as f32 / 8.0;
    (0..tex::FURNACE_FRAMES)
        .map(|k| {
            let t = k as f32 / tex::FURNACE_FRAMES as f32 * std::f32::consts::TAU;
            let mut out = lit.clone();
            if top > bottom {
                return out;
            }
            for y in h / 2..h {
                for x in 0..w {
                    let dark = unlit.pixel(x as u32, y as u32);
                    // Only inside the opening: pixels that are fire or dark when unlit.
                    let opening =
                        fire(x, y) || (dark[0] as u32 + dark[1] as u32 + dark[2] as u32) < 210;
                    let margin = (w / 32).max(1);
                    if !opening || x < margin || x >= w - margin || y >= h - margin {
                        continue;
                    }
                    let hgt = ((bottom - y) as f32 / (bottom - top).max(1) as f32).clamp(0.0, 1.2);
                    let phase = noise1(x as f32 / tongue, 1) * std::f32::consts::TAU;
                    let sway_phase = noise1(x as f32 / tongue + 17.0, 2) * std::f32::consts::TAU;
                    let lift = (t + phase).sin() * 2.6 * unit * hgt * hgt;
                    let sway = (2.0 * t + sway_phase).sin() * 1.2 * unit * hgt * hgt;
                    let (sx, sy) = (
                        (x as f32 - sway).round() as i32,
                        (y as f32 + lift).round() as i32,
                    );
                    let px = if fire(sx, sy) {
                        lit.pixel(sx as u32, sy as u32)
                    } else {
                        dark
                    };
                    let i = ((y * w + x) * 4) as usize;
                    out.rgba[i..i + 4].copy_from_slice(&px);
                }
            }
            out
        })
        .collect()
}

/// Double chest faces from a pack's `normal_left` / `normal_right` atlases (Minecraft's
/// double chest halves: long faces 15 units wide) into the `tex::CHEST_OPEN` layers with
/// the open edge on the right (left atlas) and on the left (right atlas).
fn double_chest_faces(left: &Image, right: &Image, put: &mut impl FnMut(u32, &Image)) {
    // The seam edge has no frame: its column looks like the wood 2 units further in, while
    // a framed edge does not.
    let seam_on_right = |img: &Image| {
        let k = (img.w / 15).max(1);
        let diff = |a: u32, b: u32| -> i32 {
            (0..img.h)
                .map(|y| {
                    let (p, q) = (img.pixel(a, y), img.pixel(b, y));
                    (0..3)
                        .map(|c| (p[c] as i32 - q[c] as i32).abs())
                        .sum::<i32>()
                })
                .sum()
        };
        diff(img.w - 1, img.w - 1 - 2 * k) < diff(0, 2 * k)
    };
    let flip_h = |img: &Image| {
        let row = (img.w * 4) as usize;
        let rgba = img
            .rgba
            .chunks(row)
            .flat_map(|r| r.chunks(4).rev().flatten().copied().collect::<Vec<u8>>())
            .collect();
        Image { rgba, ..*img }
    };
    for (atlas, edge) in [(left, 0), (right, 1)] {
        let k = (atlas.w / 64).max(1);
        let want_right = edge == 0;
        let x0 = if want_right { k } else { 0 };
        let orient = |img: Image, flip: bool| if flip { flip_h(&img) } else { img };
        let face = |x: u32| {
            let body = atlas.region(64, x, 33, 15, 10).flip_v();
            let flip = seam_on_right(&body) != want_right;
            let mut f = Image::blank(16 * k, 16 * k);
            let lid = orient(atlas.region(64, x, 14, 15, 5).flip_v(), flip);
            f.blit(&lid, x0, 2 * k, 15 * k, 4 * k);
            f.blit(&orient(body, flip), x0, 6 * k, 15 * k, 10 * k);
            f
        };
        let square = |x: u32| {
            let img = atlas.region(64, x, 0, 15, 14);
            let flip = seam_on_right(&img) != want_right;
            let mut f = Image::blank(16 * k, 16 * k);
            f.blit(&orient(img, flip), x0, k, 15 * k, 14 * k);
            f
        };
        put(tex::CHEST_OPEN + edge, &face(43));
        put(tex::CHEST_OPEN + 4 + edge, &face(14));
        put(tex::CHEST_OPEN + 8 + edge, &square(29));
        put(tex::CHEST_OPEN + 12 + edge, &square(14));
    }
}

/// Replaces layers with the textures of the resource packs (anything missing stays procedural).
pub(super) fn apply_pack(pack: &Packs, base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let mut put = |layer: u32, img: &Image| {
        let mut px = img.resized(TILE);
        if !is_cutout(layer) && layer != tex::GRASS_SIDE && layer != tex::TORCH_FLAME {
            // Opaque layers: alpha is the tint mask, fully set (translucent ice becomes solid).
            // The grass side keeps its own mask; the flame layer may be cleared.
            for p in px.chunks_exact_mut(4) {
                p[3] = 255;
            }
        }
        base[layer as usize * layer_bytes..][..layer_bytes].copy_from_slice(&px);
    };
    for &(layer, paths) in PACK_TEXTURES {
        if let Some(img) = pack.texture(paths) {
            // Blocks drawn as full cubes (cactus) have transparent margins in Minecraft.
            let img = match (is_cutout(layer), img.opaque_bounds()) {
                (false, Some((x, y, w, h))) => img.crop(x, y, w, h),
                _ => img,
            };
            put(layer, &img);
        }
    }
    // Water and lava animations: the frames of the packs' strips, spread over our frames.
    for (anim, paths) in [
        (tex::WATER_ANIM, "block/water_still"),
        (tex::LAVA_ANIM, "block/lava_still"),
    ] {
        if let Some(frames) = pack.frames(paths).filter(|f| f.len() > 1) {
            let n = tex::FLUID_FRAMES as usize;
            for i in 0..n {
                put(anim + i as u32, &frames[i * frames.len() / n]);
            }
        }
    }
    for stage in 0..10 {
        if let Some(img) = pack.texture(&format!("block/destroy_stage_{stage}")) {
            put(tex::CRACK + stage, &img);
        }
    }
    let tiers = ["wooden", "stone", "iron", "golden", "diamond"];
    let kinds = ["pickaxe", "axe", "shovel", "sword"];
    for (t, tier) in tiers.iter().enumerate() {
        for (k, kind) in kinds.iter().enumerate() {
            if let Some(img) = pack.texture(&format!("item/{tier}_{kind}")) {
                put(tex::TOOLS + (t * 4 + k) as u32, &img);
            }
        }
    }

    // Grass side: the grey overlay is biome tinted (alpha 255), the dirt below is not (153).
    if let (Some(side), Some(over)) = (
        pack.texture("block/grass_block_side"),
        pack.texture("block/grass_block_side_overlay"),
    ) {
        let mut out = side.clone();
        for y in 0..side.h {
            for x in 0..side.w {
                let (ox, oy) = (x * over.w / side.w, y * over.h / side.h);
                let o = over.pixel(ox, oy);
                let i = ((y * side.w + x) * 4) as usize;
                if o[3] >= 128 {
                    out.rgba[i..i + 4].copy_from_slice(&[o[0], o[1], o[2], 255]);
                } else {
                    out.rgba[i + 3] = UNTINTED;
                }
            }
        }
        put(tex::GRASS_SIDE, &out);
    }

    // Lit furnace: animation frames made from the pack's own lit front (see furnace_frames).
    if let (Some(lit), Some(unlit)) = (
        pack.texture("block/furnace_front_on"),
        pack.texture("block/furnace_front"),
    ) {
        if (lit.w, lit.h) == (unlit.w, unlit.h) {
            for (k, frame) in furnace_frames(&lit, &unlit).iter().enumerate() {
                put(tex::FURNACE_ANIM + k as u32, frame);
            }
        }
    }

    // Torch fire: Minecraft draws flame and smoke particles over the glowing tip instead of
    // a flame model, so with a pack that has the particle the built-in flame is switched off
    // (a transparent layer is discarded by the shader) and the game spawns particles.
    if let Some(flame) = pack.texture("particle/flame") {
        put(tex::FLAME_PARTICLE, &flame);
        put(tex::TORCH_FLAME, &Image::blank(1, 1));
    }
    for i in 0..SMOKE_FRAMES {
        if let Some(img) = pack.texture(&format!("particle/generic_{i}")) {
            put(tex::SMOKE + i, &img);
        }
    }

    // 3D torch: the stick and its glowing tip, cut out of the flat torch sprite.
    if let Some((x, y, w, h)) = pack.texture("block/torch").and_then(|t| t.opaque_bounds()) {
        let torch = pack.texture("block/torch").unwrap();
        let tip = (h / 5).max(1);
        put(tex::TORCH_WOOD, &torch.crop(x, y + tip, w, h - tip));
        put(tex::TORCH_CHAR, &torch.crop(x, y, w, tip));
        put(tex::TORCH_CAP, &torch.crop(x, y, w, tip));
    }

    // Chest (entity atlas, 64 units wide; the model is stored upside down since 1.15).
    // Each layer is a full block face: lid rows 2..6, body rows 6..16, 1 unit margin.
    if let Some(atlas) = pack.texture("entity/chest/normal") {
        let k = (atlas.w / 64).max(1);
        let face = |lid_x: u32| {
            let mut f = Image::blank(16 * k, 16 * k);
            f.blit(
                &atlas.region(64, lid_x, 14, 14, 5).flip_v(),
                k,
                2 * k,
                14 * k,
                4 * k,
            );
            f.blit(
                &atlas.region(64, lid_x, 33, 14, 10).flip_v(),
                k,
                6 * k,
                14 * k,
                10 * k,
            );
            f
        };
        let square = |x: u32, y: u32| {
            let mut f = Image::blank(16 * k, 16 * k);
            f.blit(&atlas.region(64, x, y, 14, 14), k, k, 14 * k, 14 * k);
            f
        };
        put(tex::CHEST_FRONT, &face(42));
        put(tex::CHEST_SIDE, &face(0));
        put(tex::CHEST_TOP, &square(28, 0));
        put(tex::CHEST_INSIDE, &square(14, 0));
        put(tex::CHEST_LATCH, &atlas.region(64, 1, 1, 2, 4));
    }
    if let (Some(l), Some(r)) = (
        pack.texture("entity/chest/normal_left"),
        pack.texture("entity/chest/normal_right"),
    ) {
        double_chest_faces(&l, &r, &mut put);
    }

    // Water bottles: Minecraft's potion bottle with its liquid overlay tinted (lake water a
    // murky blue, boiled water clear and light).
    if let (Some(bottle), Some(over)) = (
        pack.texture("item/potion"),
        pack.texture("item/potion_overlay"),
    ) {
        for (layer, tint) in [
            (tex::WATER_BOTTLE, [0.36, 0.52, 0.62]),
            (tex::PURIFIED_WATER, [0.62, 0.86, 1.0]),
        ] {
            let mut img = bottle.clone();
            for y in 0..img.h {
                for x in 0..img.w {
                    let o = over.pixel(x * over.w / img.w, y * over.h / img.h);
                    if o[3] >= 128 {
                        let i = ((y * img.w + x) * 4) as usize;
                        for c in 0..3 {
                            img.rgba[i + c] = (o[c] as f32 * tint[c]) as u8;
                        }
                        img.rgba[i + 3] = 255;
                    }
                }
            }
            put(layer, &img);
        }
    }

    // Pig and sheep skins: the whole atlas, kept square (64x32 ones fill the top half).
    for (layer, paths) in [
        (tex::PIG, "entity/pig/pig_temperate|entity/pig/pig"),
        (tex::SHEEP, "entity/sheep/sheep"),
        (tex::SHEEP_WOOL, "entity/sheep/sheep_wool|entity/sheep/sheep_fur"),
    ] {
        if let Some(skin) = pack.texture(paths) {
            let mut atlas = Image::blank(skin.w, skin.w);
            atlas.blit(&skin, 0, 0, skin.w, skin.h.min(skin.w));
            put(layer, &atlas);
        }
    }

    // Player skin (64 unit atlas): each layer is one face of a body part.
    if let Some(skin) = pack.texture("entity/player/wide/steve|entity/steve")
    {
        let parts: [(u32, [u32; 4]); 11] = [
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
        for (layer, [x, y, w, h]) in parts {
            put(layer, &skin.region(64, x, y, w, h));
        }
    }
}
