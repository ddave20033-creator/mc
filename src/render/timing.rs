//! GPU timestamps: 4 per frame slot (start, shadows done, world done, end), read back when
//! the slot comes round again (`Renderer::gpu_ms`).

use super::passes::Rec;
use super::Renderer;
use crate::engine::{Gpu, FRAMES_IN_FLIGHT};
use ash::vk;

impl Renderer {
    pub(super) unsafe fn create_query_pool(gpu: &Gpu) -> Option<(vk::QueryPool, f32)> {
        gpu.timestamp_period.map(|period| {
            let pool = gpu
                .device
                .create_query_pool(
                    &vk::QueryPoolCreateInfo::default()
                        .query_type(vk::QueryType::TIMESTAMP)
                        .query_count(4 * FRAMES_IN_FLIGHT as u32),
                    None,
                )
                .expect("create query pool");
            (pool, period)
        })
    }

    /// This slot's previous frame has finished (begin_frame waited for it): read its
    /// timestamps, then reuse them for this frame (its first written now).
    pub(super) unsafe fn begin_timestamps(&mut self, gpu: &Gpu, cmd: vk::CommandBuffer, slot: usize) {
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
    }

    /// Timestamp `i` of this frame (1 shadows done, 2 world done, 3 end).
    pub(super) unsafe fn stamp(&self, r: &Rec, i: u32) {
        if let Some((pool, _)) = self.queries {
            r.d.cmd_write_timestamp(
                r.cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                pool,
                4 * r.slot as u32 + i,
            );
        }
    }
}
