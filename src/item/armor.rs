//! Armor: helmets, chestplates, leggings and boots of wool, copper, steel (from the blast
//! furnace) and diamond, and the bulletproof vest worn over them (steel and ceramic plates
//! from the advanced furnace). What each piece protects and how long it lasts, and the worn
//! set packed into a number for the player model (and the other players).

use super::*;

/// The sixteen pieces, `ARMOR_BASE + material * 4 + piece`, and the vest.
pub const ARMOR_BASE: ItemId = 1880;
pub const BULLETPROOF_VEST: ItemId = 1128;
/// Slots worn in: helmet, chestplate, leggings, boots, and the vest over the chestplate.
pub const ARMOR_SLOTS: usize = 5;
pub const VEST_SLOT: usize = 4;
/// Materials, weakest first.
pub const MATERIALS: usize = 4;

pub fn armor_id(material: usize, piece: usize) -> ItemId {
    ARMOR_BASE + (material * 4 + piece) as ItemId
}

/// The slot a piece is worn in and its material (the vest: its own slot, material 0).
pub fn armor_of(id: ItemId) -> Option<(usize, usize)> {
    if id == BULLETPROOF_VEST {
        return Some((VEST_SLOT, 0));
    }
    (ARMOR_BASE..ARMOR_BASE + 16).contains(&id).then(|| {
        let i = (id - ARMOR_BASE) as usize;
        (i % 4, i / 4)
    })
}

/// What a material's pieces are made of.
pub fn material_item(material: usize) -> ItemId {
    [WOOL as ItemId, COPPER_INGOT, STEEL_INGOT, DIAMOND][material]
}

/// Armor points (Minecraft's): each takes 4% off the damage, 20 at most.
pub fn armor_points(id: ItemId) -> u32 {
    match armor_of(id) {
        Some((VEST_SLOT, _)) | None => 0,
        Some((piece, m)) => [[1, 3, 2, 1], [2, 5, 4, 1], [2, 6, 5, 2], [3, 8, 6, 3]][m][piece],
    }
}

/// Hits a piece takes before it breaks.
pub fn armor_durability(id: ItemId) -> u16 {
    match armor_of(id) {
        Some((VEST_SLOT, _)) => 90,
        Some((piece, m)) => [11, 16, 15, 13][piece] * [4, 10, 17, 33][m],
        None => 0,
    }
}

/// How much of a hit gets through the worn set (`bullet`: shot, `blast`: a grenade): the
/// armor points take a share of everything, the vest most of a bullet and some of a blast.
pub fn armor_factor(worn: &[Slot; ARMOR_SLOTS], bullet: bool, blast: bool) -> f32 {
    let points: u32 = worn[..4].iter().flatten().map(|s| armor_points(s.item)).sum();
    let mut k = 1.0 - points.min(20) as f32 / 25.0;
    if worn[VEST_SLOT].is_some() {
        if bullet {
            k *= 0.45;
        } else if blast {
            k *= 0.75;
        }
    }
    k
}

/// The worn set packed for the model: three bits a piece (its material + 1, 0 if none) and
/// a bit for the vest.
pub fn armor_code(worn: &[Slot; ARMOR_SLOTS]) -> u16 {
    let mut code = 0;
    for (slot, s) in worn.iter().enumerate() {
        if let Some((piece, m)) = s.and_then(|s| armor_of(s.item)) {
            if piece == VEST_SLOT {
                code |= 1 << 12;
            } else if piece == slot {
                code |= ((m as u16) + 1) << (slot * 3);
            }
        }
    }
    code
}

/// The material worn in each of the four slots, and whether the vest is on.
pub fn unpack_armor(code: u16) -> ([Option<usize>; 4], bool) {
    let pieces = std::array::from_fn(|slot| {
        let v = (code >> (slot * 3)) & 7;
        (v > 0).then(|| (v - 1) as usize)
    });
    (pieces, code & (1 << 12) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worn_set_packs_and_protects() {
        let mut worn: [Slot; ARMOR_SLOTS] = [None; ARMOR_SLOTS];
        worn[0] = Some(Stack::one(armor_id(2, 0)));
        worn[3] = Some(Stack::one(armor_id(3, 3)));
        worn[VEST_SLOT] = Some(Stack::one(BULLETPROOF_VEST));
        let (pieces, vest) = unpack_armor(armor_code(&worn));
        assert_eq!(pieces, [Some(2), None, None, Some(3)]);
        assert!(vest);
        // A full diamond set takes 80%, the vest most of a bullet on top.
        let full: [Slot; ARMOR_SLOTS] = std::array::from_fn(|i| (i < 4).then(|| Stack::one(armor_id(3, i))));
        assert!((armor_factor(&full, false, false) - 0.2).abs() < 1e-6);
        assert!(armor_factor(&worn, true, false) < armor_factor(&worn, false, false));
        for id in ARMOR_BASE..=BULLETPROOF_VEST {
            assert!(armor_of(id).is_some() && armor_durability(id) > 0, "{id}");
        }
    }
}
