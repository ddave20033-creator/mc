//! Every message encoded and decoded back the same; chat lines cut.

use super::*;
use crate::net::{book, container, hurt, mode, pose_flags, status, PROTOCOL};

fn roundtrip(m: Msg) {
    let enc = m.encode();
    assert_eq!(Msg::decode(&enc), Some(m));
    // Cut short: never panics, just fails.
    for n in 0..enc.len() {
        assert!(Msg::decode(&enc[..n]).is_none() || n == enc.len());
    }
}

#[test]
fn messages_roundtrip() {
    let stack = Stack {
        item: 300,
        count: 1,
        damage: 12,
        data: 0x0305,
    };
    let state = PlayerState {
        pos: Vec3::new(1.0, 70.5, -3.25),
        yaw: 1.0,
        pitch: -0.2,
        health: 17.0,
        needs: [20.0, 5.0, 0.0, 18.0, 1.0, 0.0, 3.0],
        mode: mode::SPECTATOR,
        flying: false,
        slot: 3,
        inventory: vec![Some(stack), None, Some(Stack::new(3, 64))],
        bed: Some(IVec3::new(4, 70, -12)),
    };
    roundtrip(Msg::Hello {
        proto: PROTOCOL,
        name: "Albí".into(),
        view: 12,
    });
    roundtrip(Msg::Pose(Pose {
        pos: Vec3::ONE,
        held: 5,
        skin: 2,
        flags: pose_flags::HURT,
        status: status::TYPING,
        gun_mods: 0b1001,
        gun_state: 0x1c5,
        held_data: 0x0915,
        gun_extra: 0x2a_2d4b,
        armor: 0x1234,
        book: book::OPEN | 5,
        book_page: book::HUNGARIAN | 7,
        spectator: true,
        sprint: 0.5,
        rod: Some(crate::model::items::angler::RodAnim {
            charge: 0.25,
            cast: Some(0.1),
            out: true,
            fight: 1.0,
            tension: 0.6,
            crank: 2.0,
            lift: None,
            bobber: Some(Vec3::new(3.0, 60.5, -8.0)),
        }),
        chop: Some(crate::model::players::chop_rig::Swing { kind: crate::model::players::chop_rig::Kind::Stump, clock: 0.4, hit: Some(0.38) }),
        ..Default::default()
    }));
    roundtrip(Msg::Grenade {
        id: 1,
        kind: 1,
        pos: Vec3::new(3.0, 64.0, 1.0),
        vel: Vec3::new(10.0, 2.0, -4.0),
        seed: 0xdead_beef,
        fuse: 1.75,
    });
    roundtrip(Msg::Blast {
        pos: Vec3::new(3.0, 64.0, 1.0),
        seed: 7,
    });
    roundtrip(Msg::Hurt {
        dmg: 4.5,
        from: Vec3::ONE,
        knock: 0.4,
        kind: hurt::BULLET,
    });
    roundtrip(Msg::Shot {
        id: 3,
        kind: 4,
        mods: 2,
        eye: Vec3::new(1.0, 70.5, -3.0),
        seed: 0.25,
        bullets: vec![Vec3::X * 120.0, Vec3::new(0.1, 0.2, 119.0)],
    });
    roundtrip(Msg::Skin {
        id: 2,
        png: vec![137, 80, 78, 71, 0, 255],
    });
    roundtrip(Msg::Welcome {
        id: 2,
        seed: 99,
        world: "Új világ".into(),
        time: 0.3,
        spawn: (-5, 7),
        creative: false,
        cheats: true,
        state: Some(state.clone()),
    });
    roundtrip(Msg::Save(state));
    roundtrip(Msg::TakeDown { id: 78 });
    roundtrip(Msg::UseOnMob { id: 78, item: 380 });
    roundtrip(Msg::Blocks(vec![
        (IVec3::new(1, 2, 3), 7),
        (IVec3::new(-9, 0, 4), 0),
    ]));
    roundtrip(Msg::Entities {
        full: false,
        gone_mobs: vec![5, 6],
        gone_items: vec![],
        mobs: vec![MobNet {
            id: 4,
            kind: 0,
            pos: Vec3::X,
            body_yaw: 1.0,
            head_yaw: 1.2,
            pitch: 0.0,
            limb_swing: 3.0,
            limb_amount: 0.4,
            hurt: true,
            death: -1.0,
            health: 6.5,
            sheared: true,
            taken: 12.5,
            last_hit: 7.0,
            flags: 5,
            collar: 3,
        }],
        items: vec![ItemNet {
            id: 9,
            pos: Vec3::Y,
            stack,
            age: 2.0,
        }],
        falling: vec![(Vec3::Z, 4)],
    });
    roundtrip(Msg::Entities {
        full: true,
        mobs: vec![],
        items: vec![],
        gone_mobs: vec![],
        gone_items: vec![1, 2, 3],
        falling: vec![],
    });
    let mut bench = GunBench::default();
    let id = bench.add(stack, 0.4, -0.1, 1.2);
    bench.event = BenchEvent { serial: 3, kind: 1, gun: id, bit: 2, gone: bench.items.clone(), made: vec![5, 6] };
    roundtrip(Msg::Bench { p: IVec3::new(1, 2, 3), bench });
    roundtrip(Msg::Container {
        p: IVec3::new(3, 4, 5),
        kind: container::CHEST,
        slots: vec![None, Some(stack), None],
    });
    roundtrip(Msg::Furnace {
        p: IVec3::new(-3, 64, 9),
        burn: 12.5,
        cook: 4.0,
        input: None,
        fuel: Some(Stack::new(257, 7)),
        output: Some(Stack::new(259, 3)),
        grill: vec![(
            2,
            Grilled {
                raw: 268,
                cook: [10.5, 3.0],
                down: 1,
                flip: 0.2,
            },
        )],
    });
    roundtrip(Msg::FurnaceUse {
        p: IVec3::new(1, 2, 3),
        part: 3,
        take: true,
        offered: Some(Stack::one(268)),
    });
    roundtrip(Msg::Chat {
        text: "<Albi> szia".into(),
        color: [255, 255, 255, 255],
    });
}

#[test]
fn long_chat_lines_are_cut() {
    let text: String = "é".repeat(MAX_CHAT_CHARS + 50);
    let enc = Msg::Chat { text, color: [255; 4] }.encode();
    let Some(Msg::Chat { text, .. }) = Msg::decode(&enc) else {
        panic!("not decoded");
    };
    assert_eq!(text.chars().count(), MAX_CHAT_CHARS);
}
