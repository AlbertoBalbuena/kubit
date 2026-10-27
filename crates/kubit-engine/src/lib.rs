//! Command registry, session, transactions and undo/redo (L4).
//!
//! Everything a user, a script or an AI agent can do is a [`CommandSpec`] in the
//! [`Registry`], executed through [`Session::execute`] with JSON parameters
//! validated against the command's JSON Schema. Programmatic execution never
//! opens dialogs. Layer and rules: `docs/ARCHITECTURE.md` §4.

mod commands;
mod error;
mod model;
mod session;
mod spec;

pub use error::CommandError;
pub use model::{
    BodyState, BoolKind, BooleanParams, BoxParams, ChamferParams, CylinderParams, ExtrudeOp,
    ExtrudeParams, FeatureStatus, FilletParams, Model, PushPullParams, ShellParams,
};
pub use session::{Event, KernelFactory, Selection, Session, UNDO_LIMIT};
pub use spec::{CommandOutcome, CommandSpec, Enablement, Registry, RunFn, always, schema_of};

#[cfg(test)]
mod tests;
