//! The scope pass: the magnified view, drawn into the scope's image for the eyepiece (and,
//! under a menu, for its blurred backdrop).

use super::{chunk_draw, push, record_indirect, set_view, IndirectDraw, Rec};
use crate::render::cull::{select_chunks, VisibleChunk};
use crate::render::dynamic::{ENTITY, OVERLAY, PARTICLES, TRANSLUCENT};
use crate::render::frame::{FrameInfo, ScopeView, SCOPE_SIZE};
use crate::render::pipelines::DrawPush;
use crate::render::Renderer;

impl Renderer {
    /// Records the scope pass, its indirect draws from command `indirect_used` on. Returns the
    /// next free command.
    pub(in crate::render) fn record_scope_pass(
        &self,
        r: &Rec,
        encoder: &mut wgpu::CommandEncoder,
        f: &FrameInfo,
        sv: &ScopeView,
        indirect_used: usize,
    ) -> usize {
        let mut indirect_used = indirect_used;
        self.scope_ubo.write(0, std::slice::from_ref(&sv.ubo));
        let pipes = &self.scope_pipes;
        let mut p = self.scope.begin(encoder);
        set_view(&mut p, 0, 0, SCOPE_SIZE, SCOPE_SIZE);
        p.set_pipeline(&pipes.sky);
        p.set_bind_group(0, &self.groups.scope, &[]);
        push(&mut p, &DrawPush::new(sv.view_proj, 0.0));
        p.draw(0..3, 0..1);
        // The chunks in its narrow view, with the detail its magnification shows (all faces:
        // none left out by direction).
        let visible = select_chunks(&self.chunks, sv.view_proj, sv.cam_pos, f.view_distance, sv.detail_px, 0);
        let mut draws: Vec<IndirectDraw> = visible
            .iter()
            .filter(|c| c.drawn > 0)
            .map(|c| chunk_draw(&c.mesh, 0, c.drawn))
            .collect();
        p.set_pipeline(&pipes.world_chunk);
        match record_indirect(&mut p, &self.arena, &self.indirect, indirect_used, &mut draws, r.gpu) {
            Some(next) => indirect_used = next,
            None => {
                for c in visible.iter().filter(|c| c.drawn > 0) {
                    c.mesh.bind(&mut p, &self.arena);
                    c.mesh.draw(&mut p, 0, c.drawn);
                }
            }
        }
        p.set_pipeline(&pipes.world);
        r.bind_dyn(&mut p);
        let (p0, pn) = r.range(PARTICLES);
        if pn > 0 {
            p.draw(p0..p0 + pn, 0..1);
        }
        let (e0, en) = r.range(ENTITY);
        if f.entity_visible && en > 0 {
            p.draw(e0..e0 + en, 0..1);
        }
        p.set_pipeline(&pipes.water_chunk);
        push(&mut p, &DrawPush::new(sv.view_proj, 1.0));
        // Water back to front.
        let mut water: Vec<&VisibleChunk> = visible.iter().filter(|c| c.water > 0).collect();
        water.sort_unstable_by(|a, b| b.dist2.total_cmp(&a.dist2));
        for c in water {
            c.mesh.bind(&mut p, &self.arena);
            c.mesh.draw(&mut p, c.opaque, c.water);
        }
        p.set_pipeline(&pipes.water);
        r.draw_dyn(&mut p, TRANSLUCENT);
        // Bullet holes and break cracks on the blocks.
        if r.range(OVERLAY).1 > 0 {
            p.set_pipeline(&pipes.overlay);
            r.draw_dyn(&mut p, OVERLAY);
        }
        indirect_used
    }
}
