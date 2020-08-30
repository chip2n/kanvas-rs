use crate::camera;
use crate::model;
use crate::pipeline;
use crate::{compile_frag, compile_vertex};
use std::mem;
use std::num::NonZeroU32;
use std::ops::Range;
use wgpu::util::DeviceExt;

// TODO support moar lights
//const MAX_LIGHTS: usize = 10;
const MAX_LIGHTS: usize = 1;

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 1024,
    height: 1024,
    depth: MAX_LIGHTS as u32,
};

pub struct ShadowPass {
    pub pipeline: wgpu::RenderPipeline,
    pub target_view: wgpu::TextureView,
    pub texture: wgpu::Texture,
    pub sampler: wgpu::Sampler,
    pub uniforms_buffer: wgpu::Buffer,
    pub uniforms_bind_group: wgpu::BindGroup,
}

impl ShadowPass {
    pub fn new(
        device: &wgpu::Device,
        shader_compiler: &mut shaderc::Compiler,
        instances_bind_group_layout: &wgpu::BindGroupLayout,
        vertex_descs: &[wgpu::VertexBufferDescriptor],
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: SHADOW_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::COPY_SRC,
            label: None,
        });

        let target_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow"),
            format: None,
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(1),
        });

        let uniforms = ShadowUniforms::new();
        let uniforms_buffer = create_buffer(device, &[uniforms]);
        let uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mem::size_of::<ShadowUniforms>() as _
                        ),
                    },
                    count: None,
                }],
                label: Some("Shadow uniforms bind group layout"),
            });

        let uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniforms_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(uniforms_buffer.slice(..)),
            }],
            label: Some("Shadow uniforms bind group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            // TODO we don't need all these
            label: Some("Shadow pipeline"),
            push_constant_ranges: &[],
            bind_group_layouts: &[&uniforms_bind_group_layout, &instances_bind_group_layout],
        });

        let vs_module = compile_vertex!(device, shader_compiler, "shadow.vert").unwrap();
        let fs_module = compile_frag!(device, shader_compiler, "shadow.frag").unwrap();

        let pipeline = pipeline::create(
            &"shadow pass",
            device,
            &pipeline_layout,
            &vs_module,
            &fs_module,
            None,
            Some(pipeline::DepthConfig::default()),
            vertex_descs.clone(),
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: None,
            ..Default::default()
        });

        Self {
            pipeline,
            target_view,
            texture,
            sampler,
            uniforms_buffer,
            uniforms_bind_group,
        }
    }

    pub fn begin<'a>(&'a self, encoder: &'a mut wgpu::CommandEncoder) -> ShadowPassRunner<'a> {
        // Clear depth buffer
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                attachment: &self.target_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.pipeline);

        ShadowPassRunner {
            render_pass,
            uniforms_bind_group: &self.uniforms_bind_group,
        }
    }
}

pub struct ShadowPassRunner<'a> {
    render_pass: wgpu::RenderPass<'a>,
    uniforms_bind_group: &'a wgpu::BindGroup,
}

impl<'a> ShadowPassRunner<'a> {
    pub fn render<'b>(&mut self, data: ShadowPassRenderData<'b>)
    where
        'b: 'a,
    {
        self.render_pass
            .set_vertex_buffer(0, data.vertex_buffer.slice(..));
        self.render_pass
            .set_index_buffer(data.index_buffer.slice(..));
        self.render_pass
            .set_bind_group(0, &self.uniforms_bind_group, &[]);
        self.render_pass
            .set_bind_group(1, &data.instances_bind_group, &[]);
        self.render_pass
            .draw_indexed(data.indices, 0, data.instances);
    }
}

pub struct ShadowPassRenderData<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub indices: Range<u32>,
    pub instances_bind_group: &'a wgpu::BindGroup,
    pub instances: Range<u32>,
}

impl<'a> ShadowPassRenderData<'a> {
    pub fn from_mesh(mesh: &'a model::Mesh, instances_bind_group: &'a wgpu::BindGroup) -> Self {
        Self {
            vertex_buffer: &mesh.vertex_buffer,
            index_buffer: &mesh.index_buffer,
            indices: 0..mesh.num_elements,
            instances_bind_group,
            instances: 0..1,
        }
    }
}

pub enum ShadowMapLightType {
    Directional,
    Point,
}

pub fn create_light_proj(light_type: ShadowMapLightType) -> cgmath::Matrix4<f32> {
    let light_proj = create_proj_mat(light_type);
    let light_view = cgmath::Matrix4::look_at(
        cgmath::Point3::new(5.0, 10.0, 20.0),
        cgmath::Point3::new(0.0, 0.0, 0.0),
        cgmath::Vector3::unit_y(),
    );

    light_proj * light_view
}

pub fn create_light_proj_cube(light_pos: cgmath::Point3<f32>) -> Vec<cgmath::Matrix4<f32>> {
    let light_proj = create_proj_mat(ShadowMapLightType::Point);
    let transforms = vec![
        light_proj
            * cgmath::Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(1.0, 0.0, 0.0),
                -cgmath::Vector3::unit_y(),
            ),
        light_proj
            * cgmath::Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(-1.0, 0.0, 0.0),
                -cgmath::Vector3::unit_y(),
            ),
        light_proj
            * cgmath::Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                cgmath::Vector3::unit_z(),
            ),
        light_proj
            * cgmath::Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, -1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * cgmath::Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 0.0, 1.0),
                -cgmath::Vector3::unit_y(),
            ),
        light_proj
            * cgmath::Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 0.0, -1.0),
                -cgmath::Vector3::unit_y(),
            ),
    ];
    transforms
}

fn create_proj_mat(light_type: ShadowMapLightType) -> cgmath::Matrix4<f32> {
    match light_type {
        ShadowMapLightType::Directional => {
            camera::OrthographicProjection::new(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0).calc_matrix()
        }
        ShadowMapLightType::Point => {
            camera::PerspectiveProjection::new(1024, 1024, cgmath::Deg(45.0), 0.1, 100.0)
                .calc_matrix()
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ShadowUniforms {
    pub light_proj: cgmath::Matrix4<f32>,
}

unsafe impl bytemuck::Pod for ShadowUniforms {}
unsafe impl bytemuck::Zeroable for ShadowUniforms {}

impl ShadowUniforms {
    pub fn new() -> Self {
        Self {
            light_proj: create_light_proj(ShadowMapLightType::Point),
        }
    }
}

pub fn create_buffer(device: &wgpu::Device, uniforms: &[ShadowUniforms]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Shadow uniforms"),
        contents: bytemuck::cast_slice(uniforms),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    })
}
