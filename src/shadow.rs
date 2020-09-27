use crate::camera;
use crate::light;
use crate::model;
use crate::pipeline;
use crate::prelude::*;
use crate::{compile_frag, compile_vertex};
use std::mem;
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 1024,
    height: 1024,
    depth: 1,
};

pub struct ShadowMapTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

pub struct ShadowCubemap {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl ShadowCubemap {
    pub fn new(context: &Context) -> Self {
        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
            label: None,
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow"),
            format: None,
            dimension: Some(wgpu::TextureViewDimension::Cube),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(1),
        });

        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
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
            texture,
            texture_view,
            sampler,
        }
    }
}

pub struct ShadowPass {
    pub pipeline: wgpu::RenderPipeline,
    pub uniforms_buffer: wgpu::Buffer,
    pub uniforms_bind_group: wgpu::BindGroup,
    pub targets: [ShadowMapTarget; 6],
    pub target_bind_group_layout: wgpu::BindGroupLayout,
}

impl ShadowPass {
    pub fn new(
        device: &wgpu::Device,
        shader_compiler: &mut shaderc::Compiler,
        instances_bind_group_layout: &wgpu::BindGroupLayout,
        vertex_descs: &[wgpu::VertexBufferDescriptor],
    ) -> Self {
        // Make room for all 6 sides of cubemap for each light
        let uniforms_size =
            (light::MAX_LIGHTS as u64 * 6 * wgpu::BIND_BUFFER_ALIGNMENT) as wgpu::BufferAddress;
        let uniforms_binding_size = NonZeroU64::new(mem::size_of::<ShadowUniforms>() as u64);
        let uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow uniforms"),
            size: uniforms_size,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: true,
                        min_binding_size: uniforms_binding_size,
                    },
                    count: None,
                }],
                label: Some("Shadow uniforms bind group layout"),
            });

        let uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniforms_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniforms_buffer,
                    offset: 0,
                    size: uniforms_binding_size,
                },
            }],
            label: Some("Shadow uniforms bind group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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

        let target_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Float,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let create_target = || {
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

            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Shadow"),
                format: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                level_count: None,
                base_array_layer: 0,
                array_layer_count: NonZeroU32::new(1),
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &target_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: None,
            });

            ShadowMapTarget {
                texture,
                view,
                bind_group,
            }
        };
        let targets = [
            create_target(),
            create_target(),
            create_target(),
            create_target(),
            create_target(),
            create_target(),
        ];

        Self {
            pipeline,
            uniforms_buffer,
            uniforms_bind_group,
            targets,
            target_bind_group_layout,
        }
    }

    pub fn copy_to_cubemap(&self, encoder: &mut wgpu::CommandEncoder, cubemap: &wgpu::Texture) {
        for (i, target) in self.targets.iter().enumerate() {
            encoder.copy_texture_to_texture(
                wgpu::TextureCopyView {
                    texture: &target.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                },
                wgpu::TextureCopyView {
                    texture: &cubemap,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                },
                wgpu::Extent3d {
                    width: 1024,
                    height: 1024,
                    depth: 1,
                },
            );
        }
    }

    pub fn begin<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        face_index: usize,
    ) -> ShadowPassRunner<'a> {
        // Clear depth buffer
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                attachment: &self.targets[face_index].view,
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
                attachment: &self.targets[face_index].view,
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

    pub fn update_light(&self, queue: &wgpu::Queue, light_index: usize, light: &light::Light) {
        let projections = create_light_proj_cube(cgmath::EuclideanSpace::from_vec(light.position));
        for (i, proj) in projections.iter().enumerate() {
            let uniforms = ShadowUniforms {
                light_proj: *proj,
                light_position: light.position,
            };
            let buffer_offset = light_buffer_offset(light_index, i) as wgpu::BufferAddress;
            queue.write_buffer(
                &self.uniforms_buffer,
                buffer_offset,
                bytemuck::bytes_of(&uniforms),
            );
        }
    }
}

pub struct ShadowPassRunner<'a> {
    render_pass: wgpu::RenderPass<'a>,
    uniforms_bind_group: &'a wgpu::BindGroup,
}

impl<'a> ShadowPassRunner<'a> {
    pub fn render<'b>(
        &mut self,
        data: ShadowPassRenderData<'b>,
        face_index: usize,
        light_index: usize,
    ) where
        'b: 'a,
    {
        let buffer_offset = light_buffer_offset(light_index, face_index) as wgpu::DynamicOffset;

        self.render_pass
            .set_vertex_buffer(0, data.vertex_buffer.slice(..));
        self.render_pass
            .set_index_buffer(data.index_buffer.slice(..));
        self.render_pass
            .set_bind_group(0, &self.uniforms_bind_group, &[buffer_offset]);
        self.render_pass
            .set_bind_group(1, &data.instances_bind_group, &[]);
        self.render_pass
            .draw_indexed(data.indices, 0, data.instances);
    }
}

fn light_buffer_offset(light_index: usize, face_index: usize) -> usize {
    let light_offset = light_index * 6 * wgpu::BIND_BUFFER_ALIGNMENT as usize;
    let face_offset = face_index * wgpu::BIND_BUFFER_ALIGNMENT as usize;
    light_offset + face_offset
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

pub fn create_light_proj_cube(light_pos: cgmath::Point3<f32>) -> Vec<Matrix4> {
    let light_proj = create_proj_mat(&light::LightType::Point);
    let transforms = vec![
        /*
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, -1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        */
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(1.0, 0.0, 0.0),
                cgmath::Vector3::unit_y(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(-1.0, 0.0, 0.0),
                cgmath::Vector3::unit_y(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 1.0, 0.0),
                cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, -1.0, 0.0),
                -cgmath::Vector3::unit_z(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 0.0, -1.0),
                cgmath::Vector3::unit_y(),
            ),
        light_proj
            * Matrix4::look_at(
                light_pos,
                light_pos + cgmath::Vector3::new(0.0, 0.0, 1.0),
                cgmath::Vector3::unit_y(),
            ),
    ];
    transforms
}

fn create_proj_mat(light_type: &light::LightType) -> Matrix4 {
    match light_type {
        light::LightType::Directional => {
            camera::OrthographicProjection::new(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0).calc_matrix()
        }
        light::LightType::Point => {
            camera::PerspectiveProjection::new(1024, 1024, cgmath::Deg(90.0), 0.1, 100.0)
                .calc_matrix()
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ShadowUniforms {
    pub light_proj: Matrix4,
    pub light_position: Vector3,
}

unsafe impl bytemuck::Pod for ShadowUniforms {}
unsafe impl bytemuck::Zeroable for ShadowUniforms {}
