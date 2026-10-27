//! The ribbon: the one place modelling commands live.
//!
//! Tabs hold groups of large icon buttons. The layout is a table here (the engine's
//! `CommandSpec` stays UI-agnostic); a test checks that every user-facing command
//! has exactly one home across the ribbon, the app menu and the top bar. In sketch
//! mode a contextual Sketch tab replaces Model with drawing tools and constraints.

use super::{State, icons};
use crate::sketch_ui::{ConstraintKind, Tool};
use egui::{Color32, RichText, Sense, Ui};
use kubit_engine::{CommandSpec, Enablement};

/// A ribbon tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum RibbonTab {
    /// Sketch, create and modify.
    #[default]
    Model,
    /// Measure (later: sections, DFM).
    Inspect,
}

/// A group of commands under a caption.
pub(super) struct Group {
    /// i18n key of the caption.
    pub key: &'static str,
    /// Command ids, in order.
    pub commands: &'static [&'static str],
}

/// The Model tab.
pub(super) const MODEL: &[Group] = &[
    Group {
        key: "ui.ribbon.sketch",
        commands: &["sketch.create"],
    },
    Group {
        key: "ui.ribbon.create",
        commands: &["solid.extrude", "solid.box", "solid.cylinder"],
    },
    Group {
        key: "ui.ribbon.modify",
        commands: &[
            "face.push_pull",
            "edge.fillet",
            "edge.chamfer",
            "body.shell",
            "body.boolean",
        ],
    },
];

/// The Inspect tab.
pub(super) const INSPECT: &[Group] = &[Group {
    key: "ui.ribbon.measure",
    commands: &["body.measure"],
}];

/// Commands of the app menu (behind the logo).
pub(super) const APP_MENU: &[&str] = &[
    "doc.new",
    "doc.open",
    "doc.save",
    "file.export_step",
    "file.export_stl",
];

/// Commands of the top bar.
pub(super) const TOP_BAR: &[&str] = &["edit.undo", "edit.redo"];

/// Size of a ribbon button, points.
const BUTTON: egui::Vec2 = egui::vec2(68.0, 52.0);

/// A large ribbon button: icon above label. Returns whether it was clicked.
pub(super) fn big_button(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    (enabled, active): (bool, bool),
    (accent, text): (Color32, Color32),
    tip: &str,
) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(BUTTON, sense);
    if active {
        ui.painter()
            .rect_filled(rect, 6.0, accent.gamma_multiply(0.22));
    } else if response.hovered() && enabled {
        ui.painter()
            .rect_filled(rect, 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let color = if !enabled {
        text.gamma_multiply(0.35)
    } else if active {
        accent
    } else {
        text
    };
    let icon_rect = egui::Rect::from_center_size(
        rect.center_top() + egui::vec2(0.0, 18.0),
        egui::vec2(24.0, 24.0),
    );
    icons::paint(ui.painter(), icon_rect, icon, color);
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 9.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.5),
        color,
    );
    let response = if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    };
    enabled && response.clicked()
}

fn group_frame(ui: &mut Ui, caption: &str, weak: Color32, body: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            body(ui);
        });
        ui.label(RichText::new(caption).size(10.5).color(weak));
    });
    ui.separator();
}

impl State {
    /// Draws the ribbon: tab strip and the active tab's groups.
    pub(super) fn ribbon(&mut self, ui: &mut Ui) {
        let p = self.theme.palette;
        let sketching = self.sketch_mode.is_some();
        ui.horizontal(|ui| {
            if sketching {
                ui.label(
                    RichText::new(self.tr("ui.ribbon.sketch_tab"))
                        .strong()
                        .color(p.accent),
                );
            } else {
                for (tab, key) in [
                    (RibbonTab::Model, "ui.ribbon.model"),
                    (RibbonTab::Inspect, "ui.ribbon.inspect"),
                ] {
                    if ui
                        .selectable_label(self.ribbon_tab == tab, self.tr(key))
                        .clicked()
                    {
                        self.ribbon_tab = tab;
                    }
                }
            }
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if sketching {
                self.sketch_ribbon(ui);
                return;
            }
            let groups = match self.ribbon_tab {
                RibbonTab::Model => MODEL,
                RibbonTab::Inspect => INSPECT,
            };
            let relevant: Vec<&'static str> = self.relevant_commands();
            for g in groups {
                let caption = self.tr(g.key).to_owned();
                group_frame(ui, &caption, p.text_weak, |ui| {
                    for id in g.commands {
                        if let Some(spec) = self.session.registry().get(id).copied() {
                            self.ribbon_command(ui, &spec, relevant.contains(&spec.id));
                        }
                    }
                });
            }
        });
    }

    /// One registry command as a ribbon button.
    fn ribbon_command(&mut self, ui: &mut Ui, spec: &CommandSpec, relevant: bool) {
        let p = self.theme.palette;
        let enablement = self.enabled(spec);
        let mut tip = spec.description.to_owned();
        if let Some(s) = spec.shortcut {
            tip = format!("{tip}\n{}", super::display_shortcut(s));
        }
        if let Enablement::Disabled(reason) = enablement {
            tip = format!("{tip}\n({reason})");
        }
        let label = self.tr(spec.label).to_owned();
        let icon = spec.icon.filter(|i| icons::exists(i)).unwrap_or("box");
        if big_button(
            ui,
            icon,
            &label,
            (enablement == Enablement::Enabled, relevant),
            (p.accent, p.text),
            &tip,
        ) {
            self.invoke(spec);
        }
    }

    /// The contextual Sketch tab.
    fn sketch_ribbon(&mut self, ui: &mut Ui) {
        let p = self.theme.palette;
        let Some(mode) = self.sketch_mode.as_ref() else {
            return;
        };
        let (tool, construction) = (mode.tool, mode.construction);
        let feature = mode.feature;
        let sketch = self
            .session
            .model()
            .sketches
            .get(&feature)
            .map(|s| s.sketch.clone());
        let i18n = self.i18n;
        let mut pick = None;
        let mut toggle = false;
        let mut apply = None;
        let mut finish = false;
        group_frame(ui, i18n.tr("ui.ribbon.draw"), p.text_weak, |ui| {
            for t in Tool::ALL.iter().filter(|t| **t != Tool::Dimension) {
                let tip = format!("{:?}", t.key());
                if big_button(
                    ui,
                    t.icon(),
                    i18n.tr(t.label_key()),
                    (true, tool == *t),
                    (p.accent, p.text),
                    &tip,
                ) {
                    pick = Some(*t);
                }
            }
            if big_button(
                ui,
                "construction",
                i18n.tr("ui.sketch.construction"),
                (true, construction),
                (p.accent, p.text),
                "X",
            ) {
                toggle = true;
            }
        });
        group_frame(ui, i18n.tr("ui.ribbon.constraints"), p.text_weak, |ui| {
            for kind in ConstraintKind::ALL {
                let command = match (&sketch, self.sketch_mode.as_ref()) {
                    (Some(s), Some(m)) => m.constraint_command(kind, s),
                    _ => None,
                };
                if big_button(
                    ui,
                    kind.icon(),
                    i18n.tr(kind.label_key()),
                    (command.is_some(), false),
                    (p.accent, p.text),
                    i18n.tr("ui.sketch.constraint_hint"),
                ) {
                    apply = command;
                }
            }
        });
        group_frame(ui, i18n.tr("ui.ribbon.dimension"), p.text_weak, |ui| {
            if big_button(
                ui,
                "dimension",
                i18n.tr("ui.sketch.dimension"),
                (true, tool == Tool::Dimension),
                (p.accent, p.text),
                "D",
            ) {
                pick = Some(Tool::Dimension);
            }
        });
        group_frame(ui, i18n.tr("ui.ribbon.finish"), p.text_weak, |ui| {
            if big_button(
                ui,
                "finish",
                i18n.tr("ui.sketch.finish"),
                (true, false),
                (p.ok, p.ok),
                "Esc",
            ) {
                finish = true;
            }
        });
        if let Some(mode) = self.sketch_mode.as_mut() {
            if let Some(t) = pick {
                mode.set_tool(t);
            }
            if toggle {
                mode.construction = !mode.construction;
            }
        }
        if let Some((id, params)) = apply {
            self.execute(id, params);
        }
        if finish {
            self.exit_sketch();
        }
    }

    /// Commands that fit the current selection (highlighted in the ribbon).
    pub(super) fn relevant_commands(&self) -> Vec<&'static str> {
        let sel = self.session.selection();
        if self.region.is_some() {
            vec!["solid.extrude"]
        } else if !sel.edges.is_empty() {
            vec!["edge.fillet", "edge.chamfer"]
        } else if !sel.faces.is_empty() {
            vec!["face.push_pull", "body.shell", "sketch.create"]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Every command a person runs from the UI has exactly one home.
    #[test]
    fn every_ui_command_has_exactly_one_home() {
        let session = kubit_engine::Session::new(Box::new(|| {
            Box::new(kubit_kernel_api::testing::BoxKernel::new())
        }));
        let mut homes: BTreeMap<&str, usize> = BTreeMap::new();
        let ribbon = MODEL.iter().chain(INSPECT).flat_map(|g| g.commands.iter());
        for id in ribbon.chain(APP_MENU).chain(TOP_BAR) {
            *homes.entry(id).or_default() += 1;
        }
        for spec in session.registry().iter() {
            let user_facing = spec.toolbar_group.is_some() || spec.icon.is_some();
            let count = homes.get(spec.id).copied().unwrap_or(0);
            if user_facing {
                assert_eq!(
                    count, 1,
                    "{} must appear exactly once, found {count}",
                    spec.id
                );
            }
        }
        for id in homes.keys() {
            assert!(
                session.registry().get(id).is_some(),
                "{id} is not a registered command"
            );
        }
    }
}
