//! Block, item and entity textures: one 128x128 layer each in a texture array (`tex` names
//! the layers). Every layer is drawn procedurally first (`procedural`), then replaced by the
//! resource packs' texture where they have one (`from_pack`; the packs themselves, zip files
//! or folders, are read by `resource_pack`); the layers made from others follow (`synth`,
//! `icons`, `logo`), and the mipmaps (`mips`) and the item sprite masks (`masks`)
//! are made from the result. `skin_pages` lays the detailed mob skins out over several
//! layers (used by `from_pack` and by the mob models).

mod from_pack;
mod icons;
mod logo;
mod masks;
mod mips;
mod procedural;
pub mod resource_pack;
pub mod skin_pages;
mod synth;
pub mod tex;

use from_pack::apply_pack;
use icons::render_item_icons;
use masks::update_item_masks;
use mips::{is_cutout, mip_chain};
use procedural::{crack_pattern, pixel};
use resource_pack::{Image, Packs};
use synth::*;

pub use icons::render_icon;
pub use logo::{logo_layers, logo_levels};
pub use masks::{ITEM_MASKS, ITEM_MASKS_VERSION, MASK};
pub use tex::tool_layer;

pub const TILE: usize = 128;
/// Alpha of opaque texels that are not biome tinted (255 = fully tinted).
const UNTINTED: u8 = 153;

/// Smoke particle sprites, like Minecraft's generic_0..7.
pub const SMOKE_FRAMES: u32 = 8;

/// The full mip chain.
#[cfg(test)]
pub fn generate(packs: &Packs) -> Vec<Vec<u8>> {
    levels(&generate_base(packs))
}

/// A layer of the player's skin `skin`. Every player wears the game's own skin for now (the
/// skin choice comes later; `PlayerPose::skin` and the network's keep its place).
pub fn skin_layer(layer: u32, _skin: u8) -> u32 {
    layer
}

/// How far the textures being made have got (0..1): the layers drawn, then the rest.
pub static PROGRESS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn set_progress(p: f32) {
    PROGRESS.store(p.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn progress() -> f32 {
    f32::from_bits(PROGRESS.load(std::sync::atomic::Ordering::Relaxed))
}

/// Every layer at full size: the procedural textures, replaced by the resource packs' where
/// they have them, and the layers made from those. This is the slow part; `levels` finishes
/// it.
pub fn generate_base(packs: &Packs) -> Vec<u8> {
    let crack = crack_pattern();
    let layers = tex::LAYERS;
    let layer_bytes = TILE * TILE * 4;
    let mut base = vec![0u8; layer_bytes * layers];
    set_progress(0.0);
    // The packs' layers first: those the packs give are not made procedurally (the built-in
    // pack gives about 150 of them).
    let given = apply_pack(packs, &mut base);
    procedural_layers(&mut base, &given, &crack);
    set_progress(0.8);
    finish_base(base)
}

/// Every layer not `given` (by the packs), made procedurally, in parallel (the layers are
/// independent of each other).
fn procedural_layers(base: &mut [u8], given: &[bool], crack: &[u16]) {
    let layer_bytes = TILE * TILE * 4;
    let done = std::sync::atomic::AtomicUsize::new(0);
    let todo = given.iter().filter(|g| !**g).count().max(1);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);
    std::thread::scope(|scope| {
        let crack = &crack;
        let done = &done;
        let mut chunks: Vec<(usize, &mut [u8])> =
            base.chunks_mut(layer_bytes).enumerate().collect();
        let per = chunks.len().div_ceil(threads);
        while !chunks.is_empty() {
            let batch: Vec<(usize, &mut [u8])> = chunks.drain(..per.min(chunks.len())).collect();
            scope.spawn(move || {
                for (l, out) in batch {
                    // The double chest faces are filled in later.
                    if (tex::CHEST_OPEN..tex::DOOR_TOP).contains(&(l as u32)) || given[l] {
                        continue;
                    }
                    for y in 0..TILE {
                        for x in 0..TILE {
                            let i = (y * TILE + x) * 4;
                            out[i..i + 4]
                                .copy_from_slice(&pixel(l as u32, x as i32, y as i32, crack));
                        }
                    }
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    set_progress(0.75 * n as f32 / todo as f32);
                }
            });
        }
    });
}

/// `generate_base` after the pack and procedural layers: the synthesized ones.
fn finish_base(mut base: Vec<u8>) -> Vec<u8> {
    let layer_bytes = TILE * TILE * 4;
    use crate::model::blockbench::{ak_vm, gun_station, pistol_vm, revolver_vm, rifle_station};
    synth_model_pages(&mut base, pistol_vm::PNG, pistol_vm::PAGES, tex::PISTOL_VIEW);
    synth_model_pages(&mut base, revolver_vm::PNG, revolver_vm::PAGES, tex::REVOLVER_VIEW);
    synth_model_pages(&mut base, ak_vm::PNG, ak_vm::PAGES, tex::AK_VIEW);
    for level in 1..tex::PISTOL_DIRT_LEVELS {
        synth_grime(&mut base, tex::PISTOL_VIEW, pistol_vm::PAGES, level);
        synth_grime(&mut base, tex::REVOLVER_VIEW, revolver_vm::PAGES, level);
        synth_grime(&mut base, tex::AK_VIEW, ak_vm::PAGES, level);
    }
    synth_model_pages(&mut base, gun_station::PNG, gun_station::PAGES, tex::GUN_STATION_MODEL);
    synth_model_pages(&mut base, rifle_station::PNG, rifle_station::PAGES, tex::RIFLE_STATION_MODEL);
    synth_model_pages(&mut base, crate::model::blockbench::grenade::PNG, crate::model::blockbench::grenade::PAGES, tex::GRENADE_MODEL);
    synth_model_pages(&mut base, crate::model::blockbench::dummy::PNG, crate::model::blockbench::dummy::PAGES, tex::DUMMY_MODEL);
    synth_model_pages(&mut base, crate::model::blockbench::fishing_rod::PNG, crate::model::blockbench::fishing_rod::PAGES, tex::FISHING_ROD_MODEL);
    render_item_icons(&mut base);
    let logo = logo_layers();
    let at = tex::LOGO as usize * layer_bytes;
    base[at..at + logo.len()].copy_from_slice(&logo);
    synth_doors(&mut base);
    synth_stump_marks(&mut base);
    synth_grilled(&mut base);
    mark_materials(&mut base);
    synth_glow(&mut base);
    for (front, cut) in [
        (tex::FURNACE_FRONT, tex::FURNACE_FRONT_CUT),
        (tex::BLAST_FRONT, tex::BLAST_FRONT_CUT),
        (tex::ADV_FRONT, tex::ADV_FRONT_CUT),
    ] {
        synth_furnace_cut(&mut base, front, cut);
    }
    synth_furnace_inside(&mut base);
    synth_fluid_frames(&mut base);
    synth_double_chest(&mut base);
    base
}

/// The full mip chain of `generate_base`'s layers (each level holds all layers back to
/// back); the item sprite masks are made from them too.
pub fn levels(base: &[u8]) -> Vec<Vec<u8>> {
    update_item_masks(base);
    mip_chain(base.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_a_pack_gives_are_not_made_first() {
        // The built-in pack over everything made procedurally, as it used to be done, and
        // the pack's layers first with the rest made: the same.
        let packs = Packs::load(&[]);
        let fast = generate_base(&packs);
        let mut slow = vec![0u8; TILE * TILE * 4 * tex::LAYERS];
        procedural_layers(&mut slow, &vec![false; tex::LAYERS], &crack_pattern());
        apply_pack(&packs, &mut slow);
        assert!(fast == finish_base(slow));
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

    /// The layer numbers `world.frag` keeps its own copies of are the game's.
    #[test]
    fn shader_layer_numbers_match() {
        let src = include_str!("../../shaders/world.frag");
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

    /// Generates every layer; with TEX_DUMP=<file.bmp> also writes a contact sheet to look at.
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
