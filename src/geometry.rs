use crate::model;
use wgpu::util::DeviceExt;

pub const PLANE_VERTICES: [SimpleVertex; 4] = [
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

pub const PLANE_INDICES: &[u32] = &[0, 1, 2, 0, 2, 3];

pub struct Plane {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl Plane {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&PLANE_VERTICES),
            usage: wgpu::BufferUsage::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&PLANE_INDICES),
            usage: wgpu::BufferUsage::INDEX,
        });
        Plane {
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        material: &'a model::Material,
        uniforms_bind_group: &'a wgpu::BindGroup,
        instances_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..));
        render_pass.set_bind_group(0, &material.bind_group, &[]);
        render_pass.set_bind_group(1, &uniforms_bind_group, &[]);
        render_pass.set_bind_group(2, &instances_bind_group, &[]);
        render_pass.draw_indexed(0..PLANE_INDICES.len() as u32, 0, 0..1);
    }
}

pub trait Vertex {
    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a>;
}

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
