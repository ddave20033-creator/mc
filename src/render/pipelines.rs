//! The shaders, the vertex layouts and the pipelines drawn with: the main pass's set (its
//! scene part made once more for the scope's pass), the shadow map's, and the ones showing the scope's image.

use super::Renderer;
use crate::engine::pipeline::{create_pipeline, PipelineDesc};
use crate::engine::Gpu;
use crate::ui::UiVertex;
use super::chunks::ChunkVertex;
use crate::world::mesh::Vertex;
use ash::vk;
use glam::Mat4;
use std::mem::size_of;

const WORLD_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/world.vert.spv"));
const WORLD_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/world.frag.spv"));
/// world.frag without its alpha tests (`NO_DISCARD`): for the plain whole-block faces, whose
/// depth test can then run before the fragment shader.
const WORLD_PLAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/world_plain.frag.spv"));
const SHADOW_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.vert.spv"));
/// world.vert and shadow.vert reading chunk meshes' packed vertices (`CHUNK`).
const WORLD_CHUNK_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/world_chunk.vert.spv"));
const SHADOW_CHUNK_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow_chunk.vert.spv"));
const SHADOW_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.frag.spv"));
const SHADOW_PLAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow_plain.frag.spv"));
const SKY_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv"));
const SKY_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv"));
const UI_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui.vert.spv"));
const UI_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui.frag.spv"));
/// The scope eyepiece: the world's vertices, the scope's view as its colour.
pub(super) const LENS_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lens.frag.spv"));
/// The menus' blurred backdrop, drawn like the eyepiece (blurring down what `BLUR_ACROSS_FRAG`
/// has blurred across).
pub(super) const BLUR_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/blur.frag.spv"));
const BLUR_ACROSS_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/blur_across.frag.spv"));

const WORLD_ATTRS: [(vk::Format, u32); 5] = [
    (vk::Format::R32G32B32_SFLOAT, 0),
    (vk::Format::R32G32_SFLOAT, 12),
    (vk::Format::R32_SFLOAT, 20),
    (vk::Format::R8G8B8A8_UNORM, 24),
    (vk::Format::R8G8B8A8_UNORM, 28),
];

/// A chunk mesh's `ChunkVertex`: position and layer, uv, light, tint.
const CHUNK_ATTRS: [(vk::Format, u32); 4] = [
    (vk::Format::R16G16B16A16_UINT, 0),
    (vk::Format::R16G16_UINT, 8),
    (vk::Format::R8G8B8A8_UNORM, 12),
    (vk::Format::R8G8B8A8_UNORM, 16),
];

const UI_ATTRS: [(vk::Format, u32); 5] = [
    (vk::Format::R32G32_SFLOAT, 0),
    (vk::Format::R32G32_SFLOAT, 8),
    (vk::Format::R32G32B32A32_SFLOAT, 16),
    (vk::Format::R32G32B32A32_SFLOAT, 32),
    (vk::Format::R32_SFLOAT, 48),
];

/// The world pipelines' push constants (`params` in frame.glsl): the view, and what is drawn
/// (x: 0 opaque, 1 translucent, 2 view model, 3 the sun's shadow map, 4 view model glass,
/// 5 a weapon light's shadow map; y: the local player's opacity when it fades).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct DrawPush {
    pub view_proj: [f32; 16],
    pub params: [f32; 4],
}

impl DrawPush {
    pub fn new(vp: Mat4, pass: f32) -> Self {
        Self {
            view_proj: vp.to_cols_array(),
            params: [pass, 0.0, 0.0, 0.0],
        }
    }
}

/// The pipelines of a pass drawing the world: sky, the chunks (all but the plain whole-block
/// faces, and water), the world's other things (entities, particles...) and what is
/// translucent of them, and the overlay. The scope's pass draws with just these.
pub(super) struct ScenePipes {
    pub sky: vk::Pipeline,
    pub world_chunk: vk::Pipeline,
    pub water_chunk: vk::Pipeline,
    pub world: vk::Pipeline,
    pub water: vk::Pipeline,
    pub overlay: vk::Pipeline,
}

/// The pipelines of the main pass: the scene's, and the plain whole-block faces, player fade,
/// lines and UI.
pub(super) struct MainPipes {
    pub scene: ScenePipes,
    pub world_plain: vk::Pipeline,
    pub player_fade: vk::Pipeline,
    pub line: vk::Pipeline,
    pub ui: vk::Pipeline,
}

/// The world's other things (`Vertex`) drawn into `render_pass`.
fn world_desc(render_pass: vk::RenderPass, samples: vk::SampleCountFlags, layout: vk::PipelineLayout) -> PipelineDesc<'static> {
    PipelineDesc {
        vert: WORLD_VERT,
        frag: WORLD_FRAG,
        stride: size_of::<Vertex>() as u32,
        attributes: &WORLD_ATTRS,
        layout,
        render_pass,
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        cull: true,
        depth_test: true,
        depth_write: true,
        blend: false,
        multiply: false,
        color: true,
        depth_bias: None,
        samples,
        alpha_to_coverage: false,
        instance: false,
    }
}

/// Chunk meshes (`ChunkVertex`) drawn into `render_pass`.
fn chunk_desc(render_pass: vk::RenderPass, samples: vk::SampleCountFlags, layout: vk::PipelineLayout) -> PipelineDesc<'static> {
    PipelineDesc {
        vert: WORLD_CHUNK_VERT,
        stride: size_of::<ChunkVertex>() as u32,
        attributes: &CHUNK_ATTRS,
        instance: true,
        ..world_desc(render_pass, samples, layout)
    }
}

impl ScenePipes {
    pub fn new(
        d: &ash::Device,
        render_pass: vk::RenderPass,
        samples: vk::SampleCountFlags,
        world_layout: vk::PipelineLayout,
    ) -> Self {
        let world_desc = world_desc(render_pass, samples, world_layout);
        let chunk_desc = chunk_desc(render_pass, samples, world_layout);
        // Only the opaque world pass smooths cut-out edges (with anti-aliasing on).
        let world = create_pipeline(
            d,
            &PipelineDesc {
                alpha_to_coverage: samples != vk::SampleCountFlags::TYPE_1,
                ..world_desc
            },
        );
        let world_chunk = create_pipeline(
            d,
            &PipelineDesc {
                alpha_to_coverage: samples != vk::SampleCountFlags::TYPE_1,
                ..chunk_desc
            },
        );
        let water_chunk = create_pipeline(
            d,
            &PipelineDesc {
                cull: false,
                depth_write: false,
                blend: true,
                ..chunk_desc
            },
        );
        let water = create_pipeline(
            d,
            &PipelineDesc {
                cull: false,
                depth_write: false,
                blend: true,
                ..world_desc
            },
        );
        let overlay = create_pipeline(
            d,
            &PipelineDesc {
                depth_write: false,
                blend: true,
                multiply: true,
                ..world_desc
            },
        );
        let sky = create_pipeline(
            d,
            &PipelineDesc {
                vert: SKY_VERT,
                frag: SKY_FRAG,
                attributes: &[],
                cull: false,
                depth_test: false,
                depth_write: false,
                ..world_desc
            },
        );
        Self { sky, world_chunk, water_chunk, world, water, overlay }
    }

    pub unsafe fn destroy(&self, d: &ash::Device) {
        for p in [self.sky, self.world_chunk, self.water_chunk, self.world, self.water, self.overlay] {
            d.destroy_pipeline(p, None);
        }
    }
}

impl MainPipes {
    pub fn new(
        d: &ash::Device,
        render_pass: vk::RenderPass,
        samples: vk::SampleCountFlags,
        world_layout: vk::PipelineLayout,
        ui_layout: vk::PipelineLayout,
    ) -> Self {
        let world_desc = world_desc(render_pass, samples, world_layout);
        // (no alpha to coverage either: that too would make the depth test wait for the shader;
        // these faces are fully covered anyway)
        let world_plain = create_pipeline(
            d,
            &PipelineDesc {
                frag: WORLD_PLAIN_FRAG,
                ..chunk_desc(render_pass, samples, world_layout)
            },
        );
        let player_fade = create_pipeline(
            d,
            &PipelineDesc {
                depth_write: false,
                blend: true,
                ..world_desc
            },
        );
        let line = create_pipeline(
            d,
            &PipelineDesc {
                topology: vk::PrimitiveTopology::LINE_LIST,
                cull: false,
                depth_write: false,
                blend: true,
                ..world_desc
            },
        );
        let ui = create_pipeline(
            d,
            &PipelineDesc {
                vert: UI_VERT,
                frag: UI_FRAG,
                stride: size_of::<UiVertex>() as u32,
                attributes: &UI_ATTRS,
                layout: ui_layout,
                cull: false,
                depth_test: false,
                depth_write: false,
                blend: true,
                ..world_desc
            },
        );
        Self {
            scene: ScenePipes::new(d, render_pass, samples, world_layout),
            world_plain,
            player_fade,
            line,
            ui,
        }
    }

    pub unsafe fn destroy(&self, d: &ash::Device) {
        self.scene.destroy(d);
        for p in [self.world_plain, self.player_fade, self.line, self.ui] {
            d.destroy_pipeline(p, None);
        }
    }
}

/// Something showing the scope's image, `frag` drawing it (`LENS_FRAG`, `BLUR_FRAG`).
pub(super) fn create_scope_view_pipe(
    d: &ash::Device,
    render_pass: vk::RenderPass,
    samples: vk::SampleCountFlags,
    layout: vk::PipelineLayout,
    frag: &[u8],
) -> vk::Pipeline {
    create_pipeline(
        d,
        &PipelineDesc {
            vert: WORLD_VERT,
            frag,
            stride: size_of::<Vertex>() as u32,
            attributes: &WORLD_ATTRS,
            layout,
            render_pass,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            cull: false,
            depth_test: true,
            depth_write: true,
            blend: false,
            multiply: false,
            color: true,
            depth_bias: None,
            samples,
            alpha_to_coverage: false,
            instance: false,
        },
    )
}

/// The backdrop's first blur pass: the scope's image blurred across, into the blur target
/// (`render_pass`), a full-screen triangle.
pub(super) fn create_blur_across_pipe(
    d: &ash::Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
) -> vk::Pipeline {
    create_pipeline(
        d,
        &PipelineDesc {
            vert: SKY_VERT,
            frag: BLUR_ACROSS_FRAG,
            stride: 0,
            attributes: &[],
            layout,
            render_pass,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            cull: false,
            depth_test: false,
            depth_write: false,
            blend: false,
            multiply: false,
            color: true,
            depth_bias: None,
            samples: vk::SampleCountFlags::TYPE_1,
            alpha_to_coverage: false,
            instance: false,
        },
    )
}

/// The shadow map's: depth only, biased against shadow acne. `plain`: without alpha tests,
/// for the plain whole-block faces; `chunk`: for chunk meshes (their packed vertices).
pub(super) fn create_shadow_pipe(
    d: &ash::Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    plain: bool,
    chunk: bool,
) -> vk::Pipeline {
    create_pipeline(
        d,
        &PipelineDesc {
            vert: if chunk { SHADOW_CHUNK_VERT } else { SHADOW_VERT },
            frag: if plain { SHADOW_PLAIN_FRAG } else { SHADOW_FRAG },
            stride: if chunk { size_of::<ChunkVertex>() } else { size_of::<Vertex>() } as u32,
            attributes: if chunk { &CHUNK_ATTRS } else { &WORLD_ATTRS },
            instance: chunk,
            layout,
            render_pass,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            cull: false,
            depth_test: true,
            depth_write: true,
            blend: false,
            multiply: false,
            color: false,
            depth_bias: Some((1.5, 2.0)),
            samples: vk::SampleCountFlags::TYPE_1,
            alpha_to_coverage: false,
        },
    )
}

impl Renderer {
    /// The main pass changed (anti-aliasing): the pipelines drawing into it are made again.
    pub(super) unsafe fn rebuild_screen_pipes(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        self.pipes.destroy(d);
        self.pipes = MainPipes::new(d, gpu.render_pass, gpu.samples, self.world_layout, self.ui_layout);
        d.destroy_pipeline(self.lens_pipe, None);
        self.lens_pipe = create_scope_view_pipe(d, gpu.render_pass, gpu.samples, self.lens_layout, LENS_FRAG);
        d.destroy_pipeline(self.blur_pipe, None);
        self.blur_pipe = create_scope_view_pipe(d, gpu.render_pass, gpu.samples, self.lens_layout, BLUR_FRAG);
        self.pass_version = gpu.pass_version;
    }
}
