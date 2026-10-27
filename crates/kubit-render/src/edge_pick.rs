//! Screen-space edge picking: project B-rep edge polylines and take the one nearest
//! the cursor.
//!
//! Edges are 1-pixel lines, too thin for the id pass, so they are picked on the CPU
//! against a pixel tolerance. Occlusion is the caller's job: it passes only the
//! edges that can be visible (for instance those bounding the face under the cursor).

use crate::math;
use crate::{Camera, Viewport};

/// Default pick tolerance, physical pixels.
pub const EDGE_PICK_TOLERANCE_PX: f64 = 6.0;

/// Physical pixel (origin top-left) of world point `p`, or `None` when it is behind
/// the eye or not finite.
pub fn project_to_pixel(camera: &Camera, viewport: Viewport, p: [f64; 3]) -> Option<[f64; 2]> {
    project_with(&camera.view_projection(viewport), viewport, p)
}

fn project_with(view_proj: &math::Mat4, viewport: Viewport, p: [f64; 3]) -> Option<[f64; 2]> {
    let [x, y, _, w] = math::transform4(view_proj, [p[0], p[1], p[2], 1.0]);
    if !(w.is_finite() && w > 1e-12) {
        return None;
    }
    let px = viewport.ndc_to_pixel([x / w, y / w]);
    px.iter().all(|c| c.is_finite()).then_some(px)
}

fn segment_distance(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
    ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
}

/// Screen distance from `cursor_px` to a polyline, pixels; `None` if no segment of it
/// projects.
pub fn polyline_distance_px(
    camera: &Camera,
    viewport: Viewport,
    points: &[[f64; 3]],
    cursor_px: [f64; 2],
) -> Option<f64> {
    let view_proj = camera.view_projection(viewport);
    let projected: Vec<Option<[f64; 2]>> = points
        .iter()
        .map(|&p| project_with(&view_proj, viewport, p))
        .collect();
    let mut best: Option<f64> = None;
    let mut consider = |d: f64| {
        if d.is_finite() && best.is_none_or(|b| d < b) {
            best = Some(d);
        }
    };
    if let [Some(only)] = projected.as_slice() {
        consider(segment_distance(*only, *only, cursor_px));
    }
    for pair in projected.windows(2) {
        if let [Some(a), Some(b)] = pair {
            consider(segment_distance(*a, *b, cursor_px));
        }
    }
    best
}

/// The candidate nearest `cursor_px` within `tolerance_px`, with its distance.
///
/// Ties keep the first candidate, so a deterministic iteration order gives a
/// deterministic pick.
pub fn nearest_edge<'a, K: Copy>(
    camera: &Camera,
    viewport: Viewport,
    cursor_px: [f64; 2],
    tolerance_px: f64,
    candidates: impl IntoIterator<Item = (K, &'a [[f64; 3]])>,
) -> Option<(K, f64)> {
    let mut best: Option<(K, f64)> = None;
    for (key, points) in candidates {
        let Some(d) = polyline_distance_px(camera, viewport, points, cursor_px) else {
            continue;
        };
        if d <= tolerance_px && best.is_none_or(|(_, b)| d < b) {
            best = Some((key, d));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StandardView;

    fn top_camera() -> (Camera, Viewport) {
        let mut camera = Camera::default();
        camera.set_view(StandardView::Top);
        camera.target = [0.0; 3];
        (camera, Viewport::new(800, 600))
    }

    #[test]
    fn origin_projects_to_the_centre_from_the_top() {
        let (camera, viewport) = top_camera();
        let Some([x, y]) = project_to_pixel(&camera, viewport, [0.0; 3]) else {
            panic!("origin must project");
        };
        assert!(
            (x - 400.0).abs() < 1e-6 && (y - 300.0).abs() < 1e-6,
            "{x} {y}"
        );
        let Some([x1, _]) = project_to_pixel(&camera, viewport, [10.0, 0.0, 0.0]) else {
            panic!("+X must project");
        };
        assert!(x1 > x, "+X is to the right from the top");
    }

    #[test]
    fn nearest_edge_respects_tolerance_and_order() {
        let (camera, viewport) = top_camera();
        let a = [[-50.0, 0.0, 0.0], [50.0, 0.0, 0.0]];
        let b = [[-50.0, 0.0, 0.0], [50.0, 0.0, 0.0]];
        let far = [[-50.0, 1000.0, 0.0], [50.0, 1000.0, 0.0]];
        let candidates = [(1u32, &a[..]), (2, &b[..]), (3, &far[..])];
        let hit = nearest_edge(&camera, viewport, [400.0, 302.0], 6.0, candidates);
        assert_eq!(hit.map(|(k, _)| k), Some(1), "tie keeps the first");
        let miss = nearest_edge(&camera, viewport, [400.0, 200.0], 6.0, candidates);
        assert_eq!(miss, None);
    }

    #[test]
    fn points_behind_the_eye_and_hostile_input_are_skipped() {
        let (camera, viewport) = top_camera();
        let eye = camera.eye();
        let behind = [eye[0], eye[1], eye[2] + 10.0];
        assert_eq!(project_to_pixel(&camera, viewport, behind), None);
        let nan = [[f64::NAN; 3], [f64::INFINITY, 0.0, 0.0]];
        assert_eq!(
            polyline_distance_px(&camera, viewport, &nan, [400.0, 300.0]),
            None
        );
        assert_eq!(
            polyline_distance_px(&camera, viewport, &[], [400.0, 300.0]),
            None
        );
    }
}
