pub mod billboard;
pub mod camera;
pub mod debug;
pub mod forward;
pub mod geometry;
pub mod light;
pub mod math;
pub mod model;
pub mod pipeline;
pub mod shader;
pub mod shadow;
pub mod texture;
pub mod ui;

pub mod prelude {
    pub use crate::math::*;
    pub use crate::Context;
}

use model::{Material, MaterialId};
use std::collections::HashMap;
use winit::window::Window;

#[derive(Default)]
pub struct Materials {
    next_id: MaterialId,
    materials: HashMap<MaterialId, Material>,
}

impl Materials {
    pub fn insert(&mut self, material: Material) -> MaterialId {
        let id = self.next_id;
        self.materials.insert(id, material);
        self.next_id += 1;
        id
    }

    pub fn get<'a>(&'a self, id: MaterialId) -> &'a Material {
        &self.materials[&id]
    }
}

pub struct Context {
    pub window: Window,
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub swap_chain: wgpu::SwapChain,
    pub shader_compiler: shaderc::Compiler,
    pub materials: Materials,
    pub lights: light::Lights,
    pub instances_bind_group_layout: wgpu::BindGroupLayout,
    pub light_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_normal_bind_group_layout: wgpu::BindGroupLayout,
}

impl Context {
    pub async fn new(window: Window) -> Self {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

        let size = window.inner_size();
        let surface = unsafe { instance.create_surface(&window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::SAMPLED_TEXTURE_BINDING_ARRAY, // used for shadow maps
                    limits: Default::default(),
                    shader_validation: true,
                },
                None,
            )
            .await
            .unwrap();

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT, // write to the screen
            format: wgpu::TextureFormat::Bgra8UnormSrgb, // how the textures will be stored on the gpu
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let shader_compiler = shaderc::Compiler::new().unwrap();

        let mut materials = Materials::default();

        let instances_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::StorageBuffer {
                        dynamic: false,
                        min_binding_size: None,
                        readonly: true,
                    },
                    count: None,
                }],
                label: Some("instances_bind_group_layout"),
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Float,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let texture_normal_bind_group_layout =
                device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::SampledTexture {
                                multisampled: false,
                                dimension: wgpu::TextureViewDimension::D2,
                                component_type: wgpu::TextureComponentType::Float,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::Sampler { comparison: false },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::SampledTexture {
                                multisampled: false,
                                dimension: wgpu::TextureViewDimension::D2,
                                component_type: wgpu::TextureComponentType::Float,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::Sampler { comparison: false },
                            count: None,
                        },
                    ],
                    label: Some("texture_bind_group_layout"),
                });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::UniformBuffer {
                            dynamic: false,
                            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                                light::LightsRaw,
                            >()
                                as _),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: std::num::NonZeroU32::new(light::MAX_LIGHTS as u32),
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::UniformBuffer {
                            dynamic: false,
                            min_binding_size: light::LightConfig::binding_size(),
                        },
                        count: None,
                    },
                ],
                label: None,
            });

        let (bulb_texture, cmd) =
            texture::Texture::load(&device, "res/tex/bulb.png", false).unwrap();
        queue.submit(std::iter::once(cmd));
        let (static_normal_map_texture, cmd) =
            texture::Texture::load(&device, "res/tex/normal_map_static.png", true).unwrap();
        queue.submit(std::iter::once(cmd));

        let light_material = materials.insert(Material::new(
            &device,
            "Light bulb",
            bulb_texture,
            static_normal_map_texture,
            &texture_normal_bind_group_layout,
        ));
        let lights = light::Lights::new(&device, &light_bind_group_layout, light_material);

        Context {
            window,
            surface,
            device,
            queue,
            sc_desc,
            swap_chain,
            shader_compiler,
            materials,
            lights,
            instances_bind_group_layout,
            light_bind_group_layout,
            texture_bind_group_layout,
            texture_normal_bind_group_layout,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn create_encoder(&self) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None })
    }

    pub fn frame(&mut self) -> wgpu::SwapChainFrame {
        self.swap_chain
            .get_current_frame()
            .expect("Timeout getting texture")
    }

    pub fn create_material(
        &mut self,
        name: &str,
        diffuse_texture: texture::Texture,
        normal_texture: texture::Texture,
        layout: &wgpu::BindGroupLayout,
    ) -> MaterialId {
        self.materials.insert(Material::new(
            &self.device,
            name,
            diffuse_texture,
            normal_texture,
            layout,
        ))
    }

    pub fn get_material(&self, id: MaterialId) -> &Material {
        &self.materials.get(id)
    }

    pub fn create_billboard(&mut self) {}
}
