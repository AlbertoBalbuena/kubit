# monstertruck 0.4.1 adapter — API notes

Crate: `monstertruck = "=0.4.1"`, features `solid` + `step` (Apache-2.0). Every signature below was
read from `~/.cargo/registry/src/*/monstertruck*-0.4.1/`. Paths are through the meta-crate
re-exports (`monstertruck::{modeling, solid, meshing, topology, step}`).

## Construction (`monstertruck_modeling`)
- `builder::vertex(p) -> Vertex`, `builder::vertices(iter) -> Vec<Vertex>`, `builder::line(&v0, &v1) -> Edge`.
- `builder::try_circle_arc(&v0, &v1, impl Into<CircularArcConstraint>) -> Result<Edge, Error>`
  (`circle_arc` is the panicking variant). A `Point3` constraint is a through-point.
- `builder::try_attach_plane(impl Into<Vec<Wire>>) -> Result<Face>`; `Error::WireNotInOnePlane`.
- `builder::extrude<T, Swept>(&T, Vector3) -> Swept` (vertex→edge→face→solid).
- `builder::revolve<T, Swept>(&T, origin, axis, SweepAngle, division) -> Swept`.
  **asserts** `division >= 2` for `SweepAngle::Closed` (`>= 1` for `Partial`), `debug_assert` unit axis.
- `builder::revolve_wire(&Wire, origin, axis, SweepAngle, division) -> Shell`: collapses edges whose
  end lies on the axis (sphere/cone apex). Uses `unwrap` on `front_vertex()`/`back_vertex()` (empty
  wire returns early) and `Edge::new_unchecked`.
- `builder::try_skin_wires(&[Wire]) -> Result<Shell>`: ruled `HomotopySurface` strips; the strip
  boundary is `[edge0, side, edge1.inverse(), side']`, so caps are `attach_plane(bottom.inverse())`
  and `attach_plane(top)`. `try_wire_homotopy` is the 2-wire special case.
- `primitive::cuboid(BoundingBox<Point3>) -> Solid`, `primitive::circle(start, origin, axis, division) -> Wire`.
- `builder::translated(&T, Vector3)`.
- `Solid::try_new(Vec<Shell>) -> Result<Solid>` (`Solid::new` panics); `Solid::not(&mut self)` inverts;
  `Solid::into_boundaries`, `boundaries`, `compress`, `is_geometric_consistent`.

## Booleans (`monstertruck_solid`)
`or / and / difference / symmetric_difference(&Solid, &Solid, tol: f64) -> Result<Solid, ShapeOpsError>`.
`tol` is the tessellation tolerance of the marching SSI backend (`marching-ssi`, default feature).
**Adapter uses `tol = 0.05`** (kernel tests use 0.05). Outputs go through `Solid::try_new`, so an `Ok`
result is closed and manifold by construction. No n-ary union: case 4 folds 100 calls.

## Fillet / chamfer (`monstertruck_fillet`, re-exported by modeling)
`fillet_edges(&mut Shell, &[Edge], Option<&FilletOptions>) -> Result<(), FilletError>`
(= `fillet_edges_generic`): converts the shell to internal NURBS, fillets, converts back. Edges are
re-matched by id/curve; tangent chains are detected (`chain.edge_ids`).
`FilletOptions { radius: RadiusSpec, divisions: NonZeroUsize (5), profile: FilletProfile }`,
`FilletOptions::constant(r)`, `::variable(Fn(f64)->f64)` (normalised edge parameter t∈[0,1]),
`.with_profile(..)`. `RadiusSpec::{Constant, Variable, PerEdge}`. `FilletProfile::{Round, Chamfer,
Ridge, Custom}` — Chamfer is a ruled surface between the two rolling-ball contacts, i.e. equal setback.
Env vars `MT_FILLET_STRICT_CLOSED` / `MT_FILLET_DEBUG` change behaviour (rollback on non-closed).
Not exposed: vertex blend, d-d / d-angle chamfer, shell/hollow, face offset/move, B-rep sweep along a curve.

## Meshing (`monstertruck_meshing::prelude`)
- `shell_to_polygon_strict(&Shell, tol) -> Result<PolygonMesh, TessellationError>`: like
  `triangulation(tol).to_polygon()` but refuses instead of silently dropping a face. Used here.
- `OptimizingFilter::put_together_same_attrs(TOLERANCE*50)`, `remove_degenerate_faces`,
  `remove_unused_attrs`; `Topology::shell_condition()` on the mesh.
- No native mass properties: `native_volume`/`native_area` stay `None`; the harness integrates the mesh.

## Topology checks (`monstertruck_topology`)
`Shell::shell_condition() == ShellCondition::Closed`, `Shell::check_solid_boundary()` (non-empty,
connected, closed, manifold). V/E deduplicated by `id()`; loops = Σ `face.boundaries().len()`.

## STEP (`monstertruck_io::step`)
- Write: `CompleteStepDisplay::new(StepModel::from(&solid.compress()), StepHeaderDescriptor{..}).to_string()`.
  Every header field is filled: `Default` calls `chrono::Utc::now()` (no clock on wasm32).
- Read: `Table::from_step(&str) -> Result<Table, LoadError>` (indexes `exchange.data[0]`),
  `table.manifold_solid_brep` (map), `table.to_compressed_solid(&holder) -> Result<CompressedSolid<Point3, Curve3D, Surface>>`,
  then `topology::Solid::extract(cs)`.

## History
None: no generated/modified/deleted information is returned by any operation.

## Results (run 2026-10-27, other backends running concurrently: timings indicative)
Native (`spike all --backends monstertruck`, `results/table.md`):
- ✅ 1a 1b 3a 5a 5d 7a 10b 10c 11a 11b 13a 13b 13c; εV ≤ 2.3e-4 (mesh volume).
- ⚠️ 5b: `fillet_edges` returns `Ok`, but the new fillet surface has NaN control points
  (visible in the written STEP). The strict mesher drops that face (`boundary-projection-failed`),
  so there is no volume. Only the selected edge is filleted (6 → 7 faces): the tangent chain is
  not propagated. The adapter marks it invalid via the NaN/inf scan of the STEP text.
- ❌ 2a (shared face), 2b (shared edge), 3b (sphere tangent to a face): `or` fails with
  "shell is not oriented and closed".
- ❌ 4: the very first plate ∪ pin already fails. Bisected: box ∪ cylinder entering one face and
  ending inside the solid fails or succeeds depending on dimensions alone. With box 100×60×40
  and a cylinder r10 from z=20 upward: z1=41 fails, 42/45/50 work, 60 fails (`difference` too).
  The marching SSI is fragile, not the adapter. The cylinder seam division (2/3/4), operand
  order and tol (0.05/0.01) change nothing.
- ❌ 5c: filleting the 3 edges at a corner in one call gives an open shell (no vertex blend);
  `Solid::try_new` refuses it.
- ❌ 6: `fillet_edges` on the 2 hole-rim edges returns `Ok` but adds no face (8 before/after):
  a silent no-op. The adapter now checks that a fillet adds ≥ 1 face and fails otherwise.
- 3a: the cylinder tangent to the top face along a line comes back as a 2-shell solid.
- N/E 7b 8a 8b 9a 9b 10a (see above).
- 12rt: 9 ✅ (round-trip εV ≤ 1.4e-4), ❌ 12rt:5b (the kernel's own STEP reader refuses the NaN
  coordinates its writer emitted). 12rt:1a/1b/6 re-import fine topologically, but the strict mesher
  drops a face on the re-imported `step_geometry::Surface`, so they have no volume.
- `Solid::is_geometric_consistent()` is false for 1a 1b 5a 5b 5d 6 7a 13c, even 7a with
  εV 2e-16, so it is too strict to use as a validity verdict. It is recorded as a note only.
- 15:1 (box − cylinder): ⏱ the runner killed it after 120 s on
  `[2.2250738585072014e-308, 82.92, -1.0, 1e9]` (denormal/negative box, 1e9 radius), so the
  kernel hangs.
- 15:5 (fillet): 4 💥/256, first `[196.9, 1e9, 175.2, 0.0]`. It panics in
  `monstertruck-traits/src/traits/curve.rs:440` (NaN curve end points). The other 252 are clean
  errors. Before the no-op check, 92 of them returned `Ok`: `fillet_edges` silently accepts
  radii of 0, negative or huge and does nothing.
- 15:9 N/E. 16: no history.

wasm (`wasm/monstertruck/result.json`): compiles; 3 117 419 B raw, 818 259 B gzip (cargo output, no
wasm-opt). Node 26: 6/18 ok, 1 invalid (5b), 5 N/E, 6 failed, 0 traps. The results match native
exactly. Needs getrandom 0.3 and 0.4 with `wasm_js` plus `--cfg getrandom_backend="wasm_js"`.

## Done / next
- Done: adapter, corpus run, wasm probe; booleans at tol 0.05.
- Kernel limits that matter for Kubit: fragile booleans in generic cases (case 4), silent fillet
  no-ops and NaN fillet surfaces, no vertex blend, no shell/offset/sweep, panics on hostile
  fillet input, a hang on hostile booleans.
- Next (if monstertruck stays in the race): report the case-4 bisection and the 5b/6 fillet
  defects upstream with the minimal inputs above.
