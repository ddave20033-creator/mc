//! Ammunition: each gun's rounds, its magazines (the pistol's standard and extended ones, the
//! AK's), the revolver's speedloader, and what belongs to a gun station's drawer: the boxes of
//! rounds and the magazine loader.

use super::*;

/// A gun's round: a row of the tools' tab with the gun.
const fn round(icon: u32, group: u8) -> ItemDef {
    ItemDef { icon: Icon::Flat(icon), creative: Creative::Tools(group), ..ITEM }
}

/// A magazine (its data is the rounds in it), for a gun (`GunRole::Magazine`) or a loader:
/// held, it does nothing (it is loaded at the gun station).
const fn magazine(gun: GunRole, icon: u32, group: u8) -> ItemDef {
    ItemDef { icon: Icon::Flat(icon), gun, on_use: OnUse::Nothing, creative: Creative::Tools(group), ..SINGLE }
}

items! {
    after guns::END;

    /// Pistol ammunition (9 mm): one is used up per shot.
    BULLET = ItemDef { key: "bullet", en: "Bullet", hu: "Töltény", ..round(tex::BULLET, 5) };
    /// Revolver ammunition (.357 Magnum): longer and heavier than the pistol's 9 mm, which does
    /// not fit the revolver (nor this the pistol).
    MAGNUM_ROUND = ItemDef { key: "magnum_round", en: "Magnum Round", hu: "Magnum töltény", ..round(tex::MAGNUM_ROUND, 5) };
    /// Rifle ammunition (7.62x39 mm): only for the AK.
    RIFLE_ROUND = ItemDef { key: "rifle_round", en: "7.62 Round", hu: "7,62-es töltény", ..round(tex::RIFLE_ROUND, 6) };

    /// The pistol's magazines: the standard one (12 rounds; the pistol's last part) and the
    /// extended one (20).
    PISTOL_MAGAZINE = ItemDef {
        key: "pistol_magazine", en: "Pistol Magazine", hu: "Pisztolytár",
        ..magazine(GunRole::Magazine(GunKind::Pistol), tex::PISTOL_PARTS + 4, 9)
    };
    EXTENDED_MAGAZINE = ItemDef {
        key: "extended_magazine", en: "Extended Magazine", hu: "Bővített tár",
        ..magazine(GunRole::Magazine(GunKind::Pistol), tex::GUN_ATTACHMENTS + 2, 5)
    };
    /// A speedloader: six rounds held in a ring, to load a revolver's cylinder at once. Its
    /// `data` is the rounds in it (loaded at the gun station, like a magazine).
    SPEEDLOADER = ItemDef {
        key: "speedloader", en: "Speedloader", hu: "Gyorstöltő",
        ..magazine(GunRole::Loader(GunKind::Revolver), tex::SPEEDLOADER, 5)
    };
    /// The AK's curved 30-round magazine (its last part).
    AK_MAGAZINE = ItemDef {
        key: "ak_magazine", en: "AK Magazine", hu: "AK-tár", ..magazine(GunRole::Magazine(GunKind::Ak), tex::AK_PARTS + 4, 6)
    };

    /// An automatic magazine loader for the rifle station's drawer: a magazine put on it is
    /// filled from the boxes of rounds beside it, one round after another.
    MAG_LOADER = ItemDef {
        key: "magazine_loader", en: "Magazine Loader", hu: "Tárazógép", icon: Icon::Flat(tex::MAG_LOADER),
        creative: Creative::Tools(6), ..SINGLE
    };
    /// A box of rounds (an ammo can): its `data` is the rounds in it, up to `AMMO_BOX_ROUNDS`
    /// (`box_rounds`). It belongs to a gun station (three in its drawer, where magazines are
    /// loaded): taken out onto its table and put back, never into an inventory, and not made
    /// (nor listed).
    AMMO_BOX = ItemDef {
        key: "ammo_box", en: "Ammo Box", hu: "Töltényes doboz", icon: Icon::Flat(tex::AMMO_BOX), ..SINGLE
    };
}

pub const AMMO_BOX_ROUNDS: u16 = 128;

/// A box of rounds holds one kind: 9 mm bullets, magnum rounds or rifle rounds (these bits of
/// its `data`, and of the value kept for a box in a gun station's drawer); the rest is how
/// many. Once rounds are in it only that kind goes in, until it is empty again.
pub const BOX_MAGNUM: u16 = 0x8000;
pub const BOX_RIFLE: u16 = 0x4000;
/// The bits that say what kind of rounds a box holds.
pub const BOX_KIND: u16 = BOX_MAGNUM | BOX_RIFLE;

/// The rounds that go into a box.
pub const BOX_AMMO: [ItemId; 3] = [BULLET, MAGNUM_ROUND, RIFLE_ROUND];

/// Rounds in a box of them.
pub fn box_rounds(st: &crate::item::Stack) -> u16 {
    box_count(st.data)
}

/// Rounds in a box (its `data`, or a drawer's box).
pub fn box_count(v: u16) -> u16 {
    (v & !BOX_KIND).min(AMMO_BOX_ROUNDS)
}

/// What kind of round a box holds (None: it is empty).
pub fn box_ammo(v: u16) -> Option<ItemId> {
    match (box_count(v), v & BOX_KIND) {
        (0, _) => None,
        (_, BOX_MAGNUM) => Some(MAGNUM_ROUND),
        (_, BOX_RIFLE) => Some(RIFLE_ROUND),
        _ => Some(BULLET),
    }
}

/// How many rounds of `item` go into a box still (none of another kind than it holds).
pub fn box_room(v: u16, item: ItemId) -> u16 {
    let fits = BOX_AMMO.contains(&item) && box_ammo(v).is_none_or(|a| a == item);
    if fits {
        AMMO_BOX_ROUNDS - box_count(v)
    } else {
        0
    }
}

/// A box with `n` more rounds of `item` in it.
pub fn box_with(v: u16, item: ItemId, n: u16) -> u16 {
    let kind = match item {
        MAGNUM_ROUND => BOX_MAGNUM,
        RIFLE_ROUND => BOX_RIFLE,
        _ => 0,
    };
    (box_count(v) + n).min(AMMO_BOX_ROUNDS) | kind
}

/// A box with `n` fewer rounds in it (empty, it takes either kind again).
pub fn box_without(v: u16, n: u16) -> u16 {
    let left = box_count(v).saturating_sub(n);
    if left == 0 {
        0
    } else {
        left | (v & BOX_KIND)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_holds_one_kind_of_round_until_it_is_empty() {
        let v = box_with(0, MAGNUM_ROUND, 5);
        assert_eq!((box_count(v), box_ammo(v)), (5, Some(MAGNUM_ROUND)));
        // The other kind does not go in; more of its own does, up to the top.
        assert_eq!(box_room(v, BULLET), 0);
        assert_eq!(box_room(v, MAGNUM_ROUND), AMMO_BOX_ROUNDS - 5);
        assert_eq!(box_room(v, PISTOL), 0);
        // Emptied, it takes either again.
        let empty = box_without(v, 5);
        assert_eq!(box_ammo(empty), None);
        assert!(box_room(empty, BULLET) == AMMO_BOX_ROUNDS && box_room(empty, MAGNUM_ROUND) == AMMO_BOX_ROUNDS);
        // Old boxes (a count only) are 9 mm.
        assert_eq!(box_ammo(40), Some(BULLET));
        // Each gun's round goes into a box.
        for k in GUN_KINDS {
            assert!(BOX_AMMO.contains(&k.ammo()));
        }
    }
}
