//! The bind group layouts, and the bind groups the passes bind: the world's for the main pass
//! and another for the scope's (each with its own frame uniforms), the UI's, and the
//! eyepiece's and the blurred backdrop's.

use super::frame::FrameUbo;
use super::targets::{BlurTarget, ScopeTarget, ShadowTarget};
use crate::engine::{Buffer, Texture};
use std::mem::size_of;

pub(super) struct Layouts {
    /// Block texture and sampler, shadow map and its comparison sampler, frame uniforms.
    pub world: wgpu::BindGroupLayout,
    /// The shadow pass's: the world's without the shadow map (which it draws into).
    pub shadow: wgpu::BindGroupLayout,
    /// Font and its sampler, block texture and its sampler.
    pub ui: wgpu::BindGroupLayout,
    /// The scope's view (or the backdrop blurred across) and its sampler.
    pub lens: wgpu::BindGroupLayout,
}

fn texture(binding: u32, dimension: wgpu::TextureViewDimension, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type, view_dimension: dimension, multisampled: false },
        count: None,
    }
}

fn sampler(binding: u32, kind: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(kind),
        count: None,
    }
}

const COLOR: wgpu::TextureSampleType = wgpu::TextureSampleType::Float { filterable: true };
const FILTERING: wgpu::SamplerBindingType = wgpu::SamplerBindingType::Filtering;

impl Layouts {
    pub fn new(d: &wgpu::Device) -> Self {
        use wgpu::TextureViewDimension::{D2Array, D2};
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
        };
        let frame = wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size_of::<FrameUbo>() as u64),
            },
            count: None,
        };
        let world = layout(
            "world",
            &[
                texture(0, D2Array, COLOR),
                sampler(1, FILTERING),
                texture(2, D2, wgpu::TextureSampleType::Depth),
                sampler(3, wgpu::SamplerBindingType::Comparison),
                frame,
            ],
        );
        let shadow = layout("shadow", &[texture(0, D2Array, COLOR), sampler(1, FILTERING), frame]);
        let ui = layout("ui", &[texture(0, D2, COLOR), sampler(1, FILTERING), texture(2, D2Array, COLOR), sampler(3, FILTERING)]);
        let lens = layout("lens", &[texture(0, D2, COLOR), sampler(1, FILTERING)]);
        Self { world, shadow, ui, lens }
    }
}

/// The bind groups, pointing at the textures, targets and uniform buffers they show (made
/// again when the block texture is replaced).
pub(super) struct Groups {
    /// The main pass's world group, and the scope pass's (its own uniforms).
    pub world: wgpu::BindGroup,
    pub scope: wgpu::BindGroup,
    /// The shadow pass's (the main pass's uniforms).
    pub shadow: wgpu::BindGroup,
    pub ui: wgpu::BindGroup,
    /// The scope's view, on the eyepiece.
    pub lens: wgpu::BindGroup,
    /// The backdrop blurred across (the `lens` layout too).
    pub blur: wgpu::BindGroup,
}

/// What the groups show.
pub(super) struct Sources<'a> {
    pub block_tex: &'a Texture,
    pub font_tex: &'a Texture,
    pub shadow: &'a ShadowTarget,
    pub scope: &'a ScopeTarget,
    pub blur: &'a BlurTarget,
    pub ubo: &'a Buffer,
    pub scope_ubo: &'a Buffer,
}

impl Groups {
    pub fn new(d: &wgpu::Device, layouts: &Layouts, s: &Sources) -> Self {
        let group = |label, layout, entries: &[wgpu::BindGroupEntry]| {
            d.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries })
        };
        fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource }
        }
        use wgpu::BindingResource::{Sampler, TextureView};
        let world = |label, ubo: &Buffer| {
            group(
                label,
                &layouts.world,
                &[
                    entry(0, TextureView(&s.block_tex.view)),
                    entry(1, Sampler(&s.block_tex.sampler)),
                    entry(2, TextureView(&s.shadow.view)),
                    entry(3, Sampler(&s.shadow.sampler)),
                    entry(4, ubo.handle.as_entire_binding()),
                ],
            )
        };
        Self {
            world: world("world", s.ubo),
            scope: world("scope", s.scope_ubo),
            shadow: group(
                "shadow",
                &layouts.shadow,
                &[
                    entry(0, TextureView(&s.block_tex.view)),
                    entry(1, Sampler(&s.block_tex.sampler)),
                    entry(4, s.ubo.handle.as_entire_binding()),
                ],
            ),
            ui: group(
                "ui",
                &layouts.ui,
                &[
                    entry(0, TextureView(&s.font_tex.view)),
                    entry(1, Sampler(&s.font_tex.sampler)),
                    entry(2, TextureView(&s.block_tex.view)),
                    entry(3, Sampler(&s.block_tex.sampler)),
                ],
            ),
            lens: group("lens", &layouts.lens, &[entry(0, TextureView(&s.scope.color)), entry(1, Sampler(&s.scope.sampler))]),
            blur: group("blur", &layouts.lens, &[entry(0, TextureView(&s.blur.color)), entry(1, Sampler(&s.scope.sampler))]),
        }
    }
}
