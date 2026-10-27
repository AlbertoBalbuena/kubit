# Kernel spike — API notes

Exact public APIs used by each adapter, read from the dependency sources (2026-10-26).
"N/E" = no public API: recorded as a result, never emulated.

## arris 0.9.1 (MIT OR Apache-2.0)
- Shape of every op: `op(&mut Model, inputs…, &Control) -> Result<(Body, Provenance), OpError>`; on `Err` the model is unchanged. Debug builds **panic** if an output fails the checker (`arris-ops/src/lib.rs:110`); release skips it unless `--features paranoid` (then `OpError::Internal`). The spike runs release.
- Primitives: `ops::primitive_box(m, min, max, ctl)` (`primitive.rs:127`), `ops::primitive_cylinder(m, Axis, r, h, ctl)` (`:377`). Sphere/cone: N/E as primitives → `revolve` of a profile.
- Booleans: `ops::{cut, fuse, common, cut_many, fuse_many}` (`boolean/mod.rs:719/945/985/790/847`). No tolerance argument (model `Precision`, default 1e-7).
- Blends: `ops::fillet(m, body, &[Edge], r, ctl)` (`blend/mod.rs:276`), `ops::chamfer(m, body, &[Edge], d, ctl)` (`:363`, equal distance only). Chains follow tangent vertices. Vertex blend: automatic at a 3-plane corner (sphere patch). **Variable radius: N/E. Chamfer d-angle / two distances: N/E.**
- Shell: `ops::shell(m, body, openings:&[Face], t, ShellSide, ctl)` (`shell/mod.rs:94`).
- Push/pull: `ops::offset_faces(m, body, faces, distance, ctl)` (`offset/mod.rs:133`), tangent faces dragged along. Move face / delete face: N/E.
- Sweeps: `extrude`, `revolve(m, &Profile, Axis, angle, ctl)`, `sweep(m, &Profile, &Path, FrameLaw, ctl)` with `PathSegment::Helix{axis, pitch, angle}`, `loft(m, &[Profile], LoftKind, closed, ctl)`.
- Mass: `ops::measure::mass_properties(m, body, ctl) -> MassProperties{volume, area, centroid, inertia}` (exact B-rep integration).
- Topology: `m.{shells,faces,edges,vertices}(body)`; checker `check::check(m, body, Level::Full) -> Report` (`is_ok()`, `euler() -> Option<EulerLine{vertices,edges,faces,loops,shells,genus}>`).
- History: **native** `Provenance` on every op: `generated_from`, `modified_from`, `deleted()`, `outputs()`, roles (`Role::Box(BoxPart::Face(Coord, Side))`, …).
- Mesh: `mesh::tessellate(m, body, chord, ctl) -> TriMesh` (chord only, no angular tol; watertight by construction).
- STEP AP214: `io::step::write(m, &[Body]) -> Result<String, StepError>`, `io::step::read(m, text, &ReadOptions, ctl) -> Result<Read{solids}, ReadError>`.
- wasm: rayon only behind `parallel` (off by default); no clock/threads/fs in library code.

## solidcore @ dbaedda (Apache-2.0)
- `Kernel::default()`; primitives in `primitive::api_primitives`: `create_box(k, origin, l, w, h)`, `create_cylinder(k, origin, r, h)`, `create_cone`, `create_sphere`, all `Result<Brep>`.
- Booleans `boolean::api_brep::{boolean_union, boolean_difference, boolean_intersection}(k, Brep, Brep) -> Result<Brep>`; no tolerance arg (`Kernel.tolerances`).
- Fillet (in place): `blend::fillet_executive::api::fillet_edges(k, &mut Brep, &[EdgeId], r, xsect_type, continuity, thumbweight)`; `variable_radius_fillet(k, b, edges, r0, r1, xsect, cont)` (linear law); corners automatic. Chamfer equal-distance = `fillet_edges(.., xsect_type=0, ..)`; **d-angle: N/E**. Copy bodies with `api_brep_copy`, not `clone()` (#1144).
- Shell: `offset::offset_executive::p2::api_shell_brep_full(...)` — needs a hand-built `OffsetNotifications` (record/attribute stores + key closures); no public test; template is `crates/difftest/src/units/offset_offset_executive_p2.rs:1644`.
- Direct edit (offset/move face, push/pull): **N/E** (only low-level `delete_face`, `replace_surface`).
- Sweeps: `create_curve_sweep(k, profile, Some(&helix), …)` + `api_curves::create_helix`; loft `create_skin_primitive`; revolve `create_rotational_sweep(k, curves, base, axis, angle_deg, cap)`.
- Mass: `mass::integrator::properties::{brep_volume, brep_area}(k, &mut b, accuracy, …)`.
- Topology: `face_list/edge_list/vertex_list`, `is_manifold_solid`, `topology::integrity::check`.
- History: **none** returned by booleans/fillets; sweeps return start/side/end faces (partial).
- **Tessellation of a Brep: N/E** (only low-level `TessSrfCache`; separate `nurbs-grid-mesh` needs a hand-written Brep→NURBS adapter).
- STEP: import `step_brep::import(bytes, &Limits) -> Result<Vec<Brep>, BuildError>`; **export of a Brep: N/E** (`step_export::write` takes the neutral model and writes only planar single-shell B-reps).
- Errors: `SolidError` incl. `Unported{..}` (`is_unported()`); library is lint-denied from unwrap/panic.
- wasm: only dependency `thiserror`; CI checks wasm32; needs 8 MiB stack.

## truck 0.6 / shapeops 0.4 / stepio 0.3 / meshalgo 0.4 (Apache-2.0)
- Builder: `builder::{vertex, line, circle_arc, tsweep, rsweep, homotopy, try_wire_homotopy, try_attach_plane}`.
- Booleans: `truck_shapeops::{and, or}(&Solid, &Solid, tol) -> Option<Solid>`; difference = `and(a, &b.not())`. `and/or` call `boundaries().iter().next().unwrap()` (panic on an empty solid).
- Fillet, chamfer, shell, offset/move face, sweep along a curve: **N/E**.
- Mesh: `MeshableShape::triangulation(tol)` + `put_together_same_attrs` to close it (meshalgo doc). Volume/area: only from the mesh.
- STEP: write via `out::CompleteStepDisplay::new(out::StepModel::from(&compressed), header)`; read via `ruststep::parser::parse` + `r#in::Table::from_data_section` + `to_compressed_shell`.

## monstertruck 0.4.1 (Apache-2.0)
See the section added after the API survey below.
