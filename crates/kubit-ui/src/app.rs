//! The eframe application: dock layout, registry-driven toolbar, parameter
//! popovers, shortcuts and the status bar.
//!
//! Every change to the document goes through `Session::execute`. Commands never
//! open dialogs; the UI opens file dialogs first and passes the chosen path as a
//! parameter.

use crate::i18n::{I18n, Lang};
use crate::theme::{Theme, ThemeMode};
use crate::value_input::{FieldAction, ValueField};
use crate::viewport::{Hit, Viewport3d, body_of};
use eframe::egui_wgpu;
use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use kubit_doc::FeatureId;
use kubit_engine::{CommandSpec, Enablement, Event, FeatureStatus, KernelFactory, Session};
use kubit_render::StandardView;
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
}

/// Dock tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// The 3D viewport.
    Viewport,
    /// Feature list, selection and kernel.
    Features,
}

/// Which multi-field parameter window is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Popover {
    Box,
    Cylinder,
    Boolean,
}

/// Operations that take one length, typed inline next to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueOp {
    PushPull,
    Fillet,
    Chamfer,
    Shell,
}

impl ValueOp {
    fn from_command(id: &str) -> Option<Self> {
        Some(match id {
            "face.push_pull" => Self::PushPull,
            "edge.fillet" => Self::Fillet,
            "edge.chamfer" => Self::Chamfer,
            "body.shell" => Self::Shell,
            _ => return None,
        })
    }

    fn command(self) -> &'static str {
        match self {
            Self::PushPull => "face.push_pull",
            Self::Fillet => "edge.fillet",
            Self::Chamfer => "edge.chamfer",
            Self::Shell => "body.shell",
        }
    }

    /// i18n key of the value's label.
    fn label_key(self) -> &'static str {
        match self {
            Self::PushPull | Self::Chamfer => "ui.param.distance",
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
        }
    }

    fn remember(&mut self, op: ValueOp, mm: f64) {
        match op {
            ValueOp::PushPull => self.distance = mm,
            ValueOp::Fillet => self.radius = mm,
            ValueOp::Chamfer => self.chamfer = mm,
            ValueOp::Shell => self.thickness = mm,
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

/// Everything except the dock tree (so the tab viewer can borrow it mutably).
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
}

/// The Kubit desktop app.
pub struct KubitApp {
    dock: DockState<Tab>,
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
        let mut dock = DockState::new(vec![Tab::Viewport]);
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), 0.76, vec![Tab::Features]);
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
        };
        for step in options.startup {
            state.execute(&step.command, step.params);
        }
        Self { dock, state }
    }
}

impl State {
    /// Runs a command and refreshes the views from its events.
    fn execute(&mut self, id: &str, params: Value) {
        self.status = match self.session.execute(id, params) {
            Ok(_) => Status::Done(id.to_owned()),
            Err(e) => Status::Failed {
                command: id.to_owned(),
                detail: e.to_string(),
            },
        };
        self.drain_events();
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
        let missing = if op.needs_edges() {
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
        };
        self.execute(op.command(), params);
    }

    /// Commands that fit the current selection (context toolbar and menu).
    fn context_commands(&self) -> Vec<CommandSpec> {
        let sel = self.session.selection();
        let ids: &[&str] = if !sel.edges.is_empty() {
            &["edge.fillet", "edge.chamfer"]
        } else if !sel.faces.is_empty() {
            &["face.push_pull", "body.shell"]
        } else if self.session.model().bodies.len() >= 2 {
            &["solid.box", "solid.cylinder", "body.boolean"]
        } else {
            &["solid.box", "solid.cylinder"]
        };
        ids.iter()
            .filter_map(|id| self.session.registry().get(id).copied())
            .collect()
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

    /// Context toolbar over the viewport's top-left corner.
    fn context_toolbar(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let specs = self.context_commands();
        if specs.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("kubit-context-toolbar"))
            .order(egui::Order::Middle)
            .fade_in(false)
            .fixed_pos(rect.left_top() + egui::vec2(10.0, 10.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for spec in &specs {
                            self.command_button(ui, spec);
                        }
                    });
                });
            });
    }

    /// The right-click menu, if open.
    fn context_menu(&mut self, ctx: &egui::Context) {
        let Some(at) = self.menu_at else {
            return;
        };
        let specs = self.context_commands();
        let extra: Vec<CommandSpec> = ["edit.undo", "edit.redo"]
            .iter()
            .filter_map(|id| self.session.registry().get(id).copied())
            .collect();
        let mut chosen = false;
        let area = egui::Area::new(egui::Id::new("kubit-context-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(140.0);
                    for spec in &specs {
                        chosen |= self.command_button(ui, spec);
                    }
                    ui.separator();
                    for spec in &extra {
                        chosen |= self.command_button(ui, spec);
                    }
                    if ui.button(self.i18n.tr("ui.view.fit")).clicked() {
                        if let Some(vp) = &mut self.viewport {
                            vp.fit();
                        }
                        chosen = true;
                    }
                });
            });
        if chosen || area.response.clicked_elsewhere() || ctx.input(|i| i.key_pressed(Key::Escape))
        {
            self.menu_at = None;
        }
    }

    /// Starts the command a toolbar button, menu entry or shortcut stands for.
    fn invoke(&mut self, spec: &CommandSpec) {
        match spec.id {
            "solid.box" => self.open_popover(Popover::Box),
            "solid.cylinder" => self.open_popover(Popover::Cylinder),
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
            id => self.execute(id, json!({})),
        }
    }

    fn enabled(&self, spec: &CommandSpec) -> Enablement {
        (spec.enabled)(&self.session)
    }

    /// A button for `spec`: translated label, tooltip with description, shortcut
    /// and (when disabled) the reason.
    /// A button for `spec`; returns whether it was clicked.
    fn command_button(&mut self, ui: &mut Ui, spec: &CommandSpec) -> bool {
        let enablement = self.enabled(spec);
        let mut tip = spec.description.to_owned();
        if let Some(s) = spec.shortcut {
            tip = format!("{tip}\n{}", display_shortcut(s));
        }
        if let Enablement::Disabled(reason) = enablement {
            tip = format!("{tip}\n({reason})");
        }
        let label = self.tr(spec.label);
        let clicked = ui
            .add_enabled(enablement == Enablement::Enabled, egui::Button::new(label))
            .on_hover_text(tip)
            .on_disabled_hover_text(match enablement {
                Enablement::Disabled(r) => r,
                Enablement::Enabled => "",
            })
            .clicked();
        if clicked {
            self.invoke(spec);
        }
        clicked
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(self.tr("ui.menu.file"), |ui| {
                for id in [
                    "doc.new",
                    "doc.open",
                    "doc.save",
                    "file.export_step",
                    "file.export_stl",
                ] {
                    if let Some(spec) = self.session.registry().get(id).copied() {
                        self.command_button(ui, &spec);
                    }
                }
            });
            ui.menu_button(self.tr("ui.menu.view"), |ui| {
                self.view_buttons(ui);
                ui.separator();
                let (key, mode) = match self.theme.mode {
                    ThemeMode::Dark => ("ui.view.theme_light", ThemeMode::Light),
                    ThemeMode::Light => ("ui.view.theme_dark", ThemeMode::Dark),
                };
                if ui.button(self.tr(key)).clicked() {
                    self.theme = Theme::new(mode);
                    self.theme.apply(ui.ctx());
                }
            });
        });
    }

    fn view_buttons(&mut self, ui: &mut Ui) {
        let Some(vp) = &mut self.viewport else {
            return;
        };
        if ui.button(self.i18n.tr("ui.view.fit")).clicked() {
            vp.fit();
        }
        for (key, view) in [
            ("ui.view.iso", StandardView::Iso),
            ("ui.view.top", StandardView::Top),
            ("ui.view.front", StandardView::Front),
            ("ui.view.right", StandardView::Right),
        ] {
            if ui.button(self.i18n.tr(key)).clicked() {
                vp.set_view(view);
            }
        }
    }

    /// Toolbar: registry commands with a `toolbar_group`, grouped in registration order.
    fn toolbar(&mut self, ui: &mut Ui) {
        let mut groups: Vec<(&'static str, Vec<CommandSpec>)> = Vec::new();
        for spec in self.session.registry().iter() {
            let Some(g) = spec.toolbar_group else {
                continue;
            };
            match groups.iter_mut().find(|(name, _)| *name == g) {
                Some((_, specs)) => specs.push(*spec),
                None => groups.push((g, vec![*spec])),
            }
        }
        ui.horizontal(|ui| {
            for (i, (group, specs)) in groups.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                let key = format!("ui.group.{group}");
                ui.label(
                    RichText::new(self.i18n.tr(&key))
                        .small()
                        .color(self.theme.palette.text_weak),
                );
                for spec in specs {
                    self.command_button(ui, spec);
                }
            }
            ui.separator();
            self.view_buttons(ui);
        });
    }

    fn status_bar(&self, ui: &mut Ui) {
        let p = &self.theme.palette;
        ui.horizontal(|ui| {
            match &self.status {
                Status::Ready => {
                    ui.label(self.tr("ui.status.ready"));
                }
                Status::Done(command) => {
                    ui.label(self.done_message(command));
                }
                Status::Failed { command, detail } => {
                    let label = self.tr(&format!("cmd.{command}")).to_owned();
                    let text = format!("{} {label}: {detail}", self.tr("ui.status.failed"));
                    ui.label(RichText::new(text).color(p.error));
                }
                Status::Error(s) => {
                    ui.label(RichText::new(self.tr(s)).color(p.error));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let info = self.session.kernel_info();
                ui.label(
                    RichText::new(format!("{} {}", info.name, info.version)).color(p.text_weak),
                );
                if let Some(text) = self.hover_text() {
                    ui.separator();
                    ui.label(text);
                }
            });
        });
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
            format!("{} ✓", self.tr(&format!("cmd.{command}")))
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

    fn features_panel(&mut self, ui: &mut Ui) {
        let p = self.theme.palette;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let doc = self.session.doc();
            if doc.features.is_empty() {
                ui.label(RichText::new(self.tr("ui.features.empty")).color(p.text_weak));
            }
            for f in &doc.features {
                let label = self
                    .session
                    .registry()
                    .get(&f.command)
                    .map_or(f.command.as_str(), |s| self.i18n.tr(s.label));
                let (color, tip) = match self.session.model().status.get(&f.id) {
                    Some(FeatureStatus::Ok) => (p.ok, self.tr("ui.feature.ok").to_owned()),
                    Some(FeatureStatus::Failed(m)) => {
                        (p.error, format!("{}: {m}", self.tr("ui.feature.failed")))
                    }
                    Some(FeatureStatus::Suppressed) | None => {
                        (p.text_weak, self.tr("ui.feature.suppressed").to_owned())
                    }
                };
                ui.horizontal(|ui| {
                    status_dot(ui, color).on_hover_text(tip);
                    ui.label(
                        RichText::new(f.id.to_string())
                            .monospace()
                            .color(p.text_weak),
                    );
                    ui.label(label);
                });
            }
            ui.add_space(self.theme.spacing.lg);
            ui.label(RichText::new(self.tr("ui.features.selection")).strong());
            let sel = self.session.selection();
            if sel.faces.is_empty() && sel.edges.is_empty() {
                ui.label(RichText::new(self.tr("ui.features.none_selected")).color(p.text_weak));
            }
            let names = sel
                .faces
                .iter()
                .map(ToString::to_string)
                .chain(sel.edges.iter().map(ToString::to_string));
            for name in names {
                ui.label(RichText::new(name).monospace());
            }
        });
    }

    fn viewport_tab(&mut self, ui: &mut Ui) {
        let style = self.theme.render_style();
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
        if let Some(at) = response.context {
            self.menu_at = Some(at);
        }
        if let Some(mm) = response.pull
            && let Some(face) = self.single_face()
        {
            self.forms.distance = mm;
            self.execute("face.push_pull", json!({ "face": face, "distance": mm }));
        }
        let ctx = ui.ctx().clone();
        self.context_toolbar(&ctx, rect);
        if let Some(hit) = response.clicked {
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
        };
        let bodies: Vec<FeatureId> = self.session.model().bodies.keys().copied().collect();
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
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
                Popover::Box => self.execute(
                    "solid.box",
                    json!({ "origin": f.box_origin, "size": f.box_size }),
                ),
                Popover::Cylinder => self.execute(
                    "solid.cylinder",
                    json!({ "origin": f.cyl_origin, "radius": f.cyl_radius, "height": f.cyl_height }),
                ),
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
        if apply || cancel || !open {
            self.popover = None;
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
            if typing && shortcut.modifiers.is_none() {
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

/// A small filled circle (feature status), sized like a line of text.
fn status_dot(ui: &mut Ui, color: egui::Color32) -> egui::Response {
    let h = ui.text_style_height(&egui::TextStyle::Body);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(h * 0.6, h), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), h * 0.22, color);
    response
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

struct Viewer<'a> {
    state: &'a mut State,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(("kubit-tab", *tab as u8))
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        let key = match tab {
            Tab::Viewport => "ui.tab.viewport",
            Tab::Features => "ui.tab.features",
        };
        self.state.tr(key).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match tab {
            Tab::Viewport => self.state.viewport_tab(ui),
            Tab::Features => self.state.features_panel(ui),
        }
    }

    fn is_closeable(&self, _tab: &Tab) -> bool {
        false
    }
}

impl KubitApp {
    /// Draws one frame into the root `ui` (eframe and the offscreen screenshot).
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let state = &mut self.state;
        state.pointer = ctx.input(|i| i.pointer.latest_pos());
        state.shortcuts(&ctx);
        let p = state.theme.palette;
        let m = state.theme.spacing.sm;
        egui::Panel::top("kubit-menu")
            .frame(egui::Frame::new().fill(p.base).inner_margin(m))
            .show(ui, |ui| state.menu_bar(ui));
        egui::Panel::top("kubit-toolbar")
            .frame(egui::Frame::new().fill(p.panel).inner_margin(m))
            .show(ui, |ui| state.toolbar(ui));
        egui::Panel::bottom("kubit-status")
            .frame(egui::Frame::new().fill(p.base).inner_margin(m))
            .show(ui, |ui| state.status_bar(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            DockArea::new(&mut self.dock)
                .style(egui_dock::Style::from_egui(ui.style().as_ref()))
                .show_close_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut Viewer { state: &mut *state });
        });
        state.popovers(&ctx);
        state.inline_value(&ctx);
        state.context_menu(&ctx);
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
        assert_eq!(app.state.done_message("body.measure"), "Medir ✓");
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
            app.state
                .context_commands()
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            app.state
                .session
                .registry()
                .iter()
                .map(|s| s.id)
                .filter(|id| ["edge.fillet", "edge.chamfer"].contains(id))
                .collect::<Vec<_>>()
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
