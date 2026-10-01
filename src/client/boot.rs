//! Starting up: the textures are made on another thread and the title screen's world loads
//! while the game's window is still hidden (the splash, `crate::splash`, shows the progress);
//! then the window is shown with the title screen.

use crate::client::Game;
use crate::world::textures;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

/// What the texture thread makes.
pub(super) struct BootTextures {
    base: Vec<u8>,
    levels: Vec<Vec<u8>>,
    torch_particles: bool,
    credit: Option<(String, String)>,
}

pub(super) struct Boot {
    textures: Option<Receiver<BootTextures>>,
    /// When it started (game time).
    start: f32,
}

/// At least this long (seconds), and at most this long waiting for the title screen's world.
const SHORTEST: f32 = 1.6;
const LONGEST: f32 = 8.0;

impl Boot {
    /// Starts making the textures (from the enabled resource packs, with the uploaded skins).
    pub(super) fn start(packs: Vec<String>, skins: std::collections::HashMap<u8, crate::pack::Image>) -> Self {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let packs = crate::pack::Packs::load(&packs);
            let base = textures::generate_base(&packs);
            let levels = textures::with_skins(&base, &skins);
            let _ = tx.send(BootTextures {
                base,
                levels,
                torch_particles: super::gfx::torch_particles(&packs),
                credit: super::gfx::pack_credit(&packs),
            });
        });
        Self { textures: Some(rx), start: 0.0 }
    }
}

impl Game {
    /// Every frame while starting up: takes the textures when they are made, and ends the
    /// start-up once the title screen's world has loaded (or it waited long enough). Whether
    /// it is still starting up (the menus take no input then).
    pub(super) fn boot_step(&mut self) -> bool {
        let Some(boot) = self.boot.as_mut() else {
            return false;
        };
        if let Some(made) = boot.textures.as_ref().and_then(|rx| rx.try_recv().ok()) {
            boot.textures = None;
            self.gfx.texture_base = Arc::new(made.base);
            self.gfx.torch_particles = made.torch_particles;
            self.gfx.pack_credit = made.credit;
            self.gfx.renderer.replace_block_textures(&self.gfx.gpu, &made.levels);
            self.book.textures_remade();
        }
        let Some(boot) = self.boot.as_mut() else {
            return false;
        };
        let age = self.clock.time - boot.start;
        let world_ready = self.gfx.renderer.chunk_count() >= 80 && self.gfx.renderer.pending() == 0;
        if boot.textures.is_none() && age >= SHORTEST && (world_ready || age >= LONGEST) {
            self.boot = None;
            return false;
        }
        true
    }

    /// How far starting up has got (0..1): the textures, then the title screen's world. None
    /// once it is done.
    pub fn boot_progress(&self) -> Option<f32> {
        let boot = self.boot.as_ref()?;
        let made = if boot.textures.is_some() { textures::progress() } else { 1.0 };
        let world = (self.gfx.renderer.chunk_count() as f32 / 80.0).min(1.0);
        Some((0.1 + 0.65 * made + 0.25 * world).min(0.99))
    }

    /// Shows the game's window (hidden while starting up), the title screen coming in.
    pub fn show_window(&mut self) {
        self.ui.age = 0.0;
        self.gfx.window.set_visible(true);
        self.gfx.window.focus_window();
    }
}
