//! Parts of the game's state kept together: what belongs to the world being played
//! (`Level`), the input, frame timing, and the state of the menus and item screens.

use super::*;

/// What is in the world being played besides its blocks: dropped items, falling blocks and
/// trees, lying trunks, mobs, saplings, block entities and the animations of things in it.
/// Made anew for every world loaded, so nothing of the last one comes along.
pub(super) struct Level {
    pub(super) items: Vec<ItemEntity>,
    pub(super) falling: Vec<FallingBlock>,
    /// Trees felled with an axe, falling over.
    pub(super) falling_trees: Vec<felling::FallingTree>,
    /// The trunks of felled trees lying on the ground, the last one's id, the one aimed at
    /// with an axe, and the one (and where) the swing going on will cut.
    pub(super) lying_logs: Vec<logs::LyingLog>,
    pub(super) next_log_id: u32,
    pub(super) mobs: Vec<Mob>,
    /// Seconds until the next try to spawn animals near the player.
    pub(super) mob_spawn_timer: f32,
    pub(super) saplings: Vec<(IVec3, f32)>,
    /// Seconds until the next look round for stump marks to grow over.
    pub(super) stump_scan: f32,
    pub(super) block_entities: BlockEntities,
    /// Chest lid animation 0..1 per chest position.
    pub(super) chest_open: crate::world::FastMap<IVec3, f32>,
    /// How far each door half is swung open (0..1), easing toward its state.
    pub(super) door_swing: crate::world::FastMap<IVec3, f32>,
    /// How far each gun station's drawer is out (0..1): it slides out while one is used.
    pub(super) bench_drawer: crate::world::FastMap<IVec3, f32>,
    /// Host: seconds each rifle station's magazine loader has been feeding the next round.
    pub(super) loader_feed: crate::world::FastMap<IVec3, f32>,
    /// The side each crafting table was last used from (its grid faces that way).
    pub(super) table_sides: crate::world::FastMap<IVec3, u8>,
    pub(super) furnace_heard: std::collections::HashMap<IVec3, u32>,
    /// The last change seen on each gun station's table and when it started (it plays out).
    pub(super) bench_anims: crate::world::FastMap<IVec3, (u16, f32)>,
    /// Torches near the player (rescanned every second) and the rescan timer.
    pub(super) torches: Vec<IVec3>,
    pub(super) torch_scan: f32,
}

impl Level {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            falling: Vec::new(),
            falling_trees: Vec::new(),
            lying_logs: Vec::new(),
            next_log_id: 0,
            mobs: Vec::new(),
            mob_spawn_timer: 5.0,
            saplings: Vec::new(),
            stump_scan: 0.0,
            block_entities: BlockEntities::default(),
            chest_open: Default::default(),
            door_swing: Default::default(),
            bench_drawer: Default::default(),
            loader_feed: Default::default(),
            table_sides: Default::default(),
            furnace_heard: Default::default(),
            bench_anims: Default::default(),
            torches: Vec::new(),
            torch_scan: 0.0,
        }
    }
}

/// The keyboard and the mouse as they are this frame (see `frame::end_input`).
pub(super) struct Input {
    pub(super) keys: HashSet<KeyCode>,
    pub(super) left_down: bool,
    pub(super) right_down: bool,
    pub(super) left_pressed: bool,
    pub(super) right_pressed: bool,
    pub(super) middle_pressed: bool,
    pub(super) mouse_delta: Vec2,
    pub(super) look_delta: Vec2,
    pub(super) scroll: f32,
    pub(super) cursor_grabbed: bool,
    pub(super) typed: String,
    pub(super) backspace: u32,
    pub(super) digit: Option<usize>,
    pub(super) last_space: f32,
    pub(super) last_w: f32,
    /// The game window is in front (not tabbed out): the others see "away" otherwise.
    pub(super) focused: bool,
}

impl Input {
    pub(super) fn new() -> Self {
        Self {
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
            last_space: -1.0,
            last_w: -1.0,
            focused: true,
        }
    }
}

/// Frame timing: the frame rate, the F3 graph and statistics, the frame limiter.
pub(super) struct FrameClock {
    pub(super) last: Instant,
    pub(super) fps: f32,
    pub(super) fps_accum: f32,
    pub(super) fps_frames: u32,
    /// Max FPS: when the next frame may start.
    pub(super) next_frame: Option<Instant>,
    /// Last frame's CPU time in ms: update, build, submit (without waiting), waiting for the GPU.
    pub(super) cpu_ms: [f32; 4],
    /// When the previous frame finished, and the time from then until this frame started.
    pub(super) frame_end: Instant,
    pub(super) between_ms: f32,
    /// Recent frame times in milliseconds (newest last), for the F3 graph.
    pub(super) frame_times: std::collections::VecDeque<f32>,
    pub(super) sys_stats: crate::stats::Monitor,
    /// Video memory (used, budget) in bytes, refreshed once a second.
    pub(super) vram: Option<(u64, u64)>,
    pub(super) vram_timer: f32,
}

impl FrameClock {
    pub(super) fn new() -> Self {
        Self {
            last: Instant::now(),
            fps: 0.0,
            fps_accum: 0.0,
            fps_frames: 0,
            next_frame: None,
            cpu_ms: [0.0; 4],
            frame_end: Instant::now(),
            between_ms: 0.0,
            frame_times: std::collections::VecDeque::with_capacity(hud::FRAME_GRAPH),
            sys_stats: crate::stats::start(),
            vram: None,
            vram_timer: 0.0,
        }
    }
}

/// At the open gun station: the brush, the mouse on its table and in its drawer, the camera,
/// scrubbing, and what was last sent.
pub(super) struct BenchUi {
    /// At the open gun station: holding its brush, and where it is; the camera's sway with
    /// the mouse (-1 .. 1); something picked up off the table (where the mouse was, to drag
    /// it); what the mouse points at there; scrubbing now, the dirt scrubbed off not yet
    /// taken off, and whether that is not sent yet; when the table was last sent.
    pub(super) brush: bool,
    pub(super) brush_at: Option<Vec3>,
    /// Where on the open gun station's table the mouse points (x, z), if it does.
    pub(super) spot: Option<(f32, f32)>,
    /// Where what is held on the mouse would lie on the open gun station's table.
    pub(super) held_spot: Option<(f32, f32)>,
    /// Where in the open drawer the mouse points (on its floor), if it does.
    pub(super) drawer_spot: Option<Vec3>,
    /// Where what is held on the mouse shows over the open gun station (world), for the
    /// others to see it there too.
    pub(super) hold_at: Option<Vec3>,
    pub(super) pan: f32,
    /// At the open gun station: looking into its drawer (the mouse went down to it), and how
    /// far the camera has gone down to it (0 over the table .. 1 over the drawer).
    pub(super) in_drawer: bool,
    pub(super) focus: f32,
    /// How long the mouse has stayed where it opens (or closes) the drawer.
    pub(super) dwell: f32,
    pub(super) drag: Option<Vec2>,
    pub(super) hover: Option<gui::BenchPick>,
    pub(super) scrubbing: bool,
    /// What the mouse is on at the gun station is where what is held goes (it lights green).
    pub(super) hover_ok: bool,
    pub(super) scrub: f32,
    pub(super) scrub_dirty: bool,
    pub(super) sent: f32,
}

impl BenchUi {
    pub(super) fn new() -> Self {
        Self {
            brush: false,
            brush_at: None,
            spot: None,
            held_spot: None,
            drawer_spot: None,
            hold_at: None,
            pan: 0.0,
            in_drawer: false,
            focus: 0.0,
            dwell: 0.0,
            drag: None,
            hover: None,
            scrubbing: false,
            hover_ok: false,
            scrub: 0.0,
            scrub_dirty: false,
            sent: 0.0,
        }
    }
}

/// The inventory screens: the creative tabs, list and search, the JEI panel, dragging and
/// clicking slots, and what the mouse is on at an open chest or table.
pub(super) struct InventoryUi {
    /// Creative list scroll in rows: the target set by the wheel, and the eased position.
    pub(super) creative_scroll: f32,
    pub(super) creative_scroll_anim: f32,
    /// Dragging the creative scroll bar.
    pub(super) scroll_drag: bool,
    /// Creative inventory search text; typing goes to it while it is focused.
    pub(super) creative_search: String,
    pub(super) search_focused: bool,
    /// The JEI panel beside the inventory screens.
    pub(super) jei: gui::Jei,
    /// The open tab of the creative inventory (index into `gui::TABS`); kept between openings.
    pub(super) creative_tab: usize,
    /// Slot drag in progress (Minecraft-style stack spreading).
    pub(super) drag: Option<gui::Drag>,
    /// The slot a stack was just picked up from with the button still held: letting go
    /// over another slot puts it there.
    pub(super) press_pick: Option<gui::SlotRef>,
    /// Time and slot of the last left click, for double-click collecting.
    pub(super) slot_click: (f32, Option<gui::SlotRef>),
    /// What the mouse points at in the open chest or on the open table, and the corners of
    /// its highlighted slot.
    pub(super) station_hover: Option<gui::SlotRef>,
    pub(super) station_frame: Option<[Vec3; 4]>,
    /// The mouse is over the open chest or table, or the inventory under it: a click there
    /// does not throw the held stack.
    pub(super) station_inside: bool,
}

impl InventoryUi {
    pub(super) fn new() -> Self {
        Self {
            creative_scroll: 0.0,
            creative_scroll_anim: 0.0,
            scroll_drag: false,
            creative_search: String::new(),
            search_focused: false,
            jei: Default::default(),
            creative_tab: 0,
            drag: None,
            press_pick: None,
            slot_click: (-1.0, None),
            station_hover: None,
            station_frame: None,
            station_inside: false,
        }
    }
}

/// The menus' state: the world list and the new world's settings, the multiplayer screen,
/// the options, the resource packs and the title screen's player.
pub(super) struct Menus {
    pub(super) worlds: Vec<WorldMeta>,
    pub(super) selected_world: Option<usize>,
    pub(super) world_scroll: f32,
    pub(super) last_click: (usize, f32),
    pub(super) create_name: String,
    pub(super) create_seed: String,
    pub(super) create_creative: bool,
    pub(super) create_cheats: bool,
    pub(super) finder: Option<crate::net::Finder>,
    pub(super) mp_address: String,
    pub(super) mp_selected: Option<std::net::SocketAddr>,
    /// Connecting to a LAN game (on a thread of its own, the window going on meanwhile): its
    /// address, and the connection when it is made.
    pub(super) joining: Option<(String, std::sync::mpsc::Receiver<std::io::Result<crate::net::Conn>>)>,
    pub(super) net_message: String,
    /// Open tab of the options screen.
    pub(super) options: screens::OptionsState,
    pub(super) pack_screen: screens::PackScreen,
    pub(super) menu_preview: screens::PreviewRotation,
    pub(super) skin_error: String,
}

impl Menus {
    pub(super) fn new() -> Self {
        Self {
            worlds: Vec::new(),
            selected_world: None,
            world_scroll: 0.0,
            last_click: (usize::MAX, -10.0),
            create_name: String::new(),
            create_seed: String::new(),
            create_creative: false,
            create_cheats: false,
            finder: None,
            mp_address: String::new(),
            mp_selected: None,
            joining: None,
            net_message: String::new(),
            options: Default::default(),
            pack_screen: Default::default(),
            menu_preview: Default::default(),
            skin_error: String::new(),
        }
    }
}
