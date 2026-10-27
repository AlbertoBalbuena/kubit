//! Kernel backend over [arris](https://crates.io/crates/arris) 0.9.1, a pure-Rust B-rep
//! kernel (MIT OR Apache-2.0). Provisional backend of ADR-0002.
//!
//! One arris [`Model`] holds every body of the kernel instance. Kubit ids are arris
//! ids packed as `generation << 32 | index`, so they decode back without a lookup
//! table and a freed slot never aliases a live id (arris bumps the generation).
//!
//! What arris lacks keeps the trait's `Unsupported` default; nothing is emulated.

use std::collections::BTreeMap;

use arris::check::flux::face_flux;
use arris::check::{Level, check};
use arris::math::{Axis, Control, Isometry, Point3, Vec3};
use arris::ops::query::face_frame;
use arris::ops::{OpError, ShellSide};
use arris::topo::provenance::Origin;
use arris::topo::{Body, EntityId, Model, Provenance, Shape};
use kubit_kernel_api::{
    BodyId, BooleanOp, BoxSpec, Capabilities, CylinderSpec, EdgeId, EntityRef, FaceId, FaceInfo,
    Interrupt, Kernel, KernelError, KernelInfo, KernelResult, MassProperties, OpHistory, OpOutput,
    SurfaceKind, Validity, VertexId,
};
use kubit_mesh::{EdgeTag, FaceTag, TriMesh};

/// Name reported in [`KernelInfo`].
pub const NAME: &str = "arris";
/// Version of the wrapped library (pinned exactly in the workspace manifest).
pub const ARRIS_VERSION: &str = "0.9.1";

/// Packs an arris `(index, generation)` pair into a Kubit id.
fn pack(index: u32, generation: u32) -> u64 {
    (u64::from(generation) << 32) | u64::from(index)
}

/// Splits a Kubit id back into `(index, generation)`.
fn unpack(id: u64) -> (u32, u32) {
    // Truncation is the point: the low and high halves.
    #[allow(clippy::cast_possible_truncation)]
    ((id & 0xFFFF_FFFF) as u32, (id >> 32) as u32)
}

fn face_key(f: arris::topo::FaceId) -> FaceId {
    FaceId(pack(f.index(), f.generation()))
}

fn edge_key(e: arris::topo::EdgeId) -> EdgeId {
    EdgeId(pack(e.index(), e.generation()))
}

fn body_key(b: arris::topo::BodyId) -> BodyId {
    BodyId(pack(b.index(), b.generation()))
}

fn arris_face(f: FaceId) -> arris::topo::FaceId {
    let (i, g) = unpack(f.0);
    arris::topo::FaceId::new(i, g)
}

fn arris_edge(e: EdgeId) -> arris::topo::EdgeId {
    let (i, g) = unpack(e.0);
    arris::topo::EdgeId::new(i, g)
}

/// The Kubit entity an arris shape names; shells have no Kubit counterpart.
fn entity_ref(s: Shape) -> Option<EntityRef> {
    match s.id {
        EntityId::Vertex(v) => Some(EntityRef::Vertex(VertexId(pack(v.index(), v.generation())))),
        EntityId::Edge(e) => Some(EntityRef::Edge(edge_key(e))),
        EntityId::Face(f) => Some(EntityRef::Face(face_key(f))),
        EntityId::Body(b) => Some(EntityRef::Body(body_key(b))),
        EntityId::Shell(_) => None,
    }
}

/// Converts arris provenance into a native `OpHistory`. Outputs generated from a
/// role (a primitive's part, a file entity) or from a shell are keyed by `None`.
fn history(p: &Provenance) -> OpHistory {
    let mut h = OpHistory::native();
    for origin in p.origins_recorded() {
        let input = match origin {
            Origin::Entity(s) => entity_ref(s),
            Origin::Role(_) => None,
        };
        for out in p
            .generated_from(origin)
            .iter()
            .copied()
            .filter_map(entity_ref)
        {
            h.add_generated(input, out);
        }
        if let Some(input) = input {
            for out in p
                .modified_from(origin)
                .iter()
                .copied()
                .filter_map(entity_ref)
            {
                h.add_modified(input, out);
            }
        }
    }
    h.deleted.extend(p.deleted().filter_map(entity_ref));
    h
}

fn op_error(op: &'static str, e: OpError) -> KernelError {
    match e {
        OpError::Interrupted(_) => KernelError::Interrupted { op },
        other => KernelError::Failed {
            op,
            message: other.to_string(),
        },
    }
}

fn failed(op: &'static str, e: impl std::fmt::Display) -> KernelError {
    KernelError::Failed {
        op,
        message: e.to_string(),
    }
}

fn point(p: [f64; 3]) -> Point3 {
    Point3::new(p[0], p[1], p[2])
}

fn array(p: Point3) -> [f64; 3] {
    [p.x, p.y, p.z]
}

/// Kubit kernel backed by one arris model.
#[derive(Default)]
pub struct ArrisKernel {
    model: Model,
    /// Live bodies by Kubit id. Ordered, so every walk over them is deterministic.
    bodies: BTreeMap<u64, Body>,
    interrupt: Option<Interrupt>,
}

impl std::fmt::Debug for ArrisKernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArrisKernel")
            .field("bodies", &self.bodies.len())
            .field("interrupt", &self.interrupt.is_some())
            .finish_non_exhaustive()
    }
}

impl ArrisKernel {
    /// An empty kernel with arris' default precision.
    pub fn new() -> Self {
        Self::default()
    }

    fn body(&self, id: BodyId) -> KernelResult<Body> {
        self.bodies
            .get(&id.0)
            .copied()
            .ok_or(KernelError::NotFound {
                kind: "body",
                id: id.0,
            })
    }

    /// Runs `f` with the model and a `Control` that polls the interrupt hook.
    fn with_control<T>(&mut self, f: impl FnOnce(&mut Model, &Control<'_>) -> T) -> T {
        let interrupt = self.interrupt.clone();
        match interrupt {
            Some(hook) => {
                let poll = move || hook();
                let control = Control::poll(&poll);
                f(&mut self.model, &control)
            }
            None => f(&mut self.model, &Control::NONE),
        }
    }

    fn control_ref<T>(&self, f: impl FnOnce(&Model, &Control<'_>) -> T) -> T {
        match self.interrupt.clone() {
            Some(hook) => {
                let poll = move || hook();
                let control = Control::poll(&poll);
                f(&self.model, &control)
            }
            None => f(&self.model, &Control::NONE),
        }
    }

    /// Registers the body an operation returned and wraps it with its history.
    fn register(&mut self, (body, provenance): (Body, Provenance)) -> OpOutput {
        let id = body_key(body.id);
        self.bodies.insert(id.0, body);
        OpOutput {
            body: id,
            history: history(&provenance),
        }
    }

    fn faces_of(&self, ids: &[FaceId], body: Body) -> KernelResult<Vec<arris::topo::Face>> {
        let all = self.model.faces(body).map_err(|e| failed("faces", e))?;
        ids.iter()
            .map(|&f| {
                let target = arris_face(f);
                all.iter()
                    .copied()
                    .find(|h| h.id == target)
                    .ok_or(KernelError::NotFound {
                        kind: "face",
                        id: f.0,
                    })
            })
            .collect()
    }

    fn edges_of(&self, ids: &[EdgeId], body: Body) -> KernelResult<Vec<arris::topo::Edge>> {
        let all = self.model.edges(body).map_err(|e| failed("edges", e))?;
        ids.iter()
            .map(|&e| {
                let target = arris_edge(e);
                all.iter()
                    .copied()
                    .find(|h| h.id == target)
                    .ok_or(KernelError::NotFound {
                        kind: "edge",
                        id: e.0,
                    })
            })
            .collect()
    }

    /// The face's use in the first live body that has it (outward orientation).
    fn face_use(&self, face: FaceId) -> KernelResult<arris::topo::Face> {
        let target = arris_face(face);
        self.bodies
            .values()
            .filter_map(|&b| self.model.faces(b).ok())
            .flatten()
            .find(|h| h.id == target)
            .ok_or(KernelError::NotFound {
                kind: "face",
                id: face.0,
            })
    }
}

impl Kernel for ArrisKernel {
    fn info(&self) -> KernelInfo {
        KernelInfo {
            name: NAME.to_owned(),
            version: ARRIS_VERSION.to_owned(),
            capabilities: Capabilities {
                boolean: true,
                offset_faces: true,
                fillet: true,
                chamfer: true,
                shell: true,
                native_history: true,
                exact_mass: true,
                step: true,
                cancellable: true,
            },
        }
    }

    fn set_interrupt(&mut self, interrupt: Option<Interrupt>) {
        self.interrupt = interrupt;
    }

    fn make_box(&mut self, spec: &BoxSpec) -> KernelResult<OpOutput> {
        let (min, max) = (point(spec.min), point(spec.max));
        let made = self.with_control(|m, c| arris::ops::primitive_box(m, min, max, c));
        Ok(self.register(made.map_err(|e| op_error("make_box", e))?))
    }

    fn make_cylinder(&mut self, spec: &CylinderSpec) -> KernelResult<OpOutput> {
        let [x, y, z] = spec.axis;
        let axis = Axis::new(point(spec.base), Vec3::new(x, y, z)).map_err(|e| {
            KernelError::InvalidInput {
                param: "axis",
                reason: e.to_string(),
            }
        })?;
        let (radius, height) = (spec.radius, spec.height);
        let made =
            self.with_control(|m, c| arris::ops::primitive_cylinder(m, axis, radius, height, c));
        Ok(self.register(made.map_err(|e| op_error("make_cylinder", e))?))
    }

    fn boolean(&mut self, op: BooleanOp, a: BodyId, b: BodyId) -> KernelResult<OpOutput> {
        let (a, b) = (self.body(a)?, self.body(b)?);
        let made = self.with_control(|m, c| match op {
            BooleanOp::Union => arris::ops::fuse(m, a, b, c),
            BooleanOp::Difference => arris::ops::cut(m, a, b, c),
            BooleanOp::Intersection => arris::ops::common(m, a, b, c),
        });
        Ok(self.register(made.map_err(|e| op_error("boolean", e))?))
    }

    fn offset_faces(
        &mut self,
        body: BodyId,
        faces: &[FaceId],
        distance: f64,
    ) -> KernelResult<OpOutput> {
        let b = self.body(body)?;
        let faces = self.faces_of(faces, b)?;
        let made = self.with_control(|m, c| arris::ops::offset_faces(m, b, &faces, distance, c));
        Ok(self.register(made.map_err(|e| op_error("offset_faces", e))?))
    }

    fn fillet(&mut self, body: BodyId, edges: &[EdgeId], radius: f64) -> KernelResult<OpOutput> {
        let b = self.body(body)?;
        let edges = self.edges_of(edges, b)?;
        let made = self.with_control(|m, c| arris::ops::fillet(m, b, &edges, radius, c));
        Ok(self.register(made.map_err(|e| op_error("fillet", e))?))
    }

    fn chamfer(&mut self, body: BodyId, edges: &[EdgeId], distance: f64) -> KernelResult<OpOutput> {
        let b = self.body(body)?;
        let edges = self.edges_of(edges, b)?;
        let made = self.with_control(|m, c| arris::ops::chamfer(m, b, &edges, distance, c));
        Ok(self.register(made.map_err(|e| op_error("chamfer", e))?))
    }

    fn shell(
        &mut self,
        body: BodyId,
        openings: &[FaceId],
        thickness: f64,
    ) -> KernelResult<OpOutput> {
        let b = self.body(body)?;
        let openings = self.faces_of(openings, b)?;
        let made = self.with_control(|m, c| {
            arris::ops::shell(m, b, &openings, thickness, ShellSide::Inward, c)
        });
        Ok(self.register(made.map_err(|e| op_error("shell", e))?))
    }

    fn delete_body(&mut self, body: BodyId) -> KernelResult<()> {
        let b = self.body(body)?;
        self.bodies.remove(&body.0);
        // Bodies share unchanged entities with the bodies they came from: free only
        // what no other live body reaches.
        let keep: Vec<Body> = self.bodies.values().copied().collect();
        self.model
            .discard(b, &keep)
            .map(|_| ())
            .map_err(|e| failed("delete_body", e))
    }

    fn copy_body(&mut self, body: BodyId) -> KernelResult<OpOutput> {
        let b = self.body(body)?;
        let made = self.with_control(|m, c| arris::ops::transform(m, b, &Isometry::identity(), c));
        Ok(self.register(made.map_err(|e| op_error("copy_body", e))?))
    }

    fn faces(&self, body: BodyId) -> KernelResult<Vec<FaceId>> {
        let b = self.body(body)?;
        let faces = self.model.faces(b).map_err(|e| failed("faces", e))?;
        Ok(faces.into_iter().map(|f| face_key(f.id)).collect())
    }

    fn edges(&self, body: BodyId) -> KernelResult<Vec<EdgeId>> {
        let b = self.body(body)?;
        let edges = self.model.edges(b).map_err(|e| failed("edges", e))?;
        Ok(edges.into_iter().map(|e| edge_key(e.id)).collect())
    }

    fn face_info(&self, face: FaceId) -> KernelResult<FaceInfo> {
        let handle = self.face_use(face)?;
        let entity = self
            .model
            .face(handle.id)
            .map_err(|e| failed("face_info", e))?;
        let surface = self
            .model
            .surface(entity.surface())
            .map_err(|e| failed("face_info", e))?;
        use arris::geom::SurfaceKind as K;
        let kind = match surface.kind() {
            K::Plane => SurfaceKind::Plane,
            K::Cylinder => SurfaceKind::Cylinder,
            K::Cone => SurfaceKind::Cone,
            K::Sphere => SurfaceKind::Sphere,
            K::Torus => SurfaceKind::Torus,
            K::EllipticCylinder | K::Nurbs => SurfaceKind::Freeform,
        };
        let normal = match kind {
            SurfaceKind::Plane => {
                let frame =
                    face_frame(&self.model, handle).map_err(|e| op_error("face_info", e))?;
                let z = frame.z();
                Some([z.x, z.y, z.z])
            }
            _ => None,
        };
        // Exact surface integrals over the trimmed face: ∫dA and ∫p dA.
        let integral = |g: fn(Point3) -> f64| {
            face_flux(&self.model, handle.id, |p, n| g(p) * n.norm())
                .map_err(|e| failed("face_info", e))
        };
        let area = integral(|_| 1.0)?;
        if !(area.is_finite() && area > 0.0) {
            return Err(KernelError::InvalidResult {
                op: "face_info",
                reason: format!("face area {area}"),
            });
        }
        let centroid = [
            integral(|p| p.x)? / area,
            integral(|p| p.y)? / area,
            integral(|p| p.z)? / area,
        ];
        Ok(FaceInfo {
            id: face,
            surface: kind,
            normal,
            centroid,
            area,
        })
    }

    fn edge_endpoints(&self, edge: EdgeId) -> KernelResult<[[f64; 3]; 2]> {
        let not_found = |_| KernelError::NotFound {
            kind: "edge",
            id: edge.0,
        };
        let entity = self.model.edge(arris_edge(edge)).map_err(not_found)?;
        let end = |v| {
            self.model
                .vertex(v)
                .map(|v| array(v.point()))
                .map_err(|e| failed("edge_endpoints", e))
        };
        Ok([end(entity.start())?, end(entity.end())?])
    }

    fn edge_faces(&self, body: BodyId, edge: EdgeId) -> KernelResult<[FaceId; 2]> {
        let b = self.body(body)?;
        let not_found = || KernelError::NotFound {
            kind: "edge",
            id: edge.0,
        };
        let uses = self
            .model
            .edge_uses(arris_edge(edge))
            .map_err(|_| not_found())?;
        // `edge_uses` is model-wide, and bodies share unchanged entities with the
        // bodies they came from: keep only the uses by faces of `body`. Within one
        // solid there are exactly two.
        let faces = self.model.faces(b).map_err(|e| failed("edge_faces", e))?;
        let own: Vec<FaceId> = uses
            .iter()
            .filter(|u| faces.iter().any(|f| f.id == u.face))
            .map(|u| face_key(u.face))
            .collect();
        match own.as_slice() {
            [] => Err(not_found()),
            &[f, g] => Ok([f, g]),
            other => Err(KernelError::Failed {
                op: "edge_faces",
                message: format!("edge is used {} time(s) in one body", other.len()),
            }),
        }
    }

    fn mass_properties(&self, body: BodyId) -> KernelResult<MassProperties> {
        let b = self.body(body)?;
        let p = self
            .control_ref(|m, c| arris::ops::measure::mass_properties(m, b, c))
            .map_err(|e| op_error("mass_properties", e))?;
        Ok(MassProperties {
            volume: p.volume,
            area: p.area,
            centroid: array(p.centroid),
            exact: true,
        })
    }

    fn tessellate(&self, body: BodyId, chord: f64) -> KernelResult<TriMesh> {
        let b = self.body(body)?;
        let mesh = self
            .control_ref(|m, c| arris::mesh::tessellate(m, b, chord, c))
            .map_err(|e| failed("tessellate", e))?;
        let triangles = mesh.triangles().to_vec();
        let mut tags: Vec<Option<FaceTag>> = vec![None; triangles.len()];
        for range in mesh.faces() {
            let tag = FaceTag(face_key(range.face).0);
            for slot in tags.get_mut(range.triangles.clone()).into_iter().flatten() {
                *slot = Some(tag);
            }
        }
        let face_tags: Option<Vec<FaceTag>> = tags.into_iter().collect();
        let face_tags = face_tags.ok_or_else(|| KernelError::InvalidResult {
            op: "tessellate",
            reason: "a triangle belongs to no face range".into(),
        })?;
        let positions = mesh.positions();
        let mut edges = Vec::with_capacity(mesh.edges().len());
        for range in mesh.edges() {
            let line: Option<Vec<[f64; 3]>> = mesh
                .edge_polyline(range.edge)
                .unwrap_or_default()
                .iter()
                .map(|&i| positions.get(i as usize).copied())
                .collect();
            let line = line.ok_or_else(|| KernelError::InvalidResult {
                op: "tessellate",
                reason: "an edge polyline indexes a missing position".into(),
            })?;
            edges.push((EdgeTag(edge_key(range.edge).0), line));
        }
        Ok(TriMesh {
            positions: positions.to_vec(),
            triangles,
            face_tags,
            edges,
        })
    }

    fn check(&self, body: BodyId) -> KernelResult<Validity> {
        let b = self.body(body)?;
        let report = check(&self.model, b, Level::Full);
        Ok(Validity {
            ok: report.is_ok(),
            issues: report
                .violations()
                .iter()
                .map(ToString::to_string)
                .collect(),
        })
    }

    fn export_step(&self, bodies: &[BodyId]) -> KernelResult<String> {
        let bodies: Vec<Body> = bodies
            .iter()
            .map(|&b| self.body(b))
            .collect::<KernelResult<_>>()?;
        arris::io::step::write(&self.model, &bodies).map_err(|e| failed("export_step", e))
    }

    fn import_step(&mut self, text: &str) -> KernelResult<Vec<OpOutput>> {
        let read = self
            .with_control(|m, c| arris::io::step::read(m, text, &Default::default(), c))
            .map_err(|e| failed("import_step", e))?;
        let mut refused = Vec::new();
        let mut made = Vec::new();
        for solid in read.solids {
            match solid.result {
                Ok(r) => made.push((r.body, r.provenance)),
                Err(e) => refused.push(e.to_string()),
            }
        }
        if !refused.is_empty() {
            // A partial import would silently lose parts: drop what was read.
            let keep: Vec<Body> = self.bodies.values().copied().collect();
            for (body, _) in &made {
                let _ = self.model.discard(*body, &keep);
            }
            return Err(KernelError::Failed {
                op: "import_step",
                message: format!("{} solid(s) refused: {}", refused.len(), refused.join("; ")),
            });
        }
        Ok(made.into_iter().map(|out| self.register(out)).collect())
    }
}
