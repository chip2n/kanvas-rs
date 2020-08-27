use crate::model::Vertex;
use crate::shader;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SimpleVertex {
    position: cgmath::Vector3<f32>,
    tex_coords: cgmath::Vector2<f32>,
}

unsafe impl bytemuck::Pod for SimpleVertex {}
unsafe impl bytemuck::Zeroable for SimpleVertex {}

impl Vertex for SimpleVertex {
    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        use std::mem;
        wgpu::VertexBufferDescriptor {
            // how wide a vertex is (shader skips this number of bytes to get to the next one)
            stride: mem::size_of::<SimpleVertex>() as wgpu::BufferAddress,

            // how often shader should move to the next vertex (e.g. for instancing)
            step_mode: wgpu::InputStepMode::Vertex,

            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3,
                },
                wgpu::VertexAttributeDescriptor {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float2,
                },
            ],
        }
    }
}

const PLANE_VERTICES: [SimpleVertex; 4] = [
    SimpleVertex {
        position: cgmath::Vector3::new(-1.0, -1.0, 0.0),
        tex_coords: cgmath::Vector2::new(0.0, 1.0),
    },
    SimpleVertex {
        position: cgmath::Vector3::new(1.0, -1.0, 0.0),
        tex_coords: cgmath::Vector2::new(1.0, 1.0),
    },
    SimpleVertex {
        position: cgmath::Vector3::new(1.0, 1.0, 0.0),
        tex_coords: cgmath::Vector2::new(1.0, 0.0),
    },
    SimpleVertex {
        position: cgmath::Vector3::new(-1.0, 1.0, 0.0),
        tex_coords: cgmath::Vector2::new(0.0, 0.0),
    },
];

const PLANE_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

pub struct DebugPass {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl DebugPass {
    pub fn new(
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        globals_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let mut verts = PLANE_VERTICES.clone();
        for vert in &mut verts {
            let new_pos = cgmath::Matrix4::from_translation(cgmath::Vector3::new(0.75, 0.75, 0.0))
                * cgmath::Matrix4::from_scale(0.25)
                * vert.position.extend(1.0);
            vert.position = new_pos.truncate();
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsage::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&PLANE_INDICES),
            usage: wgpu::BufferUsage::INDEX,
        });

        // TODO pass this in
        let mut shader_compiler = shaderc::Compiler::new().unwrap();

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                ],
                label: Some("texture_bind_group_layout"),
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
            label: None,
        });

        let pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render pipeline"),
                push_constant_ranges: &[],
                bind_group_layouts: &[&texture_bind_group_layout, globals_bind_group_layout],
            });

            let vs_src = include_str!("debug.vert");
            let fs_src = include_str!("debug.frag");

            let vs_module =
                shader::create_vertex_module(device, &mut shader_compiler, vs_src, "debug.vert")
                    .unwrap();
            let fs_module =
                shader::create_fragment_module(device, &mut shader_compiler, fs_src, "debug.frag")
                    .unwrap();

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("debug"),
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
                rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: wgpu::CullMode::Back,
                    ..Default::default()
                }),
                // description on how color are stored and processed throughout the pipeline
                color_states: &[wgpu::ColorStateDescriptor {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb, // TODO same as swap chain - refactor
                    color_blend: wgpu::BlendDescriptor::REPLACE,
                    alpha_blend: wgpu::BlendDescriptor::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[SimpleVertex::desc()],
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false,
            })
        };

        DebugPass {
            vertex_buffer,
            index_buffer,
            bind_group,
            pipeline,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::SwapChainTexture,
        globals_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            // where we're going to draw our color to
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &frame.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.pipeline);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..));
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_bind_group(1, &globals_bind_group, &[]);
        render_pass.draw_indexed(0..PLANE_INDICES.len() as u32, 0, 0..1);
    }
}

pub fn save_texture(device: &wgpu::Device, width: u32, height: u32, texture: &wgpu::Texture) {
    const U32_SIZE: u32 = std::mem::size_of::<u32>() as u32;
    let buffer_size = (U32_SIZE * width * height) as wgpu::BufferAddress;
    let buffer_desc = wgpu::BufferDescriptor {
        size: buffer_size,
        usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::MAP_READ,
        label: Some("screenshot_buffer"),
        mapped_at_creation: false,
    };

    let buffer = device.create_buffer(&buffer_desc);

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    encoder.copy_texture_to_buffer(
        wgpu::TextureCopyView {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::BufferCopyView {
            buffer: &buffer,
            layout: wgpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: U32_SIZE * width,
                rows_per_image: height,
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth: 1,
        },
    );

    let near = 0.1;
    let far = 100.0;
    std::thread::spawn(move || {
        use futures::executor::block_on;

        let slice = buffer.slice(..);
        block_on(slice.map_async(wgpu::MapMode::Read)).unwrap();
        let data: &[u8] = &slice.get_mapped_range();
        let pixels: &[f32] = bytemuck::try_cast_slice(data).unwrap();

        use image::{ImageBuffer, Pixel, Rgba};
        let mut buffer = ImageBuffer::<Rgba<u8>, _>::new(width, height);

        let mut x = 0;
        let mut y = 0;
        for pixel in pixels {
            let z = pixel * 2.0 - 1.0;
            let r = (2.0 * near * far) / (far + near - z * (far - near));
            let p = (r.floor() * 255.0 / far) as u8;

            buffer.put_pixel(x, y, Pixel::from_channels(p, p, p, 255));

            x += 1;
            if x >= width {
                x = 0;
                y += 1;
            }
        }

        buffer.save("image.png").unwrap();
    });
}
