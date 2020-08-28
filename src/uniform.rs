use crate::camera;
use wgpu::util::DeviceExt;

// TODO rename to e.g. GlobalUniforms?
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Uniforms {
    pub view_position: cgmath::Vector4<f32>,
    pub view_proj: cgmath::Matrix4<f32>,
    pub light_proj: cgmath::Matrix4<f32>,
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

impl Uniforms {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;

        // TODO Hard coded light projection for now
        let light_proj = camera::OrthographicProjection::new().calc_matrix();
        let light_view = cgmath::Matrix4::look_at(
            cgmath::Point3::new(5.0, 10.0, 20.0),
            cgmath::Point3::new(0.0, 0.0, 0.0),
            cgmath::Vector3::unit_y(),
        );

        Self {
            view_position: cgmath::Zero::zero(),
            view_proj: cgmath::Matrix4::identity(),
            light_proj: light_proj * light_view,
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

pub fn create_buffer(device: &wgpu::Device, uniforms: &[Uniforms]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniforms"),
        contents: bytemuck::cast_slice(uniforms),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    })
}
