//! Spike adapter for the solidcore kernel. See `spikes/kernel/CASES.md` for the
//! geometry of every case and `backends/solidcore/NOTES.md` for the API used.

mod shell;

use harness::corpus::*;
use harness::{failed, not_exposed, timed, Backend, Hostile, History, Produced, SpikeError, SpikeResult, Topo};
use solidcore::blend::fillet_executive::api::{fillet_edges, variable_radius_fillet};
use solidcore::boolean::api_brep::{boolean_difference, boolean_lists, boolean_union};
use solidcore::curve::curve::Curve;
use solidcore::geom::attribute::{AttributeStore, ObjectKey};
use solidcore::mass::integrator::properties::{brep_area, brep_volume, PropertyAttributes};
use solidcore::primitive::api_curves::{create_arc, create_circle, create_helix, create_line_segment, HelixParams};
use solidcore::primitive::api_primitives::{create_box, create_cylinder, create_sphere};
use solidcore::primitive::primitive_creation::SweepOptions;
use solidcore::query::brep::{calculate_bounding_box, BoundingBoxOptions};
use solidcore::sweep::api_brep::{create_curve_sweep, create_linear_sweep, create_rotational_sweep};
use solidcore::sweep::api_surfaces::create_skin_primitive;
use solidcore::topology::brep::{p1, p2};
use solidcore::topology::proto_topology::p1::api_transform::{api_rotate, api_translate, TransformObject};
use solidcore::topology::{edge, face, integrity, vertex, EdgeId, FaceId};
use solidcore::{Brep, Kernel, Point3, SolidError, Vec3};

pub struct Solidcore;

/// Accuracy passed to the mass-property integrator.
const MASS_ACCURACY: f64 = 1e-6;
/// Distance tolerance for picking edges and faces by position.
const PICK_TOL: f64 = 1e-6;

const NO_MESH: &str = "solidcore has no public B-rep tessellation (only the low-level TessSrfCache; \
the separate nurbs-grid-mesh crate would need a hand-written Brep→NURBS adapter), so mesh = None";
const NO_STEP: &str = "solidcore has no STEP export of a B-rep (step-export writes the neutral \
step-import model, planar single-shell only)";

fn kerr(op: &str, e: SolidError) -> SpikeError {
    let tag = if e.is_unported() { "Unported: " } else { "" };
    SpikeError::Failed(format!("{op}: {tag}{e}"))
}

trait Ctx<T> {
    fn ctx(self, op: &str) -> SpikeResult<T>;
}

impl<T> Ctx<T> for solidcore::Result<T> {
    fn ctx(self, op: &str) -> SpikeResult<T> {
        self.map_err(|e| kerr(op, e))
    }
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn bx(k: &mut Kernel) -> SpikeResult<Brep> {
    create_box(k, p(0.0, 0.0, 0.0), BOX_A, BOX_B, BOX_C).ctx("create_box")
}

fn rotate(k: &mut Kernel, b: &mut Brep, axis: Vec3, deg: f64) -> SpikeResult<()> {
    api_rotate(k, Some(TransformObject::Brep(b)), p(0.0, 0.0, 0.0), axis, deg).ctx("api_rotate")
}

fn rotate_curve(k: &mut Kernel, c: &mut Curve, axis: Vec3, deg: f64) -> SpikeResult<()> {
    api_rotate(k, Some(TransformObject::Curve(c)), p(0.0, 0.0, 0.0), axis, deg).ctx("api_rotate(curve)")
}

fn translate(k: &mut Kernel, b: &mut Brep, v: Vec3) -> SpikeResult<()> {
    api_translate(k, Some(TransformObject::Brep(b)), v).ctx("api_translate")
}

/// Start, end and mid point of an edge.
fn edge_samples(k: &Kernel, b: &Brep, e: EdgeId) -> SpikeResult<Vec<Point3>> {
    let mut out = Vec::new();
    let (s, t) = edge::start_end_vertices(b, e).ctx("start_end_vertices")?;
    for v in [s, t].into_iter().flatten() {
        out.push(vertex::point(b, v).ctx("vertex::point")?);
    }
    if let Some(c) = edge::curve(b, e).ctx("edge::curve")? {
        let iv = edge::interval(b, e).ctx("edge::interval")?;
        out.push(c.evaluate_point(k, iv.mid()).ctx("evaluate_point")?);
    }
    Ok(out)
}

/// Every edge whose sample points all satisfy `pred`.
fn pick_edges(k: &Kernel, b: &Brep, pred: impl Fn(Point3) -> bool) -> SpikeResult<Vec<EdgeId>> {
    let mut out = Vec::new();
    for e in p1::edges(b, None).ctx("p1::edges")? {
        let s = edge_samples(k, b, e)?;
        if !s.is_empty() && s.iter().all(|q| pred(*q)) {
            out.push(e);
        }
    }
    Ok(out)
}

fn pick_one_edge(k: &Kernel, b: &Brep, what: &str, pred: impl Fn(Point3) -> bool) -> SpikeResult<EdgeId> {
    let found = pick_edges(k, b, pred)?;
    match found.as_slice() {
        [e] => Ok(*e),
        _ => failed(format!("adapter: expected 1 edge for {what}, found {}", found.len())),
    }
}

/// The face whose surface, sampled inside its uv domain, satisfies `pred` (planar faces).
fn pick_face(k: &Kernel, b: &Brep, what: &str, pred: impl Fn(Point3) -> bool) -> SpikeResult<FaceId> {
    let mut found = Vec::new();
    for f in p1::faces(b, None).ctx("p1::faces")? {
        let rec = b.face(f).ctx("face")?;
        let Some(s) = rec.surface.as_ref() else { continue };
        let ok = [(0.37, 0.61), (0.5, 0.5), (0.13, 0.87)]
            .iter()
            .map(|&(u, v)| s.evaluate_point(k, rec.uv_domain.evaluate(u, v)))
            .collect::<solidcore::Result<Vec<_>>>()
            .ctx("surface::evaluate_point")?
            .into_iter()
            .all(&pred);
        if ok {
            found.push(f);
        }
    }
    match found.as_slice() {
        [f] => Ok(*f),
        _ => failed(format!("adapter: expected 1 face for {what}, found {}", found.len())),
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < PICK_TOL
}

/// The 5a edge: along X at y = 0, z = c.
fn top_front_edge(k: &Kernel, b: &Brep, c: f64) -> SpikeResult<EdgeId> {
    pick_one_edge(k, b, "top front edge", |q| near(q.y, 0.0) && near(q.z, c))
}

/// Box minus the through-hole of case 1a.
fn hole_box(k: &mut Kernel) -> SpikeResult<Brep> {
    let b = bx(k)?;
    let cyl = create_cylinder(k, p(50.0, 30.0, -1.0), HOLE_R, BOX_C + 2.0).ctx("create_cylinder")?;
    boolean_difference(k, b, cyl).ctx("boolean_difference")
}

/// Stadium outline in the XY plane, counter-clockwise.
fn stadium_profile(k: &mut Kernel) -> SpikeResult<Vec<Curve>> {
    let (r, l) = (STADIUM_R, STADIUM_LS);
    let _ = k;
    Ok(vec![
        Curve::from(create_line_segment(p(0.0, -r, 0.0), p(l, -r, 0.0)).ctx("line")?),
        Curve::from(create_arc(p(l, 0.0, 0.0), r, -90.0, 90.0).ctx("arc")?),
        Curve::from(create_line_segment(p(l, r, 0.0), p(0.0, r, 0.0)).ctx("line")?),
        Curve::from(create_arc(p(0.0, 0.0, 0.0), r, 90.0, 270.0).ctx("arc")?),
    ])
}

struct Built {
    brep: Brep,
    secs: f64,
    history: History,
    notes: Vec<String>,
}

fn built(brep: Brep, secs: f64, history: History) -> Built {
    Built { brep, secs, history, notes: Vec::new() }
}

fn sweep_history(start: usize, side: usize, end: usize) -> History {
    History::Partial(format!("sweep returns start/side/end faces: {start}/{side}/{end}; no edge or vertex map"))
}

fn build_case(k: &mut Kernel, id: &str) -> SpikeResult<Built> {
    let none = History::None;
    match id {
        "1a" | "13c" => {
            let (r, s) = timed(|| hole_box(k));
            Ok(built(r?, s, none))
        }
        "1b" => {
            let (r, s) = timed(|| {
                let b = bx(k)?;
                let cyl = create_cylinder(k, p(50.0, 30.0, BOX_C - BLIND_DEPTH), HOLE_R, BLIND_DEPTH + 1.0)
                    .ctx("create_cylinder")?;
                boolean_difference(k, b, cyl).ctx("boolean_difference")
            });
            Ok(built(r?, s, none))
        }
        "2a" | "2b" => {
            let dy = if id == "2a" { 0.0 } else { BOX_B };
            let (r, s) = timed(|| {
                let a = bx(k)?;
                let b = create_box(k, p(BOX_A, dy, 0.0), BOX_A, BOX_B, BOX_C).ctx("create_box")?;
                boolean_union(k, a, b).ctx("boolean_union")
            });
            Ok(built(r?, s, none))
        }
        "3a" => {
            let (r, s) = timed(|| {
                let a = bx(k)?;
                let mut c = create_cylinder(k, p(0.0, 0.0, 0.0), HOLE_R, BOX_A).ctx("create_cylinder")?;
                rotate(k, &mut c, Vec3::new(0.0, 1.0, 0.0), 90.0)?;
                // Place by the bounding box so the rotation sense does not matter.
                let bb = calculate_bounding_box(k, &c, BoundingBoxOptions::default()).ctx("bbox")?;
                let (lo, mid) = (bb.min(), bb.mid());
                translate(k, &mut c, Vec3::new(-lo.x, 30.0 - mid.y, 50.0 - mid.z))?;
                boolean_union(k, a, c).ctx("boolean_union")
            });
            Ok(built(r?, s, none))
        }
        "3b" => {
            let (r, s) = timed(|| {
                let a = bx(k)?;
                let sp = create_sphere(k, p(50.0, 30.0, 50.0), HOLE_R).ctx("create_sphere")?;
                boolean_union(k, a, sp).ctx("boolean_union")
            });
            Ok(built(r?, s, none))
        }
        "4" => {
            let (r, s) = timed(|| -> SpikeResult<(Brep, &'static str)> {
                let plate = create_box(k, p(0.0, 0.0, 0.0), PLATE[0], PLATE[1], PLATE[2]).ctx("create_box")?;
                let mut pins = Vec::new();
                for [x, y] in pin_centres() {
                    pins.push(create_cylinder(k, p(x, y, PIN_Z0), PIN_R, PIN_Z1 - PIN_Z0).ctx("create_cylinder")?);
                }
                match boolean_lists(k, std::slice::from_ref(&plate), &[], &pins, &[], 1) {
                    Ok(mut out) if out.breps.len() == 1 => {
                        if let Some(b) = out.breps.pop() {
                            return Ok((b, "n-ary union: boolean_lists([plate], [100 pins], union) → 1 body"));
                        }
                        failed("boolean_lists returned no body")
                    }
                    Ok(out) => failed(format!("boolean_lists union returned {} bodies", out.breps.len())),
                    Err(e) => Err(kerr("boolean_lists", e)),
                }
            });
            let (b, note) = r?;
            let mut out = built(b, s, none);
            out.notes.push(note.to_owned());
            Ok(out)
        }
        "4fold" => {
            let (r, s) = timed(|| {
                let mut acc = create_box(k, p(0.0, 0.0, 0.0), PLATE[0], PLATE[1], PLATE[2]).ctx("create_box")?;
                for [x, y] in pin_centres() {
                    let c = create_cylinder(k, p(x, y, PIN_Z0), PIN_R, PIN_Z1 - PIN_Z0).ctx("create_cylinder")?;
                    acc = boolean_union(k, acc, c).ctx("boolean_union")?;
                }
                Ok::<_, SpikeError>(acc)
            });
            Ok(built(r?, s, none))
        }
        "5a" | "7a" => {
            let mut b = bx(k)?;
            let e = top_front_edge(k, &b, BOX_C)?;
            let (xsect, r) = if id == "5a" { (1, FILLET_R) } else { (0, CHAMFER_D) };
            let (res, s) = timed(|| fillet_edges(k, &mut b, &[e], r, xsect, 1, 1.0));
            res.ctx("fillet_edges")?;
            let mut out = built(b, s, none);
            if id == "7a" {
                out.notes.push("chamfer = fillet_edges with xsect_type 0 (linear section, equal setback)".into());
            }
            Ok(out)
        }
        "5b" => {
            let prof = stadium_profile(k)?;
            let refs: Vec<Option<&Curve>> = prof.iter().map(Some).collect();
            let (r, s) = timed(|| -> SpikeResult<Brep> {
                let mut b = create_linear_sweep(k, &refs, Vec3::new(0.0, 0.0, 1.0), STADIUM_H, true)
                    .ctx("create_linear_sweep")?;
                let edges = pick_edges(k, &b, |q| near(q.z, STADIUM_H))?;
                if edges.len() != 4 {
                    return failed(format!("adapter: expected 4 top edges, found {}", edges.len()));
                }
                fillet_edges(k, &mut b, &edges, FILLET_R, 1, 1, 1.0).ctx("fillet_edges")?;
                Ok(b)
            });
            let mut out = built(r?, s, none);
            out.notes.push("prism by create_linear_sweep; all 4 top edges passed to fillet_edges".into());
            Ok(out)
        }
        "5c" => {
            let mut b = bx(k)?;
            let (a, bb, c) = (BOX_A, BOX_B, BOX_C);
            let mut edges = Vec::new();
            edges.push(pick_one_edge(k, &b, "X edge", |q| near(q.y, bb) && near(q.z, c))?);
            edges.push(pick_one_edge(k, &b, "Y edge", |q| near(q.x, a) && near(q.z, c))?);
            edges.push(pick_one_edge(k, &b, "Z edge", |q| near(q.x, a) && near(q.y, bb))?);
            let (res, s) = timed(|| fillet_edges(k, &mut b, &edges, FILLET_R, 1, 1, 1.0));
            res.ctx("fillet_edges")?;
            Ok(built(b, s, none))
        }
        "5d" => {
            let mut b = bx(k)?;
            let e = top_front_edge(k, &b, BOX_C)?;
            // The law runs along the edge's parameter; report its direction.
            let (sv, _) = edge::start_end_vertices(&b, e).ctx("start_end_vertices")?;
            let start_x = match sv {
                Some(v) => vertex::point(&b, v).ctx("vertex::point")?.x,
                None => 0.0,
            };
            let (r0, r1) = if start_x < BOX_A / 2.0 { (VAR_R0, VAR_R1) } else { (VAR_R1, VAR_R0) };
            let (res, s) = timed(|| variable_radius_fillet(k, &mut b, &[e], r0, r1, 1, 1));
            res.ctx("variable_radius_fillet")?;
            let mut out = built(b, s, none);
            out.notes.push(format!("linear law; edge starts at x={start_x}, radii passed ({r0}, {r1})"));
            Ok(out)
        }
        "6" => {
            let (r, s) = timed(|| -> SpikeResult<Brep> {
                let mut b = hole_box(k)?;
                let e = pick_one_edge(k, &b, "hole top rim", |q| {
                    near(q.z, BOX_C) && near(((q.x - 50.0).powi(2) + (q.y - 30.0).powi(2)).sqrt(), HOLE_R)
                })?;
                fillet_edges(k, &mut b, &[e], HOLE_FILLET_R, 1, 1, 1.0).ctx("fillet_edges")?;
                Ok(b)
            });
            Ok(built(r?, s, none))
        }
        "7b" => not_exposed(
            "distance–angle chamfer: fillet_edges/chamfer_fillet take one distance (equal setback); \
             no asymmetric or angle chamfer in the public API",
        ),
        "8a" => {
            let b = bx(k)?;
            let top = pick_face(k, &b, "top face", |q| near(q.z, BOX_C))?;
            let (r, s) = timed(|| shell::shell(k, b, &[top], -SHELL_T));
            let mut out = built(r?, s, none);
            out.notes.push("api_shell_brep_full with a hand-built OffsetNotifications (difftest template)".into());
            Ok(out)
        }
        "8b" => {
            let (r, s) = timed(|| -> SpikeResult<Brep> {
                let mut b = create_cylinder(k, p(0.0, 0.0, 0.0), CYL8_R, CYL8_H).ctx("create_cylinder")?;
                let rim = pick_one_edge(k, &b, "top rim", |q| near(q.z, CYL8_H))?;
                fillet_edges(k, &mut b, &[rim], CYL8_FILLET, 1, 1, 1.0).ctx("fillet_edges")?;
                let bottom = pick_face(k, &b, "bottom face", |q| near(q.z, 0.0))?;
                shell::shell(k, b, &[bottom], -SHELL_T)
            });
            let mut out = built(r?, s, none);
            out.notes.push("api_shell_brep_full with a hand-built OffsetNotifications (difftest template)".into());
            Ok(out)
        }
        "9a" | "9b" => not_exposed(
            "push/pull: no public face offset/move (only low-level delete_face / replace_surface)",
        ),
        "10a" => {
            let (r, s) = timed(|| -> SpikeResult<_> {
                let helix = Curve::from(
                    create_helix(&HelixParams {
                        origin: p(0.0, 0.0, 0.0),
                        height: HELIX_PITCH * HELIX_TURNS,
                        radius_start: HELIX_R,
                        radius_end: HELIX_R,
                        num_turns: HELIX_TURNS,
                        right_handed: true,
                        tolerance: 1e-4,
                    })
                    .ctx("create_helix")?,
                );
                // Circle in XY about (HELIX_R, 0, 0), turned into the XZ (meridian) plane.
                let mut c = Curve::from(create_circle(p(HELIX_R, 0.0, 0.0), HELIX_PROFILE_R).ctx("create_circle")?);
                rotate_curve(k, &mut c, Vec3::new(1.0, 0.0, 0.0), 90.0)?;
                // The profile already sits at the helix start, but with the default flags the
                // section drifts off the meridian plane (end at z≈30.0, not 32; volume −6.3 %).
                // `move_profile_center_to_path` gives the analytic volume.
                let options = SweepOptions { cap_ends: true, move_profile_center_to_path: true, ..SweepOptions::default() };
                create_curve_sweep(k, &[Some(&c)], Some(&helix), None, None, Some(&options))
                    .ctx("create_curve_sweep")
            });
            let sw = r?;
            let h = sweep_history(sw.start_faces.len(), sw.side_faces.len(), sw.end_faces.len());
            let mut out = built(sw.brep, s, h);
            out.notes.push("create_helix (B-spline, tol 1e-4) + create_curve_sweep with move_profile_center_to_path (defaults: −6.3 % volume)".into());
            Ok(out)
        }
        "10b" | "10c" => {
            let (r, s) = timed(|| -> SpikeResult<_> {
                let mut curves: Vec<Curve> = Vec::new();
                let first = if id == "10b" {
                    curves.push(Curve::from(create_circle(p(0.0, 0.0, 0.0), LOFT_R0).ctx("create_circle")?));
                    1
                } else {
                    let (hx, hy) = (LOFT_RECT[0] / 2.0, LOFT_RECT[1] / 2.0);
                    let c = [p(hx, -hy, 0.0), p(hx, hy, 0.0), p(-hx, hy, 0.0), p(-hx, -hy, 0.0)];
                    for i in 0..4 {
                        curves.push(Curve::from(create_line_segment(c[i], c[(i + 1) % 4]).ctx("line")?));
                    }
                    4
                };
                curves.push(Curve::from(create_circle(p(0.0, 0.0, LOFT_H), LOFT_R1).ctx("create_circle")?));
                let refs: Vec<Option<&Curve>> = curves.iter().map(Some).collect();
                create_skin_primitive(k, &[first, 1], &refs, 1, true).ctx("create_skin_primitive")
            });
            let sk = r?;
            let h = sweep_history(sk.start_faces.len(), sk.side_faces.len(), sk.end_faces.len());
            let mut out = built(sk.brep, s, h);
            out.notes.push("create_skin_primitive, degree 1 (ruled), capped".into());
            Ok(out)
        }
        "11a" | "11b" => {
            let (r, s) = timed(|| -> SpikeResult<Brep> {
                // Profiles drawn in XY, turned into XZ by +90° about X (y → z).
                let mut curves = if id == "11a" {
                    let rr = REV_SPHERE_R;
                    vec![
                        Curve::from(create_arc(p(0.0, 0.0, 0.0), rr, -90.0, 90.0).ctx("arc")?),
                        Curve::from(create_line_segment(p(0.0, rr, 0.0), p(0.0, -rr, 0.0)).ctx("line")?),
                    ]
                } else {
                    let (rr, h) = (REV_CONE_R, REV_CONE_H);
                    vec![
                        Curve::from(create_line_segment(p(0.0, 0.0, 0.0), p(rr, 0.0, 0.0)).ctx("line")?),
                        Curve::from(create_line_segment(p(rr, 0.0, 0.0), p(0.0, h, 0.0)).ctx("line")?),
                        Curve::from(create_line_segment(p(0.0, h, 0.0), p(0.0, 0.0, 0.0)).ctx("line")?),
                    ]
                };
                for c in &mut curves {
                    rotate_curve(k, c, Vec3::new(1.0, 0.0, 0.0), 90.0)?;
                }
                let refs: Vec<Option<&Curve>> = curves.iter().map(Some).collect();
                create_rotational_sweep(k, &refs, p(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 360.0, true)
                    .ctx("create_rotational_sweep")
            });
            Ok(built(r?, s, none))
        }
        "13a" | "13b" => {
            let (r, s) = timed(|| create_sphere(k, p(0.0, 0.0, 0.0), TESS_SPHERE_R).ctx("create_sphere"));
            Ok(built(r?, s, none))
        }
        _ => not_exposed(&format!("unknown case {id}")),
    }
}

fn attrs_volume(k: &Kernel, b: &mut Brep) -> solidcore::Result<f64> {
    let store = AttributeStore::default();
    let key_of = |_| ObjectKey(0);
    let attrs = PropertyAttributes { store: &store, key_of: &key_of };
    brep_volume(k, b, MASS_ACCURACY, &attrs)
}

/// Topology counts and the kernel's own checks; problems become notes, not errors.
fn topo(k: &mut Kernel, b: &Brep, notes: &mut Vec<String>) -> Option<Topo> {
    let faces = p1::faces(b, None).ok()?;
    let edges = p1::edges(b, None).ok()?;
    let verts = p1::vertices(b, None).ok()?;
    let mut loops = Some(0usize);
    for f in &faces {
        match face::p2::loops(b, *f, false, None) {
            Ok(l) => loops = loops.map(|n| n + l.len()),
            Err(e) => {
                notes.push(format!("face loops: {e}"));
                loops = None;
                break;
            }
        }
    }
    // Every face set has one shell per side; only the shells of finite regions bound
    // material, which is what Euler–Poincaré counts.
    let shells = match p1::shells(k, b, None) {
        Ok(s) => s
            .iter()
            .filter(|id| matches!(solidcore::topology::shell::shell_region(b, **id), Ok(Some(r)) if b.infinite_region() != Some(r)))
            .count(),
        Err(e) => {
            notes.push(format!("p1::shells: {e}"));
            0
        }
    };
    let closed = match p2::is_manifold_solid(b) {
        Ok(v) => Some(v),
        Err(e) => {
            notes.push(format!("is_manifold_solid: {e}"));
            None
        }
    };
    let valid = match integrity::check(b) {
        Ok(()) => Some(true),
        Err(e) => {
            notes.push(format!("integrity::check: {e}"));
            Some(false)
        }
    };
    Some(Topo {
        vertices: verts.len(),
        edges: edges.len(),
        faces: faces.len(),
        loops,
        shells,
        backend_closed: closed,
        backend_valid: valid,
        backend_check: Some("is_manifold_solid (closed) + topology::integrity::check (valid)".into()),
    })
}

/// Mass properties, topology and the fixed mesh/STEP answers for one body.
fn measure(k: &mut Kernel, b: &mut Brep, p: &mut Produced) {
    match attrs_volume(k, b) {
        Ok(v) => p.native_volume = Some(v),
        Err(e) => p.notes.push(format!("brep_volume: {e}")),
    }
    match brep_area(k, b, MASS_ACCURACY) {
        Ok(a) => p.native_area = Some(a),
        Err(e) => p.notes.push(format!("brep_area: {e}")),
    }
    let mut notes = Vec::new();
    p.topo = topo(k, b, &mut notes);
    p.notes.extend(notes);
}

impl Backend for Solidcore {
    fn name(&self) -> &'static str {
        "solidcore"
    }

    fn run(&self, case_id: &str, _tol: f64) -> SpikeResult<Produced> {
        let mut k = Kernel::default();
        let mut b = build_case(&mut k, case_id)?;
        let mut out = Produced {
            op_seconds: b.secs,
            history: Some(b.history),
            step_error: Some(NO_STEP.into()),
            notes: b.notes,
            ..Produced::default()
        };
        measure(&mut k, &mut b.brep, &mut out);
        out.notes.push(NO_MESH.into());
        Ok(out)
    }

    fn import_step(&self, text: &str, _tol: f64) -> SpikeResult<Produced> {
        let mut k = Kernel::default();
        let (r, s) = timed(|| step_brep::import(text.as_bytes(), &step_import::Limits::default()));
        let mut bodies = r.map_err(|e| SpikeError::Failed(format!("step_brep::import: {e}")))?;
        let mut out = Produced { op_seconds: s, ..Produced::default() };
        out.notes.push(format!("step_brep::import read {} bodies", bodies.len()));
        let (mut vol, mut area) = (Some(0.0), Some(0.0));
        let mut total = Topo::default();
        let mut any_topo = false;
        for b in &mut bodies {
            let mut one = Produced::default();
            measure(&mut k, b, &mut one);
            vol = vol.zip(one.native_volume).map(|(a, x)| a + x);
            area = area.zip(one.native_area).map(|(a, x)| a + x);
            if let Some(t) = one.topo {
                any_topo = true;
                total.vertices += t.vertices;
                total.edges += t.edges;
                total.faces += t.faces;
                total.loops = match (total.loops, t.loops) {
                    (None, l) if total.faces == t.faces => l,
                    (Some(a), Some(x)) => Some(a + x),
                    _ => None,
                };
                total.shells += t.shells;
                total.backend_closed = Some(total.backend_closed.unwrap_or(true) && t.backend_closed.unwrap_or(false));
                total.backend_valid = Some(total.backend_valid.unwrap_or(true) && t.backend_valid.unwrap_or(false));
                total.backend_check = t.backend_check;
            }
            out.notes.extend(one.notes);
        }
        if bodies.is_empty() {
            return failed("step_brep::import returned no bodies");
        }
        out.native_volume = vol;
        out.native_area = area;
        out.topo = any_topo.then_some(total);
        out.notes.push(NO_MESH.into());
        Ok(out)
    }

    fn hostile(&self, family: Hostile, params: &[f64]) -> SpikeResult<()> {
        let mut k = Kernel::default();
        let k = &mut k;
        let g = |i: usize| params.get(i).copied().unwrap_or(f64::NAN);
        let (a, b, c, r) = (g(0), g(1), g(2), g(3));
        match family {
            Hostile::BoxMinusCylinder => {
                let bx = create_box(k, p(0.0, 0.0, 0.0), a, b, c).ctx("create_box")?;
                let cyl = create_cylinder(k, p(a / 2.0, b / 2.0, -1.0), r, c + 2.0).ctx("create_cylinder")?;
                boolean_difference(k, bx, cyl).ctx("boolean_difference")?;
                Ok(())
            }
            Hostile::FilletEdge => {
                let mut bx = create_box(k, p(0.0, 0.0, 0.0), a, b, c).ctx("create_box")?;
                let e = top_front_edge(k, &bx, c)?;
                fillet_edges(k, &mut bx, &[e], r, 1, 1, 1.0).ctx("fillet_edges")
            }
            Hostile::PushPull => not_exposed("push/pull: no public face offset/move"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hostile families succeed on sane input, so their clean errors come from the inputs.
    #[test]
    fn hostile_families_accept_sane_input() {
        let b = Solidcore;
        assert!(b.hostile(Hostile::BoxMinusCylinder, &[100.0, 60.0, 40.0, 10.0]).is_ok());
        assert!(b.hostile(Hostile::FilletEdge, &[100.0, 60.0, 40.0, 5.0]).is_ok());
    }
}
