//! Milestone 1.3 direct modelling commands on the real backend: analytic volume
//! oracles, undo/redo, `.kubit` round trip and hostile inputs.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_doc::FeatureId;
use kubit_engine::{CommandError, Session};
use kubit_kernel_api::Kernel;
use kubit_kernel_arris::ArrisKernel;
use proptest::prelude::*;
use serde_json::{Value, json};
use std::f64::consts::PI;

fn session() -> Session {
    Session::new(Box::new(|| Box::new(ArrisKernel::new()) as Box<dyn Kernel>))
}

fn volume(s: &Session, body: u32) -> f64 {
    let b = &s.model().bodies[&FeatureId(body)];
    s.kernel().mass_properties(b.kernel).unwrap().volume
}

fn total_volume(s: &Session) -> f64 {
    s.model()
        .bodies
        .values()
        .map(|b| s.kernel().mass_properties(b.kernel).unwrap().volume)
        .sum()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-6 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

fn boxed(s: &mut Session, origin: [f64; 3], size: [f64; 3]) {
    s.execute("solid.box", json!({"origin": origin, "size": size}))
        .unwrap();
}

#[test]
fn cylinder_volume() {
    let mut s = session();
    s.execute(
        "solid.cylinder",
        json!({"origin": [1.0, 2.0, 3.0], "radius": 10.0, "height": 25.0}),
    )
    .unwrap();
    assert_close(volume(&s, 1), PI * 100.0 * 25.0);
    let faces = &s.model().bodies[&FeatureId(1)].names;
    let names: Vec<String> = faces.keys().map(ToString::to_string).collect();
    assert_eq!(names.len(), 3, "{names:?}");
    assert!(names.iter().any(|n| n == "F1:+Z"));
    assert!(names.iter().any(|n| n == "F1:-Z"));
}

#[test]
fn fillet_removes_the_oracle_volume() {
    let mut s = session();
    boxed(&mut s, [0.0; 3], [100.0, 60.0, 40.0]);
    let v0 = volume(&s, 1);
    s.execute(
        "edge.fillet",
        json!({"edges": ["F1:+X|F1:+Z"], "radius": 5.0}),
    )
    .unwrap();
    // The edge F1:+X|F1:+Z runs along Y (60 mm).
    assert_close(v0 - volume(&s, 1), (1.0 - PI / 4.0) * 25.0 * 60.0);
}

#[test]
fn chamfer_removes_the_oracle_volume() {
    let mut s = session();
    boxed(&mut s, [0.0; 3], [100.0, 60.0, 40.0]);
    let v0 = volume(&s, 1);
    s.execute(
        "edge.chamfer",
        json!({"edges": ["F1:+X|F1:+Z"], "distance": 5.0}),
    )
    .unwrap();
    assert_close(v0 - volume(&s, 1), 25.0 * 60.0 / 2.0);
}

#[test]
fn shell_open_at_the_top() {
    let (a, b, c, t) = (100.0, 60.0, 40.0, 3.0);
    let mut s = session();
    boxed(&mut s, [0.0; 3], [a, b, c]);
    s.execute("body.shell", json!({"faces": ["F1:+Z"], "thickness": t}))
        .unwrap();
    assert_close(
        volume(&s, 1),
        a * b * c - (a - 2.0 * t) * (b - 2.0 * t) * (c - t),
    );
}

#[test]
fn booleans_of_overlapping_boxes() {
    let (va, vb, vi) = (8000.0, 8000.0, 1000.0);
    for (op, expected) in [
        ("union", va + vb - vi),
        ("subtract", va - vi),
        ("intersect", vi),
    ] {
        let mut s = session();
        boxed(&mut s, [0.0; 3], [20.0; 3]);
        boxed(&mut s, [10.0; 3], [20.0; 3]);
        s.execute(
            "body.boolean",
            json!({"op": op, "target": "F1", "tool": "F2"}),
        )
        .unwrap();
        assert_eq!(s.model().bodies.len(), 1, "{op}: tool is consumed");
        assert!(s.model().bodies.contains_key(&FeatureId(1)));
        assert_close(volume(&s, 1), expected);
    }
}

#[test]
fn push_pull_moves_several_faces_at_once() {
    let mut s = session();
    boxed(&mut s, [0.0; 3], [10.0, 10.0, 10.0]);
    s.execute(
        "face.push_pull",
        json!({"faces": ["F1:+Z", "F1:+X"], "distance": 5.0}),
    )
    .unwrap();
    assert_close(volume(&s, 1), 15.0 * 10.0 * 15.0);
    // The single-face form keeps working.
    s.execute("face.push_pull", json!({"face": "F1:+Y", "distance": 5.0}))
        .unwrap();
    assert_close(volume(&s, 1), 15.0 * 15.0 * 15.0);
}

/// One scenario per command: (setup, command, params).
fn scenarios() -> Vec<(&'static str, Value)> {
    vec![
        ("solid.cylinder", json!({"radius": 5.0, "height": 9.0})),
        (
            "edge.fillet",
            json!({"edges": ["F1:+X|F1:+Z"], "radius": 2.0}),
        ),
        (
            "edge.chamfer",
            json!({"edges": ["F1:+X|F1:+Z"], "distance": 2.0}),
        ),
        ("body.shell", json!({"faces": ["F1:+Z"], "thickness": 2.0})),
        (
            "body.boolean",
            json!({"op": "union", "target": "F1", "tool": "F2"}),
        ),
        (
            "face.push_pull",
            json!({"faces": ["F1:+Z", "F1:+X"], "distance": 3.0}),
        ),
    ]
}

#[test]
fn every_command_undoes_and_redoes() {
    for (command, params) in scenarios() {
        let mut s = session();
        boxed(&mut s, [0.0; 3], [20.0; 3]);
        boxed(&mut s, [10.0; 3], [20.0; 3]);
        let before = s.model().clone();
        s.execute(command, params).unwrap();
        let after = s.model().clone();
        assert_ne!(after, before, "{command}");
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(*s.model(), before, "{command}: undo");
        s.execute("edit.redo", json!({})).unwrap();
        assert_eq!(*s.model(), after, "{command}: redo");
    }
}

#[test]
fn kubit_round_trip_with_every_command() {
    let mut s = session();
    boxed(&mut s, [0.0; 3], [100.0, 60.0, 40.0]); // F1
    s.execute(
        "face.push_pull",
        json!({"faces": ["F1:+Z"], "distance": 10.0}),
    )
    .unwrap(); // F2
    s.execute(
        "solid.cylinder",
        json!({"origin": [50.0, 30.0, 30.0], "radius": 8.0, "height": 20.0}),
    )
    .unwrap(); // F3
    s.execute(
        "body.boolean",
        json!({"op": "union", "target": "F1", "tool": "F3"}),
    )
    .unwrap(); // F3
    s.execute(
        "edge.fillet",
        json!({"edges": ["F1:+X|F1:+Y", "F1:-X|F1:+Y"], "radius": 4.0}),
    )
    .unwrap(); // F5
    s.execute(
        "edge.chamfer",
        json!({"edges": ["F1:+X|F1:-Y"], "distance": 3.0}),
    )
    .unwrap(); // F6
    // arris refuses to shell a body with a through-hole or pocket (genus/
    // pocket offsets collide), so the round trip uses a boss.
    s.execute("body.shell", json!({"faces": ["F1:-Z"], "thickness": 2.0}))
        .unwrap(); // F7
    let expected = total_volume(&s);
    let dir = std::env::temp_dir().join(format!("kubit-direct-ops-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("all.kubit");
    s.execute("doc.save", json!({"path": path})).unwrap();

    let mut t = session();
    t.execute("doc.open", json!({"path": path})).unwrap();
    assert!(
        t.model()
            .status
            .values()
            .all(|st| *st == kubit_engine::FeatureStatus::Ok),
        "{:?}",
        t.model().status
    );
    assert_eq!(t.model().bodies.len(), 1);
    assert_close(total_volume(&t), expected);
    assert_eq!(t.model(), s.model());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unresolved_names_fail_the_feature_on_rebuild() {
    let mut s = session();
    boxed(&mut s, [0.0; 3], [20.0; 3]);
    s.execute(
        "edge.fillet",
        json!({"edges": ["F1:+X|F1:+Z"], "radius": 2.0}),
    )
    .unwrap();
    let mut doc = s.doc().clone();
    // Corrupt the reference: the edge no longer exists.
    doc.features[1].params = json!({"edges": ["F1:+X|F9:+Z"], "radius": 2.0});
    let mut t = session();
    t.load(doc);
    assert!(matches!(
        t.model().status[&FeatureId(2)],
        kubit_engine::FeatureStatus::Failed(_)
    ));
    assert_eq!(t.model().bodies.len(), 1);
}

#[test]
fn references_across_bodies_are_rejected() {
    let mut s = session();
    boxed(&mut s, [0.0; 3], [20.0; 3]);
    boxed(&mut s, [50.0; 3], [20.0; 3]);
    let n = s.doc().features.len();
    let e = s
        .execute(
            "edge.fillet",
            json!({"edges": ["F1:+X|F1:+Z", "F2:+X|F2:+Z"], "radius": 1.0}),
        )
        .unwrap_err();
    assert!(matches!(e, CommandError::InvalidParams { .. }), "{e}");
    let e = s
        .execute(
            "body.shell",
            json!({"faces": ["F1:+Z", "F2:+Z"], "thickness": 1.0}),
        )
        .unwrap_err();
    assert!(matches!(e, CommandError::InvalidParams { .. }), "{e}");
    let e = s
        .execute(
            "face.push_pull",
            json!({"faces": ["F1:+Z", "F2:+Z"], "distance": 1.0}),
        )
        .unwrap_err();
    assert!(matches!(e, CommandError::InvalidParams { .. }), "{e}");
    assert_eq!(s.doc().features.len(), n);
}

#[test]
fn selection_gates_the_enablement_for_the_ui() {
    use kubit_engine::Enablement;
    let mut s = session();
    boxed(&mut s, [0.0; 3], [20.0; 3]);
    let enabled = |s: &Session, id: &str| (s.registry().get(id).unwrap().enabled)(s);
    assert!(matches!(
        enabled(&s, "edge.fillet"),
        Enablement::Disabled(_)
    ));
    assert!(matches!(enabled(&s, "body.shell"), Enablement::Disabled(_)));
    assert!(matches!(
        enabled(&s, "body.boolean"),
        Enablement::Disabled(_)
    ));
    assert_eq!(enabled(&s, "solid.cylinder"), Enablement::Enabled);
    s.execute("selection.set", json!({"edges": ["F1:+X|F1:+Z"]}))
        .unwrap();
    assert_eq!(enabled(&s, "edge.fillet"), Enablement::Enabled);
    assert_eq!(enabled(&s, "edge.chamfer"), Enablement::Enabled);
    s.execute("selection.set", json!({"faces": ["F1:+Z"]}))
        .unwrap();
    assert_eq!(enabled(&s, "body.shell"), Enablement::Enabled);
}

fn number() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::from(f64::NAN)),
        Just(Value::from(f64::INFINITY)),
        Just(Value::from(f64::NEG_INFINITY)),
        Just(json!(0.0)),
        Just(json!(-3.0)),
        Just(json!(1e9)),
        Just(json!(1e-12)),
        Just(json!(2.0)),
        Just(json!("two")),
    ]
}

fn name() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!("F1:+Z")),
        Just(json!("F1:+X|F1:+Z")),
        Just(json!("F1:+Z|F1:+Z")),
        Just(json!("F7:+Z")),
        Just(json!("F7:+X|F7:+Z")),
        Just(json!("")),
        Just(json!("garbage|||")),
        Just(json!("F1")),
        Just(json!(42)),
    ]
}

fn names() -> impl Strategy<Value = Value> {
    prop::collection::vec(name(), 0..4).prop_map(Value::from)
}

fn body_ref() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!("F1")),
        Just(json!("F2")),
        Just(json!("F99")),
        Just(json!("")),
        Just(json!("x")),
        Just(json!(null)),
    ]
}

fn op() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!("union")),
        Just(json!("subtract")),
        Just(json!("intersect")),
        Just(json!("xor")),
    ]
}

fn hostile() -> impl Strategy<Value = (&'static str, Value)> {
    prop_oneof![
        (number(), number(), number(), number()).prop_map(|(x, r, h, z)| (
            "solid.cylinder",
            json!({"origin": [x, 0.0, z], "radius": r, "height": h})
        )),
        (names(), number()).prop_map(|(e, r)| ("edge.fillet", json!({"edges": e, "radius": r}))),
        (names(), number()).prop_map(|(e, d)| ("edge.chamfer", json!({"edges": e, "distance": d}))),
        (names(), number()).prop_map(|(f, t)| ("body.shell", json!({"faces": f, "thickness": t}))),
        (op(), body_ref(), body_ref())
            .prop_map(|(o, a, b)| ("body.boolean", json!({"op": o, "target": a, "tool": b}))),
        (names(), name(), number()).prop_map(|(fs, f, d)| (
            "face.push_pull",
            json!({"faces": fs, "face": f, "distance": d})
        )),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn hostile_inputs_never_panic((command, params) in hostile()) {
        let mut s = session();
        boxed(&mut s, [0.0; 3], [20.0; 3]);
        boxed(&mut s, [10.0; 3], [20.0; 3]);
        let features = s.doc().features.len();
        match s.execute(command, params) {
            Ok(_) => prop_assert_eq!(s.doc().features.len(), features + 1),
            Err(_) => {
                // A failed command changes nothing.
                prop_assert_eq!(s.doc().features.len(), features);
                prop_assert_eq!(s.model().bodies.len(), 2);
            }
        }
    }
}
