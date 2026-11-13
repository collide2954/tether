use bytemuck::{Pod, Zeroable};
use tether_core::color::Palette;
use tether_core::mesh::{Mesh, Vertex};
use tether_core::view::View;

pub const WARPS: usize = 8;
pub const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const BLOOM_LEVELS: usize = 5;

pub fn bloom_sizes(size: (u32, u32)) -> [(u32, u32); BLOOM_LEVELS] {
    let mut out = [(1, 1); BLOOM_LEVELS];
    let (mut w, mut h) = size;
    for slot in &mut out {
        w = w.div_ceil(2).max(1);
        h = h.div_ceil(2).max(1);
        *slot = (w, h);
    }
    out
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub inverse: [[f32; 4]; 4],
    pub eye: [f32; 4],
    pub ground: [f32; 4],
    pub lattice: [f32; 4],
    pub viewport: [f32; 4],
    pub warps: [[f32; 4]; WARPS],
}

impl Globals {
    pub fn new(view: &View, palette: &Palette, time: f32, warps: &[[f32; 4]]) -> Self {
        let view_proj = view.view_proj();
        let count = warps.len().min(WARPS);
        let mut list = [[0.0; 4]; WARPS];
        list[..count].copy_from_slice(&warps[..count]);
        let (g, l) = (palette.ground, palette.lattice);
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            inverse: view_proj.inverse().to_cols_array_2d(),
            eye: view.eye().extend(time).to_array(),
            ground: [g.r, g.g, g.b, 1.0],
            lattice: [l.r, l.g, l.b, 1.0],
            viewport: [view.viewport.x, view.viewport.y, count as f32, 0.0],
            warps: list,
        }
    }
}

const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4];

const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &VERTEX_ATTRIBUTES,
};

struct Targets {
    size: (u32, u32),
    msaa: Option<wgpu::TextureView>,
    hdr: wgpu::TextureView,
    bloom: Vec<wgpu::TextureView>,
    down: Vec<wgpu::BindGroup>,
    up: Vec<wgpu::BindGroup>,
    composite: wgpu::BindGroup,
}

pub struct Renderer {
    samples: u32,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    ground: wgpu::RenderPipeline,
    shapes: wgpu::RenderPipeline,
    prefilter: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    targets: Option<Targets>,
}

struct Target {
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    samples: u32,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, output: wgpu::TextureFormat, samples: u32) -> Self {
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = layout(device, &[uniform_entry(0)]);
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let texture_layout = layout(device, &[texture_entry(0), sampler_entry(1)]);
        let composite_layout = layout(
            device,
            &[texture_entry(0), texture_entry(1), sampler_entry(2)],
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let scene_layout = pipeline_layout(device, &[&globals_layout]);
        let bloom_layout = pipeline_layout(device, &[&texture_layout]);
        let final_layout = pipeline_layout(device, &[&globals_layout, &composite_layout]);
        let ground_module = device.create_shader_module(wgpu::include_wgsl!("shaders/ground.wgsl"));
        let shapes_module = device.create_shader_module(wgpu::include_wgsl!("shaders/shapes.wgsl"));
        let bloom_module = device.create_shader_module(wgpu::include_wgsl!("shaders/bloom.wgsl"));
        let composite_module =
            device.create_shader_module(wgpu::include_wgsl!("shaders/composite.wgsl"));
        let hdr = |blend, samples| Target {
            format: HDR,
            blend,
            samples,
        };
        Self {
            samples,
            ground: pipeline(
                device,
                &scene_layout,
                &ground_module,
                "fs_main",
                &[],
                hdr(None, samples),
            ),
            shapes: pipeline(
                device,
                &scene_layout,
                &shapes_module,
                "fs_main",
                &[Some(VERTEX_LAYOUT)],
                hdr(
                    Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    samples,
                ),
            ),
            prefilter: pipeline(
                device,
                &bloom_layout,
                &bloom_module,
                "fs_prefilter",
                &[],
                hdr(None, 1),
            ),
            down: pipeline(
                device,
                &bloom_layout,
                &bloom_module,
                "fs_down",
                &[],
                hdr(None, 1),
            ),
            up: pipeline(
                device,
                &bloom_layout,
                &bloom_module,
                "fs_up",
                &[],
                hdr(Some(wgpu::BlendState::ADDITIVE), 1),
            ),
            composite: pipeline(
                device,
                &final_layout,
                &composite_module,
                "fs_main",
                &[],
                Target {
                    format: output,
                    blend: None,
                    samples: 1,
                },
            ),
            globals,
            globals_group,
            texture_layout,
            composite_layout,
            sampler,
            vertices: buffer(device, "vertices", wgpu::BufferUsages::VERTEX, 1 << 16),
            indices: buffer(device, "indices", wgpu::BufferUsages::INDEX, 1 << 16),
            targets: None,
        }
    }

    fn make_targets(&self, device: &wgpu::Device, size: (u32, u32)) -> Targets {
        let texture = |label: &str, (width, height): (u32, u32), samples: u32, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let sampled = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let msaa = (self.samples > 1).then(|| {
            texture(
                "msaa",
                size,
                self.samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        let hdr = texture("hdr", size, 1, sampled);
        let bloom: Vec<wgpu::TextureView> = bloom_sizes(size)
            .iter()
            .map(|&s| texture("bloom", s, 1, sampled))
            .collect();
        let group = |source: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let down = (0..BLOOM_LEVELS)
            .map(|k| group(if k == 0 { &hdr } else { &bloom[k - 1] }))
            .collect();
        let up = (0..BLOOM_LEVELS - 1)
            .map(|k| group(&bloom[k + 1]))
            .collect();
        let composite = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite"),
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&hdr),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&bloom[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Targets {
            size,
            msaa,
            hdr,
            bloom,
            down,
            up,
            composite,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        size: (u32, u32),
        globals: &Globals,
        mesh: &Mesh,
    ) {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = Some(self.make_targets(device, size));
        }
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(globals));
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&mesh.indices);
        grow(
            device,
            &mut self.vertices,
            "vertices",
            wgpu::BufferUsages::VERTEX,
            vertex_bytes.len(),
        );
        grow(
            device,
            &mut self.indices,
            "indices",
            wgpu::BufferUsages::INDEX,
            index_bytes.len(),
        );
        if !index_bytes.is_empty() {
            queue.write_buffer(&self.vertices, 0, vertex_bytes);
            queue.write_buffer(&self.indices, 0, index_bytes);
        }
        let Some(targets) = self.targets.as_ref() else {
            return;
        };
        let g = globals.ground;
        let ground = wgpu::Color {
            r: f64::from(g[0]),
            g: f64::from(g[1]),
            b: f64::from(g[2]),
            a: 1.0,
        };
        {
            let (view, resolve) = match &targets.msaa {
                Some(msaa) => (msaa, Some(&targets.hdr)),
                None => (&targets.hdr, None),
            };
            let store = if resolve.is_some() {
                wgpu::StoreOp::Discard
            } else {
                wgpu::StoreOp::Store
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(ground),
                        store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.globals_group, &[]);
            pass.set_pipeline(&self.ground);
            pass.draw(0..3, 0..1);
            if !index_bytes.is_empty() {
                pass.set_pipeline(&self.shapes);
                pass.set_vertex_buffer(0, self.vertices.slice(..vertex_bytes.len() as u64));
                pass.set_index_buffer(
                    self.indices.slice(..index_bytes.len() as u64),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
            }
        }
        for k in 0..BLOOM_LEVELS {
            let pipeline = if k == 0 { &self.prefilter } else { &self.down };
            let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
            fullscreen(
                encoder,
                &targets.bloom[k],
                clear,
                pipeline,
                &[&targets.down[k]],
            );
        }
        for k in (0..BLOOM_LEVELS - 1).rev() {
            fullscreen(
                encoder,
                &targets.bloom[k],
                wgpu::LoadOp::Load,
                &self.up,
                &[&targets.up[k]],
            );
        }
        let groups = [&self.globals_group, &targets.composite];
        fullscreen(
            encoder,
            output,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            &self.composite,
            &groups,
        );
    }
}

fn fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    pipeline: &wgpu::RenderPipeline,
    groups: &[&wgpu::BindGroup],
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("fullscreen"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    for (index, group) in groups.iter().enumerate() {
        pass.set_bind_group(index as u32, *group, &[]);
    }
    pass.draw(0..3, 0..1);
}

fn layout(device: &wgpu::Device, entries: &[wgpu::BindGroupLayoutEntry]) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries,
    })
}

fn pipeline_layout(
    device: &wgpu::Device,
    groups: &[&wgpu::BindGroupLayout],
) -> wgpu::PipelineLayout {
    let groups: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|g| Some(*g)).collect();
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &groups,
        immediate_size: 0,
    })
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    fragment: &str,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    target: Target,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: target.samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target.format,
                blend: target.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn buffer(
    device: &wgpu::Device,
    label: &str,
    usage: wgpu::BufferUsages,
    size: u64,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn grow(
    device: &wgpu::Device,
    target: &mut wgpu::Buffer,
    label: &str,
    usage: wgpu::BufferUsages,
    needed: usize,
) {
    let needed = needed as u64;
    if needed > target.size() {
        *target = buffer(device, label, usage, needed.next_power_of_two());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;
    use tether_core::math::{Mat4, Vec2};

    fn parse(source: &str) -> naga::Module {
        let module = naga::front::wgsl::parse_str(source).expect("WGSL parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("WGSL validation");
        module
    }

    fn globals_offsets(module: &naga::Module) -> (u32, Vec<u32>) {
        let ty = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Globals"))
            .map(|(_, t)| t)
            .expect("Globals type");
        match &ty.inner {
            naga::TypeInner::Struct { members, span } => {
                (*span, members.iter().map(|m| m.offset).collect())
            }
            _ => panic!("Globals is not a struct"),
        }
    }

    fn rust_offsets() -> (u32, Vec<u32>) {
        let offsets = [
            offset_of!(Globals, view_proj),
            offset_of!(Globals, inverse),
            offset_of!(Globals, eye),
            offset_of!(Globals, ground),
            offset_of!(Globals, lattice),
            offset_of!(Globals, viewport),
            offset_of!(Globals, warps),
        ];
        (
            size_of::<Globals>() as u32,
            offsets.iter().map(|&o| o as u32).collect(),
        )
    }

    #[test]
    fn ground_shader_is_valid_and_matches_globals() {
        let module = parse(include_str!("shaders/ground.wgsl"));
        assert_eq!(globals_offsets(&module), rust_offsets());
    }

    #[test]
    fn shapes_shader_is_valid_and_matches_globals() {
        let module = parse(include_str!("shaders/shapes.wgsl"));
        assert_eq!(globals_offsets(&module), rust_offsets());
    }

    #[test]
    fn bloom_shader_is_valid_with_three_passes() {
        let module = parse(include_str!("shaders/bloom.wgsl"));
        let names: Vec<&str> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        for wanted in ["vs_main", "fs_prefilter", "fs_down", "fs_up"] {
            assert!(names.contains(&wanted), "{wanted}");
        }
    }

    #[test]
    fn composite_shader_is_valid_and_matches_globals() {
        let module = parse(include_str!("shaders/composite.wgsl"));
        assert_eq!(globals_offsets(&module), rust_offsets());
    }

    #[test]
    fn bloom_sizes_halve_down_to_one() {
        assert_eq!(
            bloom_sizes((1920, 1080)),
            [(960, 540), (480, 270), (240, 135), (120, 68), (60, 34)]
        );
        assert_eq!(
            bloom_sizes((3, 1)),
            [(2, 1), (1, 1), (1, 1), (1, 1), (1, 1)]
        );
    }

    #[test]
    fn vertex_layout_covers_the_vertex() {
        assert_eq!(VERTEX_LAYOUT.array_stride, size_of::<Vertex>() as u64);
        let last = VERTEX_ATTRIBUTES[3];
        assert_eq!(last.offset + last.format.size(), size_of::<Vertex>() as u64);
    }

    #[test]
    fn globals_carry_the_view_and_palette() {
        let mut view = View::new(Vec2::new(1920.0, 1080.0));
        view.target = Vec2::new(1.0, -2.0);
        let palette = Palette::video();
        let g = Globals::new(&view, &palette, 2.5, &[]);
        assert_eq!(g.view_proj, view.view_proj().to_cols_array_2d());
        let product = Mat4::from_cols_array_2d(&g.inverse) * view.view_proj();
        assert!(product.abs_diff_eq(Mat4::IDENTITY, 1e-3));
        assert_eq!(g.eye, view.eye().extend(2.5).to_array());
        let (gr, la) = (palette.ground, palette.lattice);
        assert_eq!(g.ground, [gr.r, gr.g, gr.b, 1.0]);
        assert_eq!(g.lattice, [la.r, la.g, la.b, 1.0]);
        assert_eq!(g.viewport, [1920.0, 1080.0, 0.0, 0.0]);
    }

    #[test]
    fn globals_keep_at_most_eight_warps() {
        let warps = [[1.0, 2.0, 0.3, 0.5]; 10];
        let g = Globals::new(&View::new(Vec2::ONE), &Palette::video(), 0.0, &warps);
        assert_eq!(g.viewport[2], WARPS as f32);
        assert_eq!(g.warps[7], [1.0, 2.0, 0.3, 0.5]);
    }
}
