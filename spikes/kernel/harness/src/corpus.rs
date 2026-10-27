//! The 16-test corpus from `docs/research/tecnologia-rust-cad.md` §1.4, split into
//! sub-cases with fixed parameters and analytic oracles. All lengths in mm.

use std::f64::consts::PI;

/// Base box used by most cases: `BOX_A × BOX_B × BOX_C`, min corner at the origin.
pub const BOX_A: f64 = 100.0;
pub const BOX_B: f64 = 60.0;
pub const BOX_C: f64 = 40.0;
/// Hole radius for cases 1 and 6, centred at (50, 30).
pub const HOLE_R: f64 = 10.0;
/// Blind hole depth (case 1b), measured down from the top face.
pub const BLIND_DEPTH: f64 = 20.0;
/// Edge fillet radius (cases 5a, 5c, 9).
pub const FILLET_R: f64 = 5.0;
/// Variable fillet radii along the 100 mm edge (case 5d).
pub const VAR_R0: f64 = 2.0;
pub const VAR_R1: f64 = 6.0;
/// Stadium prism for the tangent-chain fillet (case 5b).
pub const STADIUM_LS: f64 = 60.0;
pub const STADIUM_R: f64 = 20.0;
pub const STADIUM_H: f64 = 30.0;
/// Hole-edge fillet radius (case 6).
pub const HOLE_FILLET_R: f64 = 2.0;
/// Chamfer distance and angle (case 7).
pub const CHAMFER_D: f64 = 5.0;
pub const CHAMFER_ANGLE_DEG: f64 = 30.0;
/// Shell thickness (case 8).
pub const SHELL_T: f64 = 2.0;
/// Cylinder for case 8b: radius, height, top-edge fillet.
pub const CYL8_R: f64 = 20.0;
pub const CYL8_H: f64 = 40.0;
pub const CYL8_FILLET: f64 = 5.0;
/// Push/pull distance (case 9).
pub const PUSH_D: f64 = 10.0;
/// Plate and pins (case 4).
pub const PLATE: [f64; 3] = [200.0, 200.0, 10.0];
pub const PIN_R: f64 = 4.0;
/// Pins span z ∈ [PIN_Z0, PIN_Z1]; they are embedded 5 mm into the plate.
pub const PIN_Z0: f64 = 5.0;
pub const PIN_Z1: f64 = 30.0;
pub const PIN_PITCH: f64 = 20.0;
/// Helix sweep (case 10a): a circle in the meridian plane (it contains the axis),
/// carried by a screw motion. Profile radius, helix radius, pitch, turns.
/// Volume by Pappus for screw motion: πr²·2πR·turns (no self-overlap: 2r < pitch).
pub const HELIX_PROFILE_R: f64 = 2.0;
pub const HELIX_R: f64 = 20.0;
pub const HELIX_PITCH: f64 = 10.0;
pub const HELIX_TURNS: f64 = 3.0;
/// Loft between circles (case 10b, frustum) and rectangle→circle (case 10c).
pub const LOFT_R0: f64 = 20.0;
pub const LOFT_R1: f64 = 10.0;
pub const LOFT_H: f64 = 30.0;
pub const LOFT_RECT: [f64; 2] = [40.0, 20.0];
/// Revolve profiles touching the axis (case 11).
pub const REV_SPHERE_R: f64 = 20.0;
pub const REV_CONE_R: f64 = 20.0;
pub const REV_CONE_H: f64 = 30.0;
/// Sphere used by the tessellation cases (13a/13b).
pub const TESS_SPHERE_R: f64 = 20.0;

/// Pin centres for case 4: a 10×10 grid with `PIN_PITCH` spacing, starting at 10 mm.
pub fn pin_centres() -> Vec<[f64; 2]> {
    let mut out = Vec::with_capacity(100);
    for i in 0..10 {
        for j in 0..10 {
            out.push([10.0 + PIN_PITCH * f64::from(i), 10.0 + PIN_PITCH * f64::from(j)]);
        }
    }
    out
}

/// What the harness compares a result against.
#[derive(Clone, Copy, Debug, Default)]
pub struct Oracle {
    pub volume: Option<f64>,
    pub area: Option<f64>,
    /// Expected genus of the result (for the Euler–Poincaré check).
    pub genus: Option<i64>,
    /// Expected number of B-rep faces, when it is meaningful (e.g. coplanar merge).
    pub faces: Option<usize>,
    /// For tessellation cases: analytic sphere radius to measure chordal deviation.
    pub sphere_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub id: &'static str,
    /// Test number 1..=16 in §1.4.
    pub test: u8,
    pub title: &'static str,
    pub oracle: Oracle,
    /// Whether case 12 round-trips this case through STEP.
    pub step_roundtrip: bool,
}

/// Cross-section area removed by a 90° rolling-ball fillet of radius `r`.
pub fn fillet_section_area(r: f64) -> f64 {
    (1.0 - PI / 4.0) * r * r
}

/// Distance from the sharp corner to the centroid of the removed fillet section,
/// measured along either leg (into the material).
pub fn fillet_section_centroid(r: f64) -> f64 {
    r * (5.0 / 6.0 - PI / 4.0) / (1.0 - PI / 4.0)
}

/// Volume removed by filleting the outer circular edge of a cylinder of radius `big_r`.
pub fn fillet_ring_removed(big_r: f64, r: f64) -> f64 {
    fillet_section_area(r) * 2.0 * PI * (big_r - fillet_section_centroid(r))
}

/// Volume of a cylinder with its top outer edge filleted.
pub fn filleted_cylinder_volume(big_r: f64, h: f64, r: f64) -> f64 {
    PI * big_r * big_r * h - fillet_ring_removed(big_r, r)
}

pub fn box_volume() -> f64 {
    BOX_A * BOX_B * BOX_C
}

pub fn box_area() -> f64 {
    2.0 * (BOX_A * BOX_B + BOX_B * BOX_C + BOX_C * BOX_A)
}

pub fn helix_length() -> f64 {
    HELIX_TURNS * ((2.0 * PI * HELIX_R).powi(2) + HELIX_PITCH.powi(2)).sqrt()
}

/// Every sub-case, in report order. Case 12 (STEP), 14 (wasm), 15 (hostile) and
/// 16 (history) are driven separately by the runner; they are listed here only
/// through `step_roundtrip` and the dedicated ids below.
pub fn cases() -> Vec<Case> {
    let a = BOX_A;
    let b = BOX_B;
    let c = BOX_C;
    let r = HOLE_R;
    let fr = FILLET_R;
    let vb = box_volume();
    let ab = box_area();
    let sa = fillet_section_area(fr);
    let tan = (CHAMFER_ANGLE_DEG.to_radians()).tan();
    let pins_v = PLATE[0] * PLATE[1] * PLATE[2] + 100.0 * PI * PIN_R * PIN_R * (PIN_Z1 - PLATE[2]);
    let stadium_v = STADIUM_H * (2.0 * STADIUM_R * STADIUM_LS + PI * STADIUM_R * STADIUM_R);
    let stadium_rm = sa * (2.0 * STADIUM_LS + 2.0 * PI * (STADIUM_R - fillet_section_centroid(fr)));
    let corner_rm = sa * (a + b + c - 3.0 * fr) + fr.powi(3) * (1.0 - PI / 6.0);
    let var_rm = (1.0 - PI / 4.0) * a * (VAR_R0 * VAR_R0 + VAR_R0 * VAR_R1 + VAR_R1 * VAR_R1) / 3.0;
    let hole_v = vb - PI * r * r * c;
    let hole_fillet_rm = fillet_section_area(HOLE_FILLET_R) * 2.0 * PI * (r + fillet_section_centroid(HOLE_FILLET_R));
    let t = SHELL_T;
    let shell_box = vb - (a - 2.0 * t) * (b - 2.0 * t) * (c - t);
    let shell_cyl = filleted_cylinder_volume(CYL8_R, CYL8_H, CYL8_FILLET)
        - filleted_cylinder_volume(CYL8_R - t, CYL8_H - t, CYL8_FILLET - t);
    let top_area = a * b - (4.0 - PI) * fr * fr;
    let filleted_v = vb - 4.0 * sa * c;
    let frustum = PI * LOFT_H / 3.0 * (LOFT_R0 * LOFT_R0 + LOFT_R0 * LOFT_R1 + LOFT_R1 * LOFT_R1);
    let o = |volume: f64, genus: i64| Oracle { volume: Some(volume), genus: Some(genus), ..Oracle::default() };

    vec![
        Case { id: "1a", test: 1, title: "box − through cylinder", oracle: Oracle { area: Some(ab - 2.0 * PI * r * r + 2.0 * PI * r * c), ..o(hole_v, 1) }, step_roundtrip: true },
        Case { id: "1b", test: 1, title: "box − blind cylinder", oracle: Oracle { area: Some(ab + 2.0 * PI * r * BLIND_DEPTH), ..o(vb - PI * r * r * BLIND_DEPTH, 0) }, step_roundtrip: true },
        Case { id: "2a", test: 2, title: "union of boxes sharing a face", oracle: Oracle { faces: Some(6), area: Some(2.0 * (2.0 * a * b + b * c + c * 2.0 * a)), ..o(2.0 * vb, 0) }, step_roundtrip: true },
        Case { id: "2b", test: 2, title: "union of boxes touching along an edge", oracle: Oracle { volume: Some(2.0 * vb), ..Oracle::default() }, step_roundtrip: false },
        Case { id: "3a", test: 3, title: "cylinder tangent to a face (union)", oracle: Oracle { volume: Some(vb + PI * r * r * a), ..Oracle::default() }, step_roundtrip: false },
        Case { id: "3b", test: 3, title: "sphere tangent to a box (union)", oracle: Oracle { volume: Some(vb + 4.0 / 3.0 * PI * r.powi(3)), ..Oracle::default() }, step_roundtrip: false },
        Case { id: "4", test: 4, title: "plate ∪ 100 pins", oracle: o(pins_v, 0), step_roundtrip: true },
        Case { id: "5a", test: 5, title: "fillet one straight edge", oracle: o(vb - sa * a, 0), step_roundtrip: true },
        Case { id: "5b", test: 5, title: "fillet a tangent edge chain (stadium top loop)", oracle: o(stadium_v - stadium_rm, 0), step_roundtrip: true },
        Case { id: "5c", test: 5, title: "fillet 3 edges at a box corner (vertex blend)", oracle: o(vb - corner_rm, 0), step_roundtrip: true },
        Case { id: "5d", test: 5, title: "variable-radius fillet 2→6 mm", oracle: o(vb - var_rm, 0), step_roundtrip: true },
        Case { id: "6", test: 6, title: "fillet the hole edge after a boolean", oracle: o(hole_v - hole_fillet_rm, 1), step_roundtrip: true },
        Case { id: "7a", test: 7, title: "chamfer distance–distance", oracle: o(vb - CHAMFER_D * CHAMFER_D / 2.0 * a, 0), step_roundtrip: true },
        Case { id: "7b", test: 7, title: "chamfer distance–angle (30°)", oracle: o(vb - CHAMFER_D * CHAMFER_D * tan / 2.0 * a, 0), step_roundtrip: true },
        Case { id: "8a", test: 8, title: "shell box 2 mm, top open", oracle: o(shell_box, 0), step_roundtrip: true },
        Case { id: "8b", test: 8, title: "shell filleted cylinder 2 mm, bottom open", oracle: o(shell_cyl, 0), step_roundtrip: true },
        Case { id: "9a", test: 9, title: "push/pull top face +10 on vertically-filleted box", oracle: o(filleted_v + top_area * PUSH_D, 0), step_roundtrip: true },
        Case { id: "9b", test: 9, title: "push/pull top face −10 on vertically-filleted box", oracle: o(filleted_v - top_area * PUSH_D, 0), step_roundtrip: true },
        Case { id: "10a", test: 10, title: "sweep circle (meridian plane) along helix", oracle: Oracle { volume: Some(PI * HELIX_PROFILE_R.powi(2) * 2.0 * PI * HELIX_R * HELIX_TURNS), ..Oracle::default() }, step_roundtrip: true },
        Case { id: "10b", test: 10, title: "loft circle→circle (frustum)", oracle: o(frustum, 0), step_roundtrip: true },
        Case { id: "10c", test: 10, title: "loft rectangle→circle", oracle: Oracle { genus: Some(0), ..Oracle::default() }, step_roundtrip: true },
        Case { id: "11a", test: 11, title: "revolve semicircle on axis → sphere", oracle: o(4.0 / 3.0 * PI * REV_SPHERE_R.powi(3), 0), step_roundtrip: true },
        Case { id: "11b", test: 11, title: "revolve triangle on axis → cone", oracle: o(PI * REV_CONE_R * REV_CONE_R * REV_CONE_H / 3.0, 0), step_roundtrip: true },
        Case { id: "13a", test: 13, title: "tessellate sphere, chord tol 0.01", oracle: Oracle { sphere_radius: Some(TESS_SPHERE_R), genus: Some(0), ..Oracle::default() }, step_roundtrip: false },
        Case { id: "13b", test: 13, title: "tessellate sphere, chord tol 0.1", oracle: Oracle { sphere_radius: Some(TESS_SPHERE_R), genus: Some(0), ..Oracle::default() }, step_roundtrip: false },
        Case { id: "13c", test: 13, title: "tessellate case 1a (watertight)", oracle: Oracle { genus: Some(1), ..Oracle::default() }, step_roundtrip: false },
    ]
}

/// Chord tolerance the runner asks for in each case.
pub fn tessellation_tol(case_id: &str) -> f64 {
    match case_id {
        "13b" => 0.1,
        _ => 0.01,
    }
}

pub fn find(id: &str) -> Option<Case> {
    cases().into_iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillet_section_centroid_matches_numeric_integration() {
        // Integrate the region [0,r]² outside the quarter disc centred at (r,r).
        let r = 5.0;
        let n = 2000;
        let h = r / f64::from(n);
        let (mut area, mut mx) = (0.0, 0.0);
        for i in 0..n {
            for j in 0..n {
                let x = (f64::from(i) + 0.5) * h;
                let y = (f64::from(j) + 0.5) * h;
                if (x - r).powi(2) + (y - r).powi(2) > r * r {
                    area += h * h;
                    mx += x * h * h;
                }
            }
        }
        assert!((area - fillet_section_area(r)).abs() / area < 1e-3);
        assert!((mx / area - fillet_section_centroid(r)).abs() < 1e-2);
    }

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::BTreeSet<_> = cases().iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), cases().len());
    }

    #[test]
    fn hundred_pins() {
        assert_eq!(pin_centres().len(), 100);
    }
}
