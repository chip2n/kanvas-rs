use crate::camera;
use crate::geometry;
use crate::geometry::Vertex;
use crate::model;
use crate::model::MaterialId;
use crate::pipeline;
use crate::prelude::*;
use crate::Kanvas;
use crate::{compile_frag, compile_vertex};
use std::collections::HashMap;

const MAX_BILLBOARDS: u64 = 10000;

pub type BillboardId = usize;

pub struct Billboard {
    pub position: Vector3,
    pub material: MaterialId,
}

struct BillboardData {
    num_instances: u32,
    instance_buffer: wgpu::Buffer,
    instance_bind_group: wgpu::BindGroup,
}

pub struct Billboards {
    next_id: usize,
    billboards: HashMap<BillboardId, Billboard>,
    instances: HashMap<MaterialId, BillboardData>,
    plane: geometry::Plane,
}

impl Billboards {
    pub fn new(kanvas: &Kanvas) -> Self {
        let plane = geometry::Plane::new(&kanvas.device);
        Billboards {
            next_id: 0,
            billboards: HashMap::new(),
            instances: HashMap::new(),
            plane,
        }
    }

    pub fn insert(&mut self, kanvas: &Kanvas, billboard: Billboard) -> BillboardId {
        let data = self.instances.entry(billboard.material).or_insert_with(|| {
            let instance_buffer = kanvas.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("billboard_instances"),
                size: MAX_BILLBOARDS * std::mem::size_of::<model::InstanceRaw>() as u64,
                mapped_at_creation: false,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            });

            let instance_bind_group = kanvas.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &kanvas.instances_bind_group_layout,
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
            BillboardData {
                num_instances: 0,
                instance_buffer,
                instance_bind_group,
            }
        });

        let id = self.next_id;
        self.next_id += 1;

        self.billboards.insert(id, billboard);
        data.num_instances += 1;

        id
    }

    pub fn upload(&self, kanvas: &Kanvas, camera: &camera::Camera) {
        let instance_size = std::mem::size_of::<model::InstanceRaw>();

        // TODO don't recalculate view matrix
        let view_mat = camera.calc_matrix();

        for (id, billboard) in self.billboards.iter() {
            // From: https://swiftcoder.wordpress.com/2008/11/25/constructing-a-billboard-matrix/
            // Transpose the 3x3 rotation matrix (cancels out view matrix rotation)
            let billboard_transform = Matrix4::new(
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
                billboard.position.x,
                billboard.position.y,
                billboard.position.z,
                1.0,
            );
            let instance = model::InstanceRaw {
                model: billboard_transform,
            };

            let buffer = &self.instances[&billboard.material].instance_buffer;
            kanvas.queue.write_buffer(
                buffer,
                (id * instance_size) as u64,
                bytemuck::bytes_of(&instance),
            );
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        materials: &'a crate::Materials,
        uniforms_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_vertex_buffer(0, self.plane.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.plane.index_buffer.slice(..));
        render_pass.set_bind_group(1, &uniforms_bind_group, &[]);

        for (material_id, data) in self.instances.iter() {
            let material = materials.get(*material_id);
            render_pass.set_bind_group(0, &material.bind_group, &[]);
            render_pass.set_bind_group(2, &data.instance_bind_group, &[]);
            render_pass.draw_indexed(
                0..geometry::PLANE_INDICES.len() as u32,
                0,
                0..data.num_instances,
            );
        }
    }
}

pub fn create_pipeline(
    kanvas: &mut Kanvas,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    uniform_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let layout = kanvas
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render pipeline"),
            push_constant_ranges: &[],
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &uniform_bind_group_layout,
                &kanvas.instances_bind_group_layout,
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
