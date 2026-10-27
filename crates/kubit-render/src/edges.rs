//! Feature-edge extraction: the lines drawn over shaded faces.
//!
//! An edge is a feature edge when it bounds a single triangle, is shared by more
//! than two, separates two different B-rep faces, or folds by more than the
//! crease angle. Coincident vertices are welded by exact position first, so
//! meshes tessellated face by face (duplicated seams) work too.

use kubit_mesh::TriMesh;
use std::collections::BTreeMap;

/// Default crease angle, rad (30°).
pub const DEFAULT_CREASE_ANGLE: f64 = std::f64::consts::PI / 6.0;

/// Welding key: the exact bits of a position, with −0.0 folded into +0.0.
fn key(p: [f64; 3]) -> [u64; 3] {
    p.map(|c| if c == 0.0 { 0u64 } else { c.to_bits() })
}

fn normal(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<[f64; 3]> {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    (l.is_finite() && l > 0.0).then(|| n.map(|x| x / l))
}

/// Feature edges of `mesh` as segments (pairs of end points), in a deterministic order.
///
/// Triangles with out-of-range indices are skipped; degenerate triangles contribute
/// no crease information but still count for boundaries.
pub fn feature_edges(mesh: &TriMesh, crease_angle: f64) -> Vec<[[f64; 3]; 2]> {
    let cos_crease = crease_angle.cos();
    // Weld: position key → welded index, plus the welded positions.
    let mut welded: BTreeMap<[u64; 3], u32> = BTreeMap::new();
    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut weld = |p: [f64; 3]| -> u32 {
        *welded.entry(key(p)).or_insert_with(|| {
            points.push(p);
            u32::try_from(points.len() - 1).unwrap_or(u32::MAX)
        })
    };

    // Undirected edge → triangles using it.
    let mut uses: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    let mut normals: Vec<Option<[f64; 3]>> = Vec::with_capacity(mesh.triangles.len());
    for t in 0..mesh.triangles.len() {
        let Some([a, b, c]) = mesh.corners(t) else {
            normals.push(None);
            continue;
        };
        normals.push(normal(a, b, c));
        let (wa, wb, wc) = (weld(a), weld(b), weld(c));
        for (p, q) in [(wa, wb), (wb, wc), (wc, wa)] {
            if p != q {
                uses.entry((p.min(q), p.max(q))).or_default().push(t);
            }
        }
    }

    let tag = |t: usize| mesh.face_tags.get(t).copied();
    let n = |t: usize| normals.get(t).copied().flatten();
    let point = |i: u32| points.get(i as usize).copied();
    uses.into_iter()
        .filter(|(_, tris)| match tris.as_slice() {
            [a, b] => {
                if tag(*a) != tag(*b) {
                    return true;
                }
                match (n(*a), n(*b)) {
                    (Some(na), Some(nb)) => {
                        na[0] * nb[0] + na[1] * nb[1] + na[2] * nb[2] < cos_crease
                    }
                    _ => false,
                }
            }
            _ => true, // boundary or non-manifold
        })
        .filter_map(|((p, q), _)| Some([point(p)?, point(q)?]))
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use kubit_mesh::FaceTag;

    /// Axis-aligned cube [0, s]³, two triangles per face, one tag per face (1..=6).
    /// With `split` every face has its own vertices (as a face-by-face tessellator does).
    pub(crate) fn cube(s: f64, split: bool) -> TriMesh {
        // Faces as (tag, four CCW corners seen from outside).
        let faces: [(u64, [[f64; 3]; 4]); 6] = [
            (1, [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]]), // -Z
            (2, [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]]), // +Z
            (3, [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]]), // -Y (front)
            (4, [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]]), // +Y
            (5, [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]]), // -X
            (6, [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]]), // +X
        ];
        let mut mesh = TriMesh::default();
        let mut index = BTreeMap::new();
        for (tag, quad) in faces {
            let ids: Vec<u32> = quad
                .iter()
                .map(|p| {
                    let p = p.map(|c| c * s);
                    let push = |mesh: &mut TriMesh| {
                        mesh.positions.push(p);
                        (mesh.positions.len() - 1) as u32
                    };
                    if split {
                        push(&mut mesh)
                    } else {
                        *index.entry(key(p)).or_insert_with(|| push(&mut mesh))
                    }
                })
                .collect();
            mesh.triangles.push([ids[0], ids[1], ids[2]]);
            mesh.triangles.push([ids[0], ids[2], ids[3]]);
            mesh.face_tags.extend([FaceTag(tag), FaceTag(tag)]);
        }
        mesh
    }

    #[test]
    fn cube_has_twelve_feature_edges() {
        for split in [false, true] {
            let mesh = cube(10.0, split);
            assert!(mesh.validate().is_ok());
            let edges = feature_edges(&mesh, DEFAULT_CREASE_ANGLE);
            assert_eq!(edges.len(), 12, "split = {split}");
            for [a, b] in &edges {
                let len = ((0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>()).sqrt();
                assert!((len - 10.0).abs() < 1e-12, "edge length {len}");
            }
        }
    }

    #[test]
    fn untagged_cube_still_has_twelve_edges_by_angle() {
        let mut mesh = cube(1.0, false);
        mesh.face_tags.iter_mut().for_each(|t| *t = FaceTag(0));
        assert_eq!(feature_edges(&mesh, DEFAULT_CREASE_ANGLE).len(), 12);
    }

    #[test]
    fn single_triangle_has_three_boundary_edges() {
        let mesh = TriMesh {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            triangles: vec![[0, 1, 2]],
            face_tags: vec![FaceTag(7)],
            edges: Vec::new(),
        };
        assert_eq!(feature_edges(&mesh, DEFAULT_CREASE_ANGLE).len(), 3);
    }

    #[test]
    fn hostile_mesh_does_not_panic() {
        let mesh = TriMesh {
            positions: vec![[f64::NAN; 3], [1.0, 0.0, 0.0]],
            triangles: vec![[0, 1, 9], [0, 0, 0], [0, 1, 1]],
            face_tags: vec![],
            edges: Vec::new(),
        };
        let _ = feature_edges(&mesh, DEFAULT_CREASE_ANGLE);
    }

    #[test]
    fn extraction_is_deterministic() {
        let mesh = cube(3.0, true);
        assert_eq!(
            feature_edges(&mesh, DEFAULT_CREASE_ANGLE),
            feature_edges(&mesh, DEFAULT_CREASE_ANGLE)
        );
    }
}
