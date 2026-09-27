//! Hosting a LAN game: accepting players, applying what they do with the world's rules,
//! and sending everyone poses, entities, block changes and open containers 20 times a
//! second. Players' inventories and positions are kept between visits.

use super::*;

/// The items on a crafting table (everyone sees them lying on top of it).
fn table_msg(p: IVec3, grid: &[Slot; 9]) -> Msg {
    Msg::Container {
        p,
        kind: container::TABLE,
        slots: grid.to_vec(),
        burn: 0.0,
        burn_total: 0.0,
        cook: 0.0,
    }
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
                // Existing mobs and items get ids for the players.
                for i in 0..self.mobs.len() {
                    if self.mobs[i].id == 0 {
                        self.mobs[i].id = self.entity_id();
                    }
                }
                for i in 0..self.items.len() {
                    if self.items[i].id == 0 {
                        self.items[i].id = self.entity_id();
                    }
                }
                self.net = Some(Net::Host(Host {
                    server,
                    peers: Vec::new(),
                    tick: 0.0,
                    time_tick: 0.0,
                    tables_sent: FastMap::default(),
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
                    conn.send(&Msg::Refuse("A LAN jatek megtelt.".into()));
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
                    leaving: false,
                });
            }
        }
        // Messages.
        let mut inbox = Vec::new();
        for p in host.peers.iter_mut() {
            let (msgs, open) = p.conn.poll();
            inbox.extend(msgs.into_iter().map(|m| (p.id, m)));
            if !open {
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

        // Block changes since the last frame, to everyone.
        if let Some(log) = self.terrain.world.log.as_mut() {
            if !log.is_empty() {
                let changes = std::mem::take(log);
                self.broadcast(&Msg::Blocks(changes), None);
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
                let mobs = self
                    .mobs
                    .iter()
                    .filter(|m| m.pos.distance(at) < MOB_RANGE)
                    .map(|m| m.to_net())
                    .collect();
                let items = self
                    .items
                    .iter()
                    .filter(|it| !it.is_picking_up() && it.pos.distance(at) < ITEM_RANGE)
                    .map(|it| ItemNet {
                        id: it.id,
                        pos: it.pos,
                        stack: it.stack,
                        age: it.age,
                    })
                    .collect();
                let falling = self
                    .falling
                    .iter()
                    .filter(|f| f.pos.distance(at) < ITEM_RANGE)
                    .map(|f| (f.pos, f.block))
                    .collect();
                self.send_to(
                    *id,
                    &Msg::Entities {
                        mobs,
                        items,
                        falling,
                    },
                );
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
        let mut grids = self.block_entities.tables.clone();
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
            let msg = table_msg(p, &grid);
            for (id, _, open) in peers {
                if *open != Some(p) {
                    self.send_to(*id, &msg);
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
        while i < self.items.len() {
            let it = &self.items[i];
            let near = (it.pickup_delay <= 0.0 && !it.is_picking_up())
                .then(|| {
                    takers
                        .iter()
                        .find(|(_, c)| (it.pos + Vec3::Y * 0.2).distance(*c) < 1.5)
                })
                .flatten();
            if let Some(&(id, _)) = near {
                let it = self.items.swap_remove(i);
                self.send_to(id, &Msg::Give(it.stack));
                continue;
            }
            i += 1;
        }
    }

    pub(super) fn host_handle(&mut self, id: u8, m: Msg) {
        let joined = self.peer(id).is_some_and(|p| p.joined);
        if !joined {
            if let Msg::Hello { proto, name } = m {
                self.host_welcome(id, proto, name);
            }
            return;
        }
        let from = self
            .peer(id)
            .and_then(|p| p.pose)
            .map(|p| p.pos)
            .unwrap_or(self.player.pos);
        match m {
            Msg::Pose(mut pose) => {
                pose.skin = if pose.skin >= 4 {
                    4 + id
                } else {
                    pose.skin.min(3)
                };
                if let Some(p) = self.peer(id) {
                    p.pose = Some(pose);
                }
                self.set_remote_pose(id, pose);
            }
            Msg::Place { p, b } => {
                self.place_world(p, b);
                // The player guessed the result; make sure it matches.
                let actual = self.terrain.world.geti(p);
                self.send_to(id, &Msg::Blocks(vec![(p, actual)]));
            }
            Msg::Break { p, held, creative } => {
                let b = self.terrain.world.geti(p);
                if b != AIR {
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
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) {
                    m.hurt(dmg, Some(from), knock);
                }
            }
            Msg::AttackPlayer {
                id: target,
                dmg,
                knock,
            } => {
                if target == HOST_ID {
                    self.hit_by_player(dmg, from, knock);
                } else {
                    self.send_to(target, &Msg::Hurt { dmg, from, knock });
                }
            }
            Msg::SpawnMob { kind, pos } => {
                if let Some(kind) = MobKind::from_u8(kind) {
                    self.spawn_mob(kind, pos);
                }
            }
            Msg::Shear { id: mob } => {
                if let Some(i) = self.mobs.iter().position(|m| m.id == mob) {
                    self.shear_mob(i);
                }
            }
            Msg::DropItem {
                pos,
                vel,
                stack,
                delay,
            } => {
                self.add_item(ItemEntity::new(pos, vel, stack, delay));
            }
            Msg::Open { p } => {
                let open = if p.y == CLOSED_Y {
                    None
                } else {
                    // Make sure the block entity exists.
                    let b = self.terrain.world.geti(p);
                    if is_chest(b) {
                        let (a, other) = self.chest_halves(p);
                        for q in std::iter::once(a).chain(other) {
                            self.block_entities
                                .chests
                                .entry(q)
                                .or_insert_with(|| Box::new([None; 27]));
                        }
                    } else if is_furnace(b) {
                        self.block_entities.furnaces.entry(p).or_default();
                    }
                    Some(p)
                };
                if let Some(peer) = self.peer(id) {
                    peer.open = open;
                    peer.sent_container = None;
                }
            }
            Msg::Container { p, kind, slots, .. } => {
                self.apply_container(p, kind, &slots, None);
                // What the player has now; no need to send it back.
                let bytes = self.container_msg(p).map(|m| m.encode());
                if let Some(peer) = self.peer(id) {
                    peer.sent_container = bytes;
                }
            }
            Msg::Chat { text, .. } => {
                let name = self.peer(id).map(|p| p.name.clone()).unwrap_or_default();
                self.announce(format!("<{name}> {text}"), chat::WHITE);
            }
            Msg::Command(line) => {
                // Only world-wide commands come here (the time).
                if line.starts_with("/time") {
                    self.run_command(&line);
                    self.broadcast(&Msg::Time(self.time_of_day), None);
                }
            }
            Msg::Save(state) => {
                if let Some(p) = self.peer(id) {
                    p.state = Some(state);
                }
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
        // The edited chunks; the rest the player generates from the seed.
        let world = &self.terrain.world;
        let chunks: Vec<Msg> = world
            .chunks
            .iter()
            .filter(|(p, _)| world.modified.contains(p))
            .chain(world.saved.iter())
            .map(|(p, c)| Msg::Chunk {
                pos: *p,
                rle: rle(c.raw()),
            })
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
        // Items lying on crafting tables.
        others.extend(
            self.table_grids()
                .into_iter()
                .map(|(p, grid)| table_msg(p, &grid)),
        );
        let Some(peer) = self.peer(id) else { return };
        peer.conn.send(&welcome);
        for c in &chunks {
            peer.conn.send(c);
        }
        peer.conn.send(&Msg::Ready);
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
