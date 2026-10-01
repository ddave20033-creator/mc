//! Render pipelines from a short description (shaders, vertex layout, blending, depth,
//! culling), and their layouts.

#[derive(Clone, Copy)]
pub struct PipelineDesc<'a> {
    /// The vertex shader's module and entry point, and the fragment shader's.
    pub vert: (&'a wgpu::ShaderModule, &'a str),
    pub frag: (&'a wgpu::ShaderModule, &'a str),
    pub stride: u64,
    /// (format, offset) per vertex attribute; locations are assigned in order.
    /// Empty = no vertex input (e.g. fullscreen triangle).
    pub attributes: &'a [(wgpu::VertexFormat, u64)],
    /// A second vertex buffer read per instance (after the attributes): 16 bytes, four
    /// signed ints (a chunk mesh's head, see `render::chunks::ChunkVertex`).
    pub instance: bool,
    pub layout: &'a wgpu::PipelineLayout,
    pub topology: wgpu::PrimitiveTopology,
    pub cull: bool,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend: bool,
    /// Multiplicative blend (`2 * src * dst`) instead of alpha blending, like Minecraft's crumbling layer.
    pub multiply: bool,
    /// The pass's color target (None: depth only).
    pub color: Option<wgpu::TextureFormat>,
    /// The pass's depth target (None: no depth).
    pub depth: Option<wgpu::TextureFormat>,
    /// (constant, slope) depth bias.
    pub depth_bias: Option<(i32, f32)>,
    /// Samples per pixel of the pass (anti-aliasing).
    pub samples: u32,
    /// The fragment's alpha decides how many of the pixel's samples it covers.
    pub alpha_to_coverage: bool,
}

/// A pipeline layout: the bind groups `groups` (in order) and `immediate_size` bytes of
/// immediates for both shader stages.
pub fn create_layout(device: &wgpu::Device, groups: &[&wgpu::BindGroupLayout], immediate_size: u32) -> wgpu::PipelineLayout {
    let groups: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|g| Some(*g)).collect();
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &groups,
        immediate_size,
    })
}

pub fn create_pipeline(device: &wgpu::Device, d: &PipelineDesc) -> wgpu::RenderPipeline {
    let attrs: Vec<wgpu::VertexAttribute> = d
        .attributes
        .iter()
        .enumerate()
        .map(|(i, &(format, offset))| wgpu::VertexAttribute { format, offset, shader_location: i as u32 })
        .collect();
    let head = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Sint32x4,
        offset: 0,
        shader_location: attrs.len() as u32,
    }];
    let mut buffers = Vec::new();
    if !d.attributes.is_empty() {
        buffers.push(Some(wgpu::VertexBufferLayout {
            array_stride: d.stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        }));
    }
    if d.instance {
        buffers.push(Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &head,
        }));
    }
    let blend = if d.multiply {
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Src,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }
    } else {
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }
    };
    let targets = [d.color.map(|format| wgpu::ColorTargetState {
        format,
        blend: d.blend.then_some(blend),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let (bias_constant, bias_slope) = d.depth_bias.unwrap_or((0, 0.0));
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(d.layout),
        vertex: wgpu::VertexState {
            module: d.vert.0,
            entry_point: Some(d.vert.1),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: d.topology,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: d.cull.then_some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: d.depth.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(d.depth_write),
            depth_compare: Some(if d.depth_test {
                wgpu::CompareFunction::LessEqual
            } else {
                wgpu::CompareFunction::Always
            }),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState { constant: bias_constant, slope_scale: bias_slope, clamp: 0.0 },
        }),
        multisample: wgpu::MultisampleState {
            count: d.samples,
            mask: !0,
            alpha_to_coverage_enabled: d.alpha_to_coverage,
        },
        fragment: Some(wgpu::FragmentState {
            module: d.frag.0,
            entry_point: Some(d.frag.1),
            compilation_options: Default::default(),
            targets: if d.color.is_some() { &targets } else { &[] },
        }),
        multiview_mask: None,
        cache: None,
    })
}
