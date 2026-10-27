//! Theme tokens: every colour, spacing and radius the UI uses lives here.
//!
//! Widgets read tokens from [`Theme`]; nothing else in the crate writes a colour
//! literal. [`Theme::apply`] maps the tokens onto egui's style, and
//! [`Theme::render_style`] onto the viewport renderer.

use egui::{Color32, CornerRadius, Margin, Stroke};
use kubit_render::{RenderStyle, Srgba};

/// Light or dark appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    /// Dark (default).
    #[default]
    Dark,
    /// Light.
    Light,
}

/// Colour tokens (sRGB).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Window and panel background.
    pub panel: Color32,
    /// Background behind panels (dock gaps, menu bar).
    pub base: Color32,
    /// Resting widget fill.
    pub widget: Color32,
    /// Hovered widget fill.
    pub widget_hover: Color32,
    /// Hairlines and separators.
    pub border: Color32,
    /// Primary text.
    pub text: Color32,
    /// Secondary text (hints, captions).
    pub text_weak: Color32,
    /// Accent (selection, primary buttons, focus).
    pub accent: Color32,
    /// Error text.
    pub error: Color32,
    /// Success / OK badges.
    pub ok: Color32,
    /// Viewport background.
    pub viewport: Color32,
    /// Shaded face base colour.
    pub face: Color32,
    /// Feature-edge colour.
    pub edge: Color32,
    /// Hovered-face highlight.
    pub hover: Color32,
    /// Selected-face highlight.
    pub selected: Color32,
}

/// Spacing tokens, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    /// Tight gaps (icon to label).
    pub xs: f32,
    /// Between related widgets.
    pub sm: f32,
    /// Panel padding.
    pub md: f32,
    /// Between groups.
    pub lg: f32,
}

/// Corner radius tokens, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    /// Buttons and fields.
    pub sm: u8,
    /// Popovers and windows.
    pub md: u8,
}

/// The full set of tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Which appearance these tokens are.
    pub mode: ThemeMode,
    /// Colours.
    pub palette: Palette,
    /// Spacing.
    pub spacing: Spacing,
    /// Radii.
    pub radii: Radii,
}

const SPACING: Spacing = Spacing {
    xs: 4.0,
    sm: 6.0,
    md: 10.0,
    lg: 16.0,
};

const RADII: Radii = Radii { sm: 4, md: 8 };

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    /// Tokens for `mode`.
    pub const fn new(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    /// Dark tokens.
    pub const fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            palette: Palette {
                panel: rgb(0x1c1e23),
                base: rgb(0x15171b),
                widget: rgb(0x2a2d34),
                widget_hover: rgb(0x353943),
                border: rgb(0x30333b),
                text: rgb(0xe3e6eb),
                text_weak: rgb(0x9097a3),
                accent: rgb(0x2f8fff),
                error: rgb(0xff6b6b),
                ok: rgb(0x5ccf8f),
                viewport: rgb(0x24272e),
                face: rgb(0xb8c0cc),
                edge: rgb(0x101216),
                hover: rgb(0x7cc4ff),
                selected: rgb(0x2f8fff),
            },
            spacing: SPACING,
            radii: RADII,
        }
    }

    /// Light tokens.
    pub const fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            palette: Palette {
                panel: rgb(0xf6f7f9),
                base: rgb(0xe9ebef),
                widget: rgb(0xe2e5ea),
                widget_hover: rgb(0xd5d9e0),
                border: rgb(0xd0d4db),
                text: rgb(0x1d2026),
                text_weak: rgb(0x626a77),
                accent: rgb(0x1f6fe0),
                error: rgb(0xc62828),
                ok: rgb(0x1e8a4c),
                viewport: rgb(0xdfe3e9),
                face: rgb(0xc4cad3),
                edge: rgb(0x2a2e36),
                hover: rgb(0x4aa8ff),
                selected: rgb(0x1f6fe0),
            },
            spacing: SPACING,
            radii: RADII,
        }
    }

    /// Applies the tokens to egui's style.
    pub fn apply(&self, ctx: &egui::Context) {
        let p = &self.palette;
        let mut visuals = match self.mode {
            ThemeMode::Dark => egui::Visuals::dark(),
            ThemeMode::Light => egui::Visuals::light(),
        };
        visuals.panel_fill = p.panel;
        visuals.window_fill = p.panel;
        visuals.extreme_bg_color = p.base;
        visuals.faint_bg_color = p.widget;
        visuals.override_text_color = Some(p.text);
        visuals.hyperlink_color = p.accent;
        visuals.error_fg_color = p.error;
        visuals.window_stroke = Stroke::new(1.0, p.border);
        visuals.window_corner_radius = CornerRadius::same(self.radii.md);
        visuals.menu_corner_radius = CornerRadius::same(self.radii.md);
        visuals.selection.bg_fill = p.accent;
        visuals.selection.stroke = Stroke::new(1.0, p.text);
        let w = &mut visuals.widgets;
        for (state, fill) in [
            (&mut w.inactive, p.widget),
            (&mut w.hovered, p.widget_hover),
            (&mut w.active, p.widget_hover),
            (&mut w.open, p.widget_hover),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.corner_radius = CornerRadius::same(self.radii.sm);
        }
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.corner_radius = CornerRadius::same(self.radii.sm);
        w.hovered.bg_stroke = Stroke::new(1.0, p.accent);

        ctx.set_visuals(visuals);
        ctx.all_styles_mut(|style| {
            let s = &mut style.spacing;
            s.item_spacing = egui::vec2(self.spacing.sm, self.spacing.sm);
            s.button_padding = egui::vec2(self.spacing.md, self.spacing.xs);
            s.window_margin = Margin::same(self.spacing.md as i8);
            s.menu_margin = Margin::same(self.spacing.sm as i8);
        });
    }

    /// Viewport colours for the renderer.
    pub fn render_style(&self) -> RenderStyle {
        let c = |c: Color32| Srgba::from_rgba8(c.r(), c.g(), c.b(), c.a());
        let p = &self.palette;
        RenderStyle {
            background: c(p.viewport),
            face: c(p.face),
            hover: c(p.hover),
            selected: c(p.selected),
            edge: c(p.edge),
            ..RenderStyle::default()
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_render_style_matches_renderer_defaults() {
        assert_eq!(Theme::dark().render_style(), RenderStyle::default());
    }

    #[test]
    fn modes_differ() {
        assert_ne!(Theme::dark().palette, Theme::light().palette);
        assert_eq!(Theme::new(ThemeMode::Light).mode, ThemeMode::Light);
    }
}
