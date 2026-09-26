use super::resources::{Buffer, Image};
use ash::ext::debug_utils;
use ash::khr::{surface, swapchain};
use ash::{vk, Device, Entry, Instance};
use std::ffi::{c_char, c_void, CStr};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub const FRAMES_IN_FLIGHT: usize = 2;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

struct Frame {
    cmd: vk::CommandBuffer,
    image_available: vk::Semaphore,
    fence: vk::Fence,
}

/// Owns the Vulkan instance, device, swapchain and per-frame synchronization.
pub struct Gpu {
    _entry: Entry,
    pub instance: Instance,
    debug: Option<(debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    surface_loader: surface::Instance,
    surface: vk::SurfaceKHR,
    pub physical: vk::PhysicalDevice,
    pub device: Device,
    pub queue: vk::Queue,
    pub mem_props: vk::PhysicalDeviceMemoryProperties,
    pub max_anisotropy: Option<f32>,
    pub device_name: String,
    /// VK_EXT_memory_budget is enabled (video memory use can be read).
    memory_budget: bool,
    /// Nanoseconds per GPU timestamp tick; None if the queue cannot write timestamps.
    pub timestamp_period: Option<f32>,
    /// Time the last begin_frame spent waiting for the GPU and the swapchain, in ms.
    pub wait_ms: f32,

    swapchain_loader: swapchain::Device,
    swapchain: vk::SwapchainKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub extent: vk::Extent2D,
    swap_views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    render_finished: Vec<vk::Semaphore>,
    depth: Option<Image>,
    pub render_pass: vk::RenderPass,

    command_pool: vk::CommandPool,
    frames: Vec<Frame>,
    pub frame_slot: usize,
    frame_counter: u64,
    garbage: Vec<(u64, Buffer)>,

    desired_extent: vk::Extent2D,
    needs_recreate: bool,
    vsync: bool,
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _types: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user: *mut c_void,
) -> vk::Bool32 {
    if !data.is_null() && !(*data).p_message.is_null() {
        let msg = CStr::from_ptr((*data).p_message).to_string_lossy();
        eprintln!("[Vulkan {severity:?}] {msg}");
    }
    vk::FALSE
}

impl Gpu {
    pub fn new(window: &Window, vsync: bool) -> Self {
        unsafe {
            let entry =
                Entry::load().expect("failed to load Vulkan - is a Vulkan driver installed?");
            let display = window.display_handle().expect("display handle").as_raw();
            let whandle = window.window_handle().expect("window handle").as_raw();

            let validation = cfg!(debug_assertions)
                && entry
                    .enumerate_instance_layer_properties()
                    .unwrap_or_default()
                    .iter()
                    .any(|l| l.layer_name_as_c_str() == Ok(VALIDATION_LAYER));

            let mut extensions: Vec<*const c_char> =
                ash_window::enumerate_required_extensions(display)
                    .expect("required surface extensions")
                    .to_vec();
            if validation {
                extensions.push(debug_utils::NAME.as_ptr());
            }
            let layers: Vec<*const c_char> = if validation {
                vec![VALIDATION_LAYER.as_ptr()]
            } else {
                vec![]
            };

            let app_info = vk::ApplicationInfo::default()
                .application_name(c"RustCraft")
                .application_version(1)
                .engine_name(c"RustCraft Engine")
                .engine_version(1)
                .api_version(vk::API_VERSION_1_2);
            let instance = entry
                .create_instance(
                    &vk::InstanceCreateInfo::default()
                        .application_info(&app_info)
                        .enabled_extension_names(&extensions)
                        .enabled_layer_names(&layers),
                    None,
                )
                .expect("create Vulkan instance");

            let debug = if validation {
                let loader = debug_utils::Instance::new(&entry, &instance);
                let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                    )
                    .pfn_user_callback(Some(debug_callback));
                let messenger = loader.create_debug_utils_messenger(&info, None).ok();
                messenger.map(|m| (loader, m))
            } else {
                None
            };

            let surface_khr = ash_window::create_surface(&entry, &instance, display, whandle, None)
                .expect("create window surface");
            let surface_loader = surface::Instance::new(&entry, &instance);

            // Pick the best GPU: discrete > integrated > anything, with graphics+present and swapchain.
            let mut best: Option<(vk::PhysicalDevice, u32, i32)> = None;
            for pd in instance
                .enumerate_physical_devices()
                .expect("enumerate GPUs")
            {
                let props = instance.get_physical_device_properties(pd);
                let has_swapchain = instance
                    .enumerate_device_extension_properties(pd)
                    .unwrap_or_default()
                    .iter()
                    .any(|e| e.extension_name_as_c_str() == Ok(swapchain::NAME));
                if !has_swapchain {
                    continue;
                }
                let families = instance.get_physical_device_queue_family_properties(pd);
                let family = (0..families.len()).find(|&i| {
                    families[i].queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        && surface_loader
                            .get_physical_device_surface_support(pd, i as u32, surface_khr)
                            .unwrap_or(false)
                });
                let Some(family) = family else { continue };
                let score = match props.device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                    vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                    _ => 0,
                };
                if best.is_none_or(|b| score > b.2) {
                    best = Some((pd, family as u32, score));
                }
            }
            let (physical, queue_family, _) =
                best.expect("no Vulkan GPU with presentation support found");
            let props = instance.get_physical_device_properties(physical);
            let timestamp_period = (instance.get_physical_device_queue_family_properties(physical)
                [queue_family as usize]
                .timestamp_valid_bits
                > 0)
            .then_some(props.limits.timestamp_period);
            let device_name = props
                .device_name_as_c_str()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "Unknown GPU".into());
            let features = instance.get_physical_device_features(physical);
            let anisotropy = features.sampler_anisotropy == vk::TRUE;
            let max_anisotropy = anisotropy.then(|| props.limits.max_sampler_anisotropy.min(16.0));

            let priorities = [1.0f32];
            let queue_infos = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family)
                .queue_priorities(&priorities)];
            let enabled = vk::PhysicalDeviceFeatures::default().sampler_anisotropy(anisotropy);
            let memory_budget = instance
                .enumerate_device_extension_properties(physical)
                .unwrap_or_default()
                .iter()
                .any(|e| e.extension_name_as_c_str() == Ok(ash::ext::memory_budget::NAME));
            let mut device_exts = vec![swapchain::NAME.as_ptr()];
            if memory_budget {
                device_exts.push(ash::ext::memory_budget::NAME.as_ptr());
            }
            let device = instance
                .create_device(
                    physical,
                    &vk::DeviceCreateInfo::default()
                        .queue_create_infos(&queue_infos)
                        .enabled_extension_names(&device_exts)
                        .enabled_features(&enabled),
                    None,
                )
                .expect("create logical device");
            let queue = device.get_device_queue(queue_family, 0);
            let mem_props = instance.get_physical_device_memory_properties(physical);
            let swapchain_loader = swapchain::Device::new(&instance, &device);

            let formats = surface_loader
                .get_physical_device_surface_formats(physical, surface_khr)
                .expect("surface formats");
            let surface_format = formats
                .iter()
                .copied()
                .find(|f| {
                    (f.format == vk::Format::B8G8R8A8_SRGB || f.format == vk::Format::R8G8B8A8_SRGB)
                        && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
                .unwrap_or(formats[0]);
            let render_pass = create_render_pass(&device, surface_format.format);

            let command_pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                        .queue_family_index(queue_family),
                    None,
                )
                .expect("create command pool");
            let cmds = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(FRAMES_IN_FLIGHT as u32),
                )
                .expect("allocate command buffers");
            let frames = cmds
                .into_iter()
                .map(|cmd| Frame {
                    cmd,
                    image_available: device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .unwrap(),
                    fence: device
                        .create_fence(
                            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                            None,
                        )
                        .unwrap(),
                })
                .collect();

            let size = window.inner_size();
            let mut gpu = Gpu {
                _entry: entry,
                instance,
                debug,
                surface_loader,
                surface: surface_khr,
                physical,
                device,
                queue,
                mem_props,
                max_anisotropy,
                device_name,
                memory_budget,
                timestamp_period,
                wait_ms: 0.0,
                swapchain_loader,
                swapchain: vk::SwapchainKHR::null(),
                surface_format,
                extent: vk::Extent2D {
                    width: size.width,
                    height: size.height,
                },
                swap_views: Vec::new(),
                framebuffers: Vec::new(),
                render_finished: Vec::new(),
                depth: None,
                render_pass,
                command_pool,
                frames,
                frame_slot: 0,
                frame_counter: 0,
                garbage: Vec::new(),
                desired_extent: vk::Extent2D {
                    width: size.width,
                    height: size.height,
                },
                needs_recreate: false,
                vsync,
            };
            gpu.build_swapchain();
            gpu
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.desired_extent = vk::Extent2D { width, height };
        self.needs_recreate = true;
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        if self.vsync != vsync {
            self.vsync = vsync;
            self.needs_recreate = true;
        }
    }

    /// Video memory (device-local heaps) used by this program and the budget the driver gives
    /// it, in bytes; None without VK_EXT_memory_budget.
    pub fn vram_usage(&self) -> Option<(u64, u64)> {
        if !self.memory_budget {
            return None;
        }
        let mut budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let mut props = vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
        // SAFETY: the extension is enabled and the chain is well formed.
        unsafe {
            self.instance
                .get_physical_device_memory_properties2(self.physical, &mut props)
        };
        let heaps = &self.mem_props.memory_heaps[..self.mem_props.memory_heap_count as usize];
        let local = |i: &usize| heaps[*i].flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL);
        let (mut used, mut total) = (0, 0);
        for i in (0..heaps.len()).filter(local) {
            used += budget.heap_usage[i];
            total += budget.heap_budget[i];
        }
        Some((used, total))
    }

    /// Destroy a buffer once every frame that may still use it has finished.
    pub fn defer_destroy(&mut self, buffer: Buffer) {
        self.garbage.push((self.frame_counter, buffer));
    }

    /// Record and synchronously execute a one-off command buffer.
    pub fn immediate<F: FnOnce(vk::CommandBuffer)>(&self, f: F) {
        unsafe {
            let cmd = self
                .device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(self.command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .unwrap()[0];
            self.device
                .begin_command_buffer(
                    cmd,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
            f(cmd);
            self.device.end_command_buffer(cmd).unwrap();
            let cmds = [cmd];
            self.device
                .queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default().command_buffers(&cmds)],
                    vk::Fence::null(),
                )
                .unwrap();
            self.device.queue_wait_idle(self.queue).unwrap();
            self.device.free_command_buffers(self.command_pool, &cmds);
        }
    }

    unsafe fn destroy_swapchain_resources(&mut self) {
        for fb in self.framebuffers.drain(..) {
            self.device.destroy_framebuffer(fb, None);
        }
        for v in self.swap_views.drain(..) {
            self.device.destroy_image_view(v, None);
        }
        for s in self.render_finished.drain(..) {
            self.device.destroy_semaphore(s, None);
        }
        if let Some(depth) = self.depth.take() {
            depth.destroy(&self.device);
        }
    }

    unsafe fn build_swapchain(&mut self) {
        let caps = self
            .surface_loader
            .get_physical_device_surface_capabilities(self.physical, self.surface)
            .expect("surface capabilities");
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: self
                    .desired_extent
                    .width
                    .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: self
                    .desired_extent
                    .height
                    .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        self.extent = extent;
        if extent.width == 0 || extent.height == 0 {
            // Minimized: try again later.
            self.needs_recreate = true;
            return;
        }

        let modes = self
            .surface_loader
            .get_physical_device_surface_present_modes(self.physical, self.surface)
            .unwrap_or_default();
        let present_mode = if self.vsync {
            vk::PresentModeKHR::FIFO
        } else if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            vk::PresentModeKHR::IMMEDIATE
        } else if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        };
        let mut image_count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            image_count = image_count.min(caps.max_image_count);
        }
        let composite = if caps
            .supported_composite_alpha
            .contains(vk::CompositeAlphaFlagsKHR::OPAQUE)
        {
            vk::CompositeAlphaFlagsKHR::OPAQUE
        } else {
            vk::CompositeAlphaFlagsKHR::INHERIT
        };

        let old = self.swapchain;
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(self.surface_format.format)
            .image_color_space(self.surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old);
        self.swapchain = self
            .swapchain_loader
            .create_swapchain(&info, None)
            .expect("create swapchain");
        if old != vk::SwapchainKHR::null() {
            self.swapchain_loader.destroy_swapchain(old, None);
        }

        let images = self
            .swapchain_loader
            .get_swapchain_images(self.swapchain)
            .unwrap();
        let depth = Image::new(
            &self.device,
            &self.mem_props,
            extent.width,
            extent.height,
            1,
            1,
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            vk::ImageViewType::TYPE_2D,
        );
        for &image in &images {
            let view = self
                .device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.surface_format.format)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                    None,
                )
                .unwrap();
            let attachments = [view, depth.view];
            let fb = self
                .device
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(self.render_pass)
                        .attachments(&attachments)
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1),
                    None,
                )
                .unwrap();
            self.swap_views.push(view);
            self.framebuffers.push(fb);
            self.render_finished.push(
                self.device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap(),
            );
        }
        self.depth = Some(depth);
    }

    fn recreate(&mut self) {
        unsafe {
            self.device.device_wait_idle().ok();
            self.needs_recreate = false;
            self.destroy_swapchain_resources();
            self.build_swapchain();
        }
    }

    /// Waits for this frame slot, acquires a swapchain image and begins the command buffer.
    pub fn begin_frame(&mut self) -> Option<(vk::CommandBuffer, u32)> {
        unsafe {
            let (cmd, sem, fence) = {
                let f = &self.frames[self.frame_slot];
                (f.cmd, f.image_available, f.fence)
            };
            let wait_start = std::time::Instant::now();
            self.device
                .wait_for_fences(&[fence], true, u64::MAX)
                .unwrap();
            self.wait_ms = wait_start.elapsed().as_secs_f32() * 1000.0;

            let now = self.frame_counter;
            let device = &self.device;
            self.garbage.retain(|(frame, buffer)| {
                if now >= frame + FRAMES_IN_FLIGHT as u64 {
                    buffer.destroy(device);
                    false
                } else {
                    true
                }
            });

            if self.needs_recreate {
                self.recreate();
                if self.needs_recreate {
                    return None;
                }
            }

            let image = match self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                sem,
                vk::Fence::null(),
            ) {
                Ok((i, suboptimal)) => {
                    if suboptimal {
                        self.needs_recreate = true;
                    }
                    i
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.needs_recreate = true;
                    return None;
                }
                Err(e) => panic!("acquire_next_image failed: {e:?}"),
            };
            self.wait_ms = wait_start.elapsed().as_secs_f32() * 1000.0;

            self.device.reset_fences(&[fence]).unwrap();
            self.device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.device
                .begin_command_buffer(
                    cmd,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
            Some((cmd, image))
        }
    }

    pub fn begin_render_pass(&self, cmd: vk::CommandBuffer, image: u32, clear: [f32; 4]) {
        unsafe {
            let clears = [
                vk::ClearValue {
                    color: vk::ClearColorValue { float32: clear },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            let area = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.extent,
            };
            self.device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(self.framebuffers[image as usize])
                    .render_area(area)
                    .clear_values(&clears),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.extent.width as f32,
                    height: self.extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(cmd, 0, &[area]);
        }
    }

    /// Ends the render pass, submits and presents.
    pub fn end_frame(&mut self, cmd: vk::CommandBuffer, image: u32) {
        unsafe {
            let (sem, fence) = {
                let f = &self.frames[self.frame_slot];
                (f.image_available, f.fence)
            };
            self.device.cmd_end_render_pass(cmd);
            self.device.end_command_buffer(cmd).unwrap();

            let wait = [sem];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cmds = [cmd];
            let signal = [self.render_finished[image as usize]];
            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait)
                .wait_dst_stage_mask(&stages)
                .command_buffers(&cmds)
                .signal_semaphores(&signal);
            self.device
                .queue_submit(self.queue, &[submit], fence)
                .expect("queue submit");

            let swapchains = [self.swapchain];
            let indices = [image];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal)
                .swapchains(&swapchains)
                .image_indices(&indices);
            match self.swapchain_loader.queue_present(self.queue, &present) {
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.needs_recreate = true,
                Ok(false) => {}
                Err(e) => panic!("queue_present failed: {e:?}"),
            }

            self.frame_slot = (self.frame_slot + 1) % FRAMES_IN_FLIGHT;
            self.frame_counter += 1;
        }
    }
}

fn create_render_pass(device: &Device, color_format: vk::Format) -> vk::RenderPass {
    let attachments = [
        vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
        vk::AttachmentDescription::default()
            .format(DEPTH_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    ];
    let color_ref = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let depth_ref = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)
        .depth_stencil_attachment(&depth_ref)];
    let deps = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        )
        .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        )];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
            .expect("create render pass")
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().ok();
            for (_, b) in self.garbage.drain(..) {
                b.destroy(&self.device);
            }
            self.destroy_swapchain_resources();
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            for f in &self.frames {
                self.device.destroy_semaphore(f.image_available, None);
                self.device.destroy_fence(f.fence, None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            if let Some((loader, messenger)) = self.debug.take() {
                loader.destroy_debug_utils_messenger(messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}
