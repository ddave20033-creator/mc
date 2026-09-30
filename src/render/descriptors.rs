//! Descriptor set layouts, the pool, and the sets the passes bind: a world set per frame slot
//! for the main pass and another for the scope's, the UI's, and the eyepiece's.

use super::frame::FrameUbo;
use super::targets::{ScopeTarget, ShadowTarget};
use crate::engine::{Buffer, Texture, FRAMES_IN_FLIGHT};
use ash::vk;
use std::mem::size_of;

pub(super) struct Descriptors {
    /// Block texture, shadow map, frame uniforms.
    pub world_dsl: vk::DescriptorSetLayout,
    /// Font, block texture.
    pub ui_dsl: vk::DescriptorSetLayout,
    /// The scope's view.
    pub lens_dsl: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    /// Per frame slot: the main pass's world set, and the scope pass's (its own uniforms).
    pub world_sets: Vec<vk::DescriptorSet>,
    pub scope_sets: Vec<vk::DescriptorSet>,
    pub ui_set: vk::DescriptorSet,
    pub lens_set: vk::DescriptorSet,
}

fn image_info(sampler: vk::Sampler, view: vk::ImageView, layout: vk::ImageLayout) -> [vk::DescriptorImageInfo; 1] {
    [vk::DescriptorImageInfo {
        sampler,
        image_view: view,
        image_layout: layout,
    }]
}

fn texture_info(tex: &Texture) -> [vk::DescriptorImageInfo; 1] {
    image_info(tex.sampler, tex.image.view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
}

fn image_write(set: vk::DescriptorSet, binding: u32, info: &[vk::DescriptorImageInfo]) -> vk::WriteDescriptorSet<'_> {
    vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(info)
}

/// A world set's three bindings: the block texture, the shadow map and a frame's uniforms.
fn world_set_writes<'a>(
    set: vk::DescriptorSet,
    block: &'a [vk::DescriptorImageInfo],
    shadow: &'a [vk::DescriptorImageInfo],
    ubo: &'a [vk::DescriptorBufferInfo],
) -> [vk::WriteDescriptorSet<'a>; 3] {
    [
        image_write(set, 0, block),
        image_write(set, 1, shadow),
        vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(2)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(ubo),
    ]
}

impl Descriptors {
    pub unsafe fn new(d: &ash::Device) -> Self {
        let binding = |i: u32, ty: vk::DescriptorType, stages: vk::ShaderStageFlags| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(i)
                .descriptor_type(ty)
                .descriptor_count(1)
                .stage_flags(stages)
        };
        let cis = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        let frag = vk::ShaderStageFlags::FRAGMENT;
        let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let world_bindings = [
            binding(0, cis, frag),
            binding(1, cis, frag),
            binding(2, vk::DescriptorType::UNIFORM_BUFFER, all),
        ];
        let ui_bindings = [binding(0, cis, frag), binding(1, cis, frag)];
        let lens_bindings = [binding(0, cis, frag)];
        let layout = |bindings: &[vk::DescriptorSetLayoutBinding]| {
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(bindings), None)
                .unwrap()
        };
        let world_dsl = layout(&world_bindings);
        let ui_dsl = layout(&ui_bindings);
        let lens_dsl = layout(&lens_bindings);
        // World sets and the scope's (block texture, shadow map, uniform buffer each), the
        // UI's (font, blocks) and the eyepiece's (the scope's view).
        let n = FRAMES_IN_FLIGHT as u32;
        let sizes = [
            vk::DescriptorPoolSize {
                ty: cis,
                descriptor_count: 4 * n + 2 + 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 2 * n,
            },
        ];
        let pool = d
            .create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(2 * n + 2)
                    .pool_sizes(&sizes),
                None,
            )
            .unwrap();
        let mut layouts = vec![world_dsl; FRAMES_IN_FLIGHT];
        layouts.push(ui_dsl);
        layouts.extend(std::iter::repeat_n(world_dsl, FRAMES_IN_FLIGHT));
        layouts.push(lens_dsl);
        let sets = d
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
            .unwrap();
        Self {
            world_dsl,
            ui_dsl,
            lens_dsl,
            pool,
            world_sets: sets[..FRAMES_IN_FLIGHT].to_vec(),
            ui_set: sets[FRAMES_IN_FLIGHT],
            scope_sets: sets[FRAMES_IN_FLIGHT + 1..2 * FRAMES_IN_FLIGHT + 1].to_vec(),
            lens_set: sets[2 * FRAMES_IN_FLIGHT + 1],
        }
    }

    /// Points every set at what it shows: the world sets at the textures, the shadow map and
    /// their frame slot's uniforms (`ubos` for the main pass, `scope_ubos` for the scope's).
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn write(
        &self,
        d: &ash::Device,
        block_tex: &Texture,
        font_tex: &Texture,
        shadow: &ShadowTarget,
        scope: &ScopeTarget,
        ubos: &[Buffer],
        scope_ubos: &[Buffer],
    ) {
        let block_info = texture_info(block_tex);
        let font_info = texture_info(font_tex);
        let shadow_info = image_info(shadow.sampler, shadow.image.view, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
        let lens_info = image_info(scope.sampler, scope.color.view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let ubo_info = |b: &Buffer| {
            [vk::DescriptorBufferInfo {
                buffer: b.handle,
                offset: 0,
                range: size_of::<FrameUbo>() as u64,
            }]
        };
        let ubo_infos: Vec<[vk::DescriptorBufferInfo; 1]> = scope_ubos.iter().chain(ubos).map(ubo_info).collect();
        let sets = self.scope_sets.iter().chain(&self.world_sets);
        let mut writes = Vec::new();
        for (&set, ubo) in sets.zip(&ubo_infos) {
            writes.extend(world_set_writes(set, &block_info, &shadow_info, ubo));
        }
        writes.push(image_write(self.lens_set, 0, &lens_info));
        writes.push(image_write(self.ui_set, 0, &font_info));
        writes.push(image_write(self.ui_set, 1, &block_info));
        d.update_descriptor_sets(&writes, &[]);
    }

    /// Points the sets showing the block texture at `tex` (a replacement).
    pub unsafe fn set_block_texture(&self, d: &ash::Device, tex: &Texture) {
        let info = texture_info(tex);
        for &set in self.world_sets.iter().chain(&self.scope_sets) {
            d.update_descriptor_sets(&[image_write(set, 0, &info)], &[]);
        }
        d.update_descriptor_sets(&[image_write(self.ui_set, 1, &info)], &[]);
    }

    pub unsafe fn destroy(&self, d: &ash::Device) {
        d.destroy_descriptor_set_layout(self.lens_dsl, None);
        d.destroy_descriptor_pool(self.pool, None);
        d.destroy_descriptor_set_layout(self.world_dsl, None);
        d.destroy_descriptor_set_layout(self.ui_dsl, None);
    }
}
