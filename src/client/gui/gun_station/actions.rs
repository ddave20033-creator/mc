//! What a click does at the open gun station: something picked up or laid down, a gun taken
//! apart or put together, an attachment or a magazine on or off, rounds into a magazine or a
//! box, the rifle station's loader, and the brush scrubbing. Each change goes to the others
//! (`bench_changed`), with the animation it plays.

use super::layout::{free_spot, is_free, magazine_spot, strip_targets};
use super::pick::Pick;
use super::pieces::modelled;
use super::table::Table;
use crate::client::Game;
use crate::entity::{BenchEvent, BenchItem, GunBench, bench_event};
use crate::item::inventory::take;
use crate::item::*;
use crate::lang::t;
use glam::{IVec3, Vec3};

/// Seconds between sending the table to the others while scrubbing.
const SCRUB_SYNC: f32 = 0.25;

impl Game {
    /// Whether what is held on the mouse goes on or into what it is on (`gun_bench::goes_onto`;
    /// in the drawer, a box of rounds or its place, the loader).
    pub(super) fn bench_target_ok(&self, bench: &GunBench, pick: Option<Pick>) -> bool {
        let Some(st) = self.me.items.cursor else { return pick.is_some() };
        match pick {
            // A magazine onto the loader when there is none on it; the loader into its bay.
            Some(Pick::Loader) => bench.loader_takes(Some(st)),
            Some(Pick::Ammo(i)) => box_place_takes(bench.boxes[i as usize], st.item),
            Some(k) => k.item().and_then(|id| bench.get(id)).is_some_and(|t| goes_onto(&st, &t.stack)),
            None => false,
        }
    }

    /// Something lying on the table clicked with an empty hand: picked up onto the mouse (to
    /// be dragged).
    pub(super) fn bench_pick_up(&mut self, p: IVec3, id: u16) {
        if let Some(it) = self.level.block_entities.benches.get_mut(&p).and_then(|b| b.take(id)) {
            self.me.items.cursor = Some(it.stack);
            self.bench_ui.drag = Some(self.ui.mouse);
            self.bench_changed(p, None);
        }
    }

    /// Something held on the mouse cursor put on the table: an attachment onto the gun under
    /// the mouse (if it fits and has none such), a magazine into it, rounds into a box, a
    /// speedloader or a magazine; otherwise laid where the mouse points (all of it, or one
    /// with the right button; a gun's things one at a time).
    pub(super) fn bench_put(&mut self, p: IVec3, table: &Table, pick: Option<Pick>, spot: Option<(f32, f32)>, one: bool) {
        let Some(st) = self.me.items.cursor else { return };
        let on = pick.and_then(Pick::item);
        if let Some(id) = on {
            if self.bench_mag_in(p, id, spot) {
                return;
            }
        }
        let bench = self.level.block_entities.benches.entry(p).or_default();
        if let (Some(bit), Some(id)) = (attachment_bit(st.item), on) {
            if bench.get(id).is_some_and(|g| goes_onto(&st, &g.stack)) {
                // It goes on from where the mouse let go of it.
                let (x, z) = spot.unwrap_or((0.0, 0.0));
                let gone = BenchItem { id: 0, stack: Stack::one(st.item), x, z, turn: 0.0 };
                if let Some(g) = bench.items.iter_mut().find(|g| g.id == id) {
                    let m = gun_mods(&g.stack) | bit;
                    set_gun_mods(&mut g.stack, m);
                }
                take(&mut self.me.items.cursor, 1);
                let e = BenchEvent { kind: bench_event::FIT, gun: id, bit, gone: vec![gone], ..Default::default() };
                self.bench_changed(p, Some(e));
                return;
            }
        }
        let picked = match pick {
            Some(Pick::Item(id)) => bench.get(id).copied(),
            _ => None,
        };
        let count = |cap: u8| (if one { 1 } else { st.count }).min(cap);
        match (st.item, picked) {
            // Rounds onto a box of them lying on the table: into it (only the kind it holds).
            (BULLET | MAGNUM_ROUND | RIFLE_ROUND, Some(b)) if b.stack.item == AMMO_BOX => {
                let room = box_room(b.stack.data, st.item);
                let n = (if one { 1 } else { st.count as u16 }).min(room);
                if n > 0 {
                    if let Some(b) = bench.items.iter_mut().find(|i| i.id == b.id) {
                        b.stack.data = box_with(b.stack.data, st.item, n);
                    }
                    take(&mut self.me.items.cursor, n as u8);
                    self.bench_changed(p, None);
                }
            }
            // Magnum rounds onto a speedloader lying on the table: into it.
            (MAGNUM_ROUND, Some(l)) if l.stack.item == SPEEDLOADER => {
                let cap = magazine_capacity(SPEEDLOADER).unwrap_or(6);
                let n = count(cap.saturating_sub(gun_rounds(&l.stack)));
                if n > 0 {
                    if let Some(l) = bench.items.iter_mut().find(|i| i.id == l.id) {
                        let r = gun_rounds(&l.stack) + n;
                        set_gun_rounds(&mut l.stack, r);
                    }
                    take(&mut self.me.items.cursor, n);
                    self.bench_changed(p, None);
                }
            }
            // Rounds onto a magazine lying on the table: pushed into it, one after another.
            (BULLET | RIFLE_ROUND, Some(m)) if magazine_gun(m.stack.item).is_some_and(|k| k.ammo() == st.item) => {
                let cap = magazine_capacity(m.stack.item).unwrap_or(0);
                let n = count(cap.saturating_sub(gun_rounds(&m.stack)));
                if n > 0 {
                    if let Some(g) = bench.items.iter_mut().find(|i| i.id == m.id) {
                        let r = gun_rounds(&g.stack) + n;
                        set_gun_rounds(&mut g.stack, r);
                    }
                    let (x, z) = spot.unwrap_or((m.x, m.z + 0.15));
                    let gone = BenchItem { id: 0, stack: Stack::new(st.item, n), x, z, turn: 0.0 };
                    take(&mut self.me.items.cursor, n);
                    let e = BenchEvent { kind: bench_event::LOAD, gun: m.id, bit: n, gone: vec![gone], ..Default::default() };
                    self.bench_changed(p, Some(e));
                }
            }
            _ => self.bench_lay(p, table, st, spot, one),
        }
    }

    /// What is held on the mouse laid on the table where the mouse points (all of it, or one
    /// with the right button; a gun's things one at a time), beside what already lies there.
    fn bench_lay(&mut self, p: IVec3, table: &Table, st: Stack, spot: Option<(f32, f32)>, one: bool) {
        let Some((x, z)) = spot else { return };
        if !belongs_on_bench(st.item, table.rifle()) {
            if needs_rifle_station(st.item) {
                self.gun_message(t("gun.rifle_station_only"));
            }
            return;
        }
        let model = modelled(st.item);
        let single = one || model;
        let lay = if single { Stack { count: 1, ..st } } else { st };
        let turn = if model { 0.0 } else { (self.random() - 0.5) * 0.6 };
        let bench = self.level.block_entities.benches.entry(p).or_default();
        let (x, z) = match self.bench_ui.held_spot {
            Some(q) if single && is_free(table, bench, lay, q.0, q.1, turn) => q,
            _ => free_spot(table, bench, lay, x, z, turn),
        };
        bench.add(lay, x, z, turn);
        take(&mut self.me.items.cursor, lay.count);
        self.bench_changed(p, None);
    }

    /// A right click on something on the table: a gun comes apart there, a part puts a gun
    /// together from the parts on the table (when they are all there), a box of rounds gives
    /// one onto the mouse.
    pub(super) fn bench_right_click(&mut self, p: IVec3, table: &Table, id: u16) {
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return };
        let Some(it) = bench.get(id).copied() else { return };
        match bench_role(it.stack.item) {
            BenchRole::Gun(_) => {
                bench.take(id);
                let (_, targets, loose) = strip_targets(table, &it);
                // Where something else already lies, a part goes beside it.
                let made = targets
                    .into_iter()
                    .map(|(st, x, z, turn)| {
                        let (x, z) = free_spot(table, bench, st, x, z, turn);
                        bench.add(st, x, z, turn)
                    })
                    .collect();
                let e = BenchEvent { kind: bench_event::STRIP, gun: id, gone: vec![it], made, ..Default::default() };
                self.bench_changed(p, Some(e));
                // The live rounds that were in it (the pistol's chamber, when it did not fit back
                // into the magazine; the revolver's cylinder).
                if let (true, Some(k)) = (loose > 0, GunKind::of(it.stack.item)) {
                    self.give(Stack::new(k.ammo(), loose));
                }
            }
            BenchRole::Other if it.stack.item == AMMO_BOX => {
                // A round out of it, onto the mouse.
                if let (true, Some(kind)) = (self.me.items.cursor.is_none(), box_ammo(it.stack.data)) {
                    if let Some(b) = bench.items.iter_mut().find(|b| b.id == id) {
                        b.stack.data = box_without(b.stack.data, 1);
                    }
                    self.me.items.cursor = Some(Stack::one(kind));
                    self.bench_ui.drag = Some(self.ui.mouse);
                    self.bench_changed(p, None);
                }
            }
            BenchRole::Part(kind, q) if !(kind.uses_magazine() && q == crate::model::gun::MAGAZINE) => {
                // One of each part (the clicked one first).
                let mut chosen: Vec<u16> = Vec::new();
                for &want in table_parts(kind) {
                    let is = |i: &&BenchItem| bench_role(i.stack.item) == BenchRole::Part(kind, want);
                    match bench.items.iter().filter(is).min_by_key(|i| (i.id != id) as u8) {
                        Some(c) => chosen.push(c.id),
                        None => return,
                    }
                }
                let mut gone = Vec::new();
                for c in chosen {
                    let Some(i) = bench.items.iter().position(|i| i.id == c) else { continue };
                    let it = &mut bench.items[i];
                    gone.push(BenchItem { stack: Stack { count: 1, ..it.stack }, ..*it });
                    if it.stack.count > 1 {
                        it.stack.count -= 1;
                    } else {
                        bench.items.remove(i);
                    }
                }
                let stacks: Vec<Stack> = gone.iter().map(|i| i.stack).collect();
                let gun = assembled(kind, &stacks);
                let (x, z) = free_spot(table, bench, gun, 0.0, 0.0, 0.0);
                let gid = bench.add(gun, x, z, 0.0);
                let e = BenchEvent { kind: bench_event::ASSEMBLE, gun: gid, gone, ..Default::default() };
                self.bench_changed(p, Some(e));
            }
            _ => {}
        }
    }

    /// A click (or something let go) on a place for a box of rounds in the drawer: the box
    /// taken out (a round out of it with the right button), rounds dropped into it (all, or
    /// one with the right button), a box put back where there is none.
    pub(super) fn bench_box_slot(&mut self, p: IVec3, i: usize, right: bool) {
        let bench = self.level.block_entities.benches.entry(p).or_default();
        match (self.me.items.cursor, bench.boxes[i]) {
            (None, Some(v)) if right => match box_ammo(v) {
                Some(kind) => {
                    bench.boxes[i] = Some(box_without(v, 1));
                    self.me.items.cursor = Some(Stack::one(kind));
                }
                None => return,
            },
            (None, Some(v)) => {
                bench.boxes[i] = None;
                self.me.items.cursor = Some(Stack { data: v, ..Stack::one(AMMO_BOX) });
            }
            (Some(st), Some(v)) if BOX_AMMO.contains(&st.item) => {
                // Only the kind it holds (either, when it is empty).
                let k = (if right { 1 } else { st.count as u16 }).min(box_room(v, st.item));
                if k == 0 {
                    return;
                }
                bench.boxes[i] = Some(box_with(v, st.item, k));
                take(&mut self.me.items.cursor, k as u8);
            }
            (Some(st), None) if st.item == AMMO_BOX => {
                bench.boxes[i] = Some(st.data);
                take(&mut self.me.items.cursor, 1);
            }
            _ => return,
        }
        if self.me.items.cursor.is_some() {
            self.bench_ui.drag = Some(self.ui.mouse);
        }
        self.bench_changed(p, None);
    }

    /// A click on the rifle station's magazine loader (or with one held, in its drawer): the
    /// loader put in the middle of the drawer, a magazine laid on it (it fills it from the
    /// boxes beside it), the magazine taken off it, or the loader itself taken out when it is
    /// bare.
    pub(super) fn bench_loader_click(&mut self, p: IVec3, table: &Table) {
        let drawer = self.level.bench_drawer.get(&p).copied().unwrap_or(0.0);
        let bench = self.level.block_entities.benches.entry(p).or_default();
        match self.me.items.cursor {
            Some(st) if st.item == MAG_LOADER => {
                if !table.rifle() {
                    self.gun_message(t("gun.loader_rifle_station"));
                    return;
                }
                if bench.loader || drawer < 0.8 {
                    return;
                }
                bench.loader = true;
                take(&mut self.me.items.cursor, 1);
            }
            Some(st) if is_gun_magazine(st.item) && bench.loader && bench.loader_mag.is_none() => {
                bench.loader_mag = Some(Stack { count: 1, ..st });
                take(&mut self.me.items.cursor, 1);
            }
            None if bench.loader_mag.is_some() => {
                self.me.items.cursor = bench.loader_mag.take();
                self.bench_ui.drag = Some(self.ui.mouse);
            }
            None if bench.loader => {
                bench.loader = false;
                self.me.items.cursor = Some(Stack::one(MAG_LOADER));
                self.bench_ui.drag = Some(self.ui.mouse);
            }
            _ => return,
        }
        self.bench_changed(p, None);
    }

    /// The magazine of a gun on the table clicked: it slides out and is laid beside the gun.
    pub(super) fn bench_mag_out(&mut self, p: IVec3, table: &Table, id: u16) {
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return };
        let Some(before) = bench.get(id).copied().filter(|g| gun_has_mag(&g.stack)) else { return };
        // Its rounds come with it (the round in the chamber stays there).
        let mag = magazine_in(&before.stack);
        let (x, z) = magazine_spot(table, &before);
        if let Some(g) = bench.items.iter_mut().find(|g| g.id == id) {
            remove_magazine(&mut g.stack);
        }
        let (x, z) = free_spot(table, bench, mag, x, z, before.turn);
        let mid = bench.add(mag, x, z, before.turn);
        let e = BenchEvent { kind: bench_event::MAG_OUT, gun: id, gone: vec![before], made: vec![mid], ..Default::default() };
        self.bench_changed(p, Some(e));
    }

    /// A magazine held on the mouse let go on a gun without one: it goes in.
    fn bench_mag_in(&mut self, p: IVec3, id: u16, spot: Option<(f32, f32)>) -> bool {
        let Some(st) = self.me.items.cursor.filter(|s| is_gun_magazine(s.item)) else { return false };
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return false };
        let Some(g) = bench.items.iter_mut().find(|g| g.id == id && goes_onto(&st, &g.stack)) else {
            return false;
        };
        insert_magazine(&mut g.stack, &st);
        let (x, z) = spot.unwrap_or((g.x, g.z + 0.2));
        let gone = BenchItem { id: 0, stack: Stack { count: 1, ..st }, x, z, turn: g.turn };
        take(&mut self.me.items.cursor, 1);
        let e = BenchEvent { kind: bench_event::MAG_IN, gun: id, gone: vec![gone], ..Default::default() };
        self.bench_changed(p, Some(e));
        true
    }

    /// An attachment on a gun clicked: it comes off and is laid beside the gun.
    pub(super) fn bench_unfit(&mut self, p: IVec3, table: &Table, id: u16, bit: u8) {
        let Some(bench) = self.level.block_entities.benches.get_mut(&p) else { return };
        let Some(g) = bench.items.iter_mut().find(|g| g.id == id) else { return };
        let m = gun_mods(&g.stack) & !bit;
        set_gun_mods(&mut g.stack, m);
        let (x, z) = (g.x, g.z + 0.22);
        let att = Stack::one(attachment_item(bit));
        let (x, z) = free_spot(table, bench, att, x, z, 0.0);
        let aid = bench.add(att, x, z, 0.0);
        let e = BenchEvent { kind: bench_event::UNFIT, gun: id, bit, made: vec![aid], ..Default::default() };
        self.bench_changed(p, Some(e));
    }

    /// The brush held down on something on the table: its dirt comes off (a part quickly, a
    /// whole gun slowly), with foam where it works.
    pub(super) fn scrub(&mut self, p: IVec3, id: u16, at: Option<Vec3>) {
        let dt = self.ui.dt;
        let Some(it) = self.level.block_entities.benches.get_mut(&p).and_then(|b| b.items.iter_mut().find(|i| i.id == id)) else {
            return;
        };
        let (item, damage) = (it.stack.item, it.stack.damage);
        let max = max_damage(item);
        if max == 0 || damage == 0 {
            return;
        }
        self.bench_ui.scrubbing = true;
        self.bench_ui.scrub += dt * max as f32 / scrub_seconds(item);
        let off = self.bench_ui.scrub.floor();
        if off >= 1.0 {
            self.bench_ui.scrub -= off;
            it.stack.damage = it.stack.damage.saturating_sub(off as u16);
            self.bench_ui.scrub_dirty = true;
        }
        if self.bench_ui.scrub_dirty && self.clock.time - self.bench_ui.sent > SCRUB_SYNC {
            self.bench_ui.scrub_dirty = false;
            self.bench_changed(p, None);
        }
        if let Some(at) = at {
            if self.rng.next() < dt * 25.0 {
                let (sky, blk) = self.terrain.world.light_estimate(at + Vec3::Y * 0.2);
                let jitter = Vec3::new(self.rng.next() - 0.5, 0.0, self.rng.next() - 0.5) * 0.06;
                self.level.particles.smoke_shaded(at + jitter + Vec3::Y * 0.02, 245, sky, blk);
            }
        }
    }
}
