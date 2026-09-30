//! The player's skins: the clothing layers of each outfit, the four built-in outfits and
//! the uploaded skins (a standard Minecraft skin PNG cut into the character's layers, one
//! set per player slot).

use super::mips::mip_chain;
use super::*;

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

/// The extra outfits, made from the final texture set, including resource packs. Keep
/// luminance/detail while changing cloth colour; face and exposed hands remain intact.
pub(super) fn synth_outfits(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
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
}

/// Only one player slot's skin layers with their mips, to replace in the textures already
/// made (a player joining or changing skin): the first layer, how many, and the levels.
pub fn skin_slot_levels(base: &[u8], slot: u8, skin: Option<&Image>) -> (u32, u32, Vec<Vec<u8>>) {
    let (first, layers) = skin_slot_layers(base, slot, skin);
    (first, tex::CUSTOM_SKIN_LAYERS, mip_chain(layers, first))
}

/// A player slot's skin layers (back to back, full size): cut from the uploaded skin, or the
/// default skin's (from `base`) without one. Returns its first layer too.
pub(super) fn skin_slot_layers(base: &[u8], slot: u8, skin: Option<&Image>) -> (u32, Vec<u8>) {
    let layer_bytes = TILE * TILE * 4;
    let first = tex::CUSTOM_SKIN_START + slot as u32 * tex::CUSTOM_SKIN_LAYERS;
    let mut out = Vec::with_capacity(SKIN_PARTS.len() * layer_bytes);
    for &(source, region) in SKIN_PARTS.iter() {
        if let Some(atlas) = skin {
            let mut pixels = atlas.region(64, region[0], region[1], region[2], region[3]).resized(TILE);
            for p in pixels.chunks_exact_mut(4) {
                p[3] = 255;
            }
            out.extend_from_slice(&pixels);
        } else {
            let src = source as usize * layer_bytes;
            out.extend_from_slice(&base[src..src + layer_bytes]);
        }
    }
    (first, out)
}
