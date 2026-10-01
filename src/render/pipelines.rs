//! The shaders, the vertex layouts and the pipelines drawn with: the main pass's set (its
//! scene part made once more for the scope's pass), the shadow map's, and the ones showing the scope's image.

use super::chunks::ChunkVertex;
use super::shaders::Shader;
use super::targets::{SCOPE_FORMAT, SHADOW_FORMAT};
use super::Renderer;
use crate::engine::gpu::DEPTH_FORMAT;
use crate::engine::pipeline::{create_pipeline, PipelineDesc};
use crate::engine::Gpu;
use crate::ui::UiVertex;
use crate::world::mesh::Vertex;
use glam::Mat4;
use std::collections::HashMap;
use std::mem::size_of;

const WORLD: Shader = Shader::new("world.wgsl", &[]);
/// The vertex shader reading chunk meshes' packed vertices (`CHUNK`).
const WORLD_CHUNK: Shader = Shader::new("world.wgsl", &["CHUNK"]);
/// Chunk meshes without alpha tests (`NO_DISCARD`): for the plain whole-block faces, whose
/// depth test can then run before the fragment shader.
const WORLD_PLAIN: Shader = Shader::new("world.wgsl", &["CHUNK", "NO_DISCARD"]);
const SHADOW: Shader = Shader::new("shadow.wgsl", &[]);
const SHADOW_CHUNK: Shader = Shader::new("shadow.wgsl", &["CHUNK"]);
const SHADOW_PLAIN: Shader = Shader::new("shadow.wgsl", &["CHUNK", "NO_DISCARD"]);
const SKY: Shader = Shader::new("sky.wgsl", &[]);
const UI: Shader = Shader::new("ui.wgsl", &[]);
/// The scope eyepiece: the world's vertices, the scope's view as its colour.
pub(super) const LENS: Shader = Shader::new("lens.wgsl", &[]);
/// The menus' backdrop: blurring down (`fs_main`) what `fs_across` has blurred across.
pub(super) const BLUR: Shader = Shader::new("blur.wgsl", &[]);

/// Every shader made into pipelines (shaders.rs's test checks them all).
#[cfg(test)]
pub(super) const ALL_SHADERS: &[Shader] = &[WORLD, WORLD_CHUNK, WORLD_PLAIN, SHADOW, SHADOW_CHUNK, SHADOW_PLAIN, SKY, UI, LENS, BLUR];

/// The shader modules, each made once.
pub(super) struct Modules {
    device: wgpu::Device,
    made: HashMap<Shader, wgpu::ShaderModule>,
}

impl Modules {
    pub fn new(device: &wgpu::Device) -> Self {
        Self { device: device.clone(), made: HashMap::new() }
    }

    pub fn get(&mut self, s: Shader) -> wgpu::ShaderModule {
        let device = &self.device;
        self.made
            .entry(s)
            .or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(s.file),
                    source: wgpu::ShaderSource::Wgsl(s.source().into()),
                })
            })
            .clone()
    }
}

const WORLD_ATTRS: [(wgpu::VertexFormat, u64); 5] = [
    (wgpu::VertexFormat::Float32x3, 0),
    (wgpu::VertexFormat::Float32x2, 12),
    (wgpu::VertexFormat::Float32, 20),
    (wgpu::VertexFormat::Unorm8x4, 24),
    (wgpu::VertexFormat::Unorm8x4, 28),
];

/// A chunk mesh's `ChunkVertex`: position and layer, uv, light, tint.
const CHUNK_ATTRS: [(wgpu::VertexFormat, u64); 4] = [
    (wgpu::VertexFormat::Uint16x4, 0),
    (wgpu::VertexFormat::Uint16x2, 8),
    (wgpu::VertexFormat::Unorm8x4, 12),
    (wgpu::VertexFormat::Unorm8x4, 16),
];

const UI_ATTRS: [(wgpu::VertexFormat, u64); 5] = [
    (wgpu::VertexFormat::Float32x2, 0),
    (wgpu::VertexFormat::Float32x2, 8),
    (wgpu::VertexFormat::Float32x4, 16),
    (wgpu::VertexFormat::Float32x4, 32),
    (wgpu::VertexFormat::Float32, 48),
];

/// The world pipelines' immediates (`Push` in frame.wgsl): the view, and what is drawn
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
    pub sky: wgpu::RenderPipeline,
    pub world_chunk: wgpu::RenderPipeline,
    pub water_chunk: wgpu::RenderPipeline,
    pub world: wgpu::RenderPipeline,
    pub water: wgpu::RenderPipeline,
    pub overlay: wgpu::RenderPipeline,
}

/// The pipelines of the main pass: the scene's, and the plain whole-block faces, player fade,
/// lines and UI.
pub(super) struct MainPipes {
    pub scene: ScenePipes,
    pub world_plain: wgpu::RenderPipeline,
    pub player_fade: wgpu::RenderPipeline,
    pub line: wgpu::RenderPipeline,
    pub ui: wgpu::RenderPipeline,
}

/// The world's other things (`Vertex`) drawn into a pass with `color` and depth targets.
fn world_desc<'a>(
    module: &'a wgpu::ShaderModule,
    color: wgpu::TextureFormat,
    samples: u32,
    layout: &'a wgpu::PipelineLayout,
) -> PipelineDesc<'a> {
    PipelineDesc {
        vert: (module, "vs_main"),
        frag: (module, "fs_main"),
        stride: size_of::<Vertex>() as u64,
        attributes: &WORLD_ATTRS,
        instance: false,
        layout,
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull: true,
        depth_test: true,
        depth_write: true,
        blend: false,
        multiply: false,
        color: Some(color),
        depth: Some(DEPTH_FORMAT),
        depth_bias: None,
        samples,
        alpha_to_coverage: false,
    }
}

impl ScenePipes {
    pub fn new(
        d: &wgpu::Device,
        modules: &mut Modules,
        color: wgpu::TextureFormat,
        samples: u32,
        world_layout: &wgpu::PipelineLayout,
    ) -> Self {
        let (world_m, chunk_m, sky_m) = (modules.get(WORLD), modules.get(WORLD_CHUNK), modules.get(SKY));
        let world_desc = world_desc(&world_m, color, samples, world_layout);
        // Chunk meshes (`ChunkVertex`).
        let chunk_desc = PipelineDesc {
            vert: (&chunk_m, "vs_main"),
            frag: (&chunk_m, "fs_main"),
            stride: size_of::<ChunkVertex>() as u64,
            attributes: &CHUNK_ATTRS,
            instance: true,
            ..world_desc
        };
        // Only the opaque world pass smooths cut-out edges (with anti-aliasing on).
        let world = create_pipeline(d, &PipelineDesc { alpha_to_coverage: samples > 1, ..world_desc });
        let world_chunk = create_pipeline(d, &PipelineDesc { alpha_to_coverage: samples > 1, ..chunk_desc });
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
                vert: (&sky_m, "vs_main"),
                frag: (&sky_m, "fs_main"),
                attributes: &[],
                cull: false,
                depth_test: false,
                depth_write: false,
                ..world_desc
            },
        );
        Self { sky, world_chunk, water_chunk, world, water, overlay }
    }
}

impl MainPipes {
    pub fn new(
        d: &wgpu::Device,
        modules: &mut Modules,
        color: wgpu::TextureFormat,
        samples: u32,
        world_layout: &wgpu::PipelineLayout,
        ui_layout: &wgpu::PipelineLayout,
    ) -> Self {
        let (world_m, plain_m, ui_m) = (modules.get(WORLD), modules.get(WORLD_PLAIN), modules.get(UI));
        let world_desc = world_desc(&world_m, color, samples, world_layout);
        // (no alpha to coverage either: that too would make the depth test wait for the shader;
        // these faces are fully covered anyway)
        let world_plain = create_pipeline(
            d,
            &PipelineDesc {
                vert: (&plain_m, "vs_main"),
                frag: (&plain_m, "fs_main"),
                stride: size_of::<ChunkVertex>() as u64,
                attributes: &CHUNK_ATTRS,
                instance: true,
                ..world_desc
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
                topology: wgpu::PrimitiveTopology::LineList,
                cull: false,
                depth_write: false,
                blend: true,
                ..world_desc
            },
        );
        let ui = create_pipeline(
            d,
            &PipelineDesc {
                vert: (&ui_m, "vs_main"),
                frag: (&ui_m, "fs_main"),
                stride: size_of::<UiVertex>() as u64,
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
            scene: ScenePipes::new(d, modules, color, samples, world_layout),
            world_plain,
            player_fade,
            line,
            ui,
        }
    }
}

/// Something showing the scope's image in the main pass, drawn by `frag`'s `fs_main` (`LENS`,
/// `BLUR`).
pub(super) fn create_scope_view_pipe(
    d: &wgpu::Device,
    modules: &mut Modules,
    color: wgpu::TextureFormat,
    samples: u32,
    layout: &wgpu::PipelineLayout,
    frag: Shader,
) -> wgpu::RenderPipeline {
    let (world_m, frag_m) = (modules.get(WORLD), modules.get(frag));
    create_pipeline(
        d,
        &PipelineDesc {
            frag: (&frag_m, "fs_main"),
            cull: false,
            ..world_desc(&world_m, color, samples, layout)
        },
    )
}

/// The backdrop's first blur pass: the scope's image blurred across, into the blur target, a
/// full-screen triangle.
pub(super) fn create_blur_across_pipe(d: &wgpu::Device, modules: &mut Modules, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
    let (sky_m, blur_m) = (modules.get(SKY), modules.get(BLUR));
    create_pipeline(
        d,
        &PipelineDesc {
            vert: (&sky_m, "vs_main"),
            frag: (&blur_m, "fs_across"),
            stride: 0,
            attributes: &[],
            instance: false,
            layout,
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull: false,
            depth_test: false,
            depth_write: false,
            blend: false,
            multiply: false,
            color: Some(SCOPE_FORMAT),
            depth: None,
            depth_bias: None,
            samples: 1,
            alpha_to_coverage: false,
        },
    )
}

/// The shadow map's: depth only, biased against shadow acne. `plain`: without alpha tests,
/// for the plain whole-block faces; `chunk`: for chunk meshes (their packed vertices).
pub(super) fn create_shadow_pipe(
    d: &wgpu::Device,
    modules: &mut Modules,
    layout: &wgpu::PipelineLayout,
    plain: bool,
    chunk: bool,
) -> wgpu::RenderPipeline {
    let m = modules.get(match (plain, chunk) {
        (true, _) => SHADOW_PLAIN,
        (false, true) => SHADOW_CHUNK,
        (false, false) => SHADOW,
    });
    create_pipeline(
        d,
        &PipelineDesc {
            vert: (&m, "vs_main"),
            frag: (&m, "fs_main"),
            stride: if chunk { size_of::<ChunkVertex>() } else { size_of::<Vertex>() } as u64,
            attributes: if chunk { &CHUNK_ATTRS } else { &WORLD_ATTRS },
            instance: chunk,
            layout,
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull: false,
            depth_test: true,
            depth_write: true,
            blend: false,
            multiply: false,
            color: None,
            depth: Some(SHADOW_FORMAT),
            depth_bias: Some((2, 2.0)),
            samples: 1,
            alpha_to_coverage: false,
        },
    )
}

impl Renderer {
    /// The main pass's sample count changed (anti-aliasing): the pipelines drawing into it are
    /// made again.
    pub(super) fn rebuild_screen_pipes(&mut self, gpu: &Gpu) {
        let (d, color, samples) = (&gpu.device, gpu.surface_format, gpu.samples);
        self.pipes = MainPipes::new(d, &mut self.modules, color, samples, &self.world_layout, &self.ui_layout);
        self.lens_pipe = create_scope_view_pipe(d, &mut self.modules, color, samples, &self.lens_layout, LENS);
        self.blur_pipe = create_scope_view_pipe(d, &mut self.modules, color, samples, &self.lens_layout, BLUR);
        self.pass_version = gpu.pass_version;
    }
}
