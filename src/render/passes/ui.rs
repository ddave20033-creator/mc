//! The UI, last in the main pass: its vertices in runs, each with its own scissor.

use super::{push, Rec};
use crate::render::frame::FrameInfo;
use crate::render::Renderer;

impl Renderer {
    /// Records the first `n_ui` UI vertices (as many as were written this frame).
    pub(in crate::render) fn record_ui(&self, r: &Rec, p: &mut wgpu::RenderPass, f: &FrameInfo, n_ui: usize) {
        if n_ui == 0 {
            return;
        }
        let ext = r.gpu.extent;
        let screen = [ext.width as f32, ext.height as f32, 0.0f32, 0.0];
        p.set_pipeline(&self.pipes.ui);
        p.set_bind_group(0, &self.groups.ui, &[]);
        push(p, &screen);
        p.set_vertex_buffer(0, self.ui_buf.handle.slice(..));
        for (i, &(start, clip)) in f.ui_clips.iter().enumerate() {
            let end = f
                .ui_clips
                .get(i + 1)
                .map_or(n_ui as u32, |c| c.0)
                .min(n_ui as u32);
            if end <= start {
                continue;
            }
            let (x, y, w, h) = match clip {
                Some([x, y, w, h]) => {
                    let x0 = x.floor().clamp(0.0, ext.width as f32);
                    let y0 = y.floor().clamp(0.0, ext.height as f32);
                    let x1 = (x + w).ceil().clamp(x0, ext.width as f32);
                    let y1 = (y + h).ceil().clamp(y0, ext.height as f32);
                    (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
                }
                None => (0, 0, ext.width, ext.height),
            };
            if w == 0 || h == 0 {
                continue;
            }
            p.set_scissor_rect(x, y, w, h);
            p.draw(start..end, 0..1);
        }
        p.set_scissor_rect(0, 0, ext.width, ext.height);
    }
}
