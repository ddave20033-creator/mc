//! World list, world creation/deletion, loading and saving.

use super::*;
use crate::lang::tf;
use crate::save::{self, list_worlds, seed_from_text};

/// "2026-09-23 18:04" from unix seconds (UTC).
fn format_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{y}-{m:02}-{d:02} {:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    )
}

impl Game {
    pub(super) fn open_world_list(&mut self) {
        self.worlds = list_worlds();
        self.selected_world = if self.worlds.is_empty() {
            None
        } else {
            Some(0)
        };
        self.world_scroll = 0.0;
        self.screen = Screen::SelectWorld;
    }

    pub(super) fn world_list_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.5);
        self.ui
            .text_centered(t("worlds.title"), w * 0.5, 12.0 * s, s, WHITE, true);

        // Scrollable list
        let (lw, row_h) = (270.0 * s, 32.0 * s);
        let lx = (w * 0.5 - lw * 0.5).round();
        let (top, bottom) = (28.0 * s, h - 60.0 * s);
        let visible = ((bottom - top) / row_h).floor().max(1.0) as usize;
        let max_scroll = self.worlds.len().saturating_sub(visible) as f32;
        self.world_scroll = (self.world_scroll - self.ui.scroll).clamp(0.0, max_scroll);
        let first = self.world_scroll as usize;
        self.ui.solid(
            0.0,
            top - 2.0 * s,
            w,
            bottom - top + 4.0 * s,
            rgba(0, 0, 0, 110),
        );
        if self.worlds.is_empty() {
            self.ui.text_centered(
                t("worlds.empty"),
                w * 0.5,
                (top + bottom) * 0.5 - 4.0 * s,
                s,
                rgba(170, 170, 170, 255),
                true,
            );
        }
        let mut play = None;
        for (i, meta) in self.worlds.iter().enumerate().skip(first).take(visible) {
            let y = (top + (i - first) as f32 * row_h).round();
            let hovered = self.ui.hit(lx, y, lw, row_h - 2.0 * s);
            let selected = self.selected_world == Some(i);
            if selected {
                self.ui.rect_full(
                    lx,
                    y,
                    lw,
                    row_h - 2.0 * s,
                    rgba(43, 66, 54, 245),
                    rgba(26, 42, 36, 245),
                    3.0 * s,
                    0.0,
                );
                self.ui
                    .rect(lx, y, 2.0 * s, row_h - 2.0 * s, ACCENT_GREEN, s);
            } else if hovered {
                self.ui
                    .rect(lx, y, lw, row_h - 2.0 * s, rgba(48, 54, 64, 225), 3.0 * s);
            }
            let tx = lx + 8.0 * s;
            self.ui.text(&meta.name, tx, y + 5.0 * s, s, WHITE, true);
            let mode = if meta.creative {
                t("mode.creative_long")
            } else {
                t("mode.survival_long")
            };
            let info = if meta.cheats {
                format!("{mode}, {}", t("worlds.cheats"))
            } else {
                mode.to_string()
            };
            let details = format!("{info}  |  {}", format_date(meta.last_played));
            let detail_color = if selected {
                rgba(205, 225, 211, 255)
            } else {
                rgba(180, 190, 195, 255)
            };
            self.ui
                .text(&details, tx, y + 18.0 * s, s, detail_color, false);
            if hovered && self.ui.pressed {
                if self.last_click.0 == i && self.time - self.last_click.1 < 0.35 {
                    play = Some(i);
                }
                self.selected_world = Some(i);
                self.last_click = (i, self.time);
            }
        }

        // Buttons
        let (bw, bh) = (150.0 * s, 20.0 * s);
        let x1 = (w * 0.5 - 154.0 * s).round();
        let x2 = (w * 0.5 + 4.0 * s).round();
        let y1 = (h - 52.0 * s).round();
        let y2 = (h - 28.0 * s).round();
        let has = self.selected_world.is_some();
        if self.ui.button(t("worlds.play"), x1, y1, bw, bh, has) {
            play = self.selected_world;
        }
        if self.ui.button(t("worlds.create"), x2, y1, bw, bh, true) {
            self.create_name = t("create.default_name").to_string();
            self.create_seed.clear();
            self.create_creative = false;
            self.create_cheats = false;
            self.ui.focus("world_name");
            self.screen = Screen::CreateWorld;
        }
        if self.ui.button(t("worlds.delete"), x1, y2, bw, bh, has) {
            self.screen = Screen::DeleteWorld;
        }
        if self.ui.button(t("gui.cancel"), x2, y2, bw, bh, true) {
            self.screen = Screen::MainMenu;
        }
        if let Some(i) = play {
            let meta = self.worlds[i].clone();
            self.load_world(meta);
        }
        Action::None
    }

    pub(super) fn create_world_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.5);
        let cx = w * 0.5;
        let fw = 200.0 * s;
        let fx = (cx - fw * 0.5).round();
        let mut y = (h * 0.5 - 100.0 * s).max(8.0 * s).round();
        self.ui
            .text_centered(t("worlds.create"), cx, y, s, WHITE, true);
        y += 18.0 * s;
        self.ui
            .text(t("create.name"), fx, y, s, rgba(170, 170, 170, 255), false);
        y += 10.0 * s;
        let mut name = std::mem::take(&mut self.create_name);
        self.ui
            .text_field("world_name", &mut name, fx, y, fw, 20.0 * s, "", 32);
        self.create_name = name;
        y += 28.0 * s;
        self.ui
            .text(t("create.seed"), fx, y, s, rgba(170, 170, 170, 255), false);
        y += 10.0 * s;
        let mut seed = std::mem::take(&mut self.create_seed);
        self.ui.text_field(
            "world_seed",
            &mut seed,
            fx,
            y,
            fw,
            20.0 * s,
            t("create.seed_hint"),
            32,
        );
        self.create_seed = seed;
        y += 28.0 * s;
        let mode = if self.create_creative {
            t("mode.creative")
        } else {
            t("mode.survival")
        };
        if self
            .ui
            .button(&tf("create.mode", &[&mode]), fx, y, fw, 20.0 * s, true)
        {
            self.create_creative = !self.create_creative;
            // Minecraft enables cheats by default in creative.
            self.create_cheats = self.create_creative;
        }
        y += 22.0 * s;
        let desc = if self.create_creative {
            t("create.creative_desc")
        } else {
            t("create.survival_desc")
        };
        self.ui
            .text_centered(desc, cx, y, s * 0.8, rgba(170, 170, 170, 255), false);
        y += 12.0 * s;
        if self.ui.button(
            &tf("create.cheats", &[&crate::lang::on_off(self.create_cheats)]),
            fx,
            y,
            fw,
            20.0 * s,
            true,
        ) {
            self.create_cheats = !self.create_cheats;
        }
        let (bw, bh) = (150.0 * s, 20.0 * s);
        let by = (h - 28.0 * s).round();
        if self.ui.button(
            t("worlds.create"),
            (cx - 154.0 * s).round(),
            by,
            bw,
            bh,
            true,
        ) {
            self.create_world();
        }
        if self
            .ui
            .button(t("gui.cancel"), (cx + 4.0 * s).round(), by, bw, bh, true)
        {
            self.screen = Screen::SelectWorld;
        }
        Action::None
    }

    pub(super) fn create_world(&mut self) {
        let seed = seed_from_text(&self.create_seed);
        let meta = WorldMeta::create(
            &self.create_name,
            seed,
            self.create_creative,
            self.create_cheats,
        );
        self.load_world(meta);
    }

    pub(super) fn delete_world_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.8);
        let Some(i) = self.selected_world else {
            self.screen = Screen::SelectWorld;
            return Action::None;
        };
        let name = self.worlds[i].name.clone();
        self.ui
            .text_centered(t("worlds.delete_q"), w * 0.5, h * 0.35, s, WHITE, true);
        self.ui.text_centered(
            &tf("worlds.delete_warn", &[&name]),
            w * 0.5,
            h * 0.35 + 14.0 * s,
            s,
            rgba(170, 170, 170, 255),
            true,
        );
        let (bw, bh) = (150.0 * s, 20.0 * s);
        let by = (h * 0.35 + 40.0 * s).round();
        if self.ui.button(
            t("worlds.delete"),
            (w * 0.5 - 154.0 * s).round(),
            by,
            bw,
            bh,
            true,
        ) {
            self.saver.wait();
            save::delete_world(&self.worlds[i].folder);
            self.open_world_list();
        }
        if self.ui.button(
            t("gui.cancel"),
            (w * 0.5 + 4.0 * s).round(),
            by,
            bw,
            bh,
            true,
        ) {
            self.screen = Screen::SelectWorld;
        }
        Action::None
    }

    // ---------------- loading ----------------

    pub(super) fn load_world(&mut self, meta: WorldMeta) {
        // The previous world may still be writing its chunks (possibly this same world).
        self.saver.wait();
        self.renderer.clear_chunks();
        self.terrain = Terrain::new(meta.seed);
        for (pos, c) in save::load_chunks(&meta.folder) {
            self.terrain.world.saved.insert(pos, Arc::new(c));
            self.terrain.world.modified.insert(pos);
        }
        self.fluids = Fluids::new();
        self.spawn = meta.spawn.unwrap_or_else(|| self.terrain.gen.find_spawn());
        self.bed_spawn = meta.bed;
        self.sleep = None;
        self.asleep_for = 0.0;
        self.pano = Self::panorama_pos(&self.terrain, self.spawn);
        self.inventory = Inventory::new();
        save::load_inventory(&meta.folder, &mut self.inventory.slots);
        self.block_entities = BlockEntities::default();
        self.saplings.clear();
        self.items.clear();
        self.mobs.clear();
        self.mob_target = None;
        save::load_entities(
            &meta.folder,
            &mut self.block_entities,
            &mut self.saplings,
            &mut self.items,
            &mut self.mobs,
        );
        self.falling.clear();
        self.cursor = None;
        self.craft = [None; 9];
        self.guns.bench.clear();
        self.guns.cases.clear();
        self.drag = None;
        self.mining = None;
        self.target = None;
        self.chest_open.clear();
        self.air = MAX_AIR;
        self.invuln = 0.0;
        self.game_mode = if meta.creative {
            GameMode::Creative
        } else {
            GameMode::Survival
        };
        self.cheats = meta.cheats;
        self.time_of_day = meta.time_of_day;
        self.player = Player::default();
        self.pending_player = meta.player.clone();
        self.health = MAX_HEALTH;
        self.needs = Needs::new();
        self.using = None;
        self.fire = 0.0;
        self.hurt_time = 0.0;
        self.chat = Chat::new();
        self.camera = Default::default();
        self.autosave = AUTOSAVE_SECONDS;
        self.world_meta = Some(meta);
        self.screen = Screen::Loading;
    }

    /// Where the world is being loaded around (saved player position or spawn).
    pub(super) fn load_center(&self) -> Vec3 {
        match &self.pending_player {
            Some(p) => Vec3::from(p.pos),
            None => Vec3::new(self.spawn.0 as f32, 64.0, self.spawn.1 as f32),
        }
    }

    pub(super) fn world_ready(&self) -> bool {
        let c = self.load_center();
        self.terrain
            .ready_around(World::chunk_pos(c.x.floor() as i32, c.z.floor() as i32), 3)
            && self.renderer.pending() < 32
    }

    pub(super) fn load_progress(&self) -> f32 {
        let c = self.load_center();
        let c = World::chunk_pos(c.x.floor() as i32, c.z.floor() as i32);
        let (mut done, mut total) = (0, 0);
        for dz in -3..=3 {
            for dx in -3..=3 {
                if dx * dx + dz * dz <= 9 {
                    total += 1;
                    if self.terrain.is_meshed((c.0 + dx, c.1 + dz)) {
                        done += 1;
                    }
                }
            }
        }
        done as f32 / total as f32
    }

    pub(super) fn enter_game(&mut self) {
        match self.pending_player.take() {
            Some(p) => {
                self.player = Player {
                    pos: Vec3::from(p.pos),
                    spawned: true,
                    flying: p.flying && self.creative(),
                    ..Default::default()
                };
                self.yaw = p.yaw;
                self.pitch = p.pitch;
                self.health = p.health.max(1.0);
                self.needs = p.needs.map(Needs::from_array).unwrap_or_else(Needs::new);
                self.hotbar_slot = p.slot.min(8);
                self.fall_peak = p.pos[1];
                self.body_yaw = self.yaw;
            }
            None => {
                self.spawn_player();
                self.say(t("chat.welcome"), chat::YELLOW);
            }
        }
        self.hint_timer = 14.0;
        self.hand.equip(self.held());
        self.resume();
        self.save_world();
    }

    /// Feet position on top of the highest block at the world spawn column.
    /// Works even when the spawn chunk is not loaded (e.g. after dying far away): then the
    /// edited copy or a freshly generated copy of the chunk is used.
    pub(super) fn spawn_pos(&self) -> Vec3 {
        let (x, z) = self.spawn;
        let w = &self.terrain.world;
        let cp = World::chunk_pos(x, z);
        let generated;
        let chunk: &ChunkData = match w.chunks.get(&cp).or_else(|| w.saved.get(&cp)) {
            Some(c) => c,
            None => {
                generated = self.terrain.gen.generate_chunk(cp.0, cp.1);
                &generated
            }
        };
        let (lx, lz) = (x.rem_euclid(16) as usize, z.rem_euclid(16) as usize);
        let y = (0..HEIGHT)
            .rev()
            .find(|&y| {
                let b = chunk.get(lx, y, lz);
                is_solid(b) || is_fluid(b)
            })
            .map_or(SEA, |y| y as i32)
            + 1;
        Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5)
    }

    pub(super) fn spawn_player(&mut self) {
        let pos = self.spawn_pos();
        self.sleep = None;
        self.player = Player {
            pos,
            spawned: true,
            ..Default::default()
        };
        self.fall_peak = pos.y;
        self.air = MAX_AIR;
        self.yaw = 0.0;
        self.body_yaw = self.yaw;
        self.pitch = 0.0;
    }

    pub(super) fn respawn(&mut self) {
        self.health = MAX_HEALTH;
        self.needs = Needs::new();
        self.using = None;
        self.fire = 0.0;
        self.invuln = 0.0;
        self.hurt_time = 0.0;
        self.spawn_at_home(true);
        self.resume();
    }

    /// Writes the current world to disk.
    pub(super) fn save_world(&mut self) {
        if self.world_meta.is_none() {
            return;
        }
        if self.is_client() {
            // A LAN player's things are kept by the host.
            if self.player.spawned {
                let state = self.client_state();
                self.send(crate::net::Msg::Save(state));
            }
            return;
        }
        self.save_peers();

        // A dead player is saved as respawned at home, so closing the game on the death
        // screen does not bring them back where they died.
        let dead = self.screen == Screen::Dead;
        let player = self.player.spawned.then(|| PlayerSave {
            pos: if dead {
                self.home_pos().to_array()
            } else {
                self.player.pos.to_array()
            },
            yaw: if dead { 0.0 } else { self.yaw },
            pitch: if dead { 0.0 } else { self.pitch },
            health: if dead { MAX_HEALTH } else { self.health },
            flying: self.player.flying && !dead,
            slot: self.hotbar_slot,
            needs: Some(if dead {
                Needs::new().to_array()
            } else {
                self.needs.to_array()
            }),
        });
        // An open crafting table keeps its grid; items in the 2x2 grid or on the cursor count
        // as inventory.
        self.stash_table(false);
        let slots = self.carried_slots();
        let meta = self.world_meta.as_mut().unwrap();
        meta.last_played = save::now_secs();
        meta.time_of_day = self.time_of_day;
        meta.spawn = Some(self.spawn);
        meta.bed = self.bed_spawn;
        meta.creative = self.game_mode == GameMode::Creative;
        if player.is_some() {
            meta.player = player;
        }
        meta.save();
        let folder = meta.folder.clone();
        save::save_inventory(&folder, &slots);
        save::save_entities(
            &folder,
            &self.block_entities,
            &self.saplings,
            &self.items,
            &self.mobs,
        );
        let world = &self.terrain.world;
        let chunks = world
            .modified
            .iter()
            .filter_map(|p| {
                world
                    .chunks
                    .get(p)
                    .or_else(|| world.saved.get(p))
                    .map(|c| (*p, c.clone()))
            })
            .collect();
        self.saver.save(&folder, chunks);
    }

    pub(super) fn quit_to_title(&mut self) {
        if self.is_client() {
            self.leave_server(None);
            return;
        }
        if self.screen == Screen::Dead {
            self.health = MAX_HEALTH;
            self.fire = 0.0;
            self.spawn_at_home(false);
        }
        self.wake_up();
        self.save_world();
        self.close_lan();
        self.world_meta = None;
        self.player.spawned = false;
        self.screen = Screen::MainMenu;
        self.set_grab(false);
    }
}

const ACCENT_GREEN: Color = rgba(98, 214, 120, 255);
