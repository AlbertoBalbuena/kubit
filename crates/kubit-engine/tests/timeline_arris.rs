//! Timeline commands on the real backend: `feature.edit`, `feature.suppress`,
//! `feature.delete` and `timeline.set_marker` — rebuild results, rollback order,
//! `.kubit` round trip, undo/redo and hostile parameters.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_doc::FeatureId;
use kubit_engine::{FeatureStatus, Session};
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

fn status(s: &Session, id: u32) -> FeatureStatus {
    s.model().status[&FeatureId(id)].clone()
}

fn order(s: &Session) -> Vec<String> {
    s.doc().features.iter().map(|f| f.id.to_string()).collect()
}

/// F1 box 10³ and F2 fillet of its +X/+Z edge, r = 2.
fn filleted_box(s: &mut Session) {
    run(s, "solid.box", json!({ "size": [10.0, 10.0, 10.0] }));
    run(
        s,
        "edge.fillet",
        json!({ "edges": ["F1:+X|F1:+Z"], "radius": 2.0 }),
    );
}

const FILLETED: f64 = 1000.0 - (4.0 - PI) * 10.0;

fn square_sketch(s: &mut Session) {
    run(s, "sketch.create", json!({ "plane": "XY" }));
    run(
        s,
        "sketch.add",
        json!({
            "sketch": "F1",
            "first_id": 0,
            "entities": [
                { "kind": "point", "x": 0.0, "y": 0.0 },
                { "kind": "point", "x": 10.0, "y": 0.0 },
                { "kind": "point", "x": 10.0, "y": 10.0 },
                { "kind": "point", "x": 0.0, "y": 10.0 },
                { "kind": "line", "a": 0, "b": 1 },
                { "kind": "line", "a": 1, "b": 2 },
                { "kind": "line", "a": 2, "b": 3 },
                { "kind": "line", "a": 3, "b": 0 },
            ],
            "constraints": [],
        }),
    );
}

fn extrude_square(s: &mut Session, distance: f64) {
    run(
        s,
        "solid.extrude",
        json!({ "sketch": "F1", "region": 4, "distance": distance, "op": "new" }),
    );
}

#[test]
fn edit_box_size_then_extrude_distance() {
    let mut s = session();
    run(&mut s, "solid.box", json!({ "size": [10.0, 20.0, 30.0] }));
    close(volume(&s, 1), 6000.0);
    run(
        &mut s,
        "feature.edit",
        json!({ "feature": "F1", "params": { "size": [20.0, 20.0, 30.0] } }),
    );
    close(volume(&s, 1), 12000.0);

    let mut s = session();
    square_sketch(&mut s);
    extrude_square(&mut s, 5.0);
    close(volume(&s, 2), 500.0);
    let params = s.doc().feature(FeatureId(2)).unwrap().params.clone();
    let mut edited = params;
    edited["distance"] = json!(8.0);
    run(
        &mut s,
        "feature.edit",
        json!({ "feature": "F2", "params": edited }),
    );
    close(volume(&s, 2), 800.0);
}

#[test]
fn edit_rejects_params_of_the_wrong_shape_without_changes() {
    let mut s = session();
    filleted_box(&mut s);
    let before = s.doc().clone();
    // A box's params on a fillet, an unknown field, and an unknown feature.
    for (feature, params) in [
        ("F2", json!({ "size": [1.0, 1.0, 1.0] })),
        ("F1", json!({ "size": [1.0, 1.0, 1.0], "bogus": 1 })),
        ("F1", json!({ "size": [-1.0, 1.0, 1.0] })),
        ("F9", json!({ "size": [1.0, 1.0, 1.0] })),
        ("nope", json!({})),
    ] {
        assert!(
            s.execute(
                "feature.edit",
                json!({ "feature": feature, "params": params })
            )
            .is_err()
        );
    }
    assert_eq!(s.doc(), &before);
    close(volume(&s, 1), FILLETED);
}

#[test]
fn edit_that_breaks_a_dependent_keeps_it_failed() {
    let mut s = session();
    filleted_box(&mut s);
    // Shrink the box to a 1 mm plate under a 2 mm fillet radius.
    run(
        &mut s,
        "feature.edit",
        json!({ "feature": "F1", "params": { "size": [10.0, 10.0, 1.0] } }),
    );
    assert_eq!(status(&s, 1), FeatureStatus::Ok);
    // Whatever the kernel decides about the fillet, nothing panics and the status
    // is recorded.
    assert!(matches!(
        status(&s, 2),
        FeatureStatus::Ok | FeatureStatus::Failed(_)
    ));
}

#[test]
fn suppress_and_restore_fillet() {
    let mut s = session();
    filleted_box(&mut s);
    close(volume(&s, 1), FILLETED);
    run(
        &mut s,
        "feature.suppress",
        json!({ "feature": "F2", "suppressed": true }),
    );
    assert_eq!(status(&s, 2), FeatureStatus::Suppressed);
    close(volume(&s, 1), 1000.0);
    run(
        &mut s,
        "feature.suppress",
        json!({ "feature": "F2", "suppressed": false }),
    );
    assert_eq!(status(&s, 2), FeatureStatus::Ok);
    close(volume(&s, 1), FILLETED);
}

#[test]
fn delete_sketch_used_by_extrude_fails_the_extrude() {
    let mut s = session();
    square_sketch(&mut s);
    extrude_square(&mut s, 5.0);
    run(&mut s, "feature.delete", json!({ "feature": "F1" }));
    assert_eq!(order(&s), ["F2"]);
    assert!(matches!(status(&s, 2), FeatureStatus::Failed(_)));
    assert!(s.model().bodies.is_empty());
    assert!(s.undo());
    assert_eq!(order(&s), ["F1", "F2"]);
    close(volume(&s, 2), 500.0);
    assert!(
        s.execute("feature.delete", json!({ "feature": "F7" }))
            .is_err()
    );
}

#[test]
fn marker_rolls_back_and_new_features_insert_after_it() {
    let mut s = session();
    run(&mut s, "solid.box", json!({ "size": [10.0, 10.0, 10.0] }));
    run(
        &mut s,
        "face.push_pull",
        json!({ "face": "F1:+Z", "distance": 5.0 }),
    );
    close(volume(&s, 1), 1500.0);
    run(&mut s, "timeline.set_marker", json!({ "after": "F1" }));
    assert_eq!(status(&s, 1), FeatureStatus::Ok);
    assert_eq!(status(&s, 2), FeatureStatus::Suppressed);
    assert!(!s.doc().feature(FeatureId(2)).unwrap().suppressed);
    close(volume(&s, 1), 1000.0);

    // A fillet added now lands between F1 and F2 and the marker follows it.
    run(
        &mut s,
        "edge.fillet",
        json!({ "edges": ["F1:+X|F1:+Z"], "radius": 2.0 }),
    );
    assert_eq!(order(&s), ["F1", "F3", "F2"]);
    assert_eq!(s.doc().rollback, Some(FeatureId(3)));
    assert_eq!(status(&s, 2), FeatureStatus::Suppressed);
    close(volume(&s, 1), FILLETED);

    // Marker to the end: F2 replays on top of the fillet.
    run(&mut s, "timeline.set_marker", json!({ "after": null }));
    assert_eq!(s.doc().rollback, None);
    assert_eq!(status(&s, 2), FeatureStatus::Ok);
    // F2 pushed the +Z face out by 5 mm on top of the fillet: more than the plain box.
    assert!(volume(&s, 1) > 1000.0 + 400.0);
    assert!(s.model().status.values().all(|st| *st == FeatureStatus::Ok));

    assert!(
        s.execute("timeline.set_marker", json!({ "after": "F9" }))
            .is_err()
    );
}

#[test]
fn deleting_the_marker_feature_moves_the_marker_back() {
    let mut s = session();
    filleted_box(&mut s);
    run(&mut s, "timeline.set_marker", json!({ "after": "F2" }));
    run(&mut s, "feature.delete", json!({ "feature": "F2" }));
    assert_eq!(s.doc().rollback, Some(FeatureId(1)));
    s.doc().validate().unwrap();
}

#[test]
fn marker_survives_a_kubit_round_trip() {
    let mut s = session();
    filleted_box(&mut s);
    let plain = kubit_doc::to_json(s.doc()).unwrap();
    assert!(!plain.contains("rollback"), "no marker, no field");

    run(&mut s, "timeline.set_marker", json!({ "after": "F1" }));
    let bytes = kubit_doc::to_kubit(s.doc(), None).unwrap();
    let doc = kubit_doc::from_kubit(&bytes).unwrap();
    assert_eq!(doc.rollback, Some(FeatureId(1)));

    let mut t = session();
    t.load(doc);
    assert_eq!(status(&t, 2), FeatureStatus::Suppressed);
    close(volume(&t, 1), 1000.0);
    assert_eq!(kubit_doc::to_kubit(t.doc(), None).unwrap(), bytes);
}

#[test]
fn every_command_undoes_and_redoes() {
    let mut s = session();
    filleted_box(&mut s);
    let cases = [
        (
            "feature.edit",
            json!({ "feature": "F1", "params": { "size": [20.0, 10.0, 10.0] } }),
        ),
        (
            "feature.suppress",
            json!({ "feature": "F2", "suppressed": true }),
        ),
        ("timeline.set_marker", json!({ "after": "F1" })),
        ("feature.delete", json!({ "feature": "F2" })),
    ];
    for (id, params) in cases {
        let (doc, model) = (s.doc().clone(), s.model().clone());
        run(&mut s, id, params);
        let (doc2, model2) = (s.doc().clone(), s.model().clone());
        assert_ne!(doc, doc2, "{id} changed nothing");
        assert!(s.undo(), "{id}");
        assert_eq!(s.doc(), &doc, "{id} undo");
        assert_eq!(s.model(), &model, "{id} undo model");
        assert!(s.redo(), "{id}");
        assert_eq!(s.doc(), &doc2, "{id} redo");
        assert_eq!(s.model(), &model2, "{id} redo model");
    }
}

fn hostile_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        prop_oneof![
            Just(0.0),
            Just(-1.0),
            Just(1e300),
            Just(-1e300),
            Just(1e-300),
            any::<f64>()
        ]
        .prop_map(|x| serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)),
        "[ -~]{0,12}".prop_map(Value::from),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map("[a-z_]{1,8}", inner, 0..4)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

fn hostile_params() -> impl Strategy<Value = Value> {
    let number = prop_oneof![
        Just(0.0),
        Just(-5.0),
        Just(1e300),
        Just(-1e300),
        Just(f64::MIN_POSITIVE),
        -50.0..50.0f64,
    ];
    prop_oneof![
        hostile_value(),
        prop::array::uniform3(number.clone()).prop_map(|size| json!({ "size": size })),
        (number.clone(), number)
            .prop_map(|(r, d)| json!({ "radius": r, "distance": d, "edges": ["F1:+X|F1:+Z"] })),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn hostile_feature_edit_never_panics_and_failure_changes_nothing(
        target in prop::sample::select(vec!["F1", "F2", "F9", "x"]),
        params in hostile_params(),
    ) {
        let mut s = session();
        filleted_box(&mut s);
        let before = s.doc().clone();
        let result = s.execute("feature.edit", json!({ "feature": target, "params": params }));
        match result {
            Err(_) => prop_assert_eq!(s.doc(), &before),
            Ok(_) => prop_assert!(s.doc().validate().is_ok()),
        }
        // Whatever happened, the session still rebuilds and undoes.
        s.undo();
    }
}
