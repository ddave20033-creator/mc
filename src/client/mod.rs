//! The game's client: the window, the menus, the player and what they see of the world.
//! The world itself always runs on a server (`sim::server`: the game's own on a thread, or
//! a LAN host's); this side is connected to it and shows and follows what it sends.

mod bench;
mod boot;
mod book;
mod commands;
mod frame;
mod gfx;
mod gui;
mod input;
mod lighting;
mod menus;
mod multi;
mod player;
mod scene;
mod shown;
mod testbed;
mod tools;
mod update;
mod worlds;

use frame::FrameClock;
use gfx::{Gfx, Skins};
use gui::hud::Hud;
use gui::{station, BenchUi, InventoryUi};
use input::Input;
use menus::Menus;
use multi::Session;
use player::Me;
use shown::Level;
use testbed::TestModes;
use tools::Tools;

use crate::item::ItemId;
use crate::app::settings::Settings;
use crate::ui::{Color, Ui};
use crate::ui::chat::Chat;
use crate::util::Rng;
use crate::world::*;
use crate::world::gen::SEA;
use crate::world::terrain::Terrain;
use glam::{IVec3, Vec3};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use winit::window::{Fullscreen, Window};

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
    /// Spectator mode: the other players, to watch one through their eyes.
    Spectate,
}

/// The game: the window and its frames, the menus, and the world played (as a client of its
/// server). Its parts are each kept with the code that works on them (see the modules).
pub struct Game {
    /// The window, the GPU, the renderer and the textures.
    gfx: Gfx,
    ui: Ui,
    /// Sound effects.
    audio: crate::audio::Audio,
    settings: Settings,
    screen: Screen,
    /// The keyboard and the mouse as they are this frame.
    input: Input,
    /// Frame timing, the game's time and the ticks.
    clock: FrameClock,
    /// The start-up screen, while it is up.
    boot: Option<boot::Boot>,
    /// The menus' state: the world list and the new world's settings, the multiplayer screen,
    /// the options, the resource packs and the title screen's player and panorama.
    menus: Menus,
    /// The inventory screens: the creative tabs, list and search, the JEI panel, dragging and
    /// clicking slots, and what the mouse is on at an open chest or table.
    inv_ui: InventoryUi,
    /// At the open gun station: the brush, the mouse on its table and in its drawer, the camera,
    /// scrubbing, and what was last sent.
    bench_ui: BenchUi,
    /// The camera over an open chest or crafting table (and gliding back after).
    station: Option<station::Station>,
    /// What the HUD keeps between frames (F1, F3, the hotbar's highlight, names and hints).
    hud: Hud,
    chat: Chat,
    /// The guide book in the hands: its open page, and its pages' textures.
    book: book::Book,
    /// The connection to the world's server, and the other players in it.
    session: Session,
    /// The blocks of the world played as this game knows them (out of a world: the title
    /// screen's panorama).
    terrain: Terrain,
    /// What else is known of the world played (`shown::Level`).
    level: Level,
    /// This game's player.
    me: Me,
    /// Guns, grenades and the fishing rod.
    tools: Tools,
    rng: Rng,
    /// `--bench`, `--aa-shots` and `--test` runs.
    test: TestModes,
    pub quit: bool,
}

impl Game {
    pub fn new(
        window: Arc<Window>,
        bench: bool,
        shots: Option<std::path::PathBuf>,
        test: Option<(String, Option<std::path::PathBuf>)>,
    ) -> Self {
        let bench = bench || shots.is_some();
        let mut settings = Settings::load();
        let skins = Skins::load();
        if settings.skin == 4 && !skins.custom.contains_key(&0) {
            settings.skin = 0;
        }
        if settings.fullscreen && !bench {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let ui = Ui::new();
        // The textures are made on another thread while the start-up screen shows the logo
        // (the only textures it needs); they replace these when they are ready.
        let boot = boot::Boot::start(settings.resource_packs.clone(), skins.custom.clone());
        let gfx = Gfx::new(window, settings.msaa, &ui.font.atlas, skins);
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(1337);
        // A throwaway world for the title screen panorama.
        let terrain = Terrain::new(seed);
        let pano = Self::panorama_pos(&terrain, terrain.gen.find_spawn());

        Self {
            gfx,
            ui,
            audio: crate::audio::Audio::new(),
            screen: Screen::MainMenu,
            input: Input::new(),
            clock: FrameClock::new(),
            boot: Some(boot),
            menus: Menus::new(pano),
            inv_ui: InventoryUi::new(),
            bench_ui: BenchUi::new(),
            station: None,
            hud: Hud::default(),
            chat: Chat::new(),
            book: Default::default(),
            session: Session::new(),
            terrain,
            level: Level::new(),
            me: Me::new(settings.fov),
            tools: Default::default(),
            rng: Rng::new(seed),
            test: TestModes {
                bench: bench.then(Default::default),
                shots: shots.map(bench::Shots::new),
                testbed: test.map(|(what, dir)| testbed::Testbed::new(&what, dir)),
                no_blur: false,
            },
            settings,
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
        let lan = self.session.net.is_some();
        self.leave_server(None);
        if !self.test.any() {
            self.settings.save();
        }
        if lan {
            // Let the last messages (the player's state, the goodbye) go out.
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }

    fn in_world_view(&self) -> bool {
        self.level.meta.is_some()
            && matches!(
                self.screen,
                Screen::Playing
                    | Screen::Paused
                    | Screen::Container(_)
                    | Screen::Chat
                    | Screen::Spectate
                    | Screen::Dead
                    | Screen::Options { in_game: true }
                    | Screen::ResourcePacks { in_game: true }
                    | Screen::KeyBinds { in_game: true }
            )
    }

    fn creative(&self) -> bool {
        self.me.creative()
    }

    fn spectator(&self) -> bool {
        self.me.spectator()
    }

    /// The skin this player is drawn with: the chosen one, or the uploaded one in this
    /// player's own slot (the default when there are too many players for slots).
    fn effective_skin(&self) -> u8 {
        if self.settings.skin != 4 {
            return self.settings.skin;
        }
        let id = self.session.my_id();
        if id < textures::tex::CUSTOM_SKIN_SLOTS {
            4 + id
        } else {
            0
        }
    }

    /// Remakes the textures from the enabled resource packs (after the pack screen).
    fn reload_packs(&mut self) {
        self.gfx.reload_packs(&self.settings.resource_packs);
        self.book.textures_remade();
    }

    fn say(&mut self, text: impl Into<String>, color: Color) {
        let now = self.clock.time;
        self.chat.push(text, color, now);
    }

    fn random(&mut self) -> f32 {
        self.rng.next()
    }

    /// Item in the selected hotbar slot.
    fn held(&self) -> ItemId {
        self.me.items.held()
    }

    /// The game's window (asked to draw its next frame).
    pub fn request_redraw(&self) {
        self.gfx.window.request_redraw();
    }
}

