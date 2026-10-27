# arris 0.9.1 — API used by the spike adapter

License: MIT OR Apache-2.0. Facade crate `arris` re-exports `arris_{math,geom,topo,check,mesh,ops,io}`
as `arris::{math,geom,topo,check,mesh,ops,io}`. Signatures below were read from the published
sources in `~/.cargo/registry/src/*/arris-*-0.9.1/` (paths relative to each crate's `src/`).

Every operation takes `&Control` last; the adapter passes `&Control::NONE` (no poll, no budget).
On `Err` the model is left unchanged (each op runs in `Model::transaction`).

## Primitives and sketches
- `ops::primitive_box(m: &mut Model, min: impl Into<Point3>, max: impl Into<Point3>, &Control) -> Result<(Body, Provenance), OpError>` — `arris-ops/primitive.rs:127`. Rejects non-finite corners and non-positive extents.
- `ops::primitive_cylinder(m, axis: Axis, radius: f64, height: f64, &Control)` — `primitive.rs:377`. Base disc at `axis.origin`, grows along `axis.direction`.
- `math::Axis::new(origin: Point3, direction: Vec3) -> Result<Axis, FrameError>` (`arris-math/axis.rs:31`), `Axis::z_at(origin)` (`:41`).
- `math::Frame::new(origin, z: Vec3, x_hint: Vec3) -> Result<Frame, FrameError>` (`arris-math/frame.rs:122`). Profile (u, v) = (x, z × x … i.e. y = z × x). Meridian plane: `Frame::new(o, −Y, X)` gives (u, v) = (x, z).
- `geom::Profile { plane: Frame, outer: ProfileLoop, holes: Vec<ProfileLoop> }` (`arris-geom/profile.rs:35`); `ProfileLoop::{Circle{center, radius}, Ellipse{..}, Path{start, segments}}` (`:48`); `ProfileSegment::{LineTo(Point2), ArcTo{to, via}, EllipseTo{..}}` (`:82`).
- No sphere or cone primitive: built with `revolve` of a half disc / triangle touching the axis (cases 3b, 11a, 11b, 13a/b).

## Operations (all `-> Result<(Body, Provenance), OpError>`)
- Booleans `ops::{cut(m, target, tool, ctl), fuse(m, a, b, ctl), common(m, a, b, ctl), cut_many(m, target, &[Body], ctl), fuse_many(m, &[Body], ctl)}` — `boolean/mod.rs:719/945/985/790/847`. No tolerance argument (model `Precision`). `fuse` is `fuse_many` of two. Case 4 uses `fuse_many` (n-ary).
- `ops::fillet(m, body, edges: &[Edge], radius, ctl)` — `blend/mod.rs:276`. Constant radius only. Tangent chains are followed automatically; a 3-plane corner gets a vertex blend.
- `ops::chamfer(m, body, edges, distance, ctl)` — `blend/mod.rs:363`. Equal distance only.
- `ops::shell(m, body, openings: &[Face], thickness, ShellSide::{Inward, Outward}, ctl)` — `shell/mod.rs:94`.
- `ops::offset_faces(m, body, faces: &[Face], distance, ctl)` — `offset/mod.rs:133`. Direct face offset; tangent neighbours (fillets) are dragged along; refuses topology changes.
- `ops::extrude(m, &Profile, direction: Vec3, length, ctl)` — `sweep/extrude.rs:74`.
- `ops::revolve(m, &Profile, axis: Axis, angle, ctl)` — `sweep/revolve.rs:408`. Axis must lie in the profile's plane.
- `ops::sweep(m, &Profile, &Path, FrameLaw, ctl)` — `sweep/along.rs:174`. `geom::Path { start, segments: Vec<PathSegment> }`, `PathSegment::Helix { axis, pitch, angle }` (`arris-geom/path.rs:40`), `FrameLaw::{RotationMinimizing, FixedBinormal(UnitVec3)}` (`path.rs:98`). Side faces are NURBS fitted within ¼ of `default_tolerance`.
- `ops::loft(m, sections: &[Profile], LoftKind::{Ruled, Smooth}, closed: bool, ctl)` — `sweep/loft.rs:187`.

## Queries and measurements
- `topo::Model::{faces, edges, vertices, shells}(body) -> Result<Vec<handle>, NotFound>` (`arris-topo/walk.rs:58-89`); `Model::{edge, face, curve, surface}(id) -> Result<&T, NotFound>` (accessor macro, `model.rs:151`).
- `entity::Edge::curve() -> Option<(CurveId, Interval)>`; `geom::Curve::point(t) -> Point3` (`arris-geom/curve.rs:144`). The adapter picks edges by sampling 5 points along the curve.
- `check::classify_point(m, body, Point3) -> Result<Classification, ClassifyError>` (`arris-check/classify.rs:130`); `Classification::On(Shape)` names the most specific entity (vertex, else edge, else face). The adapter picks faces with it, as the arris doc examples do.
- `ops::measure::mass_properties(m, body, ctl) -> Result<MassProperties{volume, area, centroid, inertia}, OpError>` (`arris-ops/measure.rs:213`); exact B-rep integration.
- `check::check(m, body, Level::Full) -> Report` (`arris-check/check.rs:43`): `is_ok()`, `euler() -> Option<EulerLine{vertices, edges, faces, loops, shells, genus}>`.
- `mesh::tessellate(m, body, chord, ctl) -> Result<TriMesh, MeshError>` (`arris-mesh/tessellate.rs:141`); `TriMesh::{positions() -> &[[f64;3]], triangles() -> &[[u32;3]]}`.

## History (native)
`topo::Provenance` (`arris-topo/provenance.rs:418`): `generated_from(origin)`, `modified_from(origin)`, `deleted()`, `outputs()`, `origins_recorded()`. Origins are input entities (`Shape`) or roles (`Role::Box(BoxPart::Face(Coord, Side))`, `Role::Revolve(SweepPart)`, …). The adapter reports the **last** operation's record as `History::Native`, counting distinct outputs over every origin (all entity kinds), and the face-only counts in `detail`.

## STEP
- `io::step::write(m, &[Body]) -> Result<String, StepError>` (`arris-io/step/mod.rs:207`), AP214, millimetres.
- `io::step::read(m, text, &ReadOptions, ctl) -> Result<Read{solids: Vec<ReadSolid{result: Result<ReadBody{body, provenance, healing}, Refusal>, ..}>, ..}, ReadError>` (`step/reader/mod.rs:603`). One refused solid never hides another.

## Not exposed
- Variable-radius fillet (5d), distance–angle or two-distance chamfer (7b).
- Sphere/cone primitives (built by revolve instead; not a gap for the corpus).

## Build notes
- Debug builds run the checker after every op and **panic** on a failing output (`arris-ops/lib.rs:110`). The spike runs release (checker skipped unless feature `paranoid`).

## Status (2026-10-27)
- Native corpus (`results/table.md`): 20 ok, 1 warning (2a), 3 clean kernel errors (2b, 3a, 3b), 2 N/E (5d, 7b). Every volume within 1e-12 of its oracle. STEP round-trip: 17 ok, 1 warning (2a), 2 skipped (5d, 7b). Hostile: 768/768 clean errors, 0 panics (`tests/hostile_sane.rs` shows ordinary params pass). No panics or crashes anywhere.
- 2a ⚠️: the fuse is valid and exact but keeps 10 faces (no coplanar-face merge); the oracle expects 6.
- 2b/3a/3b are documented refusals: shells touching along an edge, tangent line contact, plane–sphere point contact (`BooleanReason`).
- 10a timed out at the default 120 s in `spike all`: the sweep is 0.1 s, but the runner's 5 repeats × (tessellate at 0.002 → ~10^6 triangles of a NURBS skin, mass properties, Level::Full) take ~78 s alone, more under load. Rerun with `--timeout 400`: ok, εV 6e-13. `examples/profile10a.rs` times each step.
- wasm: 13/18 ok, 2 N/E, 3 clean errors, 0 traps; 4.70 MB raw / 1.47 MB gzip; case 4 takes ~44 s in Node.
- Next: NIST STEP import (12imp) once the fixtures are fetched.
