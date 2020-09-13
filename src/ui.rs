use crate::debug;
use crate::shadow;
use crate::Kanvas;

pub struct DebugUi {
    pub is_visible: bool,
    pub shadows_enabled: bool,
    pub context: imgui::Context,
    renderer: imgui_wgpu::Renderer,
    platform: imgui_winit_support::WinitPlatform,
    last_cursor: Option<imgui::MouseCursor>,
    shadow_map_ids: [imgui::TextureId; 6],
}

impl DebugUi {
    pub fn new(kanvas: &Kanvas) -> Self {
        let hidpi_factor = 1.0;
        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut context);
        platform.attach_window(
            context.io_mut(),
            &kanvas.window,
            imgui_winit_support::HiDpiMode::Locked(1.0),
        );
        context.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        context
            .fonts()
            .add_font(&[imgui::FontSource::DefaultFontData {
                config: Some(imgui::FontConfig {
                    oversample_h: 1,
                    pixel_snap_h: true,
                    size_pixels: font_size,
                    ..Default::default()
                }),
            }]);

        // Setup dear imgui wgpu renderer
        let mut renderer = imgui_wgpu::Renderer::new(
            &mut context,
            &kanvas.device,
            &kanvas.queue,
            kanvas.sc_desc.format,
            None,
            1,
        );

        let last_cursor = None;

        let shadow_map_ids = [
            create_texture(&kanvas.device, &mut renderer),
            create_texture(&kanvas.device, &mut renderer),
            create_texture(&kanvas.device, &mut renderer),
            create_texture(&kanvas.device, &mut renderer),
            create_texture(&kanvas.device, &mut renderer),
            create_texture(&kanvas.device, &mut renderer),
        ];

        DebugUi {
            is_visible: false,
            shadows_enabled: true,
            context,
            renderer,
            platform,
            last_cursor,
            shadow_map_ids,
        }
    }

    // TODO store reference to window in struct?
    pub fn handle_event<T>(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::Event<T>,
    ) {
        self.platform
            .handle_event(self.context.io_mut(), window, event);
    }

    pub fn get_texture<'a>(&'a self, id: imgui::TextureId) -> Option<&'a imgui_wgpu::Texture> {
        self.renderer.textures.get(id)
    }

    pub fn render(
        &mut self,
        kanvas: &Kanvas,
        output: &wgpu::SwapChainTexture,
        encoder: &mut wgpu::CommandEncoder,
        debug_pass: &debug::DebugPass,
        shadow_targets: &[shadow::ShadowMapTarget; 6],
    ) {
        for (i, tex) in self.shadow_textures().enumerate() {
            let shadow_target = &shadow_targets[i];
            debug_pass.render(encoder, &tex.view, &shadow_target.bind_group);
        }

        self.platform
            .prepare_frame(self.context.io_mut(), &kanvas.window)
            .expect("Failed to prepare frame");

        let ui = self.context.frame();

        let images: Vec<_> = self
            .shadow_map_ids
            .iter()
            .map(|id| imgui::Image::new(*id, [128.0, 128.0]))
            .collect();

        {
            let window = imgui::Window::new(imgui::im_str!("Shadow Debug"));
            let mut shadows_enabled = self.shadows_enabled;
            window
                .content_size([128.0 * 3.0, 0.0])
                .resizable(false)
                .build(&ui, || {
                    ui.checkbox(imgui::im_str!("Shadows enabled"), &mut shadows_enabled);
                    ui.separator();
                    ui.columns(3, imgui::im_str!("Columnz"), false);
                    for image in images {
                        image.build(&ui);
                        ui.next_column();
                    }
                    ui.columns(1, imgui::im_str!("Columnz"), false);
                });

            self.shadows_enabled = shadows_enabled;
        }

        if self.last_cursor != ui.mouse_cursor() {
            self.last_cursor = ui.mouse_cursor();
            self.platform.prepare_render(&ui, &kanvas.window);
        }

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &output.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        self.renderer
            .render(ui.render(), &kanvas.queue, &kanvas.device, &mut rpass)
            .expect("Rendering failed");
    }

    fn shadow_textures<'a>(&'a self) -> impl Iterator<Item = &'a imgui_wgpu::Texture> + 'a {
        self.shadow_map_ids
            .iter()
            .map(move |id| self.get_texture(*id).unwrap())
    }
}

fn create_texture(device: &wgpu::Device, renderer: &mut imgui_wgpu::Renderer) -> imgui::TextureId {
    let imgui_texture = imgui_wgpu::Texture::new(
        device,
        &renderer,
        1024,
        1024,
        wgpu::TextureFormat::Rgba8Unorm,
        None,
    );
    renderer.textures.insert(imgui_texture)
}
