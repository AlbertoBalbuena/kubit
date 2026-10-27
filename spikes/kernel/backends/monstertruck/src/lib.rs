//! Spike adapter for the monstertruck kernel. See `spikes/kernel/CASES.md` for the
//! geometry of every case and `backends/monstertruck/NOTES.md` for the API used.

use std::collections::HashSet;
use std::f64::consts::PI;

use harness::corpus::{self, *};
use harness::{failed, not_exposed, timed, Backend, Hostile, History, Produced, SpikeResult, Topo, TriMesh};
use monstertruck::meshing::prelude::{shell_to_polygon_strict, MeshableSurface, OptimizingFilter, PolylineableCurve, Topology};
use monstertruck::modeling::{
    builder, fillet_edges, primitive, BoundedCurve, BoundingBox, Curve, Edge, FilletOptions, FilletProfile,
    ParametricCurve, Point3, Shell, Solid, Surface, Vector3, Wire, TOLERANCE,
};
use monstertruck::step::load::Table;
use monstertruck::step::save::{CompleteStepDisplay, StepHeaderDescriptor, StepModel};
use monstertruck::topology::shell::ShellCondition;
use monstertruck::topology::{Shell as TShell, Solid as TSolid};

/// Tessellation tolerance handed to the boolean operators (`or`, `difference`).
/// The kernel's own tests use 0.05 at unit scale; our parts are in mm, so the
/// same value is a 0.05 mm chord for the intersection polylines.
const BOOL_TOL: f64 = 0.05;

type MSolid = Solid;
type MShell = Shell;

pub struct Monstertruck;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

// ---------------------------------------------------------------- primitives

fn cuboid(min: [f64; 3], max: [f64; 3]) -> MSolid {
    primitive::cuboid(BoundingBox::from_iter([p(min[0], min[1], min[2]), p(max[0], max[1], max[2])]))
}

fn base_box() -> MSolid {
    cuboid([0.0; 3], [BOX_A, BOX_B, BOX_C])
}

/// Cylinder of radius `r` along +Z through (cx, cy), z ∈ [z0, z1]: a closed
/// revolve of a vertex (2 arcs), a planar cap, and an extrusion.
fn cylinder_z(cx: f64, cy: f64, r: f64, z0: f64, z1: f64) -> SpikeResult<MSolid> {
    let v = builder::vertex(p(cx + r, cy, z0));
    let circle: Wire = builder::revolve(&v, p(cx, cy, z0), Vector3::unit_z(), builder::SweepAngle::Closed, 2);
    let disk = builder::try_attach_plane(vec![circle]).map_err(|e| harness::SpikeError::Failed(format!("attach_plane: {e}")))?;
    Ok(builder::extrude(&disk, Vector3::new(0.0, 0.0, z1 - z0)))
}

/// Cylinder of radius `r` along +X through (·, cy, cz), x ∈ [x0, x1].
fn cylinder_x(cy: f64, cz: f64, r: f64, x0: f64, x1: f64) -> SpikeResult<MSolid> {
    let v = builder::vertex(p(x0, cy + r, cz));
    let circle: Wire = builder::revolve(&v, p(x0, cy, cz), Vector3::unit_x(), builder::SweepAngle::Closed, 2);
    let disk = builder::try_attach_plane(vec![circle]).map_err(|e| harness::SpikeError::Failed(format!("attach_plane: {e}")))?;
    Ok(builder::extrude(&disk, Vector3::new(x1 - x0, 0.0, 0.0)))
}

/// Revolves a wire whose two end vertices lie on the Z axis by 360° and makes
/// a solid of it. Returns the solid and whether it had to be inverted.
fn revolve_on_axis(wire: &Wire) -> SpikeResult<(MSolid, bool)> {
    let shell: MShell = builder::revolve_wire(wire, p(0.0, 0.0, 0.0), Vector3::unit_z(), builder::SweepAngle::Closed, 4);
    let solid = Solid::try_new(vec![shell]).map_err(|e| harness::SpikeError::Failed(format!("Solid::try_new: {e}")))?;
    Ok(orient_outward(solid))
}

/// Sphere of radius `r` at the origin: revolve the meridian half circle
/// (0,−r) → (r,0) → (0,r) in the XZ plane about Z (case 11a construction).
fn sphere(r: f64) -> SpikeResult<(MSolid, bool)> {
    let v0 = builder::vertex(p(0.0, 0.0, -r));
    let v1 = builder::vertex(p(0.0, 0.0, r));
    let arc: Edge = builder::try_circle_arc(&v0, &v1, p(r, 0.0, 0.0))
        .map_err(|e| harness::SpikeError::Failed(format!("circle_arc: {e}")))?;
    revolve_on_axis(&Wire::from(vec![arc]))
}

/// If the mesh of `solid` has negative signed volume, the builder produced an
/// inward-oriented shell; flip it with the kernel's own `Solid::not`.
fn orient_outward(mut solid: MSolid) -> (MSolid, bool) {
    let vol = mesh_solid(&solid, 0.5).map(|(m, _)| signed_volume(&m)).unwrap_or(1.0);
    if vol < 0.0 {
        solid.not();
        (solid, true)
    } else {
        (solid, false)
    }
}

// ---------------------------------------------------------------- operations

fn bool_err(op: &str, e: impl std::fmt::Display) -> harness::SpikeError {
    harness::SpikeError::Failed(format!("{op}: {e}"))
}

fn union(a: &MSolid, b: &MSolid) -> SpikeResult<MSolid> {
    monstertruck::solid::or(a, b, BOOL_TOL).map_err(|e| bool_err("or", e))
}

fn subtract(a: &MSolid, b: &MSolid) -> SpikeResult<MSolid> {
    monstertruck::solid::difference(a, b, BOOL_TOL).map_err(|e| bool_err("difference", e))
}

fn near(a: Point3, b: Point3) -> bool {
    let d = a - b;
    (d.x * d.x + d.y * d.y + d.z * d.z).sqrt() < 1e-6 * (1.0 + a.x.abs().max(a.y.abs()).max(a.z.abs()))
}

/// First edge of the shell whose end points are `a` and `b` (either order).
fn find_edge(shell: &MShell, a: Point3, b: Point3) -> Option<Edge> {
    shell.edge_iter().find(|e| {
        let (f, k) = (e.front().point(), e.back().point());
        (near(f, a) && near(k, b)) || (near(f, b) && near(k, a))
    })
}

fn find_edges(shell: &MShell, pairs: &[(Point3, Point3)]) -> SpikeResult<Vec<Edge>> {
    pairs
        .iter()
        .map(|&(a, b)| find_edge(shell, a, b).ok_or_else(|| harness::SpikeError::Failed(format!("edge {a:?}–{b:?} not found"))))
        .collect()
}

/// Unique edges (by id) of a shell matching `pred`.
fn edges_where(shell: &MShell, pred: impl Fn(&Edge) -> bool) -> Vec<Edge> {
    let mut seen = HashSet::new();
    shell.edge_iter().filter(|e| seen.insert(e.id()) && pred(e)).collect()
}

fn edge_mid(e: &Edge) -> Point3 {
    let c = e.curve();
    let (t0, t1) = c.range_tuple();
    c.subs(0.5 * (t0 + t1))
}

/// Fillets `edges` of the single boundary shell of `solid` and rebuilds the solid.
fn fillet(solid: MSolid, select: impl FnOnce(&MShell) -> SpikeResult<Vec<Edge>>, opts: impl FnOnce(&[Edge]) -> FilletOptions) -> SpikeResult<MSolid> {
    let mut shells = solid.into_boundaries();
    if shells.len() != 1 {
        return failed(format!("expected one boundary shell, got {}", shells.len()));
    }
    let Some(mut shell) = shells.pop() else { return failed("no shell") };
    let edges = select(&shell)?;
    let opts = opts(&edges);
    fillet_edges(&mut shell, &edges, Some(&opts)).map_err(|e| harness::SpikeError::Failed(format!("fillet_edges: {e}")))?;
    Solid::try_new(vec![shell]).map_err(|e| harness::SpikeError::Failed(format!("fillet result is not a solid: {e}")))
}

fn top_front_edge() -> (Point3, Point3) {
    (p(0.0, 0.0, BOX_C), p(BOX_A, 0.0, BOX_C))
}

// ---------------------------------------------------------------- measurement

fn signed_volume(m: &TriMesh) -> f64 {
    m.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.positions.get(i as usize).copied().unwrap_or([0.0; 3]));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) + a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        })
        .sum()
}

/// Meshes every boundary shell with the strict tessellator (refuses instead of
/// silently dropping a face) and welds coincident vertices.
fn mesh_shells<'a, C, S>(shells: impl IntoIterator<Item = &'a TShell<Point3, C, S>>, tol: f64) -> Result<(TriMesh, bool), String>
where
    C: PolylineableCurve + 'a,
    S: MeshableSurface + 'a,
{
    let mut out = TriMesh::default();
    let mut closed = true;
    for shell in shells {
        let mut poly = shell_to_polygon_strict(shell, tol).map_err(|e| e.to_string())?;
        poly.put_together_same_attrs(TOLERANCE * 50.0).remove_degenerate_faces().remove_unused_attrs();
        closed &= poly.shell_condition() == ShellCondition::Closed;
        let base = u32::try_from(out.positions.len()).map_err(|e| e.to_string())?;
        out.positions.extend(poly.positions().iter().map(|q| [q.x, q.y, q.z]));
        for tri in poly.faces().triangle_iter() {
            let idx = |k: usize| u32::try_from(tri[k].pos).map(|i| i + base).map_err(|e| e.to_string());
            out.triangles.push([idx(0)?, idx(1)?, idx(2)?]);
        }
    }
    Ok((out, closed))
}

fn mesh_solid(solid: &MSolid, tol: f64) -> Result<(TriMesh, bool), String> {
    mesh_shells::<Curve, Surface>(solid.boundaries().iter(), tol)
}

fn topo_of<C, S>(shells: &[TShell<Point3, C, S>]) -> Topo
where
    C: Clone,
    S: Clone,
{
    let (mut v, mut e, mut f, mut loops) = (HashSet::new(), HashSet::new(), 0usize, 0usize);
    let mut closed = true;
    let mut valid = true;
    for shell in shells {
        for face in shell.face_iter() {
            f += 1;
            let bds = face.boundaries();
            loops += bds.len();
            for w in &bds {
                for edge in w.edge_iter() {
                    e.insert(edge.id());
                    v.insert(edge.front().id());
                    v.insert(edge.back().id());
                }
            }
        }
        closed &= shell.shell_condition() == ShellCondition::Closed;
        valid &= shell.check_solid_boundary().is_ok();
    }
    Topo {
        vertices: v.len(),
        edges: e.len(),
        faces: f,
        loops: Some(loops),
        shells: shells.len(),
        backend_closed: Some(closed),
        backend_valid: Some(valid),
        backend_check: Some("Shell::shell_condition == Closed; Shell::check_solid_boundary (non-empty, connected, closed, manifold)".into()),
    }
}

fn step_text(solid: &MSolid) -> String {
    let header = StepHeaderDescriptor {
        file_name: "kubit-spike.step".into(),
        time_stamp: "2026-01-01T00:00:00".into(),
        authors: vec!["kubit spike".into()],
        organization: vec!["kubit".into()],
        organization_system: "monstertruck 0.4.1".into(),
        authorization: String::new(),
    };
    let compressed = solid.compress();
    CompleteStepDisplay::new(StepModel::from(&compressed), header).to_string()
}

fn produce(solid: &MSolid, op_seconds: f64, tol: f64, mut notes: Vec<String>) -> Produced {
    let mut out = Produced { op_seconds, history: Some(History::None), ..Produced::default() };
    match mesh_solid(solid, tol) {
        Ok((m, closed)) => {
            if !closed {
                notes.push("tessellated mesh is not closed after put_together_same_attrs".into());
            }
            out.mesh = Some(m);
        }
        Err(e) => notes.push(format!("strict tessellation refused: {e}")),
    }
    out.topo = Some(topo_of::<Curve, Surface>(solid.boundaries()));
    if !solid.is_geometric_consistent() {
        notes.push("Solid::is_geometric_consistent() == false".into());
    }
    out.step = Some(step_text(solid));
    notes.push("no native mass properties: volume from the mesh".into());
    out.notes = notes;
    out
}

// ---------------------------------------------------------------- cases

fn hole_through() -> SpikeResult<MSolid> {
    let tool = cylinder_z(BOX_A / 2.0, BOX_B / 2.0, HOLE_R, -1.0, BOX_C + 1.0)?;
    subtract(&base_box(), &tool)
}

fn stadium() -> SpikeResult<MSolid> {
    let (r, l) = (STADIUM_R, STADIUM_LS);
    let v = builder::vertices([p(0.0, -r, 0.0), p(l, -r, 0.0), p(l, r, 0.0), p(0.0, r, 0.0)]);
    let arc = |a: usize, b: usize, through: Point3| {
        builder::try_circle_arc(&v[a], &v[b], through).map_err(|e| harness::SpikeError::Failed(format!("circle_arc: {e}")))
    };
    let wire: Wire = vec![
        builder::line(&v[0], &v[1]),
        arc(1, 2, p(l + r, 0.0, 0.0))?,
        builder::line(&v[2], &v[3]),
        arc(3, 0, p(-r, 0.0, 0.0))?,
    ]
    .into();
    let face = builder::try_attach_plane(vec![wire]).map_err(|e| harness::SpikeError::Failed(format!("attach_plane: {e}")))?;
    Ok(builder::extrude(&face, Vector3::new(0.0, 0.0, STADIUM_H)))
}

/// Closes a loft side shell between `bottom` and `top` with planar caps. The
/// side faces use the bottom edges forward and the top edges backward, so the
/// bottom cap takes the inverted bottom wire and the top cap the top wire.
fn capped_loft(bottom: &Wire, top: &Wire) -> SpikeResult<(MSolid, bool)> {
    let mut shell: MShell = builder::try_skin_wires(&[bottom.clone(), top.clone()])
        .map_err(|e| harness::SpikeError::Failed(format!("try_skin_wires: {e}")))?;
    let cap0 = builder::try_attach_plane(vec![bottom.inverse()]).map_err(|e| harness::SpikeError::Failed(format!("bottom cap: {e}")))?;
    let cap1 = builder::try_attach_plane(vec![top.clone()]).map_err(|e| harness::SpikeError::Failed(format!("top cap: {e}")))?;
    shell.push(cap0);
    shell.push(cap1);
    let solid = Solid::try_new(vec![shell]).map_err(|e| harness::SpikeError::Failed(format!("Solid::try_new: {e}")))?;
    Ok(orient_outward(solid))
}

fn circle_wire(r: f64, z: f64, division: usize) -> Wire {
    primitive::circle(p(r, 0.0, z), p(0.0, 0.0, z), Vector3::unit_z(), division)
}

/// Builds the case geometry; returns the solid and adapter notes.
fn build(case_id: &str) -> SpikeResult<(MSolid, Vec<String>)> {
    let mut notes = Vec::new();
    let solid = match case_id {
        "1a" | "13c" => hole_through()?,
        "1b" => {
            let tool = cylinder_z(BOX_A / 2.0, BOX_B / 2.0, HOLE_R, BOX_C - BLIND_DEPTH, BOX_C + 1.0)?;
            subtract(&base_box(), &tool)?
        }
        "2a" => union(&base_box(), &cuboid([BOX_A, 0.0, 0.0], [2.0 * BOX_A, BOX_B, BOX_C]))?,
        "2b" => union(&base_box(), &cuboid([BOX_A, BOX_B, 0.0], [2.0 * BOX_A, 2.0 * BOX_B, BOX_C]))?,
        "3a" => union(&base_box(), &cylinder_x(BOX_B / 2.0, BOX_C + HOLE_R, HOLE_R, 0.0, BOX_A)?)?,
        "3b" => {
            let (s, _) = sphere(HOLE_R)?;
            let s = builder::translated(&s, Vector3::new(BOX_A / 2.0, BOX_B / 2.0, BOX_C + HOLE_R));
            union(&base_box(), &s)?
        }
        "4" => {
            notes.push("no n-ary union: plate ∪ pin folded one by one with `or` (100 calls)".into());
            let mut acc = cuboid([0.0; 3], PLATE);
            for [x, y] in corpus::pin_centres() {
                acc = union(&acc, &cylinder_z(x, y, PIN_R, PIN_Z0, PIN_Z1)?)?;
            }
            acc
        }
        "5a" => fillet(
            base_box(),
            |s| {
                let (a, b) = top_front_edge();
                find_edges(s, &[(a, b)])
            },
            |_| FilletOptions::constant(FILLET_R),
        )?,
        "5b" => {
            notes.push("one edge (the straight y=−20 top edge) selected; tangent chain left to the kernel".into());
            fillet(
                stadium()?,
                |s| find_edges(s, &[(p(0.0, -STADIUM_R, STADIUM_H), p(STADIUM_LS, -STADIUM_R, STADIUM_H))]),
                |_| FilletOptions::constant(FILLET_R),
            )?
        }
        "5c" => {
            notes.push("no vertex-blend operation: the 3 corner edges passed together to one fillet_edges call".into());
            let c = p(BOX_A, BOX_B, BOX_C);
            fillet(
                base_box(),
                |s| {
                    find_edges(
                        s,
                        &[(p(0.0, BOX_B, BOX_C), c), (p(BOX_A, 0.0, BOX_C), c), (p(BOX_A, BOX_B, 0.0), c)],
                    )
                },
                |_| FilletOptions::constant(FILLET_R),
            )?
        }
        "5d" => fillet(
            base_box(),
            |s| {
                let (a, b) = top_front_edge();
                find_edges(s, &[(a, b)])
            },
            |edges| {
                // RadiusSpec::Variable takes the normalised edge parameter; flip
                // the law if the selected edge runs from x = 100 to x = 0.
                let reversed = edges.first().is_some_and(|e| e.front().point().x > e.back().point().x);
                FilletOptions::variable(move |t| {
                    let s = if reversed { 1.0 - t } else { t };
                    VAR_R0 + (VAR_R1 - VAR_R0) * s
                })
            },
        )?,
        "6" => {
            let holed = hole_through()?;
            fillet(
                holed,
                |s| {
                    let (cx, cy) = (BOX_A / 2.0, BOX_B / 2.0);
                    let on_rim = |q: Point3| (q.z - BOX_C).abs() < 1e-6 && (((q.x - cx).powi(2) + (q.y - cy).powi(2)).sqrt() - HOLE_R).abs() < 1e-3;
                    let edges = edges_where(s, |e| on_rim(e.front().point()) && on_rim(e.back().point()) && on_rim(edge_mid(e)));
                    if edges.is_empty() {
                        return failed("hole rim edges not found");
                    }
                    Ok(edges)
                },
                |_| FilletOptions::constant(HOLE_FILLET_R),
            )?
        }
        "7a" => {
            notes.push("FilletProfile::Chamfer with constant radius 5: equal setback (d–d symmetric only)".into());
            fillet(
                base_box(),
                |s| {
                    let (a, b) = top_front_edge();
                    find_edges(s, &[(a, b)])
                },
                |_| FilletOptions::constant(CHAMFER_D).with_profile(FilletProfile::Chamfer),
            )?
        }
        "7b" => return not_exposed("distance–angle chamfer: FilletProfile::Chamfer is equal-setback only"),
        "8a" | "8b" => return not_exposed("shell/hollow solid: no B-rep shell operation (surface offsets only)"),
        "9a" | "9b" => return not_exposed("push/pull: no face offset/move operation"),
        "10a" => return not_exposed("sweep along a helix: no B-rep sweep along a curve (surface-only sweep_rail)"),
        "10b" => {
            let (s, flipped) = capped_loft(&circle_wire(LOFT_R0, 0.0, 2), &circle_wire(LOFT_R1, LOFT_H, 2))?;
            notes.push("try_skin_wires (ruled homotopy) + try_attach_plane caps".into());
            if flipped {
                notes.push("loft came out inward-oriented; flipped with Solid::not".into());
            }
            s
        }
        "10c" => {
            let [w, h] = LOFT_RECT;
            let (hw, hh) = (w / 2.0, h / 2.0);
            let corners = [p(hw, -hh, 0.0), p(hw, hh, 0.0), p(-hw, hh, 0.0), p(-hw, -hh, 0.0)];
            let rv = builder::vertices(corners);
            let rect: Wire = (0..4).map(|i| builder::line(&rv[i], &rv[(i + 1) % 4])).collect();
            // Circle split at the rectangle's corner directions so edge i maps to arc i.
            let ang: Vec<f64> = corners.iter().map(|c| c.y.atan2(c.x)).collect();
            let at = |a: f64| p(LOFT_R1 * a.cos(), LOFT_R1 * a.sin(), LOFT_H);
            let cv = builder::vertices(ang.iter().map(|&a| at(a)));
            let mut arcs = Vec::new();
            for i in 0..4 {
                let (a0, mut a1) = (ang[i], ang[(i + 1) % 4]);
                if a1 < a0 {
                    a1 += 2.0 * PI;
                }
                arcs.push(
                    builder::try_circle_arc(&cv[i], &cv[(i + 1) % 4], at(0.5 * (a0 + a1)))
                        .map_err(|e| harness::SpikeError::Failed(format!("circle_arc: {e}")))?,
                );
            }
            let circle: Wire = arcs.into();
            let (s, flipped) = capped_loft(&rect, &circle)?;
            notes.push("ruled loft: rectangle edges paired with 4 arcs split at the corner directions".into());
            if flipped {
                notes.push("loft came out inward-oriented; flipped with Solid::not".into());
            }
            s
        }
        "11a" | "13a" | "13b" => {
            let (s, flipped) = sphere(REV_SPHERE_R)?;
            notes.push("revolve_wire of a half-circle arc with both ends on the axis (division 4)".into());
            if flipped {
                notes.push("revolve came out inward-oriented; flipped with Solid::not".into());
            }
            s
        }
        "11b" => {
            let v = builder::vertices([p(0.0, 0.0, REV_CONE_H), p(REV_CONE_R, 0.0, 0.0), p(0.0, 0.0, 0.0)]);
            let wire: Wire = vec![builder::line(&v[0], &v[1]), builder::line(&v[1], &v[2])].into();
            let (s, flipped) = revolve_on_axis(&wire)?;
            notes.push("revolve_wire of apex→rim→base-centre polyline (axis segment omitted, ends on axis)".into());
            if flipped {
                notes.push("revolve came out inward-oriented; flipped with Solid::not".into());
            }
            s
        }
        other => return failed(format!("unknown case {other}")),
    };
    Ok((solid, notes))
}

impl Backend for Monstertruck {
    fn name(&self) -> &'static str {
        "monstertruck"
    }

    fn run(&self, case_id: &str, tol: f64) -> SpikeResult<Produced> {
        let (built, secs) = timed(|| build(case_id));
        let (solid, mut notes) = built?;
        if matches!(case_id, "1a" | "1b" | "2a" | "2b" | "3a" | "3b" | "4" | "6" | "13c") {
            notes.push(format!("boolean tessellation tol = {BOOL_TOL}"));
        }
        Ok(produce(&solid, secs, tol, notes))
    }

    fn import_step(&self, text: &str, tol: f64) -> SpikeResult<Produced> {
        let table = Table::from_step(text).map_err(|e| harness::SpikeError::Failed(format!("Table::from_step: {e}")))?;
        let mut keys: Vec<_> = table.manifold_solid_brep.keys().copied().collect();
        keys.sort_unstable();
        let mut shells = Vec::new();
        let mut refused = Vec::new();
        for k in &keys {
            let Some(holder) = table.manifold_solid_brep.get(k) else { continue };
            match table.to_compressed_solid(holder) {
                Ok(cs) => match TSolid::extract(cs) {
                    Ok(s) => shells.extend(s.into_boundaries()),
                    Err(e) => refused.push(format!("#{k}: extract: {e}")),
                },
                Err(e) => refused.push(format!("#{k}: {e}")),
            }
        }
        if shells.is_empty() {
            return failed(format!("no solid read ({} manifold_solid_brep): {}", keys.len(), refused.join("; ")));
        }
        let mut notes = vec![format!("{} solid(s) read, {} refused", keys.len() - refused.len(), refused.len())];
        notes.extend(refused);
        let mut out = Produced { history: Some(History::None), ..Produced::default() };
        match mesh_shells(shells.iter(), tol) {
            Ok((m, closed)) => {
                if !closed {
                    notes.push("tessellated mesh is not closed".into());
                }
                out.mesh = Some(m);
            }
            Err(e) => notes.push(format!("strict tessellation refused: {e}")),
        }
        out.topo = Some(topo_of(&shells));
        out.notes = notes;
        Ok(out)
    }

    fn hostile(&self, family: Hostile, params: &[f64]) -> SpikeResult<()> {
        let g = |i: usize| params.get(i).copied().unwrap_or(f64::NAN);
        let (a, b, c, r) = (g(0), g(1), g(2), g(3));
        match family {
            Hostile::BoxMinusCylinder => {
                let tool = cylinder_z(a / 2.0, b / 2.0, r, -1.0, c + 1.0)?;
                subtract(&cuboid([0.0; 3], [a, b, c]), &tool).map(|_| ())
            }
            Hostile::FilletEdge => {
                fillet(
                    cuboid([0.0; 3], [a, b, c]),
                    |s| find_edges(s, &[(p(0.0, 0.0, c), p(a, 0.0, c))]),
                    |_| FilletOptions::constant(r),
                )
                .map(|_| ())
            }
            Hostile::PushPull => not_exposed("push/pull: no face offset/move operation"),
        }
    }
}
