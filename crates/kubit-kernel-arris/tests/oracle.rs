//! Oracle tests: analytic volumes, areas and centroids, through `Guarded`.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_kernel_api::{
    BodyId, BooleanOp, BoxSpec, CylinderSpec, FaceId, Guarded, HistorySource, Kernel, KernelError,
};
use kubit_kernel_arris::ArrisKernel;
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::sync::Arc;

const REL: f64 = 1e-9;

fn kernel() -> Guarded<ArrisKernel> {
    Guarded::new(ArrisKernel::new())
}

fn block(k: &mut Guarded<ArrisKernel>) -> BodyId {
    k.make_box(&BoxSpec {
        min: [0.0; 3],
        max: [100.0, 60.0, 40.0],
    })
    .unwrap()
    .body
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= REL * b.abs().max(1.0)
}

/// The planar face whose outward normal is `n`.
fn face_facing(k: &Guarded<ArrisKernel>, body: BodyId, n: [f64; 3]) -> FaceId {
    k.faces(body)
        .unwrap()
        .into_iter()
        .find(|&f| {
            k.face_info(f).unwrap().normal.is_some_and(|m| {
                (m[0] - n[0]).abs() < 1e-12
                    && (m[1] - n[1]).abs() < 1e-12
                    && (m[2] - n[2]).abs() < 1e-12
            })
        })
        .unwrap()
}

fn box_minus_cylinder(k: &mut Guarded<ArrisKernel>) -> BodyId {
    let b = block(k);
    let tool = k
        .make_cylinder(&CylinderSpec {
            base: [50.0, 30.0, -1.0],
            axis: [0.0, 0.0, 1.0],
            radius: 10.0,
            height: 42.0,
        })
        .unwrap()
        .body;
    k.boolean(BooleanOp::Difference, b, tool).unwrap().body
}

#[test]
fn box_volume_area_centroid() {
    let mut k = kernel();
    let b = block(&mut k);
    let p = k.mass_properties(b).unwrap();
    assert!(p.exact);
    assert!(close(p.volume, 240_000.0), "{}", p.volume);
    assert!(
        close(p.area, 2.0 * (6000.0 + 2400.0 + 4000.0)),
        "{}",
        p.area
    );
    for (c, e) in p.centroid.iter().zip([50.0, 30.0, 20.0]) {
        assert!((c - e).abs() < 1e-9, "{:?}", p.centroid);
    }
    assert_eq!(k.faces(b).unwrap().len(), 6);
    assert_eq!(k.edges(b).unwrap().len(), 12);
    assert!(k.check(b).unwrap().ok);
}

#[test]
fn face_info_is_exact_and_outward() {
    let mut k = kernel();
    let b = block(&mut k);
    let top = face_facing(&k, b, [0.0, 0.0, 1.0]);
    let info = k.face_info(top).unwrap();
    assert!(close(info.area, 6000.0));
    assert!((info.centroid[2] - 40.0).abs() < 1e-9 && (info.centroid[0] - 50.0).abs() < 1e-9);
    let total: f64 = k
        .faces(b)
        .unwrap()
        .into_iter()
        .map(|f| k.face_info(f).unwrap().area)
        .sum();
    assert!(close(total, k.mass_properties(b).unwrap().area));
}

#[test]
fn push_pull_top_face_plus_ten() {
    let mut k = kernel();
    let b = block(&mut k);
    let top = face_facing(&k, b, [0.0, 0.0, 1.0]);
    let out = k.offset_faces(b, &[top], 10.0).unwrap();
    assert_eq!(out.history.source, HistorySource::Native);
    assert!(close(
        k.mass_properties(out.body).unwrap().volume,
        300_000.0
    ));
    assert_eq!(k.faces(out.body).unwrap().len(), 6);
    let pulled = k
        .offset_faces(
            out.body,
            &[face_facing(&k, out.body, [0.0, 0.0, 1.0])],
            -20.0,
        )
        .unwrap();
    assert!(close(
        k.mass_properties(pulled.body).unwrap().volume,
        180_000.0
    ));
}

#[test]
fn box_minus_cylinder_volume_and_history() {
    let mut k = kernel();
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
    let out = k.boolean(BooleanOp::Difference, b, tool).unwrap();
    let v = k.mass_properties(out.body).unwrap().volume;
    assert!(close(v, 240_000.0 - PI * 100.0 * 40.0), "{v}");
    assert_eq!(out.history.source, HistorySource::Native);
    let (generated, modified, deleted) = out.history.counts();
    assert!(generated + modified + deleted > 0, "{:?}", out.history);
    assert_eq!(k.faces(out.body).unwrap().len(), 7);
}

#[test]
fn fillet_chamfer_shell_oracles() {
    let mut k = kernel();
    let b = block(&mut k);
    // The top front edge: along X at y = 0, z = 40.
    let edge = k
        .edges(b)
        .unwrap()
        .into_iter()
        .find(|&e| {
            let [p, q] = k.edge_endpoints(e).unwrap();
            p[1].abs() < 1e-9
                && q[1].abs() < 1e-9
                && (p[2] - 40.0).abs() < 1e-9
                && (q[2] - 40.0).abs() < 1e-9
        })
        .unwrap();
    let r = 5.0;
    let filleted = k.fillet(b, &[edge], r).unwrap().body;
    assert!(close(
        k.mass_properties(filleted).unwrap().volume,
        240_000.0 - (1.0 - PI / 4.0) * r * r * 100.0
    ));
    let chamfered = k.chamfer(b, &[edge], r).unwrap().body;
    assert!(close(
        k.mass_properties(chamfered).unwrap().volume,
        240_000.0 - r * r / 2.0 * 100.0
    ));
    let top = face_facing(&k, b, [0.0, 0.0, 1.0]);
    let cup = k.shell(b, &[top], 2.0).unwrap().body;
    assert!(close(
        k.mass_properties(cup).unwrap().volume,
        240_000.0 - 96.0 * 56.0 * 38.0
    ));
}

#[test]
fn step_round_trip_keeps_volume() {
    let mut k = kernel();
    let body = box_minus_cylinder(&mut k);
    let v0 = k.mass_properties(body).unwrap().volume;
    let text = k.export_step(&[body]).unwrap();
    assert!(text.starts_with("ISO-10303-21;"));
    let mut back = kernel();
    let read = back.import_step(&text).unwrap();
    assert_eq!(read.len(), 1);
    let v1 = back.mass_properties(read[0].body).unwrap().volume;
    assert!((v1 - v0).abs() <= 1e-6 * v0, "{v0} vs {v1}");
    assert!(back.check(read[0].body).unwrap().ok);
}

#[test]
fn tessellation_is_watertight_and_tagged() {
    let mut k = kernel();
    let body = box_minus_cylinder(&mut k);
    let mesh = k.tessellate(body, 0.01).unwrap();
    assert_eq!(mesh.face_tags.len(), mesh.triangles.len());
    assert!(
        mesh.watertightness(1e-9).is_closed(),
        "{:?}",
        mesh.watertightness(1e-9)
    );
    let tags: BTreeSet<u64> = mesh.face_tags.iter().map(|t| t.0).collect();
    let faces: BTreeSet<u64> = k.faces(body).unwrap().into_iter().map(|f| f.0).collect();
    assert_eq!(tags, faces);
    let exact = k.mass_properties(body).unwrap().volume;
    assert!((mesh.volume() - exact).abs() < 1e-3 * exact);
}

#[test]
fn delete_and_copy() {
    let mut k = kernel();
    let b = block(&mut k);
    let top = face_facing(&k, b, [0.0, 0.0, 1.0]);
    let taller = k.offset_faces(b, &[top], 10.0).unwrap().body;
    let copy = k.copy_body(taller).unwrap().body;
    k.delete_body(b).unwrap();
    assert!(matches!(
        k.mass_properties(b),
        Err(KernelError::NotFound { .. })
    ));
    // Entities shared with the deleted input stay alive in the derived bodies.
    assert!(k.check(taller).unwrap().ok);
    assert!(close(k.mass_properties(taller).unwrap().volume, 300_000.0));
    k.delete_body(taller).unwrap();
    assert!(close(k.mass_properties(copy).unwrap().volume, 300_000.0));
    assert!(k.check(copy).unwrap().ok);
}

#[test]
fn interrupt_stops_an_operation() {
    let mut k = kernel();
    k.set_interrupt(Some(Arc::new(|| true)));
    let r = k.make_box(&BoxSpec {
        min: [0.0; 3],
        max: [1.0; 3],
    });
    assert!(matches!(r, Err(KernelError::Interrupted { .. })), "{r:?}");
    k.set_interrupt(None);
    assert!(
        k.make_box(&BoxSpec {
            min: [0.0; 3],
            max: [1.0; 3]
        })
        .is_ok()
    );
}

#[test]
fn info_is_truthful() {
    let k = ArrisKernel::new();
    let info = k.info();
    assert_eq!(info.name, "arris");
    assert!(
        info.capabilities.native_history && info.capabilities.exact_mass && info.capabilities.step
    );
}
