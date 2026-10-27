//! Edge names v0 (ADR-0004) on the real backend, where push/pull shares the
//! unchanged edges between the input and the result.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_doc::{EdgeName, FeatureId};
use kubit_engine::Session;
use kubit_kernel_api::Kernel;
use kubit_kernel_arris::ArrisKernel;
use serde_json::json;
use std::collections::BTreeMap;

fn session() -> Session {
    Session::new(Box::new(|| Box::new(ArrisKernel::new()) as Box<dyn Kernel>))
}

fn edge_lengths(s: &Session) -> BTreeMap<String, f64> {
    s.model().bodies[&FeatureId(1)]
        .edges
        .iter()
        .map(|(name, e)| {
            let [p, q] = s.kernel().edge_endpoints(*e).unwrap();
            let len = (0..3).map(|i| (q[i] - p[i]).powi(2)).sum::<f64>().sqrt();
            (name.to_string(), len)
        })
        .collect()
}

#[test]
fn edge_names_are_stable_through_push_pull() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [100.0, 60.0, 40.0]}))
        .unwrap();
    let before = edge_lengths(&s);
    assert_eq!(before.len(), 12);
    s.execute("face.push_pull", json!({"face": "F1:+Z", "distance": 10.0}))
        .unwrap();
    let after = edge_lengths(&s);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    for (name, len) in &after {
        let vertical = !name.contains("+Z") && !name.contains("-Z");
        let expected = if vertical { 50.0 } else { before[name] };
        assert!((len - expected).abs() < 1e-9, "{name}: {len}");
    }
    // The bottom edges are shared with the input body; they still resolve to
    // this body's side faces.
    let body = &s.model().bodies[&FeatureId(1)];
    let bottom: EdgeName = "F1:-Y|F1:-Z".parse().unwrap();
    let pair = s
        .kernel()
        .edge_faces(body.kernel, body.edges[&bottom])
        .unwrap();
    for f in pair {
        assert!(body.name_of(f).is_some());
    }
    s.execute("selection.set", json!({"edges": ["F1:-Z|F1:-Y"]}))
        .unwrap();
    assert_eq!(s.selection().edges.len(), 1);
}

#[test]
fn mesh_edges_map_to_the_same_names_after_reopen() {
    let mut s = session();
    s.execute("solid.box", json!({"size": [10.0, 20.0, 30.0]}))
        .unwrap();
    s.execute("face.push_pull", json!({"face": "F1:+X", "distance": 5.0}))
        .unwrap();
    let f1 = FeatureId(1);
    let names = |s: &mut Session| {
        let edges = s.body_edges(f1, 0.05).unwrap().to_vec();
        let mut out: Vec<String> = edges
            .iter()
            .map(|(tag, _)| s.edge_name(f1, *tag).unwrap().to_string())
            .collect();
        out.sort();
        out
    };
    let a = names(&mut s);
    assert_eq!(a.len(), 12);
    let mut t = session();
    t.load(s.doc().clone());
    assert_eq!(names(&mut t), a);
}
