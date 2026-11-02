//! Draws a [`World`] into a [`Target`]: shadow map, opaque voxels, petals, sky, then the
//! transparent layers, and finally the post pass onto a window. The same order as src/render.rs.
//!
//! Unlike Direct3D 11, wgpu cannot change a constant buffer between two draws of one pass, so
//! every draw of a frame gets its own slot in one buffer and picks it with a dynamic offset.

use crate::gpu::{bytes, Gpu, ShadowMap, Target, COLOR, DEPTH, SHADOW};
use crate::scene::{Mesh, Scene, VERTEX_SIZE};
use crate::world::{BoardGpu, Draw, FrameCB, ObjectCB, PetalGpu, ViewCB, World};
use wgpu::util::DeviceExt;

const MAX_BOARDS: u32 = 1024;

pub struct Renderer {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    frame_buf: wgpu::Buffer,
    view_buf: wgpu::Buffer,
    object_buf: wgpu::Buffer,
    object_stride: u32,
    object_data: Vec<u8>,
    petal_buf: wgpu::Buffer,
    board_buf: wgpu::Buffer,
    uniforms: wgpu::BindGroup,
    resources_layout: wgpu::BindGroupLayout,
    resources: Option<wgpu::BindGroup>,
    smp_shadow: wgpu::Sampler,
    shadow: Option<ShadowMap>,
    shadow_size: u32,
    p_shadow: wgpu::RenderPipeline,
    p_voxel: wgpu::RenderPipeline,
    p_petal: wgpu::RenderPipeline,
    p_sky: wgpu::RenderPipeline,
    p_water: wgpu::RenderPipeline,
    p_fall: wgpu::RenderPipeline,
    p_boards: [wgpu::RenderPipeline; 3],
    water_mesh: Mesh,
    fall_mesh: Mesh,
}

fn uniform_entry(binding: u32, dynamic: bool, size: usize) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: wgpu::BufferSize::new(size as u64) },
        count: None,
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn dynamic_storage(gpu: &Gpu, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
}

fn dynamic_uniform(gpu: &Gpu, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
}

/// Scene alpha blending, as in `Gpu::blend` of the Direct3D version.
fn blend(dst: wgpu::BlendFactor) -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: dst, operation: wgpu::BlendOperation::Add },
        alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
    }
}

impl Renderer {
    pub fn new(gpu: &Gpu, scene: &Scene, shadow_size: u32) -> Renderer {
        let dev = &gpu.device;
        let m = &scene.manifest;
        let shader = dev.create_shader_module(wgpu::include_wgsl!("shaders.wgsl"));

        let uniforms_layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[uniform_entry(0, false, size_of::<FrameCB>()), uniform_entry(1, false, size_of::<ViewCB>()), uniform_entry(2, true, size_of::<ObjectCB>())],
        });
        let resources_layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
                storage_entry(2),
                storage_entry(3),
            ],
        });
        // the shadow pass must not bind the texture it renders into
        let shadow_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&uniforms_layout)], immediate_size: 0 });
        let main_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&uniforms_layout), Some(&resources_layout)], immediate_size: 0 });

        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Snorm8x4, 2 => Unorm8x4];
        let voxel_buffers = [Some(wgpu::VertexBufferLayout { array_stride: VERTEX_SIZE as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })];

        struct P<'a> {
            vs: &'a str,
            fs: Option<&'a str>,
            voxels: bool,
            cull: Option<wgpu::Face>,
            write_depth: bool,
            blend: Option<wgpu::BlendState>,
            shadow_pass: bool,
        }
        let pipeline = |p: P| {
            let targets = [Some(wgpu::ColorTargetState { format: COLOR, blend: p.blend, write_mask: wgpu::ColorWrites::ALL })];
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(p.fs.unwrap_or(p.vs)),
                layout: Some(if p.shadow_pass { &shadow_layout } else { &main_layout }),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(p.vs), compilation_options: Default::default(), buffers: if p.voxels { &voxel_buffers } else { &[] } },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // scenes use the OpenGL / three.js convention: counter-clockwise faces are front faces
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: p.cull,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: if p.shadow_pass { SHADOW } else { DEPTH },
                    depth_write_enabled: Some(p.write_depth),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: p.fs.map(|f| wgpu::FragmentState { module: &shader, entry_point: Some(f), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            })
        };
        let alpha = blend(wgpu::BlendFactor::OneMinusSrcAlpha);
        let add = blend(wgpu::BlendFactor::One);
        let opaque = |vs, fs, voxels, cull| P { vs, fs: Some(fs), voxels, cull, write_depth: true, blend: None, shadow_pass: false };
        let layer = |vs, fs, voxels, blend| P { vs, fs: Some(fs), voxels, cull: None, write_depth: false, blend, shadow_pass: false };

        let align = dev.limits().min_uniform_buffer_offset_alignment;
        let object_stride = (size_of::<ObjectCB>() as u32).next_multiple_of(align);
        // all objects, the surfaces and one first-billboard slot per billboard pass
        let slots = m.objects.len() + m.swimmers.len() + 4;
        let frame_buf = dynamic_uniform(gpu, size_of::<FrameCB>() as u64);
        let view_buf = dynamic_uniform(gpu, size_of::<ViewCB>() as u64);
        let object_buf = dynamic_uniform(gpu, slots as u64 * object_stride as u64);
        let uniforms = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &uniforms_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: view_buf.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &object_buf, offset: 0, size: wgpu::BufferSize::new(size_of::<ObjectCB>() as u64) }),
                },
            ],
        });
        let vertex_data = |contents, usage| dev.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents, usage });
        Renderer {
            vb: vertex_data(scene.vertices, wgpu::BufferUsages::VERTEX),
            ib: vertex_data(scene.indices, wgpu::BufferUsages::INDEX),
            p_shadow: pipeline(P { vs: "vs_shadow", fs: None, voxels: true, cull: Some(wgpu::Face::Front), write_depth: true, blend: None, shadow_pass: true }),
            p_voxel: pipeline(opaque("vs_voxel", "fs_voxel", true, Some(wgpu::Face::Back))),
            p_petal: pipeline(opaque("vs_petal", "fs_petal", false, None)),
            p_sky: pipeline(layer("vs_full", "fs_sky", false, None)),
            p_water: pipeline(layer("vs_surface", "fs_water", true, Some(alpha))),
            p_fall: pipeline(layer("vs_surface", "fs_fall", true, Some(alpha))),
            p_boards: [
                pipeline(layer("vs_board", "fs_point", false, Some(alpha))),
                pipeline(layer("vs_board", "fs_point", false, Some(add))),
                pipeline(layer("vs_board", "fs_glow", false, Some(add))),
            ],
            frame_buf,
            view_buf,
            object_buf,
            object_stride,
            object_data: vec![0; slots * object_stride as usize],
            petal_buf: dynamic_storage(gpu, m.petals.max.max(1) as u64 * size_of::<PetalGpu>() as u64),
            board_buf: dynamic_storage(gpu, MAX_BOARDS as u64 * size_of::<BoardGpu>() as u64),
            uniforms,
            resources_layout,
            resources: None,
            smp_shadow: dev.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }),
            shadow: None,
            shadow_size,
            water_mesh: m.meshes[m.water.mesh],
            fall_mesh: m.meshes[m.waterfall.mesh],
        }
    }

    /// Frees the shadow map (it is rebuilt on the next frame).
    pub fn release(&mut self) {
        self.shadow = None;
        self.resources = None;
    }

    fn object(&mut self, slot: usize, d: &Draw, fog: bool, first_board: u32) {
        let cb = ObjectCB {
            world: d.world.to_cols_array_2d(),
            emissive: [d.emissive.x, d.emissive.y, d.emissive.z, if d.receive { 1.0 } else { 0.0 }],
            flags: [if fog { 1.0 } else { 0.0 }, first_board as f32, 0.0, 0.0],
        };
        let at = slot * self.object_stride as usize;
        self.object_data[at..at + size_of::<ObjectCB>()].copy_from_slice(bytes(&[cb]));
    }

    fn offset(&self, slot: usize) -> u32 {
        slot as u32 * self.object_stride
    }

    /// Uploads per-frame data and refreshes the shadow map if the light moved.
    pub fn prepare(&mut self, gpu: &Gpu, world: &mut World) {
        let q = &gpu.queue;
        q.write_buffer(&self.frame_buf, 0, bytes(&[world.frame]));
        if !world.petal_gpu.is_empty() {
            q.write_buffer(&self.petal_buf, 0, bytes(&world.petal_gpu));
        }
        let n = world.boards.len().min(MAX_BOARDS as usize);
        if n > 0 {
            q.write_buffer(&self.board_buf, 0, bytes(&world.boards[..n]));
        }
        for (i, d) in world.draws.iter().enumerate() {
            self.object(i, d, true, 0);
        }
        let ident = Draw { mesh: self.water_mesh, world: glam::Mat4::IDENTITY, emissive: glam::Vec3::ZERO, receive: false, cast: false };
        let base = world.draws.len();
        self.object(base, &ident, false, 0);
        for (i, (start, _)) in world.board_ranges.into_iter().enumerate() {
            self.object(base + 1 + i, &ident, false, start);
        }
        q.write_buffer(&self.object_buf, 0, &self.object_data);

        if self.shadow.is_none() {
            let sm = ShadowMap::new(gpu, self.shadow_size);
            self.resources = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.resources_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&sm.view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.smp_shadow) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.petal_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.board_buf.as_entire_binding() },
                ],
            }));
            self.shadow = Some(sm);
            world.shadow_dirty = true;
        }
        if !world.shadow_dirty {
            return;
        }
        world.shadow_dirty = false;
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow.as_ref().unwrap().view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.p_shadow);
            pass.set_vertex_buffer(0, self.vb.slice(..));
            pass.set_index_buffer(self.ib.slice(..), wgpu::IndexFormat::Uint32);
            for (i, d) in world.draws.iter().enumerate().filter(|(_, d)| d.cast) {
                pass.set_bind_group(0, &self.uniforms, &[self.offset(i)]);
                pass.draw_indexed(d.mesh.first_index..d.mesh.first_index + d.mesh.index_count, d.mesh.first_vertex as i32, 0..1);
            }
        }
        q.submit([enc.finish()]);
    }

    pub fn render(&self, gpu: &Gpu, world: &World, view: &ViewCB, t: &Target) {
        gpu.queue.write_buffer(&self.view_buf, 0, bytes(&[*view]));
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(1, self.resources.as_ref().unwrap(), &[]);
            pass.set_vertex_buffer(0, self.vb.slice(..));
            pass.set_index_buffer(self.ib.slice(..), wgpu::IndexFormat::Uint32);

            // opaque voxel objects
            pass.set_pipeline(&self.p_voxel);
            for (i, d) in world.draws.iter().enumerate() {
                pass.set_bind_group(0, &self.uniforms, &[self.offset(i)]);
                pass.draw_indexed(d.mesh.first_index..d.mesh.first_index + d.mesh.index_count, d.mesh.first_vertex as i32, 0..1);
            }

            let base = world.draws.len();
            pass.set_bind_group(0, &self.uniforms, &[self.offset(base)]);

            // petals
            let np = world.petal_gpu.len() as u32;
            if np > 0 {
                pass.set_pipeline(&self.p_petal);
                pass.draw(0..6, 0..np);
            }

            // sky last among the opaques: only fills pixels nothing else covered
            pass.set_pipeline(&self.p_sky);
            pass.draw(0..3, 0..1);

            // pond, waterfall
            for (pipeline, mesh) in [(&self.p_water, &self.water_mesh), (&self.p_fall, &self.fall_mesh)] {
                pass.set_pipeline(pipeline);
                pass.draw_indexed(mesh.first_index..mesh.first_index + mesh.index_count, mesh.first_vertex as i32, 0..1);
            }

            // spray (alpha), fireflies and glow halos (additive)
            for (i, pipeline) in self.p_boards.iter().enumerate() {
                let (start, count) = world.board_ranges[i];
                let count = count.min(MAX_BOARDS.saturating_sub(start));
                if count == 0 {
                    continue;
                }
                pass.set_bind_group(0, &self.uniforms, &[self.offset(base + 1 + i)]);
                pass.set_pipeline(pipeline);
                pass.draw(0..6, 0..count);
            }
        }
        gpu.queue.submit([enc.finish()]);
    }
}

// ---------------------------------------------------------------- post pass

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PostCB {
    info: [f32; 4],
    uv: [f32; 4],
}

/// The last step of every frame: puts a rendered scene view onto a window (FXAA and vignette).
pub struct Post {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    smp: wgpu::Sampler,
    cb: wgpu::Buffer,
}

impl Post {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Post {
        let dev = &gpu.device;
        let shader = dev.create_shader_module(wgpu::include_wgsl!("post.wgsl"));
        let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                uniform_entry(0, false, size_of::<PostCB>()),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_post"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let smp = dev.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let cb = dynamic_uniform(gpu, size_of::<PostCB>() as u64);
        Post { pipeline, layout, smp, cb }
    }

    pub fn draw(&self, gpu: &Gpu, src: &Target, dst: &wgpu::TextureView, fxaa: bool, vignette: f32) {
        let info = [1.0 / src.w as f32, 1.0 / src.h as f32, if fxaa { 1.0 } else { 0.0 }, vignette];
        gpu.queue.write_buffer(&self.cb, 0, bytes(&[PostCB { info, uv: [1.0, 1.0, 0.0, 0.0] }]));
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.cb.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.smp) },
            ],
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([enc.finish()]);
    }
}
