//! Milestone 1.4 exit check on the real backend: a fully constrained sketch, extruded,
//! a sketch on its top face cutting through, and a dimension edit that rebuilds
//! everything without losing the cut.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_doc::FeatureId;
use kubit_engine::{FeatureStatus, Session};
use kubit_kernel_api::Kernel;
use kubit_kernel_arris::ArrisKernel;
use serde_json::{Value, json};
use std::f64::consts::PI;

fn session() -> Session {
    Session::new(Box::new(|| Box::new(ArrisKernel::new()) as Box<dyn Kernel>))
}

fn volume(s: &Session, body: u32) -> f64 {
    let b = &s.model().bodies[&FeatureId(body)];
    s.kernel().mass_properties(b.kernel).unwrap().volume
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-6 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, params)
        .unwrap_or_else(|e| panic!("{id}: {e}"))
        .data
}

fn point(x: f64, y: f64) -> Value {
    json!({ "kind": "point", "x": x, "y": y })
}

/// Rectangle 80 × 50 with a circle r = 10, fully constrained. Ids: points 0–3,
/// lines 4–7 (bottom, right, top, left), centre 8, circle 9, constraints 10–19
/// (`length` of the bottom is 10).
fn plate(s: &mut Session) {
    run(s, "sketch.create", json!({ "plane": "XY" }));
    let data = run(
        s,
        "sketch.add",
        json!({
            "sketch": "F1",
            "first_id": 0,
            "entities": [
                point(0.0, 0.0), point(78.0, 1.0), point(81.0, 52.0), point(-1.0, 49.0),
                { "kind": "line", "a": 0, "b": 1 },
                { "kind": "line", "a": 1, "b": 2 },
                { "kind": "line", "a": 2, "b": 3 },
                { "kind": "line", "a": 3, "b": 0 },
                point(38.0, 26.0),
                { "kind": "circle", "center": 8, "radius": 9.0 },
            ],
            "constraints": [
                { "kind": "length", "line": 4, "value": 80.0 },
                { "kind": "fix", "point": 0, "at": [0.0, 0.0] },
                { "kind": "horizontal", "line": 4 },
                { "kind": "horizontal", "line": 6 },
                { "kind": "vertical", "line": 5 },
                { "kind": "vertical", "line": 7 },
                { "kind": "length", "line": 7, "value": 50.0 },
                { "kind": "horizontal_distance", "a": 0, "b": 8, "value": 40.0 },
                { "kind": "vertical_distance", "a": 0, "b": 8, "value": 25.0 },
                { "kind": "radius", "circle": 9, "value": 10.0 },
            ],
        }),
    );
    let report = &data["report"];
    assert_eq!(report["dof"], 0, "{report}");
    assert_eq!(report["converged"], true);
    assert_eq!(data["constraints"][0], 10);
}

#[test]
fn sketch_extrude_cut_and_rebuild() {
    let mut s = session();
    plate(&mut s);
    // Region id: the smallest entity of its outer loop (the bottom line, 4).
    let regions = s.model().sketches[&FeatureId(1)].regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].id.0, 4);
    assert_eq!(regions[0].holes.len(), 1);
    run(
        &mut s,
        "solid.extrude",
        json!({ "sketch": "F1", "region": 4, "distance": 20, "op": "new" }),
    );
    close(volume(&s, 2), (4000.0 - 100.0 * PI) * 20.0);

    // A sketch on the top cap: its frame sits at z = 20.
    run(
        &mut s,
        "sketch.create",
        json!({ "plane": { "face": "F2:cap+" } }),
    );
    let frame = s.model().sketches[&FeatureId(3)].frame;
    assert!((frame.origin[2] - 20.0).abs() < 1e-9 && (frame.normal[2] - 1.0).abs() < 1e-12);
    run(
        &mut s,
        "sketch.add",
        json!({
            "sketch": "F3",
            "first_id": 0,
            "entities": [point(65.0, 25.0), { "kind": "circle", "center": 0, "radius": 5.0 }],
            "constraints": [
                { "kind": "fix", "point": 0, "at": [65.0, 25.0] },
                { "kind": "radius", "circle": 1, "value": 5.0 },
            ],
        }),
    );
    run(
        &mut s,
        "solid.extrude",
        json!({ "sketch": "F3", "region": 1, "distance": -20, "op": "cut", "target": "F2" }),
    );
    close(volume(&s, 2), (4000.0 - 100.0 * PI - 25.0 * PI) * 20.0);

    // Edit the width: everything rebuilds and the cut survives.
    run(
        &mut s,
        "sketch.set_dimension",
        json!({ "sketch": "F1", "constraint": 10, "value": 90 }),
    );
    assert!(
        s.model().status.values().all(|st| *st == FeatureStatus::Ok),
        "{:?}",
        s.model().status
    );
    close(volume(&s, 2), (4500.0 - 100.0 * PI - 25.0 * PI) * 20.0);

    // Undo restores the 80 mm plate; redo the 90 mm one.
    s.execute("edit.undo", json!({})).unwrap();
    close(volume(&s, 2), (4000.0 - 125.0 * PI) * 20.0);
    s.execute("edit.redo", json!({})).unwrap();
    close(volume(&s, 2), (4500.0 - 125.0 * PI) * 20.0);
}

#[test]
fn conflicts_are_reported_and_points_drag() {
    let mut s = session();
    plate(&mut s);
    let data = run(
        &mut s,
        "sketch.add",
        json!({
            "sketch": "F1",
            "first_id": 20,
            "constraints": [{ "kind": "length", "line": 5, "value": 30.0 }],
        }),
    );
    assert_eq!(
        data["report"]["conflicting"],
        json!([20]),
        "{}",
        data["report"]
    );
    run(
        &mut s,
        "sketch.delete",
        json!({ "sketch": "F1", "constraints": [20] }),
    );
    assert_eq!(s.model().sketches[&FeatureId(1)].report.dof, 0);

    // Free the circle's centre and drag it.
    run(
        &mut s,
        "sketch.delete",
        json!({ "sketch": "F1", "constraints": [17, 18] }),
    );
    assert_eq!(s.model().sketches[&FeatureId(1)].report.dof, 2);
    run(
        &mut s,
        "sketch.move_point",
        json!({ "sketch": "F1", "point": 8, "to": [20.0, 15.0] }),
    );
    let sk = &s.model().sketches[&FeatureId(1)].sketch;
    let c = sk.point(kubit_sketch::EntityId(8)).unwrap();
    assert!(
        (c[0] - 20.0).abs() < 1e-6 && (c[1] - 15.0).abs() < 1e-6,
        "{c:?}"
    );
}

#[test]
fn bad_edits_change_nothing() {
    let mut s = session();
    plate(&mut s);
    let before = s.doc().clone();
    for (id, params) in [
        (
            "sketch.add",
            json!({ "sketch": "F1", "first_id": 3, "entities": [point(0.0, 0.0)] }),
        ),
        ("sketch.add", json!({ "sketch": "F9", "first_id": 20 })),
        (
            "sketch.add",
            json!({ "sketch": "F1", "first_id": 20, "entities": [{ "kind": "line", "a": 0, "b": 0 }] }),
        ),
        (
            "sketch.set_dimension",
            json!({ "sketch": "F1", "constraint": 11, "value": 5 }),
        ),
        (
            "sketch.set_dimension",
            json!({ "sketch": "F1", "constraint": 10, "value": f64::MAX }),
        ),
        (
            "sketch.move_point",
            json!({ "sketch": "F1", "point": 4, "to": [0, 0] }),
        ),
        ("sketch.create", json!({ "plane": { "face": "F7:+Z" } })),
        (
            "solid.extrude",
            json!({ "sketch": "F1", "region": 99, "distance": 5, "op": "new" }),
        ),
        (
            "solid.extrude",
            json!({ "sketch": "F1", "distance": 5, "op": "new" }),
        ),
    ] {
        assert!(s.execute(id, params.clone()).is_err(), "{id} {params}");
    }
    assert_eq!(s.doc(), &before);
}

#[test]
fn sketches_survive_a_kubit_round_trip() {
    let mut s = session();
    plate(&mut s);
    run(
        &mut s,
        "solid.extrude",
        json!({ "sketch": "F1", "region": 4, "distance": 20, "op": "new" }),
    );
    let dir = std::env::temp_dir().join(format!("kubit-sketch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plate.kubit");
    run(&mut s, "doc.save", json!({ "path": path }));
    let mut t = session();
    run(&mut t, "doc.open", json!({ "path": path }));
    close(volume(&t, 2), (4000.0 - 100.0 * PI) * 20.0);
    assert_eq!(t.model().sketches[&FeatureId(1)].report.dof, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
