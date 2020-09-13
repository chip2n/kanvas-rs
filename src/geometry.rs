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

pub const PLANE_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

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
