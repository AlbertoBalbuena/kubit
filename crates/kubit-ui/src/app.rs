//! The eframe application. Layout (top to bottom): the top bar (logo menu, undo,
//! command search), the ribbon (the one home of modelling commands), the canvas
//! between the collapsible Navigator and Properties panels, and the history bar.
//! Next to the model there are only values (inline fields, drag arrows).
//!
//! Every change to the document goes through `Session::execute`. Commands never
//! open dialogs; the UI opens file dialogs first and passes the chosen path as a
//! parameter.

mod chrome;
mod history;
mod panels;
mod ribbon;

use crate::i18n::{I18n, Lang};
use crate::icons;
use crate::sketch_ui::{self, SketchMode, Tool, ViewInfo};
use crate::theme::{Theme, ThemeMode};
use crate::value_input::{FieldAction, ValueField};
use crate::viewport::{Hit, Viewport3d, body_of};
use eframe::egui_wgpu;
use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};
use kubit_doc::FeatureId;
use kubit_engine::{CommandSpec, Enablement, Event, KernelFactory, Session};
use serde_json::{Value, json};

/// One step of a startup script (same format as `kubit-cli run`).
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptStep {
    /// Command id.
    pub command: String,
    /// JSON parameters.
    pub params: Value,
}

/// How the app starts.
#[derive(Clone, Debug, Default)]
pub struct AppOptions {
    /// UI language; `None` uses the system locale.
    pub lang: Option<Lang>,
    /// Light or dark.
    pub theme: ThemeMode,
    /// Commands run before the first frame.
    pub startup: Vec<ScriptStep>,
    /// Open this sketch feature (e.g. `F1`) in sketch mode after the startup commands.
    pub edit_sketch: Option<String>,
    /// Start with the Navigator and Properties panels collapsed.
    pub collapsed: bool,
}

/// Which multi-field parameter window is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Popover {
    Box,
    Cylinder,
    Boolean,
    SketchPlane,
}

/// Operations that take one length, typed inline next to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueOp {
    PushPull,
    Fillet,
    Chamfer,
    Shell,
    Extrude,
}

impl ValueOp {
    fn from_command(id: &str) -> Option<Self> {
        Some(match id {
            "face.push_pull" => Self::PushPull,
            "edge.fillet" => Self::Fillet,
            "edge.chamfer" => Self::Chamfer,
            "body.shell" => Self::Shell,
            "solid.extrude" => Self::Extrude,
            _ => return None,
        })
    }

    fn command(self) -> &'static str {
        match self {
            Self::PushPull => "face.push_pull",
            Self::Fillet => "edge.fillet",
            Self::Chamfer => "edge.chamfer",
            Self::Shell => "body.shell",
            Self::Extrude => "solid.extrude",
        }
    }

    /// i18n key of the value's label.
    fn label_key(self) -> &'static str {
        match self {
            Self::PushPull | Self::Chamfer | Self::Extrude => "ui.param.distance",
            Self::Fillet => "ui.param.radius",
            Self::Shell => "ui.param.thickness",
        }
    }

    /// Fillet and chamfer act on edges; push/pull and shell on faces.
    fn needs_edges(self) -> bool {
        matches!(self, Self::Fillet | Self::Chamfer)
    }
}

/// The inline value field of a [`ValueOp`] being entered.
#[derive(Clone, Debug, PartialEq)]
struct InlineValue {
    op: ValueOp,
    field: ValueField,
    anchor: egui::Pos2,
}

/// Boolean operation picked in the Combine window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

impl BoolOp {
    const ALL: [Self; 3] = [Self::Union, Self::Subtract, Self::Intersect];

    fn param(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::Union => "ui.bool.union",
            Self::Subtract => "ui.bool.subtract",
            Self::Intersect => "ui.bool.intersect",
        }
    }
}

/// Parameters edited in the windows and inline fields (kept between uses).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Forms {
    box_origin: [f64; 3],
    box_size: [f64; 3],
    cyl_origin: [f64; 3],
    cyl_radius: f64,
    cyl_height: f64,
    distance: f64,
    radius: f64,
    chamfer: f64,
    thickness: f64,
    extrude: f64,
    bool_op: BoolOp,
    bool_target: Option<FeatureId>,
    bool_tool: Option<FeatureId>,
}

impl Default for Forms {
    fn default() -> Self {
        Self {
            box_origin: [0.0; 3],
            box_size: [100.0, 60.0, 40.0],
            cyl_origin: [0.0; 3],
            cyl_radius: 20.0,
            cyl_height: 40.0,
            distance: 10.0,
            radius: 5.0,
            chamfer: 2.0,
            thickness: 2.0,
            extrude: 10.0,
            bool_op: BoolOp::Union,
            bool_target: None,
            bool_tool: None,
        }
    }
}

impl Forms {
    fn last(&self, op: ValueOp) -> f64 {
        match op {
            ValueOp::PushPull => self.distance,
            ValueOp::Fillet => self.radius,
            ValueOp::Chamfer => self.chamfer,
            ValueOp::Shell => self.thickness,
            ValueOp::Extrude => self.extrude,
        }
    }

    fn remember(&mut self, op: ValueOp, mm: f64) {
        match op {
            ValueOp::PushPull => self.distance = mm,
            ValueOp::Fillet => self.radius = mm,
            ValueOp::Chamfer => self.chamfer = mm,
            ValueOp::Shell => self.thickness = mm,
            ValueOp::Extrude => self.extrude = mm,
        }
    }
}

/// Last message shown in the status bar.
#[derive(Clone, Debug, PartialEq)]
enum Status {
    Ready,
    /// A command succeeded (shown through `ui.done.<command>`).
    Done(String),
    /// A command failed; `detail` is the engine's message.
    Failed {
        command: String,
        detail: String,
    },
    /// An i18n key.
    Error(String),
}

/// Largest accepted parameter magnitude in the popovers, mm.
const MAX_MM: f64 = 100_000.0;

/// The app state.
struct State {
    session: Session,
    i18n: I18n,
    theme: Theme,
    viewport: Option<Viewport3d>,
    status: Status,
    popover: Option<Popover>,
    inline: Option<InlineValue>,
    /// Where the right-click menu is open.
    menu_at: Option<egui::Pos2>,
    /// Pointer position at the start of the frame (anchors inline fields).
    pointer: Option<egui::Pos2>,
    /// The viewport's rectangle last frame (anchors inline fields).
    viewport_rect: Option<egui::Rect>,
    forms: Forms,
    /// Sketch being edited, if in sketch mode.
    sketch_mode: Option<SketchMode>,
    /// Selected sketch region `(sketch, region)` (for extrusion).
    region: Option<(FeatureId, u32)>,
    /// The viewport's camera and placement last frame.
    view: Option<ViewInfo>,
    /// Active ribbon tab (outside sketch mode).
    ribbon_tab: ribbon::RibbonTab,
    /// Navigator and Properties panels expanded.
    left_open: bool,
    right_open: bool,
    /// The command palette, when open.
    palette: Option<chrome::Palette>,
    /// Bodies hidden from the view (view state, not document).
    hidden_bodies: std::collections::BTreeSet<FeatureId>,
    /// Sketch visibility overrides (default: shown until extruded).
    sketch_visibility: std::collections::BTreeMap<FeatureId, bool>,
    /// Timeline feature shown in Properties.
    selected_feature: Option<FeatureId>,
    /// A Properties edit waiting for the pointer to be released.
    pending_edit: Option<(FeatureId, Value)>,
    /// Draw B-rep edges over the shading.
    show_edges: bool,
}

/// The Kubit desktop app.
pub struct KubitApp {
    state: State,
}

impl KubitApp {
    /// Builds the app inside eframe's creation callback.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        factory: KernelFactory,
        options: AppOptions,
    ) -> Self {
        Self::build(
            &cc.egui_ctx,
            cc.wgpu_render_state.as_ref(),
            factory,
            options,
        )
    }

    /// Builds the app on any egui context; without a wgpu render state there is no
    /// viewport (the status bar says so).
    pub fn build(
        ctx: &egui::Context,
        render_state: Option<&egui_wgpu::RenderState>,
        factory: KernelFactory,
        options: AppOptions,
    ) -> Self {
        let theme = Theme::new(options.theme);
        theme.apply(ctx);
        icons::install_fonts(ctx);
        let viewport = render_state.and_then(|rs| Viewport3d::new(rs).ok());
        let mut state = State {
            session: Session::new(factory),
            i18n: I18n::new(options.lang.unwrap_or_else(Lang::system)),
            theme,
            status: if viewport.is_some() {
                Status::Ready
            } else {
                Status::Error("ui.status.no_gpu".into())
            },
            viewport,
            popover: None,
            inline: None,
            menu_at: None,
            pointer: None,
            viewport_rect: None,
            forms: Forms::default(),
            sketch_mode: None,
            region: None,
            view: None,
            ribbon_tab: ribbon::RibbonTab::default(),
            left_open: !options.collapsed,
            right_open: !options.collapsed,
            palette: None,
            hidden_bodies: std::collections::BTreeSet::new(),
            sketch_visibility: std::collections::BTreeMap::new(),
            selected_feature: None,
            pending_edit: None,
            show_edges: true,
        };
        for step in options.startup {
            state.execute(&step.command, step.params);
        }
        if let Some(id) = options
            .edit_sketch
            .and_then(|t| t.parse::<FeatureId>().ok())
        {
            state.enter_sketch(id);
        }
        Self { state }
    }
}

impl State {
    /// Runs a command and refreshes the views from its events.
    /// Runs a command and refreshes the views; returns the outcome's data on success.
    fn execute(&mut self, id: &str, params: Value) -> Option<Value> {
        let (status, data) = match self.session.execute(id, params) {
            Ok(outcome) => (Status::Done(id.to_owned()), Some(outcome.data)),
            Err(e) => (
                Status::Failed {
                    command: id.to_owned(),
                    detail: e.to_string(),
                },
                None,
            ),
        };
        self.status = status;
        self.drain_events();
        // Undo can remove the sketch being edited.
        if let Some(m) = &self.sketch_mode
            && !self.session.model().sketches.contains_key(&m.feature)
        {
            self.exit_sketch();
        }
        if let Some((sketch, region)) = self.region
            && !self
                .session
                .model()
                .sketches
                .get(&sketch)
                .is_some_and(|st| st.regions().iter().any(|r| r.id.0 == region))
        {
            self.region = None;
        }
        data
    }

    /// Enters sketch mode on sketch feature `id`.
    fn enter_sketch(&mut self, id: FeatureId) {
        let Some(normal) = self
            .session
            .model()
            .sketches
            .get(&id)
            .map(|st| st.frame.normal)
        else {
            return;
        };
        self.inline = None;
        self.popover = None;
        self.region = None;
        if !self.session.selection().faces.is_empty() || !self.session.selection().edges.is_empty()
        {
            self.execute("selection.set", json!({ "faces": [], "edges": [] }));
        }
        self.sketch_mode = Some(SketchMode::new(id));
        let bounds = self
            .session
            .model()
            .sketches
            .get(&id)
            .and_then(sketch_ui::bounds);
        if let Some(vp) = &mut self.viewport {
            vp.pick_enabled = false;
            vp.look_along(normal);
            if let Some(b) = bounds {
                vp.fit_to(&b);
            }
        }
    }

    /// Leaves sketch mode.
    fn exit_sketch(&mut self) {
        self.sketch_mode = None;
        if let Some(vp) = &mut self.viewport {
            vp.pick_enabled = true;
        }
    }

    /// Creates a sketch on `plane` and enters it.
    fn new_sketch(&mut self, plane: Value) {
        let id = self
            .execute("sketch.create", json!({ "plane": plane }))
            .and_then(|d| d.get("feature").and_then(Value::as_str).map(str::to_owned))
            .and_then(|t| t.parse::<FeatureId>().ok());
        if let Some(id) = id {
            self.enter_sketch(id);
        }
    }

    /// Sketches whose regions have not been extruded yet (drawn in 3D).
    fn visible_sketches(&self) -> Vec<FeatureId> {
        let used: std::collections::BTreeSet<String> = self
            .session
            .doc()
            .features
            .iter()
            .filter(|f| f.command == "solid.extrude")
            .filter_map(|f| {
                f.params
                    .get("sketch")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        self.session
            .model()
            .sketches
            .keys()
            .copied()
            .filter(|id| !used.contains(&id.to_string()))
            .collect()
    }

    /// The body a sketch's face plane belongs to (extrusions join or cut it).
    fn sketch_body(&self, sketch: FeatureId) -> Option<FeatureId> {
        let f = self.session.doc().feature(sketch)?;
        let face = f.params.get("plane")?.get("face")?.as_str()?;
        let name: kubit_doc::TopoName = face.parse().ok()?;
        self.session.model().body_of_face(&name).map(|(id, _)| id)
    }

    fn drain_events(&mut self) {
        let events = self.session.take_events();
        let Some(vp) = self.viewport.as_mut() else {
            return;
        };
        if events.contains(&Event::DocumentChanged)
            && let Err(e) = vp.sync_meshes(&mut self.session)
        {
            self.status = Status::Error(e);
        }
        if events
            .iter()
            .any(|e| matches!(e, Event::DocumentChanged | Event::SelectionChanged))
        {
            vp.sync_selection(&self.session);
        }
    }

    fn tr<'a>(&self, key: &'a str) -> &'a str {
        self.i18n.tr(key)
    }

    /// The only selected face, as a persistent name.
    fn single_face(&self) -> Option<String> {
        let faces = &self.session.selection().faces;
        (faces.len() == 1)
            .then(|| faces.iter().next().map(ToString::to_string))
            .flatten()
    }

    fn open_popover(&mut self, which: Popover) {
        self.inline = None;
        self.popover = Some(which);
    }

    /// Selected face names.
    fn selected_faces(&self) -> Vec<String> {
        self.session
            .selection()
            .faces
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Selected edge names.
    fn selected_edges(&self) -> Vec<String> {
        self.session
            .selection()
            .edges
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Opens the inline value field of `op` next to the pointer, if the selection fits.
    fn start_value(&mut self, op: ValueOp) {
        let missing = if op == ValueOp::Extrude {
            self.region.is_none().then_some("ui.param.need_region")
        } else if op.needs_edges() {
            self.session
                .selection()
                .edges
                .is_empty()
                .then_some("ui.param.need_edge")
        } else {
            self.session
                .selection()
                .faces
                .is_empty()
                .then_some("ui.param.need_face")
        };
        if let Some(key) = missing {
            self.status = Status::Error(key.to_owned());
            return;
        }
        let rect = self.viewport_rect;
        let pointer = self.pointer.filter(|p| rect.is_none_or(|r| r.contains(*p)));
        let anchor = pointer
            .or(rect.map(|r| r.center()))
            .unwrap_or(egui::pos2(240.0, 160.0));
        self.popover = None;
        self.inline = Some(InlineValue {
            op,
            field: ValueField::new(self.forms.last(op)),
            anchor,
        });
    }

    /// Runs `op` on the selection with `mm`.
    fn apply_value(&mut self, op: ValueOp, mm: f64) {
        self.forms.remember(op, mm);
        let faces = self.selected_faces();
        let edges = self.selected_edges();
        let params = match op {
            ValueOp::PushPull => match faces.as_slice() {
                [one] => json!({ "face": one, "distance": mm }),
                _ => json!({ "faces": faces, "distance": mm }),
            },
            ValueOp::Fillet => json!({ "edges": edges, "radius": mm }),
            ValueOp::Chamfer => json!({ "edges": edges, "distance": mm }),
            ValueOp::Shell => json!({ "faces": faces, "thickness": mm }),
            ValueOp::Extrude => {
                let Some((sketch, region)) = self.region else {
                    return;
                };
                // On a body's face: out of it joins, into it cuts. Elsewhere: a new body.
                match self.sketch_body(sketch) {
                    Some(body) => json!({
                        "sketch": sketch.to_string(), "region": region, "distance": mm,
                        "op": if mm < 0.0 { "cut" } else { "join" }, "target": body.to_string(),
                    }),
                    None => json!({
                        "sketch": sketch.to_string(), "region": region, "distance": mm, "op": "new",
                    }),
                }
            }
        };
        if op == ValueOp::Extrude {
            self.region = None;
        }
        self.execute(op.command(), params);
    }

    /// The inline value field, if one is open.
    fn inline_value(&mut self, ctx: &egui::Context) {
        let Some(mut inline) = self.inline.take() else {
            return;
        };
        let title = format!(
            "{} · {}",
            self.tr(&format!("cmd.{}", inline.op.command())),
            self.tr(inline.op.label_key())
        );
        let messages = [self.tr("ui.value.invalid"), self.tr("ui.value.not_length")];
        let hint = self.tr("ui.value.hint").to_owned();
        let p = self.theme.palette;
        let action = egui::Area::new(egui::Id::new("kubit-inline-value"))
            .order(egui::Order::Foreground)
            .fixed_pos(inline.anchor + egui::vec2(16.0, 16.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.label(RichText::new(&title).strong());
                        let action = inline.field.show(ui, p.error, messages);
                        ui.label(RichText::new(&hint).small().color(p.text_weak));
                        action
                    })
                    .inner
            })
            .inner;
        match action {
            FieldAction::Apply(mm) => self.apply_value(inline.op, mm),
            FieldAction::Cancel => {}
            FieldAction::None => self.inline = Some(inline),
        }
    }

    /// The right-click menu, if open.
    fn context_menu(&mut self, ctx: &egui::Context) {
        let Some(at) = self.menu_at else {
            return;
        };
        // View and selection actions only: modelling commands live in the ribbon.
        let selected: std::collections::BTreeSet<FeatureId> = {
            let sel = self.session.selection();
            let model = self.session.model();
            sel.faces
                .iter()
                .filter_map(|f| model.body_of_face(f).map(|(b, _)| b))
                .chain(
                    sel.edges
                        .iter()
                        .filter_map(|e| model.body_of_edge(e).map(|(b, _)| b)),
                )
                .collect()
        };
        let i18n = self.i18n;
        let mut action = None;
        let area = egui::Area::new(egui::Id::new("kubit-context-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(160.0);
                    let some = !selected.is_empty();
                    for (key, enabled, which) in [
                        ("ui.menu.hide", some, 0),
                        ("ui.menu.isolate", some, 1),
                        ("ui.menu.show_all", !self.hidden_bodies.is_empty(), 2),
                        ("ui.view.fit", true, 3),
                        ("ui.menu.deselect", some || self.region.is_some(), 4),
                    ] {
                        if ui
                            .add_enabled(enabled, egui::Button::new(i18n.tr(key)))
                            .clicked()
                        {
                            action = Some(which);
                        }
                    }
                });
            });
        match action {
            Some(0) => {
                self.hidden_bodies.extend(selected);
                self.execute("selection.set", json!({ "faces": [], "edges": [] }));
                self.sync_hidden();
            }
            Some(1) => {
                let all: Vec<FeatureId> = self.session.model().bodies.keys().copied().collect();
                self.hidden_bodies = all.into_iter().filter(|b| !selected.contains(b)).collect();
                self.sync_hidden();
            }
            Some(2) => {
                self.hidden_bodies.clear();
                self.sync_hidden();
            }
            Some(3) => {
                if let Some(vp) = &mut self.viewport {
                    vp.fit();
                }
            }
            Some(4) => {
                self.region = None;
                self.execute("selection.set", json!({ "faces": [], "edges": [] }));
            }
            _ => {}
        }
        if action.is_some()
            || area.response.clicked_elsewhere()
            || ctx.input(|i| i.key_pressed(Key::Escape))
        {
            self.menu_at = None;
        }
    }

    /// Applies a Properties edit once the pointer is released.
    fn flush_edit(&mut self, ctx: &egui::Context) {
        if self.pending_edit.is_some()
            && !ctx.input(|i| i.pointer.any_down())
            && let Some((id, params)) = self.pending_edit.take()
        {
            self.execute(
                "feature.edit",
                json!({ "feature": id.to_string(), "params": params }),
            );
        }
    }

    /// Pushes the hidden-body set to the viewport.
    fn sync_hidden(&mut self) {
        let hidden = self.hidden_bodies.clone();
        if let Some(vp) = &mut self.viewport {
            vp.set_hidden(hidden);
            if let Err(e) = vp.sync_meshes(&mut self.session) {
                self.status = Status::Error(e);
            }
        }
    }

    /// Whether a sketch is drawn in 3D: shown until extruded, unless overridden.
    fn sketch_shown(&self, id: FeatureId) -> bool {
        self.sketch_visibility
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.visible_sketches().contains(&id))
    }

    /// Starts the command a toolbar button, menu entry or shortcut stands for.
    fn invoke(&mut self, spec: &CommandSpec) {
        match spec.id {
            "solid.box" => self.open_popover(Popover::Box),
            "solid.cylinder" => self.open_popover(Popover::Cylinder),
            "sketch.create" => match self.single_face() {
                Some(face) => self.new_sketch(json!({ "face": face })),
                None => self.open_popover(Popover::SketchPlane),
            },
            "body.boolean" => {
                let bodies: Vec<FeatureId> = self.session.model().bodies.keys().copied().collect();
                self.forms.bool_target = bodies.first().copied();
                self.forms.bool_tool = bodies.get(1).copied();
                self.open_popover(Popover::Boolean);
            }
            id if ValueOp::from_command(id).is_some() => {
                if let Some(op) = ValueOp::from_command(id) {
                    self.start_value(op);
                }
            }
            "doc.open" => {
                if let Some(path) = file_dialog().pick_file() {
                    self.execute(spec.id, json!({ "path": path }));
                    if let Some(vp) = &mut self.viewport {
                        vp.fit();
                    }
                }
            }
            "doc.save" | "file.export_step" | "file.export_stl" => {
                let (name, ext) = match spec.id {
                    "doc.save" => ("model.kubit", "kubit"),
                    "file.export_step" => ("model.step", "step"),
                    _ => ("model.stl", "stl"),
                };
                if let Some(path) = file_dialog()
                    .add_filter(ext, &[ext])
                    .set_file_name(name)
                    .save_file()
                {
                    self.execute(spec.id, json!({ "path": path }));
                }
            }
            id => {
                self.execute(id, json!({}));
            }
        }
    }

    fn enabled(&self, spec: &CommandSpec) -> Enablement {
        (spec.enabled)(&self.session)
    }

    /// Status text after `command` succeeded.
    fn done_message(&self, command: &str) -> String {
        if command == "selection.set" {
            let sel = self.session.selection();
            let parts: Vec<String> = [
                ("ui.status.selected_faces", sel.faces.len()),
                ("ui.status.selected_edges", sel.edges.len()),
            ]
            .into_iter()
            .filter(|(_, n)| *n > 0)
            .map(|(key, n)| format!("{}: {n}", self.tr(key)))
            .collect();
            return if parts.is_empty() {
                self.tr("ui.features.none_selected").to_owned()
            } else {
                parts.join(" · ")
            };
        }
        let key = format!("ui.done.{command}");
        let text = self.tr(&key);
        if text == key {
            format!(
                "{} {}",
                self.tr(&format!("cmd.{command}")),
                icons::glyph("finish").unwrap_or("·")
            )
        } else {
            text.to_owned()
        }
    }

    /// "Face: F1:+Z" or "Edge: F1:+X|F1:+Z" for what is under the cursor.
    fn hover_text(&self) -> Option<String> {
        let vp = self.viewport.as_ref()?;
        if let Some((mesh, tag)) = vp.hovered_edge() {
            return self
                .session
                .edge_name(body_of(mesh)?, tag)
                .map(|n| format!("{}: {n}", self.tr("ui.status.hover_edge")));
        }
        let (mesh, tag) = vp.hovered()?;
        self.session
            .face_name(body_of(mesh)?, tag)
            .map(|n| format!("{}: {n}", self.tr("ui.status.hover")))
    }

    fn sketch_colors(&self) -> sketch_ui::Palette {
        let p = self.theme.palette;
        sketch_ui::Palette {
            fully: p.text,
            under: p.hover,
            over: p.error,
            selected: p.accent,
            hover: p.hover,
            text: p.text,
            label_bg: p.panel.gamma_multiply(0.9),
        }
    }

    /// Sketch mode for this frame: input, commands, drawing and keys.
    fn sketch_frame(&mut self, ui: &mut Ui, response: &crate::viewport::ViewportResponse) {
        let (Some(view), Some(mut mode)) = (response.view, self.sketch_mode.take()) else {
            return;
        };
        let Some(state) = self.session.model().sketches.get(&mode.feature).cloned() else {
            self.exit_sketch();
            return;
        };
        let mut commands = Vec::new();
        if let Some(r) = &response.response {
            commands.extend(mode.input(ui, r, &view, &state).commands);
        }
        let typing = ui.ctx().egui_wants_keyboard_input();
        if !typing {
            for tool in Tool::ALL {
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, tool.key())) {
                    mode.set_tool(tool);
                }
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::X)) {
                mode.construction = !mode.construction;
            }
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Delete)
                    || i.consume_key(Modifiers::NONE, Key::Backspace)
            }) && let Some(c) = mode.delete_selection()
            {
                commands.push(c);
            }
        }
        let colors = self.sketch_colors();
        let messages = [self.tr("ui.value.invalid"), self.tr("ui.value.not_length")];
        let ctx = ui.ctx().clone();
        if let Some(c) =
            mode.dimension_field(&ctx, &state.sketch, self.theme.palette.error, messages)
        {
            commands.push(c);
        }
        let escape = !typing && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let painter = ui.painter().with_clip_rect(view.rect);
        mode.paint(
            &painter,
            &view,
            &state,
            &colors,
            response.response.as_ref().and_then(|r| r.hover_pos()),
        );
        ctx.request_repaint();
        let leave = escape && !mode.busy();
        if escape {
            mode.cancel();
        }
        self.sketch_mode = Some(mode);
        for (id, params) in commands {
            self.execute(id, params);
        }
        if leave {
            self.exit_sketch();
        }
    }

    /// Draws unextruded sketches in 3D and returns the region under a click.
    fn sketches_3d(
        &mut self,
        ui: &mut Ui,
        response: &crate::viewport::ViewportResponse,
    ) -> Option<(FeatureId, u32)> {
        let view = response.view?;
        let colors = self.sketch_colors();
        let painter = ui.painter().with_clip_rect(view.rect);
        let pointer = response.response.as_ref().and_then(|r| r.hover_pos());
        let mut hovered = None;
        let ids: Vec<FeatureId> = self
            .session
            .model()
            .sketches
            .keys()
            .copied()
            .filter(|id| self.sketch_shown(*id))
            .collect();
        for id in ids {
            let Some(state) = self.session.model().sketches.get(&id) else {
                continue;
            };
            sketch_ui::paint_sketch(&painter, &view, state, &state.sketch, &colors, None);
            for region in state.regions() {
                let selected = self.region == Some((id, region.id.0));
                if selected {
                    sketch_ui::paint_region(&painter, &view, state, &region, colors.selected);
                }
            }
            if hovered.is_none()
                && let Some(p) = pointer
                && let Some(region) = sketch_ui::region_at(&view, state, p)
            {
                sketch_ui::paint_region(&painter, &view, state, &region, colors.hover);
                hovered = Some((id, region.id.0));
            }
        }
        hovered
    }

    fn viewport_tab(&mut self, ui: &mut Ui) {
        let mut style = self.theme.render_style();
        style.show_edges = self.show_edges;
        let Some(vp) = &mut self.viewport else {
            ui.centered_and_justified(|ui| {
                ui.label(self.i18n.tr("ui.status.no_gpu"));
            });
            return;
        };
        let rect = ui.max_rect();
        let response = vp.show(ui, &style);
        if vp.busy() {
            ui.ctx().request_repaint();
        }
        self.viewport_rect = Some(rect);
        self.view = response.view;
        let ctx = ui.ctx().clone();
        self.view_controls(&ctx, rect);
        if self.sketch_mode.is_some() {
            self.sketch_frame(ui, &response);
            return;
        }
        let region_hit = self.sketches_3d(ui, &response);
        if let Some(at) = response.context {
            self.menu_at = Some(at);
        }
        if let Some(mm) = response.pull
            && let Some(face) = self.single_face()
        {
            self.forms.distance = mm;
            self.execute("face.push_pull", json!({ "face": face, "distance": mm }));
        }
        if let Some(hit) = response.clicked {
            // A sketch region drawn over a face takes the click.
            if let Some(region) = region_hit {
                self.region = Some(region);
                self.execute("selection.set", json!({ "faces": [], "edges": [] }));
                return;
            }
            self.region = None;
            let (mut faces, mut edges): (Vec<String>, Vec<String>) = if response.shift {
                let sel = self.session.selection();
                (
                    sel.faces.iter().map(ToString::to_string).collect(),
                    sel.edges.iter().map(ToString::to_string).collect(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            // Shift+click toggles: a second Shift+click removes the entity.
            let toggle = |list: &mut Vec<String>, name: String| {
                if let Some(i) = list.iter().position(|n| *n == name) {
                    list.remove(i);
                } else {
                    list.push(name);
                }
            };
            match hit {
                Some(Hit::Face((mesh, tag))) => {
                    if let Some(name) = body_of(mesh).and_then(|b| self.session.face_name(b, tag)) {
                        toggle(&mut faces, name.to_string());
                    }
                }
                Some(Hit::Edge((mesh, tag))) => {
                    if let Some(name) = body_of(mesh).and_then(|b| self.session.edge_name(b, tag)) {
                        toggle(&mut edges, name.to_string());
                    }
                }
                None => {}
            }
            self.execute("selection.set", json!({ "faces": faces, "edges": edges }));
        }
    }

    fn popovers(&mut self, ctx: &egui::Context) {
        let Some(which) = self.popover else {
            return;
        };
        let title = match which {
            Popover::Box => self.tr("cmd.solid.box"),
            Popover::Cylinder => self.tr("cmd.solid.cylinder"),
            Popover::Boolean => self.tr("cmd.body.boolean"),
            Popover::SketchPlane => self.tr("ui.sketch.pick_plane"),
        };
        let bodies: Vec<FeatureId> = self.session.model().bodies.keys().copied().collect();
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let mut plane_choice: Option<&str> = None;
        egui::Window::new(title)
            .id(egui::Id::new("kubit-popover"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(
                egui::Align2::LEFT_TOP,
                egui::vec2(self.theme.spacing.lg, 72.0),
            )
            .show(ctx, |ui| {
                let i18n = self.i18n;
                let forms = &mut self.forms;
                match which {
                    Popover::Box => {
                        ui.label(i18n.tr("ui.param.size"));
                        ui.horizontal(|ui| {
                            for v in &mut forms.box_size {
                                ui.add(mm_field(v, 0.001));
                            }
                        });
                        ui.label(i18n.tr("ui.param.origin"));
                        ui.horizontal(|ui| {
                            for v in &mut forms.box_origin {
                                ui.add(mm_field(v, -MAX_MM));
                            }
                        });
                    }
                    Popover::Cylinder => {
                        egui::Grid::new("kubit-cylinder").show(ui, |ui| {
                            ui.label(i18n.tr("ui.param.radius"));
                            ui.add(mm_field(&mut forms.cyl_radius, 0.001));
                            ui.end_row();
                            ui.label(i18n.tr("ui.param.height"));
                            ui.add(mm_field(&mut forms.cyl_height, 0.001));
                            ui.end_row();
                        });
                        ui.label(i18n.tr("ui.param.origin"));
                        ui.horizontal(|ui| {
                            for v in &mut forms.cyl_origin {
                                ui.add(mm_field(v, -MAX_MM));
                            }
                        });
                    }
                    Popover::SketchPlane => {
                        ui.label(i18n.tr("ui.sketch.plane_hint"));
                        ui.horizontal(|ui| {
                            for plane in ["XY", "XZ", "YZ"] {
                                if ui.button(plane).clicked() {
                                    plane_choice = Some(plane);
                                }
                            }
                        });
                    }
                    Popover::Boolean => {
                        ui.horizontal(|ui| {
                            for op in BoolOp::ALL {
                                ui.selectable_value(
                                    &mut forms.bool_op,
                                    op,
                                    i18n.tr(op.label_key()),
                                );
                            }
                        });
                        egui::Grid::new("kubit-boolean").show(ui, |ui| {
                            for (key, slot) in [
                                ("ui.param.target", &mut forms.bool_target),
                                ("ui.param.tool", &mut forms.bool_tool),
                            ] {
                                ui.label(i18n.tr(key));
                                egui::ComboBox::from_id_salt(key)
                                    .selected_text(slot.map(|b| b.to_string()).unwrap_or_default())
                                    .show_ui(ui, |ui| {
                                        for b in &bodies {
                                            ui.selectable_value(slot, Some(*b), b.to_string());
                                        }
                                    });
                                ui.end_row();
                            }
                        });
                    }
                }
                ui.add_space(self.theme.spacing.sm);
                ui.horizontal(|ui| {
                    let can_apply = match which {
                        Popover::Boolean => {
                            forms.bool_target.is_some()
                                && forms.bool_tool.is_some()
                                && forms.bool_target != forms.bool_tool
                        }
                        Popover::Box | Popover::Cylinder => true,
                        Popover::SketchPlane => false,
                    };
                    apply = ui
                        .add_enabled(can_apply, egui::Button::new(i18n.tr("ui.apply")))
                        .clicked()
                        || (can_apply && ui.input(|i| i.key_pressed(Key::Enter)));
                    cancel = ui.button(i18n.tr("ui.cancel")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape));
                });
            });
        if apply {
            let f = self.forms;
            match which {
                Popover::Box => {
                    self.execute(
                        "solid.box",
                        json!({ "origin": f.box_origin, "size": f.box_size }),
                    );
                }
                Popover::Cylinder => {
                    self.execute(
                        "solid.cylinder",
                        json!({ "origin": f.cyl_origin, "radius": f.cyl_radius, "height": f.cyl_height }),
                    );
                }
                Popover::SketchPlane => {}
                Popover::Boolean => {
                    if let (Some(target), Some(tool)) = (f.bool_target, f.bool_tool) {
                        self.execute(
                            "body.boolean",
                            json!({
                                "op": f.bool_op.param(),
                                "target": target.to_string(),
                                "tool": tool.to_string(),
                            }),
                        );
                    }
                }
            }
        }
        if apply || cancel || !open || plane_choice.is_some() {
            self.popover = None;
        }
        if let Some(plane) = plane_choice {
            self.new_sketch(json!(plane));
        }
    }

    /// Registry shortcuts plus `F` (fit). Single keys are ignored while typing.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let mut specs: Vec<(KeyboardShortcut, CommandSpec)> = self
            .session
            .registry()
            .iter()
            .filter_map(|s| Some((parse_shortcut(s.shortcut?)?, *s)))
            .collect();
        // Most specific first, so Ctrl+Shift+Z is not taken by Ctrl+Z.
        specs.sort_by_key(|(k, _)| std::cmp::Reverse(modifier_count(k.modifiers)));
        for (shortcut, spec) in specs {
            if (typing || self.sketch_mode.is_some()) && shortcut.modifiers.is_none() {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&shortcut))
                && self.enabled(&spec) == Enablement::Enabled
            {
                self.invoke(&spec);
            }
        }
        if !typing
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F))
            && let Some(vp) = &mut self.viewport
        {
            vp.fit();
        }
    }
}

fn file_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Kubit", &["kubit", "json"])
}

/// A bounded millimetre field.
fn mm_field(v: &mut f64, min: f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .range(min..=MAX_MM)
        .speed(0.5)
        .max_decimals(3)
}

/// Parses registry shortcuts such as `Ctrl+Shift+Z` or `B`. `Ctrl` means Cmd on macOS.
pub fn parse_shortcut(text: &str) -> Option<KeyboardShortcut> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+') {
        match part.trim() {
            "Ctrl" | "Cmd" => modifiers |= Modifiers::COMMAND,
            "Shift" => modifiers |= Modifiers::SHIFT,
            "Alt" => modifiers |= Modifiers::ALT,
            other => key = Some(Key::from_name(other)?),
        }
    }
    Some(KeyboardShortcut::new(modifiers, key?))
}

fn modifier_count(m: Modifiers) -> u8 {
    u8::from(m.command || m.ctrl) + u8::from(m.shift) + u8::from(m.alt)
}

/// How a registry shortcut reads on this platform.
fn display_shortcut(text: &str) -> String {
    if cfg!(target_os = "macos") {
        text.replace("Ctrl", "Cmd")
    } else {
        text.to_owned()
    }
}

impl KubitApp {
    /// Draws one frame into the root `ui` (eframe and the offscreen screenshot).
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let state = &mut self.state;
        state.pointer = ctx.input(|i| i.pointer.latest_pos());
        state.flush_edit(&ctx);
        state.shortcuts(&ctx);
        let p = state.theme.palette;
        let m = state.theme.spacing.sm;
        egui::Panel::top("kubit-topbar")
            .frame(
                egui::Frame::new()
                    .fill(p.base)
                    .inner_margin(egui::Margin::symmetric(m as i8, 4)),
            )
            .show(ui, |ui| state.top_bar(ui));
        egui::Panel::top("kubit-ribbon")
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(m as i8, 4)),
            )
            .show(ui, |ui| state.ribbon(ui));
        egui::Panel::bottom("kubit-history")
            .frame(
                egui::Frame::new()
                    .fill(p.base)
                    .inner_margin(egui::Margin::symmetric(m as i8, 4)),
            )
            .show(ui, |ui| state.history_bar(ui));
        let side = egui::Frame::new().fill(p.panel).inner_margin(m);
        let left = egui::Panel::left("kubit-navigator")
            .frame(side)
            .show_separator_line(true);
        let left = if state.left_open {
            left.resizable(true)
                .default_size(220.0)
                .size_range(160.0..=420.0)
        } else {
            left.resizable(false).exact_size(panels::RAIL)
        };
        left.show(ui, |ui| state.navigator(ui));
        let right = egui::Panel::right("kubit-properties")
            .frame(side)
            .show_separator_line(true);
        let right = if state.right_open {
            right
                .resizable(true)
                .default_size(260.0)
                .size_range(180.0..=460.0)
        } else {
            right.resizable(false).exact_size(panels::RAIL)
        };
        right.show(ui, |ui| state.properties(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| state.viewport_tab(ui));
        state.popovers(&ctx);
        state.inline_value(&ctx);
        state.context_menu(&ctx);
        state.palette(&ctx);
    }

    /// Whether a viewport exists (false without a GPU).
    pub fn has_viewport(&self) -> bool {
        self.state.viewport.is_some()
    }
}

impl eframe::App for KubitApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factory() -> KernelFactory {
        Box::new(|| Box::new(kubit_kernel_api::testing::BoxKernel::new()))
    }

    fn app_with_box() -> (egui::Context, KubitApp) {
        let ctx = egui::Context::default();
        let options = AppOptions {
            lang: Some(Lang::En),
            startup: vec![ScriptStep {
                command: "solid.box".into(),
                params: json!({ "size": [10, 20, 30] }),
            }],
            ..AppOptions::default()
        };
        let app = KubitApp::build(&ctx, None, factory(), options);
        (ctx, app)
    }

    fn press(ctx: &egui::Context, app: &mut KubitApp, key: Key, modifiers: Modifiers) {
        let raw = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..egui::RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.draw(ui));
        out.textures_delta.clear();
    }

    fn features(app: &KubitApp) -> usize {
        app.state.session.doc().features.len()
    }

    #[test]
    fn startup_script_runs_through_the_session() {
        let (_, app) = app_with_box();
        assert_eq!(features(&app), 1);
        assert!(!app.has_viewport());
    }

    #[test]
    fn undo_and_redo_shortcuts_use_the_commands() {
        let (ctx, mut app) = app_with_box();
        press(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
        assert_eq!(features(&app), 0);
        press(
            &ctx,
            &mut app,
            Key::Z,
            Modifiers::COMMAND | Modifiers::SHIFT,
        );
        assert_eq!(features(&app), 1);
    }

    #[test]
    fn box_popover_creates_a_box_on_enter() {
        let (ctx, mut app) = app_with_box();
        press(&ctx, &mut app, Key::B, Modifiers::NONE);
        assert_eq!(app.state.popover, Some(Popover::Box));
        press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
        assert_eq!(app.state.popover, None);
        assert_eq!(features(&app), 2);
    }

    #[test]
    fn push_pull_needs_one_selected_face() {
        let (ctx, mut app) = app_with_box();
        press(&ctx, &mut app, Key::P, Modifiers::NONE);
        press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
        assert_eq!(features(&app), 1, "no face selected: nothing applied");
        app.state
            .execute("selection.set", json!({ "faces": ["F1:+Z"] }));
        press(&ctx, &mut app, Key::P, Modifiers::NONE);
        press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
        assert_eq!(features(&app), 2);
        assert_eq!(
            app.state.session.doc().features[1].command,
            "face.push_pull"
        );
    }

    #[test]
    fn status_messages_are_localized() {
        let (_, mut app) = app_with_box();
        app.state.i18n = I18n::new(Lang::Es);
        assert_eq!(app.state.done_message("solid.box"), "Caja creada");
        app.state
            .execute("selection.set", json!({ "faces": ["F1:+Z"] }));
        assert_eq!(app.state.done_message("selection.set"), "Caras: 1");
        assert_eq!(
            app.state.done_message("body.measure"),
            format!("Medir {}", icons::glyph("finish").unwrap_or("·"))
        );
        app.state.execute(
            "selection.set",
            json!({ "faces": ["F1:+Z"], "edges": ["F1:+X|F1:+Z", "F1:+Z|F1:-Y"] }),
        );
        assert_eq!(
            app.state.done_message("selection.set"),
            "Caras: 1 · Aristas: 2"
        );
    }

    #[test]
    fn value_ops_need_a_fitting_selection() {
        let (_, mut app) = app_with_box();
        app.state.start_value(ValueOp::Fillet);
        assert!(app.state.inline.is_none());
        assert!(matches!(&app.state.status, Status::Error(k) if k == "ui.param.need_edge"));
        app.state
            .execute("selection.set", json!({ "edges": ["F1:+X|F1:+Z"] }));
        app.state.start_value(ValueOp::Fillet);
        let inline = app.state.inline.clone();
        assert_eq!(inline.map(|i| i.op), Some(ValueOp::Fillet));
        assert_eq!(
            app.state.relevant_commands(),
            vec!["edge.fillet", "edge.chamfer"]
        );
    }

    #[test]
    fn inline_push_pull_applies_to_the_selected_face() {
        let (_, mut app) = app_with_box();
        app.state
            .execute("selection.set", json!({ "faces": ["F1:+Z"] }));
        app.state.apply_value(ValueOp::PushPull, 7.5);
        assert_eq!(features(&app), 2);
        assert_eq!(app.state.forms.distance, 7.5);
    }

    #[test]
    fn failed_command_shows_an_error_without_panicking() {
        let (_, mut app) = app_with_box();
        app.state
            .execute("solid.box", json!({ "size": [-1, 1, 1] }));
        assert!(matches!(app.state.status, Status::Failed { .. }));
        assert_eq!(features(&app), 1);
    }

    #[test]
    fn shortcuts_parse() {
        let z = parse_shortcut("Ctrl+Shift+Z").unwrap();
        assert_eq!(z.logical_key, Key::Z);
        assert!(z.modifiers.command && z.modifiers.shift);
        assert_eq!(parse_shortcut("B").unwrap().modifiers, Modifiers::NONE);
        assert!(parse_shortcut("Ctrl+NoSuchKey").is_none());
        assert!(parse_shortcut("Ctrl").is_none());
    }

    #[test]
    fn every_registry_shortcut_parses() {
        let s = Session::new(Box::new(|| {
            Box::new(kubit_kernel_api::testing::BoxKernel::new())
        }));
        for spec in s.registry().iter() {
            if let Some(text) = spec.shortcut {
                assert!(parse_shortcut(text).is_some(), "{text}");
            }
        }
    }
}
