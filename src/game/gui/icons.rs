//! Item icons for things as they are: a magazine or speedloader shows the rounds in it, a gun
//! its attachments, its magazine (or none), its slide held back and its dirt, a part its dirt
//! and what is fitted on it. The fixed icons (`world::textures::render_item_icons`) show each
//! item one way; one in another state gets its own icon, drawn from its 3D model the same way
//! into one of `tex::STATE_ICON_COUNT` texture layers kept for this, the ones not seen for
//! longest drawn over first. An icon asked for is drawn the next frame (until then the fixed
//! one shows).

use crate::game::*;
use crate::item::*;
use crate::world::textures::{tex, TILE};
use std::cell::RefCell;

/// What an icon shows of a stack: its item and the state that changes how it looks.
type Key = (ItemId, u16);

#[derive(Default)]
struct Cache {
    /// Each layer's icon and the frame it was last shown.
    slots: Vec<Option<(Key, u64)>>,
    /// Icons asked for that are not drawn yet (with a stack in that state).
    wanted: Vec<(Key, Stack)>,
    frame: u64,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// How dirty something looks (as drawn).
fn dirt(st: &Stack) -> u16 {
    crate::model::pistol_view::dirt_level(st.damage, max_damage(st.item)) as u16
}

/// The state of a stack its icon shows, when that is not the fixed icon's (None: the fixed
/// icon is it, or the item has no model to draw).
fn key(st: &Stack) -> Option<Key> {
    let item = st.item;
    let k = match item {
        PISTOL | AK47 => {
            let full = (gun_mods(st) as u16) | (!gun_has_mag(st) as u16) << 8 | (gun_locked(st) as u16) << 9;
            full | dirt(st) << 12
        }
        REVOLVER => dirt(st) << 12,
        PISTOL_MAGAZINE | EXTENDED_MAGAZINE | SPEEDLOADER | AK_MAGAZINE => {
            let cap = magazine_capacity(item).unwrap_or(0);
            // The fixed icons are full.
            if gun_rounds(st) >= cap && dirt(st) == 0 {
                return None;
            }
            gun_rounds(st) as u16 | dirt(st) << 12 | 0x800
        }
        _ if (PISTOL_FRAME..=PISTOL_SLIDE).contains(&item) || REVOLVER_PARTS.contains(&item) || AK_PARTS[..4].contains(&item) => {
            (gun_mods(st) as u16) | dirt(st) << 12
        }
        _ => return None,
    };
    // The fixed icons: the pistol and the AK loaded and clean, everything else clean and bare.
    (k != 0).then_some((item, k))
}

/// The texture layer of an icon of `st` as it is, when it has been drawn (asked for
/// otherwise, and None: the fixed icon shows meanwhile).
pub fn state_icon(st: &Stack) -> Option<u32> {
    let k = key(st)?;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let frame = c.frame;
        if let Some(i) = c.slots.iter().position(|s| s.is_some_and(|s| s.0 == k)) {
            if let Some(s) = &mut c.slots[i] {
                s.1 = frame;
            }
            return Some(tex::STATE_ICONS + i as u32);
        }
        if !c.wanted.iter().any(|w| w.0 == k) {
            c.wanted.push((k, *st));
        }
        None
    })
}

impl Game {
    /// Draws an icon asked for (one a frame), into a layer not in use or not seen for
    /// longest.
    pub(in crate::game) fn update_state_icons(&mut self) {
        let wanted: Vec<(Key, Stack)> = CACHE.with(|c| {
            let mut c = c.borrow_mut();
            c.frame += 1;
            if c.slots.is_empty() {
                c.slots = vec![None; tex::STATE_ICON_COUNT as usize];
            }
            // One a frame (each takes a few milliseconds).
            let n = c.wanted.len().min(1);
            c.wanted.drain(..n).collect()
        });
        for (k, st) in wanted {
            let img = crate::world::textures::render_icon(&self.texture_base, &Stack { count: 1, ..st });
            let px: Vec<[u8; 4]> = img.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect();
            let levels = crate::game::book::sheet_levels(&px, TILE, 1, 1);
            let slot = CACHE.with(|c| {
                let mut c = c.borrow_mut();
                let frame = c.frame;
                // A free layer, or the one not seen for longest (not one shown just now).
                let i = c
                    .slots
                    .iter()
                    .position(|s| s.is_none())
                    .or_else(|| {
                        c.slots
                            .iter()
                            .enumerate()
                            .filter(|(_, s)| s.is_some_and(|s| s.1 + 1 < frame))
                            .min_by_key(|(_, s)| s.map_or(0, |s| s.1))
                            .map(|(i, _)| i)
                    })?;
                c.slots[i] = Some((k, frame));
                Some(i)
            });
            if let Some(i) = slot {
                self.renderer.queue_layers(tex::STATE_ICONS + i as u32, 1, levels);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_shows_the_state_that_changes_the_look() {
        // The fixed icons need none of their own.
        let mut full = Stack::one(PISTOL_MAGAZINE);
        set_gun_rounds(&mut full, 12);
        assert_eq!(key(&full), None);
        assert_eq!(key(&Stack::one(PISTOL)), None);
        assert_eq!(key(&Stack::one(BULLET)), None);
        // A half-full magazine, an empty speedloader, a scoped pistol, a dirty revolver do.
        let mut half = full;
        set_gun_rounds(&mut half, 6);
        let mut loader = Stack::one(SPEEDLOADER);
        set_gun_rounds(&mut loader, 0);
        let mut scoped = Stack::one(PISTOL);
        set_gun_mods(&mut scoped, gun_mod::SCOPE);
        let dirty = Stack { damage: 150, ..Stack::one(REVOLVER) };
        let keys = [key(&half), key(&loader), key(&scoped), key(&dirty)];
        assert!(keys.iter().all(|k| k.is_some()));
        // Different states, different icons; the same state (other rounds in a gun, which do
        // not show), the same icon.
        assert_ne!(key(&half), key(&Stack { data: 3, ..half }));
        let mut other = scoped;
        set_gun_rounds(&mut other, 3);
        assert_eq!(key(&scoped), key(&other));
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    #[test]
    #[ignore]
    fn how_long_things_take() {
        let base = crate::world::textures::generate_base(&crate::pack::Packs(Vec::new()));
        let mut st = Stack::one(PISTOL);
        set_gun_mods(&mut st, gun_mod::SCOPE | gun_mod::SILENCER);
        let t = std::time::Instant::now();
        for _ in 0..5 {
            let _ = crate::world::textures::render_icon(&base, &st);
        }
        println!("icon: {:?} each", t.elapsed() / 5);
        let t = std::time::Instant::now();
        let mut out = Vec::new();
        for _ in 0..10 {
            out.clear();
            crate::model::gun_station::emit_ammo_box(&mut out, glam::Mat4::IDENTITY, 128 | BOX_MAGNUM, [255; 4], 0);
        }
        println!("full magnum box: {:?} each, {} vertices", t.elapsed() / 10, out.len());
    }
}
