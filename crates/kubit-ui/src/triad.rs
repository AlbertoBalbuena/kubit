//! The orientation triad: world X/Y/Z drawn in a corner of the viewport.
//!
//! Each axis has a filled tip (+) and a hollow one (−). Clicking a tip switches to
//! the standard view looking from that side. The projection is the camera's
//! rotation only, so the triad never moves or scales with the scene.

use egui::{Color32, Pos2, Stroke, Vec2};
use kubit_render::{Frame, StandardView};

/// Distance from the triad centre to an axis tip, points.
pub const ARM: f32 = 34.0;
/// Radius of an axis tip, points.
pub const TIP: f32 = 8.0;
/// Inset of the triad centre from the viewport's bottom-left corner, points.
pub const INSET: f32 = ARM + TIP + 10.0;

/// One tip of the triad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tip {
    /// 0 = X, 1 = Y, 2 = Z.
    pub axis: usize,
    /// Positive or negative end.
    pub positive: bool,
    /// Offset from the centre on screen, points (y down).
    pub offset: Vec2,
    /// Towards the viewer (> 0) or away from it.
    pub depth: f32,
}

impl Tip {
    /// The standard view that looks at the model from this tip's side.
    pub fn view(&self) -> StandardView {
        match (self.axis, self.positive) {
            (0, true) => StandardView::Right,
            (0, false) => StandardView::Left,
            (1, true) => StandardView::Back,
            (1, false) => StandardView::Front,
            (_, true) => StandardView::Top,
            (_, false) => StandardView::Bottom,
        }
    }

    /// Label drawn on a positive tip.
    pub fn label(&self) -> &'static str {
        ["X", "Y", "Z"].get(self.axis).copied().unwrap_or("")
    }
}

/// The six tips for a camera frame, sorted back to front (paint order).
pub fn tips(frame: &Frame) -> Vec<Tip> {
    let mut out = Vec::with_capacity(6);
    for axis in 0..3 {
        let mut dir = [0.0; 3];
        if let Some(d) = dir.get_mut(axis) {
            *d = 1.0;
        }
        let dot = |v: [f64; 3]| v[0] * dir[0] + v[1] * dir[1] + v[2] * dir[2];
        let (x, y, z) = (dot(frame.right), dot(frame.up), dot(frame.back));
        for positive in [true, false] {
            let s = if positive { 1.0 } else { -1.0 };
            out.push(Tip {
                axis,
                positive,
                offset: Vec2::new((s * x) as f32 * ARM, (-s * y) as f32 * ARM),
                depth: (s * z) as f32,
            });
        }
    }
    out.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    out
}

/// The tip under `pointer`, frontmost first.
pub fn hit(frame: &Frame, center: Pos2, pointer: Pos2) -> Option<Tip> {
    tips(frame)
        .into_iter()
        .rev()
        .find(|t| (center + t.offset).distance(pointer) <= TIP)
}

/// Paints the triad. `colors` are the X/Y/Z axis colours; `hovered` is highlighted.
pub fn paint(
    painter: &egui::Painter,
    frame: &Frame,
    center: Pos2,
    colors: [Color32; 3],
    text: Color32,
    hovered: Option<Tip>,
) {
    for tip in tips(frame) {
        let color = colors.get(tip.axis).copied().unwrap_or(text);
        let end = center + tip.offset;
        let lit = hovered.is_some_and(|h| h.axis == tip.axis && h.positive == tip.positive);
        let radius = if lit { TIP + 2.0 } else { TIP };
        if tip.positive {
            painter.line_segment([center, end], Stroke::new(2.0, color));
            painter.circle_filled(end, radius, color);
            painter.text(
                end,
                egui::Align2::CENTER_CENTER,
                tip.label(),
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        } else {
            painter.circle(
                end,
                radius * 0.8,
                color.gamma_multiply(0.25),
                Stroke::new(1.5, color),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kubit_render::Camera;

    fn frame(view: StandardView) -> Frame {
        let mut camera = Camera::default();
        camera.set_view(view);
        camera.frame()
    }

    #[test]
    fn top_view_points_x_right_and_y_up() {
        let tips = tips(&frame(StandardView::Top));
        let x = tips.iter().find(|t| t.axis == 0 && t.positive).copied();
        let y = tips.iter().find(|t| t.axis == 1 && t.positive).copied();
        let z = tips.iter().find(|t| t.axis == 2 && t.positive).copied();
        let (Some(x), Some(y), Some(z)) = (x, y, z) else {
            panic!("missing tips");
        };
        assert!(x.offset.x > ARM * 0.99 && x.offset.y.abs() < 1e-3);
        assert!(y.offset.y < -ARM * 0.99, "screen y is down");
        assert!(z.depth > 0.99, "+Z faces the viewer from the top");
    }

    #[test]
    fn tips_paint_back_to_front_and_hit_the_frontmost() {
        let f = frame(StandardView::Iso);
        let tips = tips(&f);
        assert_eq!(tips.len(), 6);
        assert!(tips.windows(2).all(|w| w[0].depth <= w[1].depth));
        let center = Pos2::new(100.0, 100.0);
        let z = tips.iter().find(|t| t.axis == 2 && t.positive).copied();
        let Some(z) = z else { panic!("no +Z") };
        let got = hit(&f, center, center + z.offset);
        assert_eq!(got.map(|t| t.view()), Some(StandardView::Top));
        assert_eq!(hit(&f, center, Pos2::new(0.0, 0.0)), None);
    }
}
