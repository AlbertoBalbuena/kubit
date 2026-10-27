//! The ground grid on the world XY plane and the X/Y/Z axes.
//!
//! The grid is rebuilt on the CPU every frame from the camera: its spacing follows
//! the zoom (powers of ten, minor lines every `minor`, major every `10 × minor`), it is
//! centred under the camera target, and every line fades out towards the rim so the
//! grid never shows a hard edge. Pure geometry, so it is unit-tested without a GPU.

use crate::style::{RenderStyle, Srgba};
use crate::{Camera, Viewport};

/// Major lines each side of the centre: the grid spans `2 × HALF_MAJORS` major cells.
const HALF_MAJORS: i64 = 6;
/// Segments per line, so the radial fade interpolates smoothly along it.
const SEGMENTS: usize = 8;
/// Minor lines closer than this on screen are hidden, pixels.
const MIN_MINOR_PX: f64 = 6.0;
/// ... and fully shown from this spacing up, pixels.
const FULL_MINOR_PX: f64 = 14.0;
/// Most vertices [`grid_lines`] ever returns (sizes the GPU buffer).
pub const MAX_GRID_VERTICES: usize =
    ((2 * HALF_MAJORS as usize * 10 + 1) * 2 + 2) * SEGMENTS * 2 + 4 * SEGMENTS;

/// One end of a grid line segment: position (mm) and sRGB colour with alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridVertex {
    /// World position, mm.
    pub position: [f64; 3],
    /// Colour; alpha already includes the fade.
    pub color: Srgba,
}

/// Spacing of the grid for this camera: `(minor, major)` in mm.
///
/// The minor spacing is the power of ten that puts roughly 10–100 minor cells across
/// the visible height at the target.
pub fn grid_spacing(camera: &Camera) -> (f64, f64) {
    let visible = camera.view_height();
    let visible = if visible.is_finite() && visible > 0.0 {
        visible
    } else {
        100.0
    };
    let minor = 10f64
        .powf((visible / 10.0).log10().floor())
        .clamp(1e-3, 1e5);
    (minor, minor * 10.0)
}

fn with_alpha(c: Srgba, alpha: f64) -> Srgba {
    let [r, g, b, a] = c.0;
    Srgba([r, g, b, a * alpha.clamp(0.0, 1.0) as f32])
}

/// Line-list vertices of the grid and the axes for `camera` (two per segment).
pub fn grid_lines(camera: &Camera, viewport: Viewport, style: &RenderStyle) -> Vec<GridVertex> {
    let (minor, major) = grid_spacing(camera);
    let px_per_mm = f64::from(viewport.height.max(1)) / camera.view_height().max(1e-9);
    let minor_px = minor * px_per_mm;
    let minor_alpha = (minor_px - MIN_MINOR_PX) / (FULL_MINOR_PX - MIN_MINOR_PX);
    let minor_alpha = if minor_alpha.is_finite() {
        minor_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let half = HALF_MAJORS as f64 * major;
    let snap = |v: f64| {
        if v.is_finite() {
            (v / major).round() * major
        } else {
            0.0
        }
    };
    let (cx, cy) = (snap(camera.target[0]), snap(camera.target[1]));
    let fade = |x: f64, y: f64| {
        let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / half;
        (1.0 - r).clamp(0.0, 1.0).powi(2)
    };

    let mut out = Vec::with_capacity(MAX_GRID_VERTICES);
    let mut line = |a: [f64; 2], b: [f64; 2], color: Srgba| {
        for i in 0..SEGMENTS {
            let t0 = i as f64 / SEGMENTS as f64;
            let t1 = (i + 1) as f64 / SEGMENTS as f64;
            let p0 = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0];
            let p1 = [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
            for p in [p0, p1] {
                out.push(GridVertex {
                    position: [p[0], p[1], 0.0],
                    color: with_alpha(color, fade(p[0], p[1])),
                });
            }
        }
    };

    let steps = HALF_MAJORS * 10;
    for i in -steps..=steps {
        let offset = i as f64 * minor;
        let is_major = i % 10 == 0;
        if !is_major && minor_alpha <= 0.0 {
            continue;
        }
        let color = if is_major {
            style.grid_major
        } else {
            with_alpha(style.grid_minor, minor_alpha)
        };
        let (x, y) = (cx + offset, cy + offset);
        // The axes replace the grid lines through the origin.
        if x.abs() > minor * 1e-6 {
            line([x, cy - half], [x, cy + half], color);
        }
        if y.abs() > minor * 1e-6 {
            line([cx - half, y], [cx + half, y], color);
        }
    }
    if (cy - half..=cy + half).contains(&0.0) {
        line([cx - half, 0.0], [cx + half, 0.0], style.axis_x);
    }
    if (cx - half..=cx + half).contains(&0.0) {
        line([0.0, cy - half], [0.0, cy + half], style.axis_y);
    }
    // Z spans as far as X and Y, fading with height like they fade with distance.
    let z_fade = |z: f64| (1.0 - z.abs() / half).clamp(0.0, 1.0).powi(2);
    for i in 0..2 * SEGMENTS {
        let z0 = -half + 2.0 * half * i as f64 / (2 * SEGMENTS) as f64;
        let z1 = -half + 2.0 * half * (i + 1) as f64 / (2 * SEGMENTS) as f64;
        for z in [z0, z1] {
            out.push(GridVertex {
                position: [0.0, 0.0, z],
                color: with_alpha(style.axis_z, z_fade(z)),
            });
        }
    }
    out.truncate(MAX_GRID_VERTICES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_follows_zoom_in_powers_of_ten() {
        let mut camera = Camera::default();
        camera.distance = 100.0 / (2.0 * (0.5 * camera.fov_y).tan());
        assert!((camera.view_height() - 100.0).abs() < 1e-9);
        assert_eq!(grid_spacing(&camera), (10.0, 100.0));
        camera.distance *= 10.0;
        assert_eq!(grid_spacing(&camera), (100.0, 1000.0));
    }

    #[test]
    fn grid_has_axes_and_stays_within_budget() {
        let camera = Camera::default();
        let style = RenderStyle::default();
        let lines = grid_lines(&camera, Viewport::new(800, 600), &style);
        assert_eq!(lines.len() % 2, 0);
        assert!(lines.len() <= MAX_GRID_VERTICES);
        assert!(lines.iter().any(|v| v.color.0[..3] == style.axis_x.0[..3]));
        assert!(lines.iter().any(|v| v.color.0[..3] == style.axis_y.0[..3]));
        let z_top = lines.iter().map(|v| v.position[2]).fold(0.0, f64::max);
        let z_bottom = lines.iter().map(|v| v.position[2]).fold(0.0, f64::min);
        let (_, major) = grid_spacing(&camera);
        let half = HALF_MAJORS as f64 * major;
        assert!(z_top >= half * 0.999, "Z reaches as far as X and Y");
        assert!(z_bottom <= -half * 0.999, "Z also runs below the grid");
    }

    #[test]
    fn lines_fade_to_zero_at_the_rim() {
        let camera = Camera::default();
        let lines = grid_lines(&camera, Viewport::new(800, 600), &RenderStyle::default());
        let (_, major) = grid_spacing(&camera);
        let half = HALF_MAJORS as f64 * major;
        let rim = lines
            .iter()
            .filter(|v| v.position[2] == 0.0 && v.position[0].abs() >= half * 0.999)
            .map(|v| v.color.0[3])
            .fold(0.0f32, f32::max);
        assert!(rim < 1e-6, "rim alpha {rim}");
    }

    #[test]
    fn hostile_cameras_do_not_panic() {
        let mut camera = Camera::default();
        for d in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300, 1e-300] {
            camera.distance = d;
            camera.target = [d, -d, d];
            let lines = grid_lines(&camera, Viewport::new(0, 0), &RenderStyle::default());
            assert!(lines.len() <= MAX_GRID_VERTICES);
        }
    }
}
