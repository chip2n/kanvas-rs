mod camera;
mod texture;
mod uniform;

use cgmath::prelude::*;
use futures::executor::block_on;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

#[rustfmt::skip]
const VERTICES: &[Vertex] = &[
    Vertex { position: [-0.0868241,  -0.49240386, 0.0], tex_coords: [1.0 - 0.4131759,    1.0 - 0.00759614] },  // A
    Vertex { position: [-0.49513406, -0.06958647, 0.0], tex_coords: [1.0 - 0.0048659444, 1.0 - 0.43041354] },  // B
    Vertex { position: [-0.21918549,  0.44939706, 0.0], tex_coords: [1.0 - 0.28081453,   1.0 - 0.949397057] }, // C
    Vertex { position: [ 0.35966998,  0.3473291,  0.0], tex_coords: [1.0 - 0.85967,      1.0 - 0.84732911] },  // D
    Vertex { position: [ 0.44147372, -0.2347359,  0.0], tex_coords: [1.0 - 0.9414737,    1.0 - 0.2652641] },   // E
];

#[rustfmt::skip]
const INDICES: &[u16] = &[
    0, 1, 4,
    1, 2, 4,
    2, 3, 4,
];

const NUM_INSTANCES_PER_ROW: u32 = 10;
const NUM_INSTANCES: u32 = NUM_INSTANCES_PER_ROW * NUM_INSTANCES_PER_ROW;
const INSTANCE_DISPLACEMENT: cgmath::Vector3<f32> = cgmath::Vector3::new(
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
    0.0,
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
);

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl Vertex {
    /// Defines how vertices are laid out in memory
    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        use std::mem;
        wgpu::VertexBufferDescriptor {
            // how wide a vertex is (shader skips this number of bytes to get to the next one)
            stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,

            // how often shader should move to the next vertex (e.g. for instancing)
            step_mode: wgpu::InputStepMode::Vertex,

            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3,
                },
                wgpu::VertexAttributeDescriptor {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float2,
                },
            ],
        }
    }
}

struct Instance {
    position: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
}

impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct InstanceRaw {
    model: cgmath::Matrix4<f32>,
}

unsafe impl bytemuck::Pod for InstanceRaw {}
unsafe impl bytemuck::Zeroable for InstanceRaw {}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut state = block_on(State::new(&window));

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
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
        Event::RedrawRequested(_) => {
            state.update();
            state.render();
        }
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        _ => {}
    });
}

struct State {
    surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    size: winit::dpi::PhysicalSize<u32>,
    diffuse_texture: texture::Texture,
    diffuse_bind_group: wgpu::BindGroup,
    camera: camera::Camera,
    camera_controller: camera::CameraController,
    uniforms: uniform::Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,
    depth_texture: texture::Texture,
}

impl State {
    async fn new(window: &Window) -> State {
        let size = window.inner_size();
        let surface = wgpu::Surface::create(window);
        let adapter = wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY,
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

        let diffuse_bytes = include_bytes!("happy-tree.png");
        let (diffuse_texture, cmd_buffer) =
            texture::Texture::from_bytes(&device, diffuse_bytes).unwrap();

        queue.submit(&[cmd_buffer]);

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
                ],
                label: Some("texture_bind_group_layout"),
            });

        // A BindGroup is a more specific declaration of the BindGroupLayout.
        // The reason why these are separate is to allow us to swap out BindGroups on the fly,
        // so long as they all share the same BindGroupLayout.
        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let vs_src = include_str!("shader.vert");
        let fs_src = include_str!("shader.frag");

        let mut compiler = shaderc::Compiler::new().unwrap();
        let vs_spirv = compiler
            .compile_into_spirv(
                vs_src,
                shaderc::ShaderKind::Vertex,
                "shader.vert",
                "main",
                None,
            )
            .unwrap();
        let fs_spirv = compiler
            .compile_into_spirv(
                fs_src,
                shaderc::ShaderKind::Fragment,
                "shader.frag",
                "main",
                None,
            )
            .unwrap();

        let vs_data = wgpu::read_spirv(std::io::Cursor::new(vs_spirv.as_binary_u8())).unwrap();
        let fs_data = wgpu::read_spirv(std::io::Cursor::new(fs_spirv.as_binary_u8())).unwrap();

        let vs_module = device.create_shader_module(&vs_data);
        let fs_module = device.create_shader_module(&fs_data);

        let camera = camera::Camera::new(sc_desc.width as f32 / sc_desc.height as f32);
        let camera_controller = camera::CameraController::new(0.2);

        let instances: Vec<Instance> = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let position = cgmath::Vector3 {
                        x: x as f32,
                        y: 0.0,
                        z: z as f32,
                    } - INSTANCE_DISPLACEMENT;

                    let rotation = if position.is_zero() {
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.0),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(
                            position.clone().normalize(),
                            cgmath::Deg(45.0),
                        )
                    };

                    Instance { position, rotation }
                })
            })
            .collect();

        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer_size =
            instance_data.len() * std::mem::size_of::<cgmath::Matrix4<f32>>();
        let instance_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&instance_data),
            wgpu::BufferUsage::STORAGE_READ,
        );

        let mut uniforms = uniform::Uniforms::new();
        uniforms.update_view_proj(&camera);
        let uniform_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&[uniforms]),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                ],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &uniform_buffer,
                        range: 0..std::mem::size_of_val(&uniforms) as wgpu::BufferAddress,
                    },
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &instance_buffer,
                        range: 0..instance_buffer_size as wgpu::BufferAddress,
                    },
                },
            ],
            label: Some("uniform_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &render_pipeline_layout,
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
                format: sc_desc.format,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_read_mask: 0,
                stencil_write_mask: 0,
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[Vertex::desc()],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let vertex_buffer = device
            .create_buffer_with_data(bytemuck::cast_slice(VERTICES), wgpu::BufferUsage::VERTEX);
        let num_vertices = VERTICES.len() as u32;

        let index_buffer =
            device.create_buffer_with_data(bytemuck::cast_slice(INDICES), wgpu::BufferUsage::INDEX);
        let num_indices = INDICES.len() as u32;

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &sc_desc, "depth_texture");

        State {
            surface,
            adapter,
            device,
            queue,
            sc_desc,
            swap_chain,
            render_pipeline,
            vertex_buffer,
            num_vertices,
            index_buffer,
            num_indices,
            size,
            diffuse_texture,
            diffuse_bind_group,
            camera,
            camera_controller,
            uniforms,
            uniform_buffer,
            uniform_bind_group,
            instances,
            instance_buffer,
            depth_texture,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
        self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.sc_desc, "depth_texture");
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        self.camera_controller.process_events(event)
    }

    fn update(&mut self) {
        self.camera_controller.update_camera(&mut self.camera);
        self.uniforms.update_view_proj(&self.camera);

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

        {
            // clear the screen
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
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
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                // where we're going to draw our color to
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
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
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, &self.vertex_buffer, 0, 0);
            render_pass.set_index_buffer(&self.index_buffer, 0, 0);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..NUM_INSTANCES);
        }

        self.queue.submit(&[encoder.finish()]);
    }
}
