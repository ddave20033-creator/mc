//! What the server believes of a player: the item ids they send are items there are, the
//! damage they deal is what their weapon can deal, and what they do is where they stand.
//! Cheap sanity checks for a LAN game (a player still runs their own inventory); an honest
//! player never runs into them.

use crate::item::{attack_damage, key, GunKind, ItemId, Slot, Stack, GUN_KINDS, NONE};
use glam::{IVec3, Vec3};
use std::sync::OnceLock;

/// Item ids at or above this are no items (the highest is a few hundred).
const ITEM_LIMIT: usize = 4096;

/// How far from where a player stands (their last pose) they may change a block, open or use
/// one: the reach (5), a structure's other blocks (a rifle station is three wide) and some
/// room for the pose being a moment old.
pub(crate) const BLOCK_REACH: f32 = 10.0;
/// How far from where a player stands their shot, grenade or dropped item may start.
pub(crate) const HAND_REACH: f32 = 5.0;
/// How far from where a player stands a spawn egg may put a mob (without cheats).
pub(crate) const SPAWN_REACH: f32 = 10.0;
/// A critical hit (falling): 1.5 times the weapon's damage.
const CRIT: f32 = 1.5;
/// Knockback of a sprinting hit.
const MELEE_KNOCK: f32 = 2.0;

/// Whether `id` is an item (or nothing, 0): one that the item table knows.
pub(crate) fn known_item(id: ItemId) -> bool {
    static KNOWN: OnceLock<Vec<bool>> = OnceLock::new();
    let known = KNOWN.get_or_init(|| (0..ITEM_LIMIT).map(|i| key(i as ItemId) != "unknown").collect());
    id == NONE || known.get(id as usize).copied().unwrap_or(false)
}

pub(crate) fn valid_stack(s: &Stack) -> bool {
    s.count > 0 && s.item != NONE && known_item(s.item)
}

pub(crate) fn valid_slot(s: &Slot) -> bool {
    s.as_ref().is_none_or(valid_stack)
}

/// The most a hit with `held` in the hand does (a critical one).
pub(crate) fn melee_cap(held: ItemId) -> f32 {
    attack_damage(held) * CRIT
}

/// The most a bullet of the gun `held` does (0 if it is no gun).
pub(crate) fn gun_cap(held: ItemId) -> f32 {
    GunKind::of(held).map_or(0.0, |k| k.stats().damage)
}

/// The most knockback any hit may carry (a sprinting hit, a gun's bullet).
pub(crate) fn knock_cap() -> f32 {
    GUN_KINDS.iter().map(|k| k.stats().knockback).fold(MELEE_KNOCK, f32::max)
}

/// `dmg` and `knock` as the server takes them: at most `cap` and `knock_cap`, never negative;
/// None if there is nothing to deal.
pub(crate) fn clamp_hit(dmg: f32, knock: f32, cap: f32) -> Option<(f32, f32)> {
    let dmg = dmg.min(cap);
    (dmg > 0.0).then(|| (dmg, knock.clamp(0.0, knock_cap())))
}

/// Whether the block at `p` is within `reach` of a player standing at `feet`.
pub(crate) fn block_near(feet: Vec3, p: IVec3, reach: f32) -> bool {
    let eye = feet + Vec3::Y * 1.62;
    (p.as_vec3() + Vec3::splat(0.5)).distance(eye) <= reach
}

/// Whether `at` is within `reach` of a player standing at `feet` (from their middle).
pub(crate) fn point_near(feet: Vec3, at: Vec3, reach: f32) -> bool {
    (feet + Vec3::Y * 0.9).distance(at) <= reach
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{all_items, AMMO_BOX};

    #[test]
    fn every_item_is_known() {
        for id in all_items().into_iter().chain([AMMO_BOX]) {
            assert!((id as usize) < ITEM_LIMIT, "item {id} over the limit");
            assert!(known_item(id), "item {id} not known");
        }
        assert!(known_item(NONE));
        assert!(!known_item(u16::MAX));
        assert!(!known_item(ITEM_LIMIT as ItemId));
        assert!(!valid_stack(&Stack::new(u16::MAX, 1)));
        assert!(!valid_stack(&Stack { count: 0, ..Stack::one(all_items()[0]) }));
        assert!(valid_slot(&None));
    }

    #[test]
    fn honest_hits_pass_and_others_are_cut() {
        // A falling critical hit with the best sword is what it is.
        let best = all_items().into_iter().map(melee_cap).fold(0.0, f32::max);
        let sword = all_items().into_iter().find(|&i| melee_cap(i) == best).unwrap();
        assert_eq!(clamp_hit(best, 2.0, melee_cap(sword)), Some((best, 2.0)));
        // The bare hand cannot do a sword's damage.
        assert_eq!(clamp_hit(100.0, 50.0, melee_cap(NONE)), Some((1.5, knock_cap())));
        assert_eq!(clamp_hit(-3.0, 1.0, 10.0), None);
        for k in GUN_KINDS {
            let d = k.stats().damage;
            assert_eq!(clamp_hit(d, k.stats().knockback, gun_cap(k.item())), Some((d, k.stats().knockback)));
        }
        assert_eq!(gun_cap(NONE), 0.0);
    }

    #[test]
    fn reach() {
        let feet = Vec3::new(0.5, 64.0, 0.5);
        assert!(block_near(feet, IVec3::new(4, 65, 0), BLOCK_REACH));
        assert!(!block_near(feet, IVec3::new(40, 65, 0), BLOCK_REACH));
        assert!(point_near(feet, feet + Vec3::Y * 1.6, HAND_REACH));
        assert!(!point_near(feet, feet + Vec3::X * 20.0, HAND_REACH));
    }
}
