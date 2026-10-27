//! Solver oracles, diagnosis, dragging and profiles, written in the text format.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_sketch::text::{Parsed, parse};
use kubit_sketch::{Constraint, Freedom, LoopShape, SolveOptions, drag, profiles, solve};
use proptest::prelude::*;

fn sketch(src: &str) -> Parsed {
    parse(src).unwrap_or_else(|e| panic!("{e}"))
}

fn pt(p: &Parsed, name: &str) -> [f64; 2] {
    p.sketch.point(p.id(name).unwrap()).unwrap()
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

const RECT: &str = "
point p1 0 0
point p2 38 2
point p3 41 27
point p4 -1 31
line bottom p1 p2
line right p2 p3
line top p3 p4
line left p4 p1
fix p1
horizontal bottom
horizontal top
vertical left
vertical right
length bottom 40
length left 30
";

#[test]
fn triangle_3_4_5() {
    let mut p = sketch(
        "
point a 0 0
point b 2.5 0.3
point c 0.2 3.6
line ab a b
line bc b c
line ca c a
fix a
horizontal ab
length ab 3
length bc 5
length ca 4
",
    );
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged, "{r:?}");
    assert!((dist(pt(&p, "a"), pt(&p, "b")) - 3.0).abs() < 1e-9);
    assert!((dist(pt(&p, "b"), pt(&p, "c")) - 5.0).abs() < 1e-9);
    assert!((dist(pt(&p, "c"), pt(&p, "a")) - 4.0).abs() < 1e-9);
    assert_eq!(r.dof, 0, "a fixed vertex, a direction and three lengths");
    assert!(r.entities.values().all(|f| *f == Freedom::Fully));
}

#[test]
fn rectangle_is_fully_constrained() {
    let mut p = sketch(RECT);
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged && r.dof == 0, "{r:?}");
    assert!(r.redundant.is_empty() && r.conflicting.is_empty());
    let p3 = pt(&p, "p3");
    assert!(
        (p3[0] - 40.0).abs() < 1e-9 && (p3[1] - 30.0).abs() < 1e-9,
        "{p3:?}"
    );
}

#[test]
fn under_constrained_reports_what_is_free() {
    let mut p = sketch(
        "
point a 0 0
point b 10 1
line l a b
fix a
",
    );
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert_eq!(r.dof, 2, "b moves freely");
    assert_eq!(r.entities.get(&p.id("a").unwrap()), Some(&Freedom::Fully));
    assert_eq!(
        r.entities.get(&p.id("b").unwrap()),
        Some(&Freedom::Under(2))
    );
    assert_eq!(
        r.entities.get(&p.id("l").unwrap()),
        Some(&Freedom::Under(2))
    );
}

#[test]
fn redundant_constraint_is_named() {
    // Opposite sides are already parallel through horizontal + horizontal.
    let src = format!("{RECT}\nparallel bottom top\n");
    let mut p = sketch(&src);
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged);
    assert_eq!(r.redundant, vec![*p.constraints.last().unwrap()]);
    assert!(r.conflicting.is_empty());
}

#[test]
fn conflicting_dimension_is_named() {
    let src = format!("{RECT}\nlength right 25\n");
    let mut p = sketch(&src);
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(!r.converged);
    assert!(
        r.conflicting.contains(p.constraints.last().unwrap()),
        "{r:?}"
    );
    assert!(r.entities.values().any(|f| *f == Freedom::Over));
}

#[test]
fn arc_tangent_to_two_lines() {
    // A fillet arc of radius 5 in the corner of two perpendicular lines.
    let mut p = sketch(
        "
point o 0 0
point x 20 0
point y 0 20
line lx o x
line ly o y
point c 6 6
point s 6 0.5
point e 0.4 6
arc a c s e
fix o
horizontal lx
vertical ly
length lx 20
length ly 20
on_line s lx
on_line e ly
tangent lx a
tangent ly a
radius a 5
",
    );
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged, "{r:?}");
    let c = pt(&p, "c");
    assert!(
        (c[0] - 5.0).abs() < 1e-8 && (c[1] - 5.0).abs() < 1e-8,
        "{c:?}"
    );
    // The tangent point is a double root (point on line + radius already imply the
    // touch), so it converges only to about the square root of the tolerance.
    assert!(
        (pt(&p, "s")[0] - 5.0).abs() < 1e-4,
        "tangent point under the centre"
    );
}

#[test]
fn circle_inscribed_in_a_square() {
    let src = format!(
        "{RECT}
point c 18 14
circle k c 9
tangent bottom k
tangent top k
tangent left k
"
    );
    let mut p = sketch(&src);
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged, "{r:?}");
    let k = p.id("k").unwrap();
    assert!((p.sketch.radius(k).unwrap() - 15.0).abs() < 1e-8);
    let c = pt(&p, "c");
    assert!(
        (c[0] - 15.0).abs() < 1e-8 && (c[1] - 15.0).abs() < 1e-8,
        "{c:?}"
    );
}

#[test]
fn angle_and_reference_dimensions() {
    let mut p = sketch(
        "
point a 0 0
point b 10 0
point c 3 7
line l1 a b
line l2 a c
fix a
horizontal l1
length l1 10
length l2 10
angle l1 l2 60
distance b c 1 ref
",
    );
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged && r.dof == 0, "{r:?}");
    let c = pt(&p, "c");
    assert!(
        (c[0] - 5.0).abs() < 1e-8 && (c[1] - 75f64.sqrt()).abs() < 1e-8,
        "{c:?}"
    );
    assert!(
        (dist(pt(&p, "b"), c) - 10.0).abs() < 1e-8,
        "the reference does not drive"
    );
}

#[test]
fn dragging_moves_only_what_is_free() {
    let mut p = sketch(
        "
point a 0 0
point b 10 0
point c 10 10
line ab a b
line bc b c
fix a
horizontal ab
length ab 10
vertical bc
",
    );
    solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    let c_id = p.id("c").unwrap();
    let r = drag(&mut p.sketch, c_id, [30.0, 25.0], &SolveOptions::default()).unwrap();
    assert!(r.converged, "{r:?}");
    let (a, b, c) = (pt(&p, "a"), pt(&p, "b"), pt(&p, "c"));
    assert!(
        dist(a, [0.0, 0.0]) < 1e-9 && dist(b, [10.0, 0.0]) < 1e-9,
        "fixed part stays"
    );
    assert!(
        (c[0] - 10.0).abs() < 1e-9,
        "c stays on the vertical through b"
    );
    assert!(
        (c[1] - 25.0).abs() < 1e-3,
        "and follows the pointer in v: {c:?}"
    );
}

#[test]
fn rectangle_with_a_hole_is_one_region() {
    let src = format!("{RECT}\npoint c 20 15\ncircle hole c 4\n");
    let mut p = sketch(&src);
    solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    let regions = profiles(&p.sketch);
    assert_eq!(regions.len(), 1, "{regions:#?}");
    let r = &regions[0];
    assert_eq!(r.holes.len(), 1);
    assert!(
        matches!(r.holes[0].shape, LoopShape::Circle { radius, .. } if (radius - 4.0).abs() < 1e-12)
    );
    assert_eq!(r.outer.entities.len(), 4);
    let expected = 1200.0 - std::f64::consts::PI * 16.0;
    assert!((r.area - expected).abs() < 1e-9, "{}", r.area);
}

#[test]
fn separate_loops_open_chains_and_construction() {
    let mut p = sketch(
        "
point a 0 0
point b 10 0
point c 10 10
line l1 a b
line l2 b c
line l3 c a
point d 50 0
point e 60 0
point f 60 10
point g 50 10
line m1 d e
line m2 e f
line m3 f g
line m4 g d
point h 100 0
point i 110 0
line open h i
construction m4
",
    );
    solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    let regions = profiles(&p.sketch);
    assert_eq!(
        regions.len(),
        1,
        "the square lost a side to construction: {regions:#?}"
    );
    assert_eq!(regions[0].outer.entities.len(), 3);
    p.sketch
        .entities
        .get_mut(&p.id("m4").unwrap())
        .unwrap()
        .construction = false;
    assert_eq!(profiles(&p.sketch).len(), 2);
}

#[test]
fn coincident_points_close_a_loop() {
    let mut p = sketch(
        "
point a 0 0
point b 10 0
point b2 10.5 0.2
point c 10 10
point c2 9.7 10.1
point a2 0.1 -0.2
line l1 a b
line l2 b2 c
line l3 c2 a2
coincident b b2
coincident c c2
coincident a a2
",
    );
    solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    let regions = profiles(&p.sketch);
    assert_eq!(regions.len(), 1);
    assert!((regions[0].area - 50.0).abs() < 1.0, "{}", regions[0].area);
}

#[test]
fn slot_with_arcs_is_a_region() {
    let mut p = sketch(
        "
point a 0 0
point b 20 0
point c 20 10
point d 0 10
point r 20 5
point l 0 5
line bottom a b
arc right r b c
line top c d
arc left l d a
",
    );
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged);
    let regions = profiles(&p.sketch);
    assert_eq!(regions.len(), 1);
    let expected = 200.0 + std::f64::consts::PI * 25.0;
    assert!(
        (regions[0].area - expected).abs() < 1e-9,
        "{}",
        regions[0].area
    );
}

#[test]
fn edits_are_checked() {
    let mut p = sketch(RECT);
    let bottom = p.id("bottom").unwrap();
    assert!(
        p.sketch
            .add_constraint(
                Constraint::Length {
                    line: bottom,
                    value: 1e9
                },
                false
            )
            .is_err()
    );
    assert!(parse("point a 0 0\nline l a a\n").is_err());
    assert!(parse("point a nan 0\n").is_err());
    assert!(parse("bogus a\n").is_err());
    let len = *p.constraints.last().unwrap();
    p.sketch.set_dimension(len, 45.0).unwrap();
    let r = solve(&mut p.sketch, &SolveOptions::default()).unwrap();
    assert!(r.converged);
    assert!(
        (pt(&p, "p4")[1] - 45.0).abs() < 1e-9,
        "the left side follows its new length"
    );
}

fn hostile() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        Just(0.0),
        Just(-1.0),
        Just(1e9),
        Just(1e-300),
        -1e3..1e3f64,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn hostile_numbers_never_panic(
        coords in proptest::collection::vec(hostile(), 8),
        dims in proptest::collection::vec(hostile(), 4),
        target in proptest::collection::vec(hostile(), 2),
    ) {
        let mut s = kubit_sketch::Sketch::new();
        let pts: Vec<_> = coords
            .chunks(2)
            .filter_map(|c| s.add_point(c[0], c[1]).ok())
            .collect();
        let mut lines = Vec::new();
        for w in pts.windows(2) {
            if let Ok(l) = s.add_line(w[0], w[1]) {
                lines.push(l);
            }
        }
        for (i, v) in dims.iter().enumerate() {
            if let Some(&l) = lines.get(i % lines.len().max(1)) {
                let _ = s.add_constraint(Constraint::Length { line: l, value: *v }, false);
                let _ = s.add_constraint(Constraint::Angle { a: l, b: l, value: *v }, false);
            }
            if let (Some(&a), Some(&b)) = (pts.first(), pts.get(1 + i % 3)) {
                let _ = s.add_constraint(Constraint::Distance { a, b, value: *v }, false);
                let _ = s.add_constraint(Constraint::Coincident { a, b }, false);
            }
        }
        if let Some(&l) = lines.first() {
            let _ = s.add_constraint(Constraint::Horizontal { line: l }, false);
        }
        let report = solve(&mut s, &SolveOptions::default());
        if report.is_ok() {
            prop_assert!(kubit_sketch::equations::Layout::of(&s).1.iter().all(|v| v.is_finite()));
        }
        if let Some(&p) = pts.first() {
            let _ = drag(&mut s, p, [target[0], target[1]], &SolveOptions::default());
        }
        let _ = profiles(&s);
    }
}
