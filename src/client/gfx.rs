//! The window, the GPU and the renderer, and the textures: the resource packs' (made at
//! start-up, `boot`, and again when the packs change).

use crate::engine::Gpu;
use crate::render::Renderer;
use crate::textures;
use std::sync::Arc;
use winit::window::Window;

/// The window and what draws into it.
pub(super) struct Gfx {
    // The renderer's GPU resources go before the device, and the window after both (fields
    // drop in this order).
    pub(super) renderer: Renderer,
    pub(super) gpu: Gpu,
    pub(super) window: Arc<Window>,
    /// All texture layers at full size (see `textures::generate_base`).
    pub(super) texture_base: Arc<Vec<u8>>,
    /// The resource pack draws torch fire as flame/smoke particles (like Minecraft).
    pub(super) torch_particles: bool,
    /// (title, description) of the resource pack in use, for the credits.
    pub(super) pack_credit: Option<(String, String)>,
    /// Last frame's geometry lists, to be filled again (see `frame::Scene`).
    pub(super) scene: super::scene::Scene,
}

impl Gfx {
    /// The window's GPU and renderer, with only the logo's textures (the start-up screen's)
    /// until the real ones are made (`boot`).
    pub(super) fn new(window: Arc<Window>, msaa: u32, font_atlas: &[u8]) -> Self {
        let gpu = Gpu::new(window.clone(), false, msaa);
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
            scene: Default::default(),
        }
    }

    /// Remakes the textures from the enabled resource packs (after the pack screen).
    pub(super) fn reload_packs(&mut self, enabled: &[String]) {
        let packs = crate::textures::resource_pack::Packs::load(enabled);
        self.texture_base = Arc::new(textures::generate_base(&packs));
        self.torch_particles = torch_particles(&packs);
        self.pack_credit = pack_credit(&packs);
        let levels = textures::levels(&self.texture_base);
        self.renderer.replace_block_textures(&self.gpu, &levels);
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
