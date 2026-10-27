//! The `Kernel` trait every backend implements.

use crate::error::{KernelError, KernelResult};
use crate::history::OpHistory;
use crate::types::{
    BodyId, BooleanOp, BoxSpec, CylinderSpec, EdgeId, FaceId, FaceInfo, KernelInfo, MassProperties,
    Validity,
};
use kubit_mesh::TriMesh;
use std::sync::Arc;

/// Result of a modelling operation: the new body and what happened to the topology.
#[derive(Clone, Debug, PartialEq)]
pub struct OpOutput {
    /// The resulting body (inputs stay alive until `delete_body`).
    pub body: BodyId,
    /// Topological history of the operation.
    pub history: OpHistory,
}

/// Cooperative cancellation: the backend polls it during long operations and
/// stops with `KernelError::Interrupted` when it returns `true`.
pub type Interrupt = Arc<dyn Fn() -> bool + Send + Sync>;

/// A B-rep modelling kernel behind Kubit's facade.
///
/// Rules for implementers (`docs/ARCHITECTURE.md` §2):
/// - lengths in mm, angles in radians, `f64`;
/// - never panic on purpose; return `KernelError`. The [`crate::Guarded`] wrapper
///   still catches panics, validates inputs and results, and fills history;
/// - results must be deterministic for the same sequence of calls;
/// - operations a backend lacks keep the default body, which returns
///   `KernelError::Unsupported` — never emulate them silently.
///
/// The trait is object safe so the engine can hold a `Box<dyn Kernel>`.
pub trait Kernel {
    /// Name, version and capabilities.
    fn info(&self) -> KernelInfo;

    /// Installs (or clears) the cancellation hook.
    fn set_interrupt(&mut self, _interrupt: Option<Interrupt>) {}

    // --- construction -------------------------------------------------------

    /// Axis-aligned box.
    fn make_box(&mut self, spec: &BoxSpec) -> KernelResult<OpOutput>;

    /// Right circular cylinder.
    fn make_cylinder(&mut self, spec: &CylinderSpec) -> KernelResult<OpOutput> {
        let _ = spec;
        Err(self.unsupported("make_cylinder"))
    }

    // --- operations ---------------------------------------------------------

    /// Boolean of two bodies; both inputs stay alive.
    fn boolean(&mut self, op: BooleanOp, a: BodyId, b: BodyId) -> KernelResult<OpOutput> {
        let _ = (op, a, b);
        Err(self.unsupported("boolean"))
    }

    /// Direct push/pull: moves `faces` along their outward normals by `distance`
    /// (negative = inwards), extending or trimming the neighbours.
    fn offset_faces(
        &mut self,
        body: BodyId,
        faces: &[FaceId],
        distance: f64,
    ) -> KernelResult<OpOutput> {
        let _ = (body, faces, distance);
        Err(self.unsupported("offset_faces"))
    }

    /// Constant-radius fillet on `edges`.
    fn fillet(&mut self, body: BodyId, edges: &[EdgeId], radius: f64) -> KernelResult<OpOutput> {
        let _ = (body, edges, radius);
        Err(self.unsupported("fillet"))
    }

    /// Equal-distance chamfer on `edges`.
    fn chamfer(&mut self, body: BodyId, edges: &[EdgeId], distance: f64) -> KernelResult<OpOutput> {
        let _ = (body, edges, distance);
        Err(self.unsupported("chamfer"))
    }

    /// Hollows `body` inwards by `thickness`, removing `openings`.
    fn shell(
        &mut self,
        body: BodyId,
        openings: &[FaceId],
        thickness: f64,
    ) -> KernelResult<OpOutput> {
        let _ = (body, openings, thickness);
        Err(self.unsupported("shell"))
    }

    // Extension points for later phases (sketch profiles, Phase 1): extrude,
    // revolve, sweep, loft, split, move/delete face. They are added here with
    // `Unsupported` defaults so existing backends keep compiling.

    /// Frees a body and its entities.
    fn delete_body(&mut self, body: BodyId) -> KernelResult<()>;

    /// An independent copy of `body`.
    fn copy_body(&mut self, body: BodyId) -> KernelResult<OpOutput> {
        let _ = body;
        Err(self.unsupported("copy_body"))
    }

    // --- queries ------------------------------------------------------------

    /// Faces of `body`, in a deterministic order.
    fn faces(&self, body: BodyId) -> KernelResult<Vec<FaceId>>;

    /// Edges of `body`, in a deterministic order.
    fn edges(&self, body: BodyId) -> KernelResult<Vec<EdgeId>>;

    /// Geometric summary of a face.
    fn face_info(&self, face: FaceId) -> KernelResult<FaceInfo>;

    /// End points of an edge (equal for a closed edge).
    fn edge_endpoints(&self, edge: EdgeId) -> KernelResult<[[f64; 3]; 2]>;

    /// The two faces of `body` that meet at `edge`, in the backend's order (both
    /// the same face for a seam). Callers that need a canonical order sort.
    ///
    /// Scoped to a body because backends may share unchanged entities between a
    /// body and the bodies derived from it: the same edge can border different
    /// faces in each.
    fn edge_faces(&self, body: BodyId, edge: EdgeId) -> KernelResult<[FaceId; 2]> {
        let _ = (body, edge);
        Err(self.unsupported("edge_faces"))
    }

    /// Volume, area and centroid.
    fn mass_properties(&self, body: BodyId) -> KernelResult<MassProperties>;

    /// Triangle mesh with chord tolerance `chord` (mm), tagged by face.
    fn tessellate(&self, body: BodyId, chord: f64) -> KernelResult<TriMesh>;

    /// The backend's own validity check.
    fn check(&self, body: BodyId) -> KernelResult<Validity>;

    // --- exchange -----------------------------------------------------------

    /// STEP AP214/AP242 text of the bodies, in mm.
    fn export_step(&self, bodies: &[BodyId]) -> KernelResult<String> {
        let _ = bodies;
        Err(self.unsupported("export_step"))
    }

    /// Imports every solid of a STEP file.
    fn import_step(&mut self, text: &str) -> KernelResult<Vec<OpOutput>> {
        let _ = text;
        Err(self.unsupported("import_step"))
    }

    /// Helper for the default bodies.
    fn unsupported(&self, op: &'static str) -> KernelError {
        KernelError::Unsupported {
            op,
            backend: self.info().name,
        }
    }
}

/// Forwarding impl so the engine can hold `Guarded<Box<dyn Kernel>>` and pick the
/// backend at run time.
impl<K: Kernel + ?Sized> Kernel for Box<K> {
    fn info(&self) -> KernelInfo {
        (**self).info()
    }
    fn set_interrupt(&mut self, interrupt: Option<Interrupt>) {
        (**self).set_interrupt(interrupt);
    }
    fn make_box(&mut self, spec: &BoxSpec) -> KernelResult<OpOutput> {
        (**self).make_box(spec)
    }
    fn make_cylinder(&mut self, spec: &CylinderSpec) -> KernelResult<OpOutput> {
        (**self).make_cylinder(spec)
    }
    fn boolean(&mut self, op: BooleanOp, a: BodyId, b: BodyId) -> KernelResult<OpOutput> {
        (**self).boolean(op, a, b)
    }
    fn offset_faces(
        &mut self,
        body: BodyId,
        faces: &[FaceId],
        distance: f64,
    ) -> KernelResult<OpOutput> {
        (**self).offset_faces(body, faces, distance)
    }
    fn fillet(&mut self, body: BodyId, edges: &[EdgeId], radius: f64) -> KernelResult<OpOutput> {
        (**self).fillet(body, edges, radius)
    }
    fn chamfer(&mut self, body: BodyId, edges: &[EdgeId], distance: f64) -> KernelResult<OpOutput> {
        (**self).chamfer(body, edges, distance)
    }
    fn shell(
        &mut self,
        body: BodyId,
        openings: &[FaceId],
        thickness: f64,
    ) -> KernelResult<OpOutput> {
        (**self).shell(body, openings, thickness)
    }
    fn delete_body(&mut self, body: BodyId) -> KernelResult<()> {
        (**self).delete_body(body)
    }
    fn copy_body(&mut self, body: BodyId) -> KernelResult<OpOutput> {
        (**self).copy_body(body)
    }
    fn faces(&self, body: BodyId) -> KernelResult<Vec<FaceId>> {
        (**self).faces(body)
    }
    fn edges(&self, body: BodyId) -> KernelResult<Vec<EdgeId>> {
        (**self).edges(body)
    }
    fn face_info(&self, face: FaceId) -> KernelResult<FaceInfo> {
        (**self).face_info(face)
    }
    fn edge_endpoints(&self, edge: EdgeId) -> KernelResult<[[f64; 3]; 2]> {
        (**self).edge_endpoints(edge)
    }
    fn edge_faces(&self, body: BodyId, edge: EdgeId) -> KernelResult<[FaceId; 2]> {
        (**self).edge_faces(body, edge)
    }
    fn mass_properties(&self, body: BodyId) -> KernelResult<MassProperties> {
        (**self).mass_properties(body)
    }
    fn tessellate(&self, body: BodyId, chord: f64) -> KernelResult<TriMesh> {
        (**self).tessellate(body, chord)
    }
    fn check(&self, body: BodyId) -> KernelResult<Validity> {
        (**self).check(body)
    }
    fn export_step(&self, bodies: &[BodyId]) -> KernelResult<String> {
        (**self).export_step(bodies)
    }
    fn import_step(&mut self, text: &str) -> KernelResult<Vec<OpOutput>> {
        (**self).import_step(text)
    }
}
