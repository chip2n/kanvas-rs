use crate::billboard::{Billboard, BillboardId, Billboards};
use crate::model;
use crate::prelude::*;
use crate::shadow;
use wgpu::util::DeviceExt;

pub const MAX_LIGHTS: usize = 2;

pub struct Lights {
    pub lights: [Option<Light>; MAX_LIGHTS],
    pub shadow_textures: [wgpu::Texture; MAX_LIGHTS],

    pub config: LightConfig,
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl Lights {
    pub fn new(
        context: &Context,
        billboards: &mut Billboards,
        light_material: model::MaterialId,
    ) -> Self {
        let config = LightConfig::new(&context.device);

        let lights = [
            {
                let position: Vector3 = (-15.0, 12.0, 8.0).into();
                let billboard = billboards.insert(
                    &context,
                    Billboard {
                        position,
                        material: light_material,
                    },
                );
                Some(Light::new(position, (1.0, 1.0, 1.0), billboard))
            },
            {
                let position: Vector3 = (10.0, 10.0, 0.0).into();
                let billboard = billboards.insert(
                    &context,
                    Billboard {
                        position,
                        material: light_material,
                    },
                );
                Some(Light::new(position, (1.0, 1.0, 1.0), billboard))
            },
        ];

        let buffer_data = Self::map_raw(&lights);

        // We'll want to update our lights position, so we use COPY_DST
        let buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lights"),
                contents: bytemuck::cast_slice(&[buffer_data]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            });

        let shadow_sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
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
            let shadow_cubemap1 = shadow::ShadowCubemap::new(context);
            let shadow_cubemap2 = shadow::ShadowCubemap::new(context);

            (
                [shadow_cubemap1.texture, shadow_cubemap2.texture],
                [shadow_cubemap1.texture_view, shadow_cubemap2.texture_view],
            )
        };

        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &context.light_bind_group_layout,
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
                        resource: wgpu::BindingResource::Sampler(&shadow_sampler), // TODO don't store sampler in every cubemap (?)
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
            config,
            buffer,
            bind_group,
        }
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

pub struct Light {
    pub position: Vector3,
    pub color: Vector3,
    pub light_type: LightType,
    pub billboard: BillboardId,
}

impl Light {
    pub fn new<P: Into<Vector3>, C: Into<Vector3>>(
        position: P,
        color: C,
        billboard: BillboardId,
    ) -> Self {
        Light {
            position: position.into(),
            color: color.into(),
            light_type: LightType::Point,
            billboard,
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

pub enum LightType {
    Directional,
    Point,
}

unsafe impl bytemuck::Zeroable for LightsRaw {}
unsafe impl bytemuck::Pod for LightsRaw {}
