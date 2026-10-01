//! What a gun holds, in its stack's `data`: the rounds in its magazine, its state (a magazine
//! in it, a round in the chamber, the slide held back) and its attachments; a revolver's (a
//! `cylinder_gun`) is what is in each of its six chambers instead. The guns themselves are
//! `content::items::guns`.

use super::*;

/// A gun's state besides its rounds (bits of its data; a gun without them has a magazine in,
/// a round in the chamber and its slide forward): no magazine in it, nothing in the chamber,
/// the slide held back (by an empty magazine, after its last round).
pub mod gun_state {
    pub const NO_MAG: u16 = 0x40;
    pub const CHAMBER_EMPTY: u16 = 0x80;
    pub const LOCKED: u16 = 0x1000;
}

pub fn gun_has_mag(s: &Stack) -> bool {
    cylinder_gun(s.item) || s.data & gun_state::NO_MAG == 0
}

/// A round ready to fire (a revolver: any live round in its cylinder).
pub fn gun_chambered(s: &Stack) -> bool {
    if cylinder_gun(s.item) {
        return gun_rounds(s) > 0;
    }
    s.data & gun_state::CHAMBER_EMPTY == 0
}

pub fn gun_locked(s: &Stack) -> bool {
    !cylinder_gun(s.item) && s.data & gun_state::LOCKED != 0
}

pub fn set_gun_state(s: &mut Stack, bit: u16, on: bool) {
    if cylinder_gun(s.item) {
        return;
    }
    s.data = if on { s.data | bit } else { s.data & !bit };
}

/// What is in each of a revolver's six chambers: two bits each in its `data` (chamber k in
/// bits 2k, 2k+1), and the chamber under the hammer in bits 12-14. Chamber k is the one the
/// model's `chamber{k}` is; the cylinder turns the next one (`revolver_next`) under the hammer
/// as the trigger is pulled.
pub mod chamber {
    pub const EMPTY: u8 = 0;
    pub const LIVE: u8 = 1;
    /// A fired case, left in the chamber until the cylinder is emptied.
    pub const SPENT: u8 = 2;
}

pub fn revolver_chamber(s: &Stack, k: usize) -> u8 {
    ((s.data >> (2 * k)) & 3) as u8
}

pub fn set_revolver_chamber(s: &mut Stack, k: usize, v: u8) {
    s.data = (s.data & !(3 << (2 * k))) | ((v as u16 & 3) << (2 * k));
}

pub fn revolver_index(s: &Stack) -> usize {
    (((s.data >> 12) & 7) as usize).min(5)
}

pub fn set_revolver_index(s: &mut Stack, k: usize) {
    s.data = (s.data & !(7 << 12)) | (((k % 6) as u16) << 12);
}

/// The chamber that comes under the hammer after `k` (the cylinder turning a sixth,
/// anticlockwise seen from behind).
pub fn revolver_next(k: usize) -> usize {
    (k + 5) % 6
}

/// Rounds ready to fire: in the magazine and in the chamber (a revolver's: live in its
/// cylinder).
pub fn gun_ready_rounds(s: &Stack) -> u8 {
    if cylinder_gun(s.item) {
        return gun_rounds(s);
    }
    (if gun_has_mag(s) { gun_rounds(s) } else { 0 }) + gun_chambered(s) as u8
}

/// Rounds in a gun's magazine (the low 6 bits of its data), or in a magazine; a revolver's
/// live rounds.
pub fn gun_rounds(s: &Stack) -> u8 {
    if cylinder_gun(s.item) {
        return (0..6).filter(|&k| revolver_chamber(s, k) == chamber::LIVE).count() as u8;
    }
    (s.data & 0x3f) as u8
}

/// A revolver: `n` live rounds from the chamber after the one under the hammer on, the others
/// empty.
pub fn set_gun_rounds(s: &mut Stack, n: u8) {
    if cylinder_gun(s.item) {
        let mut k = revolver_index(s);
        for i in 0..6 {
            k = revolver_next(k);
            set_revolver_chamber(s, k, if i < n as usize { chamber::LIVE } else { chamber::EMPTY });
        }
        return;
    }
    s.data = (s.data & !0x3f) | (n as u16 & 0x3f);
}

/// A gun's attachments (`gun_mod` bits: the first four in bits 8-11 of the data, the weapon
/// light and its switch in bits 13-14; a revolver takes none).
pub fn gun_mods(s: &Stack) -> u8 {
    if cylinder_gun(s.item) {
        return 0;
    }
    ((s.data >> 8) & 0x0f) as u8 | ((s.data >> 9) & 0x30) as u8
}

pub fn set_gun_mods(s: &mut Stack, mods: u8) {
    if cylinder_gun(s.item) {
        return;
    }
    s.data = (s.data & !0x6f00) | ((mods as u16 & 0x0f) << 8) | ((mods as u16 & 0x30) << 9);
}

/// The magazine in a gun as its own item (the extended one if that is in it), with the
/// gun's rounds and as dirty as the gun.
pub fn magazine_in(gun: &Stack) -> Stack {
    let standard = GunKind::of(gun.item).and_then(|k| k.magazine_item()).unwrap_or(PISTOL_MAGAZINE);
    let item = if gun_mods(gun) & gun_mod::EXTENDED_MAGAZINE != 0 { EXTENDED_MAGAZINE } else { standard };
    let mut mag = Stack { damage: gun.damage, ..Stack::one(item) };
    set_gun_rounds(&mut mag, gun_rounds(gun));
    mag
}

/// Takes the magazine out of a gun (its rounds go with it; the round in the chamber stays).
pub fn remove_magazine(gun: &mut Stack) {
    set_gun_state(gun, gun_state::NO_MAG, true);
    set_gun_rounds(gun, 0);
    set_gun_mods(gun, gun_mods(gun) & !gun_mod::EXTENDED_MAGAZINE);
}

/// Puts a magazine into a gun (its rounds, and whether it is an extended one).
pub fn insert_magazine(gun: &mut Stack, mag: &Stack) {
    set_gun_state(gun, gun_state::NO_MAG, false);
    set_gun_rounds(gun, gun_rounds(mag));
    let ext = if mag.item == EXTENDED_MAGAZINE { gun_mod::EXTENDED_MAGAZINE } else { 0 };
    set_gun_mods(gun, (gun_mods(gun) & !gun_mod::EXTENDED_MAGAZINE) | ext);
}
