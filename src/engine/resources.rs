//! GPU memory: buffers (written from the CPU through the queue) and sampled textures (texture
//! arrays with their mip levels). wgpu frees each when it is dropped and no submitted work
//! uses it any more.

use super::gpu::{block_on, Gpu};

/// The bytes of `data`.
pub fn bytes<T: Copy>(data: &[T]) -> &[u8] {
    // SAFETY: the types written are plain data (floats, integers and arrays of them).
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
}

/// A buffer the CPU writes through the queue: the writes reach it before the commands of the
/// next submission, after the ones already submitted.
pub struct Buffer {
    pub handle: wgpu::Buffer,
    queue: wgpu::Queue,
}

impl Buffer {
    pub fn new(gpu: &Gpu, size: u64, usage: wgpu::BufferUsages) -> Self {
        Self::try_new(gpu, size, usage).expect("allocate buffer memory")
    }

    /// As `new`, but the video memory running out is an answer (None), not a crash.
    pub fn try_new(gpu: &Gpu, size: u64, usage: wgpu::BufferUsages) -> Option<Self> {
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let handle = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if block_on(scope.pop()).is_some() {
            return None;
        }
        Some(Self { handle, queue: gpu.queue.clone() })
    }

    pub fn size(&self) -> u64 {
        self.handle.size()
    }

    /// Writes `data` at byte `offset` (both multiples of 4).
    pub fn write<T: Copy>(&self, offset: usize, data: &[T]) {
        let data = bytes(data);
        assert!(offset + data.len() <= self.size() as usize, "buffer write out of range");
        if !data.is_empty() {
            self.queue.write_buffer(&self.handle, offset as u64, data);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SamplerKind {
    /// Pixel-art block textures: mipmapped and anisotropic (the shaders read the nearest texel
    /// themselves when magnified: shaders/blocks.wgsl).
    Blocks,
    /// The font atlas: smoothly filtered (an anti-aliased typeface drawn at any size), clamped.
    Font,
}

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    /// `levels[i]` holds mip level `i` for every array layer, layer after layer.
    pub fn new(
        gpu: &Gpu,
        width: u32,
        height: u32,
        layers: u32,
        format: wgpu::TextureFormat,
        levels: &[Vec<u8>],
        kind: SamplerKind,
    ) -> Self {
        let mips = levels.len() as u32;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(match kind {
                SamplerKind::Blocks => "blocks",
                SamplerKind::Font => "font",
            }),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: layers },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_layers(gpu, &texture, 0, layers, levels);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(match kind {
                SamplerKind::Blocks => wgpu::TextureViewDimension::D2Array,
                SamplerKind::Font => wgpu::TextureViewDimension::D2,
            }),
            ..Default::default()
        });
        let sampler = gpu.device.create_sampler(&match kind {
            SamplerKind::Blocks => wgpu::SamplerDescriptor {
                label: Some("blocks"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                lod_max_clamp: mips as f32,
                anisotropy_clamp: if gpu.anisotropy { 16 } else { 1 },
                ..Default::default()
            },
            SamplerKind::Font => wgpu::SamplerDescriptor {
                label: Some("font"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                lod_max_clamp: 0.0,
                ..Default::default()
            },
        });
        Self { texture, view, sampler }
    }
}

/// Writes `count` array layers of `texture` from `first` on: `levels[i]` holds mip level `i`
/// of them, layer after layer.
pub fn write_layers(gpu: &Gpu, texture: &wgpu::Texture, first: u32, count: u32, levels: &[Vec<u8>]) {
    let texel = texture.format().block_copy_size(None).expect("a plain colour format");
    for (i, level) in levels.iter().enumerate() {
        let w = (texture.width() >> i).max(1);
        let h = (texture.height() >> i).max(1);
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d { x: 0, y: 0, z: first },
                aspect: wgpu::TextureAspect::All,
            },
            level,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * texel), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: count },
        );
    }
}
