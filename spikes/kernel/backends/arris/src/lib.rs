//! Spike adapter for the arris kernel. See `spikes/kernel/CASES.md` for the
//! geometry of every case and `backends/arris/NOTES.md` for the API used.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use arris::check::{check, classify_point, Classification, Level};
use arris::geom::{FrameLaw, Path, PathSegment, Profile, ProfileLoop, ProfileSegment};
use arris::math::{Axis, Frame, Point2, Point3, Vec3};
use arris::ops::measure::mass_properties;
use arris::ops::{
    chamfer, cut, fillet, fuse, fuse_many, loft, offset_faces, primitive_box, primitive_cylinder,
    revolve, shell, sweep, LoftKind, ShellSide,
};
use arris::topo::{Body, Edge, EntityKind, Face, Model, Provenance};
use arris::Control;
use harness::corpus::*;
use harness::{failed, not_exposed, timed, Backend, Hostile, History, Produced, SpikeResult, Topo, TriMesh};

const NONE: &Control<'static> = &Control::NONE;

/// Geometric tolerance used to pick edges and faces by position (mm).
const PICK_TOL: f64 = 1e-6;

pub struct Arris;

/// One modelling step's output: the body and the provenance of the last
/// operation, with the operation chain for the history detail.
struct Built {
    body: Body,
    provenance: Provenance,
    ops: Vec<&'static str>,
    notes: Vec<String>,
}

fn op_err<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> harness::SpikeError + '_ {
    move |e| harness::SpikeError::Failed(format!("{what}: {e}"))
}

fn p3(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn bx(m: &mut Model, min: [f64; 3], max: [f64; 3]) -> SpikeResult<(Body, Provenance)> {
    primitive_box(m, min, max, NONE).map_err(op_err("primitive_box"))
}

fn cylinder(m: &mut Model, origin: Point3, dir: Vec3, r: f64, h: f64) -> SpikeResult<(Body, Provenance)> {
    let axis = Axis::new(origin, dir).map_err(op_err("axis"))?;
    primitive_cylinder(m, axis, r, h, NONE).map_err(op_err("primitive_cylinder"))
}

/// The meridian plane through `origin` (normal −Y): profile (u, v) = (x, z).
fn xz_plane(origin: Point3) -> SpikeResult<Frame> {
    Frame::new(origin, -Vec3::y(), Vec3::x()).map_err(op_err("frame"))
}

fn xy_plane(z: f64) -> SpikeResult<Frame> {
    Frame::new(p3(0.0, 0.0, z), Vec3::z(), Vec3::x()).map_err(op_err("frame"))
}

/// Sphere of radius `r` centred at `c`: a half disc in the meridian plane
/// revolved a full turn about the vertical axis through `c` (case 11a).
fn sphere(m: &mut Model, c: Point3, r: f64) -> SpikeResult<(Body, Provenance)> {
    let p = |u, v| Point2::new(u, v);
    let profile = Profile {
        plane: xz_plane(c)?,
        outer: ProfileLoop::Path {
            start: p(0.0, -r),
            segments: vec![ProfileSegment::ArcTo { to: p(0.0, r), via: p(r, 0.0) }, ProfileSegment::LineTo(p(0.0, -r))],
        },
        holes: Vec::new(),
    };
    revolve(m, &profile, Axis::z_at(c), TAU, NONE).map_err(op_err("revolve"))
}

/// Points sampled along an edge's curve (ends included).
fn edge_samples(m: &Model, e: Edge) -> Option<Vec<Point3>> {
    let entity = m.edge(e.id).ok()?;
    let (curve, range) = entity.curve()?;
    let curve = m.curve(curve).ok()?;
    let (lo, hi) = (range.lo(), range.hi());
    Some((0..=4).map(|i| curve.point(lo + (hi - lo) * f64::from(i) / 4.0)).collect())
}

/// Edges of `body` every sample of which satisfies `on`.
fn edges_where(m: &Model, body: Body, on: impl Fn(&Point3) -> bool) -> SpikeResult<Vec<Edge>> {
    let edges = m.edges(body).map_err(op_err("edges"))?;
    Ok(edges
        .into_iter()
        .filter(|&e| edge_samples(m, e).is_some_and(|pts| pts.iter().all(&on)))
        .collect())
}

fn one_edge(m: &Model, body: Body, what: &str, on: impl Fn(&Point3) -> bool) -> SpikeResult<Edge> {
    let found = edges_where(m, body, on)?;
    match found.as_slice() {
        [e] => Ok(*e),
        _ => failed(format!("adapter: expected one edge for {what}, found {}", found.len())),
    }
}

/// The face the kernel's point classifier puts `point` on.
fn face_at(m: &Model, body: Body, point: Point3) -> SpikeResult<Face> {
    let class = classify_point(m, body, point).map_err(op_err("classify_point"))?;
    let Classification::On(shape) = class else {
        return failed(format!("adapter: {point:?} is not on the boundary ({class:?})"));
    };
    if shape.kind() != EntityKind::Face {
        return failed(format!("adapter: {point:?} lies on {shape}, not a face"));
    }
    let faces = m.faces(body).map_err(op_err("faces"))?;
    match faces.into_iter().find(|f| f.shape().id == shape.id) {
        Some(f) => Ok(f),
        None => failed(format!("adapter: face {shape} not in body")),
    }
}

/// The top front edge of a box `a × b × c`: along X at y = 0, z = c.
fn top_front_edge(m: &Model, body: Body, c: f64) -> SpikeResult<Edge> {
    one_edge(m, body, "top front edge", |p| p.y.abs() < PICK_TOL && (p.z - c).abs() < PICK_TOL)
}

/// The four vertical edges of a box `a × b × c`.
fn vertical_edges(m: &Model, body: Body, a: f64, b: f64) -> SpikeResult<Vec<Edge>> {
    let near = |v: f64, t: f64| (v - t).abs() < PICK_TOL;
    let edges = edges_where(m, body, |p| (near(p.x, 0.0) || near(p.x, a)) && (near(p.y, 0.0) || near(p.y, b)))?;
    if edges.len() != 4 {
        return failed(format!("adapter: expected 4 vertical edges, found {}", edges.len()));
    }
    Ok(edges)
}

fn built(body: Body, provenance: Provenance, ops: Vec<&'static str>) -> Built {
    Built { body, provenance, ops, notes: Vec::new() }
}

/// Box − cylinder r at (a/2, b/2) from z0 to z1.
fn box_minus_cylinder(m: &mut Model, [a, b, c]: [f64; 3], r: f64, z0: f64, z1: f64) -> SpikeResult<Built> {
    let (block, _) = bx(m, [0.0; 3], [a, b, c])?;
    let (tool, _) = cylinder(m, p3(a / 2.0, b / 2.0, z0), Vec3::z(), r, z1 - z0)?;
    let (body, prov) = cut(m, block, tool, NONE).map_err(op_err("cut"))?;
    Ok(built(body, prov, vec!["cut"]))
}

fn box_fillet_vertical(m: &mut Model, [a, b, c]: [f64; 3], r: f64) -> SpikeResult<(Body, Provenance)> {
    let (block, _) = bx(m, [0.0; 3], [a, b, c])?;
    let edges = vertical_edges(m, block, a, b)?;
    fillet(m, block, &edges, r, NONE).map_err(op_err("fillet"))
}

fn push_pull(m: &mut Model, abc: [f64; 3], r: f64, d: f64) -> SpikeResult<Built> {
    let [a, b, c] = abc;
    let (rounded, _) = box_fillet_vertical(m, abc, r)?;
    let top = face_at(m, rounded, p3(a / 2.0, b / 2.0, c))?;
    let (body, prov) = offset_faces(m, rounded, &[top], d, NONE).map_err(op_err("offset_faces"))?;
    Ok(built(body, prov, vec!["fillet", "offset_faces"]))
}

fn circle(plane: Frame, radius: f64) -> Profile {
    Profile { plane, outer: ProfileLoop::Circle { center: Point2::origin(), radius }, holes: Vec::new() }
}

/// Builds the geometry of one corpus case.
fn build(m: &mut Model, case_id: &str) -> SpikeResult<Built> {
    let abc = [BOX_A, BOX_B, BOX_C];
    match case_id {
        "1a" | "13c" => box_minus_cylinder(m, abc, HOLE_R, -1.0, BOX_C + 1.0),
        "1b" => box_minus_cylinder(m, abc, HOLE_R, BOX_C - BLIND_DEPTH, BOX_C + 1.0),
        "2a" | "2b" => {
            let (first, _) = bx(m, [0.0; 3], abc)?;
            let (y0, y1) = if case_id == "2a" { (0.0, BOX_B) } else { (BOX_B, 2.0 * BOX_B) };
            let (second, _) = bx(m, [BOX_A, y0, 0.0], [2.0 * BOX_A, y1, BOX_C])?;
            let (body, prov) = fuse(m, first, second, NONE).map_err(op_err("fuse"))?;
            Ok(built(body, prov, vec!["fuse"]))
        }
        "3a" => {
            let (block, _) = bx(m, [0.0; 3], abc)?;
            let (rod, _) = cylinder(m, p3(0.0, BOX_B / 2.0, BOX_C + HOLE_R), Vec3::x(), HOLE_R, BOX_A)?;
            let (body, prov) = fuse(m, block, rod, NONE).map_err(op_err("fuse"))?;
            Ok(built(body, prov, vec!["fuse"]))
        }
        "3b" => {
            let (block, _) = bx(m, [0.0; 3], abc)?;
            let (ball, _) = sphere(m, p3(BOX_A / 2.0, BOX_B / 2.0, BOX_C + HOLE_R), HOLE_R)?;
            let (body, prov) = fuse(m, block, ball, NONE).map_err(op_err("fuse"))?;
            let mut b = built(body, prov, vec!["fuse"]);
            b.notes.push("sphere = revolve of a half disc (no sphere primitive)".into());
            Ok(b)
        }
        "4" => {
            let (plate, _) = bx(m, [0.0; 3], PLATE)?;
            let mut bodies = vec![plate];
            for [x, y] in pin_centres() {
                let (pin, _) = cylinder(m, p3(x, y, PIN_Z0), Vec3::z(), PIN_R, PIN_Z1 - PIN_Z0)?;
                bodies.push(pin);
            }
            let (body, prov) = fuse_many(m, &bodies, NONE).map_err(op_err("fuse_many"))?;
            let mut b = built(body, prov, vec!["fuse_many"]);
            b.notes.push("n-ary union: ops::fuse_many over plate + 100 pins".into());
            Ok(b)
        }
        "5a" => {
            let (block, _) = bx(m, [0.0; 3], abc)?;
            let edge = top_front_edge(m, block, BOX_C)?;
            let (body, prov) = fillet(m, block, &[edge], FILLET_R, NONE).map_err(op_err("fillet"))?;
            Ok(built(body, prov, vec!["fillet"]))
        }
        "5b" => {
            let p = |u, v| Point2::new(u, v);
            let (r, ls) = (STADIUM_R, STADIUM_LS);
            let profile = Profile {
                plane: xy_plane(0.0)?,
                outer: ProfileLoop::Path {
                    start: p(0.0, -r),
                    segments: vec![
                        ProfileSegment::LineTo(p(ls, -r)),
                        ProfileSegment::ArcTo { to: p(ls, r), via: p(ls + r, 0.0) },
                        ProfileSegment::LineTo(p(0.0, r)),
                        ProfileSegment::ArcTo { to: p(0.0, -r), via: p(-r, 0.0) },
                    ],
                },
                holes: Vec::new(),
            };
            let (prism, _) = arris::ops::extrude(m, &profile, Vec3::z(), STADIUM_H, NONE).map_err(op_err("extrude"))?;
            // One straight top edge (y = −20): the kernel follows the tangent chain.
            let edge = one_edge(m, prism, "stadium top straight edge", |q| {
                (q.z - STADIUM_H).abs() < PICK_TOL && (q.y + r).abs() < PICK_TOL
            })?;
            let (body, prov) = fillet(m, prism, &[edge], FILLET_R, NONE).map_err(op_err("fillet"))?;
            let mut b = built(body, prov, vec!["extrude", "fillet"]);
            b.notes.push("one top edge selected; fillet follows the tangent chain".into());
            Ok(b)
        }
        "5c" => {
            let (block, _) = bx(m, [0.0; 3], abc)?;
            let near = |v: f64, t: f64| (v - t).abs() < PICK_TOL;
            let (a, b, c) = (BOX_A, BOX_B, BOX_C);
            let edges = edges_where(m, block, |q| {
                (near(q.y, b) && near(q.z, c)) || (near(q.x, a) && near(q.z, c)) || (near(q.x, a) && near(q.y, b))
            })?;
            if edges.len() != 3 {
                return failed(format!("adapter: expected 3 corner edges, found {}", edges.len()));
            }
            let (body, prov) = fillet(m, block, &edges, FILLET_R, NONE).map_err(op_err("fillet"))?;
            Ok(built(body, prov, vec!["fillet"]))
        }
        "5d" => not_exposed("variable-radius fillet (ops::fillet takes one constant radius)"),
        "6" => {
            let holed = box_minus_cylinder(m, abc, HOLE_R, -1.0, BOX_C + 1.0)?;
            let (cx, cy) = (BOX_A / 2.0, BOX_B / 2.0);
            let rim = one_edge(m, holed.body, "hole top rim", |q| {
                (q.z - BOX_C).abs() < PICK_TOL && (((q.x - cx).powi(2) + (q.y - cy).powi(2)).sqrt() - HOLE_R).abs() < PICK_TOL
            })?;
            let (body, prov) = fillet(m, holed.body, &[rim], HOLE_FILLET_R, NONE).map_err(op_err("fillet"))?;
            Ok(built(body, prov, vec!["cut", "fillet"]))
        }
        "7a" => {
            let (block, _) = bx(m, [0.0; 3], abc)?;
            let edge = top_front_edge(m, block, BOX_C)?;
            let (body, prov) = chamfer(m, block, &[edge], CHAMFER_D, NONE).map_err(op_err("chamfer"))?;
            Ok(built(body, prov, vec!["chamfer"]))
        }
        "7b" => not_exposed("distance–angle chamfer (ops::chamfer is equal-distance only)"),
        "8a" => {
            let (block, _) = bx(m, [0.0; 3], abc)?;
            let top = face_at(m, block, p3(BOX_A / 2.0, BOX_B / 2.0, BOX_C))?;
            let (body, prov) = shell(m, block, &[top], SHELL_T, ShellSide::Inward, NONE).map_err(op_err("shell"))?;
            Ok(built(body, prov, vec!["shell"]))
        }
        "8b" => {
            let (cyl, _) = cylinder(m, Point3::origin(), Vec3::z(), CYL8_R, CYL8_H)?;
            let rim = one_edge(m, cyl, "cylinder top rim", |q| {
                (q.z - CYL8_H).abs() < PICK_TOL && ((q.x * q.x + q.y * q.y).sqrt() - CYL8_R).abs() < PICK_TOL
            })?;
            let (rounded, _) = fillet(m, cyl, &[rim], CYL8_FILLET, NONE).map_err(op_err("fillet"))?;
            let bottom = face_at(m, rounded, p3(CYL8_R / 2.0, 0.0, 0.0))?;
            let (body, prov) =
                shell(m, rounded, &[bottom], SHELL_T, ShellSide::Inward, NONE).map_err(op_err("shell"))?;
            Ok(built(body, prov, vec!["fillet", "shell"]))
        }
        "9a" => push_pull(m, abc, FILLET_R, PUSH_D),
        "9b" => push_pull(m, abc, FILLET_R, -PUSH_D),
        "10a" => {
            let start = p3(HELIX_R, 0.0, 0.0);
            let profile = circle(xz_plane(start)?, HELIX_PROFILE_R);
            let path = Path {
                start,
                segments: vec![PathSegment::Helix {
                    axis: Axis::z_at(Point3::origin()),
                    pitch: HELIX_PITCH,
                    angle: TAU * HELIX_TURNS,
                }],
            };
            let law = FrameLaw::FixedBinormal(Vec3::z_axis());
            let (body, prov) = sweep(m, &profile, &path, law, NONE).map_err(op_err("sweep"))?;
            let mut b = built(body, prov, vec!["sweep"]);
            b.notes.push("PathSegment::Helix + FrameLaw::FixedBinormal(Z); side face is a fitted NURBS".into());
            Ok(b)
        }
        "10b" => {
            let sections = [circle(xy_plane(0.0)?, LOFT_R0), circle(xy_plane(LOFT_H)?, LOFT_R1)];
            let (body, prov) = loft(m, &sections, LoftKind::Ruled, false, NONE).map_err(op_err("loft"))?;
            Ok(built(body, prov, vec!["loft"]))
        }
        "10c" => {
            let [w, h] = LOFT_RECT;
            let p = |u, v| Point2::new(u, v);
            let rect = Profile {
                plane: xy_plane(0.0)?,
                outer: ProfileLoop::Path {
                    start: p(-w / 2.0, -h / 2.0),
                    segments: vec![
                        ProfileSegment::LineTo(p(w / 2.0, -h / 2.0)),
                        ProfileSegment::LineTo(p(w / 2.0, h / 2.0)),
                        ProfileSegment::LineTo(p(-w / 2.0, h / 2.0)),
                        ProfileSegment::LineTo(p(-w / 2.0, -h / 2.0)),
                    ],
                },
                holes: Vec::new(),
            };
            let sections = [rect, circle(xy_plane(LOFT_H)?, LOFT_R1)];
            let (body, prov) = loft(m, &sections, LoftKind::Ruled, false, NONE).map_err(op_err("loft"))?;
            Ok(built(body, prov, vec!["loft"]))
        }
        "11a" | "13a" | "13b" => {
            let r = if case_id == "11a" { REV_SPHERE_R } else { TESS_SPHERE_R };
            let (body, prov) = sphere(m, Point3::origin(), r)?;
            let mut b = built(body, prov, vec!["revolve"]);
            b.notes.push("sphere = revolve of a half disc (no sphere primitive)".into());
            Ok(b)
        }
        "11b" => {
            let p = |u, v| Point2::new(u, v);
            let profile = Profile {
                plane: xz_plane(Point3::origin())?,
                outer: ProfileLoop::Path {
                    start: p(0.0, 0.0),
                    segments: vec![
                        ProfileSegment::LineTo(p(REV_CONE_R, 0.0)),
                        ProfileSegment::LineTo(p(0.0, REV_CONE_H)),
                        ProfileSegment::LineTo(p(0.0, 0.0)),
                    ],
                },
                holes: Vec::new(),
            };
            let (body, prov) = revolve(m, &profile, Axis::z_at(Point3::origin()), TAU, NONE).map_err(op_err("revolve"))?;
            Ok(built(body, prov, vec!["revolve"]))
        }
        other => failed(format!("unknown case {other}")),
    }
}

/// History of the last operation as the kernel reports it.
fn history(p: &Provenance, ops: &[&'static str]) -> History {
    let mut generated = BTreeSet::new();
    let mut modified = BTreeSet::new();
    for origin in p.origins_recorded() {
        generated.extend(p.generated_from(origin).iter().copied());
        modified.extend(p.modified_from(origin).iter().copied());
    }
    let deleted: BTreeSet<_> = p.deleted().collect();
    let faces = |s: &BTreeSet<arris::topo::Shape>| s.iter().filter(|x| x.kind() == EntityKind::Face).count();
    let detail = format!(
        "ops {}; last op faces: generated {}, modified {}, deleted {} (counts are all entities)",
        ops.join(" → "),
        faces(&generated),
        faces(&modified),
        faces(&deleted)
    );
    History::Native { generated: generated.len(), modified: modified.len(), deleted: deleted.len(), detail }
}

fn topo(m: &Model, body: Body) -> Topo {
    let report = check(m, body, Level::Full);
    let valid = report.is_ok();
    let shells = m.shells(body).map(|s| s.len()).unwrap_or(0);
    match report.euler() {
        Some(e) => Topo {
            vertices: e.vertices,
            edges: e.edges,
            faces: e.faces,
            loops: Some(e.loops),
            shells: e.shells,
            backend_closed: Some(valid),
            backend_valid: Some(valid),
            backend_check: Some("arris::check::check(Level::Full)".into()),
        },
        None => Topo {
            vertices: m.vertices(body).map(|v| v.len()).unwrap_or(0),
            edges: m.edges(body).map(|v| v.len()).unwrap_or(0),
            faces: m.faces(body).map(|v| v.len()).unwrap_or(0),
            loops: None,
            shells,
            backend_closed: Some(valid),
            backend_valid: Some(valid),
            backend_check: Some("arris::check::check(Level::Full), no Euler line".into()),
        },
    }
}

fn mesh(m: &Model, body: Body, tol: f64) -> Result<TriMesh, String> {
    let t = arris::mesh::tessellate(m, body, tol, NONE).map_err(|e| format!("tessellate: {e}"))?;
    Ok(TriMesh { positions: t.positions().to_vec(), triangles: t.triangles().to_vec() })
}

/// Measures a body: topology, mass properties and mesh (not timed).
fn measure_into(out: &mut Produced, m: &Model, body: Body, tol: f64) {
    out.topo = Some(topo(m, body));
    match mass_properties(m, body, NONE) {
        Ok(p) => {
            out.native_volume = Some(p.volume);
            out.native_area = Some(p.area);
        }
        Err(e) => out.notes.push(format!("mass_properties: {e}")),
    }
    match mesh(m, body, tol) {
        Ok(t) => out.mesh = Some(t),
        Err(e) => out.notes.push(e),
    }
}

/// Appends `src` to `dst`, offsetting its indices.
fn append_mesh(dst: &mut TriMesh, src: TriMesh) {
    let base = u32::try_from(dst.positions.len()).unwrap_or(u32::MAX);
    dst.positions.extend(src.positions);
    dst.triangles
        .extend(src.triangles.into_iter().map(|[a, b, c]| [a + base, b + base, c + base]));
}

impl Backend for Arris {
    fn name(&self) -> &'static str {
        "arris"
    }

    fn run(&self, case_id: &str, tol: f64) -> SpikeResult<Produced> {
        let mut m = Model::default();
        let (result, op_seconds) = timed(|| build(&mut m, case_id));
        let b = result?;
        let mut out = Produced { op_seconds, notes: b.notes, ..Produced::default() };
        out.history = Some(history(&b.provenance, &b.ops));
        measure_into(&mut out, &m, b.body, tol);
        match arris::io::step::write(&m, &[b.body]) {
            Ok(text) => out.step = Some(text),
            Err(e) => out.step_error = Some(e.to_string()),
        }
        Ok(out)
    }

    fn import_step(&self, text: &str, tol: f64) -> SpikeResult<Produced> {
        let mut m = Model::default();
        let (read, op_seconds) = timed(|| arris::io::step::read(&mut m, text, &Default::default(), NONE));
        let read = read.map_err(op_err("step::read"))?;
        let mut out = Produced { op_seconds, ..Produced::default() };
        let (mut ok, mut refused, mut sheets) = (0usize, Vec::new(), 0usize);
        let (mut volume, mut area) = (0.0, 0.0);
        let mut total = Topo { loops: Some(0), backend_closed: Some(true), backend_valid: Some(true), ..Topo::default() };
        let mut merged = TriMesh::default();
        for solid in &read.solids {
            let body = match &solid.result {
                Ok(r) => r.body,
                Err(e) => {
                    refused.push(e.to_string());
                    continue;
                }
            };
            ok += 1;
            if !r_is_solid(&m, body) {
                sheets += 1;
            }
            let mut one = Produced::default();
            measure_into(&mut one, &m, body, tol);
            if let Some(t) = one.topo {
                total.vertices += t.vertices;
                total.edges += t.edges;
                total.faces += t.faces;
                total.loops = total.loops.zip(t.loops).map(|(a, b)| a + b);
                total.shells += t.shells;
                total.backend_closed = total.backend_closed.zip(t.backend_closed).map(|(a, b)| a && b);
                total.backend_valid = total.backend_valid.zip(t.backend_valid).map(|(a, b)| a && b);
            }
            volume += one.native_volume.unwrap_or(0.0);
            area += one.native_area.unwrap_or(0.0);
            if let Some(t) = one.mesh {
                append_mesh(&mut merged, t);
            }
            out.notes.extend(one.notes);
        }
        total.backend_check = Some("arris::check::check(Level::Full), all solids".into());
        out.notes.push(format!(
            "{} solids in file: {ok} read ({sheets} sheets), {} refused{}",
            read.solids.len(),
            refused.len(),
            if refused.is_empty() { String::new() } else { format!(": {}", refused.join("; ")) }
        ));
        if ok == 0 {
            return failed(out.notes.join(" | "));
        }
        out.topo = Some(total);
        out.native_volume = Some(volume);
        out.native_area = Some(area);
        out.mesh = Some(merged);
        Ok(out)
    }

    fn hostile(&self, family: Hostile, params: &[f64]) -> SpikeResult<()> {
        let get = |i: usize| params.get(i).copied().unwrap_or(f64::NAN);
        let abc = [get(0), get(1), get(2)];
        let r = get(3);
        let mut m = Model::default();
        let b = match family {
            Hostile::BoxMinusCylinder => box_minus_cylinder(&mut m, abc, r, -1.0, abc[2] + 1.0)?,
            Hostile::FilletEdge => {
                let (block, _) = bx(&mut m, [0.0; 3], abc)?;
                let edge = top_front_edge(&m, block, abc[2])?;
                let (body, prov) = fillet(&mut m, block, &[edge], r, NONE).map_err(op_err("fillet"))?;
                built(body, prov, vec!["fillet"])
            }
            Hostile::PushPull => push_pull(&mut m, abc, r, get(4))?,
        };
        let report = check(&m, b.body, Level::Full);
        if !report.is_ok() {
            return failed(format!("result fails the checker: {:?}", report));
        }
        Ok(())
    }
}

fn r_is_solid(m: &Model, body: Body) -> bool {
    mass_properties(m, body, NONE).is_ok()
}
