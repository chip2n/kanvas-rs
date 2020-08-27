pub struct DepthConfig {
    format: wgpu::TextureFormat,
    bias: i32,
    bias_slope_scale: f32,
    bias_clamp: f32,
}

impl DepthConfig {
    pub fn no_bias() -> Self {
        DepthConfig {
            format: wgpu::TextureFormat::Depth32Float,
            bias: 0,
            bias_slope_scale: 0.0,
            bias_clamp: 0.0,
        }
    }
}

impl Default for DepthConfig {
    fn default() -> Self {
        DepthConfig {
            format: wgpu::TextureFormat::Depth32Float,
            bias: 2, // corresponds to bilinear filtering
            bias_slope_scale: 2.0,
            bias_clamp: 0.0,
        }
    }
}

pub fn create(
    name: &str,
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    vs_module: &wgpu::ShaderModule,
    fs_module: &wgpu::ShaderModule,
    color_format: Option<wgpu::TextureFormat>,
    depth_config: Option<DepthConfig>,
    vertex_descs: &[wgpu::VertexBufferDescriptor],
) -> wgpu::RenderPipeline {
    let mut rasterization_state = wgpu::RasterizationStateDescriptor {
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: wgpu::CullMode::Back,
        ..Default::default()
    };
    if let Some(config) = &depth_config {
        rasterization_state.depth_bias = config.bias;
        rasterization_state.depth_bias_slope_scale = config.bias_slope_scale;
        rasterization_state.depth_bias_clamp = config.bias_clamp;
        rasterization_state.clamp_depth = device.features().contains(wgpu::Features::DEPTH_CLAMPING);
    }

    let color_states: Vec<wgpu::ColorStateDescriptor> = match color_format {
        Some(format) => vec!(wgpu::ColorStateDescriptor {
            format,
            color_blend: wgpu::BlendDescriptor::REPLACE,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            write_mask: wgpu::ColorWrite::ALL,
        }), 
        None => vec!(),
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(name),
        layout: Some(&layout),
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        // description of how to process triangles
        rasterization_state: Some(rasterization_state),
        // description on how color are stored and processed throughout the pipeline
        color_states: &color_states,
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        depth_stencil_state: depth_config.map(|config| wgpu::DepthStencilStateDescriptor {
            format: config.format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilStateDescriptor::default(),
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: vertex_descs,
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}
