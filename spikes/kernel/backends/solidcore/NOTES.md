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
  **Surprise:** with every flag `false` the call returns `Ok` with the six untrimmed offset faces
  (24 vertices, volume 0, not manifold) and no error. `merge_results: true` (or `extend_convex_edges`)
  gives the exact hollow box (35712.000 mm³). The adapter uses `merge_results: true`.
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
  **Surprise (10a):** the profile circle is centred on the helix start (20,0,0), yet with default
  options the section drifts off the meridian plane (ends at z≈30.0 instead of 32, volume −6.3 %).
  `move_profile_center_to_path: true` gives the analytic volume (rel. err 2e-6); the adapter uses it.
  `SweepOptions { cap_ends, translational_sweep, move_profile_center_to_path, orient_profile_perpendicular_to_path, move_to_profile }`.
- `sweep::api_surfaces::create_skin_primitive(k, curves_in_each_profile: &[usize], curves: &[Option<&Curve>], degree, cap_ends) -> Result<SkinPrimitive>`
  (same start/side/end face lists) — loft, degree 1 = ruled.

## Measurements and checks
- `mass::integrator::properties::brep_volume(k: &Kernel, &mut Brep, accuracy, &PropertyAttributes { store, key_of }) -> Result<f64>`
- `mass::integrator::properties::brep_area(k: &Kernel, &mut Brep, accuracy) -> Result<f64>`; accuracy 1e-6.
- Topology: `topology::brep::p1::{faces, edges, vertices}(b, None)`, `p1::shells(k: &mut Kernel, b, None)`
  (region model: one shell per side of each face set, so a box has 2; the adapter counts only shells whose
  `topology::shell::shell_region` is not `Brep::infinite_region()`),
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

## Corpus result (2026-10-27)
- 22 ok, 4 not exposed (7b, 9a, 9b, hostile 15:9), 0 failed, 0 panics, 0 crashes, 0 timeouts; no `Unported` error hit.
- Volumes (mass accuracy 1e-9; at 1e-6 the integrator itself adds up to 9e-6) within 8e-8 of the oracle,
  except 10a at 2.2e-6 (the helix is a B-spline fit at tolerance 1e-4). 2b and 3a: the kernel reports the
  edge/tangent contact as non-manifold (`is_manifold_solid` false, 2 shells), which is a correct verdict.
- 5a–5d, 6, 7a: `integrity::check` fails after `fillet_edges` with "cached face shell: face N not found" —
  the fillet API does not call `Brep::refresh_cached_topology` (upstream notes this as uncovered, #1127).
  Volumes are still exact; the ⚠️ in the table comes from this check.
- Hostile 15:1 and 15:5: 256/256 clean errors, 0 panics (a unit test confirms both families succeed on sane input).
- The exit 144 of the first run was the parent being killed (signal 16), not a case: every case runs
  alone in under 7 s.
- 12rt: not exposed (no B-rep STEP export). 12imp: no fixtures under `fixtures/{nist,local}` in this checkout.
