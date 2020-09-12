mod camera;
mod debug;
mod forward;
mod light;
mod model;
mod pipeline;
mod shader;
mod shadow;
mod texture;
mod ui;

use cgmath::prelude::*;
use futures::executor::block_on;
use model::Vertex;
use std::iter;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let kanvas = block_on(Kanvas::new(window));
    let mut state = State::new(kanvas);

    let mut last_render_time = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.kanvas.window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        WindowEvent::KeyboardInput { input, .. } => match input {
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            } => *control_flow = ControlFlow::Exit,
                            _ => {}
                        },
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    };
                }
            }
            Event::DeviceEvent { ref event, .. } => {
                state.handle_device_event(event);
            }
            Event::RedrawRequested(_) => {
                let now = std::time::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;
                state.update(dt);
                state.render();
            }
            Event::MainEventsCleared => {
                state.kanvas.window.request_redraw();
            }
            _ => {}
        }
        state.debug_ui.handle_event(&state.kanvas.window, &event);
    });
}

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
    async fn new(window: Window) -> Self {
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

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
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

struct State {
    kanvas: Kanvas,
    light_render_pipeline: wgpu::RenderPipeline,
    camera: camera::Camera,
    projection: camera::PerspectiveProjection,
    camera_controller: camera::CameraController,
    forward_pass: forward::ForwardPass,
    instances_bind_group: wgpu::BindGroup,
    instances: Vec<model::Instance>,
    obj_model: model::Model,
    light_model: model::Model,
    light: light::Light,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    light_config: light::LightConfig,
    shadow_pass: shadow::ShadowPass,
    debug_pass: debug::DebugPass,
    debug_ui: ui::DebugUi,
}

impl State {
    fn new(kanvas: Kanvas) -> State {
        let mut kanvas = kanvas;
        let camera = camera::Camera::new((0.0, 10.0, 20.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection = camera::PerspectiveProjection::new(
            kanvas.sc_desc.width,
            kanvas.sc_desc.height,
            cgmath::Deg(45.0),
            0.1,
            100.0,
        );
        let camera_controller = camera::CameraController::new(4.0, 0.8);

        let forward_pass = forward::ForwardPass::new(&mut kanvas);

        let instances = vec![model::Instance {
            position: cgmath::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: cgmath::Quaternion::from_axis_angle(
                cgmath::Vector3::unit_z(),
                cgmath::Deg(0.0),
            ),
        }];
        let instance_data = instances
            .iter()
            .map(model::Instance::to_raw)
            .collect::<Vec<_>>();
        let instance_buffer = kanvas
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instances"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsage::STORAGE,
            });

        let instances_bind_group = kanvas.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &forward_pass.instances_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &instance_buffer,
                    offset: 0,
                    size: None,
                },
            }],
            label: Some("instances_bind_group"),
        });

        let light = light::Light::new((20.0, 20.0, 0.0), (1.0, 1.0, 1.0));

        // We'll want to update our lights position, so we use COPY_DST
        let light_buffer = kanvas
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lights"),
                contents: bytemuck::cast_slice(&[light.to_raw()]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            });

        let vertex_descs = [model::ModelVertex::desc()];

        let shadow_pass = shadow::ShadowPass::new(
            &kanvas.device,
            &mut kanvas.shader_compiler,
            &forward_pass.instances_bind_group_layout,
            &vertex_descs,
        );

        let light_config = light::LightConfig::new(&kanvas.device);

        // TODO do some of this in shadow pass?
        let light_bind_group = kanvas.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &forward_pass.light_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &light_buffer,
                        offset: 0,
                        size: None,
                    },
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_pass.cube_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_pass.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: light_config.binding_resource(),
                },
            ],
            label: None,
        });

        let light_render_pipeline = {
            let layout = kanvas
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Light render pipeline"),
                    push_constant_ranges: &[],
                    bind_group_layouts: &[
                        &forward_pass.uniform_bind_group_layout,
                        &forward_pass.light_bind_group_layout,
                    ],
                });

            let vs_module =
                compile_vertex!(&kanvas.device, &mut kanvas.shader_compiler, "light.vert").unwrap();
            let fs_module =
                compile_frag!(&kanvas.device, &mut kanvas.shader_compiler, "light.frag").unwrap();

            pipeline::create(
                &"light",
                &kanvas.device,
                &layout,
                &vs_module,
                &fs_module,
                Some(kanvas.sc_desc.format),
                Some(pipeline::DepthConfig::no_bias()),
                &vertex_descs,
            )
        };

        let (obj_model, cmds) = model::Model::load(
            &kanvas.device,
            &forward_pass.texture_bind_group_layout,
            "res/models/scene.obj",
        )
        .unwrap();
        kanvas.queue.submit(cmds);

        let (light_model, cmds) = model::Model::load(
            &kanvas.device,
            &forward_pass.texture_bind_group_layout,
            "res/models/cube.obj",
        )
        .unwrap();
        kanvas.queue.submit(cmds);

        let debug_pass = debug::DebugPass::new(
            &kanvas.device,
            &shadow_pass.target_bind_group_layout,
            // TODO use own bind group layout?
            &forward_pass.uniform_bind_group_layout,
        );

        let debug_ui = ui::DebugUi::new(&kanvas);

        State {
            kanvas,
            light_render_pipeline,
            camera,
            projection,
            camera_controller,
            forward_pass,
            instances_bind_group,
            instances,
            obj_model,
            light_model,
            light,
            light_buffer,
            light_bind_group,
            light_config,
            shadow_pass,
            debug_pass,
            debug_ui,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.kanvas.resize(new_size);
        self.forward_pass
            .resize(&self.kanvas.device, &self.kanvas.sc_desc);
        self.projection.resize(new_size.width, new_size.height);
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => match key {
                VirtualKeyCode::Escape => {
                    if *state == ElementState::Pressed {
                        if self.camera_controller.is_active {
                            self.ungrab_camera();
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                VirtualKeyCode::Z => {
                    if *state == ElementState::Pressed {
                        self.debug_ui.is_visible = !self.debug_ui.is_visible;
                        true
                    } else {
                        false
                    }
                }
                _ => self.camera_controller.process_keyboard(*key, *state),
            },
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                ..
            } => {
                self.grab_camera();
                true
            }
            _ => false,
        }
    }

    fn grab_camera(&mut self) {
        self.camera_controller.is_active = true;
        self.kanvas.window.set_cursor_visible(false);
        self.kanvas.window.set_cursor_grab(true).unwrap();
    }

    fn ungrab_camera(&mut self) {
        self.camera_controller.is_active = false;
        self.kanvas.window.set_cursor_visible(true);
        self.kanvas.window.set_cursor_grab(false).unwrap();
    }

    fn handle_device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                let (mouse_dx, mouse_dy) = *delta;
                self.camera_controller.process_mouse(mouse_dx, mouse_dy);
                true
            }
            _ => false,
        }
    }

    fn update(&mut self, dt: std::time::Duration) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.forward_pass
            .uniforms
            .update_view_proj(&self.camera, &self.projection);

        let mut encoder = self.kanvas.create_encoder();

        self.forward_pass
            .upload_uniforms(&self.kanvas.device, &mut encoder);

        // Update the light
        let old_position = self.light.position;
        self.light.position = cgmath::Quaternion::from_axis_angle(
            (0.0, 1.0, 0.0).into(),
            cgmath::Deg(60.0 * dt.as_secs_f32()),
        ) * old_position;
        let staging_buffer =
            self.kanvas
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Staging"),
                    contents: bytemuck::cast_slice(&[self.light.to_raw()]),
                    usage: wgpu::BufferUsage::COPY_SRC,
                });
        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.light_buffer,
            0,
            std::mem::size_of::<light::LightRaw>() as wgpu::BufferAddress,
        );

        // Update light config
        self.light_config.shadows_enabled = self.debug_ui.shadows_enabled;
        self.light_config.upload(&self.kanvas.queue);

        self.shadow_pass
            .update_light(&self.kanvas.queue, &self.light);

        self.kanvas.queue.submit(iter::once(encoder.finish()));
    }

    fn render(&mut self) {
        let frame = self.kanvas.frame();
        let mut encoder = self.kanvas.create_encoder();

        self.render_with_encoder(&mut encoder, &frame.output);

        for (i, tex) in self.debug_ui.shadow_textures().enumerate() {
            let shadow_target = &self.shadow_pass.targets[i];
            self.debug_pass.render(
                &mut encoder,
                &tex.view,
                &shadow_target.bind_group,
                &self.forward_pass.uniform_bind_group,
            );
        }

        self.debug_ui
            .render(&self.kanvas, &frame.output, &mut encoder);

        self.kanvas.queue.submit(iter::once(encoder.finish()));
    }

    fn render_with_encoder(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::SwapChainTexture,
    ) {
        use light::DrawLight;
        use model::DrawModel;

        let back_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };

        // clear the screen
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &frame.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(back_color),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        if self.light_config.shadows_enabled {
            for face_index in 0..6 {
                // shadow pass
                let mut pass = self.shadow_pass.begin(encoder, face_index);
                for mesh in &self.obj_model.meshes {
                    pass.render(
                        shadow::ShadowPassRenderData::from_mesh(&mesh, &self.instances_bind_group),
                        face_index,
                    );
                }
            }
            self.shadow_pass.copy_to_cubemap(encoder);
        }

        {
            // forward pass
            let mut render_pass = self.forward_pass.begin(&frame.view, encoder);
            render_pass.set_pipeline(&self.forward_pass.pipeline);

            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.forward_pass.uniform_bind_group,
                &self.instances_bind_group,
                &self.light_bind_group,
            );

            render_pass.set_pipeline(&self.light_render_pipeline);

            render_pass.draw_light_model(
                &self.light_model,
                &self.forward_pass.uniform_bind_group,
                &self.light_bind_group,
            );
        }
    }
}
