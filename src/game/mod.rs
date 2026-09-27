mod bench;
mod blocks;
mod camera;
mod commands;
mod frame;
mod furnace;
mod gui;
mod guns;
mod health;
mod hud;
mod items;
mod mobs;
mod multi;
mod sleep;
mod station;
mod update;
mod worlds;

use crate::engine::Gpu;
use crate::entity::mob::{Mob, MobCtx, MobEvent, MobKind};
use crate::entity::player::{look_dir, Player};
use crate::entity::survival::{EffectKind, Needs};
use crate::entity::{BlockEntities, FallingBlock, ItemEntity};
use crate::item::inventory::Inventory;
use crate::item::{self, ItemId, Slot, NONE};
use crate::keys::{Bind, HOTBAR};
use crate::lang::t;
use crate::model::crack_overlay;
use crate::model::hand::HandAnim;
use crate::model::particles::Particles;
use crate::model::player::{
    build_player, hand_pivot, held_up, limb_targets, LimbSmoother, PlayerPose,
};
use crate::render::{FrameInfo, FrameUbo, Renderer, SHADOW_SIZE};
use crate::save::{ChunkSaver, PlayerSave, WorldMeta};
use crate::settings::Settings;
use crate::ui::chat::{self, Chat, ChatInput};
use crate::ui::screens::{self, Action, SPLASHES};
use crate::ui::{rgba, with_alpha, Color, Ui, WHITE};
use crate::util::{smoothstep, Rng};
use crate::world::fluid::Fluids;
use crate::world::gen::SEA;
use crate::world::mesh::Vertex;
use crate::world::terrain::{Terrain, TerrainEvent};
use crate::world::*;
use glam::{IVec3, Mat4, Vec2, Vec3};
use std::collections::HashSet;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window};

const DAY_LENGTH: f32 = 1200.0;
const MENU_TIME_OF_DAY: f32 = 0.085;
const SHADOW_DISTANCE: f32 = 96.0;
const MAX_HEALTH: f32 = 20.0;
const AUTOSAVE_SECONDS: f32 = 60.0;
/// Seconds of breath underwater (Minecraft: 300 ticks).
const MAX_AIR: f32 = 15.0;

/// Open item screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Container {
    /// Survival inventory with the 2x2 crafting grid.
    Inventory,
    /// Crafting table (3x3) at a position; the grid stays in the table.
    Crafting(IVec3),
    Chest(IVec3),
    /// Gun station: putting a pistol together and cleaning it.
    GunStation(IVec3),
    Creative,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    MainMenu,
    Skin,
    Options {
        in_game: bool,
    },
    /// The resource pack screen (from the options).
    ResourcePacks {
        in_game: bool,
    },
    /// The key binds screen (from the options).
    KeyBinds {
        in_game: bool,
    },
    Credits,
    SelectWorld,
    CreateWorld,
    DeleteWorld,
    Loading,
    Playing,
    Paused,
    Container(Container),
    Chat,
    Dead,
    /// LAN games list and direct connection.
    Multiplayer,
    Connecting,
    /// Left a LAN game (or could not join): shows why.
    Disconnected,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Survival,
    Creative,
}

/// Opens a web page in the default browser.
fn open_url(url: &str) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}

pub struct Game {
    // `renderer` holds GPU resources and is destroyed explicitly in Drop before `gpu`.
    renderer: Renderer,
    gpu: Gpu,
    pub window: Arc<Window>,
    ui: Ui,
    settings: Settings,
    screen: Screen,
    terrain: Terrain,
    fluids: Fluids,
    spawn: (i32, i32),
    /// The head of the bed this player last used: where they come back to life.
    bed_spawn: Option<IVec3>,
    /// Lying in a bed.
    sleep: Option<sleep::Sleep>,
    /// Seconds everyone has been asleep (host and single player).
    asleep_for: f32,
    pano: Vec3,
    chat: Chat,

    // World management
    world_meta: Option<WorldMeta>,
    pending_player: Option<PlayerSave>,
    worlds: Vec<WorldMeta>,
    selected_world: Option<usize>,
    world_scroll: f32,
    last_click: (usize, f32),
    create_name: String,
    create_seed: String,
    create_creative: bool,
    create_cheats: bool,
    autosave: f32,
    saver: ChunkSaver,

    // Player state
    player: Player,
    game_mode: GameMode,
    cheats: bool,
    health: f32,
    invuln: f32,
    /// Blocking with a sword (right mouse button held).
    blocking: bool,
    hurt_time: f32,
    last_damage: f32,
    /// Hunger, thirst and effects.
    needs: Needs,
    /// Eating or drinking: the item and seconds spent so far.
    using: Option<(ItemId, f32)>,
    fall_peak: f32,
    fire: f32,
    fire_tick: f32,
    /// Breath left underwater, in seconds.
    air: f32,
    drown_tick: f32,
    death_message: String,
    yaw: f32,
    pitch: f32,
    body_yaw: f32,
    limb_swing: f32,
    limb_amount: f32,
    /// Smoothed arm and leg rotations of the player model.
    limbs: LimbSmoother,
    /// F5 view mode and the third-person camera's state.
    camera: camera::Rig,
    target: Option<(IVec3, IVec3)>,
    /// The furnace part under the crosshair: a corner of the top (0..4) or the front's
    /// upper or lower half (`block_entity::part`).
    furnace_part: Option<(IVec3, u8)>,
    /// Something was just taken out of a furnace with the left button, still held: it does
    /// not start mining the furnace.
    furnace_hold: bool,
    /// Where the look ray meets the targeted block.
    target_point: Vec3,

    // Items
    inventory: Inventory,
    hotbar_slot: usize,
    /// Drawn position of the hotbar highlight; glides toward `hotbar_slot`.
    hotbar_anim: f32,
    cursor: Slot,
    craft: [Slot; 9],
    /// Creative list scroll in rows: the target set by the wheel, and the eased position.
    creative_scroll: f32,
    creative_scroll_anim: f32,
    /// Dragging the creative scroll bar.
    scroll_drag: bool,
    /// Creative inventory search text; typing goes to it while it is focused.
    creative_search: String,
    search_focused: bool,
    /// Chest lid animation 0..1 per chest position.
    chest_open: crate::world::FastMap<IVec3, f32>,
    /// How far each door half is swung open (0..1), easing toward its state.
    door_swing: crate::world::FastMap<IVec3, f32>,
    /// Slot drag in progress (Minecraft-style stack spreading).
    drag: Option<gui::Drag>,
    /// The slot a stack was just picked up from with the button still held: letting go
    /// over another slot puts it there.
    press_pick: Option<gui::SlotRef>,
    /// The camera over an open chest or crafting table (and gliding back after).
    station: Option<station::Station>,
    /// What the mouse points at in the open chest or on the open table, and the corners of
    /// its highlighted slot.
    station_hover: Option<gui::SlotRef>,
    station_frame: Option<[Vec3; 4]>,
    /// The mouse is over the open chest or table, or the inventory under it: a click there
    /// does not throw the held stack.
    station_inside: bool,
    /// The side each crafting table was last used from (its grid faces that way).
    table_sides: crate::world::FastMap<IVec3, u8>,
    /// What was crafted at the open table, lying in the middle of its grid until taken.
    craft_out: Slot,
    /// The ingredients sliding into the middle of the table: seconds since, and the grid as
    /// it was.
    craft_fx: Option<(f32, [Slot; 9])>,

    /// Time and slot of the last left click, for double-click collecting.
    slot_click: (f32, Option<gui::SlotRef>),
    block_entities: BlockEntities,
    /// Shooting and the gun station.
    guns: guns::Guns,
    items: Vec<ItemEntity>,
    falling: Vec<FallingBlock>,
    mobs: Vec<Mob>,
    /// Seconds until the next try to spawn animals near the player.
    mob_spawn_timer: f32,
    /// The mob the crosshair is on (index into `mobs`), when it is closer than any block.
    mob_target: Option<usize>,
    saplings: Vec<(IVec3, f32)>,

    slot_name_timer: f32,
    hint_timer: f32,
    mining: Option<(IVec3, f32)>,
    dig_timer: f32,
    action_cooldown: f32,
    hand: HandAnim,
    particles: Particles,
    time_of_day: f32,
    fov_current: f32,
    /// Field of view for simplifying detail too small for the screen (setting and zoom only).
    detail_fov: f32,
    last_space: f32,
    last_w: f32,
    w_sprint: bool,
    rng: Rng,

    // Input
    keys: HashSet<KeyCode>,
    left_down: bool,
    right_down: bool,
    left_pressed: bool,
    right_pressed: bool,
    middle_pressed: bool,
    mouse_delta: Vec2,
    look_delta: Vec2,
    scroll: f32,
    cursor_grabbed: bool,
    typed: String,
    backspace: u32,
    digit: Option<usize>,

    last: Instant,
    time: f32,
    fps: f32,
    fps_accum: f32,
    fps_frames: u32,
    splash: &'static str,
    /// (title, description) of the resource pack in use, for the credits.
    pack_credit: Option<(String, String)>,
    /// The resource pack draws torch fire as flame/smoke particles (like Minecraft).
    torch_particles: bool,
    /// Torches near the player (rescanned every second) and the rescan timer.
    torches: Vec<IVec3>,
    torch_scan: f32,
    /// Where the torch in this player's hand burns (seen last frame), for its particles.
    held_torch_tip: Option<Vec3>,
    show_debug: bool,
    /// `--bench` mode state.
    bench: Option<bench::Bench>,
    /// `--aa-shots`: anti-aliasing comparison pictures (runs in bench mode).
    shots: Option<bench::Shots>,
    /// Max FPS: when the next frame may start.
    next_frame: Option<Instant>,
    /// Last frame's CPU time in ms: update, build, submit (without waiting), waiting for the GPU.
    cpu_ms: [f32; 4],
    /// When the previous frame finished, and the time from then until this frame started.
    frame_end: Instant,
    pub between_ms: f32,
    /// F1: hide the HUD and the hand (for screenshots), like Minecraft.
    hide_hud: bool,
    /// Recent frame times in milliseconds (newest last), for the F3 graph.
    frame_times: std::collections::VecDeque<f32>,
    sys_stats: crate::stats::Monitor,
    /// Video memory (used, budget) in bytes, refreshed once a second.
    vram: Option<(u64, u64)>,
    vram_timer: f32,
    /// LAN game: hosting or joined, the other players, and the multiplayer screen state.
    net: Option<multi::Net>,
    remotes: Vec<multi::RemotePlayer>,
    next_entity_id: u32,
    finder: Option<crate::net::Finder>,
    mp_address: String,
    mp_selected: Option<usize>,
    net_message: String,
    /// The other player the crosshair is on.
    player_target: Option<u8>,
    /// Last frame's camera matrix (for name tags).
    view_proj: Mat4,
    /// Swing of the lantern in this player's hand (third person and body model).
    lantern_swing: crate::model::lantern::SmoothSwing,
    /// Open tab of the options screen.
    options: screens::OptionsState,
    pack_screen: screens::PackScreen,
    menu_preview: screens::PreviewRotation,
    /// All texture layers but the uploaded skins (see `textures::generate_base`).
    texture_base: Vec<u8>,
    custom_skins: std::collections::HashMap<u8, crate::pack::Image>,
    skin_pngs: std::collections::HashMap<u8, Vec<u8>>,
    local_skin_png: Option<Vec<u8>>,
    skin_error: String,
    pub quit: bool,
}

impl Game {
    pub fn new(window: Arc<Window>, bench: bool, shots: Option<std::path::PathBuf>) -> Self {
        let bench = bench || shots.is_some();
        let mut settings = Settings::load();
        let mut custom_skins = std::collections::HashMap::new();
        let mut skin_pngs = std::collections::HashMap::new();
        if let Ok(bytes) = std::fs::read("skins/custom.png") {
            if let Ok(image) = textures::decode_skin_png(&bytes) {
                custom_skins.insert(0, image);
                skin_pngs.insert(0, bytes);
            }
        }
        if settings.skin == 4 && !custom_skins.contains_key(&0) {
            settings.skin = 0;
        }
        let local_skin_png = skin_pngs.get(&0).cloned();
        if settings.fullscreen && !bench {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let gpu = Gpu::new(&window, false, settings.msaa);
        println!("RustCraft running on: {}", gpu.device_name);
        let ui = Ui::new();
        let packs = crate::pack::Packs::load(&settings.resource_packs);
        let texture_base = textures::generate_base(&packs);
        let renderer = Renderer::new(
            &gpu,
            &textures::with_skins(&texture_base, &custom_skins),
            &ui.font.atlas,
        );
        let torch_particles = packs.texture("particle/flame").is_some();
        // The credits name the built-in pack (always in use).
        let pack_credit = packs
            .0
            .last()
            .map(|p| (p.title().to_string(), p.description.clone()));
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(1337);
        // A throwaway world for the title screen panorama.
        let terrain = Terrain::new(seed);
        let spawn = terrain.gen.find_spawn();
        let pano = Self::panorama_pos(&terrain, spawn);

        Self {
            renderer,
            gpu,
            window,
            ui,
            fov_current: settings.fov,
            detail_fov: settings.fov,
            settings,
            screen: Screen::MainMenu,
            terrain,
            fluids: Fluids::new(),
            spawn,
            bed_spawn: None,
            sleep: None,
            asleep_for: 0.0,
            pano,
            chat: Chat::new(),
            world_meta: None,
            pending_player: None,
            worlds: Vec::new(),
            selected_world: None,
            world_scroll: 0.0,
            last_click: (usize::MAX, -10.0),
            create_name: String::new(),
            create_seed: String::new(),
            create_creative: false,
            create_cheats: false,
            autosave: AUTOSAVE_SECONDS,
            saver: ChunkSaver::default(),
            player: Player::default(),
            game_mode: GameMode::Survival,
            cheats: false,
            health: MAX_HEALTH,
            invuln: 0.0,
            blocking: false,
            hurt_time: 0.0,
            last_damage: 0.0,
            needs: Needs::new(),
            using: None,
            fall_peak: 0.0,
            fire: 0.0,
            fire_tick: 0.0,
            air: MAX_AIR,
            drown_tick: 0.0,
            death_message: String::new(),
            yaw: 0.0,
            pitch: 0.0,
            body_yaw: 0.0,
            limb_swing: 0.0,
            limb_amount: 0.0,
            limbs: LimbSmoother::default(),
            camera: camera::Rig::default(),
            target: None,
            furnace_part: None,
            furnace_hold: false,
            target_point: Vec3::ZERO,
            inventory: Inventory::new(),
            hotbar_slot: 0,
            hotbar_anim: 0.0,
            cursor: None,
            craft: [None; 9],
            creative_scroll: 0.0,
            creative_scroll_anim: 0.0,
            scroll_drag: false,
            creative_search: String::new(),
            search_focused: false,
            chest_open: Default::default(),
            door_swing: Default::default(),
            drag: None,
            press_pick: None,
            station: None,
            station_hover: None,
            station_frame: None,
            station_inside: false,
            table_sides: Default::default(),
            craft_out: None,
            craft_fx: None,

            slot_click: (-1.0, None),
            block_entities: BlockEntities::default(),
            guns: Default::default(),
            items: Vec::new(),
            falling: Vec::new(),
            mobs: Vec::new(),
            mob_spawn_timer: 5.0,
            mob_target: None,
            saplings: Vec::new(),
            slot_name_timer: 0.0,
            hint_timer: 0.0,
            mining: None,
            dig_timer: 0.0,
            action_cooldown: 0.0,
            hand: HandAnim::new(),
            particles: Particles::new(),
            time_of_day: MENU_TIME_OF_DAY,
            last_space: -1.0,
            last_w: -1.0,
            w_sprint: false,
            rng: Rng::new(seed),
            keys: HashSet::new(),
            left_down: false,
            right_down: false,
            left_pressed: false,
            right_pressed: false,
            middle_pressed: false,
            mouse_delta: Vec2::ZERO,
            look_delta: Vec2::ZERO,
            scroll: 0.0,
            cursor_grabbed: false,
            typed: String::new(),
            backspace: 0,
            digit: None,
            last: Instant::now(),
            time: 0.0,
            fps: 0.0,
            fps_accum: 0.0,
            fps_frames: 0,
            splash: SPLASHES[seed as usize % SPLASHES.len()],
            pack_credit,
            torch_particles,
            torches: Vec::new(),
            torch_scan: 0.0,
            held_torch_tip: None,
            show_debug: false,
            bench: bench.then(Default::default),
            shots: shots.map(bench::Shots::new),
            next_frame: None,
            cpu_ms: [0.0; 4],
            frame_end: Instant::now(),
            between_ms: 0.0,
            hide_hud: false,
            frame_times: std::collections::VecDeque::with_capacity(hud::FRAME_GRAPH),
            sys_stats: crate::stats::start(),
            vram: None,
            vram_timer: 0.0,
            net: None,
            remotes: Vec::new(),
            next_entity_id: 0,
            finder: None,
            mp_address: String::new(),
            mp_selected: None,
            net_message: String::new(),
            player_target: None,
            view_proj: Mat4::IDENTITY,
            lantern_swing: Default::default(),
            options: Default::default(),
            pack_screen: Default::default(),
            menu_preview: Default::default(),
            texture_base,
            custom_skins,
            skin_pngs,
            local_skin_png,
            skin_error: String::new(),
            quit: false,
        }
    }

    fn panorama_pos(terrain: &Terrain, spawn: (i32, i32)) -> Vec3 {
        let mut peak = SEA;
        for dz in -4..=4 {
            for dx in -4..=4 {
                peak = peak.max(
                    terrain
                        .gen
                        .column(spawn.0 + dx * 6, spawn.1 + dz * 6)
                        .height,
                );
            }
        }
        Vec3::new(
            spawn.0 as f32 + 0.5,
            (peak + 14) as f32,
            spawn.1 as f32 + 0.5,
        )
    }

    /// Called when the application exits.
    pub fn on_exit(&mut self) {
        let lan = self.net.is_some();
        if self.is_client() {
            self.leave_server(None);
        } else {
            self.save_world();
            self.close_lan();
        }
        self.saver.wait();
        if self.bench.is_none() {
            self.settings.save();
        }
        if lan {
            // Let the last messages (the player's state, the goodbye) go out.
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }

    fn in_world_view(&self) -> bool {
        self.world_meta.is_some()
            && matches!(
                self.screen,
                Screen::Playing
                    | Screen::Paused
                    | Screen::Container(_)
                    | Screen::Chat
                    | Screen::Dead
                    | Screen::Options { in_game: true }
                    | Screen::ResourcePacks { in_game: true }
                    | Screen::KeyBinds { in_game: true }
            )
    }

    fn creative(&self) -> bool {
        self.game_mode == GameMode::Creative
    }

    fn effective_skin(&self) -> u8 {
        if self.settings.skin != 4 {
            return self.settings.skin;
        }
        let id = match &self.net {
            Some(multi::Net::Client(c)) => c.id,
            _ => 0,
        };
        if id < textures::tex::CUSTOM_SKIN_SLOTS {
            4 + id
        } else {
            0
        }
    }

    /// Remakes the textures from the enabled resource packs (after the pack screen).
    fn reload_packs(&mut self) {
        let packs = crate::pack::Packs::load(&self.settings.resource_packs);
        self.texture_base = textures::generate_base(&packs);
        self.torch_particles = packs.texture("particle/flame").is_some();
        self.refresh_skin_textures();
    }

    fn refresh_skin_textures(&mut self) {
        let levels = textures::with_skins(&self.texture_base, &self.custom_skins);
        self.renderer.replace_block_textures(&self.gpu, &levels);
    }

    fn set_skin_png(&mut self, slot: u8, png: Vec<u8>) -> Result<(), &'static str> {
        if slot >= textures::tex::CUSTOM_SKIN_SLOTS {
            return Err("Túl sok LAN játékos.");
        }
        let image = textures::decode_skin_png(&png)?;
        self.custom_skins.insert(slot, image);
        self.skin_pngs.insert(slot, png);
        self.refresh_skin_textures();
        Ok(())
    }

    fn say(&mut self, text: impl Into<String>, color: Color) {
        let now = self.time;
        self.chat.push(text, color, now);
    }

    fn random(&mut self) -> f32 {
        self.rng.next()
    }

    /// Holding the sneak key (it also flies down and places chests unjoined).
    fn sneaking(&self) -> bool {
        self.bind_down(Bind::Sneak)
    }

    /// The key of this action is held.
    fn bind_down(&self, b: Bind) -> bool {
        self.keys.contains(&self.settings.keys.get(b))
    }

    /// Item in the selected hotbar slot.
    fn held(&self) -> ItemId {
        self.inventory.slots[self.hotbar_slot]
            .map(|s| s.item)
            .unwrap_or(NONE)
    }

    // ---------------- input ----------------

    pub fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => self.gpu.resize(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                self.ui.mouse = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = *state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        self.left_down = pressed;
                        self.left_pressed |= pressed;
                    }
                    MouseButton::Right => {
                        self.right_down = pressed;
                        self.right_pressed |= pressed;
                    }
                    MouseButton::Middle => self.middle_pressed |= pressed,
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if event.state == ElementState::Pressed {
                    // Options: the next key goes to the action waiting for one.
                    if let (Screen::KeyBinds { .. }, Some(i)) =
                        (self.screen, self.options.listening)
                    {
                        if code == KeyCode::Escape {
                            self.options.listening = None;
                        } else if crate::keys::bindable(code) {
                            self.settings.keys.0[i] = code;
                            self.options.listening = None;
                        }
                        return;
                    }
                    if self.screen == Screen::Chat {
                        match self.chat.key(code, event.text.as_ref().map(|t| t.as_str())) {
                            ChatInput::None => {}
                            ChatInput::Close => self.resume(),
                            ChatInput::Submit(line) => {
                                self.resume();
                                self.run_command(&line);
                            }
                        }
                        return;
                    }
                    if self.screen == Screen::Container(Container::Creative) && self.search_focused
                    {
                        self.search_key(code, event.text.as_ref().map(|t| t.as_str()));
                        return;
                    }
                    if self.screen == Screen::Multiplayer {
                        match code {
                            KeyCode::Escape => {
                                self.finder = None;
                                self.settings.save();
                                self.screen = Screen::MainMenu;
                            }
                            KeyCode::Backspace => self.backspace += 1,
                            KeyCode::Enter | KeyCode::NumpadEnter => {
                                if !self.mp_address.trim().is_empty() {
                                    let addr = self.mp_address.clone();
                                    self.join_server(&addr);
                                }
                            }
                            _ => {
                                if let Some(t) = &event.text {
                                    self.typed.push_str(t);
                                }
                            }
                        }
                        return;
                    }
                    if self.screen == Screen::CreateWorld {
                        match code {
                            KeyCode::Escape => self.screen = Screen::SelectWorld,
                            KeyCode::Backspace => self.backspace += 1,
                            KeyCode::Enter | KeyCode::NumpadEnter => self.create_world(),
                            _ => {
                                if let Some(t) = &event.text {
                                    self.typed.push_str(t);
                                }
                            }
                        }
                        return;
                    }
                    if !event.repeat {
                        self.key_pressed(code);
                    }
                    self.keys.insert(code);
                } else {
                    self.keys.remove(&code);
                    if self.settings.keys.is(Bind::Forward, code) {
                        self.w_sprint = false;
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.w_sprint = false;
                self.last_w = -1.0;
                self.left_down = false;
                self.right_down = false;
                // Bench and shot runs keep going in the background.
                if self.screen == Screen::Playing && self.bench.is_none() {
                    self.pause();
                }
            }
            _ => {}
        }
    }

    pub fn mouse_motion(&mut self, dx: f32, dy: f32) {
        if self.cursor_grabbed && self.screen == Screen::Playing {
            self.mouse_delta += Vec2::new(dx, dy);
        }
    }

    fn key_pressed(&mut self, code: KeyCode) {
        let is = |b: Bind| self.settings.keys.is(b, code);
        let digit = HOTBAR.iter().position(|&b| is(b));
        let forward = is(Bind::Forward);
        let stop_sprint = is(Bind::Back) || is(Bind::Sneak);
        let chat = is(Bind::Chat);
        let command = is(Bind::Command) || code == KeyCode::NumpadDivide;
        let drop = is(Bind::Drop);
        let fly = is(Bind::Fly);
        let jump = is(Bind::Jump);
        let fullscreen = is(Bind::Fullscreen);
        let hide_hud = is(Bind::HideHud);
        let debug = is(Bind::Debug);
        let perspective = is(Bind::Perspective);
        let inventory = is(Bind::Inventory);
        let reload = is(Bind::Reload);
        if code == KeyCode::Escape {
            match self.screen {
                Screen::Playing => self.pause(),
                Screen::Paused => self.resume(),
                Screen::Container(_) => self.close_container(),
                Screen::SelectWorld => self.screen = Screen::MainMenu,
                Screen::DeleteWorld => self.screen = Screen::SelectWorld,
                Screen::MainMenu | Screen::Dead => {}
                Screen::Connecting => {
                    self.leave_server(None);
                    self.open_multiplayer();
                }
                Screen::Disconnected => self.screen = Screen::MainMenu,
                _ => self.go_back(),
            }
        }
        if fullscreen {
            self.toggle_fullscreen();
        }
        if hide_hud {
            self.hide_hud = !self.hide_hud;
        }
        if debug {
            self.show_debug = !self.show_debug;
        }
        if perspective {
            self.camera.cycle();
        }
        if inventory {
            match self.screen {
                Screen::Playing => self.open_container(if self.creative() {
                    Container::Creative
                } else {
                    Container::Inventory
                }),
                Screen::Container(_) => self.close_container(),
                _ => {}
            }
        }
        if let Screen::Container(_) = self.screen {
            self.digit = digit;
            return;
        }
        if self.screen != Screen::Playing {
            return;
        }
        if let Some(i) = digit {
            self.hotbar_slot = i;
            self.slot_name_timer = 2.0;
        }
        if reload {
            self.guns.reload_pressed = true;
        }
        // Double tap forward to sprint.
        if forward && !self.keys.contains(&code) {
            if self.time - self.last_w < 0.3 {
                self.w_sprint = true;
                self.last_w = -1.0;
            } else {
                self.last_w = self.time;
            }
        }
        if stop_sprint {
            self.w_sprint = false;
        }
        if drop {
            let all = self.keys.contains(&KeyCode::ControlLeft)
                || self.keys.contains(&KeyCode::ControlRight);
            self.drop_held(all);
        }
        if fly && self.creative() {
            self.player.flying = !self.player.flying;
        }
        if jump && self.creative() {
            if self.time - self.last_space < 0.3 {
                self.player.flying = !self.player.flying;
                self.last_space = -1.0;
            } else {
                self.last_space = self.time;
            }
        }
        // Last: these leave the game screen.
        if chat {
            self.open_chat("");
        } else if command {
            self.open_chat("/");
        }
    }

    fn open_chat(&mut self, prefix: &str) {
        self.chat.open(prefix);
        self.screen = Screen::Chat;
        self.set_grab(false);
        self.keys.clear();
    }

    fn end_input(&mut self) {
        self.left_pressed = false;
        self.right_pressed = false;
        self.middle_pressed = false;
        self.look_delta = self.mouse_delta;
        self.mouse_delta = Vec2::ZERO;
        self.scroll = 0.0;
        self.typed.clear();
        self.backspace = 0;
        self.digit = None;
    }

    fn set_grab(&mut self, grab: bool) {
        let grab = grab && self.bench.is_none();
        if !grab {
            self.w_sprint = false;
            self.last_w = -1.0;
        }
        let size = self.window.inner_size();
        let center = PhysicalPosition::new(size.width as f64 / 2.0, size.height as f64 / 2.0);
        if grab {
            let _ = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined));
            self.window.set_cursor_visible(false);
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
            if self.cursor_grabbed {
                let _ = self.window.set_cursor_position(center);
                self.ui.mouse = Vec2::new(center.x as f32, center.y as f32);
            }
        }
        self.cursor_grabbed = grab;
        self.mouse_delta = Vec2::ZERO;
        self.left_down = false;
        self.right_down = false;
        self.mining = None;
    }

    fn toggle_fullscreen(&mut self) {
        self.settings.fullscreen = !self.settings.fullscreen;
        self.window.set_fullscreen(if self.settings.fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
    }

    /// Keep the model's head within a natural turn while the shoulder camera orbits freely.
    fn visual_head_yaw(&self) -> f32 {
        self.body_yaw + camera::head_turn(self.yaw, self.body_yaw)
    }

    // ---------------- screens ----------------

    fn pause(&mut self) {
        self.screen = Screen::Paused;
        self.set_grab(false);
    }

    fn resume(&mut self) {
        self.screen = Screen::Playing;
        self.set_grab(true);
    }

    fn go_back(&mut self) {
        match self.screen {
            Screen::KeyBinds { in_game } => {
                self.options.listening = None;
                self.settings.save();
                self.screen = Screen::Options { in_game };
            }
            Screen::Options { in_game } => {
                self.settings.save();
                self.screen = if in_game {
                    Screen::Paused
                } else {
                    Screen::MainMenu
                };
            }
            Screen::ResourcePacks { in_game } => {
                let enabled = self.pack_screen.enabled();
                if enabled != self.settings.resource_packs {
                    self.settings.resource_packs = enabled;
                    self.settings.save();
                    self.reload_packs();
                }
                self.screen = Screen::Options { in_game };
            }
            Screen::Credits => self.screen = Screen::MainMenu,
            Screen::Skin => self.screen = Screen::MainMenu,
            _ => {}
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Singleplayer => self.open_world_list(),
            Action::Multiplayer => self.open_multiplayer(),
            Action::SkinMenu => {
                self.skin_error.clear();
                self.screen = Screen::Skin;
            }
            Action::SelectSkin(skin) => {
                self.settings.skin = skin;
                self.settings.save();
                self.screen = Screen::MainMenu;
            }
            Action::UploadSkin => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Minecraft skin PNG", &["png"])
                    .set_title("Skin PNG kiválasztása")
                    .set_parent(&*self.window)
                    .pick_file()
                {
                    let result = std::fs::read(&path)
                        .map_err(|_| "Nem sikerült beolvasni a fájlt.")
                        .and_then(|data| self.set_skin_png(0, data.clone()).map(|_| data));
                    match result {
                        Ok(data) => {
                            let _ = std::fs::create_dir_all("skins");
                            if std::fs::write("skins/custom.png", data).is_ok() {
                                self.local_skin_png = std::fs::read("skins/custom.png").ok();
                                self.settings.skin = 4;
                                self.settings.save();
                                self.screen = Screen::MainMenu;
                            } else {
                                self.skin_error = "Nem sikerült menteni a skint.".into();
                            }
                        }
                        Err(msg) => self.skin_error = msg.into(),
                    }
                }
            }
            Action::OpenLan => self.open_to_lan(),
            Action::Options => {
                self.screen = Screen::Options {
                    in_game: self.screen == Screen::Paused,
                }
            }
            Action::Credits => self.screen = Screen::Credits,
            Action::Quit => self.quit = true,
            Action::Resume => self.resume(),
            Action::ToTitle => self.quit_to_title(),
            Action::Back => self.go_back(),
            Action::ToggleFullscreen => self.toggle_fullscreen(),
            Action::AntialiasingChanged => self.gpu.set_msaa(self.settings.msaa),
            Action::Respawn => self.respawn(),
            Action::Language => {
                self.settings.hungarian = !self.settings.hungarian;
                crate::lang::set_hungarian(self.settings.hungarian);
            }
            Action::OpenLink(url) => open_url(url),
            Action::ResourcePacks => {
                if let Screen::Options { in_game } = self.screen {
                    self.pack_screen.open(&self.settings.resource_packs);
                    self.screen = Screen::ResourcePacks { in_game };
                }
            }
            Action::KeyBinds => {
                if let Screen::Options { in_game } = self.screen {
                    self.screen = Screen::KeyBinds { in_game };
                }
            }
            Action::OpenPackFolder => {
                let _ = std::fs::create_dir_all(crate::pack::DIR);
                if let Ok(dir) = std::fs::canonicalize(crate::pack::DIR) {
                    // Without the \\?\ prefix, which Explorer does not take.
                    let dir = dir.to_string_lossy().trim_start_matches(r"\\?\").to_string();
                    open_url(&dir);
                }
            }
        }
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        self.renderer.destroy(&mut self.gpu);
    }
}
