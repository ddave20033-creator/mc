//! The gun station's rules (not how it is drawn or used with the mouse, that is
//! `client::gui::gun_station`): what each thing is on its table, what may lie there, what goes
//! on or into what, how a gun comes apart into its parts and goes together from them, and how
//! long cleaning takes. What lies on a station is its block entity (`entity::GunBench`).

use super::*;
use crate::model::guns::gun::{BARREL, FRAME, MAGAZINE, SLIDE, SPRING};

/// What something is on the gun station's table (how it lies there and what clicks on it do).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BenchRole {
    Gun(GunKind),
    /// A part of a gun (`gun::FRAME` ..; a magazine is its gun's MAGAZINE part).
    Part(GunKind, usize),
    /// An attachment (its `gun_mod` bit).
    Attachment(u8),
    /// A round of a magazine-fed gun (the pistol's, the AK's).
    Round(GunKind),
    /// The revolver's round.
    Magnum,
    Speedloader,
    /// Anything else (it lies flat).
    Other,
}

pub fn bench_role(item: ItemId) -> BenchRole {
    for w in &WEAPONS {
        if w.item == item {
            return BenchRole::Gun(w.kind);
        }
        if let Some(p) = w.parts.iter().position(|&i| i == item) {
            return BenchRole::Part(w.kind, p);
        }
        // (an extended magazine is its gun's magazine)
        if magazine_gun(item) == Some(w.kind) {
            return BenchRole::Part(w.kind, MAGAZINE);
        }
        if w.ammo == item {
            return if w.kind.uses_magazine() { BenchRole::Round(w.kind) } else { BenchRole::Magnum };
        }
    }
    match item {
        SPEEDLOADER => BenchRole::Speedloader,
        _ => attachment_bit(item).map_or(BenchRole::Other, BenchRole::Attachment),
    }
}

/// Whether something may be laid on the table: only what belongs to the guns (a gun, its
/// parts, magazines, attachments, rounds and boxes of them); a long gun's things only on the
/// rifle station's (`rifle`).
pub fn belongs_on_bench(item: ItemId, rifle: bool) -> bool {
    (bench_role(item) != BenchRole::Other || item == AMMO_BOX) && (rifle || !needs_rifle_station(item))
}

/// A long gun's thing (the gun, a part, its magazine or rounds): worked on only at the rifle
/// station, the small one is for the handguns.
pub fn needs_rifle_station(item: ItemId) -> bool {
    match bench_role(item) {
        BenchRole::Gun(k) | BenchRole::Part(k, _) | BenchRole::Round(k) => k.long(),
        _ => false,
    }
}

/// The parts a gun lies in taken apart on the table (a magazine is not one of them: it is
/// laid beside them on its own).
pub fn table_parts(kind: GunKind) -> &'static [usize] {
    if kind.uses_magazine() {
        &[FRAME, BARREL, SPRING, SLIDE]
    } else {
        &[0, 1, 2, 3, 4]
    }
}

/// The attachment an item is (`gun_mod` bit), and back.
pub fn attachment_bit(item: ItemId) -> Option<u8> {
    match gun_role(item) {
        GunRole::Attachment(bit) => Some(bit),
        _ => None,
    }
}

pub fn attachment_item(bit: u8) -> ItemId {
    ATTACHMENTS.iter().find(|a| a.0 == bit).map_or(SCOPE, |a| a.1)
}

/// The attachments that sit on a part (and are kept in the part's `data`, as on a gun).
pub fn attachments_on(part: usize) -> u8 {
    match part {
        FRAME => gun_mod::RAIL,
        BARREL => gun_mod::SILENCER,
        SLIDE => gun_mod::SCOPE,
        _ => 0,
    }
}

/// A magazine's rounds and how many it holds (a gun's: the one in it).
pub fn magazine_of(st: &Stack) -> Option<(u8, u8)> {
    if let Some(cap) = magazine_capacity(st.item) {
        return Some((gun_rounds(st), cap));
    }
    let kind = GunKind::of(st.item)?;
    gun_has_mag(st).then(|| (gun_rounds(st), kind.magazine_size(gun_mods(st))))
}

/// Whether what is held (`held`) goes on or into `target` lying on the table: an attachment
/// onto a gun that takes it and has none such, a magazine into its gun without one, rounds
/// into a magazine (or a speedloader) or a box with room.
pub fn goes_onto(held: &Stack, target: &Stack) -> bool {
    if let Some(bit) = attachment_bit(held.item) {
        return GunKind::of(target.item).is_some_and(|k| k.fits(bit)) && attachment_fits(gun_mods(target), bit);
    }
    if let Some(g) = magazine_gun(held.item) {
        return GunKind::of(target.item) == Some(g) && !gun_has_mag(target);
    }
    if let Some(g) = GUN_KINDS.into_iter().find(|k| k.uses_magazine() && k.ammo() == held.item) {
        return match (magazine_capacity(target.item), target.item) {
            (Some(cap), m) if magazine_gun(m) == Some(g) => gun_rounds(target) < cap,
            (None, AMMO_BOX) => box_room(target.data, held.item) > 0,
            _ => false,
        };
    }
    if held.item == MAGNUM_ROUND {
        return match target.item {
            SPEEDLOADER => gun_rounds(target) < magazine_capacity(SPEEDLOADER).unwrap_or(6),
            AMMO_BOX => box_room(target.data, MAGNUM_ROUND) > 0,
            _ => false,
        };
    }
    false
}

/// Whether a place for a box of rounds in the drawer (the rounds in the box there, None: no
/// box) takes what is held: rounds into a box with room for them, a box where there is none.
pub fn box_place_takes(place: Option<u16>, held: ItemId) -> bool {
    match place {
        Some(v) => box_room(v, held) > 0,
        None => held == AMMO_BOX,
    }
}

/// The magazine in a gun, taken out: as its own item (with the gun's dirt), and whether the
/// round in the chamber went back into it (when there was room).
pub fn magazine_out_of(gun: &Stack) -> (Stack, bool) {
    let mut mag = magazine_in(gun);
    let cap = magazine_capacity(mag.item).unwrap_or(0);
    let round = gun_chambered(gun) && gun_rounds(gun) < cap;
    set_gun_rounds(&mut mag, gun_rounds(gun) + round as u8);
    (mag, round)
}

/// A gun with its magazine out.
pub fn without_magazine(gun: &Stack) -> Stack {
    let mut g = *gun;
    remove_magazine(&mut g);
    g
}

/// The gun a set of parts makes: as dirty as they are on average; a pistol with the
/// attachments that were on them, no magazine in it (that is put in the usual way) and
/// nothing in the chamber; a revolver with its cylinder empty.
pub fn assembled(kind: GunKind, parts: &[Stack]) -> Stack {
    let mut gun = Stack::one(kind.item());
    let mut mods = 0;
    for st in parts {
        if let BenchRole::Part(_, p) = bench_role(st.item) {
            mods |= gun_mods(st) & attachments_on(p);
        }
    }
    set_gun_mods(&mut gun, mods);
    set_gun_state(&mut gun, gun_state::NO_MAG, true);
    set_gun_state(&mut gun, gun_state::CHAMBER_EMPTY, true);
    let n = parts.len().max(1) as u32;
    gun.damage = ((parts.iter().map(|s| s.damage as u32).sum::<u32>() + n / 2) / n) as u16;
    gun
}

/// A gun as it comes apart on the table: the round in its chamber is already out (into its
/// magazine, or back to the player); a revolver's cylinder is emptied.
pub fn emptied(st: &Stack) -> Stack {
    let mut g = *st;
    if cylinder_gun(g.item) {
        for k in 0..6 {
            set_revolver_chamber(&mut g, k, chamber::EMPTY);
        }
        return g;
    }
    set_gun_state(&mut g, gun_state::CHAMBER_EMPTY, true);
    g
}

/// Seconds of scrubbing with the brush that clean a completely dirty part, or a whole gun.
pub fn scrub_seconds(item: ItemId) -> f32 {
    if GunKind::of(item).is_some() {
        6.0
    } else {
        1.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_things_of_the_guns_are_known_at_the_bench() {
        assert_eq!(bench_role(PISTOL), BenchRole::Gun(GunKind::Pistol));
        assert_eq!(bench_role(EXTENDED_MAGAZINE), BenchRole::Part(GunKind::Pistol, MAGAZINE));
        assert_eq!(bench_role(MAGNUM_ROUND), BenchRole::Magnum);
        assert_eq!(bench_role(SCOPE), BenchRole::Attachment(gun_mod::SCOPE));
        assert_eq!(bench_role(STICK), BenchRole::Other);
        for &(bit, item) in &ATTACHMENTS {
            assert_eq!(attachment_bit(item), Some(bit));
            assert_eq!(attachment_item(bit), item);
        }
        // The AK's things only on the rifle station, a box of rounds on either.
        assert!(!belongs_on_bench(AK47, false) && belongs_on_bench(AK47, true));
        assert!(belongs_on_bench(AMMO_BOX, false) && !belongs_on_bench(STICK, true));
    }

    #[test]
    fn a_magazine_comes_out_and_goes_back_in() {
        let mut gun = Stack::one(PISTOL);
        set_gun_mods(&mut gun, gun_mod::SCOPE | gun_mod::EXTENDED_MAGAZINE);
        set_gun_rounds(&mut gun, 9);
        gun.damage = 12;
        let (mag, round) = magazine_out_of(&gun);
        // (the round in the chamber goes back into it)
        assert!(round);
        assert_eq!((mag.item, gun_rounds(&mag), mag.damage), (EXTENDED_MAGAZINE, 10, 12));
        let empty = without_magazine(&gun);
        assert!(!gun_has_mag(&empty) && gun_mods(&empty) == gun_mod::SCOPE);
        assert!(goes_onto(&mag, &empty) && !goes_onto(&mag, &gun));
        let mut back = empty;
        insert_magazine(&mut back, &magazine_in(&gun));
        assert_eq!(back, gun);
    }
}
