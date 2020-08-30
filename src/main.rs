mod camera;
mod debug;
mod light;
mod model;
mod pipeline;
mod shader;
mod shadow;
mod texture;
mod uniform;

use cgmath::prelude::*;
use futures::executor::block_on;
use model::Vertex;
use std::{iter, mem};
use wgpu::util::DeviceExt;
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
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    render_pipeline: wgpu::RenderPipeline,
    light_render_pipeline: wgpu::RenderPipeline,
    size: winit::dpi::PhysicalSize<u32>,
    camera: camera::Camera,
    projection: camera::PerspectiveProjection,
    camera_controller: camera::CameraController,
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
    shadow_pass: shadow::ShadowPass,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    save_texture: bool,
    debug_pass: debug::DebugPass,
}

impl State {
    async fn new(window: Window) -> State {
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

        // A BindGroup describes a set of resources and how they can be accessed by a shader.
        // We create a BindGroup using a BindGroupLayout.
        let texture_bind_group_layout = texture::create_bind_group_layout(&device);

        let camera = camera::Camera::new((0.0, 10.0, 20.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection = camera::PerspectiveProjection::new(
            sc_desc.width,
            sc_desc.height,
            cgmath::Deg(45.0),
            0.1,
            100.0,
        );
        let camera_controller = camera::CameraController::new(4.0, 0.8);

        let mut uniforms = uniform::Uniforms::new();
        uniforms.update_view_proj(&camera, &projection);
        let uniform_buffer = uniform::create_buffer(&device, &[uniforms]);

        // TODO move to uniform.rs?
        let globals_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mem::size_of::<uniform::Uniforms>() as _,
                        ),
                    },
                    count: None,
                }],
                label: Some("globals_bind_group_layout"),
            });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &globals_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(uniform_buffer.slice(..)),
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
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instances"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsage::STORAGE,
        });

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

        let instances_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &instances_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(instance_buffer.slice(..)),
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
        let plane_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Plane instances"),
            contents: bytemuck::cast_slice(&plane_instance_data),
            usage: wgpu::BufferUsage::STORAGE,
        });
        let plane_instances_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &instances_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(plane_instance_buffer.slice(..)),
            }],
            label: Some("plane_instances_bind_group"),
        });

        let light = light::Light::new((20.0, 20.0, 0.0), (1.0, 1.0, 1.0));

        // We'll want to update our lights position, so we use COPY_DST
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights"),
            contents: bytemuck::cast_slice(&[light.to_raw()]),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::UniformBuffer {
                            dynamic: false,
                            min_binding_size: wgpu::BufferSize::new(
                                mem::size_of::<light::LightRaw>() as _,
                            ),
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
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                ],
                label: None,
            });

        let mut shader_compiler = shaderc::Compiler::new().unwrap();
        let vertex_descs = [model::ModelVertex::desc()];

        let render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render pipeline"),
                push_constant_ranges: &[],
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &globals_bind_group_layout,
                    &instances_bind_group_layout,
                    &light_bind_group_layout,
                ],
            });

            let vs_module = compile_vertex!(&device, &mut shader_compiler, "shader.vert").unwrap();
            let fs_module = compile_frag!(&device, &mut shader_compiler, "shader.frag").unwrap();

            pipeline::create(
                &"forward",
                &device,
                &layout,
                &vs_module,
                &fs_module,
                Some(sc_desc.format),
                Some(pipeline::DepthConfig::no_bias()),
                &vertex_descs,
            )
        };

        let shadow_pass = shadow::ShadowPass::new(
            &device,
            &mut shader_compiler,
            &instances_bind_group_layout,
            &vertex_descs,
        );

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(light_buffer.slice(..)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_pass.target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_pass.sampler),
                },
            ],
            label: None,
        });

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light render pipeline"),
                push_constant_ranges: &[],
                bind_group_layouts: &[&globals_bind_group_layout, &light_bind_group_layout],
            });

            let vs_module = compile_vertex!(&device, &mut shader_compiler, "light.vert").unwrap();
            let fs_module = compile_frag!(&device, &mut shader_compiler, "light.frag").unwrap();

            pipeline::create(
                &"light",
                &device,
                &layout,
                &vs_module,
                &fs_module,
                Some(sc_desc.format),
                Some(pipeline::DepthConfig::no_bias()),
                &vertex_descs,
            )
        };

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &sc_desc, "depth_texture");

        let (obj_model, cmds) =
            model::Model::load(&device, &texture_bind_group_layout, "res/models/scene.obj")
                .unwrap();
        queue.submit(cmds);

        let (light_model, cmds) =
            model::Model::load(&device, &texture_bind_group_layout, "res/models/cube.obj").unwrap();
        queue.submit(cmds);

        let (plane_model, cmds) =
            model::Model::load(&device, &texture_bind_group_layout, "res/models/plane.obj")
                .unwrap();
        queue.submit(cmds);

        let debug_pass = debug::DebugPass::new(
            &device,
            &shadow_pass.target_view,
            &shadow_pass.sampler,
            &globals_bind_group_layout,
        );

        State {
            window,
            surface,
            device,
            queue,
            sc_desc,
            swap_chain,
            render_pipeline,
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
            save_texture: false,
            debug_pass,
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
                /*
                VirtualKeyCode::S => {
                    if *state == ElementState::Pressed {
                        self.save_texture = true;
                        true
                    } else {
                        false
                    }
                }
                */
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

        let staging_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Staging"),
                contents: bytemuck::cast_slice(&[self.uniforms]),
                usage: wgpu::BufferUsage::COPY_SRC,
            });

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
        let staging_buffer = self
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

        self.shadow_pass.update_lights(&self.queue, &[&self.light]);

        // We need to remember to submit our CommandEncoder's output
        // otherwise we won't see any change.
        self.queue.submit(iter::once(encoder.finish()));
    }

    fn render(&mut self) {
        // Get a frame to render to
        let frame = self
            .swap_chain
            .get_current_frame()
            .expect("Timeout getting texture");

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.render_with_encoder(&mut encoder, &frame.output);
        self.queue.submit(iter::once(encoder.finish()));

        if self.save_texture {
            debug::save_texture(&self.device, 1024, 1024, &self.shadow_pass.texture);
            self.save_texture = false;
        }
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

        {
            // shadow pass
            let mut pass = self.shadow_pass.begin(encoder);
            for mesh in &self.obj_model.meshes {
                pass.render(shadow::ShadowPassRenderData::from_mesh(
                    &mesh,
                    &self.instances_bind_group,
                ));
            }
        }

        {
            // forward pass
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                // where we're going to draw our color to
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(back_color),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
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

        self.debug_pass
            .render(encoder, &frame, &self.globals_bind_group);
    }
}
