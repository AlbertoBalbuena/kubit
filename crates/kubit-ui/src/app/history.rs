//! The history bar: one chip per timeline feature, the rollback marker controls, and
//! the status line (selection summary, messages, kernel).

use super::{State, Status, icons};
use egui::{RichText, Ui};
use kubit_doc::FeatureId;
use kubit_engine::FeatureStatus;
use serde_json::{Value, json};

impl State {
    /// The feature after which the marker sits (`None`: the end).
    fn marker(&self) -> Option<FeatureId> {
        self.session.doc().rollback
    }

    /// Moves the rollback marker (no-op until the engine has `timeline.set_marker`).
    fn set_marker(&mut self, after: Option<FeatureId>) {
        if self.session.registry().get("timeline.set_marker").is_some() {
            let after = after.map_or(Value::Null, |f| json!(f.to_string()));
            self.execute("timeline.set_marker", json!({ "after": after }));
        }
    }

    /// The history bar.
    pub(super) fn history_bar(&mut self, ui: &mut Ui) {
        let p = self.theme.palette;
        let features: Vec<(FeatureId, String, Option<&'static str>)> = self
            .session
            .doc()
            .features
            .iter()
            .map(|f| {
                let spec = self.session.registry().get(&f.command).copied();
                let label = spec.map_or(f.command.clone(), |s| self.tr(s.label).to_owned());
                (f.id, label, spec.and_then(|s| s.icon))
            })
            .collect();
        let has_marker = self.session.registry().get("timeline.set_marker").is_some();
        let marker = self.marker();
        let index =
            |id: Option<FeatureId>| id.and_then(|m| features.iter().position(|(f, ..)| *f == m));
        let at = index(marker).map_or(features.len(), |i| i + 1);
        ui.horizontal(|ui| {
            let mut go = None;
            // `at` features are applied; the buttons move it within 1..=n.
            let n = features.len();
            for (icon, tip, target) in [
                ("first", "ui.history.first", (n > 0).then_some(1usize)),
                ("previous", "ui.history.previous", (at > 1).then(|| at - 1)),
                ("next", "ui.history.next", (at < n).then_some(at + 1)),
                ("last", "ui.history.last", Some(n)),
            ] {
                let enabled = has_marker && target.is_some_and(|t| t != at);
                let text = RichText::new(icons::glyph(icon).unwrap_or("·")).color(if enabled {
                    p.text
                } else {
                    p.text_weak
                });
                if ui
                    .add_enabled(enabled, egui::Button::new(text).frame(false))
                    .on_hover_text(self.tr(tip))
                    .clicked()
                {
                    go = target;
                }
            }
            ui.separator();
            let mut select = None;
            let mut open = None;
            let mut menu_action = None;
            egui::ScrollArea::horizontal()
                .id_salt("kubit-history")
                .max_width(ui.available_width() - 260.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, (id, label, icon)) in features.iter().enumerate() {
                            let rolled_back = i >= at;
                            let (dot, faded) = match self.session.model().status.get(id) {
                                Some(FeatureStatus::Failed(_)) => (p.error, false),
                                Some(FeatureStatus::Suppressed) | None => (p.text_weak, true),
                                Some(FeatureStatus::Ok) => (p.ok, false),
                            };
                            let faded = faded || rolled_back;
                            let color = if faded { p.text_weak } else { p.text };
                            let selected = self.selected_feature == Some(*id);
                            let chip = ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                icons::paint(ui.painter(), rect, icon.unwrap_or("box"), color);
                                let r = ui.add(egui::Button::selectable(
                                    selected,
                                    RichText::new(format!("{id} {label}")).color(color),
                                ));
                                let (dot_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(8.0, 8.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(dot_rect.center(), 3.5, dot);
                                r
                            });
                            let r = chip.inner;
                            if r.clicked() {
                                select = Some(*id);
                            }
                            if r.double_clicked() {
                                open = Some(*id);
                            }
                            egui::Popup::context_menu(&r).show(|ui| {
                                let suppressed = self
                                    .session
                                    .doc()
                                    .feature(*id)
                                    .is_some_and(|f| f.suppressed);
                                let actions = [
                                    (
                                        "feature.suppress",
                                        if suppressed {
                                            "ui.history.unsuppress"
                                        } else {
                                            "ui.history.suppress"
                                        },
                                    ),
                                    ("feature.delete", "ui.history.delete"),
                                    ("timeline.set_marker", "ui.history.marker_here"),
                                ];
                                for (cmd, key) in actions {
                                    let available = self.session.registry().get(cmd).is_some();
                                    if ui
                                        .add_enabled(available, egui::Button::new(self.tr(key)))
                                        .clicked()
                                    {
                                        menu_action = Some((cmd, *id, suppressed));
                                        ui.close();
                                    }
                                }
                            });
                            if has_marker && i + 1 == at && at < features.len() {
                                let (rect, r) = ui.allocate_exact_size(
                                    egui::vec2(10.0, 20.0),
                                    egui::Sense::hover(),
                                );
                                let x = rect.center().x;
                                ui.painter().line_segment(
                                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                    egui::Stroke::new(2.5, p.accent),
                                );
                                ui.painter().add(egui::Shape::convex_polygon(
                                    vec![
                                        egui::pos2(x - 4.0, rect.top()),
                                        egui::pos2(x + 4.0, rect.top()),
                                        egui::pos2(x, rect.top() + 5.0),
                                    ],
                                    p.accent,
                                    egui::Stroke::NONE,
                                ));
                                r.on_hover_text(self.tr("ui.history.marker"));
                            }
                        }
                    });
                });
            if let Some(f) = select {
                self.selected_feature = Some(f);
            }
            if let Some(f) = open {
                let is_sketch = self
                    .session
                    .doc()
                    .feature(f)
                    .is_some_and(|x| x.command == "sketch.create");
                if is_sketch {
                    self.enter_sketch(f);
                } else {
                    self.selected_feature = Some(f);
                    self.right_open = true;
                }
            }
            match menu_action {
                Some(("feature.suppress", f, suppressed)) => {
                    self.execute(
                        "feature.suppress",
                        json!({ "feature": f.to_string(), "suppressed": !suppressed }),
                    );
                }
                Some(("feature.delete", f, _)) => {
                    self.execute("feature.delete", json!({ "feature": f.to_string() }));
                    if self.selected_feature == Some(f) {
                        self.selected_feature = None;
                    }
                }
                Some(("timeline.set_marker", f, _)) => self.set_marker(Some(f)),
                _ => {}
            }
            if let Some(t) = go {
                let after = if t >= n {
                    None
                } else {
                    t.checked_sub(1)
                        .and_then(|i| features.get(i))
                        .map(|(f, ..)| *f)
                };
                self.set_marker(after);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let info = self.session.kernel_info();
                ui.label(
                    RichText::new(format!("{} {}", info.name, info.version))
                        .small()
                        .color(p.text_weak),
                );
                ui.separator();
                match &self.status {
                    Status::Ready => {
                        ui.label(
                            RichText::new(self.tr("ui.status.ready"))
                                .small()
                                .color(p.text_weak),
                        );
                    }
                    Status::Done(command) => {
                        ui.label(RichText::new(self.done_message(command)).small());
                    }
                    Status::Failed { command, detail } => {
                        let label = self.tr(&format!("cmd.{command}")).to_owned();
                        let text = format!("{} {label}: {detail}", self.tr("ui.status.failed"));
                        ui.label(RichText::new(text).small().color(p.error))
                            .on_hover_text(detail.as_str());
                    }
                    Status::Error(key) => {
                        ui.label(RichText::new(self.tr(key)).small().color(p.error));
                    }
                }
                if let Some(text) = self.hover_text() {
                    ui.separator();
                    ui.label(RichText::new(text).small().monospace());
                }
            });
        });
    }
}
