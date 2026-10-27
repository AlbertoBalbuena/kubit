//! The 3D viewport: kubit-render on eframe's wgpu device, drawn offscreen into a
//! texture that egui shows as an image.
//!
//! Offscreen (instead of an egui-wgpu paint callback) because the renderer owns its
//! MSAA and depth attachments and records its own pass; egui only samples the result.
//! Picking is asynchronous: a pick is submitted on one frame and resolved on a later
//! one, so hovering never blocks the UI thread on a GPU readback.

use crate::handle::{self, Arrow, FaceFrame};
use crate::sketch_ui::{ViewInfo, facing};
use crate::triad;
use crate::value_input::format_mm;
use eframe::egui_wgpu::{self, wgpu};
use egui::{PointerButton, Rect, Sense, Ui};
use kubit_doc::FeatureId;
use kubit_engine::Session;
use kubit_mesh::{EdgeTag, FaceTag};
use kubit_render::{
    Camera, DEPTH_FORMAT, FrameTargets, MeshId, PendingPick, RenderError, RenderStyle, Renderer,
    StandardView, Viewport,
};
use kubit_render::{EDGE_PICK_TOLERANCE_PX, nearest_edge, project_to_pixel};
use std::collections::BTreeMap;

/// Chord tolerance of the viewport tessellation, mm.
const CHORD_MM: f64 = 0.1;
/// MSAA samples of the viewport.
const SAMPLES: u32 = 4;
/// What the renderer draws into: sRGB, so shading happens in linear light.
const RENDER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// How egui samples it: egui-wgpu expects gamma-encoded `Rgba8Unorm` native textures,
/// which is the same bytes as [`RENDER_FORMAT`] viewed without the sRGB decode.
const EGUI_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The renderer's colour format on this device. Viewing an sRGB texture as unorm needs
/// `VIEW_FORMATS`, which some GL / software adapters lack; there the renderer draws
/// straight into [`EGUI_FORMAT`] (shading then blends in gamma space, a minor difference).
fn render_format(adapter: &wgpu::Adapter) -> wgpu::TextureFormat {
    let flags = adapter.get_downlevel_capabilities().flags;
    if flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS) {
        RENDER_FORMAT
    } else {
        EGUI_FORMAT
    }
}
/// Orbit speed, radians per point dragged.
const ORBIT_RAD_PER_PT: f64 = 0.008;
/// Zoom speed per point scrolled.
const ZOOM_PER_PT: f64 = 0.0025;
/// Points per wheel line (egui's default `line_scroll_speed` on native).
const PT_PER_LINE: f64 = 40.0;

/// Camera motion asked for by one frame's wheel and gesture input.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WheelNav {
    /// Orbit drag, points.
    orbit: [f64; 2],
    /// Pan drag, points.
    pan: [f64; 2],
    /// Zoom factor (> 1 zooms in).
    zoom: f64,
}

/// Maps wheel events and pinch zoom to camera motion.
///
/// A mouse wheel reports lines and zooms. A trackpad (and other smooth-scrolling
/// devices) reports points: two-finger scroll orbits, Shift + two-finger scroll pans,
/// and pinch zooms. Wheel events with the zoom modifier (Cmd / Ctrl) are skipped
/// because egui already folds them into `zoom_delta`.
fn wheel_nav(events: &[egui::Event], zoom_delta: f32) -> WheelNav {
    let mut nav = WheelNav {
        orbit: [0.0; 2],
        pan: [0.0; 2],
        zoom: f64::from(zoom_delta),
    };
    for event in events {
        let egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        if modifiers.command {
            continue;
        }
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        match unit {
            egui::MouseWheelUnit::Point if modifiers.shift => {
                nav.pan[0] += dx;
                nav.pan[1] += dy;
            }
            egui::MouseWheelUnit::Point => {
                nav.orbit[0] += dx;
                nav.orbit[1] += dy;
            }
            egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                nav.zoom *= (dy * PT_PER_LINE * ZOOM_PER_PT).exp();
            }
        }
    }
    nav
}

/// A face of a document body, as the renderer sees it.
pub type FaceHit = (MeshId, FaceTag);
/// A B-rep edge of a document body.
pub type EdgeHit = (MeshId, EdgeTag);

/// What the cursor is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// A face.
    Face(FaceHit),
    /// An edge (wins over the face when within the pick tolerance).
    Edge(EdgeHit),
}

/// A B-rep edge kept for picking and highlighting.
#[derive(Clone, Debug, PartialEq)]
struct EdgeEntry {
    tag: EdgeTag,
    points: Vec<[f64; 3]>,
    /// The faces it bounds (when named); only these faces offer it for picking,
    /// which keeps edges hidden behind the body from being picked through it.
    faces: [Option<FaceTag>; 2],
}

/// Mesh id of a document body.
pub fn mesh_id(body: FeatureId) -> MeshId {
    MeshId(u64::from(body.0))
}

/// Document body of a mesh id.
pub fn body_of(mesh: MeshId) -> Option<FeatureId> {
    u32::try_from(mesh.0).ok().map(FeatureId)
}

struct Target {
    size: Viewport,
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    _resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture: egui::TextureId,
}

impl Target {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: Viewport,
        egui_renderer: &mut egui_wgpu::Renderer,
    ) -> Self {
        let egui_view_formats: &[wgpu::TextureFormat] = if format == EGUI_FORMAT {
            &[]
        } else {
            &[EGUI_FORMAT]
        };
        let texture = |label, samples, format, usage, view_formats: &[wgpu::TextureFormat]| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats,
            })
        };
        let color = texture(
            "kubit-ui viewport msaa",
            SAMPLES,
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            &[],
        );
        let resolve = texture(
            "kubit-ui viewport",
            1,
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            egui_view_formats,
        );
        let depth = texture(
            "kubit-ui viewport depth",
            SAMPLES,
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            &[],
        );
        let egui_view = resolve.create_view(&wgpu::TextureViewDescriptor {
            format: Some(EGUI_FORMAT),
            ..wgpu::TextureViewDescriptor::default()
        });
        let texture_id =
            egui_renderer.register_native_texture(device, &egui_view, wgpu::FilterMode::Nearest);
        Self {
            size,
            color_view: color.create_view(&wgpu::TextureViewDescriptor::default()),
            _color: color,
            resolve_view: resolve.create_view(&wgpu::TextureViewDescriptor::default()),
            _resolve: resolve,
            depth_view: depth.create_view(&wgpu::TextureViewDescriptor::default()),
            _depth: depth,
            texture: texture_id,
        }
    }
}

/// A push/pull handle drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PullDrag {
    /// Pointer at the start of the drag, physical pixels.
    press: [f64; 2],
    /// The arrow when the drag started (the scale of the drag).
    arrow: Arrow,
    /// Current snapped distance, mm.
    mm: f64,
}

/// What happened in the viewport this frame.
#[derive(Clone, Debug, Default)]
pub struct ViewportResponse {
    /// The camera and screen placement this frame (for overlays).
    pub view: Option<ViewInfo>,
    /// The viewport's egui response (sketch mode reads clicks and drags from it).
    pub response: Option<egui::Response>,
    /// The push/pull handle was released after dragging this far, mm.
    pub pull: Option<f64>,
    /// A right click (no drag) at this position: open the context menu.
    pub context: Option<egui::Pos2>,
    /// A primary click: the edge or face under the cursor, or `None` for empty space.
    pub clicked: Option<Option<Hit>>,
    /// Whether Shift was held on that click (add to the selection).
    pub shift: bool,
}

/// Viewport state: renderer, camera, offscreen target and picking.
pub struct Viewport3d {
    renderer: Renderer,
    render_state: egui_wgpu::RenderState,
    target: Option<Target>,
    /// The camera (view state, not part of the document).
    pub camera: Camera,
    pending: Option<PendingPick>,
    hovered: Option<FaceHit>,
    hovered_edge: Option<EdgeHit>,
    edges: BTreeMap<MeshId, Vec<EdgeEntry>>,
    selected_edges: Vec<EdgeHit>,
    face_frames: BTreeMap<FaceHit, FaceFrame>,
    /// The face carrying the push/pull arrow (the only selected face).
    pull_face: Option<FaceHit>,
    pull: Option<PullDrag>,
    /// Face/edge picking, hover and the push/pull handle (off in sketch mode).
    pub pick_enabled: bool,
    /// Bodies not drawn (view state).
    hidden: std::collections::BTreeSet<FeatureId>,
    last_cursor: Option<[f64; 2]>,
    fitted: bool,
}

impl Viewport3d {
    /// A viewport on eframe's device.
    pub fn new(render_state: &egui_wgpu::RenderState) -> Result<Self, RenderError> {
        let renderer = Renderer::new(
            &render_state.device,
            &render_state.queue,
            render_format(&render_state.adapter),
            SAMPLES,
        )?;
        Ok(Self {
            renderer,
            render_state: render_state.clone(),
            target: None,
            camera: Camera::default(),
            pending: None,
            hovered: None,
            hovered_edge: None,
            edges: BTreeMap::new(),
            selected_edges: Vec::new(),
            face_frames: BTreeMap::new(),
            pull_face: None,
            pull: None,
            pick_enabled: true,
            hidden: std::collections::BTreeSet::new(),
            last_cursor: None,
            fitted: false,
        })
    }

    /// The face under the cursor, if any.
    pub fn hovered(&self) -> Option<FaceHit> {
        self.hovered
    }

    /// The edge under the cursor, if any (it takes precedence over the face).
    pub fn hovered_edge(&self) -> Option<EdgeHit> {
        self.hovered_edge
    }

    /// Sets the bodies not to draw; takes effect at the next `sync_meshes`.
    pub fn set_hidden(&mut self, hidden: std::collections::BTreeSet<FeatureId>) {
        self.hidden = hidden;
    }

    /// Re-uploads every body of the session and drops removed ones.
    pub fn sync_meshes(&mut self, session: &mut Session) -> Result<(), String> {
        let bodies: Vec<FeatureId> = session
            .model()
            .bodies
            .keys()
            .copied()
            .filter(|b| !self.hidden.contains(b))
            .collect();
        let stale: Vec<MeshId> = self
            .renderer
            .mesh_ids()
            .filter(|m| body_of(*m).is_none_or(|b| !bodies.contains(&b)))
            .collect();
        for m in stale {
            self.renderer.remove(m);
        }
        self.edges.clear();
        self.face_frames.clear();
        for body in bodies {
            let mesh = session
                .body_mesh(body, CHORD_MM)
                .map_err(|e| e.to_string())?;
            self.renderer
                .upload(mesh_id(body), mesh)
                .map_err(|e| e.to_string())?;
            for (tag, frame) in handle::face_frames(mesh) {
                self.face_frames.insert((mesh_id(body), tag), frame);
            }
            let polylines = mesh.edges.clone();
            let entries = polylines
                .into_iter()
                .map(|(tag, points)| EdgeEntry {
                    tag,
                    points,
                    faces: edge_faces(session, body, tag),
                })
                .collect();
            self.edges.insert(mesh_id(body), entries);
        }
        self.hovered = None;
        self.hovered_edge = None;
        self.renderer.set_hovered(None);
        if !self.fitted && self.renderer.bounds().is_some() {
            self.fit();
            self.fitted = true;
        }
        Ok(())
    }

    /// Highlights the session's selected faces.
    pub fn sync_selection(&mut self, session: &Session) {
        let model = session.model();
        let faces: Vec<FaceHit> = session
            .selection()
            .faces
            .iter()
            .filter_map(|name| {
                let (body, state) = model.body_of_face(name)?;
                let face = state.names.get(name)?;
                Some((mesh_id(body), FaceTag(face.0)))
            })
            .collect();
        self.renderer.set_selected(&faces);
        self.pull_face = (faces.len() == 1 && session.selection().edges.is_empty())
            .then(|| faces.first().copied())
            .flatten();
        self.pull = None;
        self.selected_edges = session
            .selection()
            .edges
            .iter()
            .filter_map(|name| {
                let (body, state) = model.body_of_edge(name)?;
                let edge = state.edges.get(name)?;
                Some((mesh_id(body), EdgeTag(edge.0)))
            })
            .collect();
    }

    /// The edge within the pick tolerance of `cursor` (physical pixels). With a
    /// face under the cursor only that face's edges qualify; over empty space every
    /// edge does (silhouettes).
    fn edge_at(
        &self,
        face: Option<FaceHit>,
        cursor: [f64; 2],
        size: Viewport,
        ppp: f64,
    ) -> Option<EdgeHit> {
        let candidates = self.edges.iter().flat_map(|(mesh, list)| {
            list.iter()
                .filter(move |e| match face {
                    Some((m, f)) => m == *mesh && e.faces.contains(&Some(f)),
                    None => true,
                })
                .map(move |e| ((*mesh, e.tag), e.points.as_slice()))
        });
        nearest_edge(
            &self.camera,
            size,
            cursor,
            EDGE_PICK_TOLERANCE_PX * ppp,
            candidates,
        )
        .map(|(hit, _)| hit)
    }

    fn edge_points(&self, (mesh, tag): EdgeHit) -> Option<&[[f64; 3]]> {
        self.edges
            .get(&mesh)?
            .iter()
            .find(|e| e.tag == tag)
            .map(|e| e.points.as_slice())
    }

    /// Frames `bounds` (e.g. a sketch) instead of the bodies.
    pub fn fit_to(&mut self, bounds: &kubit_mesh::Aabb) {
        let aspect = self.target.as_ref().map_or(16.0 / 9.0, |t| t.size.aspect());
        self.camera.fit(bounds, aspect);
    }

    /// Frames every body.
    pub fn fit(&mut self) {
        let aspect = self.target.as_ref().map_or(16.0 / 9.0, |t| t.size.aspect());
        if let Some(bounds) = self.renderer.bounds() {
            self.camera.fit(&bounds, aspect);
        }
    }

    /// Looks straight at a plane from the side its `normal` points to.
    pub fn look_along(&mut self, normal: [f64; 3]) {
        let (yaw, pitch) = facing(normal);
        self.camera.yaw = yaw.rem_euclid(std::f64::consts::TAU);
        self.camera.pitch = pitch;
    }

    /// Switches to a standard view and frames the bodies.
    pub fn set_view(&mut self, view: StandardView) {
        self.camera.set_view(view);
        self.fit();
    }

    /// Lays out, handles input, renders and paints the viewport into `ui`.
    pub fn show(&mut self, ui: &mut Ui, style: &RenderStyle) -> ViewportResponse {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = f64::from(ui.ctx().pixels_per_point());
        let size = Viewport::new(
            (f64::from(rect.width()) * ppp).round() as u32,
            (f64::from(rect.height()) * ppp).round() as u32,
        );
        self.ensure_target(size);
        let cursor = response.hover_pos().map(|p| to_px(p - rect.min, ppp));

        self.navigate(ui, &response, rect, size, ppp);
        let cursor = cursor.filter(|_| self.pick_enabled);
        if !self.pick_enabled {
            self.pull = None;
        }
        self.update_hover(cursor, size, response.dragged());
        self.hovered_edge = match cursor {
            Some(c) if !response.dragged() => self.edge_at(self.hovered, c, size, ppp),
            _ => None,
        };
        // An edge under the cursor takes the highlight from its face.
        self.renderer.set_hovered(if self.hovered_edge.is_some() {
            None
        } else {
            self.hovered
        });

        let triad_center = rect.right_top() + egui::vec2(-triad::INSET, triad::INSET);
        let frame = self.camera.frame();
        let triad_hover = response
            .hover_pos()
            .and_then(|p| triad::hit(&frame, triad_center, p));

        let mut out = ViewportResponse::default();
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .map(|p| to_px(p - rect.min, ppp));
        let handle_arrow = self
            .pull_face
            .filter(|_| self.pick_enabled)
            .and_then(|f| self.face_frames.get(&f))
            .and_then(|frame| {
                handle::arrow(&self.camera, size, frame, 0.0, handle::ARROW_PX * ppp)
            });
        let near_handle = match (handle_arrow, cursor) {
            (Some(a), Some(c)) => {
                handle::distance_to_segment(a.start, a.end, c) <= handle::GRAB_PX * ppp
            }
            _ => false,
        };
        if response.drag_started_by(PointerButton::Primary)
            && near_handle
            && let (Some(arrow), Some(press)) = (handle_arrow, cursor)
        {
            self.pull = Some(PullDrag {
                press,
                arrow,
                mm: 0.0,
            });
        }
        if let Some(drag) = &mut self.pull {
            if let Some(now) = pointer {
                let raw = handle::drag_distance(&drag.arrow, drag.press, now);
                drag.mm = handle::snap(raw, handle::snap_step(&self.camera));
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                if drag.mm != 0.0 {
                    out.pull = Some(drag.mm);
                }
                self.pull = None;
            }
        }
        if response.clicked_by(PointerButton::Secondary) {
            out.context = response.interact_pointer_pos();
        }
        if response.clicked_by(PointerButton::Primary) {
            if let Some(tip) = triad_hover {
                self.set_view(tip.view());
            } else if self.pick_enabled {
                out.shift = ui.input(|i| i.modifiers.shift);
                out.clicked = Some(cursor.and_then(|c| {
                    let face = self.renderer.pick(&self.camera, size, c).ok().flatten();
                    self.edge_at(face, c, size, ppp)
                        .map(Hit::Edge)
                        .or(face.map(Hit::Face))
                }));
            }
        }

        self.render(size, style);
        if let Some(t) = &self.target {
            ui.painter().image(
                t.texture,
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        let opaque = |c: kubit_render::Srgba| {
            let [r, g, b, _] = c.0.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            egui::Color32::from_rgb(r, g, b)
        };
        let painter = ui.painter().with_clip_rect(rect);
        let to_screen = |p: [f64; 3]| {
            project_to_pixel(&self.camera, size, p)
                .map(|[x, y]| rect.min + egui::vec2((x / ppp) as f32, (y / ppp) as f32))
        };
        let stroke_edge = |hit: EdgeHit, color: egui::Color32| {
            let Some(points) = self.edge_points(hit) else {
                return;
            };
            for pair in points.windows(2) {
                if let [a, b] = pair
                    && let (Some(a), Some(b)) = (to_screen(*a), to_screen(*b))
                {
                    painter.line_segment([a, b], egui::Stroke::new(3.0, color));
                }
            }
        };
        for hit in &self.selected_edges {
            stroke_edge(*hit, opaque(style.selected));
        }
        if let Some(face) = self.pull_face.filter(|_| self.pick_enabled)
            && let Some(frame) = self.face_frames.get(&face).copied()
        {
            let offset = self.pull.map_or(0.0, |d| d.mm);
            let lit = self.pull.is_some() || near_handle;
            // Lighter than the selected face it stands on, outlined so it reads on any shade.
            let color = opaque(if lit { style.face } else { style.hover });
            let outline = opaque(style.edge);
            if let Some(a) =
                handle::arrow(&self.camera, size, &frame, offset, handle::ARROW_PX * ppp)
            {
                let at =
                    |p: [f64; 2]| rect.min + egui::vec2((p[0] / ppp) as f32, (p[1] / ppp) as f32);
                let (start, end) = (at(a.start), at(a.end));
                let width = if lit { 3.5 } else { 2.5 };
                painter.line_segment([start, end], egui::Stroke::new(width + 2.0, outline));
                painter.line_segment([start, end], egui::Stroke::new(width, color));
                let dir = (end - start).normalized();
                let side = egui::vec2(-dir.y, dir.x) * 7.0;
                painter.add(egui::Shape::convex_polygon(
                    vec![end + dir * 12.0, end + side, end - side],
                    color,
                    egui::Stroke::new(1.0, outline),
                ));
                painter.circle(start, 4.0, color, egui::Stroke::new(1.0, outline));
                if let Some(drag) = self.pull {
                    painter.text(
                        end + dir * 18.0 + egui::vec2(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format_mm(drag.mm),
                        egui::FontId::monospace(13.0),
                        color,
                    );
                }
            }
            // Preview: the face outline moved along its normal.
            if let Some(drag) = self.pull
                && let Some(list) = self.edges.get(&face.0)
            {
                let n = frame.normal;
                let moved = |p: [f64; 3]| {
                    [
                        p[0] + n[0] * drag.mm,
                        p[1] + n[1] * drag.mm,
                        p[2] + n[2] * drag.mm,
                    ]
                };
                for e in list.iter().filter(|e| e.faces.contains(&Some(face.1))) {
                    for pair in e.points.windows(2) {
                        if let [a, b] = pair
                            && let (Some(a), Some(b)) = (to_screen(moved(*a)), to_screen(moved(*b)))
                        {
                            painter.line_segment([a, b], egui::Stroke::new(2.0, color));
                        }
                    }
                }
            }
        }
        if let Some(hit) = self.hovered_edge {
            stroke_edge(hit, opaque(style.hover));
        }
        triad::paint(
            &painter,
            &self.camera.frame(),
            triad_center,
            [
                opaque(style.axis_x),
                opaque(style.axis_y),
                opaque(style.axis_z),
            ],
            opaque(style.face),
            triad_hover,
        );
        out.view = Some(ViewInfo {
            camera: self.camera,
            size,
            rect,
            ppp,
        });
        // A triad click is the triad's, not the sketch's.
        out.response = triad_hover.is_none().then_some(response);
        out
    }

    fn ensure_target(&mut self, size: Viewport) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let mut egui_renderer = self.render_state.renderer.write();
        if let Some(old) = self.target.take() {
            egui_renderer.free_texture(&old.texture);
        }
        self.target = Some(Target::new(
            &self.render_state.device,
            self.renderer.color_format(),
            size,
            &mut egui_renderer,
        ));
    }

    fn navigate(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        size: Viewport,
        ppp: f64,
    ) {
        let shift = ui.input(|i| i.modifiers.shift);
        let delta = response.drag_delta();
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        let middle = response.dragged_by(PointerButton::Middle);
        let secondary = response.dragged_by(PointerButton::Secondary);
        if (middle || secondary) && shift {
            self.camera.pan(dx * ppp, dy * ppp, size);
        } else if middle || secondary {
            self.camera
                .orbit(-dx * ORBIT_RAD_PER_PT, dy * ORBIT_RAD_PER_PT);
        }
        if !response.hovered() {
            return;
        }
        let nav = ui.input(|i| wheel_nav(&i.raw.events, i.zoom_delta()));
        let [ox, oy] = nav.orbit;
        if ox != 0.0 || oy != 0.0 {
            self.camera
                .orbit(-ox * ORBIT_RAD_PER_PT, oy * ORBIT_RAD_PER_PT);
        }
        let [px, py] = nav.pan;
        if px != 0.0 || py != 0.0 {
            self.camera.pan(px * ppp, py * ppp, size);
        }
        if nav.zoom.is_finite()
            && nav.zoom > 0.0
            && nav.zoom != 1.0
            && let Some(p) = response.hover_pos()
        {
            let [x, y] = to_px(p - rect.min, ppp);
            let ndc = size.pixel_to_ndc(x, y);
            self.camera.zoom(nav.zoom, ndc, size);
        }
    }

    fn update_hover(&mut self, cursor: Option<[f64; 2]>, size: Viewport, dragging: bool) {
        // Deliver finished readbacks (native: map callbacks run inside poll).
        let _ = self.render_state.device.poll(wgpu::PollType::Poll);
        if let Some(p) = &self.pending {
            match self.renderer.pick_resolve(p) {
                Ok(Some(hit)) => {
                    self.pending = None;
                    self.set_hovered(hit);
                }
                Ok(None) => {}
                Err(_) => self.pending = None,
            }
        }
        match cursor {
            Some(c) if !dragging => {
                if self.pending.is_none() && self.last_cursor != Some(c) {
                    self.last_cursor = Some(c);
                    self.pending = Some(self.renderer.pick_submit(&self.camera, size, c));
                }
            }
            _ => {
                self.last_cursor = None;
                self.set_hovered(None);
            }
        }
    }

    fn set_hovered(&mut self, hit: Option<FaceHit>) {
        if self.hovered != hit {
            self.hovered = hit;
            self.renderer.set_hovered(hit);
        }
    }

    /// Whether a pick is still in flight (the app keeps repainting until it lands).
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    fn render(&mut self, size: Viewport, style: &RenderStyle) {
        let Some(t) = &self.target else {
            return;
        };
        let device = &self.render_state.device;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("kubit-ui viewport"),
        });
        let targets = FrameTargets {
            color: &t.color_view,
            resolve: Some(&t.resolve_view),
            depth: &t.depth_view,
        };
        self.renderer
            .render(&mut encoder, &targets, &self.camera, size, style);
        self.render_state.queue.submit([encoder.finish()]);
    }
}

/// A point offset (points) to physical pixels.
/// Mesh tags of the two faces an edge bounds, from its persistent name.
fn edge_faces(session: &Session, body: FeatureId, tag: EdgeTag) -> [Option<FaceTag>; 2] {
    let Some(name) = session.edge_name(body, tag) else {
        return [None, None];
    };
    let state = session.model().bodies.get(&body);
    name.faces()
        .map(|face| state.and_then(|s| s.names.get(face)).map(|f| FaceTag(f.0)))
}

fn to_px(v: egui::Vec2, ppp: f64) -> [f64; 2] {
    [f64::from(v.x) * ppp, f64::from(v.y) * ppp]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_ids_round_trip_body_ids() {
        let body = FeatureId(7);
        assert_eq!(body_of(mesh_id(body)), Some(body));
        assert_eq!(body_of(MeshId(u64::MAX)), None);
    }

    fn wheel(
        unit: egui::MouseWheelUnit,
        x: f32,
        y: f32,
        modifiers: egui::Modifiers,
    ) -> egui::Event {
        egui::Event::MouseWheel {
            unit,
            delta: egui::vec2(x, y),
            phase: egui::TouchPhase::Move,
            modifiers,
        }
    }

    #[test]
    fn trackpad_scroll_orbits_and_shift_scroll_pans() {
        let none = egui::Modifiers::NONE;
        let events = [
            wheel(egui::MouseWheelUnit::Point, 3.0, -2.0, none),
            wheel(
                egui::MouseWheelUnit::Point,
                4.0,
                5.0,
                egui::Modifiers::SHIFT,
            ),
        ];
        let nav = wheel_nav(&events, 1.0);
        assert_eq!(nav.orbit, [3.0, -2.0]);
        assert_eq!(nav.pan, [4.0, 5.0]);
        assert_eq!(nav.zoom, 1.0);
    }

    #[test]
    fn mouse_wheel_and_pinch_zoom() {
        let events = [wheel(
            egui::MouseWheelUnit::Line,
            0.0,
            1.0,
            egui::Modifiers::NONE,
        )];
        let nav = wheel_nav(&events, 1.5);
        assert_eq!(nav.orbit, [0.0, 0.0]);
        assert_eq!(nav.pan, [0.0, 0.0]);
        let expected = 1.5 * (PT_PER_LINE * ZOOM_PER_PT).exp();
        assert!((nav.zoom - expected).abs() < 1e-12);
    }

    #[test]
    fn zoom_modifier_wheel_is_left_to_egui() {
        let events = [wheel(
            egui::MouseWheelUnit::Point,
            0.0,
            9.0,
            egui::Modifiers::COMMAND,
        )];
        let nav = wheel_nav(&events, 1.0);
        assert_eq!(nav.orbit, [0.0, 0.0]);
        assert_eq!(nav.zoom, 1.0);
    }
}
