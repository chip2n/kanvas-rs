use crate::debug;
use crate::light;
use crate::Context;

pub struct DebugUi {
    pub is_visible: bool,
    pub shadows_enabled: bool,
    pub camera_pos: cgmath::Point3<f32>,
    pub context: imgui::Context,
    renderer: imgui_wgpu::Renderer,
    platform: imgui_winit_support::WinitPlatform,
    last_cursor: Option<imgui::MouseCursor>,
    shadow_map_ids: Vec<imgui::TextureId>,
    shadow_bind_groups: Vec<wgpu::BindGroup>,
}

impl DebugUi {
    pub fn new(context: &Context, lights: &light::Lights) -> Self {
        let hidpi_factor = 1.0;
        let mut imgui_context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui_context);
        platform.attach_window(
            imgui_context.io_mut(),
            &context.window,
            imgui_winit_support::HiDpiMode::Locked(1.0),
        );
        imgui_context.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        imgui_context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;
        imgui_context.style_mut().scrollbar_size = 8.0;

        imgui_context
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
            &mut imgui_context,
            &context.device,
            &context.queue,
            context.sc_desc.format,
            None,
            1,
        );

        let last_cursor = None;

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

        let shadow_map_ids: Vec<_> = (0..6 * light::MAX_LIGHTS)
            .map(|_| create_texture(&context.device, &mut renderer))
            .collect();

        // Create a texture views for each face of each shadow cubemap texture
        let shadow_texture_views: Vec<wgpu::TextureView> = lights.shadow_textures
            .iter()
            .flat_map(|tex| {
                (0..6)
                    .map(|i| {
                        tex.create_view(&wgpu::TextureViewDescriptor {
                            label: Some("Shadow"),
                            format: None,
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: 0,
                            level_count: None,
                            base_array_layer: i,
                            array_layer_count: None,
                        })
                    })
                    .collect::<Vec<wgpu::TextureView>>()
            })
            .collect();

        let shadow_bind_groups: Vec<_> = shadow_texture_views
            .iter()
            .map(|view| {
                context
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &context.texture_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                            },
                        ],
                        label: None,
                    })
            })
            .collect();

        DebugUi {
            is_visible: false,
            shadows_enabled: true,
            camera_pos: cgmath::Point3::new(0.0, 0.0, 0.0),
            context: imgui_context,
            renderer,
            platform,
            last_cursor,
            shadow_map_ids,
            shadow_bind_groups,
        }
    }

    pub fn handle_event<T>(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::Event<T>,
    ) {
        self.platform
            .handle_event(self.context.io_mut(), window, event);
    }

    pub fn render(
        &mut self,
        context: &Context,
        output: &wgpu::SwapChainTexture,
        encoder: &mut wgpu::CommandEncoder,
        debug_pass: &debug::DebugPass,
    ) {
        // Render each shadow texture into the imgui textures
        {
            let imgui_shadow_textures = self
                .shadow_map_ids
                .iter()
                .map(|id| self.renderer.textures.get(*id).unwrap());

            for (i, tex) in imgui_shadow_textures.enumerate() {
                let shadow_bind_group = &self.shadow_bind_groups[i];
                debug_pass.render(encoder, &tex.view, &shadow_bind_group);
            }
        }

        // Create an imgui Image widget for each shadow texture
        let images: Vec<_> = self
            .shadow_map_ids
            .iter()
            .map(|id| imgui::Image::new(*id, [128.0, 128.0]))
            .collect();

        self.platform
            .prepare_frame(self.context.io_mut(), &context.window)
            .expect("Failed to prepare frame");

        let ui = self.context.frame();

        // Render camera window
        {
            let camera_pos = self.camera_pos;
            let window = imgui::Window::new(imgui::im_str!("Game world"));
            window
                .position([64.0, 64.0], imgui::Condition::FirstUseEver)
                .content_size([256.0, 128.0])
                .build(&ui, || {
                    ui.text("Camera position:");
                    ui.text(format!("- x: {:.2}", camera_pos.x));
                    ui.text(format!("- y: {:.2}", camera_pos.y));
                    ui.text(format!("- z: {:.2}", camera_pos.z));
                });
        }

        // Render shadow debug window
        {
            let window = imgui::Window::new(imgui::im_str!("Shadow Debug"));
            let mut shadows_enabled = self.shadows_enabled;
            window
                .position([64.0, 256.0], imgui::Condition::FirstUseEver)
                .size([128.0 * 3.0, 512.0], imgui::Condition::FirstUseEver)
                .resizable(false)
                .always_vertical_scrollbar(true)
                .build(&ui, || {
                    ui.checkbox(imgui::im_str!("Shadows enabled"), &mut shadows_enabled);
                    ui.separator();

                    for (i, imgs) in images.chunks(6).enumerate() {
                        let title = imgui::ImString::new(format!("Light #{}", i));

                        let header = imgui::CollapsingHeader::new(&title);
                        let is_expanded = header.build(&ui);

                        let style_token =
                            ui.push_style_vars(&[imgui::StyleVar::ItemSpacing([4.0, 4.0])]);
                        if is_expanded {
                            ui.columns(3, imgui::im_str!("Columnz"), false);
                            for img in imgs {
                                img.build(&ui);
                                ui.next_column();
                            }
                            ui.columns(1, imgui::im_str!("Columnz"), false);
                        }
                        style_token.pop(&ui);
                    }
                });

            self.shadows_enabled = shadows_enabled;
        }

        if self.last_cursor != ui.mouse_cursor() {
            self.last_cursor = ui.mouse_cursor();
            self.platform.prepare_render(&ui, &context.window);
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
            .render(ui.render(), &context.queue, &context.device, &mut rpass)
            .expect("Rendering failed");
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
