use ash::{vk, Device};

#[derive(Clone, Copy)]
pub struct PipelineDesc<'a> {
    pub vert: &'a [u8],
    pub frag: &'a [u8],
    pub stride: u32,
    /// (format, offset) per vertex attribute; locations are assigned in order.
    /// Empty = no vertex input (e.g. fullscreen triangle).
    pub attributes: &'a [(vk::Format, u32)],
    pub layout: vk::PipelineLayout,
    pub render_pass: vk::RenderPass,
    pub topology: vk::PrimitiveTopology,
    pub cull: bool,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend: bool,
    /// Multiplicative blend (`2 * src * dst`) instead of alpha blending, like Minecraft's crumbling layer.
    pub multiply: bool,
    /// Whether the render pass has a color attachment.
    pub color: bool,
    /// (constant, slope) depth bias.
    pub depth_bias: Option<(f32, f32)>,
}

pub fn create_layout(
    device: &Device,
    sets: &[vk::DescriptorSetLayout],
    push_size: u32,
) -> vk::PipelineLayout {
    let ranges = [vk::PushConstantRange {
        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        offset: 0,
        size: push_size,
    }];
    unsafe {
        device
            .create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(sets)
                    .push_constant_ranges(&ranges),
                None,
            )
            .expect("create pipeline layout")
    }
}

fn shader_module(device: &Device, bytes: &[u8]) -> vk::ShaderModule {
    let code = ash::util::read_spv(&mut std::io::Cursor::new(bytes)).expect("invalid SPIR-V");
    unsafe {
        device
            .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
            .expect("create shader module")
    }
}

pub fn create_pipeline(device: &Device, d: &PipelineDesc) -> vk::Pipeline {
    unsafe {
        let vs = shader_module(device, d.vert);
        let fs = shader_module(device, d.frag);
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fs)
                .name(c"main"),
        ];
        let bindings: Vec<vk::VertexInputBindingDescription> = if d.attributes.is_empty() {
            Vec::new()
        } else {
            vec![vk::VertexInputBindingDescription {
                binding: 0,
                stride: d.stride,
                input_rate: vk::VertexInputRate::VERTEX,
            }]
        };
        let attrs: Vec<vk::VertexInputAttributeDescription> = d
            .attributes
            .iter()
            .enumerate()
            .map(
                |(i, &(format, offset))| vk::VertexInputAttributeDescription {
                    location: i as u32,
                    binding: 0,
                    format,
                    offset,
                },
            )
            .collect();
        let vi = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attrs);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(d.topology);
        let vp = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let (bias_c, bias_s) = d.depth_bias.unwrap_or((0.0, 0.0));
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(if d.cull {
                vk::CullModeFlags::BACK
            } else {
                vk::CullModeFlags::NONE
            })
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(d.depth_bias.is_some())
            .depth_bias_constant_factor(bias_c)
            .depth_bias_slope_factor(bias_s)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(d.depth_test)
            .depth_write_enable(d.depth_write)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
        let (src_c, dst_c, src_a, dst_a) = if d.multiply {
            (
                vk::BlendFactor::DST_COLOR,
                vk::BlendFactor::SRC_COLOR,
                vk::BlendFactor::ZERO,
                vk::BlendFactor::ONE,
            )
        } else {
            (
                vk::BlendFactor::SRC_ALPHA,
                vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                vk::BlendFactor::ONE,
                vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            )
        };
        let blend = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(d.blend)
            .src_color_blend_factor(src_c)
            .dst_color_blend_factor(dst_c)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(src_a)
            .dst_alpha_blend_factor(dst_a)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let cb = if d.color {
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend)
        } else {
            vk::PipelineColorBlendStateCreateInfo::default()
        };
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vi)
            .input_assembly_state(&ia)
            .viewport_state(&vp)
            .rasterization_state(&rs)
            .multisample_state(&ms)
            .depth_stencil_state(&ds)
            .color_blend_state(&cb)
            .dynamic_state(&dy)
            .layout(d.layout)
            .render_pass(d.render_pass)
            .subpass(0);
        let pipeline = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
            .map_err(|(_, e)| e)
            .expect("create graphics pipeline")[0];
        device.destroy_shader_module(vs, None);
        device.destroy_shader_module(fs, None);
        pipeline
    }
}
