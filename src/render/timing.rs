//! GPU timestamps: 4 per frame slot (start, shadows done, world done, end), resolved into a
//! buffer of the slot's, read back when the slot comes round again (`Renderer::gpu_ms`).

use super::Renderer;
use crate::engine::{Gpu, FRAMES_IN_FLIGHT};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Where one slot's 4 timestamps are resolved (resolves start at multiples of 256 bytes).
const SLOT_STRIDE: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;

pub(super) struct Timestamps {
    set: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    /// Per frame slot: the buffer its timestamps are copied to for reading, and (while it is
    /// being mapped) whether that is done.
    readback: Vec<(wgpu::Buffer, Option<Arc<AtomicBool>>)>,
    /// Nanoseconds per tick.
    period: f32,
    /// This frame writes its timestamps (its slot's buffer was free).
    active: bool,
}

impl Timestamps {
    pub fn new(gpu: &Gpu) -> Option<Self> {
        let period = gpu.timestamp_period?;
        let d = &gpu.device;
        let set = d.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 4 * FRAMES_IN_FLIGHT as u32,
        });
        let resolved = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps"),
            size: SLOT_STRIDE * FRAMES_IN_FLIGHT as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = (0..FRAMES_IN_FLIGHT)
            .map(|_| {
                let b = d.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamps read"),
                    size: 32,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                (b, None)
            })
            .collect();
        Some(Self { set, resolved, readback, period, active: false })
    }
}

impl Renderer {
    /// This slot's previous frame's timestamps: read if they have arrived; then this frame's
    /// first one written (if the slot's buffer is free again).
    pub(super) fn begin_timestamps(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        let Some(t) = &mut self.queries else { return };
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let (buf, mapping) = &mut t.readback[slot];
        if let Some(done) = mapping {
            if !done.load(Ordering::Acquire) {
                // (the GPU has not got that far yet: no timestamps this frame)
                t.active = false;
                return;
            }
            if let Ok(data) = buf.slice(..).get_mapped_range() {
                let v: Vec<u64> = data.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
                let ms = |a: u64, b: u64| b.saturating_sub(a) as f32 * t.period / 1e6;
                self.gpu_ms = Some([ms(v[0], v[1]), ms(v[1], v[2]), ms(v[2], v[3])]);
            }
            buf.unmap();
            *mapping = None;
        }
        t.active = true;
        encoder.write_timestamp(&t.set, 4 * slot as u32);
    }

    /// Timestamp `i` of this frame (1 shadows done, 3 end), between passes.
    pub(super) fn stamp(&self, encoder: &mut wgpu::CommandEncoder, slot: usize, i: u32) {
        if let Some(t) = self.queries.as_ref().filter(|t| t.active) {
            encoder.write_timestamp(&t.set, 4 * slot as u32 + i);
        }
    }

    /// Timestamp `i` of this frame (2 world done) inside a pass.
    pub(super) fn stamp_in_pass(&self, pass: &mut wgpu::RenderPass, slot: usize, i: u32) {
        if let Some(t) = self.queries.as_ref().filter(|t| t.active) {
            pass.write_timestamp(&t.set, 4 * slot as u32 + i);
        }
    }

    /// Copies this frame's timestamps to its slot's buffer (at the end of its commands).
    pub(super) fn end_timestamps(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        if let Some(t) = self.queries.as_ref().filter(|t| t.active) {
            let q0 = 4 * slot as u32;
            let at = SLOT_STRIDE * slot as u64;
            encoder.resolve_query_set(&t.set, q0..q0 + 4, &t.resolved, at);
            encoder.copy_buffer_to_buffer(&t.resolved, at, &t.readback[slot].0, 0, 32);
        }
    }

    /// After the frame is submitted: its timestamps are read back once the GPU is done.
    pub(super) fn read_timestamps_later(&mut self, slot: usize) {
        if let Some(t) = self.queries.as_mut().filter(|t| t.active) {
            let done = Arc::new(AtomicBool::new(false));
            let flag = done.clone();
            let (buf, mapping) = &mut t.readback[slot];
            buf.slice(..).map_async(wgpu::MapMode::Read, move |r| flag.store(r.is_ok(), Ordering::Release));
            *mapping = Some(done);
        }
    }
}
