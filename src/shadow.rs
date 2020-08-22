use crate::shader;
use std::mem;

// TODO support moar lights
//const MAX_LIGHTS: usize = 10;
const MAX_LIGHTS: usize = 1;

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 512,
    height: 512,
    depth: MAX_LIGHTS as u32,
};

#[repr(C)]
struct ShadowUniforms {
    proj: [[f32; 4]; 4],
}

pub struct Pass {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub uniform_buf: wgpu::Buffer,
    pub target_view: wgpu::TextureView,
}

pub fn create_pass(
    device: &wgpu::Device,
    shader_compiler: &mut shaderc::Compiler,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    globals_bind_group_layout: &wgpu::BindGroupLayout,
    instances_bind_group_layout: &wgpu::BindGroupLayout,
    light_bind_group_layout: &wgpu::BindGroupLayout,
    vertex_descs: &[wgpu::VertexBufferDescriptor],
) -> Pass {
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: wgpu::CompareFunction::LessEqual,
        lod_min_clamp: -100.0,
        lod_max_clamp: 100.0,
    });

    let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: SHADOW_SIZE,
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SHADOW_FORMAT,
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        label: None,
    });

    let shadow_view = shadow_texture.create_default_view();
    let shadow_target_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
        format: SHADOW_FORMAT, // TODO unsure of this
        dimension: wgpu::TextureViewDimension::D2,
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        level_count: 1,             // TODO unsure of this
        base_array_layer: 0 as u32, // TODO unsure of this
        array_layer_count: 1,       // TODO unsure of this
    });

    let uniform_size = mem::size_of::<ShadowUniforms>() as wgpu::BufferAddress;
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        bindings: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStage::VERTEX,
            ty: wgpu::BindingType::UniformBuffer { dynamic: false },
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        // TODO we don't need all these
        bind_group_layouts: &[
            &texture_bind_group_layout,
            &globals_bind_group_layout,
            &instances_bind_group_layout,


            // TODO these don't match - figure out how to pass light position in a consistent way
            //&light_bind_group_layout,
            &bind_group_layout
        ],
    });

    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: uniform_size,
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[wgpu::Binding {
            binding: 0,
            resource: wgpu::BindingResource::Buffer {
                buffer: &uniform_buf,
                range: 0..uniform_size,
            },
        }],
        label: None,
    });

    let vs_src = include_str!("shadow.vert");
    let fs_src = include_str!("shadow.frag");
    let vs_module =
        shader::create_vertex_module(device, shader_compiler, vs_src, "shadow.vert").unwrap();
    let fs_module =
        shader::create_fragment_module(device, shader_compiler, fs_src, "shadow.frag").unwrap();

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::Back,
            depth_bias: 2, // corresponds to bilinear filtering
            depth_bias_slope_scale: 2.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[],
        depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
            format: SHADOW_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            // TODO not sure about these four:
            stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: vertex_descs.clone(),
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    Pass {
        pipeline,
        bind_group,
        uniform_buf,
        // TODO We're going to need one for each light
        target_view: shadow_target_view,
    }
}
