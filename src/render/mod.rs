//! The frame: shadow map, sky, chunk meshes, dynamic geometry (entities, particles, the
//! hand) and the UI, drawn with Vulkan.

mod arena;

use crate::engine::pipeline::{create_layout, create_pipeline, PipelineDesc};
use crate::engine::resources::Image;
use crate::engine::{Buffer, Gpu, SamplerKind, Texture, FRAMES_IN_FLIGHT};
use crate::ui::{UiVertex, FONT_TEX_H, FONT_TEX_W};
use crate::world::mesh::{MeshData, Vertex};
use crate::world::textures::{tex, TILE};
use crate::world::{ChunkPos, FastMap};
use ash::vk;
use glam::{Mat4, Vec3, Vec4};
use std::collections::VecDeque;
use std::mem::size_of;

const WORLD_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/world.vert.spv"));
const WORLD_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/world.frag.spv"));
const SHADOW_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.vert.spv"));
const SHADOW_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.frag.spv"));
const SKY_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv"));
const SKY_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv"));
const UI_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui.vert.spv"));
const UI_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui.frag.spv"));
const LENS_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lens.frag.spv"));

pub const SHADOW_SIZE: u32 = 4096;
/// The scope's view: its size in pixels (square) and format.
pub const SCOPE_SIZE: u32 = 512;
const SCOPE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
const SHADOW_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
const UI_MAX_VERTS: usize = 150_000;
const DYN_MAX_VERTS: usize = 250_000;
const MAX_UPLOADS_PER_FRAME: usize = 24;
/// Indirect draw commands per frame (chunks' opaque parts, a few each).
const MAX_INDIRECT: usize = 65536;
/// Per-frame staging memory for chunk uploads (reused instead of allocating per chunk).
const STAGING_SIZE: usize = 16 << 20;

fn as_bytes<T: Copy>(v: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v as *const T as *const u8, size_of::<T>()) }
}

impl Default for FrameUbo {
    fn default() -> Self {
        // (all zero: every field is floats)
        unsafe { std::mem::zeroed() }
    }
}

/// Weapon lights at once (`FrameUbo::spots`).
pub const MAX_SPOTS: usize = 4;
/// Each weapon light's shadow map: a square this big in a strip under the sun's in the same
/// depth image (as frame.glsl's `SHADOW_SPOT_*` reads them), one beside the other.
pub const SPOT_SHADOW: u32 = 1024;
/// The shadow depth image's height: the sun's square, and the weapon lights' strip under it.
pub const SHADOW_HEIGHT: u32 = SHADOW_SIZE + SPOT_SHADOW;
/// How far a weapon light reaches (blocks): its shadow map's far end.
pub const SPOT_REACH: f32 = 30.0;

/// Must match `heldLights` in frame.glsl.
pub const MAX_HELD_LIGHTS: usize = 8;

/// Mirrors `FrameData` in shaders/frame.glsl (std140, all vec4/mat4).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameUbo {
    pub view_proj: [f32; 16],
    pub inv_view_proj: [f32; 16],
    pub light_view_proj: [f32; 16],
    pub cam_pos: [f32; 4],
    pub sun_dir: [f32; 4],
    pub light_dir: [f32; 4],
    pub sun_color: [f32; 4],
    pub ambient: [f32; 4],
    pub fog: [f32; 4],
    pub misc: [f32; 4],
    /// Held torches and lanterns (this player's and the other LAN players'): position, intensity.
    pub held_lights: [[f32; 4]; MAX_HELD_LIGHTS],
    /// Weapon lights: pairs of (xyz position, w on) and (xyz direction, w the cosine of the
    /// cone's edge). Must match `spots` in frame.glsl.
    pub spots: [[f32; 4]; 2 * MAX_SPOTS],
    /// x: how many pixels a block at distance 1 covers (detail too small for the screen is
    /// simplified by it).
    pub detail: [f32; 4],
    /// Each weapon light's view (its shadow map's): what it lights, from where it is.
    pub spot_view_proj: [[f32; 16]; MAX_SPOTS],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DrawPush {
    view_proj: [f32; 16],
    params: [f32; 4],
}

struct ChunkGpu {
    /// Vertices, then (from `index_offset` on) indices; None for an empty mesh.
    mesh: Option<arena::Range>,
    index_offset: u64,
    opaque: u32,
    /// Opaque indices without the faces between leaves and the plants, and the former's count
    /// (see `MeshData`).
    solid: u32,
    /// Whole-block faces by direction, at the end of the solid indices.
    dirs: [u32; 6],
    leaf_inner: u32,
    water: u32,
    min: Vec3,
    max: Vec3,
}

pub struct FrameInfo<'a> {
    pub ubo: FrameUbo,
    pub view_proj: Mat4,
    pub vm_view_proj: Mat4,
    pub light_view_proj: Mat4,
    pub cam_pos: Vec3,
    pub view_distance: f32,
    pub shadows: bool,
    pub shadow_distance: f32,
    /// How many pixels a block at distance 1 covers: far chunks leave out what is too small.
    pub detail_px: f32,
    pub ui: &'a [UiVertex],
    /// Scissor regions of the UI vertices: (first vertex, clip rectangle or None).
    pub ui_clips: &'a [(u32, Option<[f32; 4]>)],
    /// Box around the targeted block (world corners).
    pub outline: Option<(Vec3, Vec3)>,
    pub particles: &'a [Vertex],
    pub overlay: &'a [Vertex],
    pub viewmodel: &'a [Vertex],
    /// Player model: always casts shadows, drawn only when `entity_visible` (third person).
    pub entity: &'a [Vertex],
    pub entity_visible: bool,
    /// The first vertices in `entity` are this player's model.
    pub player_vertex_count: u32,
    pub player_opacity: f32,
    pub translucent: &'a [Vertex],
    /// The first-person gun's see-through glass (drawn blended after it), and its scope's
    /// eyepiece, which shows `scope`'s view.
    pub viewmodel_glass: &'a [Vertex],
    pub lens: &'a [Vertex],
    pub scope: Option<ScopeView>,
}

/// The view through the scope (magnified): rendered into the scope image before the main
/// pass, then shown on the eyepiece (`FrameInfo::lens`).
pub struct ScopeView {
    pub ubo: FrameUbo,
    pub view_proj: Mat4,
    pub cam_pos: Vec3,
    /// Pixels a block at distance 1 covers in the scope's image.
    pub detail_px: f32,
}

/// A chunk drawn this frame: where its mesh is and how far away it is.
struct VisibleChunk {
    /// Index ranges (first, count) of its opaque part to draw: the faces turned toward the
    /// camera, without the small detail far chunks leave out.
    parts: [(u32, u32); 4],
    buffer: vk::Buffer,
    vertices: u64,
    indices: u64,
    /// Opaque indices (where the water's start).
    opaque: u32,
    water: u32,
    dist2: f32,
}

struct Frustum([Vec4; 6]);

impl Frustum {
    fn new(m: Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        Self([r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2])
    }

    fn visible(&self, min: Vec3, max: Vec3) -> bool {
        self.0.iter().all(|p| {
            let v = Vec3::new(
                if p.x >= 0.0 { max.x } else { min.x },
                if p.y >= 0.0 { max.y } else { min.y },
                if p.z >= 0.0 { max.z } else { min.z },
            );
            p.truncate().dot(v) + p.w >= 0.0
        })
    }
}

pub struct Renderer {
    world_dsl: vk::DescriptorSetLayout,
    ui_dsl: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    world_sets: Vec<vk::DescriptorSet>,
    ui_set: vk::DescriptorSet,
    world_layout: vk::PipelineLayout,
    ui_layout: vk::PipelineLayout,
    sky_pipe: vk::Pipeline,
    world_pipe: vk::Pipeline,
    water_pipe: vk::Pipeline,
    player_fade_pipe: vk::Pipeline,
    overlay_pipe: vk::Pipeline,
    line_pipe: vk::Pipeline,
    shadow_pipe: vk::Pipeline,
    ui_pipe: vk::Pipeline,
    /// `Gpu::pass_version` the main-pass pipelines were made for.
    pass_version: u64,
    block_tex: Texture,
    /// Mip levels of the block texture.
    block_mips: usize,
    /// Layers of the block texture replaced while the game runs (the guide book's pages): the
    /// first layer, how many, and each mip level of them (layer after layer).
    layer_uploads: VecDeque<(u32, u32, Vec<Vec<u8>>)>,
    font_tex: Texture,
    shadow_image: Image,
    shadow_sampler: vk::Sampler,
    shadow_pass: vk::RenderPass,
    shadow_fb: vk::Framebuffer,
    /// The scope's view (picture in picture): its images, pass and framebuffer, the pipelines
    /// it is drawn with (like the main pass's), its uniform buffers and descriptor sets per
    /// frame slot; and the eyepiece's pipeline and set, which show it on the gun.
    scope_color: Image,
    scope_depth: Image,
    scope_sampler: vk::Sampler,
    scope_pass: vk::RenderPass,
    scope_fb: vk::Framebuffer,
    scope_pipes: [vk::Pipeline; 7],
    scope_ubos: Vec<Buffer>,
    scope_sets: Vec<vk::DescriptorSet>,
    lens_dsl: vk::DescriptorSetLayout,
    lens_set: vk::DescriptorSet,
    lens_layout: vk::PipelineLayout,
    lens_pipe: vk::Pipeline,
    ubos: Vec<Buffer>,
    ui_bufs: Vec<Buffer>,
    dyn_bufs: Vec<Buffer>,
    /// Per frame slot: the chunks' opaque draws as indirect commands.
    indirect: Vec<Buffer>,
    staging: Vec<Buffer>,
    chunks: FastMap<ChunkPos, ChunkGpu>,
    arena: arena::Arena,
    /// Frames recorded so far (for giving chunk memory back once no frame uses it).
    frame: u64,
    pending: VecDeque<MeshData>,
    pub drawn_chunks: usize,
    /// GPU timestamps: 4 per frame slot (start, shadows done, world done, end).
    queries: Option<(vk::QueryPool, f32)>,
    queries_written: [bool; FRAMES_IN_FLIGHT],
    /// Last measured GPU time in milliseconds: (shadow pass, world, UI).
    pub gpu_ms: Option<[f32; 3]>,
    /// Last frame's CPU time in ms: chunk uploads, command recording, submit + present.
    pub cpu_detail: [f32; 3],
    /// Last frame's recording split in ms: buffers written, shadow pass, visible chunks
    /// picked, world draws, the rest (for --bench).
    pub rec_detail: [f32; 5],
}

type IndirectDraw = (vk::Buffer, vk::DrawIndexedIndirectCommand);

/// One indirect draw of `count` indices from `first` of a chunk's mesh.
fn chunk_draw(buffer: vk::Buffer, vertices: u64, indices: u64, first: u32, count: u32) -> IndirectDraw {
    (
        buffer,
        vk::DrawIndexedIndirectCommand {
            index_count: count,
            instance_count: 1,
            first_index: (indices / 4) as u32 + first,
            vertex_offset: (vertices / size_of::<Vertex>() as u64) as i32,
            first_instance: 0,
        },
    )
}

/// Records `draws` as indirect draws from `ind`, from command `base` on: one command per mesh
/// buffer (the meshes share a few big ones), in order within each. Returns the next free
/// command, or None (nothing recorded) when they do not fit.
unsafe fn record_indirect(
    d: &ash::Device,
    cmd: vk::CommandBuffer,
    ind: &Buffer,
    base: usize,
    draws: &mut [IndirectDraw],
    multi: bool,
) -> Option<usize> {
    if base + draws.len() > MAX_INDIRECT {
        return None;
    }
    draws.sort_by_key(|(b, _)| vk::Handle::as_raw(*b));
    let flat: Vec<vk::DrawIndexedIndirectCommand> = draws.iter().map(|(_, c)| *c).collect();
    let stride = size_of::<vk::DrawIndexedIndirectCommand>();
    ind.write(base * stride, &flat);
    let mut i = 0;
    while i < draws.len() {
        let b = draws[i].0;
        let n = draws[i..].iter().take_while(|(x, _)| *x == b).count();
        d.cmd_bind_vertex_buffers(cmd, 0, &[b], &[0]);
        d.cmd_bind_index_buffer(cmd, b, 0, vk::IndexType::UINT32);
        let offset = ((base + i) * stride) as u64;
        if multi {
            d.cmd_draw_indexed_indirect(cmd, ind.handle, offset, n as u32, stride as u32);
        } else {
            for k in 0..n {
                let at = offset + (k * stride) as u64;
                d.cmd_draw_indexed_indirect(cmd, ind.handle, at, 1, stride as u32);
            }
        }
        i += n;
    }
    Some(base + draws.len())
}

const WORLD_ATTRS: [(vk::Format, u32); 5] = [
    (vk::Format::R32G32B32_SFLOAT, 0),
    (vk::Format::R32G32_SFLOAT, 12),
    (vk::Format::R32_SFLOAT, 20),
    (vk::Format::R8G8B8A8_UNORM, 24),
    (vk::Format::R8G8B8A8_UNORM, 28),
];

/// The pipelines of the main pass: sky, world, water, player fade, overlay, lines and UI.
fn create_main_pipes(
    d: &ash::Device,
    render_pass: vk::RenderPass,
    samples: vk::SampleCountFlags,
    world_layout: vk::PipelineLayout,
    ui_layout: vk::PipelineLayout,
) -> [vk::Pipeline; 7] {
    let world_desc = PipelineDesc {
        vert: WORLD_VERT,
        frag: WORLD_FRAG,
        stride: size_of::<Vertex>() as u32,
        attributes: &WORLD_ATTRS,
        layout: world_layout,
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
    };
    // Only the opaque world pass smooths cut-out edges (with anti-aliasing on).
    let world_pipe = create_pipeline(
        d,
        &PipelineDesc {
            alpha_to_coverage: samples != vk::SampleCountFlags::TYPE_1,
            ..world_desc
        },
    );
    let water_pipe = create_pipeline(
        d,
        &PipelineDesc {
            cull: false,
            depth_write: false,
            blend: true,
            ..world_desc
        },
    );
    let player_fade_pipe = create_pipeline(
        d,
        &PipelineDesc {
            depth_write: false,
            blend: true,
            ..world_desc
        },
    );
    let overlay_pipe = create_pipeline(
        d,
        &PipelineDesc {
            depth_write: false,
            blend: true,
            multiply: true,
            ..world_desc
        },
    );
    let line_pipe = create_pipeline(
        d,
        &PipelineDesc {
            topology: vk::PrimitiveTopology::LINE_LIST,
            cull: false,
            depth_write: false,
            blend: true,
            ..world_desc
        },
    );
    let sky_pipe = create_pipeline(
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
    let ui_attrs = [
        (vk::Format::R32G32_SFLOAT, 0),
        (vk::Format::R32G32_SFLOAT, 8),
        (vk::Format::R32G32B32A32_SFLOAT, 16),
        (vk::Format::R32G32B32A32_SFLOAT, 32),
        (vk::Format::R32_SFLOAT, 48),
    ];
    let ui_pipe = create_pipeline(
        d,
        &PipelineDesc {
            vert: UI_VERT,
            frag: UI_FRAG,
            stride: size_of::<UiVertex>() as u32,
            attributes: &ui_attrs,
            layout: ui_layout,
            render_pass,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            cull: false,
            depth_test: false,
            depth_write: false,
            blend: true,
            multiply: false,
            color: true,
            depth_bias: None,
            samples,
            alpha_to_coverage: false,
        },
    );
    [
        sky_pipe,
        world_pipe,
        water_pipe,
        player_fade_pipe,
        overlay_pipe,
        line_pipe,
        ui_pipe,
    ]
}

/// The scope eyepiece's pipeline: the world's vertices, the scope's view as its colour.
fn create_lens_pipe(
    d: &ash::Device,
    render_pass: vk::RenderPass,
    samples: vk::SampleCountFlags,
    layout: vk::PipelineLayout,
) -> vk::Pipeline {
    create_pipeline(
        d,
        &PipelineDesc {
            vert: WORLD_VERT,
            frag: LENS_FRAG,
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
        },
    )
}

/// The scope's pass: a colour image the eyepiece samples afterwards, and a depth image.
fn create_scope_pass(device: &ash::Device) -> vk::RenderPass {
    let attachments = [
        vk::AttachmentDescription::default()
            .format(SCOPE_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        vk::AttachmentDescription::default()
            .format(vk::Format::D32_SFLOAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    ];
    let color_ref = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let depth_ref = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)
        .depth_stencil_attachment(&depth_ref)];
    let fragment_tests =
        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
    let deps = [
        // The last frame's eyepiece has read the image (and its depth is written) before it
        // is drawn again.
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | fragment_tests)
            .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | fragment_tests)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            ),
        // The image is drawn before the eyepiece reads it.
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
            .expect("create scope render pass")
    }
}

fn create_shadow_pass(device: &ash::Device) -> vk::RenderPass {
    let attachments = [vk::AttachmentDescription::default()
        .format(SHADOW_FORMAT)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
    let depth_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_ref)];
    let deps = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_stage_mask(
                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            ),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
            .expect("create shadow render pass")
    }
}

impl Renderer {
    /// Rebuild the shared texture array after a player uploads or receives a skin.
    /// Replaces `count` layers of the block texture from `first` on, next frame. `levels`
    /// holds every mip level of them, as `Texture::new` takes them.
    pub fn queue_layers(&mut self, first: u32, count: u32, levels: Vec<Vec<u8>>) {
        if levels.len() == self.block_mips {
            self.layer_uploads.push_back((first, count, levels));
        }
    }

    /// Records the queued layer replacements (before the frame's passes).
    unsafe fn flush_layer_uploads(&mut self, gpu: &mut Gpu, cmd: vk::CommandBuffer) {
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
        let replacement = Texture::new(
            gpu,
            TILE as u32,
            TILE as u32,
            tex::LAYERS as u32,
            vk::Format::R8G8B8A8_SRGB,
            levels,
            SamplerKind::Blocks,
        );
        let info = [vk::DescriptorImageInfo {
            sampler: replacement.sampler,
            image_view: replacement.image.view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        unsafe {
            for &set in self.world_sets.iter().chain(&self.scope_sets) {
                gpu.device.update_descriptor_sets(
                    &[vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&info)],
                    &[],
                );
            }
            gpu.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(self.ui_set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&info)],
                &[],
            );
        }
        self.block_mips = levels.len();
        // Replaced layers queued before this are in the new levels already, or will be
        // queued again.
        self.layer_uploads.clear();
        let old = std::mem::replace(&mut self.block_tex, replacement);
        old.destroy(&gpu.device);
    }

    pub fn new(gpu: &Gpu, block_levels: &[Vec<u8>], font_atlas: &[u8]) -> Self {
        assert_eq!(size_of::<Vertex>(), 32);
        assert_eq!(size_of::<UiVertex>(), 52);
        assert_eq!(size_of::<FrameUbo>(), 304 + 16 * MAX_HELD_LIGHTS + 32 * MAX_SPOTS + 16 + 64 * MAX_SPOTS);

        let block_tex = Texture::new(
            gpu,
            TILE as u32,
            TILE as u32,
            tex::LAYERS as u32,
            vk::Format::R8G8B8A8_SRGB,
            block_levels,
            SamplerKind::Blocks,
        );
        let font_tex = Texture::new(
            gpu,
            FONT_TEX_W,
            FONT_TEX_H,
            1,
            vk::Format::R8_UNORM,
            &[font_atlas.to_vec()],
            SamplerKind::Font,
        );

        unsafe {
            let d = &gpu.device;

            // Shadow map
            let shadow_image = Image::new(
                d,
                &gpu.mem_props,
                SHADOW_SIZE,
                SHADOW_HEIGHT,
                1,
                1,
                SHADOW_FORMAT,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::DEPTH,
                vk::ImageViewType::TYPE_2D,
            );
            let shadow_sampler = d
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                        .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
                        .compare_enable(true)
                        .compare_op(vk::CompareOp::LESS_OR_EQUAL)
                        .max_lod(0.0),
                    None,
                )
                .unwrap();
            let shadow_pass = create_shadow_pass(d);
            let shadow_views = [shadow_image.view];
            let shadow_fb = d
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(shadow_pass)
                        .attachments(&shadow_views)
                        .width(SHADOW_SIZE)
                        .height(SHADOW_HEIGHT)
                        .layers(1),
                    None,
                )
                .unwrap();

            // The scope's view
            let scope_color = Image::new(
                d,
                &gpu.mem_props,
                SCOPE_SIZE,
                SCOPE_SIZE,
                1,
                1,
                SCOPE_FORMAT,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
                vk::ImageViewType::TYPE_2D,
            );
            let scope_depth = Image::new(
                d,
                &gpu.mem_props,
                SCOPE_SIZE,
                SCOPE_SIZE,
                1,
                1,
                vk::Format::D32_SFLOAT,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::ImageAspectFlags::DEPTH,
                vk::ImageViewType::TYPE_2D,
            );
            let scope_sampler = d
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .max_lod(0.0),
                    None,
                )
                .unwrap();
            let scope_pass = create_scope_pass(d);
            let scope_views = [scope_color.view, scope_depth.view];
            let scope_fb = d
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(scope_pass)
                        .attachments(&scope_views)
                        .width(SCOPE_SIZE)
                        .height(SCOPE_SIZE)
                        .layers(1),
                    None,
                )
                .unwrap();

            // Descriptors
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
            let world_dsl = d
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&world_bindings),
                    None,
                )
                .unwrap();
            let ui_dsl = d
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&ui_bindings),
                    None,
                )
                .unwrap();
            let lens_bindings = [binding(0, cis, frag)];
            let lens_dsl = d
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&lens_bindings),
                    None,
                )
                .unwrap();
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
            layouts.extend(std::iter::repeat(world_dsl).take(FRAMES_IN_FLIGHT));
            layouts.push(lens_dsl);
            let sets = d
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&layouts),
                )
                .unwrap();
            let world_sets = sets[..FRAMES_IN_FLIGHT].to_vec();
            let ui_set = sets[FRAMES_IN_FLIGHT];
            let scope_sets = sets[FRAMES_IN_FLIGHT + 1..2 * FRAMES_IN_FLIGHT + 1].to_vec();
            let lens_set = sets[2 * FRAMES_IN_FLIGHT + 1];

            let host =
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let ubos: Vec<Buffer> = (0..FRAMES_IN_FLIGHT)
                .map(|_| {
                    Buffer::new(
                        gpu,
                        size_of::<FrameUbo>() as u64,
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        host,
                    )
                })
                .collect();
            let scope_ubos: Vec<Buffer> = (0..FRAMES_IN_FLIGHT)
                .map(|_| {
                    Buffer::new(
                        gpu,
                        size_of::<FrameUbo>() as u64,
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        host,
                    )
                })
                .collect();

            let block_info = [vk::DescriptorImageInfo {
                sampler: block_tex.sampler,
                image_view: block_tex.image.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let font_info = [vk::DescriptorImageInfo {
                sampler: font_tex.sampler,
                image_view: font_tex.image.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let shadow_info = [vk::DescriptorImageInfo {
                sampler: shadow_sampler,
                image_view: shadow_image.view,
                image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            }];
            let ubo_infos: Vec<[vk::DescriptorBufferInfo; 1]> = ubos
                .iter()
                .map(|b| {
                    [vk::DescriptorBufferInfo {
                        buffer: b.handle,
                        offset: 0,
                        range: size_of::<FrameUbo>() as u64,
                    }]
                })
                .collect();
            let scope_ubo_infos: Vec<[vk::DescriptorBufferInfo; 1]> = scope_ubos
                .iter()
                .map(|b| {
                    [vk::DescriptorBufferInfo {
                        buffer: b.handle,
                        offset: 0,
                        range: size_of::<FrameUbo>() as u64,
                    }]
                })
                .collect();
            let lens_info = [vk::DescriptorImageInfo {
                sampler: scope_sampler,
                image_view: scope_color.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let mut writes = Vec::new();
            for (i, &set) in scope_sets.iter().enumerate() {
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(cis)
                        .image_info(&block_info),
                );
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(1)
                        .descriptor_type(cis)
                        .image_info(&shadow_info),
                );
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(2)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&scope_ubo_infos[i]),
                );
            }
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(lens_set)
                    .dst_binding(0)
                    .descriptor_type(cis)
                    .image_info(&lens_info),
            );
            for (i, &set) in world_sets.iter().enumerate() {
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(cis)
                        .image_info(&block_info),
                );
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(1)
                        .descriptor_type(cis)
                        .image_info(&shadow_info),
                );
                writes.push(
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(2)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&ubo_infos[i]),
                );
            }
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(ui_set)
                    .dst_binding(0)
                    .descriptor_type(cis)
                    .image_info(&font_info),
            );
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(ui_set)
                    .dst_binding(1)
                    .descriptor_type(cis)
                    .image_info(&block_info),
            );
            d.update_descriptor_sets(&writes, &[]);

            // Pipelines
            let world_layout = create_layout(d, &[world_dsl], size_of::<DrawPush>() as u32);
            let ui_layout = create_layout(d, &[ui_dsl], 16);
            let [sky_pipe, world_pipe, water_pipe, player_fade_pipe, overlay_pipe, line_pipe, ui_pipe] =
                create_main_pipes(d, gpu.render_pass, gpu.samples, world_layout, ui_layout);
            let scope_pipes =
                create_main_pipes(d, scope_pass, vk::SampleCountFlags::TYPE_1, world_layout, ui_layout);
            let lens_layout = create_layout(d, &[world_dsl, lens_dsl], size_of::<DrawPush>() as u32);
            let lens_pipe = create_lens_pipe(d, gpu.render_pass, gpu.samples, lens_layout);
            let shadow_pipe = create_pipeline(
                d,
                &PipelineDesc {
                    vert: SHADOW_VERT,
                    frag: SHADOW_FRAG,
                    stride: size_of::<Vertex>() as u32,
                    attributes: &WORLD_ATTRS,
                    layout: world_layout,
                    render_pass: shadow_pass,
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
            );

            let ui_bufs = (0..FRAMES_IN_FLIGHT)
                .map(|_| {
                    Buffer::new(
                        gpu,
                        (UI_MAX_VERTS * size_of::<UiVertex>()) as u64,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        host,
                    )
                })
                .collect();
            let dyn_bufs = (0..FRAMES_IN_FLIGHT)
                .map(|_| {
                    Buffer::new(
                        gpu,
                        (DYN_MAX_VERTS * size_of::<Vertex>()) as u64,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        host,
                    )
                })
                .collect();
            let indirect = (0..FRAMES_IN_FLIGHT)
                .map(|_| {
                    Buffer::new(
                        gpu,
                        (MAX_INDIRECT * size_of::<vk::DrawIndexedIndirectCommand>()) as u64,
                        vk::BufferUsageFlags::INDIRECT_BUFFER,
                        host,
                    )
                })
                .collect();
            let staging = (0..FRAMES_IN_FLIGHT)
                .map(|_| {
                    Buffer::new(
                        gpu,
                        STAGING_SIZE as u64,
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        host,
                    )
                })
                .collect();

            Self {
                world_dsl,
                ui_dsl,
                pool,
                world_sets,
                ui_set,
                world_layout,
                ui_layout,
                sky_pipe,
                world_pipe,
                water_pipe,
                player_fade_pipe,
                overlay_pipe,
                line_pipe,
                shadow_pipe,
                ui_pipe,
                pass_version: gpu.pass_version,
                block_tex,
                block_mips: block_levels.len(),
                layer_uploads: VecDeque::new(),
                font_tex,
                shadow_image,
                shadow_sampler,
                shadow_pass,
                shadow_fb,
                scope_color,
                scope_depth,
                scope_sampler,
                scope_pass,
                scope_fb,
                scope_pipes,
                scope_ubos,
                scope_sets,
                lens_dsl,
                lens_set,
                lens_layout,
                lens_pipe,
                ubos,
                ui_bufs,
                dyn_bufs,
                indirect,
                staging,
                chunks: FastMap::default(),
                arena: arena::Arena::default(),
                frame: 0,
                pending: VecDeque::new(),
                drawn_chunks: 0,
                queries: gpu.timestamp_period.map(|period| {
                    let pool = d
                        .create_query_pool(
                            &vk::QueryPoolCreateInfo::default()
                                .query_type(vk::QueryType::TIMESTAMP)
                                .query_count(4 * FRAMES_IN_FLIGHT as u32),
                            None,
                        )
                        .expect("create query pool");
                    (pool, period)
                }),
                queries_written: [false; FRAMES_IN_FLIGHT],
                gpu_ms: None,
                cpu_detail: [0.0; 3],
                rec_detail: [0.0; 5],
            }
        }
    }

    pub fn queue_mesh(&mut self, mesh: MeshData) {
        self.pending.push_back(mesh);
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Drops every chunk mesh (when switching worlds).
    pub fn clear_chunks(&mut self) {
        self.pending.clear();
        for (_, c) in self.chunks.drain() {
            if let Some(r) = c.mesh {
                self.arena.retire(r, self.frame);
            }
        }
    }

    pub fn remove_chunk(&mut self, pos: ChunkPos) {
        self.pending.retain(|m| m.pos != pos);
        if let Some(r) = self.chunks.remove(&pos).and_then(|c| c.mesh) {
            self.arena.retire(r, self.frame);
        }
    }

    unsafe fn flush_uploads(&mut self, gpu: &mut Gpu, cmd: vk::CommandBuffer) {
        let mut any = false;
        // This frame slot's staging buffer is free: begin_frame waited for its fence.
        let ring = &self.staging[gpu.frame_slot];
        let mut ring_used = 0usize;
        for _ in 0..MAX_UPLOADS_PER_FRAME {
            let Some(m) = self.pending.pop_front() else {
                break;
            };
            let vbytes = std::mem::size_of_val(m.vertices.as_slice());
            let ioff = (vbytes + 15) & !15;
            let total = ioff + std::mem::size_of_val(m.indices.as_slice());
            if total <= STAGING_SIZE && ring_used + total > STAGING_SIZE {
                // Out of staging space this frame; upload it next frame.
                self.pending.push_front(m);
                break;
            }
            let (x0, z0) = ((m.pos.0 * 16) as f32, (m.pos.1 * 16) as f32);
            let mut chunk = ChunkGpu {
                mesh: None,
                index_offset: 0,
                opaque: m.opaque_count,
                solid: m.solid_count,
                dirs: m.dir_counts,
                leaf_inner: m.leaf_inner_count,
                water: m.indices.len() as u32 - m.opaque_count,
                min: Vec3::new(x0 - 1.0, m.min_y - 1.0, z0 - 1.0),
                max: Vec3::new(x0 + 17.0, m.max_y + 1.0, z0 + 17.0),
            };
            if !m.vertices.is_empty() {
                let range = self.arena.alloc(gpu, total as u64);
                let (src, src_offset) = if total <= STAGING_SIZE {
                    ring.write(ring_used, m.vertices.as_slice());
                    ring.write(ring_used + ioff, m.indices.as_slice());
                    let at = ring_used;
                    ring_used = (ring_used + total + 15) & !15;
                    (ring.handle, at)
                } else {
                    // Larger than the whole staging buffer: use a one-off buffer.
                    let staging = Buffer::new(
                        gpu,
                        total as u64,
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT,
                    );
                    staging.write(0, m.vertices.as_slice());
                    staging.write(ioff, m.indices.as_slice());
                    let handle = staging.handle;
                    gpu.defer_destroy(staging);
                    (handle, 0)
                };
                gpu.device.cmd_copy_buffer(
                    cmd,
                    src,
                    self.arena.buffer(range),
                    &[vk::BufferCopy {
                        src_offset: src_offset as u64,
                        dst_offset: range.offset,
                        size: total as u64,
                    }],
                );
                chunk.mesh = Some(range);
                chunk.index_offset = ioff as u64;
                any = true;
            }
            if let Some(old) = self.chunks.insert(m.pos, chunk).and_then(|c| c.mesh) {
                self.arena.retire(old, self.frame);
            }
        }
        if any {
            let barrier = vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(
                    vk::AccessFlags::VERTEX_ATTRIBUTE_READ | vk::AccessFlags::INDEX_READ,
                );
            gpu.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::VERTEX_INPUT,
                vk::DependencyFlags::empty(),
                &[barrier],
                &[],
                &[],
            );
        }
    }

    pub fn render(&mut self, gpu: &mut Gpu, f: &FrameInfo) {
        unsafe {
            let Some((cmd, image)) = gpu.begin_frame() else {
                return;
            };
            if self.pass_version != gpu.pass_version {
                // The main pass changed (anti-aliasing): its pipelines are made again.
                let d = &gpu.device;
                for p in [
                    self.sky_pipe,
                    self.world_pipe,
                    self.water_pipe,
                    self.player_fade_pipe,
                    self.overlay_pipe,
                    self.line_pipe,
                    self.ui_pipe,
                ] {
                    d.destroy_pipeline(p, None);
                }
                [
                    self.sky_pipe,
                    self.world_pipe,
                    self.water_pipe,
                    self.player_fade_pipe,
                    self.overlay_pipe,
                    self.line_pipe,
                    self.ui_pipe,
                ] = create_main_pipes(
                    d,
                    gpu.render_pass,
                    gpu.samples,
                    self.world_layout,
                    self.ui_layout,
                );
                d.destroy_pipeline(self.lens_pipe, None);
                self.lens_pipe = create_lens_pipe(d, gpu.render_pass, gpu.samples, self.lens_layout);
                self.pass_version = gpu.pass_version;
            }
            // begin_frame waited for the frame recorded FRAMES_IN_FLIGHT frames ago, so chunk
            // memory retired before that frame was recorded is no longer read.
            let this_frame = self.frame;
            self.frame += 1;
            if let Some(done) = (this_frame + 1).checked_sub(FRAMES_IN_FLIGHT as u64) {
                self.arena.collect(gpu, done);
            }
            let slot = gpu.frame_slot;
            // Indirect draw commands of this frame used so far (shadow pass, then world).
            let mut indirect_used = 0usize;
            // This slot's previous frame has finished (begin_frame waited for it): read its
            // timestamps, then reuse them for this frame.
            let q0 = 4 * slot as u32;
            if let Some((pool, period)) = self.queries {
                if self.queries_written[slot] {
                    let mut t = [0u64; 4];
                    if gpu
                        .device
                        .get_query_pool_results(pool, q0, &mut t, vk::QueryResultFlags::TYPE_64)
                        .is_ok()
                    {
                        let ms = |a: u64, b: u64| b.saturating_sub(a) as f32 * period / 1e6;
                        self.gpu_ms = Some([ms(t[0], t[1]), ms(t[1], t[2]), ms(t[2], t[3])]);
                    }
                }
                gpu.device.cmd_reset_query_pool(cmd, pool, q0, 4);
                gpu.device
                    .cmd_write_timestamp(cmd, vk::PipelineStageFlags::TOP_OF_PIPE, pool, q0);
                self.queries_written[slot] = true;
            }
            let queries = self.queries;
            let stamp = |d: &ash::Device, i: u32| {
                if let Some((pool, _)) = queries {
                    d.cmd_write_timestamp(
                        cmd,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        pool,
                        q0 + i,
                    );
                }
            };
            let clock_start = std::time::Instant::now();
            self.flush_uploads(gpu, cmd);
            self.flush_layer_uploads(gpu, cmd);
            let clock_uploaded = std::time::Instant::now();
            let mut marks = [clock_uploaded; 4];

            self.ubos[slot].write(0, std::slice::from_ref(&f.ubo));
            let n_ui = f.ui.len().min(UI_MAX_VERTS);
            if n_ui > 0 {
                self.ui_bufs[slot].write(0, &f.ui[..n_ui]);
            }

            // Dynamic geometry: particles | overlay | viewmodel | outline lines | entities | flames
            let mut lines: Vec<Vertex> = Vec::new();
            if let Some((lo, hi)) = f.outline {
                let e = 0.004;
                let lo = lo - Vec3::splat(e);
                let hi = hi + Vec3::splat(e);
                let c = |x: bool, y: bool, z: bool| {
                    [
                        if x { hi.x } else { lo.x },
                        if y { hi.y } else { lo.y },
                        if z { hi.z } else { lo.z },
                    ]
                };
                for a in [false, true] {
                    for b in [false, true] {
                        for pos in [
                            c(false, a, b),
                            c(true, a, b),
                            c(a, false, b),
                            c(a, true, b),
                            c(a, b, false),
                            c(a, b, true),
                        ] {
                            lines.push(Vertex {
                                pos,
                                layer: -1.0,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            let ranges = [
                f.particles,
                f.overlay,
                f.viewmodel,
                lines.as_slice(),
                f.entity,
                f.translucent,
                f.viewmodel_glass,
                f.lens,
            ];
            let mut offsets = [0u32; 9];
            let mut cursor = 0usize;
            for (i, r) in ranges.iter().enumerate() {
                let n = r.len().min(DYN_MAX_VERTS - cursor);
                if n > 0 {
                    self.dyn_bufs[slot].write(cursor * size_of::<Vertex>(), &r[..n]);
                }
                offsets[i] = cursor as u32;
                cursor += n;
            }
            offsets[8] = cursor as u32;
            let range = |i: usize| (offsets[i], offsets[i + 1] - offsets[i]);

            let d = &gpu.device;
            let stages = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            let world_set = self.world_sets[slot];
            let push = |vp: Mat4, pass: f32| DrawPush {
                view_proj: vp.to_cols_array(),
                params: [pass, 0.0, 0.0, 0.0],
            };

            marks[0] = std::time::Instant::now();
            // ---- Shadow pass (the sun's, and the weapon lights' under it)
            let area = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: SHADOW_SIZE,
                    height: SHADOW_HEIGHT,
                },
            };
            let sun_area = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                },
            };
            let clear = [vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            }];
            d.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.shadow_pass)
                    .framebuffer(self.shadow_fb)
                    .render_area(area)
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            if f.shadows {
                d.cmd_set_viewport(
                    cmd,
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: SHADOW_SIZE as f32,
                        height: SHADOW_SIZE as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
                d.cmd_set_scissor(cmd, 0, &[sun_area]);
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.shadow_pipe);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.world_layout,
                    0,
                    &[world_set],
                    &[],
                );
                d.cmd_push_constants(
                    cmd,
                    self.world_layout,
                    stages,
                    0,
                    as_bytes(&push(f.light_view_proj, 3.0)),
                );
                let lf = Frustum::new(f.light_view_proj);
                let sd = f.shadow_distance + 24.0;
                // Only the faces turned toward the light make the shadow map (the others lie
                // behind them), and not the faces between leaves.
                let l = Vec3::from_slice(&f.ubo.light_dir[..3]);
                let lit: [bool; 6] = std::array::from_fn(|k| {
                    let n = crate::world::mesh::FACE_N[k];
                    Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32).dot(l) > 0.0
                });
                let mut draws: Vec<IndirectDraw> = Vec::new();
                let mut shadow_chunks = Vec::new();
                // Only the chunks around the camera can be in range (not all loaded ones).
                let reach = (sd / 16.0).ceil() as i32 + 1;
                let (ccx, ccz) = (
                    (f.cam_pos.x / 16.0).floor() as i32,
                    (f.cam_pos.z / 16.0).floor() as i32,
                );
                let around = (-reach..=reach)
                    .flat_map(|dz| (-reach..=reach).map(move |dx| (ccx + dx, ccz + dz)))
                    .filter_map(|p| self.chunks.get(&p));
                for c in around {
                    let Some(r) = c.mesh else { continue };
                    if c.opaque == 0 {
                        continue;
                    }
                    let center = (c.min + c.max) * 0.5;
                    if (center.x - f.cam_pos.x).abs() > sd
                        || (center.z - f.cam_pos.z).abs() > sd
                        || !lf.visible(c.min, c.max)
                    {
                        continue;
                    }
                    let (b, v, i) = (self.arena.buffer(r), r.offset, r.offset + c.index_offset);
                    shadow_chunks.push((b, v, i, c.opaque));
                    let dirs_total: u32 = c.dirs.iter().sum();
                    let mut at = c.solid - dirs_total;
                    draws.push(chunk_draw(b, v, i, 0, at));
                    for (k, &count) in c.dirs.iter().enumerate() {
                        if lit[k] && count > 0 {
                            draws.push(chunk_draw(b, v, i, at, count));
                        }
                        at += count;
                    }
                    let plants = c.solid + c.leaf_inner;
                    draws.push(chunk_draw(b, v, i, plants, c.opaque - plants));
                }
                draws.retain(|(_, c)| c.index_count > 0);
                let ind = &self.indirect[slot];
                let multi = gpu.multi_draw_indirect;
                indirect_used = record_indirect(d, cmd, ind, 0, &mut draws, multi).unwrap_or_else(|| {
                    for &(b, v, i, n) in &shadow_chunks {
                        d.cmd_bind_vertex_buffers(cmd, 0, &[b], &[v]);
                        d.cmd_bind_index_buffer(cmd, b, i, vk::IndexType::UINT32);
                        d.cmd_draw_indexed(cmd, n, 1, 0, 0, 0);
                    }
                    0
                });
                let (e0, en) = range(4);
                if en > 0 {
                    d.cmd_bind_vertex_buffers(cmd, 0, &[self.dyn_bufs[slot].handle], &[0]);
                    d.cmd_draw(cmd, en, 1, e0, 0);
                }
            }
            // Each weapon light's shadow map: the blocks around it, seen from it (glass, grass and
            // flowers left out: the light goes through them), in its square of the strip.
            for k in 0..MAX_SPOTS {
                let at = f.ubo.spots[2 * k];
                if at[3] <= 0.0 {
                    continue;
                }
                let at = Vec3::new(at[0], at[1], at[2]);
                let vp = Mat4::from_cols_array(&f.ubo.spot_view_proj[k]);
                let tile = vk::Rect2D {
                    offset: vk::Offset2D { x: (k as u32 * SPOT_SHADOW) as i32, y: SHADOW_SIZE as i32 },
                    extent: vk::Extent2D { width: SPOT_SHADOW, height: SPOT_SHADOW },
                };
                d.cmd_set_viewport(
                    cmd,
                    0,
                    &[vk::Viewport {
                        x: tile.offset.x as f32,
                        y: tile.offset.y as f32,
                        width: SPOT_SHADOW as f32,
                        height: SPOT_SHADOW as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
                d.cmd_set_scissor(cmd, 0, &[tile]);
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.shadow_pipe);
                d.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.world_layout, 0, &[world_set], &[]);
                d.cmd_push_constants(cmd, self.world_layout, stages, 0, as_bytes(&push(vp, 5.0)));
                let seen = Frustum::new(vp);
                let reach = (SPOT_REACH / 16.0).ceil() as i32 + 1;
                let (ccx, ccz) = ((at.x / 16.0).floor() as i32, (at.z / 16.0).floor() as i32);
                for dz in -reach..=reach {
                    for dx in -reach..=reach {
                        let Some(c) = self.chunks.get(&(ccx + dx, ccz + dz)) else { continue };
                        let Some(r) = c.mesh else { continue };
                        // Only the blocks (and the outside of leaves): grass and flowers cast no
                        // shadow of the light, nor do the faces inside leaves.
                        if c.solid == 0 || !seen.visible(c.min, c.max) {
                            continue;
                        }
                        let b = self.arena.buffer(r);
                        d.cmd_bind_vertex_buffers(cmd, 0, &[b], &[r.offset]);
                        d.cmd_bind_index_buffer(cmd, b, r.offset + c.index_offset, vk::IndexType::UINT32);
                        d.cmd_draw_indexed(cmd, c.solid, 1, 0, 0, 0);
                    }
                }
            }
            d.cmd_end_render_pass(cmd);
            stamp(d, 1);

            // ---- Scope pass: the magnified view, for the eyepiece
            if let Some(sv) = &f.scope {
                self.scope_ubos[slot].write(0, std::slice::from_ref(&sv.ubo));
                let scope_set = self.scope_sets[slot];
                let area = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: SCOPE_SIZE,
                        height: SCOPE_SIZE,
                    },
                };
                let clears = [
                    vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    },
                    vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        },
                    },
                ];
                d.cmd_begin_render_pass(
                    cmd,
                    &vk::RenderPassBeginInfo::default()
                        .render_pass(self.scope_pass)
                        .framebuffer(self.scope_fb)
                        .render_area(area)
                        .clear_values(&clears),
                    vk::SubpassContents::INLINE,
                );
                d.cmd_set_viewport(
                    cmd,
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: SCOPE_SIZE as f32,
                        height: SCOPE_SIZE as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
                d.cmd_set_scissor(cmd, 0, &[area]);
                let [sky, world, water, _, overlay, ..] = self.scope_pipes;
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, sky);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.world_layout,
                    0,
                    &[scope_set],
                    &[],
                );
                d.cmd_push_constants(cmd, self.world_layout, stages, 0, as_bytes(&push(sv.view_proj, 0.0)));
                d.cmd_draw(cmd, 3, 1, 0, 0);
                // The chunks in its narrow view, with the detail its magnification shows.
                let frustum = Frustum::new(sv.view_proj);
                let max_d = f.view_distance + 24.0;
                let mut draws: Vec<IndirectDraw> = Vec::new();
                let mut waters = Vec::new();
                for c in self.chunks.values() {
                    let Some(r) = c.mesh else { continue };
                    let center = (c.min + c.max) * 0.5;
                    let (dx, dz) = (center.x - sv.cam_pos.x, center.z - sv.cam_pos.z);
                    let dist2 = dx * dx + dz * dz;
                    if dist2 > max_d * max_d || !frustum.visible(c.min, c.max) {
                        continue;
                    }
                    let near = sv.cam_pos.clamp(c.min, c.max);
                    let block_px = sv.detail_px / near.distance(sv.cam_pos).max(1e-3);
                    let drawn = if block_px >= 5.0 {
                        c.opaque
                    } else if block_px >= 1.5 {
                        c.solid + c.leaf_inner
                    } else {
                        c.solid
                    };
                    let (b, v, i) = (self.arena.buffer(r), r.offset, r.offset + c.index_offset);
                    if drawn > 0 {
                        draws.push(chunk_draw(b, v, i, 0, drawn));
                    }
                    if c.water > 0 {
                        waters.push((b, v, i, c.opaque, c.water, dist2));
                    }
                }
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, world);
                let ind = &self.indirect[slot];
                match record_indirect(d, cmd, ind, indirect_used, &mut draws, gpu.multi_draw_indirect) {
                    Some(next) => indirect_used = next,
                    None => {
                        for (b, c) in &draws {
                            d.cmd_bind_vertex_buffers(cmd, 0, &[*b], &[0]);
                            d.cmd_bind_index_buffer(cmd, *b, 0, vk::IndexType::UINT32);
                            d.cmd_draw_indexed(cmd, c.index_count, 1, c.first_index, c.vertex_offset, 0);
                        }
                    }
                }
                let dynb = self.dyn_bufs[slot].handle;
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                let (p0, pn) = range(0);
                if pn > 0 {
                    d.cmd_draw(cmd, pn, 1, p0, 0);
                }
                let (e0, en) = range(4);
                if f.entity_visible && en > 0 {
                    d.cmd_draw(cmd, en, 1, e0, 0);
                }
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, water);
                d.cmd_push_constants(cmd, self.world_layout, stages, 0, as_bytes(&push(sv.view_proj, 1.0)));
                waters.sort_unstable_by(|a, b| b.5.total_cmp(&a.5));
                for &(b, v, i, first, count, _) in &waters {
                    d.cmd_bind_vertex_buffers(cmd, 0, &[b], &[v]);
                    d.cmd_bind_index_buffer(cmd, b, i, vk::IndexType::UINT32);
                    d.cmd_draw_indexed(cmd, count, 1, first, 0, 0);
                }
                let (t0, tn) = range(5);
                if tn > 0 {
                    d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                    d.cmd_draw(cmd, tn, 1, t0, 0);
                }
                // Bullet holes and break cracks on the blocks.
                let (o0, on) = range(1);
                if on > 0 {
                    d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, overlay);
                    d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                    d.cmd_draw(cmd, on, 1, o0, 0);
                }
                d.cmd_end_render_pass(cmd);
            }

            // ---- Main pass
            gpu.begin_render_pass(cmd, image, [0.0, 0.0, 0.0, 1.0]);
            let d = &gpu.device;
            let ext = gpu.extent;

            // Sky
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipe);
            d.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.world_layout,
                0,
                &[world_set],
                &[],
            );
            d.cmd_push_constants(
                cmd,
                self.world_layout,
                stages,
                0,
                as_bytes(&push(f.view_proj, 0.0)),
            );
            d.cmd_draw(cmd, 3, 1, 0, 0);

            marks[1] = std::time::Instant::now();
            // Visible chunks
            let frustum = Frustum::new(f.view_proj);
            let max_d = f.view_distance + 24.0;
            let mut visible: Vec<VisibleChunk> = Vec::with_capacity(self.drawn_chunks + 64);
            for c in self.chunks.values() {
                let Some(r) = c.mesh else { continue };
                let center = (c.min + c.max) * 0.5;
                let (dx, dz) = (center.x - f.cam_pos.x, center.z - f.cam_pos.z);
                let dist2 = dx * dx + dz * dz;
                if dist2 > max_d * max_d || !frustum.visible(c.min, c.max) {
                    continue;
                }
                // Far chunks leave out what the shaders would drop anyway: grass and flowers
                // (world.vert, under ~5 pixels a block) and the faces between leaves (closed
                // crowns, under ~1.5).
                let near = f.cam_pos.clamp(c.min, c.max);
                let block_px = f.detail_px / near.distance(f.cam_pos).max(1e-3);
                let drawn = if block_px >= 5.0 {
                    c.opaque
                } else if block_px >= 1.5 {
                    c.solid + c.leaf_inner
                } else {
                    c.solid
                };
                // Whole-block faces turned away from the camera are left out by direction:
                // +X faces only show from the +X side of the chunk's west edge, and so on.
                let facing = [
                    f.cam_pos.x > c.min.x,
                    f.cam_pos.x < c.max.x,
                    f.cam_pos.y > c.min.y,
                    f.cam_pos.y < c.max.y,
                    f.cam_pos.z > c.min.z,
                    f.cam_pos.z < c.max.z,
                ];
                let dirs_total: u32 = c.dirs.iter().sum();
                let mut parts = [(0u32, 0u32); 4];
                let mut n = 0;
                let mut push = |first: u32, count: u32| {
                    if count == 0 {
                        return;
                    }
                    if n > 0 && parts[n - 1].0 + parts[n - 1].1 == first {
                        parts[n - 1].1 += count;
                    } else if n < parts.len() {
                        parts[n] = (first, count);
                        n += 1;
                    } else {
                        // Out of slots: draw on to the end of this one.
                        parts[n - 1].1 = first + count - parts[n - 1].0;
                    }
                };
                let mut at = c.solid - dirs_total;
                push(0, at);
                for (d, &count) in c.dirs.iter().enumerate() {
                    if facing[d] {
                        push(at, count);
                    }
                    at += count;
                }
                push(c.solid, drawn - c.solid);
                visible.push(VisibleChunk {
                    parts,
                    buffer: self.arena.buffer(r),
                    vertices: r.offset,
                    indices: r.offset + c.index_offset,
                    opaque: c.opaque,
                    water: c.water,
                    dist2,
                });
            }
            marks[2] = std::time::Instant::now();
            self.drawn_chunks = visible.len();
            // By mesh buffer, then front to back: the world's indirect draws come out grouped
            // per buffer already (the sort in `record_indirect` then only confirms it).
            visible.sort_unstable_by(|a, b| {
                vk::Handle::as_raw(a.buffer)
                    .cmp(&vk::Handle::as_raw(b.buffer))
                    .then(a.dist2.total_cmp(&b.dist2))
            });
            let bind = |c: &VisibleChunk| {
                d.cmd_bind_vertex_buffers(cmd, 0, &[c.buffer], &[c.vertices]);
                d.cmd_bind_index_buffer(cmd, c.buffer, c.indices, vk::IndexType::UINT32);
            };

            // Opaque (front to back)
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.world_pipe);
            d.cmd_push_constants(
                cmd,
                self.world_layout,
                stages,
                0,
                as_bytes(&push(f.view_proj, 0.0)),
            );
            // All chunks' opaque parts as indirect draws, one command per mesh buffer (the
            // meshes share a few big buffers).
            let mut draws: Vec<IndirectDraw> = Vec::with_capacity(visible.len() * 3);
            for c in &visible {
                for &(first, count) in c.parts.iter().filter(|p| p.1 > 0) {
                    draws.push(chunk_draw(c.buffer, c.vertices, c.indices, first, count));
                }
            }
            let ind = &self.indirect[slot];
            let multi = gpu.multi_draw_indirect;
            // Sorted by buffer, front to back within each (the sort is stable).
            let recorded = record_indirect(d, cmd, ind, indirect_used, &mut draws, multi);
            marks[3] = std::time::Instant::now();
            if recorded.is_none() {
                for c in &visible {
                    bind(c);
                    for &(first, count) in c.parts.iter().filter(|p| p.1 > 0) {
                        d.cmd_draw_indexed(cmd, count, 1, first, 0, 0);
                    }
                }
            }
            let dynb = self.dyn_bufs[slot].handle;
            let (p0, pn) = range(0);
            if pn > 0 {
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                d.cmd_draw(cmd, pn, 1, p0, 0);
            }
            let (e0, en) = range(4);
            let player_n = f.player_vertex_count.min(en);
            if f.entity_visible && en > 0 {
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                if en > player_n {
                    d.cmd_draw(cmd, en - player_n, 1, e0 + player_n, 0);
                }
                if player_n > 0 && f.player_opacity >= 0.999 {
                    d.cmd_draw(cmd, player_n, 1, e0, 0);
                }
            }

            // Outline
            let (l0, ln) = range(3);
            if ln > 0 {
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.line_pipe);
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                d.cmd_draw(cmd, ln, 1, l0, 0);
            }

            // Water (back to front)
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.water_pipe);
            d.cmd_push_constants(
                cmd,
                self.world_layout,
                stages,
                0,
                as_bytes(&push(f.view_proj, 1.0)),
            );
            // Water back to front.
            let mut water: Vec<&VisibleChunk> = visible.iter().filter(|c| c.water > 0).collect();
            water.sort_unstable_by(|a, b| b.dist2.total_cmp(&a.dist2));
            for c in water {
                bind(c);
                d.cmd_draw_indexed(cmd, c.water, 1, c.opaque, 0, 0);
            }
            let (t0, tn) = range(5);
            if tn > 0 {
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                d.cmd_draw(cmd, tn, 1, t0, 0);
            }

            // Break cracks
            let (o0, on) = range(1);
            if on > 0 {
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.overlay_pipe);
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                d.cmd_draw(cmd, on, 1, o0, 0);
            }

            if f.entity_visible && player_n > 0 && (0.01..0.999).contains(&f.player_opacity) {
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.player_fade_pipe);
                let mut fade = push(f.view_proj, 0.0);
                fade.params[1] = f.player_opacity;
                d.cmd_push_constants(cmd, self.world_layout, stages, 0, as_bytes(&fade));
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                d.cmd_draw(cmd, player_n, 1, e0, 0);
            }

            // First-person hand: clear depth so it never clips into walls
            let (v0, vn) = range(2);
            if vn > 0 {
                let vm_flame = if f
                    .viewmodel
                    .last()
                    .is_some_and(|v| v.layer == crate::world::textures::tex::TORCH_FLAME as f32)
                {
                    vn.min(24)
                } else {
                    0
                };
                d.cmd_clear_attachments(
                    cmd,
                    &[vk::ClearAttachment {
                        aspect_mask: vk::ImageAspectFlags::DEPTH,
                        color_attachment: 0,
                        clear_value: vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue {
                                depth: 1.0,
                                stencil: 0,
                            },
                        },
                    }],
                    &[vk::ClearRect {
                        rect: vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent: ext,
                        },
                        base_array_layer: 0,
                        layer_count: 1,
                    }],
                );
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.world_pipe);
                d.cmd_push_constants(
                    cmd,
                    self.world_layout,
                    stages,
                    0,
                    as_bytes(&push(f.vm_view_proj, 2.0)),
                );
                d.cmd_bind_vertex_buffers(cmd, 0, &[dynb], &[0]);
                d.cmd_draw(cmd, vn - vm_flame, 1, v0, 0);
                if vm_flame > 0 {
                    d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.water_pipe);
                    d.cmd_draw(cmd, vm_flame, 1, v0 + vn - vm_flame, 0);
                }
                // The scope's eyepiece shows its view; the glass is drawn over what is behind.
                let (n0, nn) = range(7);
                if nn > 0 && f.scope.is_some() {
                    d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.lens_pipe);
                    d.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.lens_layout,
                        0,
                        &[world_set, self.lens_set],
                        &[],
                    );
                    d.cmd_push_constants(
                        cmd,
                        self.lens_layout,
                        stages,
                        0,
                        as_bytes(&push(f.vm_view_proj, 2.0)),
                    );
                    d.cmd_draw(cmd, nn, 1, n0, 0);
                    d.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.world_layout,
                        0,
                        &[world_set],
                        &[],
                    );
                }
                let (g0, gn) = range(6);
                if gn > 0 {
                    d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.water_pipe);
                    d.cmd_push_constants(
                        cmd,
                        self.world_layout,
                        stages,
                        0,
                        as_bytes(&push(f.vm_view_proj, 4.0)),
                    );
                    d.cmd_draw(cmd, gn, 1, g0, 0);
                }
            }

            stamp(d, 2);
            // UI
            if n_ui > 0 {
                let screen = [ext.width as f32, ext.height as f32, 0.0f32, 0.0];
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.ui_pipe);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.ui_layout,
                    0,
                    &[self.ui_set],
                    &[],
                );
                d.cmd_push_constants(cmd, self.ui_layout, stages, 0, as_bytes(&screen));
                d.cmd_bind_vertex_buffers(cmd, 0, &[self.ui_bufs[slot].handle], &[0]);
                let full = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: ext,
                };
                for (i, &(start, clip)) in f.ui_clips.iter().enumerate() {
                    let end = f
                        .ui_clips
                        .get(i + 1)
                        .map_or(n_ui as u32, |c| c.0)
                        .min(n_ui as u32);
                    if end <= start {
                        continue;
                    }
                    let rect = match clip {
                        Some([x, y, w, h]) => {
                            let x0 = x.floor().clamp(0.0, ext.width as f32);
                            let y0 = y.floor().clamp(0.0, ext.height as f32);
                            let x1 = (x + w).ceil().clamp(x0, ext.width as f32);
                            let y1 = (y + h).ceil().clamp(y0, ext.height as f32);
                            vk::Rect2D {
                                offset: vk::Offset2D {
                                    x: x0 as i32,
                                    y: y0 as i32,
                                },
                                extent: vk::Extent2D {
                                    width: (x1 - x0) as u32,
                                    height: (y1 - y0) as u32,
                                },
                            }
                        }
                        None => full,
                    };
                    d.cmd_set_scissor(cmd, 0, &[rect]);
                    d.cmd_draw(cmd, end - start, 1, start, 0);
                }
                d.cmd_set_scissor(cmd, 0, &[full]);
            }

            stamp(d, 3);
            let clock_recorded = std::time::Instant::now();
            gpu.end_frame(cmd, image);
            let el = |a: std::time::Instant, b: std::time::Instant| (b - a).as_secs_f32() * 1000.0;
            self.rec_detail = [
                el(clock_uploaded, marks[0]),
                el(marks[0], marks[1]),
                el(marks[1], marks[2]),
                el(marks[2], marks[3]),
                el(marks[3], clock_recorded),
            ];
            self.cpu_detail = [
                el(clock_start, clock_uploaded),
                el(clock_uploaded, clock_recorded),
                el(clock_recorded, std::time::Instant::now()),
            ];
        }
    }

    pub fn destroy(&mut self, gpu: &mut Gpu) {
        unsafe {
            let d = &gpu.device;
            d.device_wait_idle().ok();
            if let Some((pool, _)) = self.queries {
                d.destroy_query_pool(pool, None);
            }
            self.chunks.clear();
            self.arena.destroy(d);
            for b in self
                .ui_bufs
                .drain(..)
                .chain(self.dyn_bufs.drain(..))
                .chain(self.indirect.drain(..))
                .chain(self.staging.drain(..))
                .chain(self.ubos.drain(..))
                .chain(self.scope_ubos.drain(..))
            {
                b.destroy(d);
            }
            for p in [
                self.sky_pipe,
                self.world_pipe,
                self.water_pipe,
                self.player_fade_pipe,
                self.overlay_pipe,
                self.line_pipe,
                self.shadow_pipe,
                self.ui_pipe,
                self.lens_pipe,
            ]
            .into_iter()
            .chain(self.scope_pipes)
            {
                d.destroy_pipeline(p, None);
            }
            d.destroy_pipeline_layout(self.lens_layout, None);
            d.destroy_descriptor_set_layout(self.lens_dsl, None);
            d.destroy_framebuffer(self.scope_fb, None);
            d.destroy_render_pass(self.scope_pass, None);
            d.destroy_sampler(self.scope_sampler, None);
            self.scope_color.destroy(d);
            self.scope_depth.destroy(d);
            d.destroy_pipeline_layout(self.world_layout, None);
            d.destroy_pipeline_layout(self.ui_layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.world_dsl, None);
            d.destroy_descriptor_set_layout(self.ui_dsl, None);
            d.destroy_framebuffer(self.shadow_fb, None);
            d.destroy_render_pass(self.shadow_pass, None);
            d.destroy_sampler(self.shadow_sampler, None);
            self.shadow_image.destroy(d);
            self.block_tex.destroy(d);
            self.font_tex.destroy(d);
        }
    }
}
