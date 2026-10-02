//! A fingerprint of every item's data (stacking, food, damage, durability, keys, names in
//! both languages, icons, blocks, smelting, fuel, magazines) and of every recipe (its grid,
//! what it takes and makes, by hand or at a crafting table): a change to any of them changes
//! the hash. Rewriting how items are looked up must not.
//! Items are written by their keys (their ids are not their identity: they follow the order
//! of the items' table), and listed in the order of their keys.

use super::*;
use std::fmt::Write;

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn consumable_str(id: ItemId) -> String {
    match consumable(id) {
        None => "-".into(),
        Some(c) => format!("{} {} {} {:?} {}", c.food, c.saturation, c.thirst, c.sick, c.drink),
    }
}

fn icon_str(id: ItemId) -> String {
    match icon(id) {
        Icon::Block(b) => format!("B{b}"),
        Icon::Flat(l) => format!("F{l}"),
    }
}

/// An item by its key (an id that is no item: by its number).
fn ik(id: ItemId) -> String {
    let k = key(id);
    if k == "unknown" {
        format!("#{id}")
    } else {
        k
    }
}

/// Every item that is not a block, in the order of their keys.
fn other_items() -> Vec<ItemId> {
    let mut v: Vec<ItemId> = (FIRST_ITEM..4096).filter(|&id| key(id) != "unknown").collect();
    v.sort_by_key(|&id| key(id));
    v
}

fn items_snapshot() -> String {
    let mut s = String::new();
    let ids = (0..FIRST_ITEM).map(|id| (id.to_string(), id)).chain(other_items().into_iter().map(|id| (key(id), id)));
    for (label, id) in ids {
        let k = key(id);
        let _ = writeln!(
            s,
            "{label} {} {} {} {} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?} {:?} {:?}",
            max_stack(id),
            consumable_str(id),
            attack_damage(id),
            max_damage(id),
            block_of(id),
            tool_of(id),
            GunKind::of(id),
            magazine_capacity(id),
            magazine_gun(id),
            fuel_time(id),
            smelt(id).map(ik),
            smelt_tier(id),
            icon_str(id),
            k,
            from_key(&k).map(ik),
            armor_of(id),
        );
    }
    for b in 0..BLOCK_IDS as Block {
        // (the blocks' items by their ids, the others by their keys)
        let item = match item_of_block(b) {
            Some(i) if i >= FIRST_ITEM => format!("Some({:?})", key(i)),
            i => format!("{i:?}"),
        };
        let _ = writeln!(s, "b{b} {item}");
    }
    let mut all: Vec<String> = all_items().into_iter().map(ik).collect();
    all.sort();
    let _ = writeln!(s, "{all:?}");
    for k in ["minecraft:stone", "unknown", "", "stick", "minecraft:ammo_box", "diamond_sword", "Stone"] {
        let _ = writeln!(s, "{k} {:?}", from_key(k).map(ik));
    }
    s
}

fn names_snapshot() -> String {
    let mut s = String::new();
    for id in 0..FIRST_ITEM {
        let _ = writeln!(s, "{id} {}", name(id));
    }
    for id in other_items() {
        let _ = writeln!(s, "{} {}", key(id), name(id));
    }
    for b in 0..BLOCK_IDS as Block {
        let _ = writeln!(s, "b{b} {}", block_name(b));
    }
    s
}

fn crafting_snapshot() -> String {
    let mut s = String::new();
    for item in recipe_results() {
        for (cells, n, hand) in recipes_for(item) {
            let cells: Vec<Vec<String>> = cells.iter().map(|c| c.iter().copied().map(ik).collect()).collect();
            let _ = writeln!(s, "r {} {n} {hand} {cells:?}", ik(item));
        }
    }
    for l in craft_list() {
        let needs: Vec<(Vec<String>, u8)> =
            l.needs.iter().map(|(items, n)| (items.iter().copied().map(ik).collect(), *n)).collect();
        let _ = writeln!(s, "l {} {} {} {needs:?}", ik(l.result.item), l.result.count, l.hand);
    }
    for item in recipe_results() {
        let view = recipe_view(item).map(|(r, st)| {
            let r: Vec<Vec<Vec<String>>> =
                r.into_iter().map(|row| row.into_iter().map(|c| c.into_iter().map(ik).collect()).collect()).collect();
            (r, ik(st.item), st.count)
        });
        let mut from: Vec<(String, u8)> = smelted_from(item).into_iter().map(|(i, t)| (ik(i), t)).collect();
        from.sort();
        let _ = writeln!(s, "v {} {view:?} {from:?}", ik(item));
    }
    s
}

#[test]
fn item_data_and_crafting_are_unchanged() {
    let items = items_snapshot();
    let en = names_snapshot();
    crate::app::lang::set_hungarian(true);
    let hu = names_snapshot();
    crate::app::lang::set_hungarian(false);
    let craft = crafting_snapshot();
    if let Ok(dir) = std::env::var("ITEM_SNAPSHOT_DIR") {
        for (f, t) in [("items", &items), ("en", &en), ("hu", &hu), ("craft", &craft)] {
            std::fs::write(format!("{dir}/{f}.txt"), t).unwrap();
        }
    }
    let got = [fnv(&items), fnv(&en), fnv(&hu), fnv(&craft)];
    println!("{got:#x?}");
    assert_eq!(got, EXPECTED);
}

const EXPECTED: [u64; 4] = [0x5c3426ba1a7a84d0, 0x5293f6a520c67988, 0x3dbc3468aa2dc181, 0xb6de30d3459e7205];

