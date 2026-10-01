//! The main pass's world: sky, chunks, entities, water, overlays, the first-person hand and
//! gun with the scope's eyepiece; or, under a menu, the blurred backdrop instead.

use super::{chunk_draw, push, record_indirect, set_view, IndirectDraw, Rec};
use crate::engine::Frame;
use crate::render::cull::{select_chunks, VisibleChunk};
use crate::render::dynamic::{ENTITY, LENS, LINES, OVERLAY, PARTICLES, TRANSLUCENT, VIEWMODEL, VIEWMODEL_GLASS};
use crate::render::frame::{FrameInfo, SCOPE_SIZE};
use crate::render::pipelines::DrawPush;
use crate::render::Renderer;
use glam::Mat4;
use std::time::Instant;

/// The main pass's clear colour (the sky covers it).
pub(in crate::render) const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

impl Renderer {
    /// Records the world into the main pass, its indirect draws from command `indirect_used`
    /// on. `marks` 1 to 3 get the times the chunks were picked and drawn (for --bench). Returns
    /// the main pass, to go on with (the hand's part of it, if it has one).
    pub(in crate::render) fn record_world(
        &mut self,
        r: &Rec,
        frame: &mut Frame,
        f: &FrameInfo,
        indirect_used: usize,
        marks: &mut [Instant; 4],
    ) -> wgpu::RenderPass<'static> {
        let pipes = &self.pipes;
        let world_group = &self.groups.world;
        // The first-person hand is drawn on a cleared depth (so it never clips into walls): in
        // a second part of the pass, the colour kept.
        let (v0, vn) = r.range(VIEWMODEL);
        let mut p = r.gpu.main_pass(frame, Some(CLEAR), vn == 0);

        // Sky
        p.set_pipeline(&pipes.scene.sky);
        p.set_bind_group(0, world_group, &[]);
        push(&mut p, &DrawPush::new(f.view_proj, 0.0));
        p.draw(0..3, 0..1);

        marks[1] = Instant::now();
        // Visible chunks
        let mut visible = select_chunks(
            &self.chunks,
            f.view_proj,
            f.cam_pos,
            f.view_distance,
            f.detail_px,
            self.drawn_chunks + 64,
        );
        marks[2] = Instant::now();
        self.drawn_chunks = visible.len();
        // By mesh page, then front to back: the world's indirect draws come out grouped per
        // page already (the sort in `record_indirect` then only confirms it).
        visible.sort_unstable_by(|a, b| a.mesh.page.cmp(&b.mesh.page).then(a.dist2.total_cmp(&b.dist2)));

        // Opaque (front to back): all chunks' opaque parts as indirect draws, one command per
        // mesh page (the meshes share a few big buffers). First the plain whole-block faces
        // without alpha testing (their depth test runs before the fragment shader), then the
        // rest.
        push(&mut p, &DrawPush::new(f.view_proj, 0.0));
        let mut next = Some(indirect_used);
        for (pipe, plain) in [(&pipes.world_plain, true), (&pipes.scene.world_chunk, false)] {
            p.set_pipeline(pipe);
            let mut draws: Vec<IndirectDraw> = Vec::with_capacity(visible.len() * 3);
            for c in &visible {
                for (first, count) in if plain { c.plain } else { c.parts }.iter() {
                    draws.push(chunk_draw(&c.mesh, first, count));
                }
            }
            // Sorted by page, front to back within each (the sort is stable).
            next = next.and_then(|base| record_indirect(&mut p, &self.arena, &self.indirect, base, &mut draws, r.gpu));
            if next.is_none() {
                for c in &visible {
                    c.mesh.bind(&mut p, &self.arena);
                    for (first, count) in if plain { c.plain } else { c.parts }.iter() {
                        c.mesh.draw(&mut p, first, count);
                    }
                }
            }
        }
        marks[3] = Instant::now();
        p.set_pipeline(&pipes.scene.world);
        r.draw_dyn(&mut p, PARTICLES);
        let (e0, en) = r.range(ENTITY);
        let player_n = f.player_vertex_count.min(en);
        if f.entity_visible && en > 0 {
            r.bind_dyn(&mut p);
            if en > player_n {
                p.draw(e0 + player_n..e0 + en, 0..1);
            }
            if player_n > 0 && f.player_opacity >= 0.999 {
                p.draw(e0..e0 + player_n, 0..1);
            }
        }

        // Outline
        if r.range(LINES).1 > 0 {
            p.set_pipeline(&pipes.line);
            r.draw_dyn(&mut p, LINES);
        }

        // Water (back to front)
        p.set_pipeline(&pipes.scene.water_chunk);
        push(&mut p, &DrawPush::new(f.view_proj, 1.0));
        let mut water: Vec<&VisibleChunk> = visible.iter().filter(|c| c.water > 0).collect();
        water.sort_unstable_by(|a, b| b.dist2.total_cmp(&a.dist2));
        for c in water {
            c.mesh.bind(&mut p, &self.arena);
            c.mesh.draw(&mut p, c.opaque, c.water);
        }
        p.set_pipeline(&pipes.scene.water);
        r.draw_dyn(&mut p, TRANSLUCENT);

        // Break cracks
        if r.range(OVERLAY).1 > 0 {
            p.set_pipeline(&pipes.scene.overlay);
            r.draw_dyn(&mut p, OVERLAY);
        }

        if f.entity_visible && player_n > 0 && (0.01..0.999).contains(&f.player_opacity) {
            p.set_pipeline(&pipes.player_fade);
            let mut fade = DrawPush::new(f.view_proj, 0.0);
            fade.params[1] = f.player_opacity;
            push(&mut p, &fade);
            r.bind_dyn(&mut p);
            p.draw(e0..e0 + player_n, 0..1);
        }

        if vn == 0 {
            return p;
        }
        // First-person hand: on a cleared depth, so it never clips into walls.
        drop(p);
        let mut p = r.gpu.main_pass(frame, None, true);
        let vm_flame = if f
            .viewmodel
            .last()
            .is_some_and(|v| v.layer == crate::textures::tex::TORCH_FLAME as f32)
        {
            vn.min(24)
        } else {
            0
        };
        p.set_pipeline(&pipes.scene.world);
        p.set_bind_group(0, world_group, &[]);
        push(&mut p, &DrawPush::new(f.vm_view_proj, 2.0));
        r.bind_dyn(&mut p);
        p.draw(v0..v0 + vn - vm_flame, 0..1);
        if vm_flame > 0 {
            p.set_pipeline(&pipes.scene.water);
            p.draw(v0 + vn - vm_flame..v0 + vn, 0..1);
        }
        // The scope's eyepiece shows its view; the glass is drawn over what is behind.
        let (n0, nn) = r.range(LENS);
        if nn > 0 && f.scope.is_some() && !f.backdrop_blur {
            p.set_pipeline(&self.lens_pipe);
            p.set_bind_group(1, &self.groups.lens, &[]);
            push(&mut p, &DrawPush::new(f.vm_view_proj, 2.0));
            p.draw(n0..n0 + nn, 0..1);
        }
        let (g0, gn) = r.range(VIEWMODEL_GLASS);
        if gn > 0 {
            p.set_pipeline(&pipes.scene.water);
            push(&mut p, &DrawPush::new(f.vm_view_proj, 4.0));
            p.draw(g0..g0 + gn, 0..1);
        }
        p
    }

    /// The first half of a menu's blurred backdrop, before the main pass: the scope pass's
    /// picture blurred across.
    pub(in crate::render) fn record_blur_across(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut p = self.blur.begin(encoder);
        set_view(&mut p, 0, 0, SCOPE_SIZE, SCOPE_SIZE);
        p.set_pipeline(&self.blur_across_pipe);
        p.set_bind_group(0, &self.groups.world, &[]);
        p.set_bind_group(1, &self.groups.lens, &[]);
        p.draw(0..3, 0..1);
    }

    /// A menu's blurred backdrop: the world as the scope pass drew it (blurred across by
    /// `record_blur_across`), blurred down over everything.
    pub(in crate::render) fn record_backdrop(&self, r: &Rec, p: &mut wgpu::RenderPass, f: &FrameInfo) {
        let (n0, nn) = r.range(LENS);
        if f.backdrop_blur && f.scope.is_some() && nn > 0 {
            p.set_pipeline(&self.blur_pipe);
            p.set_bind_group(0, &self.groups.world, &[]);
            p.set_bind_group(1, &self.groups.blur, &[]);
            push(p, &DrawPush::new(Mat4::IDENTITY, 2.0));
            r.bind_dyn(p);
            p.draw(n0..n0 + nn, 0..1);
        }
    }
}
