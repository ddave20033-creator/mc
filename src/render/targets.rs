//! The offscreen render targets: the shadow map (the sun's square and the weapon lights'
//! strip under it), the scope's view and the menus' backdrop blurred across, and the passes
//! drawing into them.

use super::frame::{SCOPE_SIZE, SHADOW_HEIGHT, SHADOW_SIZE};
use crate::engine::gpu::DEPTH_FORMAT;
use crate::engine::Gpu;

pub(super) const SCOPE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub(super) const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A `width` x `height` image to draw into (and sample afterwards).
fn image(gpu: &Gpu, label: &str, width: u32, height: u32, format: wgpu::TextureFormat, sampled: bool) -> wgpu::TextureView {
    let usage = if sampled {
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
    } else {
        wgpu::TextureUsages::RENDER_ATTACHMENT
    };
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// A pass's depth target, cleared to the far end (kept afterwards if `store`).
fn depth_attachment(view: &wgpu::TextureView, store: bool) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: if store { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard },
        }),
        stencil_ops: None,
    }
}

/// A pass's color target, cleared to black and kept.
fn color_attachment(view: &wgpu::TextureView) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
    }
}

/// The shadow map: one depth image the world shaders sample with comparison.
pub(super) struct ShadowTarget {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl ShadowTarget {
    pub fn new(gpu: &Gpu) -> Self {
        // (outside the image everything is lit; without the GPU's border colour the edge is
        // repeated, which the shaders fade out there anyway)
        let address = if gpu.clamp_to_border {
            wgpu::AddressMode::ClampToBorder
        } else {
            wgpu::AddressMode::ClampToEdge
        };
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            address_mode_u: address,
            address_mode_v: address,
            address_mode_w: address,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: 0.0,
            compare: Some(wgpu::CompareFunction::LessEqual),
            border_color: gpu.clamp_to_border.then_some(wgpu::SamplerBorderColor::OpaqueWhite),
            ..Default::default()
        });
        Self { view: image(gpu, "shadow map", SHADOW_SIZE, SHADOW_HEIGHT, SHADOW_FORMAT, true), sampler }
    }

    /// Begins the shadow pass over the whole image, cleared to the far end.
    pub fn begin(&self, encoder: &mut wgpu::CommandEncoder) -> wgpu::RenderPass<'static> {
        encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(depth_attachment(&self.view, true)),
                ..Default::default()
            })
            .forget_lifetime()
    }
}

/// The scope's view (picture in picture): a colour image the eyepiece samples afterwards,
/// and a depth image.
pub(super) struct ScopeTarget {
    pub color: wgpu::TextureView,
    depth: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl ScopeTarget {
    pub fn new(gpu: &Gpu) -> Self {
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scope"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        Self {
            color: image(gpu, "scope view", SCOPE_SIZE, SCOPE_SIZE, SCOPE_FORMAT, true),
            depth: image(gpu, "scope depth", SCOPE_SIZE, SCOPE_SIZE, DEPTH_FORMAT, false),
            sampler,
        }
    }

    /// Begins the scope pass, cleared to black and the far end.
    pub fn begin(&self, encoder: &mut wgpu::CommandEncoder) -> wgpu::RenderPass<'static> {
        encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scope"),
                color_attachments: &[Some(color_attachment(&self.color))],
                depth_stencil_attachment: Some(depth_attachment(&self.depth, false)),
                ..Default::default()
            })
            .forget_lifetime()
    }
}

/// The menus' backdrop blurred across (the first half of its blur, at the scope image's size):
/// a colour image the backdrop blurs down, as it is drawn over the screen.
pub(super) struct BlurTarget {
    pub color: wgpu::TextureView,
}

impl BlurTarget {
    pub fn new(gpu: &Gpu) -> Self {
        Self { color: image(gpu, "backdrop blur", SCOPE_SIZE, SCOPE_SIZE, SCOPE_FORMAT, true) }
    }

    /// Begins the pass (every pixel is drawn over).
    pub fn begin(&self, encoder: &mut wgpu::CommandEncoder) -> wgpu::RenderPass<'static> {
        encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur"),
                color_attachments: &[Some(color_attachment(&self.color))],
                depth_stencil_attachment: None,
                ..Default::default()
            })
            .forget_lifetime()
    }
}
