//! The offscreen render targets: the shadow map (the sun's square and the weapon lights'
//! strip under it), the scope's view and the menus' backdrop blurred across, each with its
//! render pass and framebuffer.

use super::frame::{SCOPE_SIZE, SHADOW_HEIGHT, SHADOW_SIZE};
use crate::engine::resources::Image;
use crate::engine::Gpu;
use ash::vk;

const SCOPE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
const SHADOW_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

/// The whole of a `width` x `height` target.
pub(super) fn full_rect(width: u32, height: u32) -> vk::Rect2D {
    vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: vk::Extent2D { width, height },
    }
}

/// The shadow map: one depth image the world shaders sample with comparison.
pub(super) struct ShadowTarget {
    pub image: Image,
    pub sampler: vk::Sampler,
    pub pass: vk::RenderPass,
    fb: vk::Framebuffer,
}

impl ShadowTarget {
    pub unsafe fn new(gpu: &Gpu) -> Self {
        let d = &gpu.device;
        let image = Image::new(
            d,
            &gpu.mem_props,
            SHADOW_SIZE,
            SHADOW_HEIGHT,
            1,
            1,
            SHADOW_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::DEPTH,
            vk::ImageViewType::TYPE_2D,
        );
        let sampler = d
            .create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
                    .compare_enable(true)
                    .compare_op(vk::CompareOp::LESS_OR_EQUAL)
                    .max_lod(0.0),
                None,
            )
            .unwrap();
        let pass = create_shadow_pass(d);
        let views = [image.view];
        let fb = d
            .create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(pass)
                    .attachments(&views)
                    .width(SHADOW_SIZE)
                    .height(SHADOW_HEIGHT)
                    .layers(1),
                None,
            )
            .unwrap();
        Self { image, sampler, pass, fb }
    }

    /// Begins the shadow pass over the whole image, cleared to the far end.
    pub unsafe fn begin(&self, d: &ash::Device, cmd: vk::CommandBuffer) {
        let clear = [vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        }];
        d.cmd_begin_render_pass(
            cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.pass)
                .framebuffer(self.fb)
                .render_area(full_rect(SHADOW_SIZE, SHADOW_HEIGHT))
                .clear_values(&clear),
            vk::SubpassContents::INLINE,
        );
    }

    pub unsafe fn destroy(&self, d: &ash::Device) {
        d.destroy_framebuffer(self.fb, None);
        d.destroy_render_pass(self.pass, None);
        d.destroy_sampler(self.sampler, None);
        self.image.destroy(d);
    }
}

/// The scope's view (picture in picture): a colour image the eyepiece samples afterwards,
/// and a depth image.
pub(super) struct ScopeTarget {
    pub color: Image,
    depth: Image,
    pub sampler: vk::Sampler,
    pub pass: vk::RenderPass,
    fb: vk::Framebuffer,
}

impl ScopeTarget {
    pub unsafe fn new(gpu: &Gpu) -> Self {
        let d = &gpu.device;
        let color = Image::new(
            d,
            &gpu.mem_props,
            SCOPE_SIZE,
            SCOPE_SIZE,
            1,
            1,
            SCOPE_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
            vk::ImageViewType::TYPE_2D,
        );
        let depth = Image::new(
            d,
            &gpu.mem_props,
            SCOPE_SIZE,
            SCOPE_SIZE,
            1,
            1,
            vk::Format::D32_SFLOAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            vk::ImageViewType::TYPE_2D,
        );
        let sampler = d
            .create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.0),
                None,
            )
            .unwrap();
        let pass = create_scope_pass(d);
        let views = [color.view, depth.view];
        let fb = d
            .create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(pass)
                    .attachments(&views)
                    .width(SCOPE_SIZE)
                    .height(SCOPE_SIZE)
                    .layers(1),
                None,
            )
            .unwrap();
        Self {
            color,
            depth,
            sampler,
            pass,
            fb,
        }
    }

    /// Begins the scope pass, cleared to black and the far end.
    pub unsafe fn begin(&self, d: &ash::Device, cmd: vk::CommandBuffer) {
        let clears = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];
        d.cmd_begin_render_pass(
            cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.pass)
                .framebuffer(self.fb)
                .render_area(full_rect(SCOPE_SIZE, SCOPE_SIZE))
                .clear_values(&clears),
            vk::SubpassContents::INLINE,
        );
    }

    pub unsafe fn destroy(&self, d: &ash::Device) {
        d.destroy_framebuffer(self.fb, None);
        d.destroy_render_pass(self.pass, None);
        d.destroy_sampler(self.sampler, None);
        self.color.destroy(d);
        self.depth.destroy(d);
    }
}

/// The menus' backdrop blurred across (the first half of its blur, at the scope image's size):
/// a colour image the backdrop blurs down, as it is drawn over the screen.
pub(super) struct BlurTarget {
    pub color: Image,
    pub pass: vk::RenderPass,
    fb: vk::Framebuffer,
}

impl BlurTarget {
    pub unsafe fn new(gpu: &Gpu) -> Self {
        let d = &gpu.device;
        let color = Image::new(
            d,
            &gpu.mem_props,
            SCOPE_SIZE,
            SCOPE_SIZE,
            1,
            1,
            SCOPE_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
            vk::ImageViewType::TYPE_2D,
        );
        let pass = create_blur_pass(d);
        let views = [color.view];
        let fb = d
            .create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(pass)
                    .attachments(&views)
                    .width(SCOPE_SIZE)
                    .height(SCOPE_SIZE)
                    .layers(1),
                None,
            )
            .unwrap();
        Self { color, pass, fb }
    }

    /// Begins the pass (every pixel is drawn: nothing cleared).
    pub unsafe fn begin(&self, d: &ash::Device, cmd: vk::CommandBuffer) {
        d.cmd_begin_render_pass(
            cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.pass)
                .framebuffer(self.fb)
                .render_area(full_rect(SCOPE_SIZE, SCOPE_SIZE)),
            vk::SubpassContents::INLINE,
        );
    }

    pub unsafe fn destroy(&self, d: &ash::Device) {
        d.destroy_framebuffer(self.fb, None);
        d.destroy_render_pass(self.pass, None);
        self.color.destroy(d);
    }
}

/// The blur's pass: one colour image, read afterwards by the backdrop.
fn create_blur_pass(device: &ash::Device) -> vk::RenderPass {
    let attachments = [vk::AttachmentDescription::default()
        .format(SCOPE_FORMAT)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let color_ref = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)];
    let deps = [
        // The last frame's backdrop has read the image before it is drawn again.
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
        // The image is drawn before the backdrop reads it.
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
            .expect("create blur render pass")
    }
}

/// The scope's pass: a colour image the eyepiece samples afterwards, and a depth image.
fn create_scope_pass(device: &ash::Device) -> vk::RenderPass {
    let attachments = [
        vk::AttachmentDescription::default()
            .format(SCOPE_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        vk::AttachmentDescription::default()
            .format(vk::Format::D32_SFLOAT)
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
    let fragment_tests =
        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
    let deps = [
        // The last frame's eyepiece has read the image (and its depth is written) before it
        // is drawn again.
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | fragment_tests)
            .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | fragment_tests)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            ),
        // The image is drawn before the eyepiece reads it.
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
            .expect("create scope render pass")
    }
}

fn create_shadow_pass(device: &ash::Device) -> vk::RenderPass {
    let attachments = [vk::AttachmentDescription::default()
        .format(SHADOW_FORMAT)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
    let depth_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_ref)];
    let deps = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_stage_mask(
                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            ),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
            .expect("create shadow render pass")
    }
}
