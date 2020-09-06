use crate::camera;
use crate::light;
use crate::model;
use crate::pipeline;
use crate::Kanvas;
use crate::{compile_frag, compile_vertex};
use model::Vertex;
use wgpu::util::DeviceExt;

pub struct ForwardPass {
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub instances_bind_group_layout: wgpu::BindGroupLayout,
    pub light_bind_group_layout: wgpu::BindGroupLayout,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,

    pub uniforms: Uniforms,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,

    pub pipeline: wgpu::RenderPipeline,
}

impl ForwardPass {
    pub fn new(kanvas: &mut Kanvas) -> Self {
        let texture_bind_group_layout =
            kanvas
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::SampledTexture {
                                multisampled: false,
                                dimension: wgpu::TextureViewDimension::D2,
                                component_type: wgpu::TextureComponentType::Uint,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::Sampler { comparison: false },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::SampledTexture {
                                multisampled: false,
                                component_type: wgpu::TextureComponentType::Float,
                                dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::Sampler { comparison: false },
                            count: None,
                        },
                    ],
                    label: Some("texture_bind_group_layout"),
                });

        let light_bind_group_layout =
            kanvas
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::UniformBuffer {
                                dynamic: false,
                                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                                    light::LightRaw,
                                >(
                                )
                                    as _),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::SampledTexture {
                                multisampled: false,
                                component_type: wgpu::TextureComponentType::Float,
                                dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::Sampler { comparison: false },
                            count: None,
                        },
                    ],
                    label: None,
                });

        let instances_bind_group_layout =
            kanvas
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    }],
                    label: Some("instances_bind_group_layout"),
                });

        let uniforms = Uniforms::new();
        let uniform_buffer = kanvas
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniforms"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            });

        let uniform_bind_group_layout =
            kanvas
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::UniformBuffer {
                            dynamic: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<Uniforms>() as _,
                            ),
                        },
                        count: None,
                    }],
                    label: Some("uniform_bind_group_layout"),
                });

        let uniform_bind_group = kanvas.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                },
            }],
            label: Some("uniform_bind_group"),
        });

        let pipeline = {
            let layout = kanvas
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render pipeline"),
                    push_constant_ranges: &[],
                    bind_group_layouts: &[
                        &texture_bind_group_layout,
                        &uniform_bind_group_layout,
                        &instances_bind_group_layout,
                        &light_bind_group_layout,
                    ],
                });

            let vs_module =
                compile_vertex!(&kanvas.device, &mut kanvas.shader_compiler, "shader.vert")
                    .unwrap();
            let fs_module =
                compile_frag!(&kanvas.device, &mut kanvas.shader_compiler, "shader.frag").unwrap();

            pipeline::create(
                &"forward",
                &kanvas.device,
                &layout,
                &vs_module,
                &fs_module,
                Some(kanvas.sc_desc.format),
                Some(pipeline::DepthConfig::no_bias()),
                &[model::ModelVertex::desc()],
            )
        };

        ForwardPass {
            texture_bind_group_layout,
            instances_bind_group_layout,
            light_bind_group_layout,
            uniform_bind_group_layout,

            uniforms,
            uniform_buffer,
            uniform_bind_group,

            pipeline,
        }
    }

    pub fn upload_uniforms(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Staging"),
            contents: bytemuck::cast_slice(&[self.uniforms]),
            usage: wgpu::BufferUsage::COPY_SRC,
        });

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.uniform_buffer,
            0,
            std::mem::size_of::<Uniforms>() as wgpu::BufferAddress,
        );
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Uniforms {
    pub view_position: cgmath::Vector4<f32>,
    pub view_proj: cgmath::Matrix4<f32>,
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

impl Uniforms {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;

        Self {
            view_position: cgmath::Zero::zero(),
            view_proj: cgmath::Matrix4::identity(),
        }
    }

    // TODO projection: Into<Matrix4>?
    pub fn update_view_proj(
        &mut self,
        camera: &camera::Camera,
        projection: &camera::PerspectiveProjection,
    ) {
        self.view_position = camera.position.to_homogeneous();
        self.view_proj = projection.calc_matrix() * camera.calc_matrix();
    }
}
