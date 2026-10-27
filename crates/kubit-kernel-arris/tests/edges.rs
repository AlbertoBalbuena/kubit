//! B-rep edges: mesh polylines and the faces that meet at each edge.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_kernel_api::{
    BodyId, BooleanOp, BoxSpec, CylinderSpec, FaceId, Guarded, Kernel, KernelError,
};
use kubit_kernel_arris::ArrisKernel;
use kubit_mesh::EdgeTag;
use std::collections::BTreeSet;
use std::f64::consts::PI;

fn block(k: &mut Guarded<ArrisKernel>) -> BodyId {
    k.make_box(&BoxSpec {
        min: [0.0; 3],
        max: [100.0, 60.0, 40.0],
    })
    .unwrap()
    .body
}

fn top(k: &Guarded<ArrisKernel>, body: BodyId) -> FaceId {
    k.faces(body)
        .unwrap()
        .into_iter()
        .find(|&f| k.face_info(f).unwrap().normal == Some([0.0, 0.0, 1.0]))
        .unwrap()
}

fn length(line: &[[f64; 3]]) -> f64 {
    line.windows(2)
        .map(|w| {
            (0..3)
                .map(|i| (w[1][i] - w[0][i]).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .sum()
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

#[test]
fn box_mesh_has_twelve_edges_of_exact_length() {
    let mut k = Guarded::new(ArrisKernel::new());
    let b = block(&mut k);
    let mesh = k.tessellate(b, 0.1).unwrap();
    let tags: BTreeSet<u64> = mesh.edges.iter().map(|(t, _)| t.0).collect();
    let ids: BTreeSet<u64> = k.edges(b).unwrap().into_iter().map(|e| e.0).collect();
    assert_eq!(tags, ids);
    assert_eq!(mesh.edges.len(), 12);
    let total: f64 = mesh.edges.iter().map(|(_, l)| length(l)).sum();
    assert!(
        (total - 4.0 * (100.0 + 60.0 + 40.0)).abs() < 1e-9,
        "{total}"
    );
    // Polylines run between the B-rep end points.
    for e in k.edges(b).unwrap() {
        let line = mesh.edge_polyline(EdgeTag(e.0)).unwrap();
        let [p, q] = k.edge_endpoints(e).unwrap();
        let (first, last) = (line[0], line[line.len() - 1]);
        let fwd = dist(first, p) + dist(last, q);
        let rev = dist(first, q) + dist(last, p);
        assert!(fwd.min(rev) < 1e-9);
    }
}

#[test]
fn circular_edges_follow_the_hole() {
    let mut k = Guarded::new(ArrisKernel::new());
    let b = block(&mut k);
    let tool = k
        .make_cylinder(&CylinderSpec {
            base: [50.0, 30.0, -1.0],
            axis: [0.0, 0.0, 1.0],
            radius: 10.0,
            height: 42.0,
        })
        .unwrap()
        .body;
    let body = k.boolean(BooleanOp::Difference, b, tool).unwrap().body;
    let mesh = k.tessellate(body, 0.01).unwrap();
    assert!(mesh.validate().is_ok());
    // Every point of a curved polyline lies on the hole's circle (r = 10).
    let curved: Vec<_> = mesh
        .edges
        .iter()
        .filter(|(_, l)| {
            l.iter().any(|p| {
                (p[0] - 50.0).abs() > 1e-6
                    && (p[0] - 50.0).abs() < 10.0 + 1e-6
                    && (p[1] - 30.0).abs() > 1e-6
            })
        })
        .collect();
    assert!(!curved.is_empty());
    let mut circumference = 0.0;
    for (_, line) in &curved {
        for p in line.iter() {
            let r = ((p[0] - 50.0).powi(2) + (p[1] - 30.0).powi(2)).sqrt();
            assert!((r - 10.0).abs() < 1e-6, "r = {r}");
        }
        circumference += length(line);
    }
    // Top and bottom circles; the chord polygon is a little shorter.
    let exact = 2.0 * 2.0 * PI * 10.0;
    assert!(
        circumference < exact && circumference > 0.99 * exact,
        "{circumference}"
    );
}

#[test]
fn edge_faces_are_the_two_neighbours() {
    let mut k = Guarded::new(ArrisKernel::new());
    let b = block(&mut k);
    let faces = k.faces(b).unwrap();
    let mut pairs = BTreeSet::new();
    for e in k.edges(b).unwrap() {
        let [f, g] = k.edge_faces(b, e).unwrap();
        assert_ne!(f, g);
        assert!(faces.contains(&f) && faces.contains(&g));
        // The end points lie on both planes.
        let [p, q] = k.edge_endpoints(e).unwrap();
        for face in [f, g] {
            let info = k.face_info(face).unwrap();
            let n = info.normal.unwrap();
            for pt in [p, q] {
                let d: f64 = (0..3).map(|i| n[i] * (pt[i] - info.centroid[i])).sum();
                assert!(d.abs() < 1e-9);
            }
        }
        pairs.insert((f.min(g), f.max(g)));
    }
    assert_eq!(pairs.len(), 12);
}

#[test]
fn shared_edges_answer_for_the_body_asked() {
    // Push/pull keeps the bottom edges shared with the input body, but their
    // side faces are new: the answer must depend on the body.
    let mut k = Guarded::new(ArrisKernel::new());
    let b = block(&mut k);
    let t = k.offset_faces(b, &[top(&k, b)], 10.0).unwrap().body;
    let (fb, ft) = (k.faces(b).unwrap(), k.faces(t).unwrap());
    let shared: Vec<_> = k
        .edges(t)
        .unwrap()
        .into_iter()
        .filter(|e| k.edges(b).unwrap().contains(e))
        .collect();
    assert!(!shared.is_empty());
    for e in shared {
        assert!(k.edge_faces(t, e).unwrap().iter().all(|f| ft.contains(f)));
        assert!(k.edge_faces(b, e).unwrap().iter().all(|f| fb.contains(f)));
    }
}

#[test]
fn unknown_edges_are_errors() {
    let mut k = Guarded::new(ArrisKernel::new());
    let b = block(&mut k);
    let other = block(&mut k);
    let e = k.edges(b).unwrap()[0];
    assert!(matches!(
        k.edge_faces(other, e),
        Err(KernelError::NotFound { .. })
    ));
    assert!(k.edge_faces(b, kubit_kernel_api::EdgeId(u64::MAX)).is_err());
    assert!(k.edge_faces(BodyId(u64::MAX), e).is_err());
    assert!(!k.is_poisoned());
}
