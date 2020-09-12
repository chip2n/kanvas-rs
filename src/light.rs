use crate::model;
use std::ops::Range;
use wgpu::util::DeviceExt;

pub struct Light {
    pub position: cgmath::Vector3<f32>,
    pub color: cgmath::Vector3<f32>,
    pub light_type: LightType,
}

impl Light {
    pub fn new<P: Into<cgmath::Vector3<f32>>, C: Into<cgmath::Vector3<f32>>>(
        position: P,
        color: C,
    ) -> Self {
        Light {
            position: position.into(),
            color: color.into(),
            light_type: LightType::Point,
        }
    }

    pub fn to_raw(&self) -> LightRaw {
        LightRaw {
            position: self.position,
            _padding: 0,
            color: self.color,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct LightRaw {
    pub position: cgmath::Vector3<f32>,
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: cgmath::Vector3<f32>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct LightConfigRaw {
    pub shadows_enabled: bool,
    _padding: [u8; 3],
}

pub struct LightConfig {
    pub shadows_enabled: bool,
    buffer: wgpu::Buffer,
}

impl LightConfig {
    pub fn new(device: &wgpu::Device) -> Self {
        let shadows_enabled = true;
        let contents = LightConfigRaw {
            shadows_enabled,
            _padding: [0; 3],
        };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[contents]),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });
        LightConfig {
            shadows_enabled,
            buffer,
        }
    }

    pub fn upload(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.to_raw()));
    }

    pub fn binding_resource(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer {
            buffer: &self.buffer,
            offset: 0,
            size: None,
        }
    }

    pub fn binding_size() -> Option<wgpu::BufferSize> {
        wgpu::BufferSize::new(std::mem::size_of::<LightConfigRaw>() as _)
    }

    fn to_raw(&self) -> LightConfigRaw {
        LightConfigRaw {
            shadows_enabled: self.shadows_enabled,
            _padding: [0; 3],
        }
    }
}

unsafe impl bytemuck::Zeroable for LightConfigRaw {}
unsafe impl bytemuck::Pod for LightConfigRaw {}

pub enum LightType {
    Directional,
    Point,
}

unsafe impl bytemuck::Zeroable for LightRaw {}
unsafe impl bytemuck::Pod for LightRaw {}

pub trait DrawLight<'a, 'b>
where
    'b: 'a,
{
    fn draw_light_mesh(
        &mut self,
        mesh: &'b model::Mesh,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    );
    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'b model::Mesh,
        instances: Range<u32>,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    );
    fn draw_light_model(
        &mut self,
        model: &'b model::Model,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    );
    fn draw_light_model_instanced(
        &mut self,
        model: &'b model::Model,
        instances: Range<u32>,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawLight<'a, 'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_light_mesh(
        &mut self,
        mesh: &'b model::Mesh,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    ) {
        self.draw_light_mesh_instanced(mesh, 0..1, uniforms, light);
    }

    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'b model::Mesh,
        instances: Range<u32>,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..));
        self.set_bind_group(0, uniforms, &[]);
        self.set_bind_group(1, light, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_light_model(
        &mut self,
        model: &'b model::Model,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    ) {
        self.draw_light_model_instanced(model, 0..1, uniforms, light);
    }

    fn draw_light_model_instanced(
        &mut self,
        model: &'b model::Model,
        instances: Range<u32>,
        uniforms: &'b wgpu::BindGroup,
        light: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_light_mesh_instanced(mesh, instances.clone(), uniforms, light);
        }
    }
}
