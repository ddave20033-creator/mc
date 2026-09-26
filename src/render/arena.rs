//! Video memory for chunk meshes. A program may only have a limited number of Vulkan memory
//! allocations (often 4096, fewer than the chunks at a long render distance), and allocating
//! is slow, so the meshes share big buffers ("pages") split into ranges instead of each
//! chunk getting its own.

use crate::engine::{Buffer, Gpu};
use ash::vk;

/// Size of one page; a mesh bigger than this gets a page of its own.
const PAGE_SIZE: u64 = 64 << 20;
/// Ranges start at multiples of this, so vertex and index data inside them stay aligned.
const ALIGN: u64 = 256;

/// The part of a page holding one mesh.
#[derive(Clone, Copy, Debug)]
pub struct Range {
    page: usize,
    pub offset: u64,
    size: u64,
}

struct Page {
    buffer: Buffer,
    /// Free parts (offset, size), sorted by offset; neighbours are always merged.
    free: Vec<(u64, u64)>,
    used: u64,
}

#[derive(Default)]
pub struct Arena {
    pages: Vec<Option<Page>>,
    /// Ranges given back while a frame in flight may still read them, with the frame number
    /// from when they were given back.
    retired: Vec<(u64, Range)>,
}

impl Arena {
    pub fn buffer(&self, r: Range) -> vk::Buffer {
        self.pages[r.page]
            .as_ref()
            .expect("range of a freed page")
            .buffer
            .handle
    }

    /// A range of at least `size` bytes (first fit, a new page if none has room).
    pub fn alloc(&mut self, gpu: &Gpu, size: u64) -> Range {
        let size = size.max(1).div_ceil(ALIGN) * ALIGN;
        for (i, page) in self.pages.iter_mut().enumerate() {
            let Some(page) = page else { continue };
            if let Some(k) = page.free.iter().position(|&(_, s)| s >= size) {
                let (offset, s) = page.free[k];
                if s == size {
                    page.free.remove(k);
                } else {
                    page.free[k] = (offset + size, s - size);
                }
                page.used += size;
                return Range {
                    page: i,
                    offset,
                    size,
                };
            }
        }
        let page_size = size.max(PAGE_SIZE);
        let page = Page {
            buffer: Buffer::new(
                gpu,
                page_size,
                vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::INDEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ),
            free: if page_size > size {
                vec![(size, page_size - size)]
            } else {
                Vec::new()
            },
            used: size,
        };
        let page_index = match self.pages.iter().position(Option::is_none) {
            Some(i) => {
                self.pages[i] = Some(page);
                i
            }
            None => {
                self.pages.push(Some(page));
                self.pages.len() - 1
            }
        };
        Range {
            page: page_index,
            offset: 0,
            size,
        }
    }

    /// Gives `r` back once the frames that may still draw from it are done. `frame` is the
    /// number of the next frame to be recorded.
    pub fn retire(&mut self, r: Range, frame: u64) {
        self.retired.push((frame, r));
    }

    /// Frees the ranges retired at frame `done` or earlier (every frame before `done` has
    /// finished on the GPU).
    pub fn collect(&mut self, gpu: &mut Gpu, done: u64) {
        let mut i = 0;
        while i < self.retired.len() {
            if self.retired[i].0 <= done {
                let (_, r) = self.retired.swap_remove(i);
                self.free(gpu, r);
            } else {
                i += 1;
            }
        }
    }

    fn free(&mut self, gpu: &mut Gpu, r: Range) {
        let Some(page) = self.pages[r.page].as_mut() else {
            return;
        };
        page.used -= r.size;
        let k = page.free.partition_point(|&(o, _)| o < r.offset);
        page.free.insert(k, (r.offset, r.size));
        if k + 1 < page.free.len() && page.free[k].0 + page.free[k].1 == page.free[k + 1].0 {
            page.free[k].1 += page.free[k + 1].1;
            page.free.remove(k + 1);
        }
        if k > 0 && page.free[k - 1].0 + page.free[k - 1].1 == page.free[k].0 {
            page.free[k - 1].1 += page.free[k].1;
            page.free.remove(k);
        }
        // Empty pages go back to the driver (except the first, which is always needed).
        if page.used == 0 && r.page > 0 {
            if let Some(page) = self.pages[r.page].take() {
                gpu.defer_destroy(page.buffer);
            }
        }
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        for page in self.pages.drain(..).flatten() {
            page.buffer.destroy(device);
        }
        self.retired.clear();
    }
}
