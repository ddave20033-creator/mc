//! World list, world creation/deletion, loading and saving.

use super::*;
use crate::lang::tf;
use crate::save::{self, list_worlds, seed_from_text};
use crate::ui::screens::{action_bar, card_title, screen_header};
use crate::ui::{ButtonKind, ACCENT, ACCENT_LIGHT, DANGER, GLASS_BOTTOM, GLASS_TOP};

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

    /// The worlds as cards (an icon of its mode, its name, chips for its mode and cheats, when
    /// it was last played), coming in one after another; the selected one lit with the
    /// accent. The actions in a bar along the bottom.
    pub(super) fn world_list_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.3);
        let (lw, row_h, card_h) = ((300.0 * s).min(w - 24.0 * s).round(), (42.0 * s).round(), (37.0 * s).round());
        let lx = (w * 0.5 - lw * 0.5).round();
        screen_header(&mut self.ui, t("worlds.title"), "", lx, lw);

        // Scrollable list
        let (top, bottom) = ((44.0 * s).round(), h - 50.0 * s);
        let visible = ((bottom - top) / row_h).floor().max(1.0) as usize;
        let max_scroll = self.worlds.len().saturating_sub(visible) as f32;
        self.world_scroll = (self.world_scroll - self.ui.scroll).clamp(0.0, max_scroll);
        let first = self.world_scroll as usize;
        if self.worlds.is_empty() {
            self.ui.text_centered(
                t("worlds.empty"),
                w * 0.5,
                (top + bottom) * 0.5 - 4.0 * s,
                s,
                rgba(170, 170, 180, 255),
                true,
            );
        }
        let mut play = None;
        for (i, meta) in self.worlds.iter().enumerate().skip(first).take(visible) {
            let y = (top + (i - first) as f32 * row_h).round();
            let a = self.ui.appear();
            let old = self.ui.style(a, Vec2::new(0.0, ((1.0 - a) * 10.0 * s).round()));
            let focused = self.ui.nav_item();
            if focused {
                self.selected_world = Some(i);
                if self.ui.nav_activated() {
                    play = Some(i);
                }
            }
            let hovered = self.ui.hit(lx, y, lw, card_h);
            let selected = self.selected_world == Some(i);
            world_card(&mut self.ui, meta, lx, y, lw, card_h, hovered || focused, selected);
            self.ui.restore(old);
            if hovered && self.ui.pressed {
                if self.last_click.0 == i && self.time - self.last_click.1 < 0.35 {
                    play = Some(i);
                }
                self.selected_world = Some(i);
                self.last_click = (i, self.time);
                self.ui.clicked = true;
            }
        }
        // A scroll bar when not all fit.
        if max_scroll > 0.0 {
            let track = bottom - top;
            let bar = (track * visible as f32 / self.worlds.len() as f32).max(12.0 * s);
            let by = top + (track - bar) * (self.world_scroll / max_scroll);
            self.ui.rect(lx + lw + 5.0 * s, top, 2.0 * s, track, rgba(255, 255, 255, 20), s);
            self.ui.rect(lx + lw + 5.0 * s, by, 2.0 * s, bar, with_alpha(ACCENT, 0.8), s);
        }

        // The actions.
        let bar_y = (h - 40.0 * s).round();
        action_bar(&mut self.ui, bar_y);
        let bh = (22.0 * s).round();
        let bw = ((lw - 3.0 * 6.0 * s) / 4.0).floor().max(60.0 * s);
        let total = bw * 4.0 + 18.0 * s;
        let x0 = (w * 0.5 - total * 0.5).round();
        let bx = |k: f32| (x0 + k * (bw + 6.0 * s)).round();
        let by = bar_y + 9.0 * s;
        let has = self.selected_world.is_some();
        if self.ui.button_primary(t("worlds.play_short"), bx(0.0), by, bw, bh, has) {
            play = self.selected_world;
        }
        if self.ui.button(t("worlds.create_short"), bx(1.0), by, bw, bh, true) {
            self.create_name = t("create.default_name").to_string();
            self.create_seed.clear();
            self.create_creative = false;
            self.create_cheats = false;
            self.ui.focus("world_name");
            self.screen = Screen::CreateWorld;
        }
        if self.ui.button_ex(t("worlds.delete"), bx(2.0), by, bw, bh, has, ButtonKind::Danger) {
            self.screen = Screen::DeleteWorld;
        }
        if self.ui.button(t("gui.cancel"), bx(3.0), by, bw, bh, true) {
            self.screen = Screen::MainMenu;
        }
        if let Some(i) = play {
            let meta = self.worlds[i].clone();
            self.load_world(meta);
        }
        Action::None
    }

    /// A new world: its name and seed, its mode and cheats as toggles, on a card.
    pub(super) fn create_world_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.3);
        let cx = w * 0.5;
        let (pw, ph) = ((250.0 * s).round(), (230.0 * s).round());
        let (px, py) = ((cx - pw * 0.5).round(), ((h - ph) * 0.45).round().max(6.0 * s));
        self.ui.panel(px, py, pw, ph);
        card_title(&mut self.ui, t("worlds.create"), cx, py + 12.0 * s, pw - 24.0 * s);
        self.ui.rect(cx - 14.0 * s, py + 27.0 * s, 28.0 * s, s, ACCENT, s * 0.5);
        let fw = pw - 28.0 * s;
        let fx = (px + 14.0 * s).round();
        let mut y = py + 38.0 * s;
        let label = rgba(160, 164, 180, 255);
        self.ui.text(t("create.name"), fx, y, s, label, false);
        y += 10.0 * s;
        let mut name = std::mem::take(&mut self.create_name);
        self.ui.text_field("world_name", &mut name, fx, y, fw, 20.0 * s, "", 32);
        self.create_name = name;
        y += 27.0 * s;
        self.ui.text(t("create.seed"), fx, y, s, label, false);
        y += 10.0 * s;
        let mut seed = std::mem::take(&mut self.create_seed);
        self.ui.text_field("world_seed", &mut seed, fx, y, fw, 20.0 * s, t("create.seed_hint"), 32);
        self.create_seed = seed;
        y += 28.0 * s;
        let mode = if self.create_creative {
            t("mode.creative")
        } else {
            t("mode.survival")
        };
        if self.ui.button(&tf("create.mode", &[&mode]), fx, y, fw, 20.0 * s, true) {
            self.create_creative = !self.create_creative;
            // Minecraft enables cheats by default in creative.
            self.create_cheats = self.create_creative;
        }
        y += 23.0 * s;
        let desc = if self.create_creative {
            t("create.creative_desc")
        } else {
            t("create.survival_desc")
        };
        let small = (s * 0.8).max(1.0);
        let lines = self.ui.wrap(desc, fw, small);
        for line in lines.iter().take(2) {
            self.ui.text_centered(line, cx, y, small, rgba(150, 154, 170, 255), false);
            y += 8.0 * small;
        }
        y += 4.0 * s;
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
        let bw = ((fw - 6.0 * s) * 0.5).floor();
        let by = (py + ph - 32.0 * s).round();
        if self.ui.button_primary(t("worlds.create_short"), fx, by, bw, 22.0 * s, true) {
            self.create_world();
        }
        if self.ui.button(t("gui.cancel"), fx + fw - bw, by, bw, 22.0 * s, true) {
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

    /// Asks before deleting a world: a card with the warning and a red button.
    pub(super) fn delete_world_screen(&mut self) -> Action {
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        screens::backdrop(&mut self.ui, 1.6);
        let Some(i) = self.selected_world else {
            self.screen = Screen::SelectWorld;
            return Action::None;
        };
        let name = self.worlds[i].name.clone();
        let (pw, ph) = ((270.0 * s).round(), (104.0 * s).round());
        let (px, py) = ((w * 0.5 - pw * 0.5).round(), ((h - ph) * 0.42).round());
        self.ui.panel(px, py, pw, ph);
        card_title(&mut self.ui, t("worlds.delete_q"), w * 0.5, py + 12.0 * s, pw - 24.0 * s);
        self.ui.rect(w * 0.5 - 14.0 * s, py + 27.0 * s, 28.0 * s, s, DANGER, s * 0.5);
        let warn = tf("worlds.delete_warn", &[&name]);
        let lines = self.ui.wrap(&warn, pw - 28.0 * s, s);
        for (k, line) in lines.iter().enumerate().take(3) {
            self.ui.text_centered(line, w * 0.5, py + 36.0 * s + k as f32 * 10.0 * s, s, rgba(190, 192, 204, 255), false);
        }
        let fw = pw - 28.0 * s;
        let bw = ((fw - 6.0 * s) * 0.5).floor();
        let (fx, by) = ((px + 14.0 * s).round(), (py + ph - 32.0 * s).round());
        if self.ui.button_ex(t("worlds.delete"), fx, by, bw, 22.0 * s, true, ButtonKind::Danger) {
            self.saver.wait();
            save::delete_world(&self.worlds[i].folder);
            self.open_world_list();
        }
        if self.ui.button(t("gui.cancel"), fx + fw - bw, by, bw, 22.0 * s, true) {
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
        // The inventory, then what is worn.
        let mut all = [None; crate::item::inventory::SIZE + crate::item::ARMOR_SLOTS];
        save::load_inventory(&meta.folder, &mut all);
        let (carried, worn) = all.split_at(crate::item::inventory::SIZE);
        self.inventory.slots.copy_from_slice(carried);
        self.inventory.armor.copy_from_slice(worn);
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
        // The cuts in its trunks; nothing of the last world's felling.
        felling::load_notches(&save::load_notches(&meta.folder));
        self.falling_trees.clear();
        self.chop = None;
        self.stump_struck = None;
        self.cursor = None;
        self.craft = [None; 9];
        self.bench_anims.clear();
        self.guns.cases.clear();
        self.drag = None;
        self.mining = None;
        self.target = None;
        self.chest_open.clear();
        self.air = MAX_AIR;
        self.invuln = 0.0;
        self.spectating = None;
        self.game_mode = if meta.spectator {
            GameMode::Spectator
        } else if meta.creative {
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
                    flying: p.flying && !matches!(self.game_mode, GameMode::Survival),
                    noclip: self.spectator(),
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
                // A new player starts with the guide book.
                self.give(crate::item::Stack::one(crate::item::GUIDE_BOOK));
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
        let mut slots = self.carried_slots().to_vec();
        slots.extend(self.inventory.armor);
        let meta = self.world_meta.as_mut().unwrap();
        meta.last_played = save::now_secs();
        meta.time_of_day = self.time_of_day;
        meta.spawn = Some(self.spawn);
        meta.bed = self.bed_spawn;
        meta.creative = self.game_mode == GameMode::Creative;
        meta.spectator = self.game_mode == GameMode::Spectator;
        if player.is_some() {
            meta.player = player;
        }
        meta.save();
        let folder = meta.folder.clone();
        save::save_inventory(&folder, &slots);
        save::save_notches(&folder, &felling::notches_text());
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
        self.land_falling_trees();
        self.save_world();
        self.close_lan();
        self.world_meta = None;
        self.player.spawned = false;
        self.screen = Screen::MainMenu;
        self.set_grab(false);
    }
}


/// A block drawn as a little isometric icon (a world's picture).
fn block_icon(ui: &mut crate::ui::Ui, c: Vec2, r: f32, b: u8) {
    use crate::world::{face_texture, icon_tint, tint_kind, TintKind};
    let tint = icon_tint(b);
    let top = if tint_kind(b, 2) != TintKind::None { tint } else { [255; 3] };
    let side = if tint_kind(b, 0) != TintKind::None { tint } else { [255; 3] };
    ui.block_icon_faces(c, r, face_texture(b, 2), face_texture(b, 5), face_texture(b, 0), top, side);
}

/// One world in the list: its mode's block, its name, chips for its mode and cheats, and
/// when it was last played. Lifted a little and lit when hovered; the selected one bordered
/// with the accent, a bar of it at its left.
#[allow(clippy::too_many_arguments)]
fn world_card(ui: &mut crate::ui::Ui, meta: &WorldMeta, x: f32, y: f32, w: f32, h: f32, hovered: bool, selected: bool) {
    let s = ui.s;
    let r = 5.0 * s;
    let lift = if hovered && !selected { -s } else { 0.0 };
    let y = y + lift;
    ui.rect_full(x, y + 3.0 * s, w, h, rgba(0, 0, 0, 80), rgba(0, 0, 0, 100), r, 7.0 * s);
    let border = if selected {
        ACCENT_LIGHT
    } else if hovered {
        rgba(255, 255, 255, 50)
    } else {
        rgba(255, 255, 255, 18)
    };
    ui.rect(x, y, w, h, border, r);
    let (top, bot) = if selected {
        (rgba(32, 32, 42, 235), rgba(28, 28, 36, 240))
    } else if hovered {
        (rgba(32, 32, 40, 228), rgba(26, 26, 32, 232))
    } else {
        (GLASS_TOP, GLASS_BOTTOM)
    };
    ui.rect_full(x + 1.0, y + 1.0, w - 2.0, h - 2.0, top, bot, r - 1.0, 0.0);
    if selected {
        ui.rect(x + 3.0 * s, y + 6.0 * s, 2.0 * s, h - 12.0 * s, ACCENT, s);
    }
    // The picture: the block of its mode on a dark tile.
    let tile = h - 10.0 * s;
    ui.rect(x + 8.0 * s, y + 5.0 * s, tile, tile, rgba(0, 0, 0, 70), 4.0 * s);
    let block = if meta.spectator {
        crate::world::GLASS
    } else if meta.creative {
        crate::world::DIAMOND_BLOCK
    } else {
        crate::world::GRASS
    };
    block_icon(ui, Vec2::new(x + 8.0 * s + tile * 0.5, y + 5.0 * s + tile * 0.5), tile * 0.36, block);

    let tx = x + 14.0 * s + tile;
    ui.text(&meta.name, tx, y + 7.0 * s, s, WHITE, true);
    let date = format_date(meta.last_played);
    let dw = ui.text_width(&date, s);
    ui.text(&date, x + w - dw - 9.0 * s, y + 7.0 * s, s, rgba(140, 144, 160, 255), false);
    let (mode, color) = if meta.spectator {
        (t("mode.spectator"), rgba(150, 156, 176, 255))
    } else if meta.creative {
        (t("mode.creative"), rgba(170, 128, 255, 255))
    } else {
        (t("mode.survival"), rgba(96, 204, 120, 255))
    };
    let cy = y + h - 16.0 * s;
    let cw = ui.chip(mode, tx, cy, color);
    if meta.cheats {
        ui.chip(t("worlds.cheats"), tx + cw + 4.0 * s, cy, ACCENT);
    }
}
