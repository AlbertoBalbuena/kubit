//! Mesh-based measurements shared by all backends.

use crate::corpus::Oracle;
use crate::{Topo, TriMesh};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MeshReport {
    pub triangles: usize,
    pub volume: f64,
    pub area: f64,
    /// Edges used by exactly one triangle after welding (0 = watertight).
    pub boundary_edges: usize,
    /// Edges used by more than two triangles.
    pub nonmanifold_edges: usize,
    /// Directed edges that appear twice (inconsistent orientation).
    pub misoriented_edges: usize,
    /// Max |‖p‖ − R| over vertices and triangle centroids, when a sphere oracle is given.
    pub max_sphere_deviation: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Evaluation {
    pub mesh: Option<MeshReport>,
    /// "native" or "mesh".
    pub volume_method: Option<String>,
    pub volume: Option<f64>,
    pub volume_rel_err: Option<f64>,
    pub area: Option<f64>,
    pub area_rel_err: Option<f64>,
    /// V − E + F − (L − F) == 2 (S − G), when loops and genus are known.
    pub euler_ok: Option<bool>,
    pub faces_ok: Option<bool>,
    /// Combined topological validity verdict (see `evaluate`).
    pub valid: Option<bool>,
}

/// Welds vertices closer than `weld` (grid quantisation) and returns new indices.
fn weld(mesh: &TriMesh, weld: f64) -> Vec<u32> {
    let mut map: BTreeMap<[i64; 3], u32> = BTreeMap::new();
    let mut out = Vec::with_capacity(mesh.positions.len());
    for p in &mesh.positions {
        // Truncation to a grid cell is the point of the cast.
        #[allow(clippy::cast_possible_truncation)]
        let key = [(p[0] / weld).round() as i64, (p[1] / weld).round() as i64, (p[2] / weld).round() as i64];
        let next = u32::try_from(map.len()).unwrap_or(u32::MAX);
        out.push(*map.entry(key).or_insert(next));
    }
    out
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

pub fn mesh_report(mesh: &TriMesh, sphere_radius: Option<f64>) -> MeshReport {
    let ids = weld(mesh, 1e-6);
    let mut volume = 0.0;
    let mut area = 0.0;
    let mut undirected: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let mut directed: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let mut max_dev: f64 = 0.0;
    let pos = |i: u32| mesh.positions.get(i as usize).copied().unwrap_or([f64::NAN; 3]);
    for t in &mesh.triangles {
        let (p0, p1, p2) = (pos(t[0]), pos(t[1]), pos(t[2]));
        volume += dot(p0, cross(p1, p2)) / 6.0;
        area += norm(cross(sub(p1, p0), sub(p2, p0))) / 2.0;
        let w = [t[0], t[1], t[2]].map(|i| ids.get(i as usize).copied().unwrap_or(u32::MAX));
        for k in 0..3 {
            let (a, b) = (w[k], w[(k + 1) % 3]);
            if a == b {
                continue;
            }
            *directed.entry((a, b)).or_insert(0) += 1;
            *undirected.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
        if let Some(r) = sphere_radius {
            let c = [(p0[0] + p1[0] + p2[0]) / 3.0, (p0[1] + p1[1] + p2[1]) / 3.0, (p0[2] + p1[2] + p2[2]) / 3.0];
            for p in [p0, p1, p2, c] {
                max_dev = max_dev.max((norm(p) - r).abs());
            }
        }
    }
    MeshReport {
        triangles: mesh.triangles.len(),
        volume,
        area,
        boundary_edges: undirected.values().filter(|&&n| n == 1).count(),
        nonmanifold_edges: undirected.values().filter(|&&n| n > 2).count(),
        misoriented_edges: directed.values().filter(|&&n| n > 1).count(),
        max_sphere_deviation: sphere_radius.map(|_| max_dev),
    }
}

fn rel(value: f64, reference: f64) -> f64 {
    if reference.abs() > 0.0 {
        (value - reference).abs() / reference.abs()
    } else {
        value.abs()
    }
}

/// Combines backend topology, mesh measurements and the oracle into one verdict.
pub fn evaluate(
    mesh: Option<&TriMesh>,
    topo: Option<&Topo>,
    native_volume: Option<f64>,
    native_area: Option<f64>,
    oracle: &Oracle,
) -> Evaluation {
    let mesh_rep = mesh.map(|m| mesh_report(m, oracle.sphere_radius));
    let (volume, method) = match (native_volume, &mesh_rep) {
        (Some(v), _) => (Some(v), Some("native".to_owned())),
        (None, Some(m)) => (Some(m.volume), Some("mesh".to_owned())),
        _ => (None, None),
    };
    let area = native_area.or(mesh_rep.as_ref().map(|m| m.area));
    let euler_ok = match (topo, oracle.genus) {
        (Some(t), Some(g)) => t.loops.map(|l| {
            let lhs = t.vertices as i64 - t.edges as i64 + t.faces as i64 - (l as i64 - t.faces as i64);
            lhs == 2 * (t.shells as i64 - g)
        }),
        _ => None,
    };
    let faces_ok = match (topo, oracle.faces) {
        (Some(t), Some(f)) => Some(t.faces == f),
        _ => None,
    };
    let watertight = mesh_rep.as_ref().map(|m| m.boundary_edges == 0 && m.nonmanifold_edges == 0 && m.misoriented_edges == 0);
    let backend_ok = topo.and_then(|t| match (t.backend_closed, t.backend_valid) {
        (None, None) => None,
        (c, v) => Some(c.unwrap_or(true) && v.unwrap_or(true)),
    });
    let positive = volume.map(|v| v > 0.0);
    let verdicts = [watertight, backend_ok, euler_ok, positive];
    let valid = if verdicts.iter().all(Option::is_none) { None } else { Some(verdicts.iter().flatten().all(|&ok| ok)) };
    Evaluation {
        volume_rel_err: volume.zip(oracle.volume).map(|(v, o)| rel(v, o)),
        area_rel_err: area.zip(oracle.area).map(|(a, o)| rel(a, o)),
        mesh: mesh_rep,
        volume_method: method,
        volume,
        area,
        euler_ok,
        faces_ok,
        valid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_cube() -> TriMesh {
        let positions = vec![
            [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4],
            [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7],
        ];
        TriMesh { positions, triangles }
    }

    #[test]
    fn cube_is_watertight_with_unit_volume() {
        let r = mesh_report(&unit_cube(), None);
        assert!((r.volume - 1.0).abs() < 1e-12);
        assert!((r.area - 6.0).abs() < 1e-12);
        assert_eq!((r.boundary_edges, r.nonmanifold_edges, r.misoriented_edges), (0, 0, 0));
    }

    #[test]
    fn missing_triangle_is_detected() {
        let mut m = unit_cube();
        m.triangles.pop();
        assert_eq!(mesh_report(&m, None).boundary_edges, 3);
    }

    #[test]
    fn flipped_triangle_is_detected() {
        let mut m = unit_cube();
        m.triangles[0] = [0, 1, 2];
        assert!(mesh_report(&m, None).misoriented_edges > 0);
    }

    #[test]
    fn euler_check_on_box_topology() {
        let topo = Topo { vertices: 8, edges: 12, faces: 6, loops: Some(6), shells: 1, ..Topo::default() };
        let oracle = Oracle { genus: Some(0), volume: Some(1.0), ..Oracle::default() };
        let e = evaluate(Some(&unit_cube()), Some(&topo), None, None, &oracle);
        assert_eq!(e.euler_ok, Some(true));
        assert_eq!(e.valid, Some(true));
        assert!(e.volume_rel_err.unwrap_or(1.0) < 1e-12);
    }
}
