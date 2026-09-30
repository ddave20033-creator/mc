//! The main pass's world: sky, chunks, entities, water, overlays, the first-person hand and
//! gun with the scope's eyepiece; or, under a menu, the blurred backdrop instead.

use super::{chunk_draw, record_indirect, IndirectDraw, Rec};
use crate::render::cull::{select_chunks, VisibleChunk};
use crate::render::dynamic::{ENTITY, LENS, LINES, OVERLAY, PARTICLES, TRANSLUCENT, VIEWMODEL, VIEWMODEL_GLASS};
use crate::render::frame::FrameInfo;
use crate::render::pipelines::DrawPush;
use crate::render::Renderer;
use ash::vk;
use glam::Mat4;
use std::time::Instant;

impl Renderer {
    /// Records the world into the main pass, its indirect draws from command `indirect_used`
    /// on. `marks` 1 to 3 get the times the chunks were picked and drawn (for --bench).
    pub(in crate::render) unsafe fn record_world(
        &mut self,
        r: &Rec,
        f: &FrameInfo,
        indirect_used: usize,
        marks: &mut [Instant; 4],
    ) {
        let (d, cmd) = (r.d, r.cmd);
        let world_set = self.desc.world_sets[r.slot];
        let pipes = &self.pipes;

        // Sky
        r.bind_pipe(pipes.sky);
        r.bind_sets(self.world_layout, &[world_set]);
        r.push(self.world_layout, &DrawPush::new(f.view_proj, 0.0));
        d.cmd_draw(cmd, 3, 1, 0, 0);

        marks[1] = Instant::now();
        // Visible chunks
        let mut visible = select_chunks(
            &self.chunks,
            &self.arena,
            f.view_proj,
            f.cam_pos,
            f.view_distance,
            f.detail_px,
            self.drawn_chunks + 64,
        );
        marks[2] = Instant::now();
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
        r.bind_pipe(pipes.world);
        r.push(self.world_layout, &DrawPush::new(f.view_proj, 0.0));
        // All chunks' opaque parts as indirect draws, one command per mesh buffer (the
        // meshes share a few big buffers).
        let mut draws: Vec<IndirectDraw> = Vec::with_capacity(visible.len() * 3);
        for c in &visible {
            for &(first, count) in c.parts.iter().filter(|p| p.1 > 0) {
                draws.push(chunk_draw(c.buffer, c.vertices, c.indices, first, count));
            }
        }
        let ind = &self.indirect[r.slot];
        let multi = r.gpu.multi_draw_indirect;
        // Sorted by buffer, front to back within each (the sort is stable).
        let recorded = record_indirect(d, cmd, ind, indirect_used, &mut draws, multi);
        marks[3] = Instant::now();
        if recorded.is_none() {
            for c in &visible {
                bind(c);
                for &(first, count) in c.parts.iter().filter(|p| p.1 > 0) {
                    d.cmd_draw_indexed(cmd, count, 1, first, 0, 0);
                }
            }
        }
        r.draw_dyn(PARTICLES);
        let (e0, en) = r.range(ENTITY);
        let player_n = f.player_vertex_count.min(en);
        if f.entity_visible && en > 0 {
            r.bind_dyn();
            if en > player_n {
                d.cmd_draw(cmd, en - player_n, 1, e0 + player_n, 0);
            }
            if player_n > 0 && f.player_opacity >= 0.999 {
                d.cmd_draw(cmd, player_n, 1, e0, 0);
            }
        }

        // Outline
        if r.range(LINES).1 > 0 {
            r.bind_pipe(pipes.line);
            r.draw_dyn(LINES);
        }

        // Water (back to front)
        r.bind_pipe(pipes.water);
        r.push(self.world_layout, &DrawPush::new(f.view_proj, 1.0));
        let mut water: Vec<&VisibleChunk> = visible.iter().filter(|c| c.water > 0).collect();
        water.sort_unstable_by(|a, b| b.dist2.total_cmp(&a.dist2));
        for c in water {
            bind(c);
            d.cmd_draw_indexed(cmd, c.water, 1, c.opaque, 0, 0);
        }
        r.draw_dyn(TRANSLUCENT);

        // Break cracks
        if r.range(OVERLAY).1 > 0 {
            r.bind_pipe(pipes.overlay);
            r.draw_dyn(OVERLAY);
        }

        if f.entity_visible && player_n > 0 && (0.01..0.999).contains(&f.player_opacity) {
            r.bind_pipe(pipes.player_fade);
            let mut fade = DrawPush::new(f.view_proj, 0.0);
            fade.params[1] = f.player_opacity;
            r.push(self.world_layout, &fade);
            r.bind_dyn();
            d.cmd_draw(cmd, player_n, 1, e0, 0);
        }

        // First-person hand: clear depth so it never clips into walls
        let (v0, vn) = r.range(VIEWMODEL);
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
                        extent: r.gpu.extent,
                    },
                    base_array_layer: 0,
                    layer_count: 1,
                }],
            );
            r.bind_pipe(pipes.world);
            r.push(self.world_layout, &DrawPush::new(f.vm_view_proj, 2.0));
            r.bind_dyn();
            d.cmd_draw(cmd, vn - vm_flame, 1, v0, 0);
            if vm_flame > 0 {
                r.bind_pipe(pipes.water);
                d.cmd_draw(cmd, vm_flame, 1, v0 + vn - vm_flame, 0);
            }
            // The scope's eyepiece shows its view; the glass is drawn over what is behind.
            let (n0, nn) = r.range(LENS);
            if nn > 0 && f.scope.is_some() && !f.backdrop_blur {
                r.bind_pipe(self.lens_pipe);
                r.bind_sets(self.lens_layout, &[world_set, self.desc.lens_set]);
                r.push(self.lens_layout, &DrawPush::new(f.vm_view_proj, 2.0));
                d.cmd_draw(cmd, nn, 1, n0, 0);
                r.bind_sets(self.world_layout, &[world_set]);
            }
            let (g0, gn) = r.range(VIEWMODEL_GLASS);
            if gn > 0 {
                r.bind_pipe(pipes.water);
                r.push(self.world_layout, &DrawPush::new(f.vm_view_proj, 4.0));
                d.cmd_draw(cmd, gn, 1, g0, 0);
            }
        }
    }

    /// A menu's blurred backdrop: the world as the scope pass drew it, over everything.
    pub(in crate::render) unsafe fn record_backdrop(&self, r: &Rec, f: &FrameInfo) {
        let (n0, nn) = r.range(LENS);
        if f.backdrop_blur && f.scope.is_some() && nn > 0 {
            r.bind_pipe(self.blur_pipe);
            r.bind_sets(self.lens_layout, &[self.desc.world_sets[r.slot], self.desc.lens_set]);
            r.push(self.lens_layout, &DrawPush::new(Mat4::IDENTITY, 2.0));
            r.bind_dyn();
            r.d.cmd_draw(r.cmd, nn, 1, n0, 0);
        }
    }
}
