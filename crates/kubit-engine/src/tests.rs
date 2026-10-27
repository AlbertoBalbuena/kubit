//! End-to-end engine tests on the exact reference kernel (oracle values).

use crate::*;
use kubit_kernel_api::Kernel;
use kubit_kernel_api::testing::BoxKernel;
use proptest::prelude::*;
use serde_json::{Value, json};

fn session() -> Session {
    Session::new(Box::new(|| Box::new(BoxKernel::new()) as Box<dyn Kernel>))
}

fn volume(s: &mut Session) -> f64 {
    s.execute("body.measure", json!({})).unwrap().data["bodies"][0]["volume_mm3"]
        .as_f64()
        .unwrap()
}

#[test]
fn box_push_pull_undo_redo() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    assert_eq!(volume(&mut s), 240_000.0);
    s.execute("face.push_pull", json!({"face": "F1:+Z", "distance": 10.0}))
        .unwrap();
    assert_eq!(volume(&mut s), 300_000.0);
    // The moved face keeps its persistent name, so it can be pushed again.
    s.execute(
        "face.push_pull",
        json!({"face": "F1:+Z", "distance": -20.0}),
    )
    .unwrap();
    assert_eq!(volume(&mut s), 180_000.0);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(volume(&mut s), 300_000.0);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(volume(&mut s), 240_000.0);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(volume(&mut s), 300_000.0);
    assert_eq!(s.doc().features.len(), 2);
    // A new change drops the redo branch.
    s.execute("face.push_pull", json!({"face": "F1:+X", "distance": 5.0}))
        .unwrap();
    assert!(!s.can_redo());
    assert_eq!(volume(&mut s), 105.0 * 60.0 * 50.0);
}

#[test]
fn box_faces_have_role_names() {
    let mut s = session();
    s.execute(
        "solid.box",
        json!({"origin": [1.0, 2.0, 3.0], "size": [10.0, 20.0, 30.0]}),
    )
    .unwrap();
    let faces = s.execute("body.faces", json!({"body": "F1"})).unwrap().data;
    let names: Vec<&str> = faces["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["face"].as_str().unwrap())
        .collect();
    for role in ["+X", "-X", "+Y", "-Y", "+Z", "-Z"] {
        assert!(names.contains(&format!("F1:{role}").as_str()), "{names:?}");
    }
}

#[test]
fn failed_command_changes_nothing() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [10.0, 10.0, 10.0]}))
        .unwrap();
    let before = s.doc().clone();
    // Would collapse the box: kernel error, transaction rolled back.
    let e = s
        .execute(
            "face.push_pull",
            json!({"face": "F1:+Z", "distance": -10.0}),
        )
        .unwrap_err();
    assert!(matches!(e, CommandError::Kernel(_)), "{e:?}");
    assert_eq!(s.doc(), &before);
    assert!(matches!(
        s.execute("face.push_pull", json!({"face": "F9:+Z", "distance": 1.0})),
        Err(CommandError::Reference(_))
    ));
    assert!(matches!(
        s.execute("solid.box", json!({"size": [1.0, 1.0]})),
        Err(CommandError::InvalidParams { .. })
    ));
    assert!(matches!(
        s.execute("solid.box", json!({"size": [1.0, 1.0, 1.0], "x": 1})),
        Err(CommandError::InvalidParams { .. })
    ));
    assert_eq!(s.doc(), &before);
}

#[test]
fn disabled_and_unknown_commands() {
    let mut s = session();
    assert!(matches!(
        s.execute("edit.undo", json!({})),
        Err(CommandError::Disabled { .. })
    ));
    assert!(matches!(
        s.execute("face.push_pull", json!({"face": "F1:+Z", "distance": 1.0})),
        Err(CommandError::Disabled { .. })
    ));
    assert!(matches!(
        s.execute("nope", Value::Null),
        Err(CommandError::UnknownCommand(_))
    ));
}

#[test]
fn save_open_round_trip_rebuilds_the_same_model() {
    let dir = std::env::temp_dir().join(format!("kubit-engine-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    s.execute("face.push_pull", json!({"face": "F1:+Z", "distance": 10.0}))
        .unwrap();
    for ext in ["kubit", "json"] {
        let path = dir.join(format!("part.{ext}"));
        s.execute("doc.save", json!({"path": path})).unwrap();
        let mut t = session();
        t.execute("doc.open", json!({"path": path})).unwrap();
        assert_eq!(t.doc(), s.doc());
        assert_eq!(
            kubit_doc::to_json(t.doc()).unwrap(),
            kubit_doc::to_json(s.doc()).unwrap()
        );
        assert_eq!(volume(&mut t), 300_000.0);
        // Names rebuild identically.
        let a = s.execute("body.faces", json!({})).unwrap().data;
        let b = t.execute("body.faces", json!({})).unwrap().data;
        assert_eq!(a, b);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stl_export_matches_volume() {
    let dir = std::env::temp_dir().join(format!("kubit-engine-stl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    let path = dir.join("box.stl");
    s.execute("file.export_stl", json!({"path": path})).unwrap();
    let mesh = kubit_io::stl::read_binary(&std::fs::read(&path).unwrap()).unwrap();
    assert!((mesh.volume() - 240_000.0).abs() < 1e-3);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn agent_author_is_recorded() {
    let mut s = session();
    s.execute_as(
        kubit_doc::Author::Agent {
            name: "test-agent".into(),
        },
        "solid.box",
        json!({"size": [1.0, 1.0, 1.0]}),
    )
    .unwrap();
    assert_eq!(
        s.doc().features[0].author,
        kubit_doc::Author::Agent {
            name: "test-agent".into()
        }
    );
}

#[test]
fn every_command_has_an_object_schema_and_description() {
    let s = session();
    assert!(s.registry().len() >= 13);
    for c in s.registry().iter() {
        let schema = (c.params_schema)();
        assert_eq!(schema["type"], "object", "{}", c.id);
        assert!(!c.description.is_empty());
        assert!(c.id.contains('.'));
    }
}

#[test]
fn garbage_collection_keeps_undo_targets_alive() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [10.0, 10.0, 10.0]}))
        .unwrap();
    for _ in 0..5 {
        s.execute("face.push_pull", json!({"face": "F1:+Z", "distance": 1.0}))
            .unwrap();
    }
    for _ in 0..5 {
        s.undo();
    }
    assert_eq!(volume(&mut s), 1000.0);
    for _ in 0..5 {
        s.redo();
    }
    assert_eq!(volume(&mut s), 1500.0);
}

/// Edge names of body F1 → edge length, from the kernel.
fn edge_lengths(s: &Session) -> std::collections::BTreeMap<String, f64> {
    let b = &s.model().bodies[&kubit_doc::FeatureId(1)];
    b.edges
        .iter()
        .map(|(name, e)| {
            let [p, q] = s.kernel().edge_endpoints(*e).unwrap();
            let len = (0..3).map(|i| (q[i] - p[i]).powi(2)).sum::<f64>().sqrt();
            (name.to_string(), len)
        })
        .collect()
}

#[test]
fn box_edges_are_named_by_their_faces() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    let lengths = edge_lengths(&s);
    assert_eq!(lengths.len(), 12);
    // Sorted face names, so `+X` comes before `+Z` and `-X` after `+X`.
    assert_eq!(lengths["F1:+X|F1:+Z"], 60.0);
    assert_eq!(lengths["F1:+Y|F1:-X"], 40.0);
    assert_eq!(lengths["F1:+Z|F1:-Y"], 100.0);
    assert!(!lengths.contains_key("F1:+Z|F1:-Z"));
}

#[test]
fn edge_names_survive_push_pull_and_undo() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    let before = edge_lengths(&s);
    s.execute("face.push_pull", json!({"face": "F1:+Z", "distance": 10.0}))
        .unwrap();
    let after = edge_lengths(&s);
    // Same twelve names; only the four vertical edges grew.
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    for (name, len) in &after {
        let vertical = !name.contains("+Z") && !name.contains("-Z");
        let expected = if vertical { 50.0 } else { before[name] };
        assert_eq!(*len, expected, "{name}");
    }
    // A selected edge resolves before and after another push/pull.
    s.execute("selection.set", json!({"edges": ["F1:+Z|F1:+X"]}))
        .unwrap();
    s.execute("face.push_pull", json!({"face": "F1:+X", "distance": 5.0}))
        .unwrap();
    let name: kubit_doc::EdgeName = "F1:+X|F1:+Z".parse().unwrap();
    assert!(s.model().body_of_edge(&name).is_some());
    assert_eq!(edge_lengths(&s)["F1:+X|F1:+Z"], 60.0);
    s.undo();
    s.undo();
    assert_eq!(edge_lengths(&s), before);
}

#[test]
fn edge_tags_map_to_names() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [10.0, 10.0, 10.0]}))
        .unwrap();
    let f1 = kubit_doc::FeatureId(1);
    let edges = s.body_edges(f1, 0.1).unwrap().to_vec();
    assert_eq!(edges.len(), 12);
    let names: std::collections::BTreeSet<String> = edges
        .iter()
        .map(|(tag, _)| s.edge_name(f1, *tag).unwrap().to_string())
        .collect();
    assert_eq!(names.len(), 12);
    assert_eq!(s.edge_name(f1, kubit_mesh::EdgeTag(u64::MAX)), None);
    assert_eq!(s.edge_name(kubit_doc::FeatureId(9), edges[0].0), None);
}

#[test]
fn selection_takes_faces_and_edges() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [10.0, 10.0, 10.0]}))
        .unwrap();
    // Old form: faces only.
    s.execute("selection.set", json!({"faces": ["F1:+Z"]}))
        .unwrap();
    assert_eq!(s.selection().faces.len(), 1);
    assert!(s.selection().edges.is_empty());
    s.execute(
        "selection.set",
        json!({"faces": ["F1:-Z"], "edges": ["F1:+Z|F1:+X", "F1:+X|F1:+Z", "F1:-Y|F1:-X"]}),
    )
    .unwrap();
    // Either order names the same edge.
    assert_eq!(s.selection().edges.len(), 2);
    let info = s.execute("doc.inspect", json!({})).unwrap().data;
    assert_eq!(
        info["selected_edges"],
        json!(["F1:+X|F1:+Z", "F1:-X|F1:-Y"])
    );
    assert_eq!(info["bodies"][0]["edges"], 12);
    s.execute("selection.set", json!({})).unwrap();
    assert!(s.selection().is_empty());
    // Bad references change nothing.
    s.execute("selection.set", json!({"edges": ["F1:+X|F1:+Z"]}))
        .unwrap();
    for bad in [
        json!({"edges": ["F1:+Z|F1:-Z"]}),
        json!({"edges": ["F1:+Z"]}),
        json!({"edges": "F1:+X|F1:+Z"}),
        json!({"edges": [1]}),
        json!({"vertices": []}),
    ] {
        assert!(s.execute("selection.set", bad.clone()).is_err(), "{bad}");
        assert_eq!(s.selection().edges.len(), 1);
    }
}

fn hostile() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(0.0),
        Just(-0.0),
        Just(-1.0),
        Just(1e-12),
        Just(f64::MAX),
        Just(1e7),
        -1e4..1e4_f64,
    ]
}

proptest! {
    #[test]
    fn hostile_box_and_push_pull_never_panic(
        origin in proptest::array::uniform3(hostile()),
        size in proptest::array::uniform3(hostile()),
        face in prop_oneof![Just("F1:+Z"), Just("F1:-X"), Just("F1:g0"), Just("garbage"), Just("F1:+Z/9")],
        d in hostile(),
        nan in any::<bool>(),
    ) {
        let mut s = session();
        // JSON cannot carry NaN/∞, so the hostile non-finite case is a string or null.
        let size_json = if nan { json!([Value::Null, "inf", 1]) } else { json!(size) };
        let _ = s.execute("solid.box", json!({"origin": origin, "size": size_json}));
        let _ = s.execute("face.push_pull", json!({"face": face, "distance": d}));
        let _ = s.execute("edit.undo", json!({}));
        let _ = s.execute("edit.redo", json!({}));
        let _ = s.execute("doc.inspect", json!({}));
        prop_assert!(s.doc().validate().is_ok());
    }

    #[test]
    fn hostile_selection_never_panics(
        faces in proptest::collection::vec(".{0,40}", 0..4),
        edges in proptest::collection::vec(
            prop_oneof![
                Just("F1:+X|F1:+Z".to_owned()),
                Just("F1:+Z|F1:+Z".to_owned()),
                Just("|".to_owned()),
                Just("F1:+X||F1:+Z".to_owned()),
                ".{0,600}",
                "F[0-9]{1,12}:[+-][XYZ](/[0-9]{1,12})?\\|F[0-9]{1,12}:g[0-9]{1,12}",
            ],
            0..4,
        ),
        raw in prop_oneof![Just(Value::Null), Just(json!(1)), Just(json!([[]])), Just(json!({"x": 1}))],
        with_body in any::<bool>(),
    ) {
        let mut s = session();
        if with_body {
            s.execute("solid.box", json!({"size": [10.0, 10.0, 10.0]})).unwrap();
        }
        let _ = s.execute("selection.set", json!({"faces": faces, "edges": edges}));
        let _ = s.execute("selection.set", json!({"edges": raw.clone()}));
        let _ = s.execute("selection.set", json!({"faces": raw}));
        // Whatever was accepted resolves in the model.
        for e in &s.selection().edges {
            prop_assert!(s.model().body_of_edge(e).is_some());
        }
    }
}
