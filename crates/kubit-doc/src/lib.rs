//! Document model and the `.kubit` file format (L2).
//!
//! The document is parameters plus a timeline of features (registered commands
//! with their parameters). Geometry is derived by replaying it through the
//! kernel, so the file stays small, diffable and deterministic. Layer and rules:
//! `docs/ARCHITECTURE.md` §3 and §9.

mod document;
mod format;
mod naming;

pub use document::{
    Author, Document, DocumentError, FORMAT_VERSION, Feature, FeatureId, LengthDisplay, Parameter,
    Tolerances,
};
pub use format::{
    CONTAINER, FormatError, MAX_ENTRIES, MAX_ENTRY_BYTES, Manifest, from_json, from_kubit, to_json,
    to_kubit,
};
pub use naming::{NameParseError, TopoName};
