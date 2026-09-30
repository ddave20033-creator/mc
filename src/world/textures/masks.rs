//! The opaque-pixel masks of the layers, for extruding flat item sprites into 3D models.

use super::*;

/// Resolution of the opaque-pixel masks used to extrude flat item sprites into 3D models: the
/// texture's own, so every side wall samples the middle of exactly one texel.
pub const MASK: usize = TILE;

/// Opaque pixels of every layer at MASK x MASK (one row per u128, bit x = column x, row 0 at the
/// top of the texture). Filled by `generate`, so it follows the active resource pack.
pub static ITEM_MASKS: std::sync::RwLock<Vec<[u128; MASK]>> = std::sync::RwLock::new(Vec::new());
/// Counts the times `ITEM_MASKS` was made anew (what is worked out from it is then stale).
pub static ITEM_MASKS_VERSION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Makes `ITEM_MASKS` anew from the finished full-size layers.
pub(super) fn update_item_masks(base: &[u8]) {
    if let Ok(mut masks) = ITEM_MASKS.write() {
        *masks = opaque_masks(&base);
        ITEM_MASKS_VERSION.fetch_add(1, std::sync::atomic::Ordering::Release);
    }
}

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
