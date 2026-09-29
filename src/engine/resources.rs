use super::gpu::Gpu;
use ash::{vk, Device};

pub fn find_memory_type(
    props: &vk::PhysicalDeviceMemoryProperties,
    bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> u32 {
    (0..props.memory_type_count)
        .find(|&i| {
            bits & (1 << i) != 0
                && props.memory_types[i as usize]
                    .property_flags
                    .contains(flags)
        })
        .expect("no suitable Vulkan memory type")
}

/// A buffer with its own device memory. Host-visible buffers stay persistently mapped.
pub struct Buffer {
    pub handle: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: u64,
    mapped: *mut u8,
}

impl Buffer {
    pub fn new(
        gpu: &Gpu,
        size: u64,
        usage: vk::BufferUsageFlags,
        flags: vk::MemoryPropertyFlags,
    ) -> Self {
        unsafe {
            let d = &gpu.device;
            let handle = d
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size)
                        .usage(usage)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .expect("create buffer");
            let req = d.get_buffer_memory_requirements(handle);
            let memory = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(find_memory_type(
                            &gpu.mem_props,
                            req.memory_type_bits,
                            flags,
                        )),
                    None,
                )
                .expect("allocate buffer memory");
            d.bind_buffer_memory(handle, memory, 0).unwrap();
            let mapped = if flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
                d.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                    .expect("map memory") as *mut u8
            } else {
                std::ptr::null_mut()
            };
            Self {
                handle,
                memory,
                size,
                mapped,
            }
        }
    }

    pub fn write<T: Copy>(&self, offset: usize, data: &[T]) {
        let bytes = std::mem::size_of_val(data);
        assert!(!self.mapped.is_null(), "buffer is not host visible");
        assert!(
            offset + bytes <= self.size as usize,
            "buffer write out of range"
        );
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                self.mapped.add(offset),
                bytes,
            );
        }
    }

    /// The contents of a host-visible buffer.
    pub fn read(&self) -> &[u8] {
        assert!(!self.mapped.is_null(), "buffer is not host visible");
        unsafe { std::slice::from_raw_parts(self.mapped, self.size as usize) }
    }

    pub fn destroy(&self, device: &Device) {
        unsafe {
            device.destroy_buffer(self.handle, None);
            device.free_memory(self.memory, None);
        }
    }
}

pub struct Image {
    pub handle: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
}

impl Image {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &Device,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
        width: u32,
        height: u32,
        mips: u32,
        layers: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        view_type: vk::ImageViewType,
    ) -> Self {
        Self::with_samples(
            device,
            mem_props,
            width,
            height,
            mips,
            layers,
            format,
            usage,
            aspect,
            view_type,
            vk::SampleCountFlags::TYPE_1,
        )
    }

    /// Like `new`, with several samples per pixel (multisampled render targets).
    #[allow(clippy::too_many_arguments)]
    pub fn with_samples(
        device: &Device,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
        width: u32,
        height: u32,
        mips: u32,
        layers: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        view_type: vk::ImageViewType,
        samples: vk::SampleCountFlags,
    ) -> Self {
        unsafe {
            let handle = device
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D {
                            width,
                            height,
                            depth: 1,
                        })
                        .mip_levels(mips)
                        .array_layers(layers)
                        .samples(samples)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(usage)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                    None,
                )
                .expect("create image");
            let req = device.get_image_memory_requirements(handle);
            let memory = device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(find_memory_type(
                            mem_props,
                            req.memory_type_bits,
                            vk::MemoryPropertyFlags::DEVICE_LOCAL,
                        )),
                    None,
                )
                .expect("allocate image memory");
            device.bind_image_memory(handle, memory, 0).unwrap();
            let view = device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(handle)
                        .view_type(view_type)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: aspect,
                            base_mip_level: 0,
                            level_count: mips,
                            base_array_layer: 0,
                            layer_count: layers,
                        }),
                    None,
                )
                .expect("create image view");
            Self {
                handle,
                memory,
                view,
            }
        }
    }

    pub fn destroy(&self, device: &Device) {
        unsafe {
            device.destroy_image_view(self.view, None);
            device.destroy_image(self.handle, None);
            device.free_memory(self.memory, None);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SamplerKind {
    /// Pixel-art block textures: nearest magnification, mipmapped + anisotropic minification.
    Blocks,
    /// The font atlas: smoothly filtered (an anti-aliased typeface drawn at any size), clamped.
    Font,
}

pub struct Texture {
    pub image: Image,
    pub sampler: vk::Sampler,
}

impl Texture {
    /// `levels[i]` holds mip level `i` for every array layer, layer after layer.
    pub fn new(
        gpu: &Gpu,
        width: u32,
        height: u32,
        layers: u32,
        format: vk::Format,
        levels: &[Vec<u8>],
        kind: SamplerKind,
    ) -> Self {
        let mips = levels.len() as u32;
        let view_type = match kind {
            SamplerKind::Blocks => vk::ImageViewType::TYPE_2D_ARRAY,
            SamplerKind::Font => vk::ImageViewType::TYPE_2D,
        };
        let image = Image::new(
            &gpu.device,
            &gpu.mem_props,
            width,
            height,
            mips,
            layers,
            format,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
            view_type,
        );

        let total: usize = levels.iter().map(|l| l.len()).sum();
        let staging = Buffer::new(
            gpu,
            total as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        let mut regions = Vec::new();
        let mut offset = 0usize;
        for (i, level) in levels.iter().enumerate() {
            staging.write(offset, level.as_slice());
            regions.push(vk::BufferImageCopy {
                buffer_offset: offset as u64,
                buffer_row_length: 0,
                buffer_image_height: 0,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i as u32,
                    base_array_layer: 0,
                    layer_count: layers,
                },
                image_offset: vk::Offset3D::default(),
                image_extent: vk::Extent3D {
                    width: (width >> i).max(1),
                    height: (height >> i).max(1),
                    depth: 1,
                },
            });
            offset += level.len();
        }

        let range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: mips,
            base_array_layer: 0,
            layer_count: layers,
        };
        gpu.immediate(|cmd| unsafe {
            let d = &gpu.device;
            let to_dst = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image.handle)
                .subresource_range(range)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
            d.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_dst],
            );
            d.cmd_copy_buffer_to_image(
                cmd,
                staging.handle,
                image.handle,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
            let to_read = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image.handle)
                .subresource_range(range)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ);
            d.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_read],
            );
        });
        staging.destroy(&gpu.device);

        let info = match kind {
            SamplerKind::Blocks => {
                let mut i = vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::REPEAT)
                    .address_mode_w(vk::SamplerAddressMode::REPEAT)
                    .min_lod(0.0)
                    .max_lod(mips as f32);
                if let Some(a) = gpu.max_anisotropy {
                    i = i.anisotropy_enable(true).max_anisotropy(a);
                }
                i
            }
            SamplerKind::Font => vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .min_lod(0.0)
                .max_lod(0.0),
        };
        let sampler = unsafe {
            gpu.device
                .create_sampler(&info, None)
                .expect("create sampler")
        };
        Self { image, sampler }
    }

    pub fn destroy(&self, device: &Device) {
        unsafe { device.destroy_sampler(self.sampler, None) };
        self.image.destroy(device);
    }
}
