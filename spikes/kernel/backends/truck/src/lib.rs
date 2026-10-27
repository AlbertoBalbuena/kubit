//! Spike adapter for the truck kernel. See `spikes/kernel/CASES.md` for the
//! geometry of every case and `backends/truck/NOTES.md` for the API used.

use std::collections::HashSet;
use std::f64::consts::PI;

use harness::corpus::{
    pin_centres, BLIND_DEPTH, BOX_A, BOX_B, BOX_C, HOLE_R, LOFT_H, LOFT_R0, LOFT_R1, LOFT_RECT, PIN_R, PIN_Z0,
    PIN_Z1, PLATE, REV_CONE_H, REV_CONE_R, REV_SPHERE_R, TESS_SPHERE_R,
};
use harness::{failed, not_exposed, timed, Backend, History, Hostile, Produced, SpikeResult, Topo, TriMesh};
use truck_meshalgo::prelude::{MeshableShape, MeshedShape, OptimizingFilter, PolygonMesh, RobustMeshableShape};
use truck_modeling::{builder, Curve, Point3, Rad, Shell, ShellCondition, Solid, Surface, Vector3, Wire};
use truck_stepio::out::{CompleteStepDisplay, StepHeaderDescriptor, StepModel};
use truck_stepio::r#in::Table;

/// Tolerance passed to `truck_shapeops::{and, or}`. The crate's own tests use 0.05
/// on unit-sized shapes; it drives the internal triangulation used to find
/// intersection curves.
/// Same-attribute welding tolerance for `put_together_same_attrs` (≫ truck's TOLERANCE = 1e-6).
const WELD_TOL: f64 = 1e-5;
const BOOL_TOL: f64 = 0.05;
/// More than 2π: `rsweep` / `cone` treat |angle| ≥ 2π as a full, closed revolution.
const FULL_TURN: Rad<f64> = Rad(7.0);

pub struct Truck;

/// Axis-aligned box from a min corner and its extents (vertex → edge → face → solid).
fn cuboid(origin: [f64; 3], size: [f64; 3]) -> Solid {
    let v = builder::vertex(Point3::new(origin[0], origin[1], origin[2]));
    let e = builder::tsweep(&v, Vector3::new(size[0], 0.0, 0.0));
    let f = builder::tsweep(&e, Vector3::new(0.0, size[1], 0.0));
    builder::tsweep(&f, Vector3::new(0.0, 0.0, size[2]))
}

/// Cylinder of radius `r` about a vertical axis through (cx, cy), z ∈ [z0, z1].
fn z_cylinder(cx: f64, cy: f64, r: f64, z0: f64, z1: f64) -> SpikeResult<Solid> {
    let v = builder::vertex(Point3::new(cx + r, cy, z0));
    let circle: Wire = builder::rsweep(&v, Point3::new(cx, cy, z0), Vector3::unit_z(), FULL_TURN);
    let disk = builder::try_attach_plane(&[circle]).or_else(|e| failed(format!("try_attach_plane: {e}")))?;
    Ok(builder::tsweep(&disk, Vector3::new(0.0, 0.0, z1 - z0)))
}

/// Sphere of radius `r` centred at `c`: half circle in the XZ plane revolved about Z with `builder::cone`.
fn sphere(c: [f64; 3], r: f64) -> SpikeResult<Solid> {
    let top = builder::vertex(Point3::new(c[0], c[1], c[2] + r));
    let bottom = builder::vertex(Point3::new(c[0], c[1], c[2] - r));
    let arc = builder::circle_arc(&top, &bottom, Point3::new(c[0] + r, c[1], c[2]));
    let wire: Wire = vec![arc].into();
    let shell = builder::cone(&wire, Vector3::unit_z(), FULL_TURN);
    solid_from_shell(shell)
}

fn solid_from_shell(shell: Shell) -> SpikeResult<Solid> {
    Solid::try_new(vec![shell]).or_else(|e| failed(format!("Solid::try_new: {e}")))
}

fn difference(a: &Solid, b: &Solid) -> SpikeResult<Solid> {
    let mut tool = b.clone();
    tool.not();
    truck_shapeops::and(a, &tool, BOOL_TOL).map_or_else(|| failed("truck_shapeops::and(a, !b) returned None"), Ok)
}

fn union(a: &Solid, b: &Solid) -> SpikeResult<Solid> {
    truck_shapeops::or(a, b, BOOL_TOL).map_or_else(|| failed("truck_shapeops::or returned None"), Ok)
}

fn case_box() -> Solid {
    cuboid([0.0; 3], [BOX_A, BOX_B, BOX_C])
}

fn box_minus_cylinder(a: f64, b: f64, c: f64, r: f64, z0: f64, z1: f64) -> SpikeResult<Solid> {
    let base = cuboid([0.0; 3], [a, b, c]);
    let tool = z_cylinder(a / 2.0, b / 2.0, r, z0, z1)?;
    difference(&base, &tool)
}

fn case_1a() -> SpikeResult<Solid> {
    box_minus_cylinder(BOX_A, BOX_B, BOX_C, HOLE_R, -1.0, BOX_C + 1.0)
}

/// Ruled loft between two closed wires with the same edge count, capped with planes.
/// `try_wire_homotopy` uses `wire0` edges forward and `wire1` edges reversed, so the
/// bottom cap takes `wire0` reversed and the top cap `wire1` as is.
fn capped_loft(wire0: &Wire, wire1: &Wire) -> SpikeResult<Solid> {
    let mut shell =
        builder::try_wire_homotopy(wire0, wire1).or_else(|e| failed(format!("try_wire_homotopy: {e}")))?;
    let bottom =
        builder::try_attach_plane(&[wire0.inverse()]).or_else(|e| failed(format!("try_attach_plane: {e}")))?;
    let top = builder::try_attach_plane(&[wire1.clone()]).or_else(|e| failed(format!("try_attach_plane: {e}")))?;
    shell.push(bottom);
    shell.push(top);
    solid_from_shell(shell)
}

fn circle_wire(r: f64, z: f64) -> Wire {
    let v = builder::vertex(Point3::new(r, 0.0, z));
    builder::rsweep(&v, Point3::new(0.0, 0.0, z), Vector3::unit_z(), FULL_TURN)
}

/// Builds the modelling result of a case. `None` means the case is handled elsewhere.
fn build(case_id: &str) -> SpikeResult<(Solid, Vec<String>)> {
    let none = Vec::new;
    match case_id {
        "1a" | "13c" => Ok((case_1a()?, none())),
        "1b" => Ok((box_minus_cylinder(BOX_A, BOX_B, BOX_C, HOLE_R, BOX_C - BLIND_DEPTH, BOX_C + 1.0)?, none())),
        "2a" => Ok((union(&case_box(), &cuboid([BOX_A, 0.0, 0.0], [BOX_A, BOX_B, BOX_C]))?, none())),
        "2b" => Ok((union(&case_box(), &cuboid([BOX_A, BOX_B, 0.0], [BOX_A, BOX_B, BOX_C]))?, none())),
        "3a" => {
            let r = HOLE_R;
            let v = builder::vertex(Point3::new(0.0, BOX_B / 2.0, BOX_C + 2.0 * r));
            let circle: Wire =
                builder::rsweep(&v, Point3::new(0.0, BOX_B / 2.0, BOX_C + r), Vector3::unit_x(), FULL_TURN);
            let disk =
                builder::try_attach_plane(&[circle]).or_else(|e| failed(format!("try_attach_plane: {e}")))?;
            let cyl = builder::tsweep(&disk, Vector3::new(BOX_A, 0.0, 0.0));
            Ok((union(&case_box(), &cyl)?, none()))
        }
        "3b" => {
            let s = sphere([BOX_A / 2.0, BOX_B / 2.0, BOX_C + HOLE_R], HOLE_R)?;
            Ok((union(&case_box(), &s)?, none()))
        }
        "4" => {
            let mut acc = cuboid([0.0; 3], PLATE);
            for c in pin_centres() {
                let pin = z_cylinder(c[0], c[1], PIN_R, PIN_Z0, PIN_Z1)?;
                acc = union(&acc, &pin)?;
            }
            Ok((acc, vec!["no n-ary union: folded truck_shapeops::or one pin at a time".into()]))
        }
        "10b" => Ok((capped_loft(&circle_wire(LOFT_R0, 0.0), &circle_wire(LOFT_R1, LOFT_H))?, none())),
        "10c" => {
            let [w, h] = LOFT_RECT;
            let (hw, hh) = (w / 2.0, h / 2.0);
            let corners = [[hw, hh], [-hw, hh], [-hw, -hh], [hw, -hh]];
            let rv: Vec<_> = corners.iter().map(|p| builder::vertex(Point3::new(p[0], p[1], 0.0))).collect();
            let rect: Wire = (0..4).map(|i| builder::line(&rv[i], &rv[(i + 1) % 4])).collect();
            // Circle split into four arcs whose end points lie over the rectangle corners.
            let r = LOFT_R1;
            let k = r / (hw * hw + hh * hh).sqrt();
            let cv: Vec<_> =
                corners.iter().map(|p| builder::vertex(Point3::new(p[0] * k, p[1] * k, LOFT_H))).collect();
            let mids = [PI / 2.0, PI, 1.5 * PI, 0.0];
            let circle: Wire = (0..4)
                .map(|i| {
                    let t = Point3::new(r * mids[i].cos(), r * mids[i].sin(), LOFT_H);
                    builder::circle_arc(&cv[i], &cv[(i + 1) % 4], t)
                })
                .collect();
            Ok((capped_loft(&rect, &circle)?, vec!["ruled loft (try_wire_homotopy) with 4 arcs on the circle".into()]))
        }
        "11a" | "13a" | "13b" => {
            let r = if case_id == "11a" { REV_SPHERE_R } else { TESS_SPHERE_R };
            let note = "revolve via builder::cone: profile is the arc only; the on-axis segment would \
                        revolve into a degenerate face, cone() collapses the end points on the axis";
            Ok((sphere([0.0; 3], r)?, vec![note.into()]))
        }
        "11b" => {
            let apex = builder::vertex(Point3::new(0.0, 0.0, REV_CONE_H));
            let rim = builder::vertex(Point3::new(REV_CONE_R, 0.0, 0.0));
            let base = builder::vertex(Point3::new(0.0, 0.0, 0.0));
            let wire: Wire = vec![builder::line(&apex, &rim), builder::line(&rim, &base)].into();
            let shell = builder::cone(&wire, Vector3::unit_z(), FULL_TURN);
            Ok((solid_from_shell(shell)?, vec!["builder::cone on the open profile apex→rim→base centre".into()]))
        }
        "5a" | "5b" | "5c" | "5d" | "6" => not_exposed("truck has no fillet"),
        "7a" | "7b" => not_exposed("truck has no chamfer"),
        "8a" | "8b" => not_exposed("truck has no shell/hollow"),
        "9a" | "9b" => not_exposed("truck has no fillet and no face offset/move"),
        "10a" => not_exposed("truck has no sweep along a curve (only tsweep/rsweep/homotopy)"),
        _ => failed(format!("unknown case {case_id}")),
    }
}

fn mesh_from_polygon(poly: &PolygonMesh) -> TriMesh {
    let positions = poly.positions().iter().map(|p| [p.x, p.y, p.z]).collect();
    let triangles = poly
        .faces()
        .triangle_iter()
        .map(|t| [t[0].pos, t[1].pos, t[2].pos].map(|i| u32::try_from(i).unwrap_or(u32::MAX)))
        .collect();
    TriMesh { positions, triangles }
}

/// Reasons `Solid::is_geometric_consistent` cannot be called without crashing the process:
/// `Surface::include` is `unimplemented!()` for `IntersectionCurve` edges (every boolean
/// result) and recurses forever for a `RevolutedCurve` surface generated by a `Line` (cone).
fn check_unsafe(solid: &Solid) -> Option<&'static str> {
    if solid.edge_iter().any(|e| matches!(e.curve(), Curve::IntersectionCurve(_))) {
        return Some("Solid::is_geometric_consistent panics (unimplemented!, geometry.rs:228) on IntersectionCurve edges, i.e. on every boolean result");
    }
    has_line_revolution(solid).then_some(
        "Solid::is_geometric_consistent overflows the stack (infinite recursion in Surface::include) on a RevolutedCurve generated by a Line (cone)",
    )
}

fn has_line_revolution(solid: &Solid) -> bool {
    solid
        .face_iter()
        .any(|f| matches!(f.surface(), Surface::RevolutedCurve(s) if matches!(s.entity_curve(), Curve::Line(_))))
}

fn topo_of(solid: &Solid) -> Topo {
    let vertices: HashSet<_> = solid.vertex_iter().map(|v| v.id()).collect();
    let edges: HashSet<_> = solid.edge_iter().map(|e| e.id()).collect();
    let faces = solid.face_iter().count();
    let loops = solid.face_iter().map(|f| f.boundaries().len()).sum();
    let closed = solid.boundaries().iter().all(|s| s.shell_condition() == ShellCondition::Closed);
    // `Solid::is_geometric_consistent` hits `unimplemented!()` in `Surface::include` for any
    // edge whose curve is an `IntersectionCurve`, i.e. for every boolean result. Skip it then.
    let valid = check_unsafe(solid).is_none().then(|| solid.is_geometric_consistent());
    Topo {
        vertices: vertices.len(),
        edges: edges.len(),
        faces,
        loops: Some(loops),
        shells: solid.boundaries().len(),
        backend_closed: Some(closed),
        backend_valid: valid,
        backend_check: Some("Shell::shell_condition == Closed; Solid::is_geometric_consistent".into()),
    }
}

fn step_header() -> StepHeaderDescriptor {
    // Every field set: `Default` reads the clock (chrono), which is not deterministic.
    StepHeaderDescriptor {
        file_name: "kubit-spike.step".into(),
        time_stamp: "2026-01-01T00:00:00".into(),
        authors: vec!["kubit".into()],
        organization: vec!["kubit".into()],
        organization_system: "truck".into(),
        authorization: String::new(),
    }
}

fn produce(solid: &Solid, tol: f64, op_seconds: f64, notes: Vec<String>) -> Produced {
    let mut poly = solid.triangulation(tol).to_polygon();
    // Without the weld the per-face meshes are not watertight. truck's weld overflows the stack
    // on the cone's mesh (apex/pole triangles), so it is skipped for line-revolved surfaces.
    if !has_line_revolution(solid) {
        poly.put_together_same_attrs(WELD_TOL);
    }
    let compressed = solid.compress();
    let step = CompleteStepDisplay::new(StepModel::from(&compressed), step_header()).to_string();
    Produced {
        mesh: Some(mesh_from_polygon(&poly)),
        topo: Some(topo_of(solid)),
        native_volume: None,
        native_area: None,
        history: Some(History::None),
        step: Some(step),
        step_error: None,
        op_seconds,
        notes,
    }
}

impl Backend for Truck {
    fn name(&self) -> &'static str {
        "truck"
    }

    fn run(&self, case_id: &str, tol: f64) -> SpikeResult<Produced> {
        let (built, secs) = timed(|| build(case_id));
        let (solid, mut notes) = built?;
        if let Some(why) = check_unsafe(&solid) {
            notes.push(format!("{why}; backend_valid left None"));
        }
        notes.push("no mass properties: volume from the closed mesh (triangulation + put_together_same_attrs)".into());
        Ok(produce(&solid, tol, secs, notes))
    }

    fn import_step(&self, text: &str, tol: f64) -> SpikeResult<Produced> {
        let (table, secs) = timed(|| Table::from_step(text));
        let Some(table) = table else {
            return failed("Table::from_step: STEP parse failed");
        };
        let mut keys: Vec<_> = table.shell.keys().copied().collect();
        keys.sort_unstable();
        let mut poly = PolygonMesh::default();
        let mut topo = Topo { loops: Some(0), ..Topo::default() };
        let (mut read, mut refused, mut closed) = (0usize, 0usize, true);
        let mut notes = Vec::new();
        for key in keys {
            let Some(holder) = table.shell.get(&key) else { continue };
            let cshell = match table.to_compressed_shell(holder) {
                Ok(c) => c,
                Err(e) => {
                    refused += 1;
                    notes.push(format!("shell #{key} refused: {e}"));
                    continue;
                }
            };
            read += 1;
            let mut p = cshell.robust_triangulation(tol).to_polygon();
            p.put_together_same_attrs(WELD_TOL);
            poly.merge(p);
            topo.vertices += cshell.vertices.len();
            topo.edges += cshell.edges.len();
            topo.faces += cshell.faces.len();
            topo.loops = topo.loops.map(|l| l + cshell.faces.iter().map(|f| f.boundaries.len()).sum::<usize>());
            topo.shells += 1;
            match truck_topology::Shell::extract(cshell) {
                Ok(shell) => closed &= shell.shell_condition() == ShellCondition::Closed,
                Err(e) => {
                    closed = false;
                    notes.push(format!("shell #{key}: Shell::extract failed: {e}"));
                }
            }
        }
        topo.backend_closed = Some(closed);
        topo.backend_check = Some("Shell::extract + shell_condition == Closed".into());
        notes.push(format!("{read} shell(s) read, {refused} refused; meshed with robust_triangulation"));
        if read == 0 {
            return failed(format!("no shell read from STEP ({refused} refused)"));
        }
        Ok(Produced {
            mesh: Some(mesh_from_polygon(&poly)),
            topo: Some(topo),
            history: Some(History::None),
            op_seconds: secs,
            notes,
            ..Produced::default()
        })
    }

    fn hostile(&self, family: Hostile, params: &[f64]) -> SpikeResult<()> {
        match family {
            Hostile::BoxMinusCylinder => {
                let &[a, b, c, r] = params else {
                    return failed(format!("expected 4 params, got {}", params.len()));
                };
                let solid = box_minus_cylinder(a, b, c, r, -1.0, c + 1.0)?;
                let _ = solid.triangulation(0.01).to_polygon();
                Ok(())
            }
            Hostile::FilletEdge => not_exposed("truck has no fillet"),
            Hostile::PushPull => not_exposed("truck has no fillet and no face offset/move"),
        }
    }
}
