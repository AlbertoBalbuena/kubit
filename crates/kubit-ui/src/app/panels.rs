//! The side panels: Navigator (origin planes, bodies, sketches) on the left and
//! Properties (whatever is selected) on the right. Both collapse to a thin rail.

use super::{State, icons};
use egui::{RichText, Ui};
use kubit_doc::FeatureId;
use kubit_engine::FeatureStatus;
use serde_json::{Value, json};

/// Width of a collapsed panel, points.
pub(super) const RAIL: f32 = 40.0;

impl State {
    /// A panel header with a collapse button; returns whether it is open.
    fn panel_header(&mut self, ui: &mut Ui, title_key: &str, left: bool) -> bool {
        let open = if left {
            self.left_open
        } else {
            self.right_open
        };
        let p = self.theme.palette;
        let toggle = ui
            .horizontal(|ui| {
                if open {
                    ui.label(RichText::new(self.tr(title_key)).strong());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let icon = icons::glyph("sidebar").unwrap_or("≡");
                    ui.add(egui::Button::new(RichText::new(icon).color(p.text_weak)).frame(false))
                        .on_hover_text(self.tr(if open {
                            "ui.panel.collapse"
                        } else {
                            "ui.panel.expand"
                        }))
                        .clicked()
                })
                .inner
            })
            .inner;
        if toggle {
            if left {
                self.left_open = !self.left_open;
            } else {
                self.right_open = !self.right_open;
            }
        }
        open
    }

    /// The Navigator: origin planes (click to sketch), bodies and sketches with
    /// visibility toggles.
    pub(super) fn navigator(&mut self, ui: &mut Ui) {
        if !self.panel_header(ui, "ui.panel.navigator", true) {
            return;
        }
        let p = self.theme.palette;
        ui.separator();
        ui.label(
            RichText::new(self.tr("ui.nav.origin"))
                .small()
                .color(p.text_weak),
        );
        for plane in ["XY", "XZ", "YZ"] {
            let r = ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                icons::paint(ui.painter(), rect, "plane", p.text_weak);
                ui.add(
                    egui::Label::new(format!("{} {plane}", self.tr("ui.nav.plane")))
                        .sense(egui::Sense::click()),
                )
            });
            if r.inner
                .on_hover_text(self.tr("ui.nav.plane_hint"))
                .clicked()
            {
                self.new_sketch(json!(plane));
            }
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(self.tr("ui.nav.bodies"))
                .small()
                .color(p.text_weak),
        );
        let bodies: Vec<FeatureId> = self.session.model().bodies.keys().copied().collect();
        if bodies.is_empty() {
            ui.label(RichText::new(self.tr("ui.nav.none")).color(p.text_weak));
        }
        let mut toggle_body = None;
        for b in &bodies {
            ui.horizontal(|ui| {
                let hidden = self.hidden_bodies.contains(b);
                let eye = icons::glyph(if hidden { "eye_off" } else { "eye" }).unwrap_or("o");
                if ui
                    .add(egui::Button::new(RichText::new(eye).color(p.text_weak)).frame(false))
                    .clicked()
                {
                    toggle_body = Some(*b);
                }
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                icons::paint(
                    ui.painter(),
                    rect,
                    "body",
                    if hidden { p.text_weak } else { p.text },
                );
                ui.label(format!("{} {b}", self.tr("ui.nav.body")));
            });
        }
        if let Some(b) = toggle_body {
            if !self.hidden_bodies.remove(&b) {
                self.hidden_bodies.insert(b);
            }
            self.sync_hidden();
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(self.tr("ui.nav.sketches"))
                .small()
                .color(p.text_weak),
        );
        let sketches: Vec<FeatureId> = self.session.model().sketches.keys().copied().collect();
        if sketches.is_empty() {
            ui.label(RichText::new(self.tr("ui.nav.none")).color(p.text_weak));
        }
        let (mut toggle_sketch, mut open) = (None, None);
        for s in &sketches {
            ui.horizontal(|ui| {
                let shown = self.sketch_shown(*s);
                let eye = icons::glyph(if shown { "eye" } else { "eye_off" }).unwrap_or("o");
                if ui
                    .add(egui::Button::new(RichText::new(eye).color(p.text_weak)).frame(false))
                    .clicked()
                {
                    toggle_sketch = Some((*s, shown));
                }
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                icons::paint(
                    ui.painter(),
                    rect,
                    "sketch",
                    if shown { p.text } else { p.text_weak },
                );
                let r = ui.add(
                    egui::Label::new(format!("{} {s}", self.tr("cmd.sketch.create")))
                        .sense(egui::Sense::click()),
                );
                if r.on_hover_text(self.tr("ui.nav.edit_hint"))
                    .double_clicked()
                {
                    open = Some(*s);
                }
            });
        }
        if let Some((s, shown)) = toggle_sketch {
            self.sketch_visibility.insert(s, !shown);
        }
        if let Some(s) = open {
            self.enter_sketch(s);
        }
    }

    /// The Properties panel.
    pub(super) fn properties(&mut self, ui: &mut Ui) {
        if !self.panel_header(ui, "ui.panel.properties", false) {
            return;
        }
        ui.separator();
        let p = self.theme.palette;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(mode) = &self.sketch_mode {
                let feature = mode.feature;
                self.sketch_properties(ui, feature);
                return;
            }
            if let Some(f) = self.selected_feature {
                self.feature_properties(ui, f);
                ui.add_space(8.0);
            }
            if let Some((sketch, region)) = self.region {
                ui.label(RichText::new(self.tr("ui.props.region")).strong());
                ui.label(RichText::new(format!("{sketch} · {region}")).monospace());
                if let Some(r) = self
                    .session
                    .model()
                    .sketches
                    .get(&sketch)
                    .and_then(|st| st.regions().into_iter().find(|r| r.id.0 == region))
                {
                    ui.label(format!("{}: {:.2} mm²", self.tr("ui.props.area"), r.area));
                }
                return;
            }
            let sel = self.session.selection();
            if sel.faces.is_empty() && sel.edges.is_empty() && self.selected_feature.is_none() {
                ui.label(RichText::new(self.tr("ui.props.empty")).color(p.text_weak));
            }
            let names: Vec<String> = sel
                .faces
                .iter()
                .map(ToString::to_string)
                .chain(sel.edges.iter().map(ToString::to_string))
                .collect();
            if !names.is_empty() {
                ui.label(RichText::new(self.tr("ui.features.selection")).strong());
                for n in names {
                    ui.label(RichText::new(n).monospace());
                }
            }
        });
    }

    fn sketch_properties(&mut self, ui: &mut Ui, feature: FeatureId) {
        let p = self.theme.palette;
        let Some(st) = self.session.model().sketches.get(&feature).cloned() else {
            return;
        };
        ui.label(RichText::new(format!("{} {feature}", self.tr("cmd.sketch.create"))).strong());
        let dof = st.report.dof;
        let (text, color) = if !st.report.conflicting.is_empty() {
            (self.tr("ui.sketch.conflict").to_owned(), p.error)
        } else if dof == 0 {
            (self.tr("ui.sketch.fully").to_owned(), p.ok)
        } else {
            (format!("{}: {dof}", self.tr("ui.sketch.dof")), p.hover)
        };
        ui.label(RichText::new(text).color(color));
        let mut remove = None;
        for c in &st.report.conflicting {
            let kind = st
                .sketch
                .constraints
                .get(c)
                .map_or("?", |r| r.constraint.kind());
            if ui
                .button(format!("{} {c} ({kind})", self.tr("ui.sketch.remove")))
                .clicked()
            {
                remove = Some(c.0);
            }
        }
        if !st.report.redundant.is_empty() {
            ui.label(
                RichText::new(format!(
                    "{}: {}",
                    self.tr("ui.sketch.redundant"),
                    st.report.redundant.len()
                ))
                .color(p.text_weak),
            );
        }
        ui.add_space(6.0);
        ui.label(format!(
            "{}: {}",
            self.tr("ui.props.entities"),
            st.sketch.entities.len()
        ));
        ui.label(format!(
            "{}: {}",
            self.tr("ui.props.constraints"),
            st.sketch.constraints.len()
        ));
        ui.label(format!(
            "{}: {}",
            self.tr("ui.props.regions"),
            st.regions().len()
        ));
        if let Some(c) = remove {
            self.execute(
                "sketch.delete",
                json!({ "sketch": feature.to_string(), "constraints": [c] }),
            );
        }
    }

    /// A timeline feature: status and its numeric parameters, editable when the
    /// engine offers `feature.edit`.
    fn feature_properties(&mut self, ui: &mut Ui, id: FeatureId) {
        let p = self.theme.palette;
        let Some(f) = self.session.doc().feature(id).cloned() else {
            self.selected_feature = None;
            return;
        };
        let label = self
            .session
            .registry()
            .get(&f.command)
            .map_or(f.command.clone(), |s| self.tr(s.label).to_owned());
        ui.label(RichText::new(format!("{label} {id}")).strong());
        match self.session.model().status.get(&id) {
            Some(FeatureStatus::Failed(m)) => {
                ui.label(
                    RichText::new(format!("{}: {m}", self.tr("ui.feature.failed"))).color(p.error),
                );
            }
            Some(FeatureStatus::Suppressed) => {
                ui.label(RichText::new(self.tr("ui.feature.suppressed")).color(p.text_weak));
            }
            _ => {}
        }
        if f.command == "sketch.create" {
            return;
        }
        let editable = self.session.registry().get("feature.edit").is_some();
        // While a value is being dragged, the edited copy is the source of truth.
        let mut params = match &self.pending_edit {
            Some((pid, pending)) if *pid == id => pending.clone(),
            _ => f.params.clone(),
        };
        let mut changed = false;
        if let Value::Object(map) = &mut params {
            egui::Grid::new(("kubit-props", id.0))
                .num_columns(2)
                .show(ui, |ui| {
                    for (key, value) in map.iter_mut() {
                        match value {
                            Value::Number(n) => {
                                let mut v = n.as_f64().unwrap_or(0.0);
                                ui.label(key.as_str());
                                if ui
                                    .add_enabled(
                                        editable,
                                        egui::DragValue::new(&mut v).speed(0.5).max_decimals(3),
                                    )
                                    .changed()
                                    && let Some(num) = serde_json::Number::from_f64(v)
                                {
                                    *n = num;
                                    changed = true;
                                }
                                ui.end_row();
                            }
                            Value::Array(items)
                                if items.iter().all(Value::is_number) && !items.is_empty() =>
                            {
                                ui.label(key.as_str());
                                ui.horizontal(|ui| {
                                    for item in items.iter_mut() {
                                        let mut v = item.as_f64().unwrap_or(0.0);
                                        if ui
                                            .add_enabled(
                                                editable,
                                                egui::DragValue::new(&mut v)
                                                    .speed(0.5)
                                                    .max_decimals(3),
                                            )
                                            .changed()
                                            && let Some(num) = serde_json::Number::from_f64(v)
                                        {
                                            *item = Value::Number(num);
                                            changed = true;
                                        }
                                    }
                                });
                                ui.end_row();
                            }
                            Value::String(s) => {
                                ui.label(key.as_str());
                                ui.label(RichText::new(s.as_str()).monospace());
                                ui.end_row();
                            }
                            _ => {}
                        }
                    }
                });
        }
        if !editable {
            ui.label(
                RichText::new(self.tr("ui.props.edit_soon"))
                    .small()
                    .color(p.text_weak),
            );
        }
        if changed {
            // Applied when the pointer is released (`State::flush_edit`), so a drag is
            // one rebuild and one undo step.
            self.pending_edit = Some((id, params));
        }
    }
}
