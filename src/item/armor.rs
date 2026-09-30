//! A worn set of armor: how much of a hit gets through it, and the set packed into a number
//! for the player model (and the other players). The pieces themselves (what each protects and
//! how long it lasts) are `content::items::armor`.

use super::*;

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
        for m in 0..MATERIALS {
            for piece in 0..4 {
                let id = armor_id(m, piece);
                assert!(armor_of(id) == Some((piece, m)) && armor_durability(id) > 0, "{id}");
            }
        }
        assert!(armor_durability(BULLETPROOF_VEST) > 0);
    }
}
