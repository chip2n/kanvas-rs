use crate::camera;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Uniforms {
    pub view_proj: cgmath::Matrix4<f32>,
}

impl Uniforms {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &camera::Camera) {
        self.view_proj = camera.build_view_projection_matrix();
    }
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

pub fn make_uniform_buffer(
    camera: &camera::Camera,
    device: &wgpu::Device,
) -> (
    Uniforms,
    wgpu::Buffer,
    wgpu::BindGroupLayout,
    wgpu::BindGroup,
) {
    let mut uniforms = Uniforms::new();
    uniforms.update_view_proj(&camera);
    let buffer = device.create_buffer_with_data(
        bytemuck::cast_slice(&[uniforms]),
        wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    );

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStage::VERTEX,
            ty: wgpu::BindingType::UniformBuffer { dynamic: false },
        }],
        label: Some("uniform_bind_group_layout"),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[wgpu::Binding {
            binding: 0,
            resource: wgpu::BindingResource::Buffer {
                buffer: &buffer,
                range: 0..std::mem::size_of_val(&uniforms) as wgpu::BufferAddress,
            },
        }],
        label: Some("uniform_bind_group"),
    });

    (uniforms, buffer, bind_group_layout, bind_group)
}
