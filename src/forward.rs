use crate::camera;
use crate::geometry::Vertex;
use crate::model;
use crate::pipeline;
use crate::prelude::*;
use crate::texture;
use crate::Context;
use crate::{compile_frag, compile_vertex};
use wgpu::util::DeviceExt;

pub struct ForwardPass {
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,

    pub uniforms: Uniforms,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,

    pub depth_texture: texture::Texture,
    pub pipeline: wgpu::RenderPipeline,
    pub billboard_pipeline: wgpu::RenderPipeline,
}

impl ForwardPass {
    pub fn new(context: &mut Context) -> Self {
        let texture_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::SampledTexture {
                                multisampled: false,
                                dimension: wgpu::TextureViewDimension::D2,
                                component_type: wgpu::TextureComponentType::Float,
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

        let uniforms = Uniforms::new();
        let uniform_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniforms"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            });

        let uniform_bind_group_layout =
            context
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

        let uniform_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
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

        let depth_texture = texture::Texture::create_depth_texture(
            &context.device,
            &context.sc_desc,
            "depth_texture",
        );

        let pipeline = {
            let layout = context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render pipeline"),
                    push_constant_ranges: &[],
                    bind_group_layouts: &[
                        &texture_bind_group_layout,
                        &uniform_bind_group_layout,
                        &context.instances_bind_group_layout,
                        &context.light_bind_group_layout,
                    ],
                });

            let vs_module =
                compile_vertex!(&context.device, &mut context.shader_compiler, "shader.vert")
                    .unwrap();
            let fs_module =
                compile_frag!(&context.device, &mut context.shader_compiler, "shader.frag").unwrap();

            pipeline::create(
                &"forward",
                &context.device,
                &layout,
                &vs_module,
                &fs_module,
                Some(context.sc_desc.format),
                Some(pipeline::DepthConfig::no_bias()),
                &[model::ModelVertex::desc()],
            )
        };

        let billboard_pipeline = crate::billboard::create_pipeline(
            context,
            &texture_bind_group_layout,
            &uniform_bind_group_layout,
        );

        ForwardPass {
            texture_bind_group_layout,
            uniform_bind_group_layout,

            uniforms,
            uniform_buffer,
            uniform_bind_group,

            depth_texture,
            pipeline,
            billboard_pipeline,
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

    pub fn resize(&mut self, device: &wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) {
        self.depth_texture =
            texture::Texture::create_depth_texture(device, sc_desc, "depth_texture");
    }

    pub fn begin<'a>(
        &'a self,
        output: &'a wgpu::TextureView,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass {
        let back_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            // where we're going to draw our color to
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(back_color),
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                attachment: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Uniforms {
    pub view_position: cgmath::Vector4<f32>,
    pub view_proj: Matrix4,
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

impl Uniforms {
    pub fn new() -> Self {
        Self {
            view_position: cgmath::Zero::zero(),
            view_proj: Matrix4::identity(),
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
