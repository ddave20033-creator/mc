//! A fingerprint of every item's data (stacking, food, damage, durability, keys, names in
//! both languages, icons, blocks, smelting, fuel, magazines) and of what the crafting grid
//! makes of every recipe's own grid (shifted, mirrored, spoilt a cell) and of random grids:
//! a change to any of them changes the hash. Rewriting how items are looked up must not.

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

fn items_snapshot() -> String {
    let mut s = String::new();
    for id in 0..1024u16 {
        let k = key(id);
        let _ = writeln!(
            s,
            "{id} {} {} {} {} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?} {:?} {:?}",
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
            smelt(id),
            smelt_tier(id),
            icon_str(id),
            k,
            from_key(&k),
            armor_of(id),
        );
    }
    for b in 0..BLOCK_IDS as Block {
        let _ = writeln!(s, "b{b} {:?}", item_of_block(b));
    }
    let _ = writeln!(s, "{:?}", all_items());
    for k in ["minecraft:stone", "unknown", "", "stick", "minecraft:ammo_box", "diamond_sword", "Stone"] {
        let _ = writeln!(s, "{k} {:?}", from_key(k));
    }
    s
}

fn names_snapshot() -> String {
    let mut s = String::new();
    for id in 0..1024u16 {
        let _ = writeln!(s, "{id} {}", name(id));
    }
    for b in 0..BLOCK_IDS as Block {
        let _ = writeln!(s, "b{b} {}", block_name(b));
    }
    s
}

fn craft_str(grid: &[Slot], size: usize) -> String {
    match craft(grid, size) {
        None => "-".into(),
        Some(st) => format!("{} {} {} {}", st.item, st.count, st.damage, st.data),
    }
}

fn crafting_snapshot() -> String {
    let mut s = String::new();
    let mut pool: Vec<ItemId> = Vec::new();
    let mut grids: Vec<[Option<ItemId>; 9]> = Vec::new();
    for item in recipe_results() {
        for (cells, n) in recipes_for(item) {
            let _ = writeln!(s, "r {item} {n}");
            for c in &cells {
                for &i in c {
                    if !pool.contains(&i) {
                        pool.push(i);
                    }
                }
            }
            // Each alternative for the cells (the first and the last), shifted about.
            for alt in [0usize, usize::MAX] {
                let g: [Option<ItemId>; 9] =
                    std::array::from_fn(|i| cells[i].get(alt.min(cells[i].len().saturating_sub(1))).copied());
                for dy in 0..3 {
                    for dx in 0..3 {
                        let mut h = [None; 9];
                        let mut fits = true;
                        for y in 0..3 {
                            for x in 0..3 {
                                if let Some(it) = g[y * 3 + x] {
                                    if x + dx >= 3 || y + dy >= 3 {
                                        fits = false;
                                    } else {
                                        h[(y + dy) * 3 + x + dx] = Some(it);
                                    }
                                }
                            }
                        }
                        if fits {
                            grids.push(h);
                            // Mirrored.
                            grids.push(std::array::from_fn(|i| h[(i / 3) * 3 + 2 - i % 3]));
                        }
                    }
                }
            }
        }
    }
    pool.sort();
    // Random grids, and each recipe grid with one cell spoilt.
    let mut seed: u64 = 12345;
    let mut rnd = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n
    };
    let base = grids.clone();
    for g in &base {
        for _ in 0..3 {
            let mut h = *g;
            let i = rnd(9);
            h[i] = if rnd(3) == 0 { None } else { Some(pool[rnd(pool.len())]) };
            grids.push(h);
        }
    }
    for _ in 0..6000 {
        let few = rnd(4);
        grids.push(std::array::from_fn(|_| (rnd(5) <= few).then(|| pool[rnd(pool.len())])));
    }
    for g in &grids {
        let slots: Vec<Slot> = g.iter().map(|c| c.map(|i| Stack::new(i, 1 + (i % 7) as u8))).collect();
        let _ = write!(s, "{} ", craft_str(&slots, 3));
        // The top left 2x2, as the inventory's grid.
        let small: Vec<Slot> = [0, 1, 3, 4].iter().map(|&i| slots[i]).collect();
        let _ = writeln!(s, "{}", craft_str(&small, 2));
    }
    // The same grid twice in a row, and one cell's count, damage and data changed.
    let mut slots: Vec<Slot> = vec![None; 9];
    slots[4] = Some(Stack::new(OAK_LOG as ItemId, 3));
    let _ = writeln!(s, "{} {}", craft_str(&slots, 3), craft_str(&slots, 3));
    slots[4] = Some(Stack { item: OAK_LOG as ItemId, count: 64, damage: 5, data: 9 });
    let _ = writeln!(s, "{}", craft_str(&slots, 3));
    slots[4] = None;
    let _ = writeln!(s, "{}", craft_str(&slots, 3));
    for item in recipe_results() {
        let _ = writeln!(s, "v {item} {:?} {:?}", recipe_view(item).map(|(r, st)| (r, st.item, st.count)), smelted_from(item));
    }
    s
}

#[test]
fn item_data_and_crafting_are_unchanged() {
    let items = items_snapshot();
    let en = names_snapshot();
    crate::lang::set_hungarian(true);
    let hu = names_snapshot();
    crate::lang::set_hungarian(false);
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

const EXPECTED: [u64; 4] = [0x60e3749d20e045a5, 0x1034e3267b837a11, 0xe20c1e6cb4959ed9, 0xe36efeceeba9c072];

