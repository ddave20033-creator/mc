//! The start-up screen: the logo on black while the textures are made on another thread and
//! the title screen's world loads behind it; then it fades away into the title screen.

use super::*;
use std::sync::mpsc::{channel, Receiver};

/// What the texture thread makes.
pub(super) struct BootTextures {
    base: Vec<u8>,
    levels: Vec<Vec<u8>>,
    torch_particles: bool,
    credit: Option<(String, String)>,
}

pub(super) struct Boot {
    textures: Option<Receiver<BootTextures>>,
    /// When it started, and when it began to fade away (game time).
    start: f32,
    leaving: Option<f32>,
    /// Where the logo is in the textures: the start-up screen's own few, then all of them.
    logo_layer: u32,
}

/// At least this long on the screen (seconds), and at most this long waiting for the world
/// behind it.
const SHORTEST: f32 = 1.6;
const LONGEST: f32 = 8.0;
const FADE: f32 = 0.7;

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
                torch_particles: packs.texture("particle/flame").is_some(),
                credit: packs.0.last().map(|p| (p.title().to_string(), p.description.clone())),
            });
        });
        Self { textures: Some(rx), start: 0.0, leaving: None, logo_layer: 0 }
    }
}

impl Game {
    /// Every frame while starting up: takes the textures when they are made, and lets the
    /// start-up screen go once the title screen's world has loaded behind it (or it waited
    /// long enough). Whether it is still up (the menus take no input then).
    pub(super) fn boot_step(&mut self) -> bool {
        let Some(boot) = self.boot.as_mut() else {
            return false;
        };
        if let Some(made) = boot.textures.as_ref().and_then(|rx| rx.try_recv().ok()) {
            boot.textures = None;
            boot.logo_layer = textures::tex::LOGO;
            self.texture_base = made.base;
            self.torch_particles = made.torch_particles;
            self.pack_credit = made.credit;
            self.renderer.replace_block_textures(&self.gpu, &made.levels);
            self.book.textures_remade();
        }
        let Some(boot) = self.boot.as_mut() else {
            return false;
        };
        let age = self.time - boot.start;
        // (the menu pictures begin with the start-up screen)
        if std::env::var("GUN_SHOTS_MENU").is_ok() {
            let dt = self.ui.dt;
            for (at, name) in [(0.5, "boot_in"), (1.4, "boot")] {
                if age - dt < at && age >= at {
                    if let Some(dir) = self.gun_shots.as_ref().map(|g| g.dir.clone()) {
                        self.gpu.capture = Some(dir.join(format!("menu_{name}.png")));
                    }
                }
            }
        }
        let Some(boot) = self.boot.as_mut() else {
            return false;
        };
        let world_ready = self.renderer.chunk_count() >= 80 && self.renderer.pending() == 0;
        if boot.leaving.is_none() && boot.textures.is_none() && age >= SHORTEST && (world_ready || age >= LONGEST) {
            boot.leaving = Some(self.time);
            // The title screen comes in as the start-up screen goes.
            self.ui.age = 0.0;
        }
        if boot.leaving.is_some_and(|t| self.time - t >= FADE) {
            self.boot = None;
            return false;
        }
        true
    }

    /// The start-up screen over everything: black, the logo coming up in the middle, a thin
    /// line under it with a light running along it.
    pub(super) fn draw_boot(&mut self) {
        let Some(boot) = self.boot.as_ref() else {
            return;
        };
        let (w, h, s) = (self.ui.w, self.ui.h, self.ui.s);
        let age = self.time - boot.start;
        let out = boot.leaving.map_or(0.0, |t| ((self.time - t) / FADE).clamp(0.0, 1.0));
        let shown = 1.0 - crate::ui::ease_out(out);
        let logo_layer = boot.logo_layer;
        self.ui.solid(0.0, 0.0, w, h, rgba(6, 7, 10, (255.0 * shown) as u8));
        let come = crate::ui::ease_out((age - 0.15) / 0.9);
        let lw = (w * 0.5).min(900.0).round();
        let rise = ((1.0 - come) * 10.0 * s + out * -8.0 * s).round();
        let old = self.ui.style(come * shown, glam::Vec2::new(0.0, rise));
        let y = (h * 0.5 - lw / 16.0 - 8.0 * s).round();
        self.ui.logo(logo_layer, w * 0.5, y, lw);
        // The line: a light running along it while it waits.
        let (bw, bh) = ((lw * 0.3).round(), s.max(1.0));
        let (bx, by) = ((w * 0.5 - bw * 0.5).round(), (y + lw / 8.0 + 14.0 * s).round());
        self.ui.solid(bx, by, bw, bh, rgba(255, 255, 255, 26));
        let run = (age * 0.8).fract();
        let seg = bw * 0.25;
        let sx = bx + (bw + seg) * run - seg;
        let (x0, x1) = (sx.max(bx), (sx + seg).min(bx + bw));
        if x1 > x0 {
            self.ui.solid(x0, by, x1 - x0, bh, rgba(236, 128, 66, 220));
        }
        self.ui.restore(old);
    }
}
