//! The wgpu device and the window's surface, the depth and multisample targets, and the frames
//! (acquiring the surface's image, the main pass drawing into it, submitting and presenting,
//! screenshots of what was presented).

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Instant;
use winit::window::Window;

/// Frames the CPU may prepare ahead of the GPU (the surface's frame latency; the renderer's
/// timestamps keep one set per frame slot).
pub const FRAMES_IN_FLIGHT: usize = 2;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

/// Runs a wgpu future to its end (on the native backends they are ready at once, or after a
/// `Device::poll`).
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// One frame being recorded: the surface's image it ends up in and the commands.
pub struct Frame {
    surface: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

/// Owns the device, the window's surface and the main pass's targets.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
    pub extent: Extent,
    /// Indirect draws may start at an instance other than 0 (the chunk meshes' heads).
    pub indirect_first_instance: bool,
    /// Samplers may filter anisotropically.
    pub anisotropy: bool,
    /// The shadow map's sampler can give "lit" outside the image (else its edge is repeated).
    pub clamp_to_border: bool,
    pub device_name: String,
    /// Nanoseconds per GPU timestamp tick; None if timestamps cannot be written inside passes.
    pub timestamp_period: Option<f32>,
    /// Time the last begin_frame spent waiting for the GPU and the surface, in ms.
    pub wait_ms: f32,
    /// The surface's images can be copied out (screenshots).
    can_capture: bool,
    /// Save the next presented frame to this PNG file.
    pub capture: Option<std::path::PathBuf>,
    depth: wgpu::TextureView,
    /// The multisampled color target (with anti-aliasing), resolved into the surface's image.
    msaa_color: Option<wgpu::TextureView>,
    /// Samples per pixel of the main pass (anti-aliasing), and the counts the GPU can draw with.
    pub samples: u32,
    pub max_samples: u32,
    sample_counts: Vec<u32>,
    /// Changes when the main pass's sample count does (pipelines made for it must be remade).
    pub pass_version: u64,
    pub frame_slot: usize,
    desired_extent: Extent,
    needs_recreate: bool,
}

impl Gpu {
    /// `msaa`: samples per pixel for anti-aliasing (1 = off; lowered to what the GPU supports).
    pub fn new(window: Arc<Window>, vsync: bool, msaa: u32) -> Self {
        let mut flags = wgpu::InstanceFlags::from_build_config();
        // The indirect draws are the renderer's own (chunk ranges it allocated): checking them
        // on the GPU every frame costs time for nothing (WGPU_VALIDATION_INDIRECT_CALL=1 turns
        // it on again).
        if !cfg!(debug_assertions) {
            flags.remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
        }
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::PRIMARY;
        desc.flags = flags;
        // (WGPU_BACKEND=vulkan|dx12, WGPU_VALIDATION=0... override these)
        let instance = wgpu::Instance::new(desc.with_env());
        let surface = instance.create_surface(window.clone()).expect("create window surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .expect("no GPU with presentation support found (Vulkan, DirectX 12 or Metal)");
        let info = adapter.get_info();
        let device_name = format!("{} ({:?})", info.name, info.backend);

        let available = adapter.features();
        assert!(
            available.contains(wgpu::Features::IMMEDIATES),
            "the GPU ({device_name}) has no immediates (push constants)"
        );
        let timestamps = wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        let wanted = wgpu::Features::IMMEDIATES
            | wgpu::Features::INDIRECT_FIRST_INSTANCE
            | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | timestamps;
        let features = available & wanted;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("RustCraft"),
            required_features: features,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("create GPU device");
        let timestamp_period = features.contains(timestamps).then(|| queue.get_timestamp_period());

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| matches!(f, wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb))
            .unwrap_or(caps.formats[0]);
        let can_capture = caps.usages.contains(wgpu::TextureUsages::COPY_SRC);
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("the surface does not work with this GPU");
        config.format = surface_format;
        config.usage = if can_capture {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        config.present_mode = present_mode(&caps.present_modes, vsync);
        config.alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        // RUSTCRAFT_IMAGES=n: as many images in the swapchain (to compare, e.g. with --bench).
        let images = std::env::var("RUSTCRAFT_IMAGES").ok().and_then(|v| v.parse::<u32>().ok());
        config.desired_maximum_frame_latency = images.map_or(FRAMES_IN_FLIGHT as u32, |n| n.saturating_sub(1).max(1));
        config.view_formats = Vec::new();

        // Sample counts both the colour and the depth target can be drawn with.
        let counts = |f: wgpu::TextureFormat| adapter.get_texture_format_features(f).flags;
        let (color_counts, depth_counts) = (counts(surface_format), counts(DEPTH_FORMAT));
        let sample_counts: Vec<u32> = [1, 2, 4, 8]
            .into_iter()
            .filter(|&n| n == 1 || color_counts.sample_count_supported(n) && depth_counts.sample_count_supported(n))
            .collect();
        let max_samples = *sample_counts.last().unwrap();
        let samples = pick_samples(&sample_counts, msaa);
        let extent = Extent { width: size.width, height: size.height };
        let (depth, msaa_color) = targets(&device, extent, surface_format, samples);

        let mut gpu = Gpu {
            indirect_first_instance: features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE),
            anisotropy: adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING),
            clamp_to_border: features.contains(wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER),
            device,
            queue,
            surface,
            config,
            surface_format,
            extent,
            device_name,
            timestamp_period,
            wait_ms: 0.0,
            can_capture,
            capture: None,
            depth,
            msaa_color,
            samples,
            max_samples,
            sample_counts,
            pass_version: 0,
            frame_slot: 0,
            desired_extent: extent,
            needs_recreate: true,
        };
        gpu.recreate();
        gpu
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.desired_extent = Extent { width, height };
        self.needs_recreate = true;
    }

    /// Anti-aliasing: samples per pixel (1, 2, 4 or 8; at most what the GPU supports).
    pub fn set_msaa(&mut self, msaa: u32) {
        let samples = pick_samples(&self.sample_counts, msaa);
        if samples != self.samples {
            self.samples = samples;
            self.pass_version += 1;
            self.needs_recreate = true;
        }
    }

    /// GPU memory the allocator has handed out to this program and the memory it holds for
    /// that, in bytes; None where the backend does not tell.
    pub fn vram_usage(&self) -> Option<(u64, u64)> {
        self.device
            .generate_allocator_report()
            .map(|r| (r.total_allocated_bytes, r.total_reserved_bytes))
    }

    /// The surface and the targets at the window's size (none while it is minimized).
    fn recreate(&mut self) {
        let Extent { width, height } = self.desired_extent;
        self.extent = self.desired_extent;
        if width == 0 || height == 0 {
            // Minimized: try again later.
            return;
        }
        self.needs_recreate = false;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (depth, msaa_color) = targets(&self.device, self.extent, self.surface_format, self.samples);
        self.depth = depth;
        self.msaa_color = msaa_color;
    }

    /// Acquires the surface's next image and begins recording; None when there is nothing to
    /// draw into (minimized, or the surface is being remade).
    pub fn begin_frame(&mut self) -> Option<Frame> {
        if self.needs_recreate {
            self.recreate();
            if self.needs_recreate {
                return None;
            }
        }
        let wait_start = Instant::now();
        let surface = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                self.needs_recreate = true;
                t
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return None,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.needs_recreate = true;
                return None;
            }
            wgpu::CurrentSurfaceTexture::Validation => panic!("acquiring the surface's image failed"),
        };
        self.wait_ms = wait_start.elapsed().as_secs_f32() * 1000.0;
        let view = surface.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        Some(Frame { surface, view, encoder })
    }

    /// Begins (a part of) the main pass: color and depth, the depth cleared; the color cleared
    /// to `clear` or kept from the part before. With anti-aliasing the multisampled color is
    /// resolved into the surface's image at the end of the `last` part.
    pub fn main_pass(&self, frame: &mut Frame, clear: Option<[f32; 4]>, last: bool) -> wgpu::RenderPass<'static> {
        let (view, resolve_target, store) = match &self.msaa_color {
            Some(color) if last => (color, Some(&frame.view), wgpu::StoreOp::Discard),
            Some(color) => (color, None, wgpu::StoreOp::Store),
            None => (&frame.view, None, wgpu::StoreOp::Store),
        };
        let load = match clear {
            Some([r, g, b, a]) => wgpu::LoadOp::Clear(wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: a as f64 }),
            None => wgpu::LoadOp::Load,
        };
        frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target,
                    ops: wgpu::Operations { load, store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                ..Default::default()
            })
            .forget_lifetime()
    }

    /// Submits the frame and presents it (saving it first if a screenshot was asked for).
    pub fn end_frame(&mut self, frame: Frame) {
        let Frame { surface, view, mut encoder } = frame;
        drop(view);
        let shot = if self.can_capture { self.capture.take() } else { None };
        let shot = shot.map(|path| {
            let (w, h) = (self.extent.width, self.extent.height);
            let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("screenshot"),
                size: row as u64 * h as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                surface.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buf,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            (path, buf, row)
        });
        self.queue.submit([encoder.finish()]);
        self.queue.present(surface);
        if let Some((path, buf, row)) = shot {
            buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
            if let Ok(data) = buf.slice(..).get_mapped_range() {
                self.save_png(&path, &data, row as usize);
            }
        }
        self.frame_slot = (self.frame_slot + 1) % FRAMES_IN_FLIGHT;
    }

    fn save_png(&self, path: &std::path::Path, data: &[u8], row: usize) {
        let (w, h) = (self.extent.width as usize, self.extent.height as usize);
        let bgr = matches!(
            self.surface_format,
            wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm
        );
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for p in data[y * row..y * row + w * 4].chunks_exact(4) {
                if bgr {
                    rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
                } else {
                    rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
                }
            }
        }
        let Ok(file) = std::fs::File::create(path) else {
            return;
        };
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        if let Ok(mut wr) = e.write_header() {
            let _ = wr.write_image_data(&rgba);
        }
    }
}

/// Without vsync: mailbox (no tearing, no cap) if there is one, else immediate.
/// RUSTCRAFT_PRESENT=immediate|mailbox|fifo: to compare (e.g. with --bench).
fn present_mode(modes: &[wgpu::PresentMode], vsync: bool) -> wgpu::PresentMode {
    use wgpu::PresentMode::{Fifo, Immediate, Mailbox};
    let forced = std::env::var("RUSTCRAFT_PRESENT").ok().and_then(|m| match m.as_str() {
        "immediate" => Some(Immediate),
        "mailbox" => Some(Mailbox),
        "fifo" => Some(Fifo),
        _ => None,
    });
    if let Some(m) = forced.filter(|m| modes.contains(m)) {
        m
    } else if vsync {
        Fifo
    } else if modes.contains(&Mailbox) {
        Mailbox
    } else if modes.contains(&Immediate) {
        Immediate
    } else {
        Fifo
    }
}

/// The most samples per pixel of `counts` (what the GPU can do) up to `msaa`.
fn pick_samples(counts: &[u32], msaa: u32) -> u32 {
    counts.iter().copied().filter(|&n| n <= msaa.max(1)).max().unwrap_or(1)
}

/// The main pass's depth target, and its multisampled color target (with anti-aliasing).
fn targets(
    device: &wgpu::Device,
    extent: Extent,
    format: wgpu::TextureFormat,
    samples: u32,
) -> (wgpu::TextureView, Option<wgpu::TextureView>) {
    let target = |label, format| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: extent.width.max(1),
                    height: extent.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let depth = target("depth", DEPTH_FORMAT);
    let color = (samples > 1).then(|| target("msaa color", format));
    (depth, color)
}
