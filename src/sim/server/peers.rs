//! The players on the server: their connections (the owner's in memory, LAN players' over
//! TCP), joining and leaving, and what they are told every tick: the others' poses, the
//! mobs, items and falling blocks near them, block changes, open containers, what lies on
//! crafting tables and in furnaces, and the time.

use super::Server;
use crate::entity::Furnace;
use crate::item::Slot;
use crate::lang::{t, tf};
use crate::net::{container, pose_flags, Conn, EntitySync, Frame, ItemNet, Msg, PlayerState, Pose, PROTOCOL};
use crate::save::rle;
use crate::world::*;
use glam::{IVec3, Vec3};
use std::path::PathBuf;
use std::sync::Arc;

/// Seconds a new connection has to say hello (`Msg::Hello`) before it is let go.
const HELLO_WAIT: f32 = 10.0;
/// How far around a player mobs are sent; items and falling blocks a bit less.
const MOB_RANGE: f32 = 96.0;
const ITEM_RANGE: f32 = 64.0;
/// `Msg::Open` at this height means the player closed their container.
pub(super) const CLOSED_Y: i32 = i32::MIN;
/// Chat colors.
const YELLOW: [u8; 4] = [255, 255, 85, 255];
pub(super) const WHITE: [u8; 4] = [255, 255, 255, 255];
pub(super) const RED: [u8; 4] = [255, 85, 85, 255];

/// A connected player.
pub(super) struct Peer {
    pub id: u8,
    pub name: String,
    pub conn: Conn,
    /// The world's owner (the game running this server): kept in the world's own files.
    pub owner: bool,
    pub joined: bool,
    /// Latest state (inventory, health...) from the player, kept between visits.
    pub state: Option<PlayerState>,
    pub pose: Option<Pose>,
    /// Block entity the player has open, and what was last sent of it.
    pub open: Option<IVec3>,
    pub sent_container: Option<Vec<u8>>,
    /// Chests others have open (their contents show in them), as this player last got them.
    pub seen_chests: FastMap<IVec3, Vec<u8>>,
    pub leaving: bool,
    /// Seconds since it connected (one that never says hello is let go).
    pub age: f32,
    /// The player's render distance (chunks): block changes farther away wait in
    /// `far_chunks` and the chunk goes whole when the player gets near.
    pub view: i32,
    pub far_chunks: FastSet<ChunkPos>,
    /// The mobs, items and falling blocks near the player as they last got them.
    pub entities: EntitySync,
    /// The most a bullet of this player's may do, and until when (server time).
    pub shot_damage: (f32, f32),
    /// The owner has the game paused.
    pub paused: bool,
}

impl Peer {
    fn new(id: u8, conn: Conn, owner: bool) -> Peer {
        Peer {
            id,
            name: String::new(),
            conn,
            owner,
            joined: false,
            state: None,
            pose: None,
            open: None,
            sent_container: None,
            seen_chests: FastMap::default(),
            leaving: false,
            age: 0.0,
            view: 12,
            far_chunks: FastSet::default(),
            entities: EntitySync::default(),
            shot_damage: (0.0, 0.0),
            paused: false,
        }
    }

    /// Its pose, if it is in the world and alive (not dead, not a spectator: mobs, items,
    /// beds and weapons leave those alone).
    pub fn alive_pose(&self) -> Option<Pose> {
        self.pose.filter(|p| self.joined && p.flags & pose_flags::DEAD == 0 && !p.spectator)
    }
}

/// Whether a player standing at `feet` with this render distance has chunk `c` loaded (or
/// nearly): block changes there go to them one by one.
fn chunk_in_view(feet: Vec3, c: ChunkPos, view: i32) -> bool {
    let at = World::chunk_pos(feet.x.floor() as i32, feet.z.floor() as i32);
    // (players unload chunks farther than their render distance + 3)
    let r = view + 4;
    (c.0 - at.0).pow(2) + (c.1 - at.1).pow(2) <= r * r
}

/// The items on a crafting table (everyone sees them lying on top of it).
fn table_msg(p: IVec3, grid: &[Slot; 9]) -> Msg {
    Msg::Container { p, kind: container::TABLE, slots: grid.to_vec() }
}

impl Server {
    /// The game running this server joins it, as the world's owner.
    pub(super) fn add_owner(&mut self, conn: Conn) {
        self.peers.push(Peer::new(0, conn, true));
    }

    /// The owner is gone (the game left the world): the server stops.
    pub(super) fn owner_left(&self) -> bool {
        !self.peers.iter().any(|p| p.owner)
    }

    pub(super) fn peer(&mut self, id: u8) -> Option<&mut Peer> {
        self.peers.iter_mut().find(|p| p.id == id)
    }

    pub fn player_name(&self, id: u8) -> Option<String> {
        self.peers.iter().find(|p| p.id == id && p.joined).map(|p| p.name.clone())
    }

    // ------------------------------------------------------------------ the LAN

    /// Opens the world to the LAN: players can join from other computers.
    pub(super) fn open_lan(&mut self, host: &str) -> Result<String, String> {
        if let Some(lan) = &self.lan {
            return Ok(lan_address(lan.port));
        }
        let lan = crate::net::Server::start(&self.meta.name, host).map_err(|e| e.to_string())?;
        let address = lan_address(lan.port);
        self.lan = Some(lan);
        Ok(address)
    }

    /// New LAN connections.
    pub(super) fn accept(&mut self) {
        let Some(lan) = &self.lan else { return };
        for stream in lan.accept() {
            let Ok(mut conn) = Conn::new(stream) else { continue };
            let Some(id) = (0..crate::world::textures::tex::CUSTOM_SKIN_SLOTS).find(|id| self.peers.iter().all(|p| p.id != *id)) else {
                conn.send(&Msg::Refuse(t("lan.full").to_string()));
                conn.close();
                continue;
            };
            self.peers.push(Peer::new(id, conn, false));
        }
    }

    /// What the players sent, handled; the ones gone are let go.
    pub(super) fn receive(&mut self) {
        let mut inbox = Vec::new();
        for p in self.peers.iter_mut() {
            let (msgs, open) = p.conn.poll();
            inbox.extend(msgs.into_iter().map(|m| (p.id, m)));
            if !open || (!p.joined && p.age > HELLO_WAIT) {
                p.leaving = true;
            }
        }
        for (id, m) in inbox {
            self.handle(id, m);
        }
        let gone: Vec<u8> = self.peers.iter().filter(|p| p.leaving).map(|p| p.id).collect();
        for id in gone {
            self.peer_left(id);
        }
        // The owner paused, alone in the world: it stands still.
        let others = self.peers.iter().any(|p| !p.owner && p.joined);
        self.paused = !others && self.peers.iter().any(|p| p.owner && p.paused);
    }

    // ------------------------------------------------------------------ sending

    pub fn send_to(&self, id: u8, m: &Msg) {
        if let Some(p) = self.peers.iter().find(|p| p.id == id && p.joined) {
            p.conn.send(m);
        }
    }

    pub(super) fn send_frame_to(&self, id: u8, f: &Frame) {
        if let Some(p) = self.peers.iter().find(|p| p.id == id && p.joined) {
            p.conn.send_frame(f);
        }
    }

    /// To every player (but `except`), encoded once.
    pub fn broadcast(&self, m: &Msg, except: Option<u8>) {
        if self.peers.iter().any(|p| p.joined && Some(p.id) != except) {
            self.broadcast_frame(&Frame::new(m), except);
        }
    }

    pub(super) fn broadcast_frame(&self, f: &Frame, except: Option<u8>) {
        for p in self.peers.iter().filter(|p| p.joined && Some(p.id) != except) {
            p.conn.send_frame(f);
        }
    }

    /// A chat line to everyone.
    pub fn announce(&self, text: String, color: [u8; 4]) {
        self.broadcast(&Msg::Chat { text, color }, None);
    }

    // ------------------------------------------------------------------ joining, leaving

    fn players_dir(&self) -> PathBuf {
        PathBuf::from("saves").join(&self.meta.folder).join("players")
    }

    fn player_file(&self, name: &str) -> PathBuf {
        let clean: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
        self.players_dir().join(format!("{}.dat", crate::util::windows_safe(clean)))
    }

    /// A LAN player's state kept from their last visit.
    fn load_player(&self, name: &str) -> Option<PlayerState> {
        let mut data = std::fs::read(self.player_file(name)).ok()?;
        if Msg::decode(&data).is_none() {
            // Saved before beds: the state ends where the bed would start (none).
            data.push(0);
        }
        match Msg::decode(&data)? {
            Msg::Save(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn save_player(&self, name: &str, state: &PlayerState) {
        if self.meta.folder.is_empty() {
            return;
        }
        let _ = std::fs::create_dir_all(self.players_dir());
        crate::save::write(self.player_file(name), &Msg::Save(state.clone()).encode());
    }

    /// A player said hello: they get the world (its edited chunks, what lies on its tables...)
    /// and the others are told; or they are refused (another version, a name taken).
    pub(super) fn welcome(&mut self, id: u8, proto: u16, name: String, view: u8) {
        let name: String = name.trim().chars().take(16).collect();
        let taken = self.peers.iter().any(|p| p.joined && p.name.eq_ignore_ascii_case(&name));
        let refuse = if proto != PROTOCOL {
            Some(t("lan.bad_version"))
        } else if name.is_empty() {
            Some(t("lan.bad_name"))
        } else if taken {
            Some(t("lan.name_taken"))
        } else {
            None
        };
        if let Some(reason) = refuse {
            if let Some(p) = self.peer(id) {
                p.conn.send(&Msg::Refuse(reason.to_string()));
                p.conn.close();
                p.leaving = true;
            }
            return;
        }
        let owner = self.peers.iter().any(|p| p.id == id && p.owner);
        let state = if owner { self.owner_state() } else { self.load_player(&name) };
        let welcome = Msg::Welcome {
            id,
            seed: self.meta.seed,
            world: self.meta.name.clone(),
            time: self.time_of_day,
            spawn: self.spawn,
            creative: self.meta.creative,
            cheats: self.cheats,
            state: state.clone(),
        };
        // The edited chunks; the rest the player generates from the seed. (Shared
        // copy-on-write: how they are now; the changes after go after them.)
        let world = &self.world;
        let chunks: Vec<(ChunkPos, Arc<ChunkData>)> = world
            .chunks
            .iter()
            .filter(|(p, _)| world.modified.contains(p))
            .chain(world.saved.iter())
            .map(|(p, c)| (*p, c.clone()))
            .collect();
        let mut others: Vec<Msg> = self
            .peers
            .iter()
            .filter(|p| p.joined)
            .map(|p| Msg::Join { id: p.id, name: p.name.clone() })
            .collect();
        others.extend(self.skins.iter().map(|(&id, png)| Msg::Skin { id, png: png.clone() }));
        others.extend(self.level.block_entities.benches.iter().map(|(p, b)| Msg::Bench { p: *p, bench: b.clone() }));
        others.extend(self.level.block_entities.tables.iter().map(|(p, grid)| table_msg(*p, grid)));
        others.extend(self.level.block_entities.furnaces.iter().map(|(p, f)| furnace_msg(*p, f)));
        // The cuts in trunks, and the trunks lying about.
        others.extend(self.world.notches.iter().map(|(p, n)| Msg::Notch { p: *p, notch: Some(*n) }));
        others.push(Msg::Logs(self.level.lying_logs.clone()));
        let Some(peer) = self.peer(id) else { return };
        peer.view = (view as i32).clamp(2, 64);
        peer.conn.send(&welcome);
        // The chunks are encoded on a worker thread (a big world has thousands); everything
        // sent to this player meanwhile waits for them.
        let streamed = peer.conn.stream().is_some_and(|stream| {
            std::thread::Builder::new()
                .name("net-welcome".into())
                .spawn(move || {
                    for (pos, c) in chunks {
                        let chunk = Msg::Chunk { pos, rle: rle(&c.to_vec()) };
                        if stream.send(Frame::new(&chunk)).is_err() {
                            return;
                        }
                    }
                    let _ = stream.send(Frame::new(&Msg::Ready));
                })
                .is_ok()
        });
        if !streamed {
            peer.conn.send(&Msg::Refuse(t("mp.lost").to_string()));
            peer.conn.close();
            peer.leaving = true;
            return;
        }
        for m in &others {
            peer.conn.send(m);
        }
        peer.name = name.clone();
        peer.state = state;
        peer.joined = true;
        self.broadcast(&Msg::Join { id, name: name.clone() }, Some(id));
        if !owner {
            self.announce(tf("lan.joined", &[&name]), YELLOW);
        }
    }

    /// A player left (or was let go): their state is kept, the others are told.
    pub(super) fn peer_left(&mut self, id: u8) {
        let Some(i) = self.peers.iter().position(|p| p.id == id) else { return };
        let peer = self.peers.remove(i);
        if !peer.joined {
            return;
        }
        if let Some(s) = &peer.state {
            if peer.owner {
                self.keep_owner_state(s);
            } else {
                self.save_player(&peer.name, s);
            }
        }
        self.skins.remove(&id);
        self.broadcast(&Msg::Leave { id }, None);
        if !peer.owner {
            self.announce(tf("lan.left", &[&peer.name]), YELLOW);
        }
    }

    /// Saves every player's state (with the world's autosave).
    pub(super) fn save_players(&mut self) {
        let states: Vec<(bool, String, PlayerState)> = self
            .peers
            .iter()
            .filter(|p| p.joined)
            .filter_map(|p| p.state.clone().map(|s| (p.owner, p.name.clone(), s)))
            .collect();
        for (owner, name, s) in states {
            if owner {
                self.keep_owner_state(&s);
            } else {
                self.save_player(&name, &s);
            }
        }
    }

    // ------------------------------------------------------------------ every tick

    /// What the players are told every tick.
    pub(super) fn sync(&mut self) {
        for p in self.peers.iter_mut() {
            p.age += crate::sim::clock::TICK_SECS;
        }
        // Block changes since the last tick, to everyone near them.
        self.resend_near_chunks();
        if let Some(log) = self.world.log.as_mut() {
            if !log.is_empty() {
                let changes = std::mem::take(log);
                self.send_blocks(&changes);
            }
        }
        // The time, once a second.
        self.synced += 1;
        if self.synced % 20 == 0 {
            self.broadcast(&Msg::Time(self.time_of_day), None);
        }
        let players: Vec<(u8, Option<Pose>, Option<IVec3>)> =
            self.peers.iter().filter(|p| p.joined).map(|p| (p.id, p.pose, p.open)).collect();
        let poses: Vec<(u8, Pose)> = players.iter().filter_map(|(id, p, _)| p.map(|p| (*id, p))).collect();
        for (id, pose, open) in &players {
            let others: Vec<(u8, Pose)> = poses.iter().filter(|(i, _)| i != id).copied().collect();
            self.send_to(*id, &Msg::Poses(others));
            if let Some(pose) = pose {
                self.send_entities(*id, pose.pos);
            }
            if let Some(p) = open {
                if let Some(msg) = self.container_msg(*p) {
                    let bytes = msg.encode();
                    if let Some(peer) = self.peer(*id) {
                        if peer.sent_container.as_ref() != Some(&bytes) {
                            peer.conn.send(&msg);
                            peer.sent_container = Some(bytes);
                        }
                    }
                }
            }
        }
        self.sync_tables(&players);
        self.sync_open_chests(&players);
        self.sync_furnaces();
    }

    /// The mobs, items and falling blocks near `at`, as changes since the player last got
    /// them.
    fn send_entities(&mut self, id: u8, at: Vec3) {
        let name = self.player_name(id);
        let mobs: Vec<crate::net::MobNet> = self
            .level
            .mobs
            .iter()
            .filter(|m| m.pos.distance(at) < MOB_RANGE)
            .map(|m| crate::net::MobNet { flags: m.wolf_flags(name.as_deref()), ..m.to_net() })
            .collect();
        let items: Vec<ItemNet> = self
            .level
            .items
            .iter()
            .filter(|it| it.pos.distance(at) < ITEM_RANGE)
            .map(|it| ItemNet { id: it.id, pos: it.pos, stack: it.stack, age: it.age })
            .collect();
        let falling: Vec<(Vec3, Block)> =
            self.level.falling.iter().filter(|f| f.pos.distance(at) < ITEM_RANGE).map(|f| (f.pos, f.block)).collect();
        let now = self.time as f32;
        if let Some(peer) = self.peer(id) {
            if let Some(msg) = peer.entities.update(now, &mobs, &items, &falling) {
                peer.conn.send(&msg);
            }
        }
    }

    /// Block changes to every player who has (or nearly has) their chunk loaded; for the
    /// others the chunk is marked, and goes whole when they come near (`resend_near_chunks`).
    /// A player whose pose is not known yet gets them all.
    fn send_blocks(&mut self, changes: &[(IVec3, Block)]) {
        let mut all: Option<Frame> = None;
        for peer in self.peers.iter_mut().filter(|p| p.joined) {
            let (pose, view) = (peer.pose, peer.view);
            let near = |p: IVec3| pose.is_none_or(|pose| chunk_in_view(pose.pos, World::chunk_pos(p.x, p.z), view));
            if changes.iter().all(|&(p, _)| near(p)) {
                let frame = all.get_or_insert_with(|| Frame::new(&Msg::Blocks(changes.to_vec())));
                peer.conn.send_frame(frame);
                continue;
            }
            let mut mine = Vec::new();
            for &(p, b) in changes {
                if near(p) {
                    mine.push((p, b));
                } else {
                    peer.far_chunks.insert(World::chunk_pos(p.x, p.z));
                }
            }
            if !mine.is_empty() {
                peer.conn.send(&Msg::Blocks(mine));
            }
        }
    }

    /// Chunks that changed while a player was far from them go whole, as they are now, once
    /// the player comes near.
    fn resend_near_chunks(&mut self) {
        let world = &self.world;
        for peer in self.peers.iter_mut().filter(|p| p.joined && !p.far_chunks.is_empty()) {
            let Some(pose) = peer.pose else { continue };
            let view = peer.view;
            let near: Vec<ChunkPos> = peer.far_chunks.iter().copied().filter(|&c| chunk_in_view(pose.pos, c, view)).collect();
            for c in near {
                peer.far_chunks.remove(&c);
                if let Some(data) = world.chunks.get(&c).or_else(|| world.saved.get(&c)) {
                    peer.conn.send(&Msg::Chunk { pos: c, rle: rle(&data.to_vec()) });
                }
            }
        }
    }

    /// Contents of the block entity at `p` as a message (a chest: both halves of a double
    /// one; a crafting table: its grid).
    pub(super) fn container_msg(&self, p: IVec3) -> Option<Msg> {
        let b = self.world.geti(p);
        let (kind, slots) = if is_chest(b) {
            self.level.block_entities.chests.get(&p)?;
            (container::CHEST, self.chest_slots(p))
        } else if b == CRAFTING_TABLE {
            let grid = self.level.block_entities.tables.get(&p).copied().unwrap_or([None; 9]);
            (container::TABLE, grid.to_vec())
        } else {
            return None;
        };
        Some(Msg::Container { p, kind, slots })
    }

    fn chest_slots(&self, p: IVec3) -> Vec<Slot> {
        let (a, b) = self.chest_halves(p);
        let get = |q: IVec3| self.level.block_entities.chests.get(&q).map_or([None; 27], |c| **c);
        let mut out = get(a).to_vec();
        if let Some(b) = b {
            out.extend(get(b));
        }
        out
    }

    /// Stores a container's contents (sent by the player who has it open).
    pub(super) fn apply_container(&mut self, p: IVec3, kind: u8, slots: &[Slot]) {
        let get = |i: usize| slots.get(i).copied().flatten();
        match kind {
            container::CHEST => {
                let (a, b) = self.chest_halves(p);
                let n = if b.is_some() { 54 } else { 27 };
                let all: Vec<Slot> = (0..n).map(get).collect();
                for (q, part) in std::iter::once(a).chain(b).zip(all.chunks(27)) {
                    let c = self.level.block_entities.chests.entry(q).or_insert_with(|| Box::new([None; 27]));
                    for (s, v) in c.iter_mut().zip(part) {
                        *s = *v;
                    }
                }
            }
            container::TABLE => {
                let grid: [Slot; 9] = std::array::from_fn(get);
                if grid.iter().any(|s| s.is_some()) {
                    self.level.block_entities.tables.insert(p, grid);
                } else {
                    self.level.block_entities.tables.remove(&p);
                }
            }
            _ => {}
        }
    }

    /// Crafting table grids that changed go to everyone (the items lie on top of the tables),
    /// but to a player who has that table open (they get it as their container).
    fn sync_tables(&mut self, players: &[(u8, Option<Pose>, Option<IVec3>)]) {
        let now = self.level.block_entities.tables.clone();
        let mut changed: Vec<(IVec3, [Slot; 9])> =
            now.iter().filter(|(p, g)| self.tables_sent.get(p) != Some(g)).map(|(p, g)| (*p, *g)).collect();
        changed.extend(self.tables_sent.keys().filter(|p| !now.contains_key(p)).map(|p| (*p, [None; 9])));
        self.tables_sent = now;
        for (p, grid) in changed {
            let frame = Frame::new(&table_msg(p, &grid));
            for (id, _, open) in players {
                if *open != Some(p) {
                    self.send_frame_to(*id, &frame);
                }
            }
        }
    }

    /// Furnaces that changed go to everyone (the meat on top cooks and turns over in front of
    /// all): at once when something is put in, taken out, turned, done or burnt; the seconds
    /// ticking on (which the players count themselves) only every second.
    fn sync_furnaces(&mut self) {
        let now = self.time as f32;
        let furnaces = &self.level.block_entities.furnaces;
        self.furnaces_sent.retain(|p, _| furnaces.contains_key(p));
        let mut send = Vec::new();
        for (p, f) in furnaces {
            let key = furnace_key(f);
            match self.furnaces_sent.get_mut(p) {
                Some((k, _, t)) if *k == key && now - *t < 1.0 => {}
                Some((k, m, t)) if *k == key => {
                    let full = furnace_msg(*p, f).encode();
                    *t = now;
                    if *m != full {
                        send.push(Frame::from_body(&full));
                        *m = full;
                    }
                }
                _ => {
                    let full = furnace_msg(*p, f).encode();
                    send.push(Frame::from_body(&full));
                    self.furnaces_sent.insert(*p, (key, full, now));
                }
            }
        }
        for frame in &send {
            self.broadcast_frame(frame, None);
        }
    }

    /// The contents of chests someone has open show in them for everyone, so players looking
    /// at a chest another player has open see it change as they go. (Whoever has it open
    /// gets it as their container.)
    fn sync_open_chests(&mut self, players: &[(u8, Option<Pose>, Option<IVec3>)]) {
        let mut chests: Vec<(IVec3, Msg)> = Vec::new();
        for p in players.iter().filter_map(|(_, _, o)| *o) {
            if !is_chest(self.world.geti(p)) {
                continue;
            }
            // Both halves of a double chest come in one message, keyed by its first half.
            let first = self.chest_halves(p).0;
            if chests.iter().any(|(q, _)| self.chest_halves(*q).0 == first) {
                continue;
            }
            if let Some(msg) = self.container_msg(p) {
                chests.push((p, msg));
            }
        }
        let encoded: Vec<(IVec3, Vec<u8>, Frame)> = chests
            .iter()
            .map(|(p, m)| {
                let bytes = m.encode();
                let frame = Frame::from_body(&bytes);
                (self.chest_halves(*p).0, bytes, frame)
            })
            .collect();
        for (id, _, own) in players {
            let own_first = own.filter(|q| is_chest(self.world.geti(*q))).map(|q| self.chest_halves(q).0);
            let Some(peer) = self.peer(*id) else { continue };
            // Forget chests that were closed, so opening them again sends them again.
            peer.seen_chests.retain(|q, _| encoded.iter().any(|(f, _, _)| f == q));
            for (first, bytes, frame) in &encoded {
                if own_first == Some(*first) {
                    continue;
                }
                if peer.seen_chests.get(first) != Some(bytes) {
                    peer.conn.send_frame(frame);
                    peer.seen_chests.insert(*first, bytes.clone());
                }
            }
        }
    }
}

fn lan_address(port: u16) -> String {
    let ip = crate::net::local_ip().map(|ip| ip.to_string()).unwrap_or_else(|| "127.0.0.1".into());
    format!("{ip}:{port}")
}

/// A furnace as everyone sees it.
pub(super) fn furnace_msg(p: IVec3, f: &Furnace) -> Msg {
    Msg::Furnace {
        p,
        burn: f.burn,
        cook: f.cook,
        input: f.input,
        fuel: f.fuel,
        output: f.output,
        grill: f.grill.iter().enumerate().filter_map(|(i, g)| g.map(|g| (i as u8, g))).collect(),
    }
}

/// What of a furnace changes all at once and must reach the players right away: its
/// contents, whether it burns, how done each side of the meat is and whether it is being
/// turned over. (The seconds in between go out now and then; players count them on.)
fn furnace_key(f: &Furnace) -> Vec<u8> {
    use crate::entity::block_entity::doneness;
    let contents = Furnace { burn: 0.0, cook: 0.0, grill: [None; 4], ..f.clone() };
    let mut key = furnace_msg(IVec3::ZERO, &contents).encode();
    key.push((f.burn > 0.0) as u8);
    for g in &f.grill {
        key.push(match g {
            None => 255,
            Some(g) => {
                let side = |t: f32| doneness(t) as u8;
                side(g.cook[0]) * 16 + side(g.cook[1]) * 4 + g.down * 2 + (g.flip > 0.0) as u8
            }
        });
    }
    key
}
