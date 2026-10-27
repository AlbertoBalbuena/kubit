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

## Status (paused 2026-10-26)
- Done: API survey above; adapter for every case (1a–13c), `import_step`, `hostile`; builds in release with the runner.
- Not yet run: `spike all --backends monstertruck` was started and interrupted by the pause, so there are no results yet.
- Next: run the corpus, fix geometry wherever volume errors are large (orientation of revolve/loft, 5b chain selection, 6 rim edges), record 5c behaviour.
- Next: wasm probe in `wasm/monstertruck/` (cdylib, wasm-bindgen 0.2.129, getrandom `wasm_js`), `run.mjs`, `measure.sh`, `result.json`.
