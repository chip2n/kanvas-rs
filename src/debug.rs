use crate::texture;
use std::ops::Deref;

pub fn copy_texture_to_new_buffer(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &texture::Texture,
) -> (wgpu::Buffer, wgpu::BufferAddress) {
    let width = texture.size.width;
    let height = texture.size.height;
    let u32_size = std::mem::size_of::<u32>() as u32;
    let output_buffer_size = (u32_size * width * height) as wgpu::BufferAddress;
    let output_buffer_desc = wgpu::BufferDescriptor {
        size: output_buffer_size,
        usage: wgpu::BufferUsage::COPY_DST
            // this tells wpgu that we want to read this buffer from the cpu
            | wgpu::BufferUsage::MAP_READ,

        mapped_at_creation: false,
        label: None,
    };
    let output_buffer = device.create_buffer(&output_buffer_desc);

    encoder.copy_texture_to_buffer(
        wgpu::TextureCopyView {
            texture: &texture.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::BufferCopyView {
            buffer: &output_buffer,
            layout: wgpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: u32_size * width,
                rows_per_image: height,
            },
        },
        texture.size,
    );
    (output_buffer, output_buffer_size)
}

pub async fn save_buffer(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    buffer_size: wgpu::BufferAddress, // TODO unused
    width: u32,
    height: u32,
) {
    // NOTE: We have to create the mapping THEN device.poll(). If we don't
    // the application will freeze.
    let slice = buffer.slice(..);
    let mapping = slice.map_async(wgpu::MapMode::Read);
    device.poll(wgpu::Maintain::Wait);

    let _ = mapping.await.unwrap();
    let buffer_view = slice.get_mapped_range();
    let data = buffer_view.deref();

    /*
    use image::{ImageBuffer, Rgba};
    let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
        width,
        height,
        data,
    ).unwrap();

    buffer.save("image.png").unwrap();
    */

    let pixels: &[f32] = bytemuck::try_cast_slice(data).unwrap();
    let (near, far) = (0.1, 100.0);

    use image::{ImageBuffer, Pixel, Rgba};
    let mut buffer = ImageBuffer::<Rgba<u8>, _>::new(width, height);

    let mut x = 0;
    let mut y = 0;
    for pixel in pixels {
        let z = pixel * 2.0 - 1.0;
        let r = (2.0 * near * far) / (far + near - z * (far - near));
        let p = (r.floor() * 255.0 / far) as u8;

        buffer.put_pixel(x, y, Pixel::from_channels(p, p, p, 255));

        x += 1;
        if x >= width {
            x = 0;
            y += 1;
        }
    }

    buffer.save("image.png").unwrap();
}
