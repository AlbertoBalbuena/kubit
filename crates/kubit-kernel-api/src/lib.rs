//! Kernel facade (L1): the only way the rest of Kubit talks to a B-rep kernel.
//!
//! - [`Kernel`]: the trait every backend (`kubit-kernel-*`) implements.
//! - [`Guarded`]: wraps a backend; validates inputs and results, catches panics,
//!   and reconstructs [`OpHistory`] when the backend does not report it.
//! - [`OpHistory`]: generated / modified / deleted entities of every operation.
//!
//! Units: mm and radians in `f64`. Layer and rules: `docs/ARCHITECTURE.md` §2.

mod error;
mod guard;
mod history;
mod kernel;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod types;

pub use error::{KernelError, KernelResult};
pub use guard::{CHORD_RANGE, Guarded, check_direction, check_length, check_point};
pub use history::{FaceRole, HistorySource, MatchTolerance, OpHistory, reconstruct_faces};
pub use kernel::{Interrupt, Kernel, OpOutput};
pub use kubit_mesh::TriMesh;
pub use types::*;

#[cfg(test)]
mod guard_tests;
