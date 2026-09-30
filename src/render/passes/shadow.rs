//! The shadow pass: the sun's shadow map, and the weapon lights' in the strip under it.

use super::{chunk_draw, record_indirect, IndirectDraw, Rec};
use crate::render::cull::Frustum;
use crate::render::dynamic::ENTITY;
use crate::render::frame::{FrameInfo, MAX_SPOTS, SHADOW_SIZE, SPOT_REACH, SPOT_SHADOW};
use crate::render::pipelines::DrawPush;
use crate::render::targets::full_rect;
use crate::render::Renderer;
use ash::vk;
use glam::{Mat4, Vec3};

impl Renderer {
    /// Records the shadow pass. Returns how many indirect commands it used (the passes after
    /// it write theirs after them).
    pub(in crate::render) unsafe fn record_shadow_pass(&self, r: &Rec, f: &FrameInfo) -> usize {
        let (d, cmd) = (r.d, r.cmd);
        let world_set = self.desc.world_sets[r.slot];
        let mut indirect_used = 0;
        self.shadow.begin(d, cmd);
        if f.shadows {
            r.set_view(full_rect(SHADOW_SIZE, SHADOW_SIZE));
            r.bind_pipe(self.shadow_pipe);
            r.bind_sets(self.world_layout, &[world_set]);
            r.push(self.world_layout, &DrawPush::new(f.light_view_proj, 3.0));
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
                let Some(m) = c.mesh else { continue };
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
                let (b, v, i) = (self.arena.buffer(m), m.offset, m.offset + c.index_offset);
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
            let ind = &self.indirect[r.slot];
            let multi = r.gpu.multi_draw_indirect;
            indirect_used = record_indirect(d, cmd, ind, 0, &mut draws, multi).unwrap_or_else(|| {
                for &(b, v, i, n) in &shadow_chunks {
                    d.cmd_bind_vertex_buffers(cmd, 0, &[b], &[v]);
                    d.cmd_bind_index_buffer(cmd, b, i, vk::IndexType::UINT32);
                    d.cmd_draw_indexed(cmd, n, 1, 0, 0, 0);
                }
                0
            });
            r.draw_dyn(ENTITY);
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
            r.set_view(vk::Rect2D {
                offset: vk::Offset2D { x: (k as u32 * SPOT_SHADOW) as i32, y: SHADOW_SIZE as i32 },
                extent: vk::Extent2D { width: SPOT_SHADOW, height: SPOT_SHADOW },
            });
            r.bind_pipe(self.shadow_pipe);
            r.bind_sets(self.world_layout, &[world_set]);
            r.push(self.world_layout, &DrawPush::new(vp, 5.0));
            let seen = Frustum::new(vp);
            let reach = (SPOT_REACH / 16.0).ceil() as i32 + 1;
            let (ccx, ccz) = ((at.x / 16.0).floor() as i32, (at.z / 16.0).floor() as i32);
            for dz in -reach..=reach {
                for dx in -reach..=reach {
                    let Some(c) = self.chunks.get(&(ccx + dx, ccz + dz)) else { continue };
                    let Some(m) = c.mesh else { continue };
                    // Only the blocks (and the outside of leaves): grass and flowers cast no
                    // shadow of the light, nor do the faces inside leaves.
                    if c.solid == 0 || !seen.visible(c.min, c.max) {
                        continue;
                    }
                    let b = self.arena.buffer(m);
                    d.cmd_bind_vertex_buffers(cmd, 0, &[b], &[m.offset]);
                    d.cmd_bind_index_buffer(cmd, b, m.offset + c.index_offset, vk::IndexType::UINT32);
                    d.cmd_draw_indexed(cmd, c.solid, 1, 0, 0, 0);
                }
            }
        }
        d.cmd_end_render_pass(cmd);
        indirect_used
    }
}
