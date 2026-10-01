//! The frame: shadow map, sky, chunk meshes, dynamic geometry (entities, particles, the
//! hand) and the UI, drawn with Vulkan.

mod arena;
pub(crate) mod chunks;
mod cull;
mod descriptors;
mod dynamic;
mod frame;
mod passes;
mod pipelines;
mod targets;
mod textures;
mod timing;

pub use frame::{FrameInfo, FrameUbo, ScopeView, MAX_HELD_LIGHTS, MAX_SPOTS, SCOPE_SIZE, SHADOW_SIZE, SPOT_REACH};

use crate::engine::pipeline::create_layout;
use crate::engine::{Buffer, Gpu, SamplerKind, Texture, FRAMES_IN_FLIGHT};
use crate::ui::{UiVertex, FONT_TEX_H, FONT_TEX_W};
use crate::world::mesh::{MeshData, Vertex};
use crate::world::{ChunkPos, FastMap};
use ash::vk;
use chunks::{ChunkGpu, STAGING_SIZE};
use descriptors::Descriptors;
use dynamic::{DYN_MAX_VERTS, LENS};
use passes::{Rec, MAX_INDIRECT};
use pipelines::{create_blur_across_pipe, create_scope_view_pipe, create_shadow_pipe, DrawPush, MainPipes, ScenePipes, BLUR_FRAG, LENS_FRAG};
use std::collections::VecDeque;
use std::mem::size_of;
use std::time::Instant;
use targets::{BlurTarget, ScopeTarget, ShadowTarget};

const UI_MAX_VERTS: usize = 150_000;

pub struct Renderer {
    desc: Descriptors,
    world_layout: vk::PipelineLayout,
    ui_layout: vk::PipelineLayout,
    /// The main pass's pipelines.
    pipes: MainPipes,
    /// The shadow map's pipelines: with alpha tests, and without for plain whole-block faces.
    /// Shadow maps: the world's other things (entities...), and the chunks' faces (alpha
    /// tested, and the plain whole-block ones).
    shadow_pipe: vk::Pipeline,
    shadow_chunk_pipe: vk::Pipeline,
    shadow_plain_pipe: vk::Pipeline,
    /// `Gpu::pass_version` the main-pass pipelines were made for.
    pass_version: u64,
    block_tex: Texture,
    /// Mip levels of the block texture.
    block_mips: usize,
    /// Layers of the block texture replaced while the game runs (the guide book's pages): the
    /// first layer, how many, and each mip level of them (layer after layer).
    layer_uploads: VecDeque<(u32, u32, Vec<Vec<u8>>)>,
    font_tex: Texture,
    shadow: ShadowTarget,
    /// The shadow map has been through its pass once (so it is in the layout the shaders
    /// read it in): with no shadows and no weapon lights the pass is then left out.
    shadow_ready: bool,
    /// The scope's view (picture in picture): its images, pass and framebuffer, the pipelines
    /// it is drawn with (like the main pass's), its uniform buffers per frame slot (its
    /// descriptor sets are `desc.scope_sets`); and the eyepiece's pipeline, which shows it on
    /// the gun, and the menus' blurred backdrop's.
    scope: ScopeTarget,
    scope_pipes: ScenePipes,
    scope_ubos: Vec<Buffer>,
    lens_layout: vk::PipelineLayout,
    lens_pipe: vk::Pipeline,
    blur_pipe: vk::Pipeline,
    /// The backdrop's blur across (its first half), into `blur`.
    blur: BlurTarget,
    blur_across_pipe: vk::Pipeline,
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
    /// Chunk meshes uploaded last frame.
    pub uploaded: u32,
}

/// One host-visible buffer of `size` bytes per frame slot.
fn per_slot(gpu: &Gpu, size: usize, usage: vk::BufferUsageFlags) -> Vec<Buffer> {
    let host = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
    (0..FRAMES_IN_FLIGHT)
        .map(|_| Buffer::new(gpu, size as u64, usage, host))
        .collect()
}

impl Renderer {
    pub fn new(gpu: &Gpu, block_levels: &[Vec<u8>], font_atlas: &[u8]) -> Self {
        assert_eq!(size_of::<Vertex>(), 32);
        assert_eq!(size_of::<UiVertex>(), 52);
        assert_eq!(size_of::<FrameUbo>(), 304 + 16 * MAX_HELD_LIGHTS + 32 * MAX_SPOTS + 16 + 64 * MAX_SPOTS);

        // (as many layers as given: the start-up screen's few, then all)
        let block_tex = textures::block_texture(gpu, block_levels);
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
            let shadow = ShadowTarget::new(gpu);
            let scope = ScopeTarget::new(gpu);
            let blur = BlurTarget::new(gpu);

            let desc = Descriptors::new(d);
            let ubo_size = size_of::<FrameUbo>();
            let ubos = per_slot(gpu, ubo_size, vk::BufferUsageFlags::UNIFORM_BUFFER);
            let scope_ubos = per_slot(gpu, ubo_size, vk::BufferUsageFlags::UNIFORM_BUFFER);
            desc.write(d, &block_tex, &font_tex, &shadow, &scope, &blur, &ubos, &scope_ubos);

            // Pipelines
            let world_layout = create_layout(d, &[desc.world_dsl], size_of::<DrawPush>() as u32);
            let ui_layout = create_layout(d, &[desc.ui_dsl], 16);
            let pipes = MainPipes::new(d, gpu.render_pass, gpu.samples, world_layout, ui_layout);
            let scope_pipes = ScenePipes::new(d, scope.pass, vk::SampleCountFlags::TYPE_1, world_layout);
            let lens_layout = create_layout(d, &[desc.world_dsl, desc.lens_dsl], size_of::<DrawPush>() as u32);
            let lens_pipe = create_scope_view_pipe(d, gpu.render_pass, gpu.samples, lens_layout, LENS_FRAG);
            let blur_pipe = create_scope_view_pipe(d, gpu.render_pass, gpu.samples, lens_layout, BLUR_FRAG);
            let blur_across_pipe = create_blur_across_pipe(d, blur.pass, lens_layout);
            let shadow_pipe = create_shadow_pipe(d, shadow.pass, world_layout, false, false);
            let shadow_chunk_pipe = create_shadow_pipe(d, shadow.pass, world_layout, false, true);
            let shadow_plain_pipe = create_shadow_pipe(d, shadow.pass, world_layout, true, true);

            Self {
                desc,
                world_layout,
                ui_layout,
                pipes,
                shadow_pipe,
                shadow_chunk_pipe,
                shadow_plain_pipe,
                pass_version: gpu.pass_version,
                block_tex,
                block_mips: block_levels.len(),
                layer_uploads: VecDeque::new(),
                font_tex,
                shadow,
                shadow_ready: false,
                scope,
                scope_pipes,
                scope_ubos,
                lens_layout,
                lens_pipe,
                blur_pipe,
                blur,
                blur_across_pipe,
                ubos,
                ui_bufs: per_slot(gpu, UI_MAX_VERTS * size_of::<UiVertex>(), vk::BufferUsageFlags::VERTEX_BUFFER),
                dyn_bufs: per_slot(gpu, DYN_MAX_VERTS * size_of::<Vertex>(), vk::BufferUsageFlags::VERTEX_BUFFER),
                indirect: per_slot(
                    gpu,
                    MAX_INDIRECT * size_of::<vk::DrawIndexedIndirectCommand>(),
                    vk::BufferUsageFlags::INDIRECT_BUFFER,
                ),
                staging: per_slot(gpu, STAGING_SIZE, vk::BufferUsageFlags::TRANSFER_SRC),
                chunks: FastMap::default(),
                arena: arena::Arena::default(),
                frame: 0,
                pending: VecDeque::new(),
                drawn_chunks: 0,
                queries: Self::create_query_pool(gpu),
                queries_written: [false; FRAMES_IN_FLIGHT],
                gpu_ms: None,
                cpu_detail: [0.0; 3],
                rec_detail: [0.0; 5],
                uploaded: 0,
            }
        }
    }

    pub fn render(&mut self, gpu: &mut Gpu, f: &FrameInfo) {
        unsafe {
            let Some((cmd, image)) = gpu.begin_frame() else {
                return;
            };
            if self.pass_version != gpu.pass_version {
                self.rebuild_screen_pipes(gpu);
            }
            // begin_frame waited for the frame recorded FRAMES_IN_FLIGHT frames ago, so chunk
            // memory retired before that frame was recorded is no longer read.
            let this_frame = self.frame;
            self.frame += 1;
            if let Some(done) = (this_frame + 1).checked_sub(FRAMES_IN_FLIGHT as u64) {
                self.arena.collect(gpu, done);
            }
            let slot = gpu.frame_slot;
            self.begin_timestamps(gpu, cmd, slot);
            let clock_start = Instant::now();
            self.flush_uploads(gpu, cmd, f.ubo.cam_pos[3]);
            self.flush_layer_uploads(gpu, cmd);
            let clock_uploaded = Instant::now();
            let mut marks = [clock_uploaded; 4];

            self.ubos[slot].write(0, std::slice::from_ref(&f.ubo));
            let n_ui = f.ui.len().min(UI_MAX_VERTS);
            if n_ui > 0 {
                self.ui_bufs[slot].write(0, &f.ui[..n_ui]);
            }
            let r = Rec {
                gpu,
                d: &gpu.device,
                cmd,
                slot,
                dyn_buf: self.dyn_bufs[slot].handle,
                dyn_ranges: dynamic::write(&self.dyn_bufs[slot], f),
            };

            marks[0] = Instant::now();
            // Indirect draw commands of this frame are used by the shadow pass, then the
            // scope's, then the world's.
            // With no sun shadows and no weapon lights nothing reads the shadow map (world.frag
            // checks both first): its pass (clearing 4096x5120 depths) is left out, once it has
            // run to put the image in its layout.
            let spots = (0..MAX_SPOTS).any(|k| f.ubo.spots[2 * k][3] > 0.0);
            let mut indirect_used = 0;
            if f.shadows || spots || !self.shadow_ready {
                indirect_used = self.record_shadow_pass(&r, f);
                self.shadow_ready = true;
            }
            self.stamp(&r, 1);
            if let Some(sv) = &f.scope {
                indirect_used = self.record_scope_pass(&r, f, sv, indirect_used);
            }
            // Under a menu's blurred backdrop (which covers all of it) the world is not drawn
            // again: the scope pass has drawn it for the blur, blurred across here.
            let blurred = f.backdrop_blur && f.scope.is_some() && r.range(LENS).1 > 0;
            if blurred {
                self.record_blur_across(&r);
            }

            gpu.begin_render_pass(cmd, image, [0.0, 0.0, 0.0, 1.0]);
            if !blurred {
                self.record_world(&r, f, indirect_used, &mut marks);
            }
            self.record_backdrop(&r, f);
            self.stamp(&r, 2);
            self.record_ui(&r, f, n_ui);
            self.stamp(&r, 3);

            let clock_recorded = Instant::now();
            gpu.end_frame(cmd, image);
            let el = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
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
                el(clock_recorded, Instant::now()),
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
            self.pipes.destroy(d);
            self.scope_pipes.destroy(d);
            for p in [self.shadow_pipe, self.shadow_chunk_pipe, self.shadow_plain_pipe, self.lens_pipe, self.blur_pipe, self.blur_across_pipe] {
                d.destroy_pipeline(p, None);
            }
            d.destroy_pipeline_layout(self.lens_layout, None);
            d.destroy_pipeline_layout(self.world_layout, None);
            d.destroy_pipeline_layout(self.ui_layout, None);
            self.scope.destroy(d);
            self.blur.destroy(d);
            self.desc.destroy(d);
            self.shadow.destroy(d);
            self.block_tex.destroy(d);
            self.font_tex.destroy(d);
        }
    }
}
