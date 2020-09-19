use crate::camera;
use crate::geometry;
use crate::geometry::Vertex;
use crate::model;
use crate::pipeline;
use crate::Kanvas;
use crate::{compile_frag, compile_vertex};
use cgmath::prelude::*;
use wgpu::util::DeviceExt;

pub struct Billboard {
    instance_buffer: wgpu::Buffer,
    pub instance_bind_group: wgpu::BindGroup,
    position: cgmath::Point3<f32>,
    material: model::Material,
    plane: geometry::Plane,
}

impl Billboard {
    pub fn new(
        kanvas: &Kanvas,
        material: model::Material,
        instances_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
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
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            });

        let instance_bind_group = kanvas.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &instances_bind_group_layout,
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

        Billboard {
            instance_buffer,
            instance_bind_group,
            position: (0.0, 10.0, 0.0).into(),
            material,
            plane: geometry::Plane::new(&kanvas.device),
        }
    }

    pub fn update(&mut self, kanvas: &Kanvas, camera: &camera::Camera) {
        // TODO don't recalculate view matrix
        let view_mat = camera.calc_matrix();

        // From: https://swiftcoder.wordpress.com/2008/11/25/constructing-a-billboard-matrix/
        // Transpose the 3x3 rotation matrix (cancels out view matrix rotation)
        let billboard_transform = cgmath::Matrix4::new(
            view_mat.x.x,
            view_mat.y.x,
            view_mat.z.x,
            0.0,
            view_mat.x.y,
            view_mat.y.y,
            view_mat.z.y,
            0.0,
            view_mat.x.z,
            view_mat.y.z,
            view_mat.z.z,
            0.0,
            self.position.x,
            self.position.y,
            self.position.z,
            1.0,
        );
        let instance = model::InstanceRaw {
            model: billboard_transform,
        };
        kanvas
            .queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::bytes_of(&instance));
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        uniforms_bind_group: &'a wgpu::BindGroup,
    ) {
        self.plane.render(
            render_pass,
            &self.material,
            uniforms_bind_group,
            &self.instance_bind_group,
        );
    }
}

pub fn create_pipeline(
    kanvas: &mut Kanvas,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    uniform_bind_group_layout: &wgpu::BindGroupLayout,
    instances_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let layout = kanvas
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render pipeline"),
            push_constant_ranges: &[],
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &uniform_bind_group_layout,
                &instances_bind_group_layout,
            ],
        });

    let vs_module = compile_vertex!(
        &kanvas.device,
        &mut kanvas.shader_compiler,
        "billboard.vert"
    )
    .unwrap();
    let fs_module = compile_frag!(
        &kanvas.device,
        &mut kanvas.shader_compiler,
        "billboard.frag"
    )
    .unwrap();

    pipeline::create(
        &"forward",
        &kanvas.device,
        &layout,
        &vs_module,
        &fs_module,
        Some(kanvas.sc_desc.format),
        Some(pipeline::DepthConfig::no_bias()),
        &[geometry::SimpleVertex::desc()],
    )
}
