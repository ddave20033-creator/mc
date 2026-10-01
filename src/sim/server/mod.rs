//! The server: it runs a world (its blocks, fluids, items, mobs, machines, time) on a thread
//! of its own, 20 ticks a second, and the players play in it through connections. In single
//! player the game runs one for itself and joins it through a connection in memory
//! (`Conn::pair`); opened to the LAN, others join the same server over TCP. Every player is
//! the same to it: they tell it what they do (`Msg`), it applies that with the world's rules
//! and tells everyone what changed.
//!
//! Each player moves, fights and keeps their own inventory, health and hunger; the server
//! keeps them between visits (the world's owner's in the world's own files, the others' in
//! `players/`).

mod blocks;
mod checks;
mod chunks;
mod felling;
mod grenades;
mod handle;
mod items;
mod machines;
mod mobs;
mod peers;
mod save;
#[cfg(test)]
mod tests;

use crate::entity::mob::Mob;
use crate::entity::{BlockEntities, FallingBlock, ItemEntity};
use crate::net::{Conn, Msg};
use crate::world::save::{ChunkSaver, WorldMeta};
use crate::sim::clock::{Clock, TICK_SECS};
use crate::util::Rng;
use crate::world::fluid::Fluids;
use crate::world::gen::Generator;
use crate::world::*;
use glam::{IVec3, Vec3};
use peers::Peer;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Seconds between autosaves.
const AUTOSAVE_SECONDS: f32 = 60.0;
/// Seconds everyone has to be asleep before the morning comes (Minecraft: 100 ticks).
const SKIP_AFTER: f32 = 5.0;
/// Most players in a world: the host and up to 15 LAN guests (player ids 0..16).
const MAX_PLAYERS: u8 = 16;

/// What the world holds besides its blocks, on the server.
pub(crate) struct Level {
    pub items: Vec<ItemEntity>,
    pub falling: Vec<FallingBlock>,
    pub mobs: Vec<Mob>,
    /// Trees felled, falling over; the trunks lying on the ground; the last ids given.
    pub falling_trees: Vec<crate::sim::felling::FallingTree>,
    pub lying_logs: Vec<crate::sim::felling::LyingLog>,
    pub next_tree_id: u32,
    pub next_log_id: u32,
    /// Seconds until the next try to spawn animals near the players.
    pub mob_spawn_timer: f32,
    pub saplings: Vec<(IVec3, f32)>,
    /// Seconds until the next look round for stump marks to grow over.
    pub stump_scan: f32,
    pub block_entities: BlockEntities,
    /// Seconds each rifle station's magazine loader has been feeding the next round.
    pub loader_feed: FastMap<IVec3, f32>,
}

impl Level {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            falling: Vec::new(),
            mobs: Vec::new(),
            falling_trees: Vec::new(),
            lying_logs: Vec::new(),
            next_tree_id: 0,
            next_log_id: 0,
            mob_spawn_timer: 5.0,
            saplings: Vec::new(),
            stump_scan: 0.0,
            block_entities: BlockEntities::default(),
            loader_feed: FastMap::default(),
        }
    }
}

/// What the game tells its own server (besides the messages of its connection).
pub enum Control {
    /// Open the world to the LAN; the answer is the address, or why it could not.
    OpenLan(String, Sender<Result<String, String>>),
    /// Save everything and stop; the answer comes once it is on disk.
    Stop(Sender<()>),
}

/// The game's handle on the server it runs (the game joins it like any other player, through
/// the connection `start` gives with it): a way to tell it things, and its thread.
pub struct Local {
    control: Sender<Control>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Local {
    /// Opens the world to the LAN (see `Control::OpenLan`); waits for the answer.
    pub fn open_lan(&self, host_name: &str) -> Result<String, String> {
        let (tx, rx) = channel();
        self.control
            .send(Control::OpenLan(host_name.to_string(), tx))
            .map_err(|_| "server stopped".to_string())?;
        rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "no answer".to_string())?
    }

    /// Saves and stops the server, waiting (up to 20 s) until it has.
    pub fn stop(&mut self) {
        let (tx, rx) = channel();
        let stopped = self.control.send(Control::Stop(tx)).is_ok() && rx.recv_timeout(Duration::from_secs(20)).is_ok();
        // (a server that did not answer in time is left to finish on its own, not waited on
        // forever; one that is gone already is joined at once)
        if let Some(t) = self.thread.take() {
            if stopped || t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

impl Drop for Local {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts a server for the world `meta` (loaded from its folder) on a thread of its own; the
/// game joins it through the returned connection, as the world's owner.
pub fn start(meta: WorldMeta) -> (Local, Conn) {
    let (game_end, server_end) = Conn::pair();
    let (control, controls) = channel();
    let thread = std::thread::Builder::new()
        .name("server".into())
        .spawn(move || {
            let mut server = Server::load(meta);
            server.add_owner(server_end);
            // (an error in the world's rules stops the world, but not before it is saved and
            // everyone is told)
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.run(controls)));
            if ran.is_err() {
                server.crashed();
            }
        })
        .expect("start the server");
    (Local { control, thread: Some(thread) }, game_end)
}

impl Server {
    /// After a panic: everyone is told, and what can be saved is.
    fn crashed(&mut self) {
        for p in &self.peers {
            p.conn.send(&Msg::Refuse(crate::app::lang::t("server.crashed").to_string()));
        }
        let saved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.save_all();
            self.saver.wait();
        }));
        if saved.is_err() {
            eprintln!("the world could not be saved after the error");
        }
    }
}

pub(crate) struct Server {
    pub meta: WorldMeta,
    pub world: World,
    pub gen: Arc<Generator>,
    chunks: chunks::Loader,
    pub fluids: Fluids,
    pub level: Level,
    /// Grenades flying or lying about (the copies that decide).
    grenades: Vec<crate::sim::grenade::Grenade>,
    /// Stump marks in each loaded chunk (found when it loads, and as they are made).
    pub stump_marks: FastMap<ChunkPos, Vec<IVec3>>,
    /// Ticks the world has run and seconds it has run (they stop while it stands still), the
    /// ticks the players have been told what changed, the time of day (0..1), and how long all
    /// the players have been asleep.
    pub ticks: u64,
    pub time: f64,
    synced: u64,
    pub time_of_day: f32,
    pub asleep_for: f32,
    pub spawn: (i32, i32),
    pub cheats: bool,
    autosave: f32,
    saver: ChunkSaver,
    next_entity_id: u32,
    pub rng: Rng,
    // The players.
    peers: Vec<Peer>,
    lan: Option<crate::net::Server>,
    /// Crafting table grids and furnaces as the players last got them.
    tables_sent: FastMap<IVec3, [crate::item::Slot; 9]>,
    furnaces_sent: FastMap<IVec3, (Vec<u8>, Vec<u8>, f32)>,
    /// The owner has the game paused and nobody else is in: the world stands still.
    paused: bool,
    stop: Option<Sender<()>>,
}

impl Server {
    /// The world `meta`, loaded from its folder.
    fn load(meta: WorldMeta) -> Self {
        let gen = Arc::new(Generator::new(meta.seed));
        let mut world = World::new();
        world.log = Some(Vec::new());
        let spawn = meta.spawn.unwrap_or_else(|| gen.find_spawn());
        let mut server = Server {
            world,
            chunks: chunks::Loader::new(gen.clone()),
            gen,
            fluids: Fluids::new(),
            level: Level::new(),
            grenades: Vec::new(),
            stump_marks: FastMap::default(),
            ticks: 0,
            time: 0.0,
            synced: 0,
            time_of_day: meta.time_of_day,
            asleep_for: 0.0,
            spawn,
            cheats: meta.cheats,
            autosave: AUTOSAVE_SECONDS,
            saver: ChunkSaver::default(),
            next_entity_id: 0,
            rng: Rng::new(crate::world::save::now_secs() as u32 ^ meta.seed),
            peers: Vec::new(),
            lan: None,
            tables_sent: FastMap::default(),
            furnaces_sent: FastMap::default(),
            paused: false,
            stop: None,
            meta,
        };
        server.load_world();
        server
    }

    /// The server's loop: the connections' messages, the ticks due, the players told what
    /// changed; then asleep until the next tick.
    fn run(&mut self, controls: Receiver<Control>) {
        let mut clock = Clock::new(Instant::now());
        loop {
            while let Ok(c) = controls.try_recv() {
                self.control(c);
            }
            self.accept();
            self.receive();
            if self.owner_left() {
                break;
            }
            if self.stop.is_some() {
                // (the owner's last state, sent as they left, may still be on its way: until
                // their connection is closed, or a moment at most)
                let until = Instant::now() + Duration::from_secs(2);
                while !self.owner_left() && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                    self.receive();
                }
                break;
            }
            let due = clock.due(Instant::now());
            for _ in 0..due {
                if !self.paused {
                    self.tick();
                }
                self.sync();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        // (a tree still going over lands at once: its drops are not lost with it)
        for t in std::mem::take(&mut self.level.falling_trees) {
            self.land_now(t);
        }
        // Anyone else still in is told the world closed.
        for p in self.peers.iter().filter(|p| !p.owner && p.joined) {
            p.conn.send(&Msg::Refuse(crate::app::lang::t("lan.host_left").to_string()));
        }
        self.save_all();
        self.saver.wait();
        if let Some(done) = self.stop.take() {
            let _ = done.send(());
        }
    }

    fn control(&mut self, c: Control) {
        match c {
            Control::OpenLan(host, answer) => {
                let _ = answer.send(self.open_lan(&host));
            }
            Control::Stop(done) => self.stop = Some(done),
        }
    }

    pub fn random(&mut self) -> f32 {
        self.rng.next()
    }

    pub fn entity_id(&mut self) -> u32 {
        self.next_entity_id = self.next_entity_id.wrapping_add(1).max(1);
        self.next_entity_id
    }

    /// Where every living player stands (not spectators, not the dead).
    pub fn player_positions(&self) -> Vec<Vec3> {
        self.peers.iter().filter_map(|p| p.alive_pose()).map(|p| p.pos).collect()
    }

    /// A tick of the world.
    fn tick(&mut self) {
        let dt = TICK_SECS;
        self.ticks += 1;
        self.time = self.ticks as f64 * TICK_SECS as f64;
        let centers: Vec<ChunkPos> = self
            .peers
            .iter()
            .filter_map(|p| p.pose.map(|p| World::chunk_pos(p.pos.x.floor() as i32, p.pos.z.floor() as i32)))
            .collect();
        for p in self.chunks.update(&mut self.world, &centers) {
            self.chunk_loaded(p);
        }

        // Fluids
        let mut changed = Vec::new();
        self.fluids.update(dt, self.time as f32, &mut self.world, &mut changed);
        for (p, b) in std::mem::take(&mut self.fluids.broken) {
            let r = self.random();
            for s in crate::item::drops(b, crate::item::NONE, r) {
                self.spawn_drop(p.as_vec3() + Vec3::splat(0.5), s);
            }
        }

        self.update_furnaces(dt);
        self.grow_saplings(dt);
        self.update_stump_marks(dt);
        self.update_items(dt);
        self.update_loaders(dt);
        self.update_mobs(dt);
        self.spawn_animals(dt);
        self.update_falling_trees(dt);
        self.update_falling(dt);
        self.update_grenades(dt);

        self.time_of_day = crate::sim::advance_time(self.time_of_day, dt);
        self.update_sleepers(dt);
        self.autosave -= dt;
        if self.autosave <= 0.0 {
            self.autosave = AUTOSAVE_SECONDS;
            self.save_all();
        }
    }

    /// A chunk loaded (generated, or back from the edited ones): its fluids may still flow, and
    /// its stump marks are found.
    fn chunk_loaded(&mut self, p: ChunkPos) {
        self.fluids.wake_chunk(&self.world, p);
        let Some(c) = self.world.chunks.get(&p) else { return };
        let mut marks = Vec::new();
        for y in 0..=c.max_y as usize {
            for z in 0..16 {
                for (x, &b) in c.row(y, z).iter().enumerate() {
                    if is_stump_mark(b) {
                        marks.push(IVec3::new(p.0 * 16 + x as i32, y as i32, p.1 * 16 + z as i32));
                    }
                }
            }
        }
        if marks.is_empty() {
            self.stump_marks.remove(&p);
        } else {
            self.stump_marks.insert(p, marks);
        }
    }

    /// Saplings grow into trees. One whose tree would reach into a chunk not loaded waits
    /// (ready) until it is: the tree is neither cut off nor the sapling forgotten.
    fn grow_saplings(&mut self, dt: f32) {
        let mut grow = Vec::new();
        let world = &self.world;
        let area_loaded = |p: IVec3| [(-8, -8), (-8, 8), (8, -8), (8, 8)].iter().all(|&(dx, dz)| world.is_loaded(p.x + dx, p.z + dz));
        for (p, t) in self.level.saplings.iter_mut() {
            *t = (*t - dt).max(0.0);
            if *t <= 0.0 && area_loaded(*p) {
                grow.push(*p);
            }
        }
        for p in grow {
            self.level.saplings.retain(|(q, _)| *q != p);
            let b = self.world.geti(p);
            if is_sapling(b) && !self.grow_tree(p, b) {
                // No room yet: try again later.
                self.level.saplings.push((p, 30.0));
            }
        }
    }

    /// The night is skipped once every living player (spectators aside) has been asleep a
    /// while.
    fn update_sleepers(&mut self, dt: f32) {
        use crate::net::pose_flags::SLEEPING;
        let alive: Vec<_> = self.peers.iter().filter_map(|p| p.alive_pose()).collect();
        let asleep = alive.iter().filter(|p| p.flags & SLEEPING != 0).count();
        if alive.is_empty() || asleep < alive.len() || !crate::sim::is_night(self.time_of_day) {
            self.asleep_for = 0.0;
            return;
        }
        self.asleep_for += dt;
        if self.asleep_for >= SKIP_AFTER {
            self.asleep_for = 0.0;
            self.time_of_day = 0.0;
            self.broadcast(&Msg::Time(self.time_of_day), None);
        }
    }
}
