//! The top bar (logo menu, undo/redo, command search), the command palette and the
//! view controls over the canvas.

use super::ribbon::{APP_MENU, TOP_BAR};
use super::{State, icons};
use crate::i18n::{I18n, Lang};
use crate::theme::{Theme, ThemeMode};
use egui::{Key, Modifiers, RichText, Ui};
use kubit_engine::{CommandSpec, Enablement};
use kubit_render::StandardView;

/// The open command palette.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Palette {
    query: String,
    index: usize,
    focused: bool,
}

/// Whether `query` matches `text` loosely: every query character appears in order.
pub(super) fn fuzzy(query: &str, text: &str) -> bool {
    let mut chars = text.chars().flat_map(char::to_lowercase);
    query
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !c.is_whitespace())
        .all(|q| chars.any(|c| c == q))
}

/// An icon-only button. Returns whether it was clicked.
fn icon_button(ui: &mut Ui, icon: &str, tip: &str, enabled: bool, color: egui::Color32) -> bool {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), sense);
    if response.hovered() && enabled {
        ui.painter()
            .rect_filled(rect, 5.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let color = if enabled {
        color
    } else {
        color.gamma_multiply(0.35)
    };
    icons::paint(ui.painter(), rect.shrink(5.0), icon, color);
    enabled && response.on_hover_text(tip).clicked()
}

impl State {
    /// The top bar: logo menu, document name, undo/redo, search.
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        let p = self.theme.palette;
        ui.horizontal(|ui| {
            let logo = ui.add(
                egui::Button::new(
                    RichText::new(format!("{}  Kubit", icons::glyph("body").unwrap_or("K")))
                        .strong()
                        .color(p.accent),
                )
                .frame(false),
            );
            egui::Popup::menu(&logo).show(|ui| self.app_menu(ui));
            ui.separator();
            ui.label(RichText::new(self.tr("ui.doc.untitled")).color(p.text_weak));
            ui.separator();
            for id in TOP_BAR {
                if let Some(spec) = self.session.registry().get(id).copied() {
                    let enabled = self.enabled(&spec) == Enablement::Enabled;
                    let tip = format!(
                        "{} ({})",
                        self.tr(spec.label),
                        spec.shortcut
                            .map(super::display_shortcut)
                            .unwrap_or_default()
                    );
                    if icon_button(ui, spec.icon.unwrap_or("undo"), &tip, enabled, p.text) {
                        self.invoke(&spec);
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = format!(
                    "{}  {}   S",
                    icons::glyph("search").unwrap_or(""),
                    self.tr("ui.palette.search")
                );
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).color(p.text_weak))
                            .min_size(egui::vec2(220.0, 26.0)),
                    )
                    .clicked()
                {
                    self.palette = Some(Palette::default());
                }
            });
        });
    }

    /// The logo menu: file commands, theme and language.
    fn app_menu(&mut self, ui: &mut Ui) {
        let specs: Vec<CommandSpec> = APP_MENU
            .iter()
            .filter_map(|id| self.session.registry().get(id).copied())
            .collect();
        for spec in &specs {
            let label = format!(
                "{}  {}",
                icons::glyph(spec.icon.unwrap_or("new")).unwrap_or(""),
                self.tr(spec.label)
            );
            let enabled = self.enabled(spec) == Enablement::Enabled;
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                self.invoke(spec);
                ui.close();
            }
        }
        ui.separator();
        let (key, mode, icon) = match self.theme.mode {
            ThemeMode::Dark => ("ui.view.theme_light", ThemeMode::Light, "theme_light"),
            ThemeMode::Light => ("ui.view.theme_dark", ThemeMode::Dark, "theme_dark"),
        };
        if ui
            .button(format!(
                "{}  {}",
                icons::glyph(icon).unwrap_or(""),
                self.tr(key)
            ))
            .clicked()
        {
            self.theme = Theme::new(mode);
            self.theme.apply(ui.ctx());
            ui.close();
        }
        let (lang, label) = match self.i18n.lang {
            Lang::Es => (Lang::En, "English"),
            Lang::En => (Lang::Es, "Español"),
        };
        if ui
            .button(format!(
                "{}  {label}",
                icons::glyph("language").unwrap_or("")
            ))
            .clicked()
        {
            self.i18n = I18n::new(lang);
            ui.close();
        }
    }

    /// Commands the palette offers: every user-facing command.
    fn palette_commands(&self) -> Vec<CommandSpec> {
        self.session
            .registry()
            .iter()
            .filter(|s| s.icon.is_some() || s.toolbar_group.is_some())
            .copied()
            .collect()
    }

    /// Palette matches for `query`, enabled first.
    pub(super) fn palette_matches(&self, query: &str) -> Vec<CommandSpec> {
        let mut out: Vec<CommandSpec> = self
            .palette_commands()
            .into_iter()
            .filter(|s| fuzzy(query, self.tr(s.label)) || fuzzy(query, s.id))
            .collect();
        out.sort_by_key(|s| self.enabled(s) != Enablement::Enabled);
        out
    }

    /// Opens the palette on `S` or Cmd/Ctrl+K and draws it while open.
    pub(super) fn palette(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        if self.palette.is_none()
            && (ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K))
                || (!typing
                    && self.sketch_mode.is_none()
                    && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::S))))
        {
            self.palette = Some(Palette::default());
        }
        let Some(mut pal) = self.palette.take() else {
            return;
        };
        let matches = self.palette_matches(&pal.query);
        let mut run = None;
        let mut close = false;
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("kubit-palette"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(screen.center().x - 220.0, screen.top() + 90.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(440.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut pal.query)
                            .hint_text(self.tr("ui.palette.hint"))
                            .desired_width(f32::INFINITY),
                    );
                    if !pal.focused {
                        edit.request_focus();
                        pal.focused = true;
                    }
                    if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                        pal.index = (pal.index + 1).min(matches.len().saturating_sub(1));
                    }
                    if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                        pal.index = pal.index.saturating_sub(1);
                    }
                    pal.index = pal.index.min(matches.len().saturating_sub(1));
                    for (i, spec) in matches.iter().take(10).enumerate() {
                        let enabled = self.enabled(spec) == Enablement::Enabled;
                        let shortcut = spec
                            .shortcut
                            .map(super::display_shortcut)
                            .unwrap_or_default();
                        let text = format!("{}   {}", self.tr(spec.label), shortcut);
                        let r =
                            ui.add_enabled(enabled, egui::Button::selectable(i == pal.index, text));
                        if r.clicked() {
                            run = Some(*spec);
                        }
                    }
                    if ui.input(|i| i.key_pressed(Key::Enter))
                        && let Some(spec) = matches.get(pal.index)
                        && self.enabled(spec) == Enablement::Enabled
                    {
                        run = Some(*spec);
                    }
                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                });
            });
        if let Some(spec) = run {
            self.invoke(&spec);
        } else if !close {
            self.palette = Some(pal);
        }
    }

    /// View controls over the canvas's top-right corner (below the triad).
    pub(super) fn view_controls(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let p = self.theme.palette;
        let i18n = self.i18n;
        let mut fit = false;
        let mut view = None;
        let mut edges = None;
        egui::Area::new(egui::Id::new("kubit-view-controls"))
            .order(egui::Order::Middle)
            .fade_in(false)
            .fixed_pos(rect.right_top() + egui::vec2(-44.0, 118.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(4.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            if icon_button(
                                ui,
                                "fit",
                                &format!("{} (F)", i18n.tr("ui.view.fit")),
                                true,
                                p.text,
                            ) {
                                fit = true;
                            }
                            let views = egui::Popup::menu(
                                &ui.add(
                                    egui::Button::new(icons::glyph("display").unwrap_or("▣"))
                                        .frame(false),
                                )
                                .on_hover_text(i18n.tr("ui.view.views")),
                            );
                            views.show(|ui| {
                                for (key, v) in [
                                    ("ui.view.iso", StandardView::Iso),
                                    ("ui.view.top", StandardView::Top),
                                    ("ui.view.front", StandardView::Front),
                                    ("ui.view.right", StandardView::Right),
                                ] {
                                    if ui.button(i18n.tr(key)).clicked() {
                                        view = Some(v);
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                if ui
                                    .checkbox(
                                        &mut self.show_edges.clone(),
                                        i18n.tr("ui.view.edges"),
                                    )
                                    .clicked()
                                {
                                    edges = Some(!self.show_edges);
                                }
                            });
                        });
                    });
            });
        if let Some(e) = edges {
            self.show_edges = e;
        }
        if let Some(vp) = &mut self.viewport {
            if fit {
                vp.fit();
            }
            if let Some(v) = view {
                vp.set_view(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_in_order_ignoring_case_and_spaces() {
        assert!(fuzzy("extr", "Extruir"));
        assert!(fuzzy("ext ru", "Extrude"));
        assert!(fuzzy("", "anything"));
        assert!(!fuzzy("ed x", "Extrude"));
        assert!(fuzzy("chaf", "Chaflán"));
    }
}
