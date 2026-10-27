# truck adapter notes (truck-modeling 0.6.0, shapeops 0.4.0, stepio 0.3.0, meshalgo 0.4.0)

Signatures verified in `~/.cargo/registry/src/*/truck-*`:

- `builder::vertex(Point3) -> Vertex`; `line(&v0,&v1)`; `circle_arc(&v0,&v1,transit) -> Edge`.
- `builder::tsweep(&elem, Vector3)` (vertex→edge→face→solid). `rsweep(&elem, origin, axis, Rad)`: |angle| ≥ 2π is a closed sweep.
- `builder::cone(&Wire, axis, Rad) -> Shell` (revolve whose end points on the axis are collapsed); `Solid::try_new(vec![shell])`.
- `builder::try_attach_plane(&[Wire]) -> Result<Face>`; `try_wire_homotopy(&Wire,&Wire) -> Result<Shell>` (ruled loft, same edge count).
- `truck_shapeops::{and, or}(&Solid,&Solid,tol) -> Option<Solid>`; difference = `and(a, &b.not())`. `unwrap`s on empty solids; kernel panics in `Solid::new` seen (3a).
- Mesh: `MeshableShape::triangulation(tol).to_polygon()` + `put_together_same_attrs(tol)`; `RobustMeshableShape::robust_triangulation` for STEP-imported shells. Volume/area only from the mesh.
- STEP out: `CompleteStepDisplay::new(StepModel::from(&solid.compress()), StepHeaderDescriptor{..})` (fill all fields: `Default` reads chrono clock). STEP in: `Table::from_step(&str)` → `table.shell` → `to_compressed_shell` → `robust_triangulation`.
- Not available: fillet, chamfer, shell, face offset/move, sweep along a curve → `not_exposed`.

## Findings so far
- `Solid::is_geometric_consistent` panics (`unimplemented!`, truck-modeling geometry.rs:228/234) on any edge with an `IntersectionCurve` = every boolean result. Adapter skips it for such solids (`backend_valid = None`) and says so in `notes`.
- 2a/2b: `or` returns `None`. 3a: panic "This shell is not oriented and closed" inside the boolean. 4 (100-pin fold): >120 s timeout. 11b (cone): stack overflow SIGABRT in the runner (investigate: `builder::cone` profile or tessellation). STEP round-trip of 1a hangs/times out (stderr "Lookup failed for #578"). Hostile BoxMinusCylinder with NaN: timeout.
- 10b/10c ok; 11a/13a/13b sphere mesh not watertight after weld (b0 nm6 mo12 — degenerate pole faces).

## Done / next (paused)
- Done: adapter for all cases, builds, corpus run once. wasm probe NOT started.
- `results/table.md` looked stale vs the live run log (1a/1b/13c ok in log, panic in table): re-run `cargo build --release -p runner --no-default-features --features truck && ./target/release/spike all --backends truck` and re-read.
- Next: investigate 11b stack overflow, sphere watertightness, 12rt hang; write `wasm/truck/` per `wasm/README.md`; final report.
