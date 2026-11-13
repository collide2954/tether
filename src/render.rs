use bytemuck::{Pod, Zeroable};
use tether_core::color::Palette;
use tether_core::mesh::{Mesh, Vertex};
use tether_core::view::View;

pub const WARPS: usize = 8;

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

pub struct Renderer {
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    ground: wgpu::RenderPipeline,
    shapes: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tether"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let ground_module = device.create_shader_module(wgpu::include_wgsl!("shaders/ground.wgsl"));
        let shapes_module = device.create_shader_module(wgpu::include_wgsl!("shaders/shapes.wgsl"));
        let ground = pipeline(device, &pipeline_layout, &ground_module, &[], format, None);
        let shapes = pipeline(
            device,
            &pipeline_layout,
            &shapes_module,
            &[Some(VERTEX_LAYOUT)],
            format,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        Self {
            globals,
            bind_group,
            ground,
            shapes,
            vertices: buffer(device, "vertices", wgpu::BufferUsages::VERTEX, 1 << 16),
            indices: buffer(device, "indices", wgpu::BufferUsages::INDEX, 1 << 16),
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        globals: &Globals,
        mesh: &Mesh,
    ) {
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
        let g = globals.ground;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: g[0] as f64,
                        g: g[1] as f64,
                        b: g[2] as f64,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
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
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
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
