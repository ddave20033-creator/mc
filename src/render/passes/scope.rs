//! The scope pass: the magnified view, drawn into the scope's image for the eyepiece (and,
//! under a menu, for its blurred backdrop).

use super::{chunk_draw, record_indirect, IndirectDraw, Rec};
use crate::render::cull::{select_chunks, VisibleChunk};
use crate::render::dynamic::{ENTITY, OVERLAY, PARTICLES, TRANSLUCENT};
use crate::render::frame::{FrameInfo, ScopeView, SCOPE_SIZE};
use crate::render::pipelines::DrawPush;
use crate::render::targets::full_rect;
use crate::render::Renderer;

impl Renderer {
    /// Records the scope pass, its indirect draws from command `indirect_used` on. Returns the
    /// next free command.
    pub(in crate::render) unsafe fn record_scope_pass(
        &self,
        r: &Rec,
        f: &FrameInfo,
        sv: &ScopeView,
        indirect_used: usize,
    ) -> usize {
        let (d, cmd) = (r.d, r.cmd);
        let mut indirect_used = indirect_used;
        self.scope_ubos[r.slot].write(0, std::slice::from_ref(&sv.ubo));
        let scope_set = self.desc.scope_sets[r.slot];
        let pipes = &self.scope_pipes;
        self.scope.begin(d, cmd);
        r.set_view(full_rect(SCOPE_SIZE, SCOPE_SIZE));
        r.bind_pipe(pipes.sky);
        r.bind_sets(self.world_layout, &[scope_set]);
        r.push(self.world_layout, &DrawPush::new(sv.view_proj, 0.0));
        d.cmd_draw(cmd, 3, 1, 0, 0);
        // The chunks in its narrow view, with the detail its magnification shows (all faces:
        // none left out by direction).
        let visible = select_chunks(
            &self.chunks,
            &self.arena,
            sv.view_proj,
            sv.cam_pos,
            f.view_distance,
            sv.detail_px,
            0,
        );
        let mut draws: Vec<IndirectDraw> = visible
            .iter()
            .filter(|c| c.drawn > 0)
            .map(|c| chunk_draw(&c.mesh, 0, c.drawn))
            .collect();
        r.bind_pipe(pipes.world_chunk);
        let ind = &self.indirect[r.slot];
        match record_indirect(d, cmd, ind, indirect_used, &mut draws, r.gpu) {
            Some(next) => indirect_used = next,
            None => {
                for c in visible.iter().filter(|c| c.drawn > 0) {
                    c.mesh.bind(d, cmd);
                    c.mesh.draw(d, cmd, 0, c.drawn);
                }
            }
        }
        r.bind_pipe(pipes.world);
        r.bind_dyn();
        let (p0, pn) = r.range(PARTICLES);
        if pn > 0 {
            d.cmd_draw(cmd, pn, 1, p0, 0);
        }
        let (e0, en) = r.range(ENTITY);
        if f.entity_visible && en > 0 {
            d.cmd_draw(cmd, en, 1, e0, 0);
        }
        r.bind_pipe(pipes.water_chunk);
        r.push(self.world_layout, &DrawPush::new(sv.view_proj, 1.0));
        // Water back to front.
        let mut water: Vec<&VisibleChunk> = visible.iter().filter(|c| c.water > 0).collect();
        water.sort_unstable_by(|a, b| b.dist2.total_cmp(&a.dist2));
        for c in water {
            c.mesh.bind(d, cmd);
            c.mesh.draw(d, cmd, c.opaque, c.water);
        }
        r.bind_pipe(pipes.water);
        r.draw_dyn(TRANSLUCENT);
        // Bullet holes and break cracks on the blocks.
        if r.range(OVERLAY).1 > 0 {
            r.bind_pipe(pipes.overlay);
            r.draw_dyn(OVERLAY);
        }
        d.cmd_end_render_pass(cmd);
        indirect_used
    }
}
