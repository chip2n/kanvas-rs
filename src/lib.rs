pub mod billboard;
pub mod camera;
pub mod debug;
pub mod forward;
pub mod geometry;
pub mod light;
pub mod model;
pub mod pipeline;
pub mod shader;
pub mod shadow;
pub mod texture;
pub mod ui;

use winit::window::Window;

pub struct Kanvas {
    pub window: Window,
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub swap_chain: wgpu::SwapChain,
    pub shader_compiler: shaderc::Compiler,
}

impl Kanvas {
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
                    features: wgpu::Features::empty(),
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

        Kanvas {
            window,
            surface,
            device,
            queue,
            sc_desc,
            swap_chain,
            shader_compiler,
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
}
