use crate::camera;

#[rustfmt::skip]
const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Uniforms {
    pub view_proj: cgmath::Matrix4<f32>,
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

impl Uniforms {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &camera::Camera) {
        self.view_proj = OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix();
    }
}
