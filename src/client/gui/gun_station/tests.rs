//! The table's layout and animations checked without a window: things lie on the table and
//! not in each other, guns come apart and go back together, animations end where things lie.

use super::anim::{animation, event_length, scene};
use super::layout::{footprint, free_spot, free_spot_near, occupied, strip_targets};
use super::pick::{Pick, hidden_by_top};
use super::pieces::{Piece, corners, lying_pieces};
use super::table::{HALF_D, HALF_W, Table};
use crate::entity::{BenchEvent, BenchItem, GunBench, bench_event};
use crate::item::*;
use glam::{Vec2, Vec3};

fn table() -> Table {
    Table { center: Vec3::new(1.0, 65.0, 0.5), right: Vec3::X, toward: Vec3::Z, wide: 2.0, half_w: HALF_W }
}

fn gun(mods: u8) -> Stack {
    let mut g = Stack::one(PISTOL);
    set_gun_mods(&mut g, mods);
    set_gun_rounds(&mut g, 7);
    g.damage = 20;
    g
}

/// Where the pieces reach, on the table: across, up, toward the front.
fn extent(t: &Table, pieces: &[Piece]) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for pc in pieces {
        for c in pc.visible() {
            for q in corners(c, pc.mats[c.bone]) {
                let w = q - t.center;
                let q = Vec3::new(w.dot(t.right), w.y, w.dot(t.toward));
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
    }
    (lo, hi)
}

#[test]
fn the_top_hides_the_drawer_under_it() {
    let t = table();
    // Looking down onto the top's middle: something under the top's slab there is hidden,
    // what lies on the top is not.
    let o = t.center + Vec3::new(0.0, 1.0, 0.8);
    let d = (t.center - o).normalize();
    let top = (o - t.center).length();
    assert!(hidden_by_top(&t, o, d, top + 0.3));
    assert!(!hidden_by_top(&t, o, d, top - 0.02));
    // Out in front of the table, looking into the open drawer below the top's level: seen.
    let drawer = t.center + t.toward * 0.7 - Vec3::Y * 0.2;
    let o = drawer + Vec3::new(0.0, 0.6, 0.6);
    let d = (drawer - o).normalize();
    assert!(!hidden_by_top(&t, o, d, (drawer - o).length()));
}

#[test]
fn a_gun_lies_on_the_table_muzzle_to_the_right() {
    let t = table();
    let it = BenchItem { id: 1, stack: gun(gun_mod::SCOPE | gun_mod::SILENCER | gun_mod::LASER), x: 0.0, z: 0.0, turn: 0.0 };
    let pieces = lying_pieces(&t, &it).unwrap();
    let (lo, hi) = extent(&t, &pieces);
    assert!(lo.y.abs() < 0.01 && hi.y > 0.02, "{lo} {hi}");
    assert!(lo.x > -HALF_W && hi.x < HALF_W && lo.z > -HALF_D && hi.z < HALF_D, "{lo} {hi}");
    let (b, m) = crate::model::guns::pistol_view::muzzle(&crate::model::guns::pistol_view::PISTOL, gun_mod::SILENCER);
    let muzzle = pieces[0].mats[b].transform_point3(m) - t.center;
    assert!(muzzle.dot(t.right) > 0.1, "{muzzle}");
}

#[test]
fn a_gun_comes_apart_on_the_table_and_goes_back_together() {
    let t = table();
    let st = gun(gun_mod::SCOPE | gun_mod::LASER);
    let it = BenchItem { id: 1, stack: st, x: 0.5, z: 0.2, turn: 0.0 };
    // Near the table's corner: it all still ends up on it.
    let it = BenchItem { x: 0.8, z: 0.35, ..it };
    let (_, parts, _) = strip_targets(&t, &it);
    // Frame, barrel, spring, slide, and the magazine that was in it.
    assert_eq!(parts.len(), 5);
    let mut bench = GunBench::default();
    for (s, x, z, turn) in &parts {
        bench.add(*s, *x, *z, *turn);
    }
    // Lying there, every part rests on the table, all of it on it, none on another.
    let (pieces, _) = scene(&t, &bench, None);
    let boxes: Vec<(Vec3, Vec3)> = pieces.iter().map(|pc| extent(&t, std::slice::from_ref(pc))).collect();

    for (i, (lo, hi)) in boxes.iter().enumerate() {
        assert!(lo.y.abs() < 0.01, "{lo}");
        assert!(lo.x >= -HALF_W - 1e-3 && hi.x <= HALF_W + 1e-3 && lo.z >= -HALF_D - 1e-3 && hi.z <= HALF_D + 1e-3, "{i}: {lo} {hi}");
    }
    // Put together again: the same gun (its attachments back from its parts), as dirty,
    // with no magazine in it and nothing in the chamber.
    let back = assembled(GunKind::Pistol, &bench.items.iter().map(|i| i.stack).collect::<Vec<_>>());
    assert_eq!(gun_mods(&back), gun_mods(&st));
    assert_eq!(back.damage, st.damage);
    assert!(!gun_has_mag(&back) && !gun_chambered(&back));
}

#[test]
fn a_revolver_comes_apart_into_its_five_parts_and_goes_back_together() {
    let t = table();
    let mut st = Stack::one(REVOLVER);
    set_gun_rounds(&mut st, 4);
    st.damage = 30;
    let it = BenchItem { id: 1, stack: st, x: 0.0, z: 0.0, turn: 0.0 };
    let pieces = lying_pieces(&t, &it).unwrap();
    let (lo, hi) = extent(&t, &pieces);
    assert!(lo.y.abs() < 0.01 && hi.y > 0.02, "{lo} {hi}");
    let (parts, loose) = {
        let (_, parts, loose) = strip_targets(&t, &it);
        (parts, loose)
    };
    // Frame, barrel, mainspring, cylinder, hammer; its four live rounds back to the player.
    assert_eq!(parts.len(), 5);
    assert_eq!(loose, 4);
    let mut bench = GunBench::default();
    for (s, x, z, turn) in &parts {
        assert_eq!(s.damage, 30);
        bench.add(*s, *x, *z, *turn);
    }
    let (pieces, _) = scene(&t, &bench, None);
    for pc in &pieces {
        let (lo, hi) = extent(&t, std::slice::from_ref(pc));
        assert!(lo.y.abs() < 0.01, "{lo}");
        assert!(lo.x >= -HALF_W - 1e-3 && hi.x <= HALF_W + 1e-3 && lo.z >= -HALF_D - 1e-3 && hi.z <= HALF_D + 1e-3, "{lo} {hi}");
    }
    let back = assembled(GunKind::Revolver, &bench.items.iter().map(|i| i.stack).collect::<Vec<_>>());
    assert_eq!(back.item, REVOLVER);
    assert_eq!(back.damage, 30);
    assert_eq!(gun_rounds(&back), 0);
}

#[test]
fn nothing_is_laid_into_something_else() {
    let t = table();
    let mut bench = GunBench::default();
    let g = gun(0);
    bench.add(g, 0.0, 0.0, 0.0);
    // Another gun laid right on it goes beside it instead, on the table.
    let (x, z) = free_spot(&t, &bench, g, 0.05, 0.02, 0.0);
    bench.add(g, x, z, 0.0);
    let boxes = occupied(&t, &bench);
    let (a, b) = (boxes[0], boxes[1]);
    let apart = a.1.x <= b.0.x || b.1.x <= a.0.x || a.1.y <= b.0.y || b.1.y <= a.0.y;
    assert!(apart, "{a:?} {b:?}");
    assert!(t.on(x, z), "{x} {z}");
    // And a round laid where both are finds room too.
    let (rx, rz) = free_spot(&t, &bench, Stack::one(BULLET), 0.0, 0.0, 0.0);
    let r = footprint(&t, Stack::one(BULLET), 0.0);
    let r = (r.0 + Vec2::new(rx, rz), r.1 + Vec2::new(rx, rz));
    for (c, d) in occupied(&t, &bench) {
        assert!(r.1.x <= c.x || d.x <= r.0.x || r.1.y <= c.y || d.y <= r.0.y, "{r:?} in {c:?} {d:?}");
    }
}

#[test]
fn something_held_slides_along_what_is_in_its_way() {
    let t = table();
    let mut bench = GunBench::default();
    bench.add(gun(0), 0.0, 0.0, 0.0);
    let round = Stack::one(BULLET);
    for z in [-0.1, 0.0, 0.08] {
        // Moved across the gun in small steps: it goes round it (a jump or two, from one
        // side to the other), never back and forth.
        let mut last = free_spot(&t, &bench, round, -0.6, z, 0.0);
        let mut x = -0.6;
        let mut jumps = 0;
        while x < 0.6 {
            x += 0.004;
            let now = free_spot_near(&t, &bench, round, x, z, 0.0, Some(last));
            if Vec2::new(now.0 - last.0, now.1 - last.1).length() > 0.06 {
                jumps += 1;
            }
            last = now;
        }
        assert!(jumps <= 2, "{z}: {jumps} jumps");
    }
}

#[test]
fn a_magazine_shows_its_rounds_as_they_go_in() {
    let t = table();
    let mut bench = GunBench::default();
    let mut mag = Stack::one(PISTOL_MAGAZINE);
    set_gun_rounds(&mut mag, 5);
    let id = bench.add(mag, 0.0, 0.0, 0.0);
    let brass = |bench: &GunBench, time: Option<f32>| {
        let (pieces, _) = scene(&t, bench, time);
        pieces
            .iter()
            .filter(|pc| pc.pick == Some(Pick::Item(id)))
            .flat_map(|pc| pc.visible())
            .filter(|c| c.name.starts_with("witness_brass_"))
            .count()
    };
    let five = brass(&bench, None);
    set_gun_rounds(&mut bench.items[0].stack, 12);
    let full = brass(&bench, None);
    assert!(five > 0 && full > five, "{five} {full}");
    // Seven more going in: at first it still shows five, at the end twelve.
    bench.event = BenchEvent {
        serial: 1,
        kind: bench_event::LOAD,
        gun: id,
        bit: 7,
        gone: vec![BenchItem { id: 0, stack: Stack::new(BULLET, 7), x: 0.5, z: 0.2, turn: 0.0 }],
        made: Vec::new(),
    };
    assert_eq!(brass(&bench, Some(0.01)), five);
    assert_eq!(brass(&bench, Some(event_length(&bench.event) + 0.1)), full);
    // A round is on its way in the meantime.
    assert_eq!(animation(&t, &bench, 0.5).len(), 1);
}

#[test]
fn every_animation_ends_where_things_lie() {
    let t = table();
    let mut bench = GunBench::default();
    let st = gun(0);
    let id = bench.add(st, -0.3, 0.0, 0.0);
    // An attachment going on: at the end it is where it sits on the gun.
    let mut with = st;
    set_gun_mods(&mut with, gun_mod::SCOPE);
    bench.items[0].stack = with;
    bench.event = BenchEvent {
        serial: 1,
        kind: bench_event::FIT,
        gun: id,
        bit: gun_mod::SCOPE,
        gone: vec![BenchItem { id: 0, stack: Stack::one(SCOPE), x: 0.5, z: 0.3, turn: 0.0 }],
        made: Vec::new(),
    };
    let end = event_length(&bench.event) - 1e-3;
    let moving = animation(&t, &bench, end);
    let lying = lying_pieces(&t, &bench.items[0]).unwrap();
    let on = lying.iter().find(|pc| pc.pick == Some(Pick::Mod(id, gun_mod::SCOPE))).unwrap();
    let middle = |pc: &Piece| {
        let (lo, hi) = pc.reach();
        (lo + hi) * 0.5
    };
    assert!(middle(&moving[0]).distance(middle(on)) < 0.01);
}
