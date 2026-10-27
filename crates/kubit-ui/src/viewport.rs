//! The 3D viewport: kubit-render on eframe's wgpu device, drawn offscreen into a
//! texture that egui shows as an image.
//!
//! Offscreen (instead of an egui-wgpu paint callback) because the renderer owns its
//! MSAA and depth attachments and records its own pass; egui only samples the result.
//! Picking is asynchronous: a pick is submitted on one frame and resolved on a later
//! one, so hovering never blocks the UI thread on a GPU readback.

use eframe::egui_wgpu::{self, wgpu};
use egui::{PointerButton, Rect, Sense, Ui};
use kubit_doc::FeatureId;
use kubit_engine::Session;
use kubit_mesh::FaceTag;
use kubit_render::{
    Camera, DEPTH_FORMAT, FrameTargets, MeshId, PendingPick, RenderError, RenderStyle, Renderer,
    StandardView, Viewport,
};

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

/// A face of a document body, as the renderer sees it.
pub type FaceHit = (MeshId, FaceTag);

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

/// What happened in the viewport this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewportResponse {
    /// A primary click: the face under the cursor, or `None` for empty space.
    pub clicked: Option<Option<FaceHit>>,
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
            last_cursor: None,
            fitted: false,
        })
    }

    /// The face under the cursor, if any.
    pub fn hovered(&self) -> Option<FaceHit> {
        self.hovered
    }

    /// Re-uploads every body of the session and drops removed ones.
    pub fn sync_meshes(&mut self, session: &mut Session) -> Result<(), String> {
        let bodies: Vec<FeatureId> = session.model().bodies.keys().copied().collect();
        let stale: Vec<MeshId> = self
            .renderer
            .mesh_ids()
            .filter(|m| body_of(*m).is_none_or(|b| !bodies.contains(&b)))
            .collect();
        for m in stale {
            self.renderer.remove(m);
        }
        for body in bodies {
            let mesh = session
                .body_mesh(body, CHORD_MM)
                .map_err(|e| e.to_string())?;
            self.renderer
                .upload(mesh_id(body), mesh)
                .map_err(|e| e.to_string())?;
        }
        self.hovered = None;
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
    }

    /// Frames every body.
    pub fn fit(&mut self) {
        let aspect = self.target.as_ref().map_or(16.0 / 9.0, |t| t.size.aspect());
        if let Some(bounds) = self.renderer.bounds() {
            self.camera.fit(&bounds, aspect);
        }
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
        self.update_hover(cursor, size, response.dragged());

        let mut out = ViewportResponse::default();
        if response.clicked_by(PointerButton::Primary) {
            out.clicked =
                Some(cursor.and_then(|c| self.renderer.pick(&self.camera, size, c).ok().flatten()));
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
        if middle && shift {
            self.camera.pan(dx * ppp, dy * ppp, size);
        } else if middle || response.dragged_by(PointerButton::Secondary) {
            self.camera
                .orbit(-dx * ORBIT_RAD_PER_PT, dy * ORBIT_RAD_PER_PT);
        }
        if response.hovered() {
            let scroll = f64::from(ui.input(|i| i.smooth_scroll_delta.y));
            if scroll != 0.0
                && let Some(p) = response.hover_pos()
            {
                let [x, y] = to_px(p - rect.min, ppp);
                let ndc = size.pixel_to_ndc(x, y);
                self.camera.zoom((scroll * ZOOM_PER_PT).exp(), ndc, size);
            }
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
}
