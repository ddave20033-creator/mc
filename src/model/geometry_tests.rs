//! The CPU-built models, vertex for vertex: a hash of every vertex (its position, texture
//! point, layer, light and tint, bit for bit) that a representative set of them emits, so a
//! change to how they are built that should not change what is drawn can be checked.
//! The round log is hashed by its triangles that have an area (see `log_triangles`).

use crate::item::{icon, Icon, ItemId, Stack};
use crate::world::mesh::{flags, Vertex};
use glam::{IVec3, Mat4, Quat, Vec2, Vec3};

/// FNV-1a over the vertices' bits.
fn hash(verts: &[Vertex]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    for v in verts {
        for f in v.pos.iter().chain(&v.uv).chain(std::iter::once(&v.layer)) {
            f.to_bits().to_le_bytes().into_iter().for_each(&mut eat);
        }
        v.light.into_iter().chain(v.tint).for_each(&mut eat);
    }
    // (and how many there are)
    (verts.len() as u64).to_le_bytes().into_iter().for_each(&mut eat);
    h
}

/// The triangles with an area (no two corners at the same place), in order.
fn log_triangles(verts: &[Vertex]) -> Vec<Vertex> {
    assert_eq!(verts.len() % 3, 0);
    verts
        .chunks(3)
        .filter(|t| t[0].pos != t[1].pos && t[1].pos != t[2].pos && t[0].pos != t[2].pos)
        .flatten()
        .copied()
        .collect()
}

/// Somewhere turned, scaled and out of the origin, so every part of the transforms counts.
fn somewhere() -> Mat4 {
    Mat4::from_translation(Vec3::new(3.25, -1.5, 7.75))
        * Mat4::from_quat(Quat::from_euler(glam::EulerRot::YXZ, 0.7, -0.3, 0.2))
        * Mat4::from_scale(Vec3::splat(0.8))
}

const LIGHT: [u8; 4] = [200, 180, 40, 0];

fn is_log_item(item: ItemId) -> bool {
    matches!(icon(item), Icon::Block(b) if crate::world::is_log(b))
}

/// A bucket with a liquid in it drawn in the world animates it by the clock: left out.
fn clocked(item: ItemId, fl: u8) -> bool {
    fl != 0 && crate::model::bucket::Fill::of(item).is_some_and(|f| f != crate::model::bucket::Fill::Empty)
}

fn items() -> Vec<Vertex> {
    let mut out = Vec::new();
    // (in the order of their keys: their ids follow the items' table)
    let mut all = crate::item::all_items();
    all.push(crate::item::AMMO_BOX);
    all.sort_by_key(|&i| crate::item::key(i));
    let m = somewhere();
    for &item in &all {
        if is_log_item(item) {
            continue;
        }
        for fl in [0, flags::ENTITY, flags::VIEWMODEL] {
            if clocked(item, fl) {
                continue;
            }
            let st = Stack::one(item);
            crate::model::emit_held_data(&mut out, m, &st, LIGHT, fl);
            crate::model::emit_lying(&mut out, m, &st, LIGHT, fl);
            crate::model::emit_item_flat_or_block(&mut out, m, &st, 0.4, LIGHT, fl);
        }
    }
    crate::model::emit_torch(&mut out, m, LIGHT, 0, 17);
    crate::model::emit_flame(&mut out, m, flags::ENTITY, 9);
    crate::model::crack_overlay(&mut out, IVec3::new(4, -2, 9), 0.55);
    crate::model::emit_sprite_sides(&mut out, m, [5, 6], 7, LIGHT, flags::ENTITY);
    out
}

fn logs() -> Vec<Vertex> {
    let mut out = Vec::new();
    use crate::world::block::*;
    // (in the order they were numbered once, which the fingerprint follows)
    let all = [
        OAK_LOG, SPRUCE_LOG, BIRCH_LOG, OAK_LOG_X, OAK_LOG_Z, SPRUCE_LOG_X, SPRUCE_LOG_Z, BIRCH_LOG_X,
        BIRCH_LOG_Z, OAK_BRANCH, OAK_BRANCH_X, OAK_BRANCH_Z, BIRCH_BRANCH, BIRCH_BRANCH_X, BIRCH_BRANCH_Z,
        SPRUCE_BRANCH_X, SPRUCE_BRANCH_Z,
    ];
    assert_eq!(all.len(), (0..BLOCK_IDS as Block).filter(|&b| is_log(b)).count());
    for b in all {
        crate::model::emit_item(&mut out, somewhere(), b, LIGHT, flags::ENTITY);
    }
    out
}

fn player_pose() -> crate::model::player::PlayerPose {
    crate::model::player::PlayerPose {
        pos: Vec3::new(1.5, 2.0, -3.0),
        body_yaw: 0.4,
        head_yaw: 0.6,
        pitch: -0.2,
        limb_swing: 0.0,
        limb_amount: 0.0,
        attack: 0.0,
        crouch: 0.0,
        sprint: 0.0,
        held: crate::item::NONE,
        skin: 0,
        time: 1.25,
        hurt: false,
        first_person: false,
        burning: false,
        blocking: false,
        hide_arms: false,
        hide_right_arm: false,
        lantern: None,
        gun_mods: 0,
        gun_dirt: 0,
        held_data: 0,
        gun: Default::default(),
        armor: 0,
        book: None,
        grenade: None,
        rod: None,
        chop: None,
    }
}

fn players() -> Vec<Vertex> {
    use crate::model::player::{build_player, limb_targets, PlayerPose};
    let mut out = Vec::new();
    let mut glass = Vec::new();
    let variants: Vec<Box<dyn Fn(&mut PlayerPose)>> = vec![
        Box::new(|_| {}),
        Box::new(|p| {
            p.limb_swing = 2.3;
            p.limb_amount = 0.8;
            p.sprint = 1.0;
        }),
        Box::new(|p| {
            p.crouch = 1.0;
            p.limb_swing = 1.1;
            p.limb_amount = 0.5;
            p.hurt = true;
            p.skin = 1;
        }),
        Box::new(|p| {
            p.held = crate::item::PISTOL;
            p.armor = 0x0fff;
        }),
        Box::new(|p| {
            p.held = crate::world::LANTERN as ItemId;
            p.lantern = Some(Vec3::new(0.2, -1.0, 0.1));
            p.attack = 0.4;
        }),
        Box::new(|p| {
            p.held = crate::world::TORCH as ItemId;
            p.first_person = true;
            p.burning = true;
        }),
        Box::new(|p| {
            p.held = crate::item::BUCKET;
            p.hide_right_arm = true;
        }),
        Box::new(|p| {
            p.held = crate::item::FISHING_ROD;
            p.rod = Some(Default::default());
        }),
        Box::new(|p| {
            p.chop = Some(crate::model::chop_rig::Swing { kind: crate::model::chop_rig::Kind::Chop, clock: 0.3, hit: None });
            p.armor = 0x0fff;
        }),
        Box::new(|p| {
            p.chop = Some(crate::model::chop_rig::Swing { kind: crate::model::chop_rig::Kind::Stump, clock: 0.5, hit: Some(0.4) });
            p.first_person = true;
            p.burning = true;
        }),
    ];
    for v in &variants {
        let mut p = player_pose();
        v(&mut p);
        let limbs = limb_targets(&p);
        build_player(&mut out, &mut glass, &p, &limbs, 12, 3);
    }
    // First-person chop arms.
    let swing = crate::model::chop_rig::Swing { kind: crate::model::chop_rig::Kind::Chop, clock: 0.35, hit: None };
    crate::model::chop_rig::emit(
        &mut out,
        somewhere(),
        &swing.pose(),
        crate::model::chop_rig::Parts::Arms,
        crate::item::NONE,
        0,
        [255; 3],
        0,
        0.0,
        LIGHT,
        flags::VIEWMODEL,
    );
    out.extend(glass);
    out
}

fn hands() -> Vec<Vertex> {
    use crate::model::hand::HandAnim;
    let mut all = Vec::new();
    let setups: Vec<(ItemId, Box<dyn Fn(&mut HandAnim, usize)>)> = vec![
        (crate::item::NONE, Box::new(|h, i| if i == 5 { h.swing() })),
        (crate::item::PISTOL, Box::new(|h, _| h.aim = 0.5)),
        (crate::item::REVOLVER, Box::new(|_, _| {})),
        (crate::world::LANTERN as ItemId, Box::new(|h, _| h.fancy_lantern = true)),
        (crate::item::BUCKET, Box::new(|_, _| {})),
        (crate::item::FRAG_GRENADE, Box::new(|h, i| h.grenade = Some((i as f32 * 0.03, 0.5)))),
        (crate::item::FRAG_GRENADE, Box::new(|h, i| if i == 10 { h.throw() })),
        (crate::item::FISHING_ROD, Box::new(|h, _| h.rod = Some(Default::default()))),
        (crate::world::TORCH as ItemId, Box::new(|h, i| if i == 3 { h.swing() })),
        (crate::world::STONE as ItemId, Box::new(|_, _| {})),
    ];
    let cam = somewhere();
    for (item, setup) in &setups {
        let mut h = HandAnim::new();
        h.equip(*item);
        let mut out = Vec::new();
        for i in 0..20 {
            setup(&mut h, i);
            h.update(1.0 / 60.0, false, 2.0, true, Vec2::new(3.0, -1.0));
            out.clear();
            h.build(&mut out, cam, 12, 4, 1.0 / 60.0, 1);
        }
        all.extend(out);
        all.extend(h.glass.iter().copied());
    }
    all
}

fn block_entities() -> Vec<Vertex> {
    use crate::entity::block_entity::*;
    let mut out = Vec::new();
    let p = IVec3::new(5, 64, -7);
    for facing in 0..4u8 {
        for side in [-1, 0, 1] {
            build_chest_lid(&mut out, p, facing, side, 0.6, 12, 3);
        }
    }
    for b in crate::world::OAK_DOOR..crate::world::OAK_DOOR + 64 {
        build_door(&mut out, p, b, 0.35, 12, 3);
    }
    let mut slots: Vec<crate::item::Slot> = vec![None; 27];
    for (i, item) in [crate::world::STONE as ItemId, crate::item::PISTOL, crate::world::TORCH as ItemId, crate::item::BUCKET, crate::item::COAL]
        .into_iter()
        .enumerate()
    {
        slots[i * 4] = Some(Stack { count: 3, ..Stack::one(item) });
    }
    build_chest_items(&mut out, p, 2, 0, &slots, Some(4), 12, 3);
    let grid: [crate::item::Slot; 9] = std::array::from_fn(|i| slots[i * 2]);
    build_table_items(&mut out, p, 1, &grid, Some(0), 12, 3);
    build_table_made(&mut out, p, 1, &Stack::one(crate::item::COAL), &grid, 0.2, true, 12, 3);
    let mut f = Furnace {
        input: Some(Stack { count: 5, ..Stack::one(crate::world::IRON_ORE as ItemId) }),
        fuel: Some(Stack { count: 4, ..Stack::one(crate::item::COAL) }),
        burn: 3.0,
        burn_total: 8.0,
        cook: 2.0,
        ..Default::default()
    };
    f.grill[1] = Some(Grilled { raw: crate::item::PORKCHOP, cook: [3.0, 1.0], down: 1, flip: 0.2 });
    build_furnace_items(&mut out, p, 3, &f, 1.5, 12, 3, [255, 200, 100, 0], true);
    f.fuel = Some(Stack::one(crate::item::LAVA_BUCKET));
    build_furnace_items(&mut out, p, 0, &f, 0.5, 12, 3, [255, 200, 100, 0], false);
    build_glow(&mut out, [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 0.2, 1.0), Vec3::Z]);
    out
}

fn mobs() -> Vec<Vertex> {
    use crate::content::mobs::{pig::PIG, sheep::SHEEP, wolf::WOLF, MobState, TARGET_DUMMY};
    use crate::entity::mob::Mob;
    let mut out = Vec::new();
    for kind in [PIG, SHEEP, TARGET_DUMMY, WOLF] {
        let mut m = Mob::new(kind, Vec3::new(2.0, 70.0, -4.0), 0.8, 11);
        m.build(&mut out, 12, 3);
        match &mut m.state {
            MobState::Sheep(w) => w.sheared = true,
            MobState::Wolf(p) => {
                p.owner = Some("x".into());
                p.sitting = true;
                p.collar = 3;
            }
            _ => {}
        }
        m.build(&mut out, 9, 1);
    }
    out
}

fn lanterns_and_buckets() -> Vec<Vertex> {
    use crate::model::bucket;
    use crate::model::lantern::{emit_held_lantern, emit_lantern, LanternKind, FIRST_PERSON, ON_MODEL};
    let mut out = Vec::new();
    for kind in [LanternKind::Standing, LanternKind::Hanging, LanternKind::Held(1.5)] {
        emit_lantern(&mut out, somewhere(), LIGHT, flags::ENTITY, kind);
    }
    emit_held_lantern(&mut out, ON_MODEL, Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.2, -1.0, 0.1).normalize(), 0.7, LIGHT, 0);
    emit_held_lantern(&mut out, FIRST_PERSON, Vec3::new(1.0, 2.0, 3.0), Vec3::NEG_Y, -0.3, LIGHT, flags::VIEWMODEL);
    for fill in [bucket::Fill::Empty, bucket::Fill::Water, bucket::Fill::Lava] {
        for handle in [0.0, 0.55, 1.0] {
            bucket::emit(&mut out, somewhere(), fill, &bucket::Surface::still(true), handle, LIGHT, 0);
        }
        bucket::emit(&mut out, somewhere() * Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)), fill, &bucket::Surface::still(false), 0.3, LIGHT, 0);
    }
    out
}

fn gun_stations() -> Vec<Vertex> {
    use crate::model::gun_station::{emit_block, emit_crate, emit_item, Loader};
    let mut out = Vec::new();
    let p = IVec3::new(3, 60, 8);
    for rifle in [false, true] {
        emit_block(&mut out, rifle, p, Vec3::new(0.3, 0.0, -1.0), 0.7, true, [Some(40), None, Some(3)], Loader { there: true, feed: Some(0.3) }, true, LIGHT, flags::ENTITY);
        emit_item(&mut out, rifle, somewhere(), LIGHT, 0);
    }
    emit_crate(&mut out, p, Vec3::X, [5, 9], LIGHT, flags::ENTITY);
    out
}

fn gun_fx() -> Vec<Vertex> {
    use crate::model::ballistics::{emit_laser_dot, emit_muzzle_flash, emit_tracer};
    let mut out = Vec::new();
    let cam = Vec3::new(0.5, 1.7, 2.0);
    for laser in [false, true] {
        emit_tracer(&mut out, Vec3::new(1.0, 1.5, -2.0), Vec3::new(9.0, 2.5, -30.0), cam, 0.02, laser);
    }
    emit_laser_dot(&mut out, Vec3::new(3.0, 1.0, -8.0), Vec3::X, Vec3::Y, 0.05);
    for (seed, k) in [(0.1, 1.0), (0.7, 0.4)] {
        emit_muzzle_flash(&mut out, Vec3::new(0.3, 1.4, -1.0), Vec3::new(0.1, 0.05, -1.0), cam, 0.3, seed, k);
    }
    out
}

fn groups() -> Vec<(&'static str, u64)> {
    vec![
        ("items", hash(&items())),
        ("logs", hash(&log_triangles(&logs()))),
        ("players", hash(&players())),
        ("hands", hash(&hands())),
        ("block_entities", hash(&block_entities())),
        ("mobs", hash(&mobs())),
        ("lanterns_buckets", hash(&lanterns_and_buckets())),
        ("gun_stations", hash(&gun_stations())),
        ("gun_fx", hash(&gun_fx())),
    ]
}

/// The hashes as the models were before their shared pieces were merged into `prim`.
const EXPECTED: [(&str, u64); 9] = [
    ("items", 0xfa1c1547f1b81e83),
    ("logs", 0x0660b16dd6a12110),
    ("players", 0xe3ca5a17bd4b7933),
    ("hands", 0xc61a12322bfd8c68),
    ("block_entities", 0xc217f7d52ef0b0e4),
    ("mobs", 0x1083b750e4ad0fa2),
    ("lanterns_buckets", 0x0187d9767c083298),
    ("gun_stations", 0x0d93fe35725e60aa),
    ("gun_fx", 0xbd26d53969799bed),
];

#[test]
fn models_are_built_vertex_for_vertex_as_before() {
    // The flat items' side walls follow their textures' opaque pixels.
    crate::textures::generate(&crate::textures::resource_pack::Packs::none());
    for (name, n) in [("items", items().len()), ("logs", logs().len()), ("players", players().len()), ("hands", hands().len())] {
        println!("{name}: {n} vertices");
        assert!(n > 0);
    }
    let got = groups();
    // (the same twice: nothing in them depends on the clock)
    assert_eq!(got, groups());
    for (name, h) in &got {
        println!("(\"{name}\", 0x{h:016x}),");
    }
    for ((name, h), (want_name, want)) in got.iter().zip(EXPECTED) {
        assert_eq!(*name, want_name);
        assert_eq!(*h, want, "{name} changed");
    }
}
