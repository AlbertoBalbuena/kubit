//! Sketch mode: drawing tools, snapping and inference, dimensions, DOF colours and
//! point dragging, on top of the viewport.
//!
//! Every edit becomes a `sketch.*` command (the engine solves and stores it); this
//! module only turns pointer input into those commands and draws the sketch. New
//! entities are numbered from the sketch's `next_id`, which the engine checks, so one
//! `sketch.add` can create points and the lines that use them.

use crate::handle;
use crate::value_input::{FieldAction, ValueField, format_mm};
use egui::{Color32, Pos2, Rect, Stroke};
use kubit_doc::FeatureId;
use kubit_engine::{PlaneFrame, SketchState};
use kubit_render::{Camera, Viewport, project_to_pixel};
use kubit_sketch::{
    Constraint, ConstraintId, Entity, EntityId, Freedom, Region, Sketch, SolveOptions,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Points closer than this snap together, points (UI units).
const SNAP_PT: f32 = 9.0;
/// Curves closer than this are hit, points.
const HIT_PT: f32 = 7.0;
/// A line within this angle of the u or v axis is inferred horizontal or vertical.
const INFER_RAD: f64 = 0.035;

/// The camera and where the viewport sits on screen this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewInfo {
    /// Camera.
    pub camera: Camera,
    /// Physical size.
    pub size: Viewport,
    /// Screen rectangle, points.
    pub rect: Rect,
    /// Pixels per point.
    pub ppp: f64,
}

impl ViewInfo {
    /// Screen position of a world point.
    pub fn screen_of(&self, w: [f64; 3]) -> Option<Pos2> {
        let [x, y] = project_to_pixel(&self.camera, self.size, w)?;
        Some(self.rect.min + egui::vec2((x / self.ppp) as f32, (y / self.ppp) as f32))
    }

    /// The point of `frame`'s plane under screen position `pos`.
    pub fn plane_point(&self, frame: &PlaneFrame, pos: Pos2) -> Option<[f64; 2]> {
        let px = [
            f64::from(pos.x - self.rect.min.x) * self.ppp,
            f64::from(pos.y - self.rect.min.y) * self.ppp,
        ];
        let ndc = self.size.pixel_to_ndc(px[0], px[1]);
        let (o, d) = self.camera.ray(self.size, ndc)?;
        let n = frame.normal;
        let den = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
        if den.abs() < 1e-9 {
            return None;
        }
        let rel = [
            frame.origin[0] - o[0],
            frame.origin[1] - o[1],
            frame.origin[2] - o[2],
        ];
        let t = (rel[0] * n[0] + rel[1] * n[1] + rel[2] * n[2]) / den;
        let hit = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
        let p = frame.to_plane(hit);
        p.iter().all(|c| c.is_finite()).then_some(p)
    }
}

/// Camera yaw and pitch that look straight at a plane from its normal side.
pub fn facing(normal: [f64; 3]) -> (f64, f64) {
    let pitch = normal[2].clamp(-1.0, 1.0).asin();
    let yaw = if normal[2].abs() > 0.999 {
        -std::f64::consts::FRAC_PI_2
    } else {
        normal[1].atan2(normal[0])
    };
    (yaw, pitch)
}

/// A drawing tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Select, drag points, delete.
    Select,
    /// Chained lines.
    Line,
    /// Two-corner rectangle.
    Rectangle,
    /// Centre and radius.
    Circle,
    /// Centre, start, end (counter-clockwise).
    Arc,
    /// Length, radius or point-to-point distance.
    Dimension,
}

impl Tool {
    /// All tools, in toolbar order.
    pub const ALL: [Self; 6] = [
        Self::Select,
        Self::Line,
        Self::Rectangle,
        Self::Circle,
        Self::Arc,
        Self::Dimension,
    ];

    /// i18n key.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Select => "ui.sketch.select",
            Self::Line => "ui.sketch.line",
            Self::Rectangle => "ui.sketch.rectangle",
            Self::Circle => "ui.sketch.circle",
            Self::Arc => "ui.sketch.arc",
            Self::Dimension => "ui.sketch.dimension",
        }
    }

    /// Icon name (`crate::icons`).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Line => "line",
            Self::Rectangle => "rectangle",
            Self::Circle => "circle",
            Self::Arc => "arc",
            Self::Dimension => "dimension",
        }
    }

    /// Single-key shortcut.
    pub fn key(self) -> egui::Key {
        match self {
            Self::Select => egui::Key::Q,
            Self::Line => egui::Key::L,
            Self::Rectangle => egui::Key::R,
            Self::Circle => egui::Key::C,
            Self::Arc => egui::Key::A,
            Self::Dimension => egui::Key::D,
        }
    }
}

/// A constraint applied by hand to the selected entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintKind {
    /// Selected lines horizontal.
    Horizontal,
    /// Selected lines vertical.
    Vertical,
    /// Two lines parallel.
    Parallel,
    /// Two lines perpendicular.
    Perpendicular,
    /// A line and a circle/arc, or two circles/arcs, tangent.
    Tangent,
    /// Two points together, or a point on a curve.
    Coincident,
    /// A point at the middle of a line.
    Midpoint,
    /// Two lines of equal length, or two circles/arcs of equal radius.
    Equal,
    /// Selected points fixed where they are.
    Fix,
}

impl ConstraintKind {
    /// In ribbon order.
    pub const ALL: [Self; 9] = [
        Self::Horizontal,
        Self::Vertical,
        Self::Parallel,
        Self::Perpendicular,
        Self::Tangent,
        Self::Coincident,
        Self::Midpoint,
        Self::Equal,
        Self::Fix,
    ];

    /// Icon name.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Parallel => "parallel",
            Self::Perpendicular => "perpendicular",
            Self::Tangent => "tangent",
            Self::Coincident => "coincident",
            Self::Midpoint => "midpoint",
            Self::Equal => "equal",
            Self::Fix => "fix",
        }
    }

    /// i18n key.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Horizontal => "ui.constraint.horizontal",
            Self::Vertical => "ui.constraint.vertical",
            Self::Parallel => "ui.constraint.parallel",
            Self::Perpendicular => "ui.constraint.perpendicular",
            Self::Tangent => "ui.constraint.tangent",
            Self::Coincident => "ui.constraint.coincident",
            Self::Midpoint => "ui.constraint.midpoint",
            Self::Equal => "ui.constraint.equal",
            Self::Fix => "ui.constraint.fix",
        }
    }
}

/// A placed point: where, and the existing point it snapped to.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Snap {
    at: [f64; 2],
    point: Option<EntityId>,
    /// Snapped to the sketch origin (gets a fix constraint).
    origin: bool,
}

/// What a dimension being typed will constrain.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DimTarget {
    /// A new dimension (its value is replaced by what is typed).
    New(Constraint),
    /// An existing dimension.
    Existing(ConstraintId),
}

/// The inline field of a dimension.
#[derive(Clone, Debug, PartialEq)]
struct DimEdit {
    target: DimTarget,
    field: ValueField,
    anchor: Pos2,
    angle: bool,
}

/// What the cursor is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hover {
    Entity(EntityId),
    Dimension(ConstraintId),
}

/// The colours sketch mode draws with.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Fully constrained geometry.
    pub fully: Color32,
    /// Geometry that can still move.
    pub under: Color32,
    /// Geometry in a conflict.
    pub over: Color32,
    /// Selection.
    pub selected: Color32,
    /// Hover, previews and snaps.
    pub hover: Color32,
    /// Dimension labels.
    pub text: Color32,
    /// Label background.
    pub label_bg: Color32,
}

/// Sketch mode state.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchMode {
    /// The sketch feature being edited.
    pub feature: FeatureId,
    /// Active tool.
    pub tool: Tool,
    /// New geometry is construction geometry.
    pub construction: bool,
    /// Selected entities (Select tool).
    pub selected: BTreeSet<EntityId>,
    /// Selected dimension (Select tool).
    pub selected_dimension: Option<ConstraintId>,
    clicks: Vec<Snap>,
    hover: Option<Hover>,
    drag: Option<(EntityId, Sketch)>,
    dim_first: Option<EntityId>,
    edit: Option<DimEdit>,
}

/// What sketch mode asks the app to do this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchOutput {
    /// Commands to run, in order.
    pub commands: Vec<(&'static str, Value)>,
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Builds one `sketch.add` call, predicting the ids the engine will hand out.
struct Add {
    first_id: u32,
    entities: Vec<Value>,
    constraints: Vec<Value>,
    construction: bool,
}

impl Add {
    fn new(sketch: &Sketch, construction: bool) -> Self {
        Self {
            first_id: sketch.next_id,
            entities: Vec::new(),
            constraints: Vec::new(),
            construction,
        }
    }

    fn entity(&mut self, mut e: Value) -> u32 {
        if self.construction
            && e.get("kind") != Some(&json!("point"))
            && let Some(obj) = e.as_object_mut()
        {
            obj.insert("construction".into(), json!(true));
        }
        let id = self.first_id + self.entities.len() as u32;
        self.entities.push(e);
        id
    }

    fn point(&mut self, s: &Snap) -> u32 {
        if let Some(p) = s.point {
            return p.0;
        }
        let id = self.entity(json!({ "kind": "point", "x": s.at[0], "y": s.at[1] }));
        if s.origin {
            self.constraints
                .push(json!({ "kind": "fix", "point": id, "at": [0.0, 0.0] }));
        }
        id
    }

    fn line(&mut self, a: &Snap, b: &Snap) -> u32 {
        let (pa, pb) = (self.point(a), self.point(b));
        let line = self.entity(json!({ "kind": "line", "a": pa, "b": pb }));
        let (dx, dy) = (b.at[0] - a.at[0], b.at[1] - a.at[1]);
        let angle = dy.atan2(dx).abs();
        if angle < INFER_RAD || (std::f64::consts::PI - angle) < INFER_RAD {
            self.constraints
                .push(json!({ "kind": "horizontal", "line": line }));
        } else if (angle - std::f64::consts::FRAC_PI_2).abs() < INFER_RAD {
            self.constraints
                .push(json!({ "kind": "vertical", "line": line }));
        }
        line
    }

    fn finish(self, feature: FeatureId) -> Option<(&'static str, Value)> {
        (!self.entities.is_empty() || !self.constraints.is_empty()).then(|| {
            (
                "sketch.add",
                json!({
                    "sketch": feature.to_string(),
                    "first_id": self.first_id,
                    "entities": self.entities,
                    "constraints": self.constraints,
                }),
            )
        })
    }
}

/// Polyline of a curve, in sketch coordinates.
fn curve_points(sketch: &Sketch, e: &Entity) -> Vec<[f64; 2]> {
    match *e {
        Entity::Point { x, y } => vec![[x, y]],
        Entity::Line { a, b } => [sketch.point(a), sketch.point(b)]
            .into_iter()
            .flatten()
            .collect(),
        Entity::Circle { center, radius } => sketch.point(center).map_or_else(
            |_| Vec::new(),
            |c| {
                (0..=64)
                    .map(|i| {
                        let t = std::f64::consts::TAU * f64::from(i) / 64.0;
                        [c[0] + radius * t.cos(), c[1] + radius * t.sin()]
                    })
                    .collect()
            },
        ),
        Entity::Arc { center, start, end } => {
            let (Ok(c), Ok(s), Ok(e)) =
                (sketch.point(center), sketch.point(start), sketch.point(end))
            else {
                return Vec::new();
            };
            let r = dist(c, s);
            let a0 = (s[1] - c[1]).atan2(s[0] - c[0]);
            let a1 = (e[1] - c[1]).atan2(e[0] - c[0]);
            let mut sweep = (a1 - a0).rem_euclid(std::f64::consts::TAU);
            if sweep <= 0.0 {
                sweep = std::f64::consts::TAU;
            }
            (0..=48)
                .map(|i| {
                    let t = a0 + sweep * f64::from(i) / 48.0;
                    [c[0] + r * t.cos(), c[1] + r * t.sin()]
                })
                .collect()
        }
    }
}

fn segment_distance(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 {
        ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + ab * t).distance(p)
}

/// Where a dimension's label goes, in sketch coordinates, and its text.
fn dimension_label(sketch: &Sketch, c: &Constraint, reference: bool) -> Option<([f64; 2], String)> {
    use Constraint as C;
    let mid = |a: EntityId, b: EntityId| -> Option<[f64; 2]> {
        let (p, q) = (sketch.point(a).ok()?, sketch.point(b).ok()?);
        Some([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0])
    };
    let (at, text) = match *c {
        C::Length { line, value } => match sketch.entity(line).ok()? {
            Entity::Line { a, b } => (mid(*a, *b)?, format_mm(value)),
            _ => return None,
        },
        C::Distance { a, b, value } => (mid(a, b)?, format_mm(value)),
        // Horizontal labels sit at the first point's height, vertical ones at its u,
        // so a pair of them on the same points does not overlap.
        C::HorizontalDistance { a, b, value } => {
            let m = mid(a, b)?;
            (
                [m[0], sketch.point(a).ok()?[1]],
                format!("↔ {}", format_mm(value)),
            )
        }
        C::VerticalDistance { a, b, value } => {
            let m = mid(a, b)?;
            (
                [sketch.point(a).ok()?[0], m[1]],
                format!("↕ {}", format_mm(value)),
            )
        }
        C::Radius { circle, value } | C::Diameter { circle, value } => {
            let (center, r) = match *sketch.entity(circle).ok()? {
                Entity::Circle { center, radius } => (sketch.point(center).ok()?, radius),
                Entity::Arc { center, .. } => {
                    (sketch.point(center).ok()?, sketch.radius(circle).ok()?)
                }
                _ => return None,
            };
            let at = [center[0] + r * 0.707, center[1] + r * 0.707];
            let prefix = if matches!(c, C::Radius { .. }) {
                "R"
            } else {
                "⌀"
            };
            (
                at,
                format!("{prefix}{}", format_mm(value).trim_end_matches(" mm")),
            )
        }
        C::Angle { a, value, .. } => match sketch.entity(a).ok()? {
            Entity::Line { a: p, .. } => (
                sketch.point(*p).ok()?,
                format!("{:.1}°", value.to_degrees()),
            ),
            _ => return None,
        },
        _ => return None,
    };
    Some((at, if reference { format!("({text})") } else { text }))
}

impl SketchMode {
    /// Sketch mode on `feature` with the line tool.
    pub fn new(feature: FeatureId) -> Self {
        Self {
            feature,
            tool: Tool::Line,
            construction: false,
            selected: BTreeSet::new(),
            selected_dimension: None,
            clicks: Vec::new(),
            hover: None,
            drag: None,
            dim_first: None,
            edit: None,
        }
    }

    /// Switches tool and drops anything half-drawn.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.clicks.clear();
        self.dim_first = None;
        self.edit = None;
    }

    /// Whether a tool is mid-gesture (Esc cancels it rather than leaving the sketch).
    pub fn busy(&self) -> bool {
        !self.clicks.is_empty()
            || self.dim_first.is_some()
            || self.edit.is_some()
            || self.drag.is_some()
    }

    /// Esc: cancel the gesture.
    pub fn cancel(&mut self) {
        self.clicks.clear();
        self.dim_first = None;
        self.edit = None;
        self.drag = None;
    }

    /// The existing point near `pos`, if any.
    fn point_near(
        &self,
        view: &ViewInfo,
        state: &SketchState,
        sketch: &Sketch,
        pos: Pos2,
    ) -> Option<EntityId> {
        sketch
            .entities
            .iter()
            .filter(|(_, r)| matches!(r.entity, Entity::Point { .. }))
            .filter_map(|(id, _)| {
                let p = sketch.point(*id).ok()?;
                let s = view.screen_of(state.frame.to_world(p))?;
                let d = s.distance(pos);
                (d <= SNAP_PT).then_some((*id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    fn snap(&self, view: &ViewInfo, state: &SketchState, pos: Pos2) -> Option<Snap> {
        if let Some(id) = self.point_near(view, state, &state.sketch, pos) {
            let at = state.sketch.point(id).ok()?;
            return Some(Snap {
                at,
                point: Some(id),
                origin: false,
            });
        }
        let origin = view.screen_of(state.frame.to_world([0.0, 0.0]));
        if origin.is_some_and(|o| o.distance(pos) <= SNAP_PT) {
            return Some(Snap {
                at: [0.0, 0.0],
                point: None,
                origin: true,
            });
        }
        let raw = view.plane_point(&state.frame, pos)?;
        let step = handle::snap_step(&view.camera);
        Some(Snap {
            at: raw.map(|c| handle::snap(c, step)),
            point: None,
            origin: false,
        })
    }

    fn hover_at(&self, view: &ViewInfo, state: &SketchState, pos: Pos2) -> Option<Hover> {
        let sketch = &state.sketch;
        for (id, c) in &sketch.constraints {
            if let Some((at, _)) = dimension_label(sketch, &c.constraint, c.reference)
                && view
                    .screen_of(state.frame.to_world(at))
                    .is_some_and(|s| s.distance(pos) <= 16.0)
            {
                return Some(Hover::Dimension(*id));
            }
        }
        if let Some(p) = self.point_near(view, state, sketch, pos) {
            return Some(Hover::Entity(p));
        }
        sketch
            .entities
            .iter()
            .filter(|(_, r)| !matches!(r.entity, Entity::Point { .. }))
            .filter_map(|(id, r)| {
                let pts: Vec<Pos2> = curve_points(sketch, &r.entity)
                    .into_iter()
                    .filter_map(|p| view.screen_of(state.frame.to_world(p)))
                    .collect();
                let d = pts.windows(2).filter_map(|w| match w {
                    [a, b] => Some(segment_distance(*a, *b, pos)),
                    _ => None,
                });
                let d = d.fold(f32::INFINITY, f32::min);
                (d <= HIT_PT).then_some((*id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| Hover::Entity(id))
    }

    /// Handles input over the viewport. `response` is the viewport's.
    pub fn input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        view: &ViewInfo,
        state: &SketchState,
    ) -> SketchOutput {
        let mut out = SketchOutput::default();
        let pointer = response.hover_pos();
        self.hover = match (pointer, self.drag.is_some()) {
            (Some(p), false) => self.hover_at(view, state, p),
            _ => None,
        };
        let shift = ui.input(|i| i.modifiers.shift);
        let clicked = response.clicked_by(egui::PointerButton::Primary);
        let double = response.double_clicked();
        let sketch = &state.sketch;
        let f = self.feature;

        // Dragging points with the select tool: local preview, one command on release.
        if self.tool == Tool::Select {
            if response.drag_started_by(egui::PointerButton::Primary)
                && let Some(p) = response.interact_pointer_pos()
                && let Some(id) = self.point_near(view, state, sketch, p)
            {
                self.drag = Some((id, sketch.clone()));
            }
            if let Some((id, preview)) = &mut self.drag {
                if let Some(p) = ui.input(|i| i.pointer.latest_pos())
                    && let Some(at) = view.plane_point(&state.frame, p)
                {
                    *preview = sketch.clone();
                    let _ = kubit_sketch::drag(preview, *id, at, &SolveOptions::default());
                }
                if !ui.input(|i| i.pointer.primary_down()) {
                    if let Ok(to) = preview.point(*id) {
                        out.commands.push((
                            "sketch.move_point",
                            json!({ "sketch": f.to_string(), "point": id.0, "to": to }),
                        ));
                    }
                    self.drag = None;
                }
                return out;
            }
        }

        if double && self.tool == Tool::Line {
            self.clicks.clear();
        }
        if !clicked {
            return out;
        }
        let Some(pos) = pointer else { return out };
        match self.tool {
            Tool::Select => match self.hover {
                Some(Hover::Entity(id)) => {
                    self.selected_dimension = None;
                    if !shift {
                        self.selected.clear();
                    }
                    if !self.selected.remove(&id) {
                        self.selected.insert(id);
                    }
                }
                Some(Hover::Dimension(c)) => {
                    self.selected.clear();
                    self.selected_dimension = Some(c);
                    self.open_existing(state, c, pos);
                }
                None => {
                    self.selected.clear();
                    self.selected_dimension = None;
                }
            },
            Tool::Line => {
                let Some(s) = self.snap(view, state, pos) else {
                    return out;
                };
                if let Some(first) = self.clicks.first().copied() {
                    if dist(first.at, s.at) < 1e-6 {
                        return out;
                    }
                    let mut add = Add::new(sketch, self.construction);
                    add.line(&first, &s);
                    let end = s
                        .point
                        .unwrap_or(EntityId(add.first_id + add.entities.len() as u32 - 2));
                    let closes = s.point.is_some();
                    if let Some(c) = add.finish(f) {
                        out.commands.push(c);
                    }
                    self.clicks.clear();
                    if !closes {
                        self.clicks.push(Snap {
                            at: s.at,
                            point: Some(end),
                            origin: false,
                        });
                    }
                } else {
                    self.clicks.push(s);
                }
            }
            Tool::Rectangle => {
                let Some(s) = self.snap(view, state, pos) else {
                    return out;
                };
                if let Some(a) = self.clicks.first().copied() {
                    if (a.at[0] - s.at[0]).abs() < 1e-6 || (a.at[1] - s.at[1]).abs() < 1e-6 {
                        return out;
                    }
                    let mut add = Add::new(sketch, self.construction);
                    let b = Snap {
                        at: [s.at[0], a.at[1]],
                        point: None,
                        origin: false,
                    };
                    let d = Snap {
                        at: [a.at[0], s.at[1]],
                        point: None,
                        origin: false,
                    };
                    let (pa, pb, pc, pd) =
                        (add.point(&a), add.point(&b), add.point(&s), add.point(&d));
                    for (p, q, kind) in [
                        (pa, pb, "horizontal"),
                        (pb, pc, "vertical"),
                        (pc, pd, "horizontal"),
                        (pd, pa, "vertical"),
                    ] {
                        let line = add.entity(json!({ "kind": "line", "a": p, "b": q }));
                        add.constraints.push(json!({ "kind": kind, "line": line }));
                    }
                    if let Some(c) = add.finish(f) {
                        out.commands.push(c);
                    }
                    self.clicks.clear();
                } else {
                    self.clicks.push(s);
                }
            }
            Tool::Circle => {
                let Some(s) = self.snap(view, state, pos) else {
                    return out;
                };
                if let Some(c) = self.clicks.first().copied() {
                    let r = dist(c.at, s.at);
                    if r > 1e-6 {
                        let mut add = Add::new(sketch, self.construction);
                        let center = add.point(&c);
                        add.entity(json!({ "kind": "circle", "center": center, "radius": r }));
                        if let Some(cmd) = add.finish(f) {
                            out.commands.push(cmd);
                        }
                    }
                    self.clicks.clear();
                } else {
                    self.clicks.push(s);
                }
            }
            Tool::Arc => {
                let Some(s) = self.snap(view, state, pos) else {
                    return out;
                };
                self.clicks.push(s);
                if let [c, a, b] = self.clicks.as_slice() {
                    let r = dist(c.at, a.at);
                    let ang = (b.at[1] - c.at[1]).atan2(b.at[0] - c.at[0]);
                    let end = if b.point.is_some() {
                        *b
                    } else {
                        Snap {
                            at: [c.at[0] + r * ang.cos(), c.at[1] + r * ang.sin()],
                            point: None,
                            origin: false,
                        }
                    };
                    if r > 1e-6 && dist(a.at, end.at) > 1e-6 {
                        let mut add = Add::new(sketch, self.construction);
                        let (pc, pa, pb) = (add.point(c), add.point(a), add.point(&end));
                        add.entity(json!({ "kind": "arc", "center": pc, "start": pa, "end": pb }));
                        if let Some(cmd) = add.finish(f) {
                            out.commands.push(cmd);
                        }
                    }
                    self.clicks.clear();
                }
            }
            Tool::Dimension => self.dimension_click(state, pos),
        }
        out
    }

    fn open_existing(&mut self, state: &SketchState, c: ConstraintId, pos: Pos2) {
        let Some(record) = state.sketch.constraints.get(&c) else {
            return;
        };
        let Some(value) = record.constraint.value() else {
            return;
        };
        let angle = matches!(record.constraint, Constraint::Angle { .. });
        self.edit = Some(DimEdit {
            target: DimTarget::Existing(c),
            field: ValueField::new(if angle { value.to_degrees() } else { value }),
            anchor: pos,
            angle,
        });
    }

    fn dimension_click(&mut self, state: &SketchState, pos: Pos2) {
        let sketch = &state.sketch;
        match self.hover {
            Some(Hover::Dimension(c)) => self.open_existing(state, c, pos),
            Some(Hover::Entity(id)) => {
                let new = match (sketch.entity(id), self.dim_first) {
                    (Ok(Entity::Point { .. }), Some(first)) if first != id => {
                        self.dim_first = None;
                        let d = dist(
                            sketch.point(first).unwrap_or_default(),
                            sketch.point(id).unwrap_or_default(),
                        );
                        Some((
                            Constraint::Distance {
                                a: first,
                                b: id,
                                value: d,
                            },
                            d,
                        ))
                    }
                    (Ok(Entity::Point { .. }), _) => {
                        self.dim_first = Some(id);
                        None
                    }
                    (Ok(Entity::Line { a, b }), _) => {
                        let d = dist(
                            sketch.point(*a).unwrap_or_default(),
                            sketch.point(*b).unwrap_or_default(),
                        );
                        Some((Constraint::Length { line: id, value: d }, d))
                    }
                    (Ok(Entity::Circle { .. } | Entity::Arc { .. }), _) => {
                        let r = sketch.radius(id).unwrap_or(1.0);
                        Some((
                            Constraint::Radius {
                                circle: id,
                                value: r,
                            },
                            r,
                        ))
                    }
                    _ => None,
                };
                if let Some((c, v)) = new {
                    self.edit = Some(DimEdit {
                        target: DimTarget::New(c),
                        field: ValueField::new(v),
                        anchor: pos,
                        angle: false,
                    });
                }
            }
            None => self.dim_first = None,
        }
    }

    /// The dimension field, if open; returns the command it produced.
    pub fn dimension_field(
        &mut self,
        ctx: &egui::Context,
        sketch: &Sketch,
        error: Color32,
        messages: [&str; 2],
    ) -> Option<(&'static str, Value)> {
        let mut edit = self.edit.take()?;
        let action = egui::Area::new(egui::Id::new("kubit-sketch-dimension"))
            .order(egui::Order::Foreground)
            .fixed_pos(edit.anchor + egui::vec2(14.0, 14.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| edit.field.show(ui, error, messages))
                    .inner
            })
            .inner;
        let f = self.feature.to_string();
        match action {
            FieldAction::None => {
                self.edit = Some(edit);
                None
            }
            FieldAction::Cancel => None,
            FieldAction::Apply(v) => {
                let value = if edit.angle { v.to_radians() } else { v };
                match edit.target {
                    DimTarget::Existing(c) => Some((
                        "sketch.set_dimension",
                        json!({ "sketch": f, "constraint": c.0, "value": value }),
                    )),
                    DimTarget::New(mut c) => {
                        c.set_value(value);
                        let constraint = serde_json::to_value(kubit_sketch::ConstraintRecord {
                            constraint: c,
                            reference: false,
                        })
                        .ok()?;
                        Some((
                            "sketch.add",
                            json!({ "sketch": f, "first_id": sketch.next_id, "constraints": [constraint] }),
                        ))
                    }
                }
            }
        }
    }

    /// The `sketch.add` that applies `kind` to the selection, or `None` when the
    /// selection does not fit it (the ribbon disables the button).
    pub fn constraint_command(
        &self,
        kind: ConstraintKind,
        sketch: &Sketch,
    ) -> Option<(&'static str, Value)> {
        let of = |pred: fn(&Entity) -> bool| -> Vec<u32> {
            self.selected
                .iter()
                .filter(|id| sketch.entity(**id).is_ok_and(pred))
                .map(|id| id.0)
                .collect()
        };
        let points = of(|e| matches!(e, Entity::Point { .. }));
        let lines = of(|e| matches!(e, Entity::Line { .. }));
        let rounds = of(|e| matches!(e, Entity::Circle { .. } | Entity::Arc { .. }));
        let n = self.selected.len();
        let pair = |a: &[u32], b: &[u32]| match (a, b) {
            ([x], [y]) => Some((*x, *y)),
            _ => None,
        };
        let constraints: Vec<Value> = match kind {
            ConstraintKind::Horizontal | ConstraintKind::Vertical
                if !lines.is_empty() && lines.len() == n =>
            {
                let k = if kind == ConstraintKind::Horizontal {
                    "horizontal"
                } else {
                    "vertical"
                };
                lines
                    .iter()
                    .map(|l| json!({ "kind": k, "line": l }))
                    .collect()
            }
            ConstraintKind::Parallel | ConstraintKind::Perpendicular
                if lines.len() == 2 && n == 2 =>
            {
                let k = if kind == ConstraintKind::Parallel {
                    "parallel"
                } else {
                    "perpendicular"
                };
                let [a, b] = lines.as_slice() else {
                    return None;
                };
                vec![json!({ "kind": k, "a": a, "b": b })]
            }
            ConstraintKind::Tangent
                if n == 2 && !rounds.is_empty() && lines.len() + rounds.len() == 2 =>
            {
                let all: Vec<u32> = lines.iter().chain(&rounds).copied().collect();
                let [a, b] = all.as_slice() else { return None };
                vec![json!({ "kind": "tangent", "a": a, "b": b })]
            }
            ConstraintKind::Equal if n == 2 && (lines.len() == 2 || rounds.len() == 2) => {
                let all: Vec<u32> = lines.iter().chain(&rounds).copied().collect();
                let [a, b] = all.as_slice() else { return None };
                vec![json!({ "kind": "equal", "a": a, "b": b })]
            }
            ConstraintKind::Coincident if n == 2 => {
                if let [a, b] = points.as_slice() {
                    vec![json!({ "kind": "coincident", "a": a, "b": b })]
                } else if let Some((p, l)) = pair(&points, &lines) {
                    vec![json!({ "kind": "point_on_line", "point": p, "line": l })]
                } else if let Some((p, c)) = pair(&points, &rounds) {
                    vec![json!({ "kind": "point_on_circle", "point": p, "circle": c })]
                } else {
                    return None;
                }
            }
            ConstraintKind::Midpoint if n == 2 => {
                let (p, l) = pair(&points, &lines)?;
                vec![json!({ "kind": "midpoint", "point": p, "line": l })]
            }
            ConstraintKind::Fix if !points.is_empty() && points.len() == n => points
                .iter()
                .filter_map(|p| {
                    let at = sketch.point(EntityId(*p)).ok()?;
                    Some(json!({ "kind": "fix", "point": p, "at": at }))
                })
                .collect(),
            _ => return None,
        };
        Some((
            "sketch.add",
            json!({ "sketch": self.feature.to_string(), "first_id": sketch.next_id, "constraints": constraints }),
        ))
    }

    /// Delete key: removes the selection.
    pub fn delete_selection(&mut self) -> Option<(&'static str, Value)> {
        if self.selected.is_empty() && self.selected_dimension.is_none() {
            return None;
        }
        let entities: Vec<u32> = self.selected.iter().map(|e| e.0).collect();
        let constraints: Vec<u32> = self.selected_dimension.iter().map(|c| c.0).collect();
        self.selected.clear();
        self.selected_dimension = None;
        Some((
            "sketch.delete",
            json!({ "sketch": self.feature.to_string(), "entities": entities, "constraints": constraints }),
        ))
    }

    /// Draws the sketch being edited, previews and snaps.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        view: &ViewInfo,
        state: &SketchState,
        colors: &Palette,
        pointer: Option<Pos2>,
    ) {
        let sketch = self.drag.as_ref().map_or(&state.sketch, |(_, s)| s);
        paint_sketch(painter, view, state, sketch, colors, Some(self));
        let to = |p: [f64; 2]| view.screen_of(state.frame.to_world(p));
        let Some(pos) = pointer else { return };
        let snap = self.snap(view, state, pos);
        if let Some(s) = snap
            && self.tool != Tool::Select
            && self.tool != Tool::Dimension
            && let Some(at) = to(s.at)
        {
            let r = if s.point.is_some() || s.origin {
                5.0
            } else {
                3.0
            };
            painter.circle_stroke(at, r, Stroke::new(1.5, colors.hover));
        }
        let Some(s) = snap else { return };
        let stroke = Stroke::new(1.5, colors.hover);
        match (self.tool, self.clicks.as_slice()) {
            (Tool::Line, [a]) => {
                if let (Some(p), Some(q)) = (to(a.at), to(s.at)) {
                    painter.line_segment([p, q], stroke);
                    let (dx, dy) = (s.at[0] - a.at[0], s.at[1] - a.at[1]);
                    let angle = dy.atan2(dx).abs();
                    let tag = if angle < INFER_RAD || (std::f64::consts::PI - angle) < INFER_RAD {
                        "H"
                    } else if (angle - std::f64::consts::FRAC_PI_2).abs() < INFER_RAD {
                        "V"
                    } else {
                        ""
                    };
                    painter.text(
                        q + egui::vec2(10.0, -10.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{tag} {}", format_mm(dist(a.at, s.at))),
                        egui::FontId::monospace(12.0),
                        colors.hover,
                    );
                }
            }
            (Tool::Rectangle, [a]) => {
                let corners = [a.at, [s.at[0], a.at[1]], s.at, [a.at[0], s.at[1]], a.at];
                let pts: Vec<Pos2> = corners.iter().filter_map(|c| to(*c)).collect();
                painter.add(egui::Shape::line(pts, stroke));
            }
            (Tool::Circle, [c]) => {
                let r = dist(c.at, s.at);
                let pts: Vec<Pos2> = (0..=64)
                    .filter_map(|i| {
                        let t = std::f64::consts::TAU * f64::from(i) / 64.0;
                        to([c.at[0] + r * t.cos(), c.at[1] + r * t.sin()])
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, stroke));
            }
            (Tool::Arc, [c, ..]) => {
                if let (Some(p), Some(q)) = (to(c.at), to(s.at)) {
                    painter.line_segment([p, q], Stroke::new(1.0, colors.hover));
                }
            }
            _ => {}
        }
    }
}

/// Draws a sketch: curves coloured by freedom, points, and dimension labels.
/// `mode` adds selection and hover.
pub fn paint_sketch(
    painter: &egui::Painter,
    view: &ViewInfo,
    state: &SketchState,
    sketch: &Sketch,
    colors: &Palette,
    mode: Option<&SketchMode>,
) {
    let to = |p: [f64; 2]| view.screen_of(state.frame.to_world(p));
    let editing = mode.is_some();
    for (id, r) in &sketch.entities {
        let freedom = state
            .report
            .entities
            .get(id)
            .copied()
            .unwrap_or(Freedom::Under(1));
        let mut color = match freedom {
            Freedom::Fully => colors.fully,
            Freedom::Under(_) => colors.under,
            Freedom::Over => colors.over,
        };
        let mut width = if editing { 2.0 } else { 1.5 };
        if let Some(m) = mode {
            if m.selected.contains(id) {
                color = colors.selected;
                width = 3.0;
            } else if m.hover == Some(Hover::Entity(*id)) {
                color = colors.hover;
                width = 2.5;
            }
        }
        if r.construction {
            color = color.gamma_multiply(0.55);
            width = 1.0;
        }
        if !editing {
            color = color.gamma_multiply(0.8);
        }
        match r.entity {
            Entity::Point { x, y } => {
                if editing && let Some(p) = to([x, y]) {
                    painter.circle_filled(p, if width > 2.0 { 4.0 } else { 2.5 }, color);
                }
            }
            ref e => {
                let pts: Vec<Pos2> = curve_points(sketch, e).into_iter().filter_map(to).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(width, color)));
            }
        }
    }
    if !editing {
        return;
    }
    for (id, c) in &sketch.constraints {
        let Some((at, text)) = dimension_label(sketch, &c.constraint, c.reference) else {
            continue;
        };
        let Some(p) = to(at) else { continue };
        let lit = mode.is_some_and(|m| {
            m.hover == Some(Hover::Dimension(*id)) || m.selected_dimension == Some(*id)
        });
        let conflict = state.report.conflicting.contains(id);
        let color = if conflict {
            colors.over
        } else if lit {
            colors.hover
        } else {
            colors.text
        };
        let galley = painter.layout_no_wrap(text, egui::FontId::monospace(12.0), color);
        let rect = Rect::from_center_size(p, galley.size() + egui::vec2(8.0, 4.0));
        painter.rect_filled(rect, 3.0, colors.label_bg);
        painter.galley(rect.min + egui::vec2(4.0, 2.0), galley, color);
    }
}

/// World bounds of a sketch's geometry, padded; `None` when it is empty.
pub fn bounds(state: &SketchState) -> Option<kubit_mesh::Aabb> {
    let sketch = &state.sketch;
    let pts: Vec<[f64; 3]> = sketch
        .entities
        .values()
        .flat_map(|r| curve_points(sketch, &r.entity))
        .map(|p| state.frame.to_world(p))
        .collect();
    let first = *pts.first()?;
    let (mut min, mut max) = (first, first);
    for p in &pts {
        for ((lo, hi), v) in min.iter_mut().zip(max.iter_mut()).zip(p) {
            *lo = lo.min(*v);
            *hi = hi.max(*v);
        }
    }
    let mut b = kubit_mesh::Aabb { min, max };
    let pad = 0.15 * b.diagonal().max(10.0);
    b.min = b.min.map(|v| v - pad);
    b.max = b.max.map(|v| v + pad);
    Some(b)
}

/// The region of a sketch under `pos`.
pub fn region_at(view: &ViewInfo, state: &SketchState, pos: Pos2) -> Option<Region> {
    let p = view.plane_point(&state.frame, pos)?;
    state.regions().into_iter().find(|r| r.contains(p))
}

/// Outlines a region (hover or selection).
pub fn paint_region(
    painter: &egui::Painter,
    view: &ViewInfo,
    state: &SketchState,
    region: &Region,
    color: Color32,
) {
    let to = |p: [f64; 2]| view.screen_of(state.frame.to_world(p));
    for l in std::iter::once(&region.outer).chain(&region.holes) {
        let mut pts: Vec<Pos2> = l.polygon().into_iter().filter_map(to).collect();
        if let Some(first) = pts.first().copied() {
            pts.push(first);
        }
        painter.add(egui::Shape::line(pts, Stroke::new(3.0, color)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kubit_render::StandardView;

    fn view(v: StandardView) -> ViewInfo {
        let mut camera = Camera::default();
        camera.set_view(v);
        camera.target = [0.0; 3];
        ViewInfo {
            camera,
            size: Viewport::new(800, 600),
            rect: Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0)),
            ppp: 1.0,
        }
    }

    #[test]
    fn plane_point_inverts_to_screen() {
        let v = view(StandardView::Iso);
        let frame = PlaneFrame {
            origin: [0.0, 0.0, 5.0],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let w = frame.to_world([12.0, -7.0]);
        let s = v.screen_of(w).unwrap();
        let back = v.plane_point(&frame, s).unwrap();
        // Screen positions are f32: the round trip is good to about 1e-5 mm here.
        assert!(
            (back[0] - 12.0).abs() < 1e-3 && (back[1] + 7.0).abs() < 1e-3,
            "{back:?}"
        );
    }

    #[test]
    fn facing_matches_standard_views() {
        let (yaw, pitch) = facing([0.0, -1.0, 0.0]);
        let a = Camera {
            yaw: yaw.rem_euclid(std::f64::consts::TAU),
            pitch,
            ..Camera::default()
        };
        let mut b = Camera::default();
        b.set_view(StandardView::Front);
        assert!((a.yaw - b.yaw).abs() < 1e-12 && (a.pitch - b.pitch).abs() < 1e-12);
        assert!((facing([0.0, 0.0, 1.0]).1 - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn add_predicts_ids_and_infers_constraints() {
        let mut s = Sketch::new();
        s.add_point(0.0, 0.0).unwrap();
        let mut add = Add::new(&s, false);
        let a = Snap {
            at: [0.0, 0.0],
            point: Some(EntityId(0)),
            origin: false,
        };
        let b = Snap {
            at: [10.0, 0.0],
            point: None,
            origin: false,
        };
        let line = add.line(&a, &b);
        assert_eq!(line, 2, "the new point is 1, the line 2");
        assert_eq!(
            add.constraints,
            vec![json!({ "kind": "horizontal", "line": 2 })]
        );
        let (_, params) = add.finish(FeatureId(4)).unwrap();
        assert_eq!(params["first_id"], 1);
    }
}
