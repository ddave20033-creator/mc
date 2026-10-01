//! The window, the GPU and the renderer, and the textures: the resource packs' (made at
//! start-up, `boot`, and again when the packs change) and the players' skins.

use crate::engine::Gpu;
use crate::app::lang::t;
use crate::render::Renderer;
use crate::textures;
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

/// The window and what draws into it.
pub(super) struct Gfx {
    // `renderer` holds GPU resources and is destroyed explicitly in Drop before `gpu`; the
    // window goes after them (fields drop in this order).
    pub(super) renderer: Renderer,
    pub(super) gpu: Gpu,
    pub(super) window: Arc<Window>,
    /// All texture layers but the uploaded skins (see `textures::generate_base`).
    pub(super) texture_base: Arc<Vec<u8>>,
    /// The resource pack draws torch fire as flame/smoke particles (like Minecraft).
    pub(super) torch_particles: bool,
    /// (title, description) of the resource pack in use, for the credits.
    pub(super) pack_credit: Option<(String, String)>,
    pub(super) skins: Skins,
    /// Last frame's geometry lists, to be filled again (see `frame::Scene`).
    pub(super) scene: super::scene::Scene,
}

/// The uploaded skins by player slot (this player's own in slot 0 out of a world, and in its
/// own player id's slot in one), and the PNGs they came from (sent to the other players).
pub(super) struct Skins {
    pub(super) custom: HashMap<u8, crate::textures::resource_pack::Image>,
    pngs: HashMap<u8, Vec<u8>>,
    /// This player's own skin (`skins/custom.png`), if it has one.
    pub(super) local_png: Option<Vec<u8>>,
}

impl Skins {
    /// This player's own skin from `skins/custom.png`, in slot 0.
    pub(super) fn load() -> Self {
        let mut custom = HashMap::new();
        let mut pngs = HashMap::new();
        if let Ok(bytes) = std::fs::read("skins/custom.png") {
            if let Ok(image) = textures::decode_skin_png(&bytes) {
                custom.insert(0, image);
                pngs.insert(0, bytes);
            }
        }
        let local_png = pngs.get(&0).cloned();
        Self { custom, pngs, local_png }
    }
}

impl Gfx {
    /// The window's GPU and renderer, with only the logo's textures (the start-up screen's)
    /// until the real ones are made (`boot`).
    pub(super) fn new(window: Arc<Window>, msaa: u32, font_atlas: &[u8], skins: Skins) -> Self {
        let gpu = Gpu::new(&window, false, msaa);
        println!("Your Worlds running on: {}", gpu.device_name);
        let renderer = Renderer::new(&gpu, &textures::logo_levels(), font_atlas);
        Self {
            renderer,
            gpu,
            window,
            texture_base: Arc::new(Vec::new()),
            torch_particles: false,
            // The credits name the built-in pack (always in use).
            pack_credit: None,
            skins,
            scene: Default::default(),
        }
    }

    /// Remakes the textures from the enabled resource packs (after the pack screen).
    pub(super) fn reload_packs(&mut self, enabled: &[String]) {
        let packs = crate::textures::resource_pack::Packs::load(enabled);
        self.texture_base = Arc::new(textures::generate_base(&packs));
        self.torch_particles = torch_particles(&packs);
        self.pack_credit = pack_credit(&packs);
        self.refresh_skin_textures();
    }

    /// Uploads all the texture layers again (the skins put into the base).
    fn refresh_skin_textures(&mut self) {
        let levels = textures::with_skins(&self.texture_base, &self.skins.custom);
        self.renderer.replace_block_textures(&self.gpu, &levels);
    }

    /// Only one player slot's skin layers are made again and uploaded (not all the textures).
    fn refresh_skin_slot(&mut self, slot: u8) {
        if self.texture_base.is_empty() {
            // (the textures are still being made: they will have it)
            return;
        }
        let (first, count, levels) = textures::skin_slot_levels(&self.texture_base, slot, self.skins.custom.get(&slot));
        self.renderer.queue_layers(first, count, levels);
    }

    /// A player slot's skin from a PNG (an uploaded one, or another player's).
    pub(super) fn set_skin_png(&mut self, slot: u8, png: Vec<u8>) -> Result<(), &'static str> {
        if slot >= textures::tex::CUSTOM_SKIN_SLOTS {
            return Err(t("skin.too_many"));
        }
        let image = textures::decode_skin_png(&png)?;
        self.skins.custom.insert(slot, image);
        self.skins.pngs.insert(slot, png);
        self.refresh_skin_slot(slot);
        Ok(())
    }

    /// Another player's skin goes (they left): its slot shows the default skin again, for
    /// whoever gets it next.
    pub(super) fn remove_skin(&mut self, slot: u8) {
        self.skins.custom.remove(&slot);
        self.skins.pngs.remove(&slot);
        if slot < textures::tex::CUSTOM_SKIN_SLOTS {
            self.refresh_skin_slot(slot);
        }
    }

    /// Out of a world: only this player's own skin is kept, in slot 0 again.
    pub(super) fn forget_other_skins(&mut self) {
        self.skins.custom.retain(|&id, _| id == 0);
        self.skins.pngs.retain(|&id, _| id == 0);
        match self.skins.local_png.clone() {
            Some(png) => {
                let _ = self.set_skin_png(0, png);
            }
            // (slot 0 had the host's skin in a LAN game)
            None => self.remove_skin(0),
        }
    }
}

/// The resource packs draw torch fire as flame/smoke particles (like Minecraft).
pub(super) fn torch_particles(packs: &crate::textures::resource_pack::Packs) -> bool {
    packs.texture("particle/flame").is_some()
}

/// (title, description) of the resource pack on top, for the credits (none: the built-in one).
pub(super) fn pack_credit(packs: &crate::textures::resource_pack::Packs) -> Option<(String, String)> {
    packs.0.last().map(|p| (p.title().to_string(), p.description.clone()))
}

impl Drop for Gfx {
    fn drop(&mut self) {
        self.renderer.destroy(&mut self.gpu);
    }
}
