use anyhow::Context;
use image::GenericImageView;
use std::path::Path;
use wgpu::util::DeviceExt;

pub struct Texture {
    pub size: wgpu::Extent3d,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn load<P: AsRef<Path>>(
        device: &wgpu::Device,
        path: P,
        is_normal_map: bool,
    ) -> Result<(Self, wgpu::CommandBuffer), anyhow::Error> {
        let path = path.as_ref();
        let path_copy = path.to_path_buf();
        let label = path_copy.to_str();
        let img = image::open(path)
            .with_context(|| format!("texture path: {}", path.to_string_lossy()))?;
        Self::from_image(device, &img, label, is_normal_map)
    }

    pub fn create_depth_texture(
        device: &wgpu::Device,
        sc_desc: &wgpu::SwapChainDescriptor,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: sc_desc.width,
            height: sc_desc.height,
            depth: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT // we're rendering to this texture
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::COPY_SRC,
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            size,
            texture,
            view,
            sampler,
        }
    }

    pub fn from_image(
        device: &wgpu::Device,
        img: &image::DynamicImage,
        label: Option<&str>,
        is_normal_map: bool,
    ) -> Result<(Self, wgpu::CommandBuffer), anyhow::Error> {
        let rgba = img.to_rgba();
        let dimensions = img.dimensions();

        // All textures are stored as 3d
        // We represent our 2d texture by setting depth of 1
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if is_normal_map {
                wgpu::TextureFormat::Rgba8Unorm
            } else {
                wgpu::TextureFormat::Rgba8UnormSrgb
            },
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
            label,
        });

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label,
            contents: bytemuck::cast_slice(&rgba),
            usage: wgpu::BufferUsage::COPY_SRC,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("texture_buffer_copy_encoder"),
        });

        encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &buffer,
                layout: wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: 4 * dimensions.0,
                    rows_per_image: dimensions.1,
                },
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            size,
        );

        let cmd_buffer = encoder.finish();
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare: None,
            ..Default::default()
        });

        Ok((
            Self {
                size,
                texture,
                view,
                sampler,
            },
            cmd_buffer,
        ))
    }
}
