use crate::model;
use crate::prelude::*;
use crate::shadow;
use wgpu::util::DeviceExt;

pub type LightId = usize;

/// The maximum number of lights supported at once
pub const MAX_LIGHTS: usize = 2;

pub struct Lights {
    pub lights: [Option<Light>; MAX_LIGHTS],

    /// Cubemap textures for each light in the world
    pub shadow_textures: [wgpu::Texture; MAX_LIGHTS],

    /// Material used to render light billboards
    pub material: model::MaterialId,

    pub config: LightConfig,
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl Lights {
    pub fn new(
        device: &wgpu::Device,
        light_bind_group_layout: &wgpu::BindGroupLayout,
        material: model::MaterialId,
    ) -> Self {
        let config = LightConfig::new(device);
        let lights = [None; MAX_LIGHTS];

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: None,
            ..Default::default()
        });

        let (shadow_textures, shadow_texture_views) = {
            let shadow_cubemap1 = shadow::ShadowCubemap::new(device);
            let shadow_cubemap2 = shadow::ShadowCubemap::new(device);

            (
                [shadow_cubemap1.texture, shadow_cubemap2.texture],
                [shadow_cubemap1.texture_view, shadow_cubemap2.texture_view],
            )
        };

        let buffer_data = Self::map_raw(&lights);

        // We'll want to update our lights position, so we use COPY_DST
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights"),
            contents: bytemuck::cast_slice(&[buffer_data]),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: light_bind_group_layout,
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
                    resource: wgpu::BindingResource::TextureViewArray(&shadow_texture_views),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: config.binding_resource(),
                },
            ],
            label: None,
        });

        Self {
            lights,
            shadow_textures,
            material,
            config,
            buffer,
            bind_group,
        }
    }

    pub fn add_light(&mut self, position: Vector3) -> Option<LightId> {
        let index = self.lights.iter().position(|l| l.is_none());
        match index {
            Some(i) => self.lights[i] = Some(Light::new(position, (1.0, 1.0, 1.0))),
            None => eprintln!("Unable to add light - max count reached"),
        }
        index
    }

    pub fn to_raw(&self) -> LightsRaw {
        Self::map_raw(&self.lights)
    }

    fn map_raw(lights: &[Option<Light>]) -> LightsRaw {
        let mut positions = [Vector4::zero(); MAX_LIGHTS];
        let mut colors = [Vector4::zero(); MAX_LIGHTS];

        for i in 0..MAX_LIGHTS {
            let light = &lights[i];
            if let Some(light) = light {
                positions[i] = light.position.extend(0.0);
                colors[i] = light.color.extend(0.0);
            }
        }

        LightsRaw { positions, colors }
    }
}

#[derive(Copy, Clone)]
pub struct Light {
    pub position: Vector3,
    pub color: Vector3,
    pub light_type: LightType,
}

impl Light {
    pub fn new<P: Into<Vector3>, C: Into<Vector3>>(
        position: P,
        color: C,
    ) -> Self {
        Light {
            position: position.into(),
            color: color.into(),
            light_type: LightType::Point,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct LightsRaw {
    // We store these as Vector4 because vectors require 16 byte alignment
    pub positions: [Vector4; MAX_LIGHTS],
    pub colors: [Vector4; MAX_LIGHTS],
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

#[derive(Copy, Clone)]
pub enum LightType {
    Directional,
    Point,
}

unsafe impl bytemuck::Zeroable for LightsRaw {}
unsafe impl bytemuck::Pod for LightsRaw {}
