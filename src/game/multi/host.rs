//! Hosting a LAN game: accepting players, applying what they do with the world's rules,
//! and sending everyone poses, entities, block changes and open containers 20 times a
//! second. Players' inventories and positions are kept between visits.

use super::checks::{self, known_item, valid_slot, valid_stack};
use super::*;

/// The items on a crafting table (everyone sees them lying on top of it).
fn table_msg(p: IVec3, grid: &[Slot; 9]) -> Msg {
    Msg::Container {
        p,
        kind: container::TABLE,
        slots: grid.to_vec(),
    }
}
/// Seconds a new connection has to say hello (`Msg::Hello`) before it is let go.
const HELLO_WAIT: f32 = 10.0;
/// Seconds after a shot its bullets may still hit (they fly a few hundred blocks at most).
const BULLET_TIME: f32 = 10.0;

/// Whether a player standing at `feet` with this render distance has chunk `c` loaded (or
/// nearly): block changes there go to them one by one.
fn chunk_in_view(feet: Vec3, c: ChunkPos, view: i32) -> bool {
    let at = World::chunk_pos(feet.x.floor() as i32, feet.z.floor() as i32);
    // (players unload chunks farther than their render distance + 3)
    let r = view + 4;
    (c.0 - at.0).pow(2) + (c.1 - at.1).pow(2) <= r * r
}

impl Game {
    pub(super) fn players_dir(&self) -> Option<PathBuf> {
        let meta = self.world_meta.as_ref()?;
        Some(PathBuf::from("saves").join(&meta.folder).join("players"))
    }

    pub(super) fn player_file(&self, name: &str) -> Option<PathBuf> {
        let clean: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let clean = crate::util::windows_safe(clean);
        Some(self.players_dir()?.join(format!("{clean}.dat")))
    }

    pub(super) fn load_player(&self, name: &str) -> Option<PlayerState> {
        let mut data = std::fs::read(self.player_file(name)?).ok()?;
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
        if let (Some(dir), Some(file)) = (self.players_dir(), self.player_file(name)) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(file, Msg::Save(state.clone()).encode());
        }
    }

    /// Host: saves every connected player's state (with the world's autosave).
    pub(in crate::game) fn save_peers(&self) {
        if let Some(Net::Host(h)) = &self.net {
            for p in h.peers.iter().filter(|p| p.joined) {
                if let Some(s) = &p.state {
                    self.save_player(&p.name, s);
                }
            }
        }
    }

    /// Pause menu: opens this world to the LAN.
    pub(in crate::game) fn open_to_lan(&mut self) {
        if self.net.is_some() || self.world_meta.is_none() {
            return;
        }
        let world = self
            .world_meta
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_default();
        match Server::start(&world, &self.settings.name) {
            Ok(server) => {
                let ip = crate::net::local_ip()
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|| "127.0.0.1".into());
                let address = format!("{ip}:{}", server.port);
                self.say(tf("lan.opened", &[&address]), chat::YELLOW);
                self.terrain.world.log = Some(Vec::new());
                self.net = Some(Net::Host(Host {
                    server,
                    peers: Vec::new(),
                    tick: 0.0,
                    time_tick: 0.0,
                    tables_sent: FastMap::default(),
                    furnaces_sent: FastMap::default(),
                    address,
                }));
                self.resume();
            }
            Err(e) => self.say(tf("lan.failed", &[&e]), chat::RED),
        }
    }

    /// Host: closes the LAN game (players are told why).
    pub(in crate::game) fn close_lan(&mut self) {
        if let Some(Net::Host(_)) = &self.net {
            self.save_peers();
            self.broadcast(&Msg::Refuse(t("lan.host_left").to_string()), None);
        }
        self.net = None;
        self.remotes.clear();
        self.terrain.world.log = None;
        self.terrain.extra_centers.clear();
    }

    pub(super) fn host(&mut self) -> Option<&mut Host> {
        match &mut self.net {
            Some(Net::Host(h)) => Some(h),
            _ => None,
        }
    }

    pub(super) fn peer(&mut self, id: u8) -> Option<&mut Peer> {
        self.host()?.peers.iter_mut().find(|p| p.id == id)
    }

    pub(super) fn host_tick(&mut self, dt: f32) {
        let Some(host) = self.host() else { return };
        // New connections.
        for stream in host.server.accept() {
            if let Ok(mut conn) = Conn::new(stream) {
                let Some(id) = (1..crate::world::textures::tex::CUSTOM_SKIN_SLOTS)
                    .find(|id| host.peers.iter().all(|p| p.id != *id))
                else {
                    conn.send(&Msg::Refuse(t("lan.full").to_string()));
                    conn.close();
                    continue;
                };
                host.peers.push(Peer {
                    id,
                    name: String::new(),
                    conn,
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
                    entity_bytes: [0; 2],
                    shot_damage: (0.0, 0.0),
                });
            }
        }
        // Messages.
        let mut inbox = Vec::new();
        for p in host.peers.iter_mut() {
            let (msgs, open) = p.conn.poll();
            inbox.extend(msgs.into_iter().map(|m| (p.id, m)));
            p.age += dt;
            if !open || (p.name.is_empty() && p.age > HELLO_WAIT) {
                p.leaving = true;
            }
        }
        for (id, m) in inbox {
            self.host_handle(id, m);
        }
        let gone: Vec<u8> = self
            .host()
            .map(|h| h.peers.iter().filter(|p| p.leaving).map(|p| p.id).collect())
            .unwrap_or_default();
        for id in gone {
            self.peer_left(id);
        }

        // Block changes since the last frame, to everyone near them.
        self.resend_near_chunks();
        if let Some(log) = self.terrain.world.log.as_mut() {
            if !log.is_empty() {
                let changes = std::mem::take(log);
                self.send_blocks(&changes);
            }
        }

        // Items the other players walk over.
        self.host_pickups();

        let Some(host) = self.host() else { return };
        host.tick += dt;
        host.time_tick += dt;
        if host.tick < TICK {
            return;
        }
        host.tick = 0.0;
        let send_time = host.time_tick >= 1.0;
        if send_time {
            host.time_tick = 0.0;
        }

        // Poses of everyone, entities near each player, open containers.
        let mut poses = vec![(HOST_ID, self.my_pose())];
        let peers: Vec<(u8, Option<Pose>, Option<IVec3>)> = self
            .host()
            .map(|h| {
                h.peers
                    .iter()
                    .filter(|p| p.joined)
                    .map(|p| (p.id, p.pose, p.open))
                    .collect()
            })
            .unwrap_or_default();
        poses.extend(peers.iter().filter_map(|(id, p, _)| p.map(|p| (*id, p))));
        let time = self.time_of_day;
        for (id, pose, open) in &peers {
            let others: Vec<(u8, Pose)> = poses.iter().filter(|(i, _)| i != id).copied().collect();
            self.send_to(*id, &Msg::Poses(others));
            if send_time {
                self.send_to(*id, &Msg::Time(time));
            }
            if let Some(pose) = pose {
                let at = pose.pos;
                let name = self.player_name(*id);
                let mobs: Vec<crate::net::MobNet> = self
                    .level.mobs
                    .iter()
                    .filter(|m| m.pos.distance(at) < MOB_RANGE)
                    .map(|m| crate::net::MobNet { flags: m.wolf_flags(name.as_deref()), ..m.to_net() })
                    .collect();
                let items: Vec<ItemNet> = self
                    .level.items
                    .iter()
                    .filter(|it| !it.is_picking_up() && it.pos.distance(at) < ITEM_RANGE)
                    .map(|it| ItemNet {
                        id: it.id,
                        pos: it.pos,
                        stack: it.stack,
                        age: it.age,
                    })
                    .collect();
                let falling: Vec<(Vec3, u8)> = self
                    .level.falling
                    .iter()
                    .filter(|f| f.pos.distance(at) < ITEM_RANGE)
                    .map(|f| (f.pos, f.block))
                    .collect();
                // Only what changed since this player last got it.
                let now = self.time;
                if let Some(peer) = self.peer(*id) {
                    let whole = crate::net::full_list_bytes(mobs.len(), items.len(), falling.len());
                    peer.entity_bytes[1] += whole as u64;
                    if let Some(msg) = peer.entities.update(now, &mobs, &items, &falling) {
                        let frame = Frame::new(&msg);
                        peer.entity_bytes[0] += frame.len() as u64;
                        peer.conn.send_frame(&frame);
                    }
                }
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
        self.sync_tables(&peers);
        self.sync_open_chests(&peers);
        self.sync_furnaces();
        // Keep the world loaded (and running) around the other players.
        self.terrain.extra_centers = peers
            .iter()
            .filter_map(|(_, p, _)| {
                p.map(|p| World::chunk_pos(p.pos.x.floor() as i32, p.pos.z.floor() as i32))
            })
            .collect();
    }

    /// Crafting table grids as everyone sees them: the stored ones, and the host's live grid
    /// while the host has a table open.
    pub(super) fn table_grids(&self) -> FastMap<IVec3, [Slot; 9]> {
        let mut grids = self.level.block_entities.tables.clone();
        if let Screen::Container(Container::Crafting(p)) = self.screen {
            if self.craft.iter().any(|s| s.is_some()) {
                grids.insert(p, self.craft);
            } else {
                grids.remove(&p);
            }
        }
        grids
    }

    /// Host: sends crafting table grids that changed to everyone (the items lie on top of
    /// the tables), except to a player who has that table open (they get it as a container).
    pub(super) fn sync_tables(&mut self, peers: &[(u8, Option<Pose>, Option<IVec3>)]) {
        let now = self.table_grids();
        let Some(host) = self.host() else { return };
        let mut changed: Vec<(IVec3, [Slot; 9])> = now
            .iter()
            .filter(|(p, g)| host.tables_sent.get(p) != Some(g))
            .map(|(p, g)| (*p, *g))
            .collect();
        changed.extend(
            host.tables_sent
                .keys()
                .filter(|p| !now.contains_key(p))
                .map(|p| (*p, [None; 9])),
        );
        host.tables_sent = now;
        for (p, grid) in changed {
            let frame = Frame::new(&table_msg(p, &grid));
            for (id, _, open) in peers {
                if *open != Some(p) {
                    self.send_frame_to(*id, &frame);
                }
            }
        }
    }

    /// Host: sends the furnaces that changed to everyone (the meat on top cooks and turns
    /// over in front of all players): at once when something is put in, taken out, turned,
    /// done or burnt; the seconds ticking on (which the players count themselves) only every
    /// second.
    pub(super) fn sync_furnaces(&mut self) {
        let now = self.time;
        let Some(Net::Host(host)) = &mut self.net else { return };
        let furnaces = &self.level.block_entities.furnaces;
        host.furnaces_sent.retain(|p, _| furnaces.contains_key(p));
        let mut send = Vec::new();
        for (p, f) in furnaces {
            let key = Self::furnace_key(f);
            match host.furnaces_sent.get_mut(p) {
                // Nothing that shows at once changed, and the seconds went out lately.
                Some((k, _, t)) if *k == key && now - *t < 1.0 => {}
                // The seconds, now and then (if they moved at all).
                Some((k, m, t)) if *k == key => {
                    let full = Self::furnace_msg(*p, f).encode();
                    *t = now;
                    if *m != full {
                        send.push(Frame::from_body(&full));
                        *m = full;
                    }
                }
                _ => {
                    let full = Self::furnace_msg(*p, f).encode();
                    send.push(Frame::from_body(&full));
                    host.furnaces_sent.insert(*p, (key, full, now));
                }
            }
        }
        for frame in &send {
            self.broadcast_frame(frame, None);
        }
    }

    /// Host: the contents of chests someone has open show in them for everyone, so players
    /// looking at a chest another player (or the host) has open see it change as they go.
    /// (Whoever has it open gets it as their container.)
    pub(super) fn sync_open_chests(&mut self, peers: &[(u8, Option<Pose>, Option<IVec3>)]) {
        let mut open: Vec<IVec3> = peers.iter().filter_map(|(_, _, o)| *o).collect();
        if let Screen::Container(Container::Chest(p)) = self.screen {
            open.push(p);
        }
        let mut chests: Vec<(IVec3, Msg)> = Vec::new();
        for p in open {
            if !is_chest(self.terrain.world.geti(p)) {
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
        for (id, _, own) in peers {
            let own_first = own
                .filter(|q| is_chest(self.terrain.world.geti(*q)))
                .map(|q| self.chest_halves(q).0);
            let Some(peer) = self.peer(*id) else { continue };
            // Forget chests that were closed, so opening them again sends them again.
            peer.seen_chests
                .retain(|q, _| encoded.iter().any(|(f, _, _)| f == q));
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

    /// Host: dropped items picked up by the other players.
    pub(super) fn host_pickups(&mut self) {
        let takers: Vec<(u8, Vec3)> = self
            .remotes
            .iter()
            .filter(|r| r.alive())
            .map(|r| (r.id, r.target.pos + Vec3::Y * 0.9))
            .collect();
        if takers.is_empty() {
            return;
        }
        let mut i = 0;
        while i < self.level.items.len() {
            let it = &self.level.items[i];
            let near = (it.pickup_delay <= 0.0 && !it.is_picking_up())
                .then(|| {
                    takers
                        .iter()
                        .find(|(_, c)| (it.pos + Vec3::Y * 0.2).distance(*c) < 1.5)
                })
                .flatten();
            if let Some(&(id, _)) = near {
                let it = self.level.items.swap_remove(i);
                self.send_to(id, &Msg::Give(it.stack));
                continue;
            }
            i += 1;
        }
    }

    /// Host: block changes to every player who has (or nearly has) their chunk loaded; for
    /// the others the chunk is marked, and goes whole when they come near
    /// (`resend_near_chunks`). A player whose pose is not known yet gets them all.
    pub(super) fn send_blocks(&mut self, changes: &[(IVec3, u8)]) {
        let Some(Net::Host(h)) = &mut self.net else { return };
        let mut all: Option<Frame> = None;
        for peer in h.peers.iter_mut().filter(|p| p.joined) {
            let (pose, view) = (peer.pose, peer.view);
            let near = |p: IVec3| {
                pose.is_none_or(|pose| chunk_in_view(pose.pos, World::chunk_pos(p.x, p.z), view))
            };
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

    /// Host: chunks that changed while a player was far from them go whole, as they are
    /// now, once the player comes near.
    pub(super) fn resend_near_chunks(&mut self) {
        let Some(Net::Host(h)) = &mut self.net else { return };
        let world = &self.terrain.world;
        for peer in h.peers.iter_mut().filter(|p| p.joined && !p.far_chunks.is_empty()) {
            let Some(pose) = peer.pose else { continue };
            let view = peer.view;
            let near: Vec<ChunkPos> = peer
                .far_chunks
                .iter()
                .copied()
                .filter(|&c| chunk_in_view(pose.pos, c, view))
                .collect();
            for c in near {
                peer.far_chunks.remove(&c);
                if let Some(data) = world.chunks.get(&c).or_else(|| world.saved.get(&c)) {
                    peer.conn.send(&Msg::Chunk {
                        pos: c,
                        rle: rle(data.raw()),
                    });
                }
            }
        }
    }

    /// Host: the most damage player `id` can deal now: with what they hold (a critical hit,
    /// a bullet of the gun) or with the bullets of their last shot (`melee`, `bullet`).
    fn damage_caps(&mut self, id: u8, held: ItemId) -> (f32, f32) {
        let now = self.time;
        let shot = self
            .peer(id)
            .map_or(0.0, |p| if now <= p.shot_damage.1 { p.shot_damage.0 } else { 0.0 });
        (checks::melee_cap(held), checks::gun_cap(held).max(shot))
    }

    pub(super) fn host_handle(&mut self, id: u8, m: Msg) {
        let joined = self.peer(id).is_some_and(|p| p.joined);
        if !joined {
            if let Msg::Hello { proto, name, view } = m {
                if let Some(p) = self.peer(id) {
                    p.view = (view as i32).clamp(2, 64);
                }
                self.host_welcome(id, proto, name);
            }
            return;
        }
        let pose = self.peer(id).and_then(|p| p.pose);
        let from = pose.map(|p| p.pos).unwrap_or(self.player.pos);
        // What a player does has to be where they stand (their latest pose: a player sends
        // it right before anything checked here).
        let feet = pose.map(|p| p.pos);
        let near_block = |p: IVec3| feet.is_some_and(|f| checks::block_near(f, p, checks::BLOCK_REACH));
        let near_hand = |at: Vec3| feet.is_some_and(|f| checks::point_near(f, at, checks::HAND_REACH));
        let held = pose.map_or(crate::item::NONE, |p| p.held);
        match m {
            Msg::Pose(mut pose) => {
                pose.skin = if pose.skin >= 4 {
                    4 + id
                } else {
                    pose.skin.min(3)
                };
                if !known_item(pose.held) {
                    pose.held = crate::item::NONE;
                }
                if let Some(p) = self.peer(id) {
                    p.pose = Some(pose);
                }
                self.set_remote_pose(id, pose);
            }
            Msg::Place { p, b } => {
                // (every u8 is a block id: only where it is is checked)
                if near_block(p) {
                    self.place_world(p, b);
                }
                // The player guessed the result; make sure it matches.
                let actual = self.terrain.world.geti(p);
                self.send_to(id, &Msg::Blocks(vec![(p, actual)]));
            }
            Msg::Break { p, held, creative } => {
                let b = self.terrain.world.geti(p);
                if b != AIR && near_block(p) && known_item(held) {
                    // (mined as in creative, without drops, only by a player in creative)
                    let creative = creative && pose.is_some_and(|p| p.flags & pose_flags::CREATIVE != 0);
                    self.break_world(p, held, creative);
                    self.break_fx(p, b, true, Some(id));
                }
                let actual = self.terrain.world.geti(p);
                self.send_to(id, &Msg::Blocks(vec![(p, actual)]));
            }
            Msg::AttackMob {
                id: mob,
                dmg,
                knock,
            } => {
                let (melee, bullet) = self.damage_caps(id, held);
                let Some((dmg, knock)) = checks::clamp_hit(dmg, knock, melee.max(bullet)) else {
                    return;
                };
                if let Some(m) = self.level.mobs.iter_mut().find(|m| m.id == mob) {
                    m.hurt(dmg, Some(from), knock);
                    self.attacked(crate::entity::mob::Foe::Mob(mob), id);
                }
            }
            Msg::AttackPlayer {
                id: target,
                dmg,
                knock,
                kind,
            } => {
                // (a player hits with their hand or their bullets; blasts and bites are the
                // host's own)
                let (melee, bullet) = self.damage_caps(id, held);
                let cap = match kind {
                    crate::net::hurt::MELEE => melee,
                    crate::net::hurt::BULLET => bullet,
                    _ => return,
                };
                let Some((dmg, knock)) = checks::clamp_hit(dmg, knock, cap) else {
                    return;
                };
                self.attacked(crate::entity::mob::Foe::Player(target), id);
                if target == HOST_ID {
                    self.hit_by_player(dmg, from, knock, kind);
                } else {
                    self.send_to(
                        target,
                        &Msg::Hurt {
                            dmg,
                            from,
                            knock,
                            kind,
                        },
                    );
                }
            }
            Msg::Grenade {
                kind,
                pos,
                vel,
                seed,
                fuse,
                ..
            } => {
                if kind <= 1 && near_hand(pos) {
                    self.remote_grenade(id, kind, pos, vel, seed, fuse.clamp(0.0, 10.0));
                }
            }
            Msg::SpawnMob { kind, pos } => {
                // A spawn egg puts it in front of the player; farther only by a command.
                let allowed = self.cheats || feet.is_some_and(|f| checks::point_near(f, pos, checks::SPAWN_REACH));
                if let (true, Some(kind)) = (allowed, MobKind::from_u8(kind)) {
                    self.spawn_mob(kind, pos);
                }
            }
            Msg::UseOnMob { id: mob, item } => {
                if !known_item(item) {
                    return;
                }
                if let Some(i) = self.level.mobs.iter().position(|m| m.id == mob) {
                    self.wolf_used(i, item, id);
                }
            }
            Msg::BreakDummy { id: mob } => {
                if let Some(i) = self.level.mobs.iter().position(|m| m.id == mob) {
                    self.break_dummy(i, true);
                }
            }
            Msg::Shear { id: mob } => {
                if let Some(i) = self.level.mobs.iter().position(|m| m.id == mob) {
                    self.shear_mob(i);
                }
            }
            Msg::DropItem {
                pos,
                vel,
                stack,
                delay,
            } => {
                if valid_stack(&stack) && near_hand(pos) {
                    self.add_item(ItemEntity::new(pos, vel, stack, delay.max(0.0)));
                }
            }
            Msg::Open { p } => {
                let open = if p.y == CLOSED_Y || !near_block(p) {
                    None
                } else {
                    // Make sure the block entity exists.
                    let b = self.terrain.world.geti(p);
                    if let Some(bench) = self.level.block_entities.benches.get(&p).filter(|_| is_gun_bench(b)) {
                        // What lies on the gun station, as it is now.
                        let msg = Msg::Bench { p, bench: bench.clone() };
                        self.send_to(id, &msg);
                    }
                    if is_chest(b) {
                        let (a, other) = self.chest_halves(p);
                        for q in std::iter::once(a).chain(other) {
                            self.level.block_entities
                                .chests
                                .entry(q)
                                .or_insert_with(|| Box::new([None; 27]));
                        }
                    }
                    Some(p)
                };
                if let Some(peer) = self.peer(id) {
                    peer.open = open;
                    peer.sent_container = None;
                }
            }
            Msg::FurnaceUse {
                p,
                part,
                take,
                offered,
            } => {
                if !valid_slot(&offered) {
                    return;
                }
                if near_block(p) {
                    self.remote_use_furnace(id, p, part, take, offered);
                } else if let Some(st) = offered {
                    // (too far: what they offered goes back)
                    self.send_to(id, &Msg::Give(st));
                }
            }
            Msg::Bench { p, bench } => {
                // A player changed what lies on a gun station: the others see it too.
                let valid = bench.items.iter().all(|i| valid_stack(&i.stack))
                    && bench.loader_mag.as_ref().is_none_or(valid_stack);
                if valid && near_block(p) && is_gun_bench(self.terrain.world.geti(p)) {
                    self.set_bench(p, bench.clone());
                    self.broadcast(&Msg::Bench { p, bench }, Some(id));
                }
            }
            Msg::Container { p, kind, slots } => {
                // Only into the container this player has open, and only items there are.
                let theirs = self.peer(id).is_some_and(|peer| peer.open == Some(p));
                let current = self.container_msg(p);
                let fits = matches!(&current, Some(Msg::Container { kind: k, slots: now, .. }) if *k == kind && now.len() == slots.len());
                if !theirs || !fits || !slots.iter().all(valid_slot) {
                    // (what they have is not what is there: the next tick sends it again)
                    if let Some(peer) = self.peer(id).filter(|peer| peer.open == Some(p)) {
                        peer.sent_container = None;
                    }
                    return;
                }
                // Only the slots this player changed (from what they last got) are taken, so
                // two players working in the same chest do not undo each other.
                let base = self
                    .peer(id)
                    .and_then(|peer| peer.sent_container.as_deref().and_then(Msg::decode));
                let merged = match (base, current) {
                    (
                        Some(Msg::Container {
                            p: bp,
                            kind: bk,
                            slots: before,
                        }),
                        Some(Msg::Container { slots: now, .. }),
                    ) if bp == p && bk == kind && before.len() == slots.len() => now
                        .iter()
                        .zip(&before)
                        .zip(&slots)
                        .map(|((cur, was), theirs)| if theirs != was { *theirs } else { *cur })
                        .collect(),
                    _ => slots.clone(),
                };
                self.apply_container(p, kind, &merged);
                // What the player has now: if the merge differs, the next tick sends it.
                let theirs = Msg::Container { p, kind, slots }.encode();
                if let Some(peer) = self.peer(id) {
                    peer.sent_container = Some(theirs);
                }
            }
            Msg::Chat { text, .. } => {
                let name = self.peer(id).map(|p| p.name.clone()).unwrap_or_default();
                self.announce(format!("<{name}> {text}"), chat::WHITE);
            }
            Msg::Command(line) => {
                // Only world-wide commands come here (the time), and only with cheats on.
                if self.cheats && line.starts_with("/time") {
                    self.run_command(&line);
                    self.broadcast(&Msg::Time(self.time_of_day), None);
                }
            }
            Msg::Save(mut state) => {
                for s in &mut state.inventory {
                    if !valid_slot(s) {
                        *s = None;
                    }
                }
                if let Some(p) = self.peer(id) {
                    p.state = Some(state);
                }
            }
            Msg::Shot {
                kind,
                mods,
                eye,
                seed,
                bullets,
                ..
            } => {
                let Some(gun) = crate::item::GUN_KINDS.get(kind as usize) else {
                    return;
                };
                if !near_hand(eye) {
                    return;
                }
                // Its bullets may hit for a while (even after the gun is put away).
                let (damage, until) = (gun.stats().damage, self.time + BULLET_TIME);
                if let Some(p) = self.peer(id) {
                    p.shot_damage = (damage, until);
                }
                // Shown here, and to everyone else as this player's.
                self.remote_shot(id, kind, mods, eye, seed, &bullets);
                let shot = Msg::Shot {
                    id,
                    kind,
                    mods,
                    eye,
                    seed,
                    bullets,
                };
                self.broadcast(&shot, Some(id));
            }
            Msg::Skin { png, .. } if self.set_skin_png(id, png.clone()).is_ok() => {
                self.broadcast(&Msg::Skin { id, png }, Some(id));
            }
            _ => {}
        }
    }

    pub(super) fn host_welcome(&mut self, id: u8, proto: u16, name: String) {
        let name: String = name.trim().chars().take(16).collect();
        let taken = name.eq_ignore_ascii_case(&self.settings.name)
            || self.host().is_some_and(|h| {
                h.peers
                    .iter()
                    .any(|p| p.joined && p.name.eq_ignore_ascii_case(&name))
            });
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

        let state = self.load_player(&name);
        let meta = self.world_meta.as_ref().unwrap();
        let welcome = Msg::Welcome {
            id,
            seed: meta.seed,
            world: meta.name.clone(),
            time: self.time_of_day,
            spawn: self.spawn,
            creative: meta.creative,
            cheats: self.cheats,
            state: state.clone(),
        };
        // The edited chunks; the rest the player generates from the seed. (Shared
        // copy-on-write: this is how they are now, whatever changes meanwhile; those changes
        // go after them.)
        let world = &self.terrain.world;
        let chunks: Vec<(ChunkPos, Arc<ChunkData>)> = world
            .chunks
            .iter()
            .filter(|(p, _)| world.modified.contains(p))
            .chain(world.saved.iter())
            .map(|(p, c)| (*p, c.clone()))
            .collect();
        let mut others = vec![Msg::Join {
            id: HOST_ID,
            name: self.settings.name.clone(),
        }];
        if let Some(h) = self.host() {
            others.extend(h.peers.iter().filter(|p| p.joined).map(|p| Msg::Join {
                id: p.id,
                name: p.name.clone(),
            }));
        }
        others.extend(self.skin_pngs.iter().map(|(&id, png)| Msg::Skin {
            id,
            png: png.clone(),
        }));
        // What lies on the gun stations.
        others.extend(
            self.level.block_entities
                .benches
                .iter()
                .map(|(p, b)| Msg::Bench { p: *p, bench: b.clone() }),
        );
        // Items lying on crafting tables, and the furnaces.
        others.extend(
            self.table_grids()
                .into_iter()
                .map(|(p, grid)| table_msg(p, &grid)),
        );
        others.extend(
            self.level.block_entities
                .furnaces
                .iter()
                .map(|(p, f)| Self::furnace_msg(*p, f)),
        );
        let Some(peer) = self.peer(id) else { return };
        peer.conn.send(&welcome);
        // The chunks are encoded on a worker thread (a big world has thousands: the host
        // would stop for a while); everything sent to this player meanwhile waits for them.
        let streamed = peer.conn.stream().is_some_and(|stream| {
            std::thread::Builder::new()
                .name("net-welcome".into())
                .spawn(move || {
                    for (pos, c) in chunks {
                        let chunk = Msg::Chunk {
                            pos,
                            rle: rle(c.raw()),
                        };
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
        self.broadcast(
            &Msg::Join {
                id,
                name: name.clone(),
            },
            Some(id),
        );
        self.add_remote(id, name.clone());
        self.announce(tf("lan.joined", &[&name]), chat::YELLOW);
    }

    pub(super) fn peer_left(&mut self, id: u8) {
        let Some(host) = self.host() else { return };
        let Some(i) = host.peers.iter().position(|p| p.id == id) else {
            return;
        };
        let peer = host.peers.remove(i);
        if !peer.joined {
            return;
        }
        if let Some(s) = &peer.state {
            self.save_player(&peer.name, s);
        }
        self.remove_remote(id);
        self.broadcast(&Msg::Leave { id }, None);
        self.announce(tf("lan.left", &[&peer.name]), chat::YELLOW);
    }
}
