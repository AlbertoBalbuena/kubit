//! Icons: Kubit's own CAD icons, drawn as vector strokes, and Phosphor glyphs for
//! generic actions.
//!
//! CAD icons are designed on a 24 × 24 grid and painted into any rectangle with the
//! caller's colour, so they stay crisp at every scale and follow the theme. Generic
//! icons (undo, open, search, eye…) come from the Phosphor font (MIT), registered
//! by [`install_fonts`]. [`paint`] draws either kind by name.

use egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};
use egui_phosphor::regular as ph;

/// Registers the Phosphor font as a fallback of the proportional family.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// The Phosphor glyph for a generic icon name.
pub fn glyph(name: &str) -> Option<&'static str> {
    Some(match name {
        "undo" => ph::ARROW_U_UP_LEFT,
        "redo" => ph::ARROW_U_UP_RIGHT,
        "new" => ph::FILE_PLUS,
        "open" => ph::FOLDER_OPEN,
        "save" => ph::FLOPPY_DISK,
        "export" => ph::EXPORT,
        "measure" => ph::RULER,
        "search" => ph::MAGNIFYING_GLASS,
        "eye" => ph::EYE,
        "eye_off" => ph::EYE_SLASH,
        "history" => ph::CLOCK_COUNTER_CLOCKWISE,
        "layers" => ph::STACK,
        "fit" => ph::FRAME_CORNERS,
        "settings" => ph::GEAR,
        "finish" => ph::CHECK,
        "fix" => ph::LOCK,
        "menu" => ph::LIST,
        "sidebar" => ph::SIDEBAR_SIMPLE,
        "first" => ph::SKIP_BACK,
        "last" => ph::SKIP_FORWARD,
        "previous" => ph::CARET_LEFT,
        "next" => ph::CARET_RIGHT,
        "close" => ph::X,
        "theme_light" => ph::SUN,
        "theme_dark" => ph::MOON,
        "language" => ph::GLOBE,
        "body" => ph::CUBE,
        "display" => ph::CUBE_TRANSPARENT,
        _ => return None,
    })
}

/// Names of Kubit's own vector icons.
pub const CAD: &[&str] = &[
    "box",
    "cylinder",
    "sketch",
    "extrude",
    "push_pull",
    "fillet",
    "chamfer",
    "shell",
    "boolean",
    "line",
    "rectangle",
    "circle",
    "arc",
    "dimension",
    "construction",
    "select",
    "plane",
    "horizontal",
    "vertical",
    "parallel",
    "perpendicular",
    "tangent",
    "coincident",
    "midpoint",
    "equal",
];

/// Whether `name` resolves to an icon.
pub fn exists(name: &str) -> bool {
    CAD.contains(&name) || glyph(name).is_some()
}

/// Draws icon `name` centred in `rect`; `false` if there is no such icon.
pub fn paint(painter: &Painter, rect: Rect, name: &str, color: Color32) -> bool {
    if let Some(g) = glyph(name) {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            g,
            FontId::proportional(rect.height() * 0.9),
            color,
        );
        return true;
    }
    let pen = Pen::new(painter, rect, color);
    pen.draw(name)
}

/// Draws on a 24 × 24 grid mapped into a square inside `rect`.
struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    unit: f32,
    color: Color32,
    stroke: Stroke,
}

impl<'a> Pen<'a> {
    fn new(painter: &'a Painter, rect: Rect, color: Color32) -> Self {
        let side = rect.width().min(rect.height());
        let unit = side / 24.0;
        let origin = rect.center() - egui::vec2(side, side) / 2.0;
        Self {
            painter,
            origin,
            unit,
            color,
            stroke: Stroke::new((1.6 * unit).max(1.0), color),
        }
    }

    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + egui::vec2(x, y) * self.unit
    }

    fn line(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter.add(Shape::line(pts, self.stroke));
    }

    fn closed(&self, pts: &[(f32, f32)], fill: bool) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        let fill = if fill {
            self.color.gamma_multiply(0.28)
        } else {
            Color32::TRANSPARENT
        };
        self.painter
            .add(Shape::convex_polygon(pts, fill, self.stroke));
    }

    fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_stroke(self.p(x, y), r * self.unit, self.stroke);
    }

    fn dot(&self, x: f32, y: f32) {
        self.painter
            .circle_filled(self.p(x, y), 1.6 * self.unit, self.color);
    }

    /// Arc around `(x, y)` from `a0` to `a1` degrees (0 = +x, counter-clockwise on
    /// screen means negative y).
    fn arc(&self, x: f32, y: f32, r: f32, a0: f32, a1: f32) {
        let n = 24;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let t = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                (x + r * t.cos(), y - r * t.sin())
            })
            .collect();
        self.line(&pts);
    }

    fn ellipse(&self, x: f32, y: f32, rx: f32, ry: f32, a0: f32, a1: f32) {
        let n = 32;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let t = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                (x + rx * t.cos(), y - ry * t.sin())
            })
            .collect();
        self.line(&pts);
    }

    fn arrow_up(&self, x: f32, y0: f32, y1: f32) {
        self.line(&[(x, y0), (x, y1)]);
        self.closed(
            &[(x, y1 - 2.5), (x - 3.0, y1 + 1.5), (x + 3.0, y1 + 1.5)],
            false,
        );
    }

    /// The isometric box most solid icons start from.
    fn cube(&self, fill_top: bool) {
        self.closed(
            &[(4.0, 8.0), (12.0, 4.0), (20.0, 8.0), (12.0, 12.0)],
            fill_top,
        );
        self.line(&[
            (4.0, 8.0),
            (4.0, 16.0),
            (12.0, 20.0),
            (20.0, 16.0),
            (20.0, 8.0),
        ]);
        self.line(&[(12.0, 12.0), (12.0, 20.0)]);
    }

    fn draw(&self, name: &str) -> bool {
        match name {
            "box" => self.cube(true),
            "cylinder" => {
                self.ellipse(12.0, 7.0, 7.0, 3.0, 0.0, 360.0);
                self.line(&[(5.0, 7.0), (5.0, 17.0)]);
                self.line(&[(19.0, 7.0), (19.0, 17.0)]);
                self.ellipse(12.0, 17.0, 7.0, 3.0, 180.0, 360.0);
            }
            "sketch" => {
                self.line(&[(3.0, 21.0), (21.0, 21.0)]);
                self.closed(&[(6.0, 15.0), (16.0, 5.0), (19.0, 8.0), (9.0, 18.0)], true);
                self.closed(&[(6.0, 15.0), (9.0, 18.0), (4.5, 19.5)], false);
            }
            "extrude" => {
                self.closed(
                    &[(3.0, 17.0), (12.0, 13.0), (21.0, 17.0), (12.0, 21.0)],
                    true,
                );
                self.arrow_up(12.0, 17.0, 5.0);
            }
            "push_pull" => {
                self.closed(
                    &[(4.0, 12.0), (12.0, 8.0), (20.0, 12.0), (12.0, 16.0)],
                    true,
                );
                self.line(&[
                    (4.0, 12.0),
                    (4.0, 17.0),
                    (12.0, 21.0),
                    (20.0, 17.0),
                    (20.0, 12.0),
                ]);
                self.arrow_up(12.0, 11.0, 2.5);
            }
            "fillet" => {
                self.line(&[(4.0, 20.0), (4.0, 13.0)]);
                self.arc(12.0, 13.0, 8.0, 180.0, 90.0);
                self.line(&[(12.0, 5.0), (20.0, 5.0)]);
                self.dot(12.0, 13.0);
            }
            "chamfer" => {
                self.line(&[(4.0, 20.0), (4.0, 12.0)]);
                self.line(&[(4.0, 12.0), (11.0, 5.0)]);
                self.line(&[(11.0, 5.0), (20.0, 5.0)]);
                self.painter.add(Shape::line_segment(
                    [self.p(4.0, 12.0), self.p(11.0, 5.0)],
                    Stroke::new(self.stroke.width * 1.8, self.color),
                ));
            }
            "shell" => {
                self.cube(false);
                self.closed(&[(7.5, 8.0), (12.0, 5.8), (16.5, 8.0), (12.0, 10.2)], true);
            }
            "boolean" => {
                self.circle(9.0, 12.0, 6.0);
                self.circle(15.0, 12.0, 6.0);
                self.painter.add(Shape::convex_polygon(
                    [
                        (12.0, 7.0),
                        (14.0, 9.5),
                        (14.0, 14.5),
                        (12.0, 17.0),
                        (10.0, 14.5),
                        (10.0, 9.5),
                    ]
                    .iter()
                    .map(|(x, y)| self.p(*x, *y))
                    .collect(),
                    self.color.gamma_multiply(0.45),
                    Stroke::NONE,
                ));
            }
            "line" => {
                self.line(&[(5.0, 19.0), (19.0, 5.0)]);
                self.dot(5.0, 19.0);
                self.dot(19.0, 5.0);
            }
            "rectangle" => {
                self.closed(&[(4.0, 6.0), (20.0, 6.0), (20.0, 18.0), (4.0, 18.0)], false);
                for (x, y) in [(4.0, 6.0), (20.0, 6.0), (20.0, 18.0), (4.0, 18.0)] {
                    self.dot(x, y);
                }
            }
            "circle" => {
                self.circle(12.0, 12.0, 8.0);
                self.dot(12.0, 12.0);
            }
            "arc" => {
                self.arc(12.0, 15.0, 8.0, 10.0, 170.0);
                self.dot(12.0, 15.0);
                self.dot(19.9, 13.6);
                self.dot(4.1, 13.6);
            }
            "dimension" => {
                self.line(&[(4.0, 8.0), (4.0, 18.0)]);
                self.line(&[(20.0, 8.0), (20.0, 18.0)]);
                self.line(&[(5.0, 13.0), (19.0, 13.0)]);
                self.closed(&[(5.0, 13.0), (8.0, 11.0), (8.0, 15.0)], true);
                self.closed(&[(19.0, 13.0), (16.0, 11.0), (16.0, 15.0)], true);
            }
            "construction" => {
                for i in 0..4 {
                    let t = i as f32 * 4.0;
                    self.line(&[(4.0 + t, 20.0 - t), (6.5 + t, 17.5 - t)]);
                }
            }
            "select" => {
                self.closed(&[(6.0, 4.0), (18.0, 13.0), (12.5, 13.5), (9.5, 19.5)], true);
            }
            "plane" => {
                self.closed(
                    &[(3.0, 14.0), (13.0, 8.0), (21.0, 11.0), (11.0, 17.0)],
                    true,
                );
            }
            "horizontal" => {
                self.line(&[(4.0, 12.0), (20.0, 12.0)]);
                self.line(&[(4.0, 9.0), (4.0, 15.0)]);
                self.line(&[(20.0, 9.0), (20.0, 15.0)]);
            }
            "vertical" => {
                self.line(&[(12.0, 4.0), (12.0, 20.0)]);
                self.line(&[(9.0, 4.0), (15.0, 4.0)]);
                self.line(&[(9.0, 20.0), (15.0, 20.0)]);
            }
            "parallel" => {
                self.line(&[(5.0, 16.0), (15.0, 6.0)]);
                self.line(&[(9.0, 19.0), (19.0, 9.0)]);
            }
            "perpendicular" => {
                self.line(&[(4.0, 19.0), (20.0, 19.0)]);
                self.line(&[(12.0, 19.0), (12.0, 5.0)]);
                self.line(&[(12.0, 15.0), (16.0, 15.0), (16.0, 19.0)]);
            }
            "tangent" => {
                self.circle(12.0, 14.0, 6.0);
                self.line(&[(3.0, 8.0), (21.0, 8.0)]);
            }
            "coincident" => {
                self.line(&[(4.0, 18.0), (12.0, 12.0)]);
                self.line(&[(12.0, 12.0), (20.0, 16.0)]);
                self.dot(12.0, 12.0);
                self.circle(12.0, 12.0, 3.5);
            }
            "midpoint" => {
                self.line(&[(4.0, 16.0), (20.0, 16.0)]);
                self.closed(&[(12.0, 9.0), (15.0, 14.0), (9.0, 14.0)], true);
            }
            "equal" => {
                self.line(&[(5.0, 9.0), (19.0, 9.0)]);
                self.line(&[(5.0, 15.0), (19.0, 15.0)]);
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_and_unknown_names_do_not() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(24.0, 24.0));
            for name in CAD {
                assert!(paint(&painter, rect, name, Color32::WHITE), "{name}");
            }
            assert!(paint(&painter, rect, "undo", Color32::WHITE));
            assert!(!paint(&painter, rect, "no-such-icon", Color32::WHITE));
        });
        out.textures_delta.clear();
        assert!(!exists("no-such-icon"));
    }

    #[test]
    fn every_registry_icon_exists() {
        let session = kubit_engine::Session::new(Box::new(|| {
            Box::new(kubit_kernel_api::testing::BoxKernel::new())
        }));
        for spec in session.registry().iter() {
            if let Some(icon) = spec.icon {
                assert!(exists(icon), "{} uses missing icon `{icon}`", spec.id);
            }
        }
    }
}
