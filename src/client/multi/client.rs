//! The connection to the world's server: joining (the game's own world, or a LAN game),
//! leaving, and following what the server sends.

use crate::client::{AUTOSAVE_SECONDS, Game, MAX_HEALTH, Screen};
use crate::client::multi::{Client, Session, color_from};
use crate::client::player::GameMode;
use crate::entity::{FallingBlock, ItemEntity};
use crate::entity::mob::Mob;
use crate::entity::survival::Needs;
use crate::app::lang::{t, tf};
use crate::net::{Conn, ItemNet, Msg, PROTOCOL, PlayerState};
use crate::world::save::{PlayerSave, WorldMeta, unrle};
use crate::sim::clock::TICK_SECS;
use crate::ui::chat;
use crate::world::{Block, ChunkData, FastMap, FastSet, HEIGHT, World};
use crate::world::terrain::Terrain;
use glam::{IVec3, Vec3};
use std::sync::Arc;

impl Game {
    /// Connects to a LAN game at "ip:port" (or just "ip").
    pub(in crate::client) fn join_server(&mut self, addr: &str) {
        let addr = addr.trim();
        let addr = if addr.contains(':') {
            addr.to_string()
        } else {
            format!("{addr}:{}", crate::net::DEFAULT_PORT)
        };
        self.menus.finder = None;
        self.settings.save();
        self.connect_to(addr);
    }

    /// Connects to a LAN game at "ip:port" (the settings as they are, not saved).
    pub(super) fn connect_to(&mut self, addr: String) {
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

    /// Connecting to a LAN game: once connected, says hello (the server answers with its world).
    pub(super) fn poll_joining(&mut self) {
        let Some((_, rx)) = &self.menus.joining else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(std::io::Error::other("?")),
        };
        self.menus.joining = None;
        match result {
            Ok(conn) => self.join_with(conn),
            Err(e) => {
                self.menus.net_message = tf("mp.connect_failed", &[&e]);
                self.screen = Screen::Disconnected;
            }
        }
    }

    /// Says hello through `conn` to the server at its other end (a LAN host's, or the one this
    /// game runs itself): it answers with its world.
    fn join_with(&mut self, conn: Conn) {
        conn.send(&Msg::Hello {
            proto: PROTOCOL,
            name: self.settings.name.clone(),
            view: self.settings.render_distance.round().clamp(0.0, 255.0) as u8,
        });
        self.session.net = Some(Client {
            conn,
            id: 0,
            tick: 0.0,
            item_targets: FastMap::default(),
            container_known: None,
            collecting: FastMap::default(),
            falling_targets: Vec::new(),
            falling_at: 0.0,
        });
        self.screen = Screen::Connecting;
    }

    /// Plays the world `meta`: a server for it runs on a thread of its own, and this game joins
    /// it like any player.
    pub(in crate::client) fn play_world(&mut self, meta: WorldMeta) {
        let (local, conn) = crate::sim::server::start(meta);
        self.session.local = Some(local);
        self.menus.net_message = t("mp.connecting").to_string();
        self.join_with(conn);
    }

    /// Pause menu: opens this world to the LAN (its server takes the LAN players).
    pub(in crate::client) fn open_to_lan(&mut self) {
        let Some(local) = &self.session.local else { return };
        if self.session.lan_address.is_some() {
            return;
        }
        match local.open_lan(&self.settings.name) {
            Ok(address) => {
                self.say(tf("lan.opened", &[&address]), chat::YELLOW);
                self.session.lan_address = Some(address);
                self.resume();
            }
            Err(e) => self.say(tf("lan.failed", &[&e]), chat::RED),
        }
    }

    /// Leaves the world (saying goodbye to its server with the latest state); `message`: why,
    /// shown on the disconnected screen.
    pub(in crate::client) fn leave_server(&mut self, message: Option<String>) {
        // (still connecting: given up; the connection, if it is made, is dropped)
        self.menus.joining = None;
        if self.me.body.spawned {
            let state = self.client_state();
            self.send(Msg::Save(state));
        }
        if let Some(c) = &mut self.session.net {
            c.conn.close();
        }
        // (the connection goes, then the game's own server, which saves and stops)
        self.session = Session::new();
        self.forget_world();
        self.set_grab(false);
        match message {
            Some(m) => {
                self.menus.net_message = m;
                self.screen = Screen::Disconnected;
            }
            None => self.screen = Screen::MainMenu,
        }
    }

    /// Gives up connecting: back to the world list (the game's own world) or the multiplayer
    /// screen (a LAN game).
    pub(in crate::client) fn cancel_connecting(&mut self) {
        let own = self.session.local.is_some();
        self.leave_server(None);
        if own {
            self.open_world_list();
        } else {
            self.open_multiplayer();
        }
    }

    /// Everything the server keeps for this player.
    pub(in crate::client) fn client_state(&self) -> PlayerState {
        let slots = self.carried_slots();
        // A dead player is kept as come back to life at home, so leaving on the death screen
        // does not bring them back where they died.
        let dead = self.screen == Screen::Dead;
        PlayerState {
            pos: if dead { self.home_pos() } else { self.me.body.pos },
            yaw: if dead { 0.0 } else { self.me.look.yaw },
            pitch: if dead { 0.0 } else { self.me.look.pitch },
            health: if dead { MAX_HEALTH } else { self.me.vitals.health },
            needs: if dead {
                Needs::new().to_array()
            } else {
                self.me.vitals.needs.to_array()
            },
            mode: match self.me.mode {
                GameMode::Survival => crate::net::mode::SURVIVAL,
                GameMode::Creative => crate::net::mode::CREATIVE,
                GameMode::Spectator => crate::net::mode::SPECTATOR,
            },
            flying: self.me.body.flying,
            slot: self.me.items.hotbar_slot as u8,
            inventory: slots.iter().chain(&self.me.items.inventory.armor).copied().collect(),
            bed: self.me.vitals.bed_spawn,
        }
    }

    pub(super) fn client_tick(&mut self, dt: f32) {
        // (a change to the open chest or table goes out before the server's copy of it is
        // taken in: it would be lost under a copy sent before the server had it)
        self.net_container_sync();
        let Some(c) = &mut self.session.net else {
            return;
        };
        let (msgs, open) = c.conn.poll();
        for m in msgs {
            self.client_handle(m);
            if self.session.net.is_none() {
                return; // refused
            }
        }
        if !open {
            self.leave_server(Some(t("mp.lost").to_string()));
            return;
        }
        if !self.me.body.spawned {
            return;
        }
        let pose = self.my_pose();
        if let Some(c) = &mut self.session.net {
            // (20 a second whatever the frame rate; after a stall not all at once)
            c.tick += dt;
            if c.tick >= TICK_SECS {
                c.tick = (c.tick - TICK_SECS).min(TICK_SECS);
                c.conn.send(&Msg::Pose(pose));
            }
        }
        self.net_container_sync();
    }

    fn client_handle(&mut self, m: Msg) {
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
                if let Some(c) = &mut self.session.net {
                    c.id = id;
                }
                self.begin_remote_world(seed, world, time, spawn, creative, cheats, state);
            }
            Msg::Refuse(reason) => self.leave_server(Some(reason)),
            Msg::Chunk { pos, rle } => {
                if let Some(c) = ChunkData::from_vec(&unrle(&rle)) {
                    let w = &mut self.terrain.world;
                    // The server's copy as it is now: changes waiting for it are older, and it
                    // is kept when unloaded (not generated again from the seed).
                    w.pending.remove(&pos);
                    w.modified.insert(pos);
                    if let Some(old) = w.chunks.get_mut(&pos) {
                        // Already generated here meanwhile: the server's copy replaces it.
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
            Msg::Time(t) => self.level.time_of_day = t,
            Msg::Join { id, name } => self.add_remote(id, name),
            Msg::Leave { id } => self.remove_remote(id),
            Msg::Poses(list) => {
                for (id, pose) in list {
                    self.set_remote_pose(id, pose);
                }
            }
            Msg::Entities {
                full,
                mobs,
                items,
                gone_mobs,
                gone_items,
                falling,
            } => self.sync_entities(full, mobs, items, &gone_mobs, &gone_items, falling),
            Msg::Give(stack) => self.give(stack),
            Msg::Hurt {
                dmg,
                from,
                knock,
                kind,
            } => self.hit_by_player(dmg, from, knock, kind),
            Msg::Grenade { kind, pos, vel, seed, fuse, .. } => self.remote_grenade(kind, pos, vel, seed, fuse),
            Msg::Blast { pos, seed } => self.remote_blast(pos, seed),
            Msg::BreakFx { p, block } => self.break_fx(p, block),
            Msg::Fx { kind, pos } => self.show_fx(kind, pos),
            Msg::Notch { p, notch } => {
                self.terrain.world.set_notch(p, notch);
                self.terrain.block_changed(p, false);
            }
            Msg::TreeFalls(t) => self.tree_falls(*t),
            Msg::TreeLands { id } => self.tree_landed(id),
            Msg::Collect { item, by } => {
                let now = self.clock.time;
                if let Some(it) = self.level.items.iter_mut().find(|it| it.id == item && !it.is_picking_up()) {
                    it.start_pickup(now);
                    if let Some(c) = &mut self.session.net {
                        c.collecting.insert(item, by);
                    }
                }
            }
            Msg::Logs(list) => self.level.lying_logs = list,
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
                if let (true, Some(c)) = (open_here, &mut self.session.net) {
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
            _ => {}
        }
    }

    /// Something the server did, to see and hear (`net::fx`).
    fn show_fx(&mut self, kind: u8, pos: Vec3) {
        use crate::net::fx;
        let (sky, blk) = self.terrain.world.light_estimate(pos);
        match kind {
            fx::WOLF_TAKES | fx::WOLF_REFUSES => {
                self.level.particles.crumbs(pos, crate::textures::tex::BONE, 6, sky, blk);
                if kind == fx::WOLF_TAKES {
                    self.audio.play(crate::audio::Sound::WolfBark, Some(pos), 0.8);
                } else {
                    for _ in 0..4 {
                        self.level.particles.smoke_shaded(pos + Vec3::Y * 0.3, 70, sky, blk);
                    }
                }
            }
            fx::POOF => self.level.particles.poof(pos, sky, blk),
            _ => {}
        }
    }

    /// Sets up the server's world (the chunks follow, then `Ready`): nothing of the last one
    /// is left.
    #[allow(clippy::too_many_arguments)]
    fn begin_remote_world(
        &mut self,
        seed: u32,
        world: String,
        time: f32,
        spawn: (i32, i32),
        creative: bool,
        cheats: bool,
        state: Option<PlayerState>,
    ) {
        self.forget_world();
        self.gfx.renderer.clear_chunks();
        self.terrain = Terrain::new(seed);
        self.level.spawn = spawn;
        self.menus.pano = Self::panorama_pos(&self.terrain, spawn);
        self.me.mode = if creative { GameMode::Creative } else { GameMode::Survival };
        self.me.cheats = cheats;
        self.level.time_of_day = time;
        self.level.meta = Some(WorldMeta {
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
        });
        self.screen = Screen::Connecting;
        if let Some(s) = state {
            self.me.vitals.bed_spawn = s.bed;
            self.me.items.inventory.slots = std::array::from_fn(|i| s.inventory.get(i).copied().flatten());
            let worn = crate::item::inventory::SIZE;
            self.me.items.inventory.armor = std::array::from_fn(|i| s.inventory.get(worn + i).copied().flatten());
            self.me.vitals.needs = Needs::from_array(s.needs);
            self.me.mode = match s.mode {
                crate::net::mode::CREATIVE => GameMode::Creative,
                crate::net::mode::SPECTATOR => GameMode::Spectator,
                _ => GameMode::Survival,
            };
            self.session.pending_player = Some(PlayerSave {
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

    /// A block change from the server.
    fn apply_remote_block(&mut self, p: IVec3, b: Block) {
        // (a furnace, chest, table or gun station broken: what was known of it goes)
        self.level.block_entities.forget_unless(p, b);
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
            // (the latest change of each block is enough: a block changing over and over,
            // like flowing water, does not pile up)
            let list = w.pending.entry(cp).or_default();
            match list.iter_mut().find(|(q, _)| *q == p) {
                Some(e) => e.1 = b,
                None => list.push((p, b)),
            }
        }
    }

    /// Mobs, dropped items and falling blocks near this player, from the server: the new and
    /// changed ones, those gone (`full`: all near are listed, the rest goes).
    fn sync_entities(
        &mut self,
        full: bool,
        mobs: Vec<crate::net::MobNet>,
        items: Vec<ItemNet>,
        gone_mobs: &[u32],
        gone_items: &[u32],
        falling: Vec<(Vec3, Block)>,
    ) {
        // Whether an entity stays: listed when all are, not gone otherwise.
        let stays = |listed: &FastSet<u32>, gone: &FastSet<u32>, id: u32| {
            if full {
                listed.contains(&id)
            } else {
                !gone.contains(&id)
            }
        };
        // Mobs
        let ids: FastSet<u32> = mobs.iter().map(|m| m.id).collect();
        let gone_ids: FastSet<u32> = gone_mobs.iter().copied().collect();
        let mut gone = Vec::new();
        self.level.mobs.retain(|m| {
            let keep = stays(&ids, &gone_ids, m.id);
            if !keep && m.death.is_some_and(|d| d > 0.6) {
                gone.push(m.center());
            }
            keep
        });
        for c in gone {
            let w = &self.terrain.world;
            let (sky, blk) = (w.sky_estimate(c), w.block_light_estimate(c));
            self.level.particles.poof(c, sky, blk);
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
        let gone_ids: FastSet<u32> = gone_items.iter().copied().collect();
        // (one flying to whoever picked it up goes on until it is there)
        self.level.items.retain(|it| it.is_picking_up() || stays(&ids, &gone_ids, it.id));
        let Some(c) = &mut self.session.net else {
            return;
        };
        if full {
            c.item_targets.clear();
        }
        for id in gone_items {
            c.item_targets.remove(id);
        }
        let at: FastMap<u32, usize> = self.level.items.iter().enumerate().map(|(i, it)| (it.id, i)).collect();
        for s in &items {
            c.item_targets.insert(s.id, (s.pos, Default::default()));
            match at.get(&s.id).map(|&i| &mut self.level.items[i]) {
                Some(it) => {
                    it.stack = s.stack;
                    // (its age runs on here between the updates: put right only when it is
                    // off, not snapped back and forth by the updates' few milliseconds, which
                    // would make its bobbing and turning shiver)
                    if (it.age - s.age).abs() > 0.25 {
                        it.age = s.age;
                    }
                }
                None => {
                    let mut it = ItemEntity::new(s.pos, Vec3::ZERO, s.stack, 0.0);
                    it.id = s.id;
                    it.age = s.age;
                    self.level.items.push(it);
                }
            }
        }
        // Falling blocks: each one here goes on from where it is drawn (found by its block and
        // column), gliding to where the server has it, at the speed it fell since.
        let now = self.clock.time;
        let since = (now - c.falling_at).max(TICK_SECS);
        c.falling_at = now;
        let mut old: Vec<Option<(FallingBlock, Vec3)>> =
            self.level.falling.drain(..).zip(c.falling_targets.drain(..)).map(Some).collect();
        for (pos, block) in falling {
            let same = |f: &FallingBlock| f.block == block && (f.pos.x - pos.x).abs() < 0.01 && (f.pos.z - pos.z).abs() < 0.01;
            let near = old
                .iter()
                .enumerate()
                .filter_map(|(i, o)| o.as_ref().filter(|(f, _)| same(f)).map(|(_, t)| (i, (t.y - pos.y).abs())))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|&(_, d)| d < 4.0);
            let f = match near.and_then(|(i, _)| old[i].take()) {
                Some((f, was)) => FallingBlock { vel_y: (pos.y - was.y) / since, ..f },
                None => FallingBlock { pos, vel_y: 0.0, block },
            };
            self.level.falling.push(f);
            c.falling_targets.push(pos);
        }
    }

    /// The world's things between the server's updates: they move toward what it sent.
    pub(in crate::client) fn client_world(&mut self, dt: f32) {
        for m in &mut self.level.mobs {
            m.follow(dt);
            if let Some(s) = m.sound(dt) {
                self.audio.play(s, Some(m.center()), 1.0);
            }
        }
        // Items fly to whoever picked them up (this player's eye, or another player).
        let me = match &self.session.net {
            Some(c) => c.id,
            _ => 0,
        };
        let eye = self.eye() - Vec3::Y * 0.25;
        let remotes: Vec<(u8, Vec3)> = self.session.remotes.iter().map(|r| (r.id, r.pose.pos + Vec3::Y * 1.2)).collect();
        if let Some(c) = &mut self.session.net {
            let k = crate::util::damp(15.0, dt);
            let mut landed = Vec::new();
            for it in &mut self.level.items {
                if it.is_picking_up() {
                    let by = c.collecting.get(&it.id).copied();
                    let target = match by {
                        Some(id) if id != me => remotes.iter().find(|r| r.0 == id).map_or(it.pos, |r| r.1),
                        _ => eye,
                    };
                    if it.update_pickup(dt, target) {
                        landed.push(it.id);
                    }
                    continue;
                }
                if let Some((target, glide)) = c.item_targets.get_mut(&it.id) {
                    it.pos = it.pos.lerp(*target, glide.step(dt));
                }
                it.age += dt;
            }
            for id in &landed {
                c.collecting.remove(id);
            }
            self.level.items.retain(|it| !(it.is_picking_up() && landed.contains(&it.id)));
            // Falling blocks go on falling, and glide to where the server has them.
            for (f, target) in self.level.falling.iter_mut().zip(&mut c.falling_targets) {
                target.y += f.vel_y * dt;
                f.pos.y += f.vel_y * dt;
                f.pos = f.pos.lerp(*target, k);
            }
        }
        self.level.time_of_day = crate::sim::advance_time(self.level.time_of_day, dt);
        self.session.autosave -= dt;
        if self.session.autosave <= 0.0 {
            self.session.autosave = AUTOSAVE_SECONDS;
            self.save_world();
        }
    }
}
