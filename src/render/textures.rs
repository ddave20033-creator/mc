//! The block texture array while the game runs: single layers replaced in place (the guide
//! book's pages, icons) and the whole array swapped (a new texture pack).

use super::Renderer;
use crate::engine::{Buffer, Gpu, SamplerKind, Texture};
use crate::textures::TILE;
use ash::vk;

impl Renderer {
    /// Replaces `count` layers of the block texture from `first` on, next frame. `levels`
    /// holds every mip level of them, as `Texture::new` takes them.
    pub fn queue_layers(&mut self, first: u32, count: u32, levels: Vec<Vec<u8>>) {
        if levels.len() == self.block_mips {
            self.layer_uploads.push_back((first, count, levels));
        }
    }

    /// Records the queued layer replacements (before the frame's passes).
    pub(super) unsafe fn flush_layer_uploads(&mut self, gpu: &mut Gpu, cmd: vk::CommandBuffer) {
        while let Some((first, count, levels)) = self.layer_uploads.pop_front() {
            let total: usize = levels.iter().map(|l| l.len()).sum();
            let staging = Buffer::new(
                gpu,
                total as u64,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            let mut regions = Vec::new();
            let mut offset = 0usize;
            for (i, level) in levels.iter().enumerate() {
                staging.write(offset, level.as_slice());
                let size = (TILE as u32 >> i).max(1);
                regions.push(vk::BufferImageCopy {
                    buffer_offset: offset as u64,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: i as u32,
                        base_array_layer: first,
                        layer_count: count,
                    },
                    image_offset: vk::Offset3D::default(),
                    image_extent: vk::Extent3D {
                        width: size,
                        height: size,
                        depth: 1,
                    },
                });
                offset += level.len();
            }
            let range = vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: levels.len() as u32,
                base_array_layer: first,
                layer_count: count,
            };
            let image = self.block_tex.image.handle;
            let barrier = |old, new, src, dst| {
                vk::ImageMemoryBarrier::default()
                    .old_layout(old)
                    .new_layout(new)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(range)
                    .src_access_mask(src)
                    .dst_access_mask(dst)
            };
            let d = gpu.device.clone();
            // Earlier frames may still be sampling these layers.
            d.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::VERTEX_SHADER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier(
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::AccessFlags::SHADER_READ,
                    vk::AccessFlags::TRANSFER_WRITE,
                )],
            );
            d.cmd_copy_buffer_to_image(
                cmd,
                staging.handle,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
            d.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::VERTEX_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier(
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::AccessFlags::TRANSFER_WRITE,
                    vk::AccessFlags::SHADER_READ,
                )],
            );
            gpu.defer_destroy(staging);
        }
    }

    pub fn replace_block_textures(&mut self, gpu: &Gpu, levels: &[Vec<u8>]) {
        unsafe {
            gpu.device
                .device_wait_idle()
                .expect("wait before skin texture swap")
        };
        let replacement = block_texture(gpu, levels);
        unsafe { self.desc.set_block_texture(&gpu.device, &replacement) };
        self.block_mips = levels.len();
        // Replaced layers queued before this are in the new levels already, or will be
        // queued again.
        self.layer_uploads.clear();
        let old = std::mem::replace(&mut self.block_tex, replacement);
        old.destroy(&gpu.device);
    }
}

/// The block texture array from its mip levels (as many layers as given).
pub(super) fn block_texture(gpu: &Gpu, levels: &[Vec<u8>]) -> Texture {
    Texture::new(
        gpu,
        TILE as u32,
        TILE as u32,
        (levels[0].len() / (TILE * TILE * 4)) as u32,
        vk::Format::R8G8B8A8_SRGB,
        levels,
        SamplerKind::Blocks,
    )
}
