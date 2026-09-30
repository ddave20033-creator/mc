//! The UI, last in the main pass: its vertices in runs, each with its own scissor.

use super::Rec;
use crate::render::frame::FrameInfo;
use crate::render::Renderer;
use ash::vk;

impl Renderer {
    /// Records the first `n_ui` UI vertices (as many as were written this frame).
    pub(in crate::render) unsafe fn record_ui(&self, r: &Rec, f: &FrameInfo, n_ui: usize) {
        if n_ui == 0 {
            return;
        }
        let (d, cmd) = (r.d, r.cmd);
        let ext = r.gpu.extent;
        let screen = [ext.width as f32, ext.height as f32, 0.0f32, 0.0];
        r.bind_pipe(self.pipes.ui);
        r.bind_sets(self.ui_layout, &[self.desc.ui_set]);
        r.push(self.ui_layout, &screen);
        d.cmd_bind_vertex_buffers(cmd, 0, &[self.ui_bufs[r.slot].handle], &[0]);
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
}
