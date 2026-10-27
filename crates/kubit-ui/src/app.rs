//! The eframe application: dock layout, registry-driven toolbar, parameter
//! popovers, shortcuts and the status bar.
//!
//! Every change to the document goes through `Session::execute`. Commands never
//! open dialogs; the UI opens file dialogs first and passes the chosen path as a
//! parameter.

use crate::i18n::{I18n, Lang};
use crate::theme::{Theme, ThemeMode};
use crate::viewport::{Viewport3d, body_of};
use eframe::egui_wgpu;
use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
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

/// Which parameter popover is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Popover {
    Box,
    PushPull,
}

/// Parameters edited in the popovers (kept between uses).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Forms {
    box_origin: [f64; 3],
    box_size: [f64; 3],
    distance: f64,
}

impl Default for Forms {
    fn default() -> Self {
        Self {
            box_origin: [0.0; 3],
            box_size: [100.0, 60.0, 40.0],
            distance: 10.0,
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

    /// Starts the command a toolbar button, menu entry or shortcut stands for.
    fn invoke(&mut self, spec: &CommandSpec) {
        match spec.id {
            "solid.box" => self.popover = Some(Popover::Box),
            "face.push_pull" => self.popover = Some(Popover::PushPull),
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
    fn command_button(&mut self, ui: &mut Ui, spec: &CommandSpec) {
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
                if let Some(name) = self.hovered_face() {
                    ui.separator();
                    ui.label(format!("{}: {name}", self.tr("ui.status.hover")));
                }
            });
        });
    }

    /// Status text after `command` succeeded.
    fn done_message(&self, command: &str) -> String {
        if command == "selection.set" {
            let n = self.session.selection().faces.len();
            return if n == 0 {
                self.tr("ui.features.none_selected").to_owned()
            } else {
                format!("{}: {n}", self.tr("ui.status.selected"))
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

    fn hovered_face(&self) -> Option<String> {
        let (mesh, tag) = self.viewport.as_ref()?.hovered()?;
        self.session
            .face_name(body_of(mesh)?, tag)
            .map(|n| n.to_string())
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
            let faces = &self.session.selection().faces;
            if faces.is_empty() {
                ui.label(RichText::new(self.tr("ui.features.none_selected")).color(p.text_weak));
            }
            for name in faces {
                ui.label(RichText::new(name.to_string()).monospace());
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
        let response = vp.show(ui, &style);
        if vp.busy() {
            ui.ctx().request_repaint();
        }
        if let Some(hit) = response.clicked {
            let faces: Vec<String> = hit
                .and_then(|(mesh, tag)| self.session.face_name(body_of(mesh)?, tag))
                .map(|n| n.to_string())
                .into_iter()
                .collect();
            self.execute("selection.set", json!({ "faces": faces }));
        }
    }

    fn popovers(&mut self, ctx: &egui::Context) {
        let Some(which) = self.popover else {
            return;
        };
        let title = match which {
            Popover::Box => self.tr("cmd.solid.box"),
            Popover::PushPull => self.tr("cmd.face.push_pull"),
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let face = self.single_face();
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
                    Popover::PushPull => {
                        ui.horizontal(|ui| {
                            ui.label(i18n.tr("ui.param.face"));
                            match &face {
                                Some(f) => ui.label(RichText::new(f).monospace()),
                                None => ui.label(i18n.tr("ui.param.need_face")),
                            };
                        });
                        ui.horizontal(|ui| {
                            ui.label(i18n.tr("ui.param.distance"));
                            ui.add(mm_field(&mut forms.distance, -MAX_MM));
                        });
                    }
                }
                ui.add_space(self.theme.spacing.sm);
                ui.horizontal(|ui| {
                    let can_apply = which == Popover::Box || face.is_some();
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
                Popover::PushPull => {
                    if let Some(face) = face {
                        self.execute(
                            "face.push_pull",
                            json!({ "face": face, "distance": f.distance }),
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
        assert_eq!(
            app.state.done_message("selection.set"),
            "Caras seleccionadas: 1"
        );
        assert_eq!(app.state.done_message("body.measure"), "Medir ✓");
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
