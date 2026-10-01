//! The game's logo as texture layers, and on its own with its mipmaps for the start-up
//! screen.

use super::*;

/// The logo (`textures/logo.png`, a row of `tex::LOGO_TILES` squares) as that many texture layers.
pub fn logo_layers() -> Vec<u8> {
    let layer_bytes = TILE * TILE * 4;
    let tiles = tex::LOGO_TILES as usize;
    let mut out = vec![0u8; layer_bytes * tiles];
    let Some(img) = crate::textures::resource_pack::decode_png(include_bytes!("logo.png")) else {
        return out;
    };
    if img.w as usize != TILE * tiles || img.h as usize != TILE {
        return out;
    }
    for t in 0..tiles {
        for y in 0..TILE {
            let src = (y * TILE * tiles + t * TILE) * 4;
            let dst = t * layer_bytes + y * TILE * 4;
            out[dst..dst + TILE * 4].copy_from_slice(&img.rgba[src..src + TILE * 4]);
        }
    }
    out
}

/// Just the logo's layers (the first `tex::LOGO_TILES`) with their mipmaps: all the
/// textures the start-up screen needs while the rest are made.
pub fn logo_levels() -> Vec<Vec<u8>> {
    let tiles = tex::LOGO_TILES as usize;
    let mut levels = vec![logo_layers()];
    let mut size = TILE;
    while size > 1 {
        let prev = levels.last().unwrap();
        let ns = size / 2;
        let mut next = vec![0u8; ns * ns * 4 * tiles];
        for l in 0..tiles {
            for y in 0..ns {
                for x in 0..ns {
                    let (mut rgb, mut a) = ([0.0f32; 3], 0.0f32);
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let i = ((l * size + y * 2 + dy) * size + x * 2 + dx) * 4;
                        let w = prev[i + 3] as f32 / 255.0;
                        for c in 0..3 {
                            rgb[c] += prev[i + c] as f32 * w;
                        }
                        a += w;
                    }
                    let o = ((l * ns + y) * ns + x) * 4;
                    for c in 0..3 {
                        next[o + c] = if a > 0.0 { (rgb[c] / a) as u8 } else { 0 };
                    }
                    next[o + 3] = (a / 4.0 * 255.0).round() as u8;
                }
            }
        }
        levels.push(next);
        size = ns;
    }
    levels
}
