//! The push/pull handle: an arrow on the selected face, dragged along its normal.
//!
//! Pure geometry (no egui state), so the drag maths is unit-tested: where the arrow
//! sits on screen, how a pointer movement maps to millimetres along the normal, and
//! the snapping step.

use kubit_mesh::{FaceTag, TriMesh};
use kubit_render::{Camera, Viewport, grid_spacing, project_to_pixel};
use std::collections::BTreeMap;

/// Arrow length on screen, physical pixels per point of UI scale.
pub const ARROW_PX: f64 = 70.0;
/// The arrow is hidden when it projects shorter than this (normal facing the eye).
const MIN_ARROW_PX: f64 = 12.0;
/// Pointer distance that grabs the arrow, physical pixels per point.
pub const GRAB_PX: f64 = 10.0;

/// Where a face sits: area-weighted centroid and unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceFrame {
    /// Centroid, mm.
    pub centroid: [f64; 3],
    /// Outward unit normal (area-weighted; meaningful for planar faces).
    pub normal: [f64; 3],
}

/// Centroid and normal of every face of `mesh`, by tag.
pub fn face_frames(mesh: &TriMesh) -> BTreeMap<FaceTag, FaceFrame> {
    let mut acc: BTreeMap<FaceTag, ([f64; 3], [f64; 3], f64)> = BTreeMap::new();
    for (tri, tag) in mesh.triangles.iter().zip(&mesh.face_tags) {
        let p = |i: u32| mesh.positions.get(i as usize).copied();
        let [i0, i1, i2] = *tri;
        let (Some(a), Some(b), Some(c)) = (p(i0), p(i1), p(i2)) else {
            continue;
        };
        let [ax, ay, az] = a;
        let [bx, by, bz] = b;
        let [cx, cy, cz] = c;
        let (u, v) = ([bx - ax, by - ay, bz - az], [cx - ax, cy - ay, cz - az]);
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let area2 = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if !area2.is_finite() || area2 <= 0.0 {
            continue;
        }
        let centre = [
            (ax + bx + cx) / 3.0,
            (ay + by + cy) / 3.0,
            (az + bz + cz) / 3.0,
        ];
        let entry = acc.entry(*tag).or_insert(([0.0; 3], [0.0; 3], 0.0));
        for (sum, c) in entry.0.iter_mut().zip(centre) {
            *sum += c * area2;
        }
        for (sum, c) in entry.1.iter_mut().zip(n) {
            *sum += c;
        }
        entry.2 += area2;
    }
    acc.into_iter()
        .filter_map(|(tag, (sum, n, area))| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            (area > 0.0 && len > 0.0).then(|| {
                (
                    tag,
                    FaceFrame {
                        centroid: sum.map(|s| s / area),
                        normal: n.map(|c| c / len),
                    },
                )
            })
        })
        .collect()
}

/// The arrow on screen: start and end pixels and its length in mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    /// Base, on the face centroid (physical pixels).
    pub start: [f64; 2],
    /// Tip (physical pixels).
    pub end: [f64; 2],
    /// World length the arrow stands for, mm.
    pub length_mm: f64,
}

/// The arrow of `frame` displaced `offset_mm` along the normal, `arrow_px` long on
/// screen; `None` when it would be too short to grab (normal towards the eye).
pub fn arrow(
    camera: &Camera,
    viewport: Viewport,
    frame: &FaceFrame,
    offset_mm: f64,
    arrow_px: f64,
) -> Option<Arrow> {
    let px_per_mm = f64::from(viewport.height.max(1)) / camera.view_height().max(1e-9);
    let length_mm = arrow_px / px_per_mm;
    let at = |t: f64| {
        let c = frame.centroid;
        let n = frame.normal;
        [c[0] + n[0] * t, c[1] + n[1] * t, c[2] + n[2] * t]
    };
    let start = project_to_pixel(camera, viewport, at(offset_mm))?;
    let end = project_to_pixel(camera, viewport, at(offset_mm + length_mm))?;
    let len = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
    (len.is_finite() && len >= MIN_ARROW_PX && length_mm.is_finite()).then_some(Arrow {
        start,
        end,
        length_mm,
    })
}

/// Millimetres along the normal for a pointer moved from `press` to `now`.
pub fn drag_distance(arrow: &Arrow, press: [f64; 2], now: [f64; 2]) -> f64 {
    let d = [arrow.end[0] - arrow.start[0], arrow.end[1] - arrow.start[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    if !(len2.is_finite() && len2 > 0.0) {
        return 0.0;
    }
    let moved = [now[0] - press[0], now[1] - press[1]];
    let along = (moved[0] * d[0] + moved[1] * d[1]) / len2;
    let mm = along * arrow.length_mm;
    if mm.is_finite() { mm } else { 0.0 }
}

/// Snapping step for the current zoom: a tenth of the minor grid spacing.
pub fn snap_step(camera: &Camera) -> f64 {
    grid_spacing(camera).0 / 10.0
}

/// `mm` rounded to `step` (unchanged for a non-positive step).
pub fn snap(mm: f64, step: f64) -> f64 {
    if step > 0.0 && step.is_finite() {
        (mm / step).round() * step
    } else {
        mm
    }
}

/// Distance from `p` to the segment `a`–`b`, pixels.
pub fn distance_to_segment(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kubit_render::StandardView;

    /// Unit square at z = 1 facing +Z, as two triangles of face 5.
    fn square() -> TriMesh {
        TriMesh {
            positions: vec![
                [0.0, 0.0, 1.0],
                [2.0, 0.0, 1.0],
                [2.0, 2.0, 1.0],
                [0.0, 2.0, 1.0],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            face_tags: vec![FaceTag(5), FaceTag(5)],
            edges: Vec::new(),
        }
    }

    #[test]
    fn frame_is_centroid_and_normal() {
        let frames = face_frames(&square());
        let f = frames.get(&FaceTag(5)).copied();
        assert_eq!(
            f,
            Some(FaceFrame {
                centroid: [1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            })
        );
    }

    #[test]
    fn dragging_along_the_arrow_gives_its_length() {
        let mut camera = Camera::default();
        camera.set_view(StandardView::Front);
        let viewport = Viewport::new(800, 600);
        let frame = FaceFrame {
            centroid: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        };
        let Some(a) = arrow(&camera, viewport, &frame, 0.0, ARROW_PX) else {
            panic!("a +Z arrow is visible from the front");
        };
        assert!(a.end[1] < a.start[1], "+Z points up on screen");
        let full = drag_distance(&a, a.start, a.end);
        assert!((full - a.length_mm).abs() < 1e-6 * a.length_mm.max(1.0));
        let back = drag_distance(&a, a.end, a.start);
        assert!((back + a.length_mm).abs() < 1e-6 * a.length_mm.max(1.0));
        let sideways = drag_distance(&a, a.start, [a.start[0] + 50.0, a.start[1]]);
        assert!(sideways.abs() < 1e-6, "{sideways}");
    }

    #[test]
    fn arrow_towards_the_eye_is_hidden() {
        let mut camera = Camera::default();
        camera.set_view(StandardView::Top);
        let frame = FaceFrame {
            centroid: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        };
        assert_eq!(
            arrow(&camera, Viewport::new(800, 600), &frame, 0.0, ARROW_PX),
            None
        );
    }

    #[test]
    fn snapping() {
        assert_eq!(snap(12.34, 1.0), 12.0);
        assert_eq!(snap(-0.26, 0.5), -0.5);
        assert_eq!(snap(3.3, 0.0), 3.3);
        assert!((distance_to_segment([0.0, 0.0], [10.0, 0.0], [5.0, 3.0]) - 3.0).abs() < 1e-12);
    }
}
