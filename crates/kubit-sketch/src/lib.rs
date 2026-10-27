//! 2D sketches and Kubit's own geometric constraint solver (L1).
//!
//! A [`Sketch`] holds points, lines, circles and arcs on a plane, and the
//! constraints and dimensions between them. [`solve`] moves the geometry to satisfy
//! the constraints (Levenberg-Marquardt per connected component, exact Jacobians by
//! forward-mode duals) and reports what is still free, what is redundant and what
//! conflicts. [`drag`] moves a point with minimal perturbation of the rest.
//! [`profiles`] finds the closed regions an extrusion can use.
//!
//! No UI, kernel or document types: the engine stores sketches in its features and
//! turns regions into kernel profiles. Design: ADR-0005.

pub mod dual;
pub mod equations;
pub mod model;
pub mod profile;
pub mod solve;
pub mod text;

pub use model::{
    Constraint, ConstraintId, ConstraintRecord, Entity, EntityId, EntityRecord, MAX_COORD, Sketch,
    SketchError,
};
pub use profile::{Loop, LoopShape, Region, RegionId, Seg, profiles};
pub use solve::{Freedom, SolveOptions, SolveReport, drag, solve};
