//! Ids, inputs and query results shared by every backend.

use serde::{Deserialize, Serialize};

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        ///
        /// Kernel-local and only stable inside one kernel instance; persistent
        /// names live in `kubit-doc`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub u64);
    };
}

id!(
    /// A solid body.
    BodyId
);
id!(
    /// A B-rep face.
    FaceId
);
id!(
    /// A B-rep edge.
    EdgeId
);
id!(
    /// A B-rep vertex.
    VertexId
);

/// Any topological entity, for history records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityRef {
    /// A body.
    Body(BodyId),
    /// A face.
    Face(FaceId),
    /// An edge.
    Edge(EdgeId),
    /// A vertex.
    Vertex(VertexId),
}

/// Largest absolute coordinate or length accepted by the facade, in mm (1 km).
pub const MAX_LENGTH: f64 = 1.0e6;
/// Smallest positive length accepted by the facade, in mm.
pub const MIN_LENGTH: f64 = 1.0e-6;

/// Axis-aligned box from `min` to `max` (mm).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoxSpec {
    /// Minimum corner.
    pub min: [f64; 3],
    /// Maximum corner; every component strictly greater than `min`.
    pub max: [f64; 3],
}

/// Right circular cylinder: base centre, unit axis, radius and height (mm).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CylinderSpec {
    /// Centre of the base disc.
    pub base: [f64; 3],
    /// Axis direction; normalised by the guard.
    pub axis: [f64; 3],
    /// Radius, mm.
    pub radius: f64,
    /// Height along the axis, mm.
    pub height: f64,
}

/// Boolean operation kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOp {
    /// A ∪ B.
    Union,
    /// A − B.
    Difference,
    /// A ∩ B.
    Intersection,
}

/// Surface family of a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    /// Plane.
    Plane,
    /// Circular cylinder.
    Cylinder,
    /// Circular cone.
    Cone,
    /// Sphere.
    Sphere,
    /// Torus.
    Torus,
    /// NURBS / B-spline or anything else.
    Freeform,
}

/// Geometric summary of a face, used for picking, history matching and DFM.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceInfo {
    /// The face.
    pub id: FaceId,
    /// Surface family.
    pub surface: SurfaceKind,
    /// Outward unit normal for planar faces.
    pub normal: Option<[f64; 3]>,
    /// Area centroid, mm.
    pub centroid: [f64; 3],
    /// Area, mm².
    pub area: f64,
}

/// Mass properties at unit density.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MassProperties {
    /// Volume, mm³.
    pub volume: f64,
    /// Surface area, mm².
    pub area: f64,
    /// Volume centroid, mm.
    pub centroid: [f64; 3],
    /// Whether the numbers are exact B-rep integrals or come from a mesh.
    pub exact: bool,
}

/// Result of the kernel's own validity checker.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Validity {
    /// No violation found.
    pub ok: bool,
    /// Human-readable violations (English; for logs and bug reports).
    pub issues: Vec<String>,
}

/// What a backend can do. Missing operations return `KernelError::Unsupported`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Booleans.
    pub boolean: bool,
    /// Direct face offset / push-pull.
    pub offset_faces: bool,
    /// Constant-radius fillet.
    pub fillet: bool,
    /// Equal-distance chamfer.
    pub chamfer: bool,
    /// Shell with openings.
    pub shell: bool,
    /// The backend reports generated/modified/deleted itself.
    pub native_history: bool,
    /// Exact mass properties.
    pub exact_mass: bool,
    /// STEP read and write.
    pub step: bool,
    /// Honours `Kernel::set_interrupt`.
    pub cancellable: bool,
}

/// Identity of a backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelInfo {
    /// Backend name, e.g. `"arris"`.
    pub name: String,
    /// Library version it wraps.
    pub version: String,
    /// Supported operations.
    pub capabilities: Capabilities,
}
