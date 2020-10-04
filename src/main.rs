use futures::executor::block_on;
use geometry::Vertex;
use kanvas::prelude::*;
use kanvas::*;
use model::DrawModel;
use std::iter;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let context = block_on(Context::new(window));
    let mut state = State::new(context);

    let mut last_render_time = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.context.window.id() => {
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
                state.context.window.request_redraw();
            }
            _ => {}
        }
        state.debug_ui.handle_event(&state.context.window, &event);
    });
}

struct State {
    context: Context,
    camera: camera::Camera,
    projection: camera::PerspectiveProjection,
    camera_controller: camera::CameraController,
    forward_pass: forward::ForwardPass,
    instances_bind_group: wgpu::BindGroup,
    instances: Vec<model::Instance>,
    obj_model: model::Model,
    billboards: billboard::Billboards,
    shadow_pass: shadow::ShadowPass,
    debug_pass: debug::DebugPass,
    debug_ui: ui::DebugUi,
    light_billboards: [Option<billboard::BillboardId>; light::MAX_LIGHTS],
}

impl State {
    fn new(context: Context) -> State {
        let mut context = context;
        let camera = camera::Camera::new((0.0, 10.0, 20.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection = camera::PerspectiveProjection::new(
            context.sc_desc.width,
            context.sc_desc.height,
            cgmath::Deg(45.0),
            0.1,
            100.0,
        );
        let camera_controller = camera::CameraController::new(4.0, 0.8);

        let forward_pass = forward::ForwardPass::new(&mut context);

        let instances = vec![model::Instance {
            position: Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: cgmath::Quaternion::from_axis_angle(Vector3::unit_z(), cgmath::Deg(0.0)),
        }];
        let instance_data = instances
            .iter()
            .map(model::Instance::to_raw)
            .collect::<Vec<_>>();
        let instance_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instances"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsage::STORAGE,
                });

        let instances_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &context.instances_bind_group_layout,
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

        let vertex_descs = [model::ModelVertex::desc()];

        let shadow_pass = shadow::ShadowPass::new(
            &context.device,
            &mut context.shader_compiler,
            &context.instances_bind_group_layout,
            &vertex_descs,
        );

        let (obj_model, cmds) = model::Model::load(
            &context.device,
            &context.texture_normal_bind_group_layout,
            "res/models/scene.obj",
        )
        .unwrap();
        context.queue.submit(cmds);

        let mut billboards = billboard::Billboards::new(&context);
        let mut light_billboards: [Option<billboard::BillboardId>; light::MAX_LIGHTS] = [None; light::MAX_LIGHTS];

        {
            let position: Vector3 = (-15.0, 12.0, 8.0).into();
            let billboard = billboards.insert(
                &context,
                billboard::Billboard {
                    position,
                    material: context.lights.material,
                },
            );
            let light_id = context.lights.add_light(position).unwrap();
            light_billboards[light_id] = Some(billboard);
        }

        {
            let position: Vector3 = (10.0, 10.0, 8.0).into();
            let billboard = billboards.insert(
                &context,
                billboard::Billboard {
                    position,
                    material: context.lights.material,
                },
            );
            let light_id = context.lights.add_light(position).unwrap();
            light_billboards[light_id] = Some(billboard);
        }

        let debug_pass = debug::DebugPass::new(&mut context);
        let debug_ui = ui::DebugUi::new(&context, &context.lights);

        State {
            context,
            camera,
            projection,
            camera_controller,
            forward_pass,
            instances_bind_group,
            instances,
            obj_model,
            billboards,
            shadow_pass,
            debug_pass,
            debug_ui,
            light_billboards,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.context.resize(new_size);
        self.forward_pass
            .resize(&self.context.device, &self.context.sc_desc);
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
                VirtualKeyCode::Z => {
                    if *state == ElementState::Pressed {
                        if !self.camera_controller.is_active {
                            self.grab_camera();
                            self.debug_ui.is_visible = false;
                        } else {
                            self.ungrab_camera();
                            self.debug_ui.is_visible = true;
                        }
                        true
                    } else {
                        false
                    }
                }
                _ => self.camera_controller.process_keyboard(*key, *state),
            },
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                ..
            } => {
                if !self.debug_ui.context.io().want_capture_mouse {
                    self.grab_camera();
                }
                true
            }
            _ => false,
        }
    }

    fn grab_camera(&mut self) {
        self.camera_controller.is_active = true;
        self.context.window.set_cursor_visible(false);
        self.context.window.set_cursor_grab(true).unwrap();
    }

    fn ungrab_camera(&mut self) {
        self.camera_controller.is_active = false;
        self.context.window.set_cursor_visible(true);
        self.context.window.set_cursor_grab(false).unwrap();
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

        let mut encoder = self.context.create_encoder();

        self.forward_pass
            .upload_uniforms(&self.context.device, &mut encoder);

        // Update the light
        {
            for (i, light) in self.context.lights.lights.iter_mut().enumerate() {
                if let Some(light) = light {
                    let old_position = light.position;
                    light.position = cgmath::Quaternion::from_axis_angle(
                        (0.0, 1.0, 0.0).into(),
                        cgmath::Deg(60.0 * dt.as_secs_f32()),
                    ) * old_position;

                    let light_billboard = self.light_billboards[i].unwrap();
                    if let Some(billboard) = self.billboards.get(light_billboard) {
                        billboard.position = light.position;
                    }
                }
            }

            let raw_lights = self.context.lights.to_raw();
            let staging_buffer =
                self.context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Staging"),
                        contents: bytemuck::cast_slice(&[raw_lights]),
                        usage: wgpu::BufferUsage::COPY_SRC,
                    });
            encoder.copy_buffer_to_buffer(
                &staging_buffer,
                0,
                &self.context.lights.buffer,
                0,
                std::mem::size_of::<light::LightsRaw>() as wgpu::BufferAddress,
            );
        }

        // Update light config
        self.context.lights.config.shadows_enabled = self.debug_ui.shadows_enabled;
        self.context.lights.config.upload(&self.context.queue);

        for (i, light) in self.context.lights.lights.iter().enumerate() {
            if let Some(light) = light {
                self.shadow_pass
                    .update_light(&self.context.queue, i, &light);
            }
        }

        self.context.queue.submit(iter::once(encoder.finish()));

        // Update billboards
        self.billboards.upload(&self.context, &self.camera);

        // Update ui
        self.debug_ui.camera_pos = self.camera.position;
    }

    fn render(&mut self) {
        let frame = self.context.frame();
        let mut encoder = self.context.create_encoder();

        // clear the screen
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &frame.output.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        // render shadow maps
        if self.context.lights.config.shadows_enabled {
            for (i, light) in self.context.lights.lights.iter().enumerate() {
                if let Some(_) = light {
                    for face_index in 0..6 {
                        // shadow pass
                        let mut pass = self.shadow_pass.begin(&mut encoder, face_index);
                        for mesh in &self.obj_model.meshes {
                            pass.render(
                                shadow::ShadowPassRenderData::from_mesh(
                                    &mesh,
                                    &self.instances_bind_group,
                                ),
                                face_index,
                                i,
                            );
                        }
                    }

                    self.shadow_pass
                        .copy_to_cubemap(&mut encoder, &self.context.lights.shadow_textures[i]);
                }
            }
        }

        {
            // forward pass
            let mut render_pass = self.forward_pass.begin(&frame.output.view, &mut encoder);
            render_pass.set_pipeline(&self.forward_pass.pipeline);

            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.forward_pass.uniform_bind_group,
                &self.instances_bind_group,
                &self.context.lights.bind_group,
            );

            render_pass.set_pipeline(&self.forward_pass.billboard_pipeline);
            self.billboards.render(
                &mut render_pass,
                &self.context.materials,
                &self.forward_pass.uniform_bind_group,
            );
        }

        // Render debug UI
        if self.debug_ui.is_visible {
            self.debug_ui
                .render(&self.context, &frame.output, &mut encoder, &self.debug_pass);
        }

        self.context.queue.submit(iter::once(encoder.finish()));
    }
}
