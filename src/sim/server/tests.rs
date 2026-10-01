//! The server driven only through its messages, as a game would: joining, placing a block,
//! leaving, and the world coming back as it was.

use super::*;
use crate::net::{Pose, PROTOCOL};

/// A world folder of its own under the system's temp folder.
fn scratch_world(name: &str) -> WorldMeta {
    let folder = std::env::temp_dir().join(format!("yourworlds-server-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let _ = std::fs::create_dir_all(&folder);
    // (not `WorldMeta::create`: that makes a folder for it among the real saves)
    WorldMeta {
        folder: folder.to_string_lossy().into_owned(),
        name: "server test".into(),
        seed: 4242,
        creative: true,
        spectator: false,
        cheats: true,
        last_played: 0,
        time_of_day: 0.03,
        spawn: None,
        bed: None,
        player: None,
    }
}

/// Messages from the server until `want` says one is it (up to 20 s).
fn wait_for(conn: &Conn, mut want: impl FnMut(&Msg) -> bool) -> Vec<Msg> {
    let mut seen = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        let (msgs, open) = conn.poll();
        for m in msgs {
            let done = want(&m);
            seen.push(m);
            if done {
                return seen;
            }
        }
        assert!(open, "the server closed the connection");
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the server did not answer in time");
}

fn join(conn: &Conn) -> Vec<Msg> {
    conn.send(&Msg::Hello { proto: PROTOCOL, name: "Tester".into(), view: 8 });
    wait_for(conn, |m| matches!(m, Msg::Ready))
}

#[test]
fn a_block_placed_stays_after_the_world_is_left_and_played_again() {
    let meta = scratch_world("place");
    let at = IVec3::new(3, 150, 3);
    {
        let (mut local, conn) = start(meta.clone());
        let hello = join(&conn);
        assert!(hello.iter().any(|m| matches!(m, Msg::Welcome { seed: 4242, .. })));
        // Standing next to where it goes (a player may only place within reach).
        let pose = Pose { pos: Vec3::new(3.5, 148.0, 5.5), held: GLOWSTONE, ..Default::default() };
        conn.send(&Msg::Pose(pose));
        // (asked again until the chunks around the player have loaded and it is placed)
        let start = Instant::now();
        let mut placed = false;
        while !placed && start.elapsed() < Duration::from_secs(30) {
            conn.send(&Msg::Place { p: at, b: GLOWSTONE });
            std::thread::sleep(Duration::from_millis(200));
            let (msgs, open) = conn.poll();
            assert!(open, "the server closed the connection");
            placed = msgs.iter().any(|m| matches!(m, Msg::Blocks(list) if list.contains(&(at, GLOWSTONE))));
        }
        assert!(placed, "the block was placed");
        conn.send(&Msg::Save(crate::net::PlayerState {
            pos: pose.pos,
            yaw: 0.0,
            pitch: 0.0,
            health: 20.0,
            needs: crate::entity::survival::Needs::new().to_array(),
            mode: crate::net::mode::CREATIVE,
            flying: false,
            slot: 2,
            inventory: vec![Some(crate::item::Stack::one(crate::item::STICK))],
            bed: None,
        }));
        std::thread::sleep(Duration::from_millis(200));
        drop(conn);
        local.stop();
    }
    // Played again: the edited chunk comes with the world, the player where they were.
    let meta = WorldMeta::load(&meta.folder).expect("the world's level file");
    let (mut local, conn) = start(meta.clone());
    let hello = join(&conn);
    let state = hello.iter().find_map(|m| match m {
        Msg::Welcome { state, .. } => state.clone(),
        _ => None,
    });
    let state = state.expect("the owner's state is kept");
    assert_eq!(state.slot, 2);
    assert_eq!(state.inventory.first().copied().flatten().map(|s| s.item), Some(crate::item::STICK));
    let chunk = World::chunk_pos(at.x, at.z);
    let placed = hello.iter().any(|m| match m {
        Msg::Chunk { pos, rle } if *pos == chunk => {
            let c = ChunkData::from_vec(&crate::save::unrle(rle)).unwrap();
            c.get(at.x as usize, at.y as usize, at.z as usize) == GLOWSTONE
        }
        _ => false,
    });
    assert!(placed, "the edited chunk, with the glowstone in it");
    drop(conn);
    local.stop();
    let _ = std::fs::remove_dir_all(&meta.folder);
}
