use crate::prelude::*;
use crate::shadow;
use wgpu::util::DeviceExt;

pub struct Lights {
    // TODO bundle together to support multiple lights
    pub light: Light,
    pub shadow_cubemap: shadow::ShadowCubemap,

    pub config: LightConfig,
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl Lights {
    pub fn new(
        context: &Context,
        light_bind_group_layout: &wgpu::BindGroupLayout, // TODO store in context?
    ) -> Self {
        let config = LightConfig::new(&context.device);

        let light = Light::new((20.0, 20.0, 0.0), (1.0, 1.0, 1.0));

        // We'll want to update our lights position, so we use COPY_DST
        let buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lights"),
                contents: bytemuck::cast_slice(&[light.to_raw()]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            });

        let shadow_cubemap = shadow::ShadowCubemap::new(context);

        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &light_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &buffer,
                            offset: 0,
                            size: None,
                        },
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&shadow_cubemap.texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&shadow_cubemap.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: config.binding_resource(),
                    },
                ],
                label: None,
            });

        Self {
            light,
            shadow_cubemap,
            config,
            buffer,
            bind_group,
        }
    }
}

pub struct Light {
    pub position: Vector3,
    pub color: Vector3,
    pub light_type: LightType,
}

impl Light {
    pub fn new<P: Into<Vector3>, C: Into<Vector3>>(position: P, color: C) -> Self {
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
    pub position: Vector3,
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: Vector3,
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
