//! The wgpu renderer: GPU meshes, pipelines, the shaded pass, picking and snapshots.

#[cfg(not(target_arch = "wasm32"))]
use crate::RenderTarget;
use crate::edges::{DEFAULT_CREASE_ANGLE, feature_edges};
use crate::grid::{MAX_GRID_VERTICES, grid_lines};
use crate::ids::{FaceTable, IdMap, MeshId};
use crate::readback::{Readback, ReadbackStatus};
use crate::{Camera, DEPTH_FORMAT, RenderError, RenderStyle, Result, Viewport, math};
use kubit_mesh::{Aabb, FaceTag, TriMesh};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

/// Selected faces beyond this many are not highlighted (fixed-size uniform array).
pub const MAX_SELECTED: usize = 16;

const SHADER: &str = include_str!("shaders/scene.wgsl");
const PICK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

/// Uniform block; mirrors `Globals` in `shaders/scene.wgsl` (all fields 16-byte aligned).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    face_color: [f32; 4],
    hover_color: [f32; 4],
    selected_color: [f32; 4],
    edge_color: [f32; 4],
    shading: [f32; 4],
    ids: [u32; 4],
    selected: [[u32; 4]; MAX_SELECTED / 4],
}

/// One vertex of the flat-shaded face buffer (three per triangle, unwelded).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FaceVertex {
    position: [f32; 3],
    normal: [f32; 3],
    /// Mesh-local face index; the shader adds the mesh's id base (instance attribute).
    local_face: u32,
}

const FACE_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32];
const BASE_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![3 => Uint32];
const EDGE_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
const LINE_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

/// One vertex of the grid line buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LineVertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[derive(Debug)]
struct GpuMesh {
    faces: wgpu::Buffer,
    face_vertices: u32,
    edges: wgpu::Buffer,
    edge_vertices: u32,
    /// One `u32`: the first global id of this mesh (per-instance attribute).
    id_base: wgpu::Buffer,
    base: u32,
    table: FaceTable,
    aabb: Option<Aabb>,
}

/// Colour, optional MSAA resolve, and depth attachments for one frame.
#[derive(Clone, Copy, Debug)]
pub struct FrameTargets<'a> {
    /// Colour attachment (multisampled when the renderer's `sample_count > 1`).
    pub color: &'a wgpu::TextureView,
    /// Single-sample resolve target, required when `sample_count > 1`.
    pub resolve: Option<&'a wgpu::TextureView>,
    /// [`DEPTH_FORMAT`] view with the renderer's sample count.
    pub depth: &'a wgpu::TextureView,
}

/// A tightly packed RGBA8 (sRGB) image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width, px.
    pub width: u32,
    /// Height, px.
    pub height: u32,
    /// `width * height * 4` bytes, rows top to bottom.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Pixel at (x, y), or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = self.rgba.get(i..i + 4)?;
        Some([*p.first()?, *p.get(1)?, *p.get(2)?, *p.get(3)?])
    }

    /// PNG encoding (8-bit RGBA, sRGB chunk).
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder
            .write_header()
            .map_err(|e| RenderError::Png(e.to_string()))?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|e| RenderError::Png(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| RenderError::Png(e.to_string()))?;
        Ok(out)
    }
}

/// An in-flight pick; resolve it with [`Renderer::pick_resolve`].
#[derive(Debug)]
pub struct PendingPick {
    /// `None` when the cursor was outside the viewport (resolves to "nothing").
    readback: Option<Readback>,
}

/// Window-less renderer. Owns GPU meshes and pipelines for one colour format.
///
/// `wgpu::Device` and `wgpu::Queue` are reference-counted handles, so the renderer
/// keeps clones of the caller's: no lifetime ties it to the UI's device.
#[derive(Debug)]
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    pick_globals: wgpu::Buffer,
    pick_bind: wgpu::BindGroup,
    face_pipeline: wgpu::RenderPipeline,
    edge_pipeline: wgpu::RenderPipeline,
    grid_pipeline: wgpu::RenderPipeline,
    /// Grid vertices, rewritten every frame (`MAX_GRID_VERTICES` capacity).
    grid_buffer: wgpu::Buffer,
    id_pipeline: wgpu::RenderPipeline,
    pick_color: wgpu::TextureView,
    pick_texture: wgpu::Texture,
    pick_depth: wgpu::TextureView,
    meshes: BTreeMap<MeshId, GpuMesh>,
    ids: IdMap,
    hovered: Option<(MeshId, FaceTag)>,
    selected: Vec<(MeshId, FaceTag)>,
    /// Errors caught by the uncaptured-error handler of a headless device.
    gpu_errors: Option<Arc<Mutex<Vec<String>>>>,
}

fn globals_layout_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Globals>() as u64),
        },
        count: None,
    }
}

/// Runs `f` inside a validation error scope; on native the scope is resolved
/// synchronously so creation errors surface as `RenderError::Gpu`.
fn scoped<T>(device: &wgpu::Device, f: impl FnOnce() -> T) -> Result<T> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let value = f();
    let error = scope.pop();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(e) = pollster::block_on(error) {
        return Err(RenderError::Gpu(e.to_string()));
    }
    // On the web the future resolves on the event loop; errors then reach the
    // device's uncaptured-error handler instead.
    #[cfg(target_arch = "wasm32")]
    drop(error);
    Ok(value)
}

fn to_u32(n: usize, what: &str) -> Result<u32> {
    u32::try_from(n).map_err(|_| RenderError::MeshTooLarge(format!("{n} {what}")))
}

impl Renderer {
    /// Builds pipelines on an existing device (the UI shares eframe's device).
    ///
    /// `color_format` is the format of the targets passed to [`Renderer::render`];
    /// with an sRGB format colours are converted to linear first. `sample_count` is 1
    /// or 4 (MSAA).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Result<Self> {
        if !matches!(sample_count, 1 | 4) {
            return Err(RenderError::SampleCount(sample_count));
        }
        scoped(device, || {
            Self::build(device, queue, color_format, sample_count)
        })
    }

    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kubit-render scene.wgsl"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kubit-render globals"),
            entries: &[globals_layout_entry()],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kubit-render pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let uniform = |label| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Globals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            (buffer, bind)
        };
        let (globals, globals_bind) = uniform("kubit-render globals");
        let (pick_globals, pick_bind) = uniform("kubit-render pick globals");

        let face_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<FaceVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &FACE_ATTRIBUTES,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: 4,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &BASE_ATTRIBUTES,
            }),
        ];
        let edge_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 12,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &EDGE_ATTRIBUTES,
        })];
        let depth = |write: bool, compare, bias| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: wgpu::StencilState::default(),
            bias,
        };
        // Faces are pushed slightly back so coplanar edges win the depth test. WebGPU
        // forbids depth bias on line topologies, so the bias lives on the faces.
        let face_bias = wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 2.0,
            clamp: 0.0,
        };
        let pipeline = |label,
                        buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                        vs,
                        fs,
                        format,
                        topology,
                        depth_state,
                        samples,
                        blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    // No culling: open shells and inside views must still render.
                    cull_mode: None,
                    ..wgpu::PrimitiveState::default()
                },
                depth_stencil: Some(depth_state),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..wgpu::MultisampleState::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let face_pipeline = pipeline(
            "kubit-render faces",
            &face_buffers,
            "vs_face",
            "fs_face",
            color_format,
            wgpu::PrimitiveTopology::TriangleList,
            depth(true, wgpu::CompareFunction::Less, face_bias),
            sample_count,
            None,
        );
        let edge_pipeline = pipeline(
            "kubit-render edges",
            &edge_buffers,
            "vs_edge",
            "fs_edge",
            color_format,
            wgpu::PrimitiveTopology::LineList,
            depth(
                false,
                wgpu::CompareFunction::LessEqual,
                wgpu::DepthBiasState::default(),
            ),
            sample_count,
            None,
        );
        let line_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &LINE_ATTRIBUTES,
        })];
        let grid_pipeline = pipeline(
            "kubit-render grid",
            &line_buffers,
            "vs_line",
            "fs_line",
            color_format,
            wgpu::PrimitiveTopology::LineList,
            depth(
                false,
                wgpu::CompareFunction::LessEqual,
                wgpu::DepthBiasState::default(),
            ),
            sample_count,
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kubit-render grid"),
            size: (MAX_GRID_VERTICES * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Integer targets cannot be multisampled or resolved: the id pass is 1×.
        let id_pipeline = pipeline(
            "kubit-render face ids",
            &face_buffers,
            "vs_face",
            "fs_id",
            PICK_FORMAT,
            wgpu::PrimitiveTopology::TriangleList,
            depth(
                true,
                wgpu::CompareFunction::Less,
                wgpu::DepthBiasState::default(),
            ),
            1,
            None,
        );

        let one_pixel = |label, format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let pick_texture = one_pixel("kubit-render pick ids", PICK_FORMAT);
        let pick_depth = one_pixel("kubit-render pick depth", DEPTH_FORMAT)
            .create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            device: device.clone(),
            queue: queue.clone(),
            color_format,
            sample_count,
            globals,
            globals_bind,
            pick_globals,
            pick_bind,
            face_pipeline,
            edge_pipeline,
            grid_pipeline,
            grid_buffer,
            id_pipeline,
            pick_color: pick_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            pick_texture,
            pick_depth,
            meshes: BTreeMap::new(),
            ids: IdMap::default(),
            hovered: None,
            selected: Vec::new(),
            gpu_errors: None,
        }
    }

    /// Creates its own instance, adapter and device (CLI snapshots, MCP screenshots,
    /// tests). Colour is `Rgba8UnormSrgb` with 4× MSAA. Works on every target; on the
    /// web it must be awaited from the event loop.
    pub async fn headless_async() -> Result<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("kubit-render headless"),
                required_limits: adapter.limits(),
                ..wgpu::DeviceDescriptor::default()
            })
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;
        // wgpu's default handler panics; a headless renderer records errors instead.
        let errors = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&errors);
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            if let Ok(mut list) = sink.lock() {
                list.push(e.to_string());
            }
        }));
        let mut renderer = Self::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb, 4)?;
        renderer.gpu_errors = Some(errors);
        Ok(renderer)
    }

    /// Blocking [`Renderer::headless_async`] (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn headless() -> Result<Self> {
        pollster::block_on(Self::headless_async())
    }

    /// The device this renderer draws with.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The queue this renderer submits to.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Colour format of the targets.
    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }

    /// MSAA sample count of the colour and depth targets.
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Uploads (or replaces) a mesh: f64 mm → f32, flat normals, feature edges.
    pub fn upload(&mut self, id: MeshId, mesh: &TriMesh) -> Result<()> {
        mesh.validate()
            .map_err(|d| RenderError::InvalidMesh(format!("{d:?}")))?;
        let (table, per_triangle) = FaceTable::build(mesh)
            .ok_or_else(|| RenderError::MeshTooLarge("more than 2^32 faces".into()))?;

        let mut vertices = Vec::with_capacity(mesh.triangles.len() * 3);
        for (t, local_face) in per_triangle.iter().enumerate() {
            let Some(corners) = mesh.corners(t) else {
                continue;
            };
            let [a, b, c] = corners;
            let n = math::normalize(math::cross(math::sub(b, a), math::sub(c, a)))
                .unwrap_or([0.0, 0.0, 1.0]);
            let normal = n.map(|v| v as f32);
            vertices.extend(corners.map(|p| FaceVertex {
                position: p.map(|v| v as f32),
                normal,
                local_face: *local_face,
            }));
        }
        // B-rep edges when the producer has them; otherwise edges guessed from the mesh.
        let segments: Vec<[[f64; 3]; 2]> = if mesh.edges.is_empty() {
            feature_edges(mesh, DEFAULT_CREASE_ANGLE)
        } else {
            mesh.edges
                .iter()
                .flat_map(|(_, points)| points.windows(2))
                .filter_map(|w| match w {
                    [a, b] => Some([*a, *b]),
                    _ => None,
                })
                .collect()
        };
        let lines: Vec<[f32; 3]> = segments
            .into_iter()
            .flat_map(|[a, b]| [a.map(|v| v as f32), b.map(|v| v as f32)])
            .collect();
        let face_vertices = to_u32(vertices.len(), "vertices")?;
        let edge_vertices = to_u32(lines.len(), "edge vertices")?;

        let base = self.allocate_ids(id, table.len())?;
        let init = |label, contents: &[u8]| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                })
        };
        let gpu = GpuMesh {
            faces: init("kubit-render faces", bytemuck::cast_slice(&vertices)),
            face_vertices,
            edges: init("kubit-render edges", bytemuck::cast_slice(&lines)),
            edge_vertices,
            id_base: init("kubit-render id base", bytemuck::bytes_of(&base)),
            base,
            table,
            aabb: mesh.aabb(),
        };
        self.meshes.insert(id, gpu);
        Ok(())
    }

    /// Releases the id range of a replaced mesh and allocates a new one, compacting
    /// the id space if it ran out.
    fn allocate_ids(&mut self, id: MeshId, count: u32) -> Result<u32> {
        if let Some(old) = self.meshes.remove(&id) {
            self.ids.release(old.base);
        }
        if let Some(base) = self.ids.allocate(id, count) {
            return Ok(base);
        }
        // Re-pack every live mesh from 1; only the 4-byte base buffers change.
        self.ids.compact();
        for (&mesh_id, mesh) in &mut self.meshes {
            let base = self
                .ids
                .allocate(mesh_id, mesh.table.len())
                .ok_or(RenderError::IdSpaceExhausted)?;
            mesh.base = base;
            self.queue
                .write_buffer(&mesh.id_base, 0, bytemuck::bytes_of(&base));
        }
        self.ids
            .allocate(id, count)
            .ok_or(RenderError::IdSpaceExhausted)
    }

    /// Removes a mesh; unknown ids are ignored.
    pub fn remove(&mut self, id: MeshId) {
        if let Some(old) = self.meshes.remove(&id) {
            self.ids.release(old.base);
        }
        self.hovered = self.hovered.filter(|(m, _)| *m != id);
        self.selected.retain(|(m, _)| *m != id);
    }

    /// Ids of the uploaded meshes, in draw order.
    pub fn mesh_ids(&self) -> impl Iterator<Item = MeshId> + '_ {
        self.meshes.keys().copied()
    }

    /// Bounding box of everything uploaded (for [`Camera::fit`]).
    pub fn bounds(&self) -> Option<Aabb> {
        self.meshes
            .values()
            .filter_map(|m| m.aabb)
            .reduce(|mut acc, b| {
                for (lo, v) in acc.min.iter_mut().zip(b.min) {
                    *lo = lo.min(v);
                }
                for (hi, v) in acc.max.iter_mut().zip(b.max) {
                    *hi = hi.max(v);
                }
                acc
            })
    }

    /// Sets the hovered face (`None` clears it).
    pub fn set_hovered(&mut self, face: Option<(MeshId, FaceTag)>) {
        self.hovered = face;
    }

    /// Sets the selected faces; only the first [`MAX_SELECTED`] are highlighted.
    pub fn set_selected(&mut self, faces: &[(MeshId, FaceTag)]) {
        self.selected = faces.to_vec();
    }

    fn global_id(&self, (mesh, tag): (MeshId, FaceTag)) -> Option<u32> {
        let m = self.meshes.get(&mesh)?;
        m.base.checked_add(m.table.local(tag)?)
    }

    fn globals_for(&self, view_proj: &math::Mat4, camera: &Camera, style: &RenderStyle) -> Globals {
        let srgb_target = self.color_format.is_srgb();
        let color = |c: crate::Srgba| if srgb_target { c.to_linear() } else { c.0 };
        let mut selected = [[0u32; 4]; MAX_SELECTED / 4];
        let ids: Vec<u32> = self
            .selected
            .iter()
            .filter_map(|&f| self.global_id(f))
            .take(MAX_SELECTED)
            .collect();
        for (i, id) in ids.iter().enumerate() {
            if let Some(cell) = selected.get_mut(i / 4).and_then(|row| row.get_mut(i % 4)) {
                *cell = *id;
            }
        }
        // Headlight tilted up and to the left of the view axis, so faces equally
        // inclined to the view (an exact iso) still read with different shades.
        let frame = camera.frame();
        let tilted = math::add(
            frame.back,
            math::add(math::scale(frame.up, 0.35), math::scale(frame.right, -0.2)),
        );
        let light = math::normalize(tilted)
            .unwrap_or(frame.back)
            .map(|v| v as f32);
        let unit = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Globals {
            view_proj: math::to_f32(view_proj),
            light_dir: [light[0], light[1], light[2], 0.0],
            face_color: color(style.face),
            hover_color: color(style.hover),
            selected_color: color(style.selected),
            edge_color: color(style.edge),
            shading: [
                unit(style.ambient),
                unit(style.diffuse),
                unit(style.hover_mix),
                0.0,
            ],
            ids: [
                self.hovered.and_then(|f| self.global_id(f)).unwrap_or(0),
                ids.len() as u32,
                0,
                0,
            ],
            selected,
        }
    }

    fn draw_faces(&self, pass: &mut wgpu::RenderPass<'_>) {
        for mesh in self.meshes.values().filter(|m| m.face_vertices > 0) {
            pass.set_vertex_buffer(0, mesh.faces.slice(..));
            pass.set_vertex_buffer(1, mesh.id_base.slice(..));
            pass.draw(0..mesh.face_vertices, 0..1);
        }
    }

    /// Records the shaded pass (clear, faces, edges) into `encoder`.
    ///
    /// The uniforms are written with `Queue::write_buffer`, which takes effect at the
    /// next `submit`: record at most one `render` per renderer per submission.
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &FrameTargets<'_>,
        camera: &Camera,
        viewport: Viewport,
        style: &RenderStyle,
    ) {
        let globals = self.globals_for(&camera.view_projection(viewport), camera, style);
        self.queue
            .write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let bg = if self.color_format.is_srgb() {
            style.background.to_linear()
        } else {
            style.background.0
        };
        let grid_vertices = if style.show_grid {
            self.write_grid(camera, viewport, style)
        } else {
            0
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kubit-render scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: targets.color,
                depth_slice: None,
                resolve_target: targets.resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(bg[0]),
                        g: f64::from(bg[1]),
                        b: f64::from(bg[2]),
                        a: f64::from(bg[3]),
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..wgpu::RenderPassDescriptor::default()
        });
        pass.set_bind_group(0, &self.globals_bind, &[]);
        pass.set_pipeline(&self.face_pipeline);
        self.draw_faces(&mut pass);
        pass.set_pipeline(&self.edge_pipeline);
        for mesh in self.meshes.values().filter(|m| m.edge_vertices > 0) {
            pass.set_vertex_buffer(0, mesh.edges.slice(..));
            pass.draw(0..mesh.edge_vertices, 0..1);
        }
        // Blended after the opaque passes; the depth test hides it behind bodies.
        if grid_vertices > 0 {
            pass.set_pipeline(&self.grid_pipeline);
            pass.set_vertex_buffer(0, self.grid_buffer.slice(..));
            pass.draw(0..grid_vertices, 0..1);
        }
    }

    /// Uploads this frame's grid; returns the vertex count to draw.
    fn write_grid(&self, camera: &Camera, viewport: Viewport, style: &RenderStyle) -> u32 {
        let srgb_target = self.color_format.is_srgb();
        let vertices: Vec<LineVertex> = grid_lines(camera, viewport, style)
            .into_iter()
            .map(|v| {
                let [r, g, b, a] = if srgb_target {
                    v.color.to_linear()
                } else {
                    v.color.0
                };
                LineVertex {
                    position: v.position.map(|c| c as f32),
                    color: [r, g, b, a],
                }
            })
            .take(MAX_GRID_VERTICES)
            .collect();
        if vertices.is_empty() {
            return 0;
        }
        self.queue
            .write_buffer(&self.grid_buffer, 0, bytemuck::cast_slice(&vertices));
        u32::try_from(vertices.len()).unwrap_or(0)
    }

    /// Starts a pick at `cursor_px` (physical pixels, origin top-left). Renders the id
    /// pass into a single pixel through [`Camera::pick_projection`] and requests the
    /// readback. Resolve with [`Renderer::pick_resolve`]: on native right away after
    /// polling, on the web on a later frame.
    pub fn pick_submit(
        &self,
        camera: &Camera,
        viewport: Viewport,
        cursor_px: [f64; 2],
    ) -> PendingPick {
        let [x, y] = cursor_px;
        let inside = x.is_finite()
            && y.is_finite()
            && (0.0..f64::from(viewport.width)).contains(&x)
            && (0.0..f64::from(viewport.height)).contains(&y);
        if !inside {
            return PendingPick { readback: None };
        }
        let globals = self.globals_for(
            &camera.pick_projection(viewport, cursor_px),
            camera,
            &RenderStyle::default(),
        );
        self.queue
            .write_buffer(&self.pick_globals, 0, bytemuck::bytes_of(&globals));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kubit-render pick"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kubit-render pick"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.pick_color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.pick_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..wgpu::RenderPassDescriptor::default()
            });
            pass.set_bind_group(0, &self.pick_bind, &[]);
            pass.set_pipeline(&self.id_pipeline);
            self.draw_faces(&mut pass);
        }
        let readback = Readback::new(&self.device, u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.pick_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: readback.buffer(),
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        readback.request_map();
        PendingPick {
            readback: Some(readback),
        }
    }

    /// `Ok(None)` while pending, `Ok(Some(hit))` when done (`hit` is `None` over the
    /// background). Ids are mapped back with the renderer's current mesh table.
    pub fn pick_resolve(&self, pending: &PendingPick) -> Result<Option<Option<(MeshId, FaceTag)>>> {
        let Some(readback) = &pending.readback else {
            return Ok(Some(None));
        };
        match readback.status() {
            ReadbackStatus::Pending => Ok(None),
            ReadbackStatus::Failed => Err(RenderError::Readback),
            ReadbackStatus::Ready => {
                let bytes = readback.bytes()?;
                let id = bytes
                    .get(0..4)
                    .and_then(|b| b.try_into().ok())
                    .map(u32::from_le_bytes)
                    .ok_or(RenderError::Readback)?;
                Ok(Some(self.decode(id)))
            }
        }
    }

    /// Maps a global face id from the id pass to `(mesh, tag)`.
    pub fn decode(&self, id: u32) -> Option<(MeshId, FaceTag)> {
        let (mesh, local) = self.ids.decode(id)?;
        Some((mesh, self.meshes.get(&mesh)?.table.tag(local)?))
    }

    /// Blocking pick (native only): the face under `cursor_px`, if any.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick(
        &self,
        camera: &Camera,
        viewport: Viewport,
        cursor_px: [f64; 2],
    ) -> Result<Option<(MeshId, FaceTag)>> {
        let pending = scoped(&self.device, || {
            self.pick_submit(camera, viewport, cursor_px)
        })?;
        if let Some(readback) = &pending.readback {
            readback.wait(&self.device)?;
        }
        self.check_gpu_errors()?;
        self.pick_resolve(&pending)?.ok_or(RenderError::Readback)
    }

    /// Renders offscreen and reads back RGBA8 (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_to_image(
        &self,
        camera: &Camera,
        size: Viewport,
        style: &RenderStyle,
    ) -> Result<Image> {
        use wgpu::TextureFormat as F;
        let swap_rb = match self.color_format {
            F::Rgba8Unorm | F::Rgba8UnormSrgb => false,
            F::Bgra8Unorm | F::Bgra8UnormSrgb => true,
            other => return Err(RenderError::ReadbackFormat(other)),
        };
        let max = self.device.limits().max_texture_dimension_2d;
        if size.width > max || size.height > max {
            return Err(RenderError::ImageSize {
                width: size.width,
                height: size.height,
                max,
            });
        }
        let row = size.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row = row.div_ceil(align) * align;
        let (target, readback) = scoped(&self.device, || {
            let target =
                RenderTarget::new(&self.device, size, self.color_format, self.sample_count);
            let readback =
                Readback::new(&self.device, u64::from(padded_row) * u64::from(size.height));
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("kubit-render snapshot"),
                });
            self.render(&mut encoder, &target.targets(), camera, size, style);
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: target.output(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: readback.buffer(),
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_row),
                        rows_per_image: Some(size.height),
                    },
                },
                wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
            );
            self.queue.submit([encoder.finish()]);
            readback.request_map();
            (target, readback)
        })?;
        readback.wait(&self.device)?;
        drop(target);
        self.check_gpu_errors()?;
        let padded = readback.bytes()?;
        let mut rgba = Vec::with_capacity((row * size.height) as usize);
        for line in padded
            .chunks(padded_row as usize)
            .take(size.height as usize)
        {
            let pixels = line.get(..row as usize).ok_or(RenderError::Readback)?;
            if swap_rb {
                for &[b, g, r, a] in pixels.as_chunks::<4>().0 {
                    rgba.extend_from_slice(&[r, g, b, a]);
                }
            } else {
                rgba.extend_from_slice(pixels);
            }
        }
        Ok(Image {
            width: size.width,
            height: size.height,
            rgba,
        })
    }

    /// Renders offscreen and encodes a PNG (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_to_png(
        &self,
        camera: &Camera,
        size: Viewport,
        style: &RenderStyle,
    ) -> Result<Vec<u8>> {
        self.render_to_image(camera, size, style)?.to_png()
    }

    /// Fails with the first error the headless device reported since the last check.
    #[cfg(not(target_arch = "wasm32"))]
    fn check_gpu_errors(&self) -> Result<()> {
        let Some(errors) = &self.gpu_errors else {
            return Ok(());
        };
        let mut list = errors
            .lock()
            .map_err(|_| RenderError::Gpu("error log poisoned".into()))?;
        match list.drain(..).next() {
            Some(first) => Err(RenderError::Gpu(first)),
            None => Ok(()),
        }
    }
}
