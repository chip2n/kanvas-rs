use crate::model;
use crate::shader;
use std::num::NonZeroU32;
use std::ops::Range;

// TODO support moar lights
//const MAX_LIGHTS: usize = 10;
const MAX_LIGHTS: usize = 1;

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 512,
    height: 512,
    depth: MAX_LIGHTS as u32,
};

pub struct Pass {
    pub pipeline: wgpu::RenderPipeline,
    pub target_view: wgpu::TextureView,
}

impl Pass {
    pub fn new(
        device: &wgpu::Device,
        shader_compiler: &mut shaderc::Compiler,
        globals_bind_group_layout: &wgpu::BindGroupLayout,
        instances_bind_group_layout: &wgpu::BindGroupLayout,
        vertex_descs: &[wgpu::VertexBufferDescriptor],
    ) -> Self {
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: SHADOW_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
            label: None,
        });

        let target_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow"),
            format: None,
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(1),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            // TODO we don't need all these
            label: Some("Shadow pipeline"),
            push_constant_ranges: &[],
            bind_group_layouts: &[&globals_bind_group_layout, &instances_bind_group_layout],
        });

        let vs_src = include_str!("shadow.vert");
        let fs_src = include_str!("shadow.frag");
        let vs_module =
            shader::create_vertex_module(device, shader_compiler, vs_src, "shadow.vert").unwrap();
        let fs_module =
            shader::create_fragment_module(device, shader_compiler, fs_src, "shadow.frag").unwrap();

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow pipeline"),
            layout: Some(&pipeline_layout),
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
                clamp_depth: device.features().contains(wgpu::Features::DEPTH_CLAMPING),
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: SHADOW_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilStateDescriptor::default(),
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: vertex_descs.clone(),
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        Self {
            pipeline,
            target_view,
        }
    }

    pub fn begin<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                attachment: &self.target_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass
    }
}

pub struct ShadowPassRenderData<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub indices: Range<u32>,
    pub uniforms_bind_group: &'a wgpu::BindGroup,
    pub instances_bind_group: &'a wgpu::BindGroup,
    pub instances: Range<u32>,
}

impl<'a> ShadowPassRenderData<'a> {
    pub fn from_mesh(
        mesh: &'a model::Mesh,
        uniforms_bind_group: &'a wgpu::BindGroup,
        instances_bind_group: &'a wgpu::BindGroup,
    ) -> Self {
        Self {
            vertex_buffer: &mesh.vertex_buffer,
            index_buffer: &mesh.index_buffer,
            indices: 0..mesh.num_elements,
            uniforms_bind_group,
            instances_bind_group,
            instances: 0..1,
        }
    }
}

pub fn render<'a, 'b>(render_pass: &mut wgpu::RenderPass<'a>, data: ShadowPassRenderData<'b>)
where
    'b: 'a,
{
    render_pass.set_vertex_buffer(0, data.vertex_buffer.slice(..));
    render_pass.set_index_buffer(data.index_buffer.slice(..));
    render_pass.set_bind_group(0, &data.uniforms_bind_group, &[]);
    render_pass.set_bind_group(1, &data.instances_bind_group, &[]);
    render_pass.draw_indexed(data.indices, 0, data.instances);
}
