//! The frame: shadow map, sky, chunk meshes, dynamic geometry (entities, particles, the
//! hand) and the UI, drawn with wgpu.

mod arena;
mod bindings;
pub(crate) mod chunks;
mod cull;
mod dynamic;
mod frame;
mod passes;
mod pipelines;
mod shaders;
mod targets;
mod textures;
mod timing;

pub use frame::{FrameInfo, FrameUbo, ScopeView, MAX_HELD_LIGHTS, MAX_SPOTS, SCOPE_SIZE, SHADOW_SIZE, SPOT_REACH};

use crate::engine::pipeline::create_layout;
use crate::engine::{Buffer, Gpu, SamplerKind, Texture};
use crate::ui::{UiVertex, FONT_TEX_H, FONT_TEX_W};
use crate::world::mesh::{MeshData, Vertex};
use crate::world::{ChunkPos, FastMap};
use bindings::{Groups, Layouts, Sources};
use chunks::ChunkGpu;
use dynamic::{DYN_MAX_VERTS, LENS};
use passes::{Rec, MAX_INDIRECT};
use pipelines::{create_blur_across_pipe, create_scope_view_pipe, create_shadow_pipe, DrawPush, MainPipes, Modules, ScenePipes, BLUR};
use std::collections::VecDeque;
use std::mem::size_of;
use std::time::Instant;
use targets::{BlurTarget, ScopeTarget, ShadowTarget, SCOPE_FORMAT};
use timing::Timestamps;

const UI_MAX_VERTS: usize = 150_000;

pub struct Renderer {
    modules: Modules,
    layouts: Layouts,
    groups: Groups,
    world_layout: wgpu::PipelineLayout,
    ui_layout: wgpu::PipelineLayout,
    /// The main pass's pipelines.
    pipes: MainPipes,
    /// Shadow maps: the world's other things (entities...), and the chunks' faces (alpha
    /// tested, and the plain whole-block ones).
    shadow_pipe: wgpu::RenderPipeline,
    shadow_chunk_pipe: wgpu::RenderPipeline,
    shadow_plain_pipe: wgpu::RenderPipeline,
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
    /// The shadow map has been through its pass once (cleared to the far end): with no
    /// shadows and no weapon lights the pass is then left out.
    shadow_ready: bool,
    /// The scope's view (picture in picture): its images, the pipelines it is drawn with (like
    /// the main pass's), its uniform buffer (its bind group is `groups.scope`); and the
    /// eyepiece's pipeline, which shows it on the gun, and the menus' blurred backdrop's.
    scope: ScopeTarget,
    scope_pipes: ScenePipes,
    scope_ubo: Buffer,
    lens_layout: wgpu::PipelineLayout,
    lens_pipe: wgpu::RenderPipeline,
    blur_pipe: wgpu::RenderPipeline,
    /// The backdrop's blur across (its first half), into `blur`.
    blur: BlurTarget,
    blur_across_pipe: wgpu::RenderPipeline,
    ubo: Buffer,
    ui_buf: Buffer,
    dyn_buf: Buffer,
    /// The chunks' opaque draws as indirect commands (the shadow pass's, the scope's and the
    /// world's one after the other).
    indirect: Buffer,
    chunks: FastMap<ChunkPos, ChunkGpu>,
    arena: arena::Arena,
    /// Frames recorded so far (for giving chunk memory back once no frame uses it).
    frame: u64,
    pending: VecDeque<MeshData>,
    pub drawn_chunks: usize,
    /// GPU timestamps: 4 per frame slot (start, shadows done, world done, end).
    queries: Option<Timestamps>,
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
            wgpu::TextureFormat::R8Unorm,
            &[font_atlas.to_vec()],
            SamplerKind::Font,
        );
        let d = &gpu.device;
        let shadow = ShadowTarget::new(gpu);
        let scope = ScopeTarget::new(gpu);
        let blur = BlurTarget::new(gpu);
        let ubo_size = size_of::<FrameUbo>() as u64;
        let ubo = Buffer::new(gpu, ubo_size, wgpu::BufferUsages::UNIFORM);
        let scope_ubo = Buffer::new(gpu, ubo_size, wgpu::BufferUsages::UNIFORM);
        let layouts = Layouts::new(d);
        let groups = Groups::new(
            d,
            &layouts,
            &Sources { block_tex: &block_tex, font_tex: &font_tex, shadow: &shadow, scope: &scope, blur: &blur, ubo: &ubo, scope_ubo: &scope_ubo },
        );

        // Pipelines
        let mut modules = Modules::new(d);
        let push = size_of::<DrawPush>() as u32;
        let world_layout = create_layout(d, &[&layouts.world], push);
        let ui_layout = create_layout(d, &[&layouts.ui], 16);
        let shadow_layout = create_layout(d, &[&layouts.shadow], push);
        let lens_layout = create_layout(d, &[&layouts.world, &layouts.lens], push);
        let (color, samples) = (gpu.surface_format, gpu.samples);
        let pipes = MainPipes::new(d, &mut modules, color, samples, &world_layout, &ui_layout);
        let scope_pipes = ScenePipes::new(d, &mut modules, SCOPE_FORMAT, 1, &world_layout);
        let lens_pipe = create_scope_view_pipe(d, &mut modules, color, samples, &lens_layout, pipelines::LENS);
        let blur_pipe = create_scope_view_pipe(d, &mut modules, color, samples, &lens_layout, BLUR);
        let blur_across_pipe = create_blur_across_pipe(d, &mut modules, &lens_layout);
        let shadow_pipe = create_shadow_pipe(d, &mut modules, &shadow_layout, false, false);
        let shadow_chunk_pipe = create_shadow_pipe(d, &mut modules, &shadow_layout, false, true);
        let shadow_plain_pipe = create_shadow_pipe(d, &mut modules, &shadow_layout, true, true);

        Self {
            modules,
            layouts,
            groups,
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
            scope_ubo,
            lens_layout,
            lens_pipe,
            blur_pipe,
            blur,
            blur_across_pipe,
            ubo,
            ui_buf: Buffer::new(gpu, (UI_MAX_VERTS * size_of::<UiVertex>()) as u64, wgpu::BufferUsages::VERTEX),
            dyn_buf: Buffer::new(gpu, (DYN_MAX_VERTS * size_of::<Vertex>()) as u64, wgpu::BufferUsages::VERTEX),
            indirect: Buffer::new(
                gpu,
                (MAX_INDIRECT * size_of::<wgpu::util::DrawIndexedIndirectArgs>()) as u64,
                wgpu::BufferUsages::INDIRECT,
            ),
            chunks: FastMap::default(),
            arena: arena::Arena::default(),
            frame: 0,
            pending: VecDeque::new(),
            drawn_chunks: 0,
            queries: Timestamps::new(gpu),
            gpu_ms: None,
            cpu_detail: [0.0; 3],
            rec_detail: [0.0; 5],
            uploaded: 0,
        }
    }

    /// What the bind groups show.
    fn sources(&self) -> Sources<'_> {
        Sources {
            block_tex: &self.block_tex,
            font_tex: &self.font_tex,
            shadow: &self.shadow,
            scope: &self.scope,
            blur: &self.blur,
            ubo: &self.ubo,
            scope_ubo: &self.scope_ubo,
        }
    }

    pub fn render(&mut self, gpu: &mut Gpu, f: &FrameInfo) {
        let Some(mut frame) = gpu.begin_frame() else {
            return;
        };
        if self.pass_version != gpu.pass_version {
            self.rebuild_screen_pipes(gpu);
        }
        // Chunk memory retired FRAMES_IN_FLIGHT frames ago is no longer read (and the writes
        // reusing it come after those frames on the queue anyway).
        let this_frame = self.frame;
        self.frame += 1;
        if let Some(done) = (this_frame + 1).checked_sub(crate::engine::FRAMES_IN_FLIGHT as u64) {
            self.arena.collect(done);
        }
        let slot = gpu.frame_slot;
        self.begin_timestamps(gpu, &mut frame.encoder, slot);
        let clock_start = Instant::now();
        self.flush_uploads(gpu, f.ubo.cam_pos[3]);
        self.flush_layer_uploads(gpu);
        let clock_uploaded = Instant::now();
        let mut marks = [clock_uploaded; 4];

        self.ubo.write(0, std::slice::from_ref(&f.ubo));
        let n_ui = f.ui.len().min(UI_MAX_VERTS);
        if n_ui > 0 {
            self.ui_buf.write(0, &f.ui[..n_ui]);
        }
        let r = Rec {
            gpu,
            dyn_buf: self.dyn_buf.handle.clone(),
            dyn_ranges: dynamic::write(&self.dyn_buf, f),
        };

        marks[0] = Instant::now();
        // Indirect draw commands of this frame are used by the shadow pass, then the
        // scope's, then the world's.
        // With no sun shadows and no weapon lights nothing reads the shadow map (world.wgsl
        // checks both first): its pass (clearing 4096x5120 depths) is left out, once it has
        // run to clear the image.
        let spots = (0..MAX_SPOTS).any(|k| f.ubo.spots[2 * k][3] > 0.0);
        let mut indirect_used = 0;
        if f.shadows || spots || !self.shadow_ready {
            indirect_used = self.record_shadow_pass(&r, &mut frame.encoder, f);
            self.shadow_ready = true;
        }
        self.stamp(&mut frame.encoder, slot, 1);
        if let Some(sv) = &f.scope {
            indirect_used = self.record_scope_pass(&r, &mut frame.encoder, f, sv, indirect_used);
        }
        // Under a menu's blurred backdrop (which covers all of it) the world is not drawn
        // again: the scope pass has drawn it for the blur, blurred across here.
        let blurred = f.backdrop_blur && f.scope.is_some() && r.range(LENS).1 > 0;
        if blurred {
            self.record_blur_across(&mut frame.encoder);
        }

        let mut pass = if blurred {
            gpu.main_pass(&mut frame, Some(passes::CLEAR), true)
        } else {
            self.record_world(&r, &mut frame, f, indirect_used, &mut marks)
        };
        self.record_backdrop(&r, &mut pass, f);
        self.stamp_in_pass(&mut pass, slot, 2);
        self.record_ui(&r, &mut pass, f, n_ui);
        drop(pass);
        self.stamp(&mut frame.encoder, slot, 3);
        self.end_timestamps(&mut frame.encoder, slot);
        drop(r);

        let clock_recorded = Instant::now();
        gpu.end_frame(frame);
        self.read_timestamps_later(slot);
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
