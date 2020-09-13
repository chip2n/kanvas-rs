use crate::model::Vertex;
use crate::{compile_frag, compile_vertex};
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
    pipeline: wgpu::RenderPipeline,
}

impl DebugPass {
    pub fn new(
        device: &wgpu::Device,
        shader_compiler: &mut shaderc::Compiler,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let verts = PLANE_VERTICES.clone();

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

        let pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render pipeline"),
                push_constant_ranges: &[],
                bind_group_layouts: &[&texture_bind_group_layout],
            });

            let vs_module = compile_vertex!(&device, shader_compiler, "debug.vert").unwrap();
            let fs_module = compile_frag!(&device, shader_compiler, "debug.frag").unwrap();

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
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    //format: wgpu::TextureFormat::Bgra8UnormSrgb,
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
            pipeline,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        texture_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &output,
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
        render_pass.set_bind_group(0, &texture_bind_group, &[]);
        render_pass.draw_indexed(0..PLANE_INDICES.len() as u32, 0, 0..1);
    }
}
