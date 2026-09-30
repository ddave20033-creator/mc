//! Playing in someone else's LAN game: joining, leaving, and following the host's world.

use super::*;
impl Game {
    /// Connects to a LAN game at "ip:port" (or just "ip").
    pub(in crate::game) fn join_server(&mut self, addr: &str) {
        let addr = addr.trim();
        let addr = if addr.contains(':') {
            addr.to_string()
        } else {
            format!("{addr}:{}", crate::net::DEFAULT_PORT)
        };
        self.menus.finder = None;
        self.settings.save();
        // (looking the address up and connecting can take seconds: not on the window's thread)
        let (tx, rx) = std::sync::mpsc::channel();
        let target = addr.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Conn::connect(&target));
        });
        self.menus.joining = Some((addr.clone(), rx));
        self.menus.net_message = tf("mp.connecting_to", &[&addr]);
        self.screen = Screen::Connecting;
    }

    /// Connecting to a LAN game: once connected, says hello (the host answers with its world).
    pub(super) fn poll_joining(&mut self) {
        let Some((_, rx)) = &self.menus.joining else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(std::io::Error::other("?")),
        };
        self.menus.joining = None;
        match result {
            Ok(conn) => {
                conn.send(&Msg::Hello {
                    proto: PROTOCOL,
                    name: self.settings.name.clone(),
                });
                self.net = Some(Net::Client(Client {
                    conn,
                    id: 0,
                    tick: 0.0,
                    item_targets: FastMap::default(),
                    container_known: None,
                }));
            }
            Err(e) => {
                self.menus.net_message = tf("mp.connect_failed", &[&e]);
                self.screen = Screen::Disconnected;
            }
        }
    }

    /// LAN player: leaves the game (saying goodbye to the host with the latest state).
    pub(in crate::game) fn leave_server(&mut self, message: Option<String>) {
        // (still connecting: given up; the connection, if it is made, is dropped)
        self.menus.joining = None;
        if self.is_client() {
            if self.player.spawned {
                let state = self.client_state();
                self.send(Msg::Save(state));
            }
            if let Some(Net::Client(c)) = &mut self.net {
                c.conn.close();
            }
        }
        self.net = None;
        self.remotes.clear();
        self.world_meta = None;
        self.custom_skins.retain(|&id, _| id == 0);
        self.skin_pngs.retain(|&id, _| id == 0);
        if let Some(png) = self.local_skin_png.clone() {
            let _ = self.set_skin_png(0, png);
        }
        self.player.spawned = false;
        self.set_grab(false);
        match message {
            Some(m) => {
                self.menus.net_message = m;
                self.screen = Screen::Disconnected;
            }
            None => self.screen = Screen::MainMenu,
        }
    }

    /// Everything the host keeps for this player.
    pub(in crate::game) fn client_state(&self) -> PlayerState {
        let slots = self.carried_slots();
        let dead = self.screen == Screen::Dead;
        PlayerState {
            pos: self.player.pos,
            yaw: self.yaw,
            pitch: self.pitch,
            health: if dead { MAX_HEALTH } else { self.health },
            needs: if dead {
                Needs::new().to_array()
            } else {
                self.needs.to_array()
            },
            mode: match self.game_mode {
                GameMode::Survival => crate::net::mode::SURVIVAL,
                GameMode::Creative => crate::net::mode::CREATIVE,
                GameMode::Spectator => crate::net::mode::SPECTATOR,
            },
            flying: self.player.flying,
            slot: self.hotbar_slot as u8,
            inventory: slots.iter().chain(&self.inventory.armor).copied().collect(),
            bed: self.bed_spawn,
        }
    }

    pub(super) fn client_tick(&mut self, dt: f32) {
        let Some(Net::Client(c)) = &mut self.net else {
            return;
        };
        let (msgs, open) = c.conn.poll();
        for m in msgs {
            self.client_handle(m);
            if !self.is_client() {
                return; // refused
            }
        }
        if !open {
            self.leave_server(Some(t("mp.lost").to_string()));
            return;
        }
        if !self.player.spawned {
            return;
        }
        let pose = self.my_pose();
        if let Some(Net::Client(c)) = &mut self.net {
            c.tick += dt;
            if c.tick >= TICK {
                c.tick = 0.0;
                c.conn.send(&Msg::Pose(pose));
            }
        }
        self.net_container_sync();
    }

    pub(super) fn client_handle(&mut self, m: Msg) {
        match m {
            Msg::Welcome {
                id,
                seed,
                world,
                time,
                spawn,
                creative,
                cheats,
                state,
            } => {
                if let Some(Net::Client(c)) = &mut self.net {
                    c.id = id;
                }
                self.begin_remote_world(seed, world, time, spawn, creative, cheats, state);
                if self.settings.skin == 4 {
                    if let Some(png) = self.local_skin_png.clone() {
                        if self.set_skin_png(id, png.clone()).is_ok() {
                            self.send(Msg::Skin { id, png });
                        }
                    }
                }
            }
            Msg::Refuse(reason) => self.leave_server(Some(reason)),
            Msg::Chunk { pos, rle } => {
                if let Some(c) = ChunkData::from_raw(unrle(&rle)) {
                    let w = &mut self.terrain.world;
                    if let Some(old) = w.chunks.get_mut(&pos) {
                        // Already generated here meanwhile: the host's copy replaces it.
                        *old = Arc::new(c);
                        let mid = IVec3::new(pos.0 * 16 + 8, 64, pos.1 * 16 + 8);
                        self.terrain.block_changed(mid, true);
                    } else {
                        w.saved.insert(pos, Arc::new(c));
                    }
                }
            }
            Msg::Ready => {
                self.screen = Screen::Loading;
            }
            Msg::Blocks(list) => {
                for (p, b) in list {
                    self.apply_remote_block(p, b);
                }
            }
            Msg::Time(t) => self.time_of_day = t,
            Msg::Join { id, name } => self.add_remote(id, name),
            Msg::Leave { id } => self.remove_remote(id),
            Msg::Poses(list) => {
                for (id, pose) in list {
                    self.set_remote_pose(id, pose);
                }
            }
            Msg::Entities {
                mobs,
                items,
                falling,
            } => self.sync_entities(mobs, items, falling),
            Msg::Give(stack) => self.give(stack),
            Msg::Hurt {
                dmg,
                from,
                knock,
                kind,
            } => self.hit_by_player(dmg, from, knock, kind),
            Msg::Grenade {
                id,
                kind,
                pos,
                vel,
                seed,
                fuse,
            } => self.remote_grenade(id, kind, pos, vel, seed, fuse),
            Msg::Blast { pos, seed } => self.remote_blast(pos, seed),
            Msg::BreakFx { p, block } => self.break_fx(p, block, true, None),
            Msg::Furnace {
                p,
                burn,
                cook,
                input,
                fuel,
                output,
                grill,
            } => self.apply_furnace(p, burn, cook, [input, fuel, output], grill),
            Msg::Bench { p, bench } => self.set_bench(p, bench),
            Msg::Container { p, kind, slots } => {
                let msg = Msg::Container {
                    p,
                    kind,
                    slots: slots.clone(),
                };
                self.apply_container(p, kind, &slots);
                let open_here = matches!(self.screen, Screen::Container(c) if Self::container_pos(c) == Some(p));
                if let (true, Some(Net::Client(c))) = (open_here, &mut self.net) {
                    c.container_known = Some(msg.encode());
                }
            }
            Msg::Chat { text, color } => self.say(text, color_from(color)),
            Msg::Shot {
                id,
                kind,
                mods,
                eye,
                seed,
                bullets,
            } => self.remote_shot(id, kind, mods, eye, seed, &bullets),
            Msg::Skin { id, png } => {
                let own = matches!(&self.net, Some(Net::Client(c)) if c.id == id);
                if !own {
                    let _ = self.set_skin_png(id, png);
                }
            }
            _ => {}
        }
    }

    /// LAN player: sets up the host's world (the chunks follow, then `Ready`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_remote_world(
        &mut self,
        seed: u32,
        world: String,
        time: f32,
        spawn: (i32, i32),
        creative: bool,
        cheats: bool,
        state: Option<PlayerState>,
    ) {
        let meta = WorldMeta {
            folder: String::new(),
            name: world,
            seed,
            creative,
            spectator: false,
            cheats,
            last_played: 0,
            time_of_day: time,
            spawn: Some(spawn),
            bed: None,
            player: None,
        };
        self.load_world(meta);
        self.screen = Screen::Connecting;
        if let Some(s) = state {
            self.bed_spawn = s.bed;
            self.inventory.slots = std::array::from_fn(|i| s.inventory.get(i).copied().flatten());
            let worn = crate::item::inventory::SIZE;
            self.inventory.armor = std::array::from_fn(|i| s.inventory.get(worn + i).copied().flatten());
            self.needs = Needs::from_array(s.needs);
            self.game_mode = match s.mode {
                crate::net::mode::CREATIVE => GameMode::Creative,
                crate::net::mode::SPECTATOR => GameMode::Spectator,
                _ => GameMode::Survival,
            };
            self.pending_player = Some(PlayerSave {
                pos: s.pos.to_array(),
                yaw: s.yaw,
                pitch: s.pitch,
                health: s.health,
                flying: s.flying,
                slot: s.slot as usize,
                needs: Some(s.needs),
            });
        }
    }

    /// LAN player: a block change from the host.
    pub(super) fn apply_remote_block(&mut self, p: IVec3, b: u8) {
        let cp = World::chunk_pos(p.x, p.z);
        let w = &mut self.terrain.world;
        if w.chunks.contains_key(&cp) {
            if w.geti(p) != b {
                self.set_block(p, b);
            }
        } else if let Some(c) = w.saved.get_mut(&cp) {
            if (0..HEIGHT as i32).contains(&p.y) {
                Arc::make_mut(c).set(
                    p.x.rem_euclid(16) as usize,
                    p.y as usize,
                    p.z.rem_euclid(16) as usize,
                    b,
                );
            }
        } else {
            w.pending.entry(cp).or_default().push((p, b));
        }
    }

    /// LAN player: mobs, dropped items and falling blocks near this player, from the host.
    pub(super) fn sync_entities(
        &mut self,
        mobs: Vec<crate::net::MobNet>,
        items: Vec<ItemNet>,
        falling: Vec<(Vec3, u8)>,
    ) {
        // Mobs
        let ids: FastSet<u32> = mobs.iter().map(|m| m.id).collect();
        let mut gone = Vec::new();
        self.level.mobs.retain(|m| {
            let keep = ids.contains(&m.id);
            if !keep && m.death.is_some_and(|d| d > 0.6) {
                gone.push(m.center());
            }
            keep
        });
        for c in gone {
            let w = &self.terrain.world;
            let (sky, blk) = (w.sky_estimate(c), w.block_light_estimate(c));
            self.particles.poof(c, sky, blk);
        }
        // (found by id through a map: after a tree comes down there can be hundreds)
        let at: FastMap<u32, usize> = self.level.mobs.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
        for s in &mobs {
            match at.get(&s.id).map(|&i| &mut self.level.mobs[i]) {
                Some(m) => m.apply_net(s),
                None => {
                    if let Some(m) = Mob::from_net(s) {
                        self.level.mobs.push(m);
                    }
                }
            }
        }
        // Items
        let ids: FastSet<u32> = items.iter().map(|i| i.id).collect();
        self.level.items.retain(|it| ids.contains(&it.id));
        let Some(Net::Client(c)) = &mut self.net else {
            return;
        };
        c.item_targets.clear();
        let at: FastMap<u32, usize> = self.level.items.iter().enumerate().map(|(i, it)| (it.id, i)).collect();
        for s in &items {
            c.item_targets.insert(s.id, s.pos);
            match at.get(&s.id).map(|&i| &mut self.level.items[i]) {
                Some(it) => {
                    it.stack = s.stack;
                    it.age = s.age;
                }
                None => {
                    let mut it = ItemEntity::new(s.pos, Vec3::ZERO, s.stack, 0.0);
                    it.id = s.id;
                    it.age = s.age;
                    self.level.items.push(it);
                }
            }
        }
        // Falling blocks
        self.level.falling = falling
            .into_iter()
            .map(|(pos, block)| FallingBlock {
                pos,
                vel_y: 0.0,
                block,
            })
            .collect();
    }

    /// LAN player's `update_world`: things move toward what the host sent.
    pub(in crate::game) fn client_world(&mut self, dt: f32) {
        for m in &mut self.level.mobs {
            m.follow(dt);
            if let Some(s) = m.sound(dt) {
                self.audio.play(s, Some(m.center()), 1.0);
            }
        }
        if let Some(Net::Client(c)) = &self.net {
            let k = crate::util::damp(15.0, dt);
            for it in &mut self.level.items {
                if let Some(&target) = c.item_targets.get(&it.id) {
                    it.pos = it.pos.lerp(target, k);
                }
                it.age += dt;
            }
        }
        if self.torch_particles {
            self.torch_fire(dt);
        }
        self.time_of_day = (self.time_of_day + dt / DAY_LENGTH).fract();
        self.autosave -= dt;
        if self.autosave <= 0.0 {
            self.autosave = AUTOSAVE_SECONDS;
            self.save_world();
        }
    }
}
