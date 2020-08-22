mod camera;
mod camera2;
mod debug;
mod light;
mod model;
mod shader;
mod shadow;
mod texture;
mod uniform;

use cgmath::prelude::*;
use futures::executor::block_on;
use model::Vertex;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut state = block_on(State::new(window));
    let mut last_render_time = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window.id() => {
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
            state.window.request_redraw();
        }
        _ => {}
    });
}

struct State {
    window: Window,
    surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    render_pipeline: wgpu::RenderPipeline,
    render_pipeline2: wgpu::RenderPipeline,
    render_pipeline_debug_depth: wgpu::RenderPipeline,
    light_render_pipeline: wgpu::RenderPipeline,
    size: winit::dpi::PhysicalSize<u32>,
    camera: camera2::Camera,
    projection: camera2::Projection,
    camera_controller: camera2::CameraController,
    uniforms: uniform::Uniforms,
    uniform_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    instances_bind_group_layout: wgpu::BindGroupLayout,
    instances_bind_group: wgpu::BindGroup,
    instances: Vec<model::Instance>,
    instance_buffer: wgpu::Buffer,
    plane_instances_bind_group: wgpu::BindGroup,
    depth_texture: texture::Texture,
    obj_model: model::Model,
    plane_model: model::Model,
    light_model: model::Model,
    light: light::Light,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    shadow_pass: shadow::Pass,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    test_material: model::Material,
}

impl State {
    async fn new(window: Window) -> State {
        let size = window.inner_size();
        let surface = wgpu::Surface::create(&window);
        let adapter = wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::VULKAN,
        )
        .await
        .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                extensions: wgpu::Extensions {
                    anisotropic_filtering: false,
                },
                limits: Default::default(),
            })
            .await;

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT, // write to the screen
            format: wgpu::TextureFormat::Bgra8UnormSrgb, // how the textures will be stored on the gpu
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        // A BindGroup describes a set of resources and how they can be accessed by a shader.
        // We create a BindGroup using a BindGroupLayout.
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Uint,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let camera =
            camera2::Camera::new((0.0, 10.0, 20.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection =
            camera2::Projection::new(sc_desc.width, sc_desc.height, cgmath::Deg(45.0), 0.1, 100.0);
        let camera_controller = camera2::CameraController::new(4.0, 0.8);

        let mut uniforms = uniform::Uniforms::new();
        uniforms.update_view_proj(&camera, &projection);
        let uniform_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&[uniforms]),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let globals_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
                label: Some("globals_bind_group_layout"),
            });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &globals_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    range: 0..std::mem::size_of_val(&uniforms) as wgpu::BufferAddress,
                },
            }],
            label: Some("globals_bind_group"),
        });

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
        let instance_buffer_size =
            instance_data.len() * std::mem::size_of::<cgmath::Matrix4<f32>>();
        let instance_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&instance_data),
            wgpu::BufferUsage::STORAGE_READ,
        );

        let instances_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::StorageBuffer {
                        dynamic: false,
                        readonly: true,
                    },
                }],
                label: Some("instances_bind_group_layout"),
            });

        let instances_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &instances_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &instance_buffer,
                    range: 0..instance_buffer_size as wgpu::BufferAddress,
                },
            }],
            label: Some("instances_bind_group"),
        });

        let plane_instances = vec![model::Instance {
            position: cgmath::Vector3 {
                x: 0.0,
                y: 5.0,
                z: -3.0,
            },
            rotation: cgmath::Quaternion::from_axis_angle(
                cgmath::Vector3::unit_z(),
                cgmath::Deg(180.0),
            ) * cgmath::Quaternion::from_axis_angle(
                cgmath::Vector3::unit_x(),
                cgmath::Deg(90.0),
            ),
        }];
        let plane_instance_data = plane_instances
            .iter()
            .map(model::Instance::to_raw)
            .collect::<Vec<_>>();
        let plane_instance_buffer_size =
            plane_instance_data.len() * std::mem::size_of::<cgmath::Matrix4<f32>>();
        let plane_instance_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&plane_instance_data),
            wgpu::BufferUsage::STORAGE_READ,
        );
        let plane_instances_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &instances_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &plane_instance_buffer,
                    range: 0..plane_instance_buffer_size as wgpu::BufferAddress,
                },
            }],
            label: Some("plane_instances_bind_group"),
        });

        let light = light::Light::new((1.5, 4.5, 1.5).into(), (1.0, 1.0, 1.0).into());

        // We'll want to update our lights position, so we use COPY_DST
        let light_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&[light]),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
                label: None,
            });
        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &light_buffer,
                    range: 0..std::mem::size_of_val(&light) as wgpu::BufferAddress,
                },
            }],
            label: None,
        });

        let mut shader_compiler = shaderc::Compiler::new().unwrap();
        let vertex_descs = [model::ModelVertex::desc()];

        let render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &globals_bind_group_layout,
                    &instances_bind_group_layout,
                    &light_bind_group_layout,
                ],
            });

            let vs_src = include_str!("shader.vert");
            let fs_src = include_str!("shader.frag");

            create_render_pipeline(
                &device,
                &layout,
                sc_desc.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &vertex_descs,
                &mut shader_compiler,
                vs_src,
                fs_src,
            )
        };
        let render_pipeline2 = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &globals_bind_group_layout,
                    &instances_bind_group_layout,
                    &light_bind_group_layout,
                ],
            });

            let vs_src = include_str!("shader.vert");
            let fs_src = include_str!("shader.frag");

            create_render_pipeline2(
                &device,
                &layout,
                Some(texture::Texture::DEPTH_FORMAT),
                &vertex_descs,
                &mut shader_compiler,
                vs_src,
                fs_src,
            )
        };

        let render_pipeline_debug_depth = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &globals_bind_group_layout,
                    &instances_bind_group_layout,
                    &light_bind_group_layout,
                ],
            });

            let vs_src = include_str!("debug_depth.vert");
            let fs_src = include_str!("debug_depth.frag");

            create_render_pipeline(
                &device,
                &layout,
                sc_desc.format,
                None,
                &vertex_descs,
                &mut shader_compiler,
                vs_src,
                fs_src,
            )
        };

        let shadow_pass = shadow::create_pass(
            &device,
            &mut shader_compiler,
            &texture_bind_group_layout,
            &globals_bind_group_layout,
            &instances_bind_group_layout,
            &light_bind_group_layout,
            &vertex_descs,
        );

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&globals_bind_group_layout, &light_bind_group_layout],
            });

            let vs_src = include_str!("light.vert");
            let fs_src = include_str!("light.frag");

            create_render_pipeline(
                &device,
                &layout,
                sc_desc.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &vertex_descs,
                &mut shader_compiler,
                vs_src,
                fs_src,
            )
        };

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &sc_desc, "depth_texture");

        let (obj_model, cmds) =
            model::Model::load(&device, &texture_bind_group_layout, "res/models/scene.obj")
                .unwrap();
        queue.submit(&cmds);

        let (light_model, cmds) =
            model::Model::load(&device, &texture_bind_group_layout, "res/models/cube.obj").unwrap();
        queue.submit(&cmds);

        let (plane_model, cmds) =
            model::Model::load(&device, &texture_bind_group_layout, "res/models/plane.obj")
                .unwrap();
        queue.submit(&cmds);

        let test_material =
            create_test_material(&device, &sc_desc, &texture_bind_group_layout, &queue);

        State {
            window,
            surface,
            adapter,
            device,
            queue,
            sc_desc,
            swap_chain,
            render_pipeline,
            render_pipeline2,
            render_pipeline_debug_depth,
            light_render_pipeline,
            size,
            camera,
            projection,
            camera_controller,
            uniforms,
            uniform_buffer,
            globals_bind_group,
            instances_bind_group_layout,
            instances_bind_group,
            instances,
            instance_buffer,
            plane_instances_bind_group,
            depth_texture,
            obj_model,
            plane_model,
            light_model,
            light,
            light_buffer,
            light_bind_group,
            shadow_pass,
            texture_bind_group_layout,
            test_material,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
        self.depth_texture =
            texture::Texture::create_depth_texture(&self.device, &self.sc_desc, "depth_texture");

        self.projection.resize(new_size.width, new_size.height);

        self.test_material = create_test_material(
            &self.device,
            &self.sc_desc,
            &self.texture_bind_group_layout,
            &self.queue,
        );
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
        self.window.set_cursor_visible(false);
        self.window.set_cursor_grab(true).unwrap();
    }

    fn ungrab_camera(&mut self) {
        self.camera_controller.is_active = false;
        self.window.set_cursor_visible(true);
        self.window.set_cursor_grab(false).unwrap();
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
        self.uniforms
            .update_view_proj(&self.camera, &self.projection);

        // Copy operations are performed on the gpu, so we'll need
        // a CommandEncoder for that
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("update encoder"),
            });

        let staging_buffer = self.device.create_buffer_with_data(
            bytemuck::cast_slice(&[self.uniforms]),
            wgpu::BufferUsage::COPY_SRC,
        );

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.uniform_buffer,
            0,
            std::mem::size_of::<uniform::Uniforms>() as wgpu::BufferAddress,
        );

        // Update the light
        let old_position = self.light.position;
        self.light.position = cgmath::Quaternion::from_axis_angle(
            (0.0, 1.0, 0.0).into(),
            cgmath::Deg(60.0 * dt.as_secs_f32()),
        ) * old_position;
        let staging_buffer = self.device.create_buffer_with_data(
            bytemuck::cast_slice(&[self.light]),
            wgpu::BufferUsage::COPY_SRC,
        );
        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.light_buffer,
            0,
            std::mem::size_of::<light::Light>() as wgpu::BufferAddress,
        );

        // We need to remember to submit our CommandEncoder's output
        // otherwise we won't see any change.
        self.queue.submit(&[encoder.finish()]);
    }

    fn render(&mut self) {
        // Get a frame to render to
        let frame = self
            .swap_chain
            .get_next_texture()
            .expect("Timeout getting texture");

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.render_with_encoder(&mut encoder, &frame.view);

        let texture = &self.test_material.diffuse_texture;
        let (buffer, buffer_size) = debug::copy_texture_to_new_buffer(&self.device, &mut encoder, &texture);

        self.queue.submit(&[encoder.finish()]);

        //block_on(debug::save_buffer(&self.device, &buffer, buffer_size, texture.size.width, texture.size.height));
        //block_on(debug::save_texture(&self.device, &self.depth_texture));
    }

    fn render_with_encoder(&self, encoder: &mut wgpu::CommandEncoder, frame: &wgpu::TextureView) {
        use light::DrawLight;
        use model::DrawModel;

        // clear the screen
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: frame,
                resolve_target: None,
                load_op: wgpu::LoadOp::Clear,
                store_op: wgpu::StoreOp::Store,
                clear_color: wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                },
            }],
            depth_stencil_attachment: None,
        });

        // clear the test material
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &self.test_material.diffuse_texture.view,
                resolve_target: None,
                load_op: wgpu::LoadOp::Clear,
                store_op: wgpu::StoreOp::Store,
                clear_color: wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                },
            }],
            depth_stencil_attachment: None,
        });

        {
            // shadow pass

            /*
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.shadow_pass.target_view,
                    depth_load_op: wgpu::LoadOp::Clear,
                    depth_store_op: wgpu::StoreOp::Store,
                    clear_depth: 1.0,
                    stencil_load_op: wgpu::LoadOp::Clear,
                    stencil_store_op: wgpu::StoreOp::Store,
                    clear_stencil: 0,
                }),
            });

            shadow_pass.set_pipeline(&self.shadow_pass.pipeline);
            // TODO use e.g. DrawShadow trait?
            shadow_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.globals_bind_group,
                &self.instances_bind_group,
                &self.light_bind_group,
            );
            */
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[],
                /*
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    //attachment: &frame.view,
                    attachment: &self.test_material.diffuse_texture.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Load, // prevent clearing after each draw
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    },
                }],
                */
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    //attachment: &self.shadow_pass.target_view,
                    //attachment: &self.depth_texture.view,
                    attachment: &self.test_material.diffuse_texture.view,
                    depth_load_op: wgpu::LoadOp::Clear,
                    depth_store_op: wgpu::StoreOp::Store,
                    clear_depth: 1.0,
                    stencil_load_op: wgpu::LoadOp::Clear,
                    stencil_store_op: wgpu::StoreOp::Store,
                    clear_stencil: 0,
                }),
            });

            render_pass.set_pipeline(&self.render_pipeline2);

            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.globals_bind_group,
                &self.instances_bind_group,
                &self.light_bind_group,
            );
        }

        {
            // forward pass

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                // where we're going to draw our color to
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Load, // prevent clearing after each draw
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.depth_texture.view,
                    depth_load_op: wgpu::LoadOp::Clear,
                    depth_store_op: wgpu::StoreOp::Store,
                    clear_depth: 1.0,
                    stencil_load_op: wgpu::LoadOp::Clear,
                    stencil_store_op: wgpu::StoreOp::Store,
                    clear_stencil: 0,
                }),
            });

            render_pass.set_pipeline(&self.render_pipeline);

            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.globals_bind_group,
                &self.instances_bind_group,
                &self.light_bind_group,
            );

            render_pass.set_pipeline(&self.light_render_pipeline);

            render_pass.draw_light_model(
                &self.light_model,
                &self.globals_bind_group,
                &self.light_bind_group,
            );
        }

        {
            // debug depth pass

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                // where we're going to draw our color to
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Load, // prevent clearing after each draw
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline_debug_depth);

            render_pass.draw_model_instanced_with_material(
                &self.plane_model,
                &self.test_material,
                0..1,
                &self.globals_bind_group,
                &self.plane_instances_bind_group,
                &self.light_bind_group,
            );
        }
    }
}

fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_descs: &[wgpu::VertexBufferDescriptor],
    shader_compiler: &mut shaderc::Compiler,
    vs_src: &str,
    fs_src: &str,
) -> wgpu::RenderPipeline {
    let vs_module =
        shader::create_vertex_module(device, shader_compiler, vs_src, "shader.vert").unwrap();
    let fs_module =
        shader::create_fragment_module(device, shader_compiler, fs_src, "shader.frag").unwrap();

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        // description of how to process triangles
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::Back,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        // description on how color are stored and processed throughout the pipeline
        color_states: &[wgpu::ColorStateDescriptor {
            format: color_format,
            color_blend: wgpu::BlendDescriptor::REPLACE,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            write_mask: wgpu::ColorWrite::ALL,
        }],
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        depth_stencil_state: depth_format.map(|format| wgpu::DepthStencilStateDescriptor {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: vertex_descs,
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}

fn create_render_pipeline2(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_descs: &[wgpu::VertexBufferDescriptor],
    shader_compiler: &mut shaderc::Compiler,
    vs_src: &str,
    fs_src: &str,
) -> wgpu::RenderPipeline {
    let vs_module =
        shader::create_vertex_module(device, shader_compiler, vs_src, "shader.vert").unwrap();
    let fs_module =
        shader::create_fragment_module(device, shader_compiler, fs_src, "shader.frag").unwrap();

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        // description of how to process triangles
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::Back,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        // description on how color are stored and processed throughout the pipeline
        color_states: &[],
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        depth_stencil_state: depth_format.map(|format| wgpu::DepthStencilStateDescriptor {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: vertex_descs,
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}

fn create_test_material(
    device: &wgpu::Device,
    sc_desc: &wgpu::SwapChainDescriptor,
    layout: &wgpu::BindGroupLayout,
    queue: &wgpu::Queue,
) -> model::Material {
    let texture = texture::Texture::create_depth_texture(device, sc_desc, "test_depth");
    //texture::Texture::create_color_texture(device, sc_desc, "test_depth");
    let (normal_map, cmd) =
        texture::Texture::load(&device, "res/tex/normal_map_static.png", true).unwrap();
    queue.submit(&[cmd]);
    model::Material::new(&device, "test", texture, normal_map, layout)
}
