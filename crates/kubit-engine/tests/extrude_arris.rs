//! Milestone 1.4b `solid.extrude` on the real backend: analytic volume oracles,
//! join/cut, stable face names, undo/redo, `.kubit` round trip and hostile
//! profiles.
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

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-6 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

fn face_names(s: &Session, body: u32) -> Vec<String> {
    s.model().bodies[&FeatureId(body)]
        .names
        .keys()
        .map(ToString::to_string)
        .collect()
}

fn line_loop(points: &[[f64; 2]]) -> Value {
    let segments: Vec<Value> = points[1..]
        .iter()
        .chain(&points[..1])
        .map(|p| json!({"line_to": p}))
        .collect();
    json!({"segments": {"start": points[0], "segments": segments}})
}

/// 40 × 30 rectangle on the XY plane with an r = 4 hole at its centre.
fn plate_profile() -> Value {
    json!({
        "origin": [0.0, 0.0, 0.0],
        "x_axis": [1.0, 0.0, 0.0],
        "normal": [0.0, 0.0, 1.0],
        "outer": line_loop(&[[0.0, 0.0], [40.0, 0.0], [40.0, 30.0], [0.0, 30.0]]),
        "holes": [{"circle": {"center": [20.0, 15.0], "radius": 4.0}}],
    })
}

fn circle_profile(origin: [f64; 3], normal: [f64; 3], center: [f64; 2], radius: f64) -> Value {
    json!({
        "origin": origin,
        "x_axis": [1.0, 0.0, 0.0],
        "normal": normal,
        "outer": {"circle": {"center": center, "radius": radius}},
    })
}

fn extrude(s: &mut Session, profile: Value, distance: f64) {
    s.execute(
        "solid.extrude",
        json!({"profile": profile, "distance": distance, "op": "new"}),
    )
    .unwrap();
}

#[test]
fn plate_with_hole_has_the_oracle_volume_and_named_faces() {
    let mut s = session();
    extrude(&mut s, plate_profile(), 10.0);
    assert_close(volume(&s, 1), (1200.0 - 16.0 * PI) * 10.0);
    let mut names = face_names(&s, 1);
    names.sort();
    assert_eq!(
        names,
        [
            "F1:cap+",
            "F1:cap-",
            "F1:side0.0",
            "F1:side0.1",
            "F1:side0.2",
            "F1:side0.3",
            "F1:side1.0"
        ]
    );
    // cap+ is the face the extrusion points out of.
    let body = &s.model().bodies[&FeatureId(1)];
    let top = s
        .kernel()
        .face_info(body.names["F1:cap+".parse::<kubit_doc::TopoName>().as_ref().unwrap()])
        .unwrap();
    assert_close(top.centroid[2], 10.0);
    assert_close(top.normal.unwrap()[2], 1.0);
}

#[test]
fn arc_segment_profile_has_the_analytic_volume() {
    // Half disc of radius 5: an arc over the top, closed by the diameter.
    let profile = json!({
        "origin": [0.0, 0.0, 0.0],
        "x_axis": [1.0, 0.0, 0.0],
        "normal": [0.0, 0.0, 1.0],
        "outer": {"segments": {"start": [0.0, 0.0], "segments": [
            {"arc_to": {"to": [10.0, 0.0], "via": [5.0, 5.0]}},
            {"line_to": [0.0, 0.0]},
        ]}},
    });
    let mut s = session();
    extrude(&mut s, profile, 8.0);
    assert_close(volume(&s, 1), PI * 25.0 / 2.0 * 8.0);
    assert!(face_names(&s, 1).contains(&"F1:side0.0".to_owned()));
}

#[test]
fn tilted_plane_extrudes_along_its_normal() {
    // Plane x = 0 seen from +X: normal +X, x_axis +Y.
    let profile = json!({
        "origin": [0.0, 0.0, 0.0],
        "x_axis": [0.0, 1.0, 0.0],
        "normal": [1.0, 0.0, 0.0],
        "outer": line_loop(&[[0.0, 0.0], [10.0, 0.0], [10.0, 5.0], [0.0, 5.0]]),
    });
    let mut s = session();
    extrude(&mut s, profile, 7.0);
    assert_close(volume(&s, 1), 10.0 * 5.0 * 7.0);
    let body = &s.model().bodies[&FeatureId(1)];
    let name: kubit_doc::TopoName = "F1:cap+".parse().unwrap();
    let info = s.kernel().face_info(body.names[&name]).unwrap();
    assert_close(info.centroid[0], 7.0);
}

#[test]
fn cut_a_circle_through_the_top_of_a_box() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    // Starts above the top face, points down, goes past the bottom.
    let profile = circle_profile([50.0, 30.0, 45.0], [0.0, 0.0, -1.0], [0.0, 0.0], 5.0);
    s.execute(
        "solid.extrude",
        json!({"profile": profile, "distance": 55.0, "op": "cut", "target": "F1"}),
    )
    .unwrap();
    assert_eq!(s.model().bodies.len(), 1, "the prism is consumed");
    assert_close(volume(&s, 1), 100.0 * 60.0 * 40.0 - PI * 25.0 * 40.0);
    let status = &s.model().status;
    assert!(status.values().all(|st| *st == FeatureStatus::Ok));
}

#[test]
fn join_a_boss_onto_a_box() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    let profile = circle_profile([50.0, 30.0, 40.0], [0.0, 0.0, 1.0], [0.0, 0.0], 10.0);
    s.execute(
        "solid.extrude",
        json!({"profile": profile, "distance": 15.0, "op": "join", "target": "F1"}),
    )
    .unwrap();
    assert_eq!(s.model().bodies.len(), 1);
    assert_close(volume(&s, 1), 100.0 * 60.0 * 40.0 + PI * 100.0 * 15.0);
    // The box's names survive and the boss' faces are named by their feature.
    let names = face_names(&s, 1);
    assert!(names.contains(&"F1:-Z".to_owned()), "{names:?}");
    assert!(names.contains(&"F2:cap+".to_owned()), "{names:?}");
    assert!(names.contains(&"F2:side0.0".to_owned()), "{names:?}");
}

#[test]
fn names_are_stable_on_rebuild_and_edit() {
    let mut s = session();
    extrude(&mut s, plate_profile(), 10.0);
    let mut t = session();
    t.load(s.doc().clone());
    assert_eq!(t.model(), s.model());
    assert_eq!(face_names(&t, 1), face_names(&s, 1));
    // Editing the distance in the document keeps every name.
    let mut doc = s.doc().clone();
    doc.features[0].params["distance"] = json!(25.0);
    t.load(doc);
    assert_eq!(face_names(&t, 1), face_names(&s, 1));
    assert_close(volume(&t, 1), (1200.0 - 16.0 * PI) * 25.0);
}

#[test]
fn extrude_undoes_and_redoes() {
    let mut s = session();
    let before = s.model().clone();
    extrude(&mut s, plate_profile(), 10.0);
    let after = s.model().clone();
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(*s.model(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(*s.model(), after);
}

#[test]
fn kubit_round_trip() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    let boss = circle_profile([50.0, 30.0, 40.0], [0.0, 0.0, 1.0], [0.0, 0.0], 10.0);
    s.execute(
        "solid.extrude",
        json!({"profile": boss, "distance": 15.0, "op": "join", "target": "F1"}),
    )
    .unwrap();
    extrude(&mut s, plate_profile(), 3.0);
    let dir = std::env::temp_dir().join(format!("kubit-extrude-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("extrude.kubit");
    s.execute("doc.save", json!({"path": path})).unwrap();
    let mut t = session();
    t.execute("doc.open", json!({"path": path})).unwrap();
    assert!(
        t.model().status.values().all(|st| *st == FeatureStatus::Ok),
        "{:?}",
        t.model().status
    );
    assert_eq!(t.model(), s.model());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn bad_target_params_are_rejected() {
    let mut s = session();
    for params in [
        json!({"profile": plate_profile(), "distance": 5.0, "op": "join"}),
        json!({"profile": plate_profile(), "distance": 5.0, "op": "cut", "target": "F9"}),
        json!({"profile": plate_profile(), "distance": 5.0, "op": "new", "target": "F1"}),
    ] {
        let n = s.doc().features.len();
        assert!(s.execute("solid.extrude", params).is_err());
        assert_eq!(s.doc().features.len(), n);
    }
}

fn number() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => -100.0f64..100.0,
        1 => Just(0.0),
        1 => Just(f64::NAN),
        1 => Just(f64::INFINITY),
        1 => Just(-1.0e12),
        1 => Just(1.0e-9),
    ]
}

fn pt() -> impl Strategy<Value = [f64; 2]> {
    [number(), number()]
}

fn seg() -> impl Strategy<Value = Value> {
    prop_oneof![
        pt().prop_map(|p| json!({"line_to": p})),
        (pt(), pt()).prop_map(|(to, via)| json!({"arc_to": {"to": to, "via": via}})),
    ]
}

fn hostile_loop() -> impl Strategy<Value = Value> {
    prop_oneof![
        (pt(), number()).prop_map(|(c, r)| json!({"circle": {"center": c, "radius": r}})),
        (pt(), prop::collection::vec(seg(), 0..6)).prop_map(
            |(start, segments)| json!({"segments": {"start": start, "segments": segments}})
        ),
    ]
}

fn axis() -> impl Strategy<Value = [f64; 3]> {
    prop_oneof![
        Just([1.0, 0.0, 0.0]),
        Just([0.0, 1.0, 0.0]),
        Just([0.0, 0.0, 1.0]),
        Just([0.0, 0.0, 0.0]),
        Just([2.0, 0.0, 0.0]),
        Just([f64::NAN, 0.0, 1.0]),
        Just([0.6, 0.8, 0.0]),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn hostile_profiles_never_panic(
        origin in [number(), number(), number()],
        x_axis in axis(),
        normal in axis(),
        outer in hostile_loop(),
        holes in prop::collection::vec(hostile_loop(), 0..3),
        distance in number(),
    ) {
        let mut s = session();
        let profile = json!({
            "origin": origin, "x_axis": x_axis, "normal": normal,
            "outer": outer, "holes": holes,
        });
        let before = s.model().clone();
        let r = s.execute(
            "solid.extrude",
            json!({"profile": profile, "distance": distance, "op": "new"}),
        );
        if r.is_err() {
            prop_assert_eq!(s.model(), &before);
            prop_assert!(!s.kernel().is_poisoned());
        }
    }
}

#[test]
fn degenerate_profiles_are_refused_cleanly() {
    let square = line_loop(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]);
    // Bow-tie: the loop touches itself.
    let bow = line_loop(&[[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]]);
    let base = |outer: Value, x: Value, n: Value, d: f64| {
        json!({"profile": {"origin": [0,0,0], "x_axis": x, "normal": n, "outer": outer},
               "distance": d, "op": "new"})
    };
    let (x, z) = (json!([1, 0, 0]), json!([0, 0, 1]));
    let cases = [
        base(square.clone(), json!([2, 0, 0]), z.clone(), 5.0),
        base(square.clone(), x.clone(), json!([0, 0, 2]), 5.0),
        base(square.clone(), x.clone(), json!([1, 0, 0]), 5.0),
        base(square.clone(), x.clone(), z.clone(), 0.0),
        base(square.clone(), x.clone(), z.clone(), -3.0),
        base(bow, x.clone(), z.clone(), 5.0),
        base(
            line_loop(&[[0.0, 0.0], [0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]),
            x,
            z,
            5.0,
        ),
    ];
    for params in cases {
        let mut s = session();
        assert!(
            s.execute("solid.extrude", params.clone()).is_err(),
            "{params}"
        );
        assert!(s.model().bodies.is_empty());
        assert!(!s.kernel().is_poisoned());
    }
}
