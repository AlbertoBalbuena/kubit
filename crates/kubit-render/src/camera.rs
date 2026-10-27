//! Orbit camera: pure math, input-agnostic, no GPU types.
//!
//! World is Z-up, millimetres. The camera orbits `target` at `distance` with a
//! turntable `yaw` (around +Z) and `pitch` (elevation). Clip space follows wgpu:
//! x, y in [-1, 1] (y up), depth in [0, 1].

use crate::math::{self, Mat4, Vec3};
use kubit_mesh::Aabb;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

/// Smallest and largest allowed orbit distance, mm.
const DISTANCE_RANGE: (f64, f64) = (1e-6, 1e9);
/// Fit leaves this much margin around the bounding sphere.
const FIT_MARGIN: f64 = 1.1;

/// A viewport size in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    /// Width, px (≥ 1).
    pub width: u32,
    /// Height, px (≥ 1).
    pub height: u32,
}

impl Viewport {
    /// A viewport; zero sizes are clamped to 1.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }

    /// Width / height.
    pub fn aspect(&self) -> f64 {
        f64::from(self.width.max(1)) / f64::from(self.height.max(1))
    }

    /// Pixel position (origin top-left, y down; pixel centres at +0.5) to NDC (y up).
    pub fn pixel_to_ndc(&self, x: f64, y: f64) -> [f64; 2] {
        [
            2.0 * x / f64::from(self.width.max(1)) - 1.0,
            1.0 - 2.0 * y / f64::from(self.height.max(1)),
        ]
    }

    /// Inverse of [`Viewport::pixel_to_ndc`].
    pub fn ndc_to_pixel(&self, ndc: [f64; 2]) -> [f64; 2] {
        [
            (ndc[0] + 1.0) * 0.5 * f64::from(self.width.max(1)),
            (1.0 - ndc[1]) * 0.5 * f64::from(self.height.max(1)),
        ]
    }
}

/// Projection kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    /// Perspective with the camera's vertical field of view.
    Perspective,
    /// Orthographic; the visible height equals the perspective height at the target,
    /// so toggling keeps the apparent size of the object.
    Orthographic,
}

/// Standard views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    /// Looking down −Z (X right, Y up on screen).
    Top,
    /// Looking up +Z from below.
    Bottom,
    /// Looking along +Y (X right, Z up).
    Front,
    /// Looking along −Y.
    Back,
    /// Looking along −X (Y right, Z up).
    Right,
    /// Looking along +X.
    Left,
    /// From (+X, −Y, +Z), the usual isometric.
    Iso,
}

/// An orbit camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Orbit centre, mm.
    pub target: Vec3,
    /// Distance from eye to target, mm.
    pub distance: f64,
    /// Azimuth around +Z, rad. 0 puts the eye on +X.
    pub yaw: f64,
    /// Elevation, rad, clamped to [−π/2, π/2].
    pub pitch: f64,
    /// Vertical field of view, rad (also sets the orthographic scale).
    pub fov_y: f64,
    /// Perspective or orthographic.
    pub projection: Projection,
    /// Radius of the scene, mm, used to place the clip planes.
    pub scene_radius: f64,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera = Self {
            target: [0.0; 3],
            distance: 500.0,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: 40f64.to_radians(),
            projection: Projection::Perspective,
            scene_radius: 100.0,
        };
        camera.set_view(StandardView::Iso);
        camera
    }
}

/// The camera frame: unit vectors in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Screen right.
    pub right: Vec3,
    /// Screen up.
    pub up: Vec3,
    /// From target towards the eye (the view direction is `-back`).
    pub back: Vec3,
}

impl Camera {
    /// Camera frame from yaw and pitch. Well defined at the poles (no world-up cross product).
    pub fn frame(&self) -> Frame {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let back = [cp * cy, cp * sy, sp];
        let right = [-sy, cy, 0.0];
        let up = math::cross(back, right);
        Frame { right, up, back }
    }

    /// Eye position, mm.
    pub fn eye(&self) -> Vec3 {
        math::add(self.target, math::scale(self.frame().back, self.distance))
    }

    /// Visible height at the target plane, mm.
    pub fn view_height(&self) -> f64 {
        2.0 * self.distance * (0.5 * self.fov_y).tan()
    }

    /// Near and far clip distances along the view direction.
    pub fn clip_planes(&self) -> (f64, f64) {
        let reach = 4.0 * self.scene_radius.max(self.distance);
        match self.projection {
            Projection::Perspective => (self.distance * 1e-2, self.distance + reach),
            Projection::Orthographic => (self.distance - reach, self.distance + reach),
        }
    }

    /// World → view matrix (right-handed, camera looks down −Z).
    pub fn view_matrix(&self) -> Mat4 {
        let Frame { right, up, back } = self.frame();
        let eye = self.eye();
        [
            [right[0], up[0], back[0], 0.0],
            [right[1], up[1], back[1], 0.0],
            [right[2], up[2], back[2], 0.0],
            [
                -math::dot(right, eye),
                -math::dot(up, eye),
                -math::dot(back, eye),
                1.0,
            ],
        ]
    }

    /// View → clip matrix for `aspect` (width / height), depth in [0, 1].
    pub fn projection_matrix(&self, aspect: f64) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let (near, far) = self.clip_planes();
        match self.projection {
            Projection::Perspective => {
                let f = 1.0 / (0.5 * self.fov_y).tan();
                let range = far - near;
                [
                    [f / aspect, 0.0, 0.0, 0.0],
                    [0.0, f, 0.0, 0.0],
                    [0.0, 0.0, -far / range, -1.0],
                    [0.0, 0.0, -far * near / range, 0.0],
                ]
            }
            Projection::Orthographic => {
                let half_h = 0.5 * self.view_height();
                let half_w = half_h * aspect;
                let range = far - near;
                [
                    [1.0 / half_w, 0.0, 0.0, 0.0],
                    [0.0, 1.0 / half_h, 0.0, 0.0],
                    [0.0, 0.0, -1.0 / range, 0.0],
                    [0.0, 0.0, -near / range, 1.0],
                ]
            }
        }
    }

    /// World → clip matrix.
    pub fn view_projection(&self, viewport: Viewport) -> Mat4 {
        math::mul(
            &self.projection_matrix(viewport.aspect()),
            &self.view_matrix(),
        )
    }

    /// World → clip matrix that maps the point under `cursor_px` to the whole of a
    /// 1×1 target, so picking renders a single pixel (a "pick matrix").
    pub fn pick_projection(&self, viewport: Viewport, cursor_px: [f64; 2]) -> Mat4 {
        let [cx, cy] = viewport.pixel_to_ndc(cursor_px[0], cursor_px[1]);
        let sx = f64::from(viewport.width.max(1));
        let sy = f64::from(viewport.height.max(1));
        // x' = sx·(x − cx·w), y' = sy·(y − cy·w): the pixel around the cursor fills NDC.
        let pick: Mat4 = [
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-sx * cx, -sy * cy, 0.0, 1.0],
        ];
        math::mul(&pick, &self.view_projection(viewport))
    }

    /// Ray through an NDC point: origin on the near plane and unit direction.
    pub fn ray(&self, viewport: Viewport, ndc: [f64; 2]) -> Option<(Vec3, Vec3)> {
        let inv = math::invert(&self.view_projection(viewport))?;
        let near = math::project_point(&inv, [ndc[0], ndc[1], 0.0])?;
        let far = math::project_point(&inv, [ndc[0], ndc[1], 1.0])?;
        Some((near, math::normalize(math::sub(far, near))?))
    }

    /// Turntable orbit around the target. Non-finite input is ignored.
    pub fn orbit(&mut self, d_yaw: f64, d_pitch: f64) {
        if !(d_yaw.is_finite() && d_pitch.is_finite()) {
            return;
        }
        self.yaw = (self.yaw + d_yaw).rem_euclid(std::f64::consts::TAU);
        self.pitch = (self.pitch + d_pitch).clamp(-FRAC_PI_2, FRAC_PI_2);
    }

    /// Pans so the scene follows a cursor drag of (`dx_px`, `dy_px`) (y down).
    pub fn pan(&mut self, dx_px: f64, dy_px: f64, viewport: Viewport) {
        if !(dx_px.is_finite() && dy_px.is_finite()) {
            return;
        }
        let mm_per_px = self.view_height() / f64::from(viewport.height.max(1));
        let Frame { right, up, .. } = self.frame();
        let shift = math::add(
            math::scale(right, -dx_px * mm_per_px),
            math::scale(up, dy_px * mm_per_px),
        );
        self.target = math::add(self.target, shift);
    }

    /// Dollies by `factor` (> 1 zooms in) keeping the point under `cursor_ndc`, on the
    /// target plane, fixed on screen. Non-finite or non-positive factors are ignored.
    pub fn zoom(&mut self, factor: f64, cursor_ndc: [f64; 2], viewport: Viewport) {
        if !(factor.is_finite() && factor > 0.0 && cursor_ndc.iter().all(|c| c.is_finite())) {
            return;
        }
        let new_distance = (self.distance / factor).clamp(DISTANCE_RANGE.0, DISTANCE_RANGE.1);
        let applied = self.distance / new_distance;
        let half_h = 0.5 * self.view_height();
        let half_w = half_h * viewport.aspect();
        let Frame { right, up, .. } = self.frame();
        let offset = math::add(
            math::scale(right, cursor_ndc[0] * half_w),
            math::scale(up, cursor_ndc[1] * half_h),
        );
        self.target = math::add(self.target, math::scale(offset, 1.0 - 1.0 / applied));
        self.distance = new_distance;
    }

    /// Frames `aabb` so all of it is visible for the given aspect, keeping the view direction.
    pub fn fit(&mut self, aabb: &Aabb, aspect: f64) {
        let ok = aabb
            .min
            .iter()
            .chain(aabb.max.iter())
            .all(|c| c.is_finite());
        if !ok {
            return;
        }
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let radius = (0.5 * aabb.diagonal()).max(1e-3);
        let half_fov_y = 0.5 * self.fov_y;
        // Narrowest half-angle (horizontal when the viewport is taller than wide).
        let half_fov = (half_fov_y.tan() * aspect.min(1.0)).atan();
        // sin (sphere tangent, perspective) is stricter than tan (orthographic), so it fits both.
        self.distance =
            (FIT_MARGIN * radius / half_fov.sin()).clamp(DISTANCE_RANGE.0, DISTANCE_RANGE.1);
        self.target = aabb.center();
        self.scene_radius = radius;
    }

    /// Orients the camera to a standard view (target and distance unchanged).
    pub fn set_view(&mut self, view: StandardView) {
        let (yaw, pitch) = match view {
            StandardView::Top => (-FRAC_PI_2, FRAC_PI_2),
            StandardView::Bottom => (-FRAC_PI_2, -FRAC_PI_2),
            StandardView::Front => (-FRAC_PI_2, 0.0),
            StandardView::Back => (FRAC_PI_2, 0.0),
            StandardView::Right => (0.0, 0.0),
            StandardView::Left => (std::f64::consts::PI, 0.0),
            StandardView::Iso => (-FRAC_PI_4, (1.0 / 2f64.sqrt()).atan()),
        };
        self.yaw = yaw.rem_euclid(std::f64::consts::TAU);
        self.pitch = pitch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{invert, mul, project_point};

    fn approx(a: Vec3, b: Vec3, tol: f64) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < tol)
    }

    fn corners(b: &Aabb) -> Vec<Vec3> {
        (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { b.min[0] } else { b.max[0] },
                    if i & 2 == 0 { b.min[1] } else { b.max[1] },
                    if i & 4 == 0 { b.min[2] } else { b.max[2] },
                ]
            })
            .collect()
    }

    #[test]
    fn orbit_keeps_distance_to_target() {
        let mut c = Camera {
            target: [10.0, -5.0, 3.0],
            distance: 250.0,
            ..Camera::default()
        };
        for (dy, dp) in [(0.3, 0.2), (-1.7, 0.9), (4.0, -3.0), (0.01, 2.0)] {
            c.orbit(dy, dp);
            let d = math::length(math::sub(c.eye(), c.target));
            assert!((d - 250.0).abs() < 1e-9, "distance {d}");
            assert!(c.pitch.abs() <= FRAC_PI_2);
        }
    }

    #[test]
    fn orbit_ignores_non_finite_input() {
        let mut c = Camera::default();
        let before = c;
        c.orbit(f64::NAN, 0.1);
        c.pan(f64::INFINITY, 0.0, Viewport::new(100, 100));
        c.zoom(f64::NAN, [0.0, 0.0], Viewport::new(100, 100));
        c.zoom(-2.0, [0.0, 0.0], Viewport::new(100, 100));
        assert_eq!(c, before);
    }

    #[test]
    fn frame_is_orthonormal_everywhere_including_poles() {
        let mut c = Camera::default();
        for view in [StandardView::Top, StandardView::Bottom, StandardView::Iso] {
            c.set_view(view);
            let f = c.frame();
            for v in [f.right, f.up, f.back] {
                assert!((math::length(v) - 1.0).abs() < 1e-12);
            }
            assert!(math::dot(f.right, f.up).abs() < 1e-12);
            assert!(math::dot(f.right, f.back).abs() < 1e-12);
            assert!(math::dot(f.up, f.back).abs() < 1e-12);
            // Right-handed: right × up = back.
            assert!(approx(math::cross(f.right, f.up), f.back, 1e-12));
        }
    }

    #[test]
    fn standard_views_look_along_expected_axes() {
        let mut c = Camera::default();
        let cases = [
            (
                StandardView::Top,
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ),
            (
                StandardView::Front,
                [0.0, -1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            (
                StandardView::Right,
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
        ];
        for (view, back, right, up) in cases {
            c.set_view(view);
            let f = c.frame();
            assert!(approx(f.back, back, 1e-12), "{view:?} back {:?}", f.back);
            assert!(
                approx(f.right, right, 1e-12),
                "{view:?} right {:?}",
                f.right
            );
            assert!(approx(f.up, up, 1e-12), "{view:?} up {:?}", f.up);
        }
        c.set_view(StandardView::Iso);
        let b = c.frame().back;
        let s = 1.0 / 3f64.sqrt();
        assert!(approx(b, [s, -s, s], 1e-12), "iso back {b:?}");
    }

    #[test]
    fn fit_contains_the_box_for_both_projections_and_aspects() {
        let aabb = Aabb {
            min: [-10.0, 0.0, 5.0],
            max: [90.0, 40.0, 25.0],
        };
        for projection in [Projection::Perspective, Projection::Orthographic] {
            for aspect in [0.4, 1.0, 2.5] {
                for view in [StandardView::Iso, StandardView::Top, StandardView::Front] {
                    let mut c = Camera {
                        projection,
                        ..Camera::default()
                    };
                    c.set_view(view);
                    c.fit(&aabb, aspect);
                    let vp = Viewport::new((1000.0 * aspect) as u32, 1000);
                    let m = c.view_projection(vp);
                    for p in corners(&aabb) {
                        let ndc = project_point(&m, p).unwrap();
                        assert!(
                            ndc[0].abs() <= 1.0 && ndc[1].abs() <= 1.0,
                            "{projection:?} {aspect} {view:?}: {ndc:?}"
                        );
                        assert!((0.0..=1.0).contains(&ndc[2]), "depth {}", ndc[2]);
                    }
                }
            }
        }
    }

    #[test]
    fn target_projects_to_screen_centre() {
        let c = Camera::default();
        let ndc = project_point(&c.view_projection(Viewport::new(800, 600)), c.target).unwrap();
        assert!(ndc[0].abs() < 1e-12 && ndc[1].abs() < 1e-12);
    }

    #[test]
    fn view_and_projection_are_invertible() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let c = Camera {
                projection,
                target: [3.0, 4.0, 5.0],
                ..Camera::default()
            };
            let vp = Viewport::new(640, 480);
            let m = c.view_projection(vp);
            let inv = invert(&m).unwrap();
            let id = mul(&inv, &m);
            for (i, col) in id.iter().enumerate() {
                for (j, v) in col.iter().enumerate() {
                    let e = if i == j { 1.0 } else { 0.0 };
                    assert!((v - e).abs() < 1e-9, "{projection:?} [{i}][{j}] = {v}");
                }
            }
            // Round trip a world point through clip space.
            let p = [12.0, -7.0, 30.0];
            let ndc = project_point(&m, p).unwrap();
            let back = project_point(&inv, ndc).unwrap();
            assert!(approx(back, p, 1e-6), "{back:?}");
        }
    }

    #[test]
    fn ray_through_centre_hits_target() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let c = Camera {
                projection,
                ..Camera::default()
            };
            let (o, d) = c.ray(Viewport::new(300, 200), [0.0, 0.0]).unwrap();
            let to_target = math::sub(c.target, o);
            let along = math::dot(to_target, d);
            let closest = math::add(o, math::scale(d, along));
            assert!(approx(closest, c.target, 1e-6), "{projection:?}");
        }
    }

    #[test]
    fn ndc_pixel_mapping_round_trips() {
        let vp = Viewport::new(800, 600);
        assert_eq!(vp.pixel_to_ndc(0.0, 0.0), [-1.0, 1.0]);
        assert_eq!(vp.pixel_to_ndc(800.0, 600.0), [1.0, -1.0]);
        assert_eq!(vp.pixel_to_ndc(400.0, 300.0), [0.0, 0.0]);
        for (x, y) in [(12.5, 33.0), (799.0, 1.0), (400.0, 599.5)] {
            let back = vp.ndc_to_pixel(vp.pixel_to_ndc(x, y));
            assert!((back[0] - x).abs() < 1e-9 && (back[1] - y).abs() < 1e-9);
        }
    }

    #[test]
    fn zoom_keeps_point_under_cursor_fixed() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let mut c = Camera {
                projection,
                ..Camera::default()
            };
            let vp = Viewport::new(800, 600);
            let cursor = [0.5, -0.3];
            // World point on the target plane under the cursor.
            let f = c.frame();
            let half_h = 0.5 * c.view_height();
            let p = math::add(
                c.target,
                math::add(
                    math::scale(f.right, cursor[0] * half_h * vp.aspect()),
                    math::scale(f.up, cursor[1] * half_h),
                ),
            );
            let d0 = c.distance;
            c.zoom(2.0, cursor, vp);
            assert!((c.distance - d0 / 2.0).abs() < 1e-9);
            let ndc = project_point(&c.view_projection(vp), p).unwrap();
            assert!((ndc[0] - cursor[0]).abs() < 1e-9, "{projection:?} {ndc:?}");
            assert!((ndc[1] - cursor[1]).abs() < 1e-9, "{projection:?} {ndc:?}");
        }
    }

    #[test]
    fn pan_moves_target_with_cursor() {
        let mut c = Camera::default();
        let vp = Viewport::new(800, 600);
        let p = c.target;
        let before = project_point(&c.view_projection(vp), p).unwrap();
        c.pan(80.0, -60.0, vp);
        let after = project_point(&c.view_projection(vp), p).unwrap();
        let moved_px = [
            (after[0] - before[0]) * 400.0,
            -(after[1] - before[1]) * 300.0,
        ];
        assert!((moved_px[0] - 80.0).abs() < 1e-6, "{moved_px:?}");
        assert!((moved_px[1] + 60.0).abs() < 1e-6, "{moved_px:?}");
    }

    #[test]
    fn pick_projection_maps_cursor_to_centre() {
        let c = Camera::default();
        let vp = Viewport::new(640, 480);
        let cursor = [100.0, 200.0];
        let world = {
            let ndc = vp.pixel_to_ndc(cursor[0], cursor[1]);
            let (o, d) = c.ray(vp, ndc).unwrap();
            math::add(o, math::scale(d, c.distance))
        };
        let ndc = project_point(&c.pick_projection(vp, cursor), world).unwrap();
        assert!(ndc[0].abs() < 1e-6 && ndc[1].abs() < 1e-6, "{ndc:?}");
    }
}
