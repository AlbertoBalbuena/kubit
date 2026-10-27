# solidcore adapter — API used

Source: `tangency-labs/solidcore` @ `dbaedda4abc4b390494d2e86f79cc5115168e889` (Apache-2.0),
read from `~/.cargo/git/checkouts/solidcore-a854643c1e2d477c/dbaedda/`. Crates used:
`solidcore`, `step-brep`, `step-import` (for `Limits`). Every signature below was read
from that source.

## Kernel and errors
- `solidcore::Kernel::default()` — one kernel per case (tolerances, RNG, caches live in it).
- `solidcore::Result<T> = Result<T, SolidError>`; `SolidError::is_unported(&self) -> bool`.
  The adapter prefixes `Unported:` to the message of such errors.

## Primitives (`primitive::api_primitives`)
- `create_box(k: &mut Kernel, origin: Point3, length, width, height) -> Result<Brep>` (min corner at origin).
- `create_cylinder(k, origin: Point3, radius, height) -> Result<Brep>` — always about +Z from `origin`.
- `create_sphere(k, origin, radius) -> Result<Brep>`.
- No oriented cylinder: case 3a rotates a Z cylinder and then places it by its bounding box.

## Transforms (`topology::proto_topology::p1::api_transform`)
- `api_rotate(k, Option<TransformObject<'_>>, ref_pt: Point3, axis: Vec3, angle_deg: f64) -> Result<()>`
- `api_translate(k, Option<TransformObject<'_>>, translate: Vec3) -> Result<()>`
- `TransformObject::{Brep(&mut Brep), Curve(&mut Curve), ..}` — curves are rotated into the XZ plane.
- `query::brep::calculate_bounding_box(k: &Kernel, b: impl Into<LazyBrep>, BoundingBoxOptions) -> Result<Extent3d>`; `Extent3d::{min, max, mid}`.

## Curves (`primitive::api_curves`), `Curve::from(..)`
- `create_line_segment(start, end) -> Result<Line>`
- `create_circle(center, radius) -> Result<BSplineCurve>` (XY plane, quadratic rational)
- `create_arc(center, radius, start_deg, end_deg) -> Result<BSplineCurve>` (XY plane)
- `create_helix(&HelixParams { origin, height, radius_start, radius_end, num_turns, right_handed, tolerance }) -> Result<BSplineCurve>`
  — a B-spline approximation of the helix (tolerance 1e-4 used, as the thread test does).

## Booleans (`boolean::api_brep`)
- `boolean_union / boolean_difference (k, Brep, Brep) -> Result<Brep>` (operands consumed; no tolerance arg).
- `boolean_lists(k, breps1: &[Brep], surfaces1: &[Surface], breps2: &[Brep], surfaces2: &[Surface], operation: i32) -> Result<BooleanListsResult>`
  (`operation` 1 = union; `.breps: Vec<Brep>` one per separate solid). Used as the n-ary union for case 4.
- No history output (`BooleanOutcome` from `boolean()` carries only the result).

## Blends (`blend::fillet_executive::api`)
- `fillet_edges(k, &mut Brep, edges: &[EdgeId], radius, xsect_type: u32, continuity: u32, thumbweight: f64) -> Result<()>`
  — `xsect_type` 0 linear (= chamfer, equal setback), 2 blend curve, other = circular. Used with `(1, 1, 1.0)` for fillets
  and `(0, 1, 1.0)` for case 7a. Corners are made by the session automatically (case 5c).
- `variable_radius_fillet(k, &mut Brep, edges, start_radius, end_radius, xsect_type, continuity) -> Result<()>` — linear law along the edge parameter.
- `chamfer_fillet(k, &mut Brep, distance)` chamfers **every** edge (not used). No distance–angle or two-distance chamfer → 7b `not_exposed`.

## Shell (`offset::offset_executive::p2`)
- `api_shell_brep_full(k, original: Option<BrepRef<'_, BrepId>>, options: ShellOptions, faces: &[Option<FaceKey>], copy: &ShellCopyIdentities<'_>, notifications: &mut OffsetNotifications<'_>) -> Result<Brep>`
- `ShellOptions { offset: OffsetOptions { distance (negative = inset), extend_convex_edges, merge_results }, create_offset_solid, merge_sheet_side_walls }`.
- `FaceKey = (BrepId, FaceId)`: the `BrepId` is an adapter handle that must match `BrepRef::handle`.
- `OffsetNotifications` needs: a `BTreeMap<ObjectKey, Topology>` with every source record registered,
  an `AttributeStore`, key closures for source/result entities and curves, Boolean source-list handles,
  the reserved result B-rep key and a `SweepIdentities` impl for merge-created B-reps.
  `src/shell.rs` reproduces the identity scheme of solidcore's own difftest
  (`crates/difftest/src/units/offset_offset_executive_p2.rs` ~1416-1660). This is ~150 lines of
  plumbing that a product integration would have to own.

## Sweeps
- `sweep::api_brep::create_linear_sweep(k, curves: &[Option<&Curve>], dir: Vec3, dist, cap_ends: bool) -> Result<Brep>` (5b prism).
- `sweep::api_brep::create_rotational_sweep(k, curves, base_pt, axis, angle_deg, cap_ends) -> Result<Brep>` (11a/11b). No face lists → history `None`.
- `sweep::api_brep::create_curve_sweep(k, profile: &[Option<&Curve>], path: Option<&Curve>, scale_ref, scale, Option<&SweepOptions>) -> Result<CurveSweep>`;
  `CurveSweep { brep, start_faces, side_faces, end_faces }` → history `Partial`.
  `SweepOptions { cap_ends, translational_sweep, move_profile_center_to_path, orient_profile_perpendicular_to_path, move_to_profile }`.
- `sweep::api_surfaces::create_skin_primitive(k, curves_in_each_profile: &[usize], curves: &[Option<&Curve>], degree, cap_ends) -> Result<SkinPrimitive>`
  (same start/side/end face lists) — loft, degree 1 = ruled.

## Measurements and checks
- `mass::integrator::properties::brep_volume(k: &Kernel, &mut Brep, accuracy, &PropertyAttributes { store, key_of }) -> Result<f64>`
- `mass::integrator::properties::brep_area(k: &Kernel, &mut Brep, accuracy) -> Result<f64>`; accuracy 1e-6.
- Topology: `topology::brep::p1::{faces, edges, vertices}(b, None)`, `p1::shells(k: &mut Kernel, b, None)`,
  `topology::face::p2::loops(b, f, false, None)` (all loops of a face).
- Closed: `topology::brep::p2::is_manifold_solid(b) -> Result<bool>`; valid: `topology::integrity::check(b) -> Result<()>`.
- Picking: `edge::{start_end_vertices, curve, interval}`, `vertex::point`, `Curve::evaluate_point(k, t)`;
  faces by sampling `face.surface.evaluate_point(k, face.uv_domain.evaluate(u, v))`.

## Not exposed
- **Tessellation of a B-rep**: none public (`TessSrfCache` is internal per-surface). The sibling crate
  `nurbs-grid-mesh` meshes NURBS grids, but feeding it a trimmed `Brep` needs a hand-written
  Brep→NURBS/trim adapter — not written here, so `mesh = None` for every case.
- **STEP export of a B-rep**: `step-export` writes the neutral `step-import` model only (planar,
  single shell); there is no `Brep → STEP` path → `step_error`.
- **Push/pull / face offset / move face** (9a/9b): only low-level `delete_face`, `replace_surface`.
- **Distance–angle chamfer** (7b).

## STEP import
- `step_brep::import(bytes: &[u8], limits: &step_import::Limits) -> Result<Vec<Brep>, BuildError>`; `Limits::default()`.

## Copies
- Bodies are never copied in this adapter (every case rebuilds from scratch). If a copy were needed it
  must be `topology::brep::p3::api_brep_copy` (solidcore#1144: `clone()` loses geometry ownership and
  breaks later fillets).

## Status (paused 2026-10-26)
- Done: adapter for every case (`src/lib.rs`), shell through a hand-built `OffsetNotifications` (`src/shell.rs`), STEP import, hostile families; `cargo build --release -p backend-solidcore` compiles with no warnings. wasm probe scaffolded in `wasm/solidcore/` (Cargo.toml, run.mjs, measure.sh), not built yet.
- Not done: the corpus has **not** run yet (the first `spike all --backends solidcore` was killed, exit 144, before any output); no `result.json`.
- Next: run `./target/release/spike all --backends solidcore`, fix the geometry against `results/table.md` (check 3a placement, 5d law direction, 10a profile frame, 11a/11b curve rotation), confirm whether `boolean_lists` really unites n-ary for case 4 (fall back to a fold if it fails), then run `wasm/solidcore/measure.sh` and commit `result.json`.
