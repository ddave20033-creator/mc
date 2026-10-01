//! The world's things in files of its own: the owner's inventory (`inventory.txt`), the
//! block entities, growing saplings, dropped items and mobs (`entities.txt`), the cuts in
//! trunks (`notches.txt`) and the trunks of felled trees lying (`logs.txt`).

use super::{dir, parse_pos, pos_str, write};
use crate::entity::mob::{Mob, MobKind};
use crate::entity::{BlockEntities, Furnace, ItemEntity};
use crate::item::{from_key, key, Slot, Stack};
use glam::{IVec3, Vec3};
use std::fs;

// ---------------------------------------------------------------- slots

fn slot_str(s: &Slot) -> String {
    match s {
        Some(s) if s.data != 0 => {
            format!("{}*{}*{}*{}", key(s.item), s.count, s.damage, s.data)
        }
        Some(s) => format!("{}*{}*{}", key(s.item), s.count, s.damage),
        None => "-".into(),
    }
}

fn parse_slot(s: &str) -> Slot {
    let p: Vec<&str> = s.split('*').collect();
    // The item's extra data (a pistol's rounds and attachments) is left out when it is 0.
    if !(3..=4).contains(&p.len()) {
        return None;
    }
    Some(Stack {
        item: from_key(p[0])?,
        count: p[1].parse().ok().filter(|&n| n > 0)?,
        damage: p[2].parse().ok()?,
        data: match p.get(3) {
            Some(d) => d.parse().ok()?,
            None => 0,
        },
    })
}

fn slots_str(slots: &[Slot]) -> String {
    slots.iter().map(slot_str).collect::<Vec<_>>().join("|")
}

fn parse_slots(s: &str, out: &mut [Slot]) {
    for (i, part) in s.split('|').enumerate().take(out.len()) {
        out[i] = parse_slot(part);
    }
}


pub fn save_inventory(folder: &str, slots: &[Slot]) {
    write(
        dir(folder).join("inventory.txt"),
        slots_str(slots).as_bytes(),
    );
}

pub fn load_inventory(folder: &str, out: &mut [Slot]) {
    if let Ok(s) = fs::read_to_string(dir(folder).join("inventory.txt")) {
        parse_slots(s.trim(), out);
    }
}

/// The axe's cuts in trunks and the stumps of felled trees (`client::shown::felling`).
pub fn save_notches(folder: &str, text: &str) {
    write(dir(folder).join("notches.txt"), text.as_bytes());
}

pub fn load_notches(folder: &str) -> String {
    fs::read_to_string(dir(folder).join("notches.txt")).unwrap_or_default()
}

/// The trunks of felled trees lying on the ground (`client::shown::logs`).
pub fn save_logs(folder: &str, text: &str) {
    write(dir(folder).join("logs.txt"), text.as_bytes());
}

pub fn load_logs(folder: &str) -> String {
    fs::read_to_string(dir(folder).join("logs.txt")).unwrap_or_default()
}

/// Block entities, growing saplings (position -> seconds until it grows), dropped items and mobs.
pub fn save_entities(
    folder: &str,
    be: &BlockEntities,
    saplings: &[(IVec3, f32)],
    items: &[ItemEntity],
    mobs: &[Mob],
) {
    let mut s = String::new();
    for it in items.iter().filter(|it| !it.is_picking_up()) {
        s += &format!(
            "item:{},{},{}:{}:{}\n",
            it.pos.x,
            it.pos.y,
            it.pos.z,
            slot_str(&Some(it.stack)),
            it.age
        );
    }
    for m in mobs.iter().filter(|m| m.alive()) {
        // (its kind's own state last: `MobState::save`)
        s += &format!(
            "mob:{}:{},{},{}:{}:{}:{}\n",
            m.kind.key(),
            m.pos.x,
            m.pos.y,
            m.pos.z,
            m.body_yaw,
            m.health,
            m.state.save()
        );
    }
    for (p, f) in &be.furnaces {
        s += &format!(
            "furnace:{}:{},{},{}:{}\n",
            pos_str(*p),
            f.burn,
            f.burn_total,
            f.cook,
            slots_str(&[f.input, f.fuel, f.output])
        );
    }
    for (p, f) in &be.furnaces {
        for (i, g) in f.grill.iter().enumerate() {
            if let Some(g) = g {
                s += &format!(
                    "grill:{}:{}:{}:{},{}:{}\n",
                    pos_str(*p),
                    i,
                    crate::item::key(g.raw),
                    g.cook[0],
                    g.cook[1],
                    g.down
                );
            }
        }
    }
    for (p, c) in &be.chests {
        s += &format!("chest:{}:{}\n", pos_str(*p), slots_str(&c[..]));
    }
    for (p, t) in &be.tables {
        s += &format!("table:{}:{}\n", pos_str(*p), slots_str(&t[..]));
    }
    // What lies on the gun stations: each thing's stack and where (x/z/turn), `|` between.
    for (p, b) in &be.benches {
        let items: Vec<String> = b
            .items
            .iter()
            .map(|i| format!("{}/{}/{}/{}", slot_str(&Some(i.stack)), i.x, i.z, i.turn))
            .collect();
        let boxes: Vec<String> = b.boxes.iter().map(|n| n.map_or("-".to_string(), |n| n.to_string())).collect();
        s += &format!(
            "bench:{}:{}:{}:{}:{}:{},{}\n",
            pos_str(*p),
            items.join("|"),
            boxes.join(","),
            b.loader as u8,
            slot_str(&b.loader_mag),
            b.grenades[0],
            b.grenades[1]
        );
    }
    for (p, t) in saplings {
        s += &format!("sapling:{}:{}\n", pos_str(*p), t);
    }
    write(dir(folder).join("entities.txt"), s.as_bytes());
}

pub fn load_entities(
    folder: &str,
    be: &mut BlockEntities,
    saplings: &mut Vec<(IVec3, f32)>,
    items: &mut Vec<ItemEntity>,
    mobs: &mut Vec<Mob>,
) {
    let Ok(text) = fs::read_to_string(dir(folder).join("entities.txt")) else {
        return;
    };
    for line in text.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts[0] == "item" && parts.len() >= 4 {
            let p: Vec<f32> = parts[1].split(',').filter_map(|x| x.parse().ok()).collect();
            if let (3, Some(stack)) = (p.len(), parse_slot(parts[2])) {
                let mut it = ItemEntity::new(Vec3::new(p[0], p[1], p[2]), Vec3::ZERO, stack, 0.0);
                it.age = parts[3].parse().unwrap_or(0.0);
                items.push(it);
            }
            continue;
        }
        if parts[0] == "mob" && parts.len() >= 5 {
            let p: Vec<f32> = parts[2].split(',').filter_map(|x| x.parse().ok()).collect();
            if let (3, Some(kind)) = (p.len(), MobKind::from_key(parts[1])) {
                // (the server gives it its id, and its random numbers with it)
                let yaw = parts[3].parse().unwrap_or(0.0);
                let mut m = Mob::new(kind, Vec3::new(p[0], p[1], p[2]), yaw, 0);
                m.state.load(parts.get(5).copied().unwrap_or(""));
                m.health = parts[4].parse().unwrap_or(m.max_health()).min(m.max_health());
                mobs.push(m);
            }
            continue;
        }
        let Some(p) = parts.get(1).and_then(|s| parse_pos(s)) else {
            continue;
        };
        match parts[0] {
            "furnace" if parts.len() >= 4 => {
                let t: Vec<f32> = parts[2].split(',').filter_map(|x| x.parse().ok()).collect();
                let mut slots = [None; 3];
                parse_slots(parts[3], &mut slots);
                let mut f = Furnace {
                    input: slots[0],
                    fuel: slots[1],
                    output: slots[2],
                    ..Default::default()
                };
                if t.len() == 3 {
                    (f.burn, f.burn_total, f.cook) = (t[0], t[1], t[2]);
                }
                be.furnaces.insert(p, f);
            }
            // Meat on a furnace's top (after its furnace line).
            "grill" if parts.len() >= 6 => {
                let t: Vec<f32> = parts[4].split(',').filter_map(|x| x.parse().ok()).collect();
                let (Ok(i), Some(raw), 2) = (
                    parts[2].parse::<usize>(),
                    crate::item::from_key(parts[3]),
                    t.len(),
                ) else {
                    continue;
                };
                if let (Some(f), true) = (be.furnaces.get_mut(&p), i < 4) {
                    f.grill[i] = Some(crate::entity::Grilled {
                        raw,
                        cook: [t[0], t[1]],
                        down: parts[5].parse::<u8>().unwrap_or(1) & 1,
                        flip: 0.0,
                    });
                }
            }
            "chest" if parts.len() >= 3 => {
                let mut slots = Box::new([None; 27]);
                parse_slots(parts[2], &mut slots[..]);
                be.chests.insert(p, slots);
            }
            "table" if parts.len() >= 3 => {
                let mut slots = [None; 9];
                parse_slots(parts[2], &mut slots);
                be.tables.insert(p, slots);
            }
            "bench" if parts.len() >= 3 => {
                let mut bench = crate::entity::GunBench::default();
                for item in parts[2].split('|').filter(|i| !i.is_empty()) {
                    let f: Vec<&str> = item.split('/').collect();
                    let num = |i: usize| f.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                    if let Some(st) = parse_slot(f[0]) {
                        bench.add(st, num(1), num(2), num(3));
                    }
                }
                if let Some(a) = parts.get(3) {
                    for (i, n) in a.split(',').take(3).enumerate() {
                        bench.boxes[i] = n.parse::<u16>().ok().map(|n| {
                            crate::item::box_count(n) | (n & crate::item::BOX_KIND)
                        });
                    }
                }
                bench.loader = parts.get(4) == Some(&"1");
                bench.loader_mag = parts.get(5).and_then(|s| parse_slot(s));
                if let Some(g) = parts.get(6) {
                    let max = crate::model::gun_station::CRATE_MAX;
                    for (i, n) in g.split(',').take(2).enumerate() {
                        bench.grenades[i] = n.parse::<u8>().unwrap_or(0).min(max);
                    }
                }
                be.benches.insert(p, bench);
            }
            "sapling" if parts.len() >= 3 => saplings.push((p, parts[2].parse().unwrap_or(60.0))),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- cuts in trunks

/// All the cuts in trunks, for saving (`x,y,z,angle,height,depth,felled` a line).
pub fn notches_text(w: &crate::world::World) -> String {
    w.notches
        .iter()
        .map(|(p, n)| format!("{},{},{},{},{},{},{}\n", p.x, p.y, p.z, n.angle, n.height, n.depth, n.felled as u8))
        .collect()
}

/// The cuts saved with a world (any there were before are gone).
pub fn apply_notches(w: &mut crate::world::World, text: &str) {
    w.notches.clear();
    for line in text.lines() {
        let v: Vec<&str> = line.trim().split(',').collect();
        if v.len() != 7 {
            continue;
        }
        let i = |k: usize| v[k].parse::<i32>().ok();
        let f = |k: usize| v[k].parse::<f32>().ok();
        if let (Some(x), Some(y), Some(z), Some(angle), Some(height), Some(depth)) = (i(0), i(1), i(2), f(3), f(4), f(5)) {
            w.set_notch(IVec3::new(x, y, z), Some(crate::world::mesh::Notch { angle, height, depth, felled: v[6] == "1" }));
        }
    }
}

#[cfg(test)]
mod bench_tests {
    use super::*;

    #[test]
    fn a_gun_station_keeps_what_lies_on_it_and_its_boxes() {
        let folder = "zz_bench_save_test";
        let _ = fs::create_dir_all(dir(folder));
        let mut be = BlockEntities::default();
        let mut bench = crate::entity::GunBench::default();
        let mut mag = Stack::one(crate::item::PISTOL_MAGAZINE);
        crate::item::set_gun_rounds(&mut mag, 7);
        bench.add(mag, 0.25, -0.1, 0.0);
        bench.add(Stack { data: 40, ..Stack::one(crate::item::AMMO_BOX) }, -0.5, 0.2, 0.3);
        // (the last a box of magnum rounds: its kind is kept)
        bench.boxes = [Some(128), None, Some(3 | crate::item::BOX_MAGNUM)];
        // (and grenades in the rifle station's crate)
        bench.grenades = [5, 12];
        be.benches.insert(IVec3::new(4, 70, -9), bench.clone());
        save_entities(folder, &be, &[], &[], &[]);
        let mut back = BlockEntities::default();
        load_entities(folder, &mut back, &mut Vec::new(), &mut Vec::new(), &mut Vec::new());
        let _ = fs::remove_dir_all(dir(folder));
        let got = back.benches.get(&IVec3::new(4, 70, -9)).expect("saved");
        assert_eq!(got.boxes, bench.boxes);
        assert_eq!(got.grenades, [5, 12]);
        assert_eq!(got.items.len(), 2);
        assert_eq!(got.items[0].stack, mag);
        assert_eq!(crate::item::box_rounds(&got.items[1].stack), 40);
    }
}
