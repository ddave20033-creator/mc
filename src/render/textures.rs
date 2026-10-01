//! The block texture array while the game runs: single layers replaced in place (the guide
//! book's pages, icons) and the whole array swapped (a new texture pack).

use super::bindings::Groups;
use super::Renderer;
use crate::engine::resources::write_layers;
use crate::engine::{Gpu, SamplerKind, Texture};
use crate::textures::TILE;

impl Renderer {
    /// Replaces `count` layers of the block texture from `first` on, next frame. `levels`
    /// holds every mip level of them, as `Texture::new` takes them.
    pub fn queue_layers(&mut self, first: u32, count: u32, levels: Vec<Vec<u8>>) {
        if levels.len() == self.block_mips {
            self.layer_uploads.push_back((first, count, levels));
        }
    }

    /// Writes the queued layer replacements (they arrive before the frame's passes, after the
    /// frames already submitted).
    pub(super) fn flush_layer_uploads(&mut self, gpu: &Gpu) {
        while let Some((first, count, levels)) = self.layer_uploads.pop_front() {
            write_layers(gpu, &self.block_tex.texture, first, count, &levels);
        }
    }

    pub fn replace_block_textures(&mut self, gpu: &Gpu, levels: &[Vec<u8>]) {
        self.block_tex = block_texture(gpu, levels);
        self.groups = Groups::new(&gpu.device, &self.layouts, &self.sources());
        self.block_mips = levels.len();
        // Replaced layers queued before this are in the new levels already, or will be
        // queued again.
        self.layer_uploads.clear();
    }
}

/// The block texture array from its mip levels (as many layers as given).
pub(super) fn block_texture(gpu: &Gpu, levels: &[Vec<u8>]) -> Texture {
    Texture::new(
        gpu,
        TILE as u32,
        TILE as u32,
        (levels[0].len() / (TILE * TILE * 4)) as u32,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        levels,
        SamplerKind::Blocks,
    )
}
