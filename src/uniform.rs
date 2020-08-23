use crate::camera2;
use wgpu::util::DeviceExt;

// TODO rename to e.g. GlobalUniforms?
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

    pub fn update_view_proj(&mut self, camera: &camera2::Camera, projection: &camera2::Projection) {
        self.view_position = camera.position.to_homogeneous();
        self.view_proj = projection.calc_matrix() * camera.calc_matrix();
    }
}

pub fn create_buffer(device: &wgpu::Device, uniforms: &[Uniforms]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniforms"),
        contents: bytemuck::cast_slice(uniforms),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    })
}
