//! The single error type of the facade. Every failure is recoverable.

use thiserror::Error;

/// Error returned by every kernel call.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum KernelError {
    /// A parameter was rejected before reaching the backend.
    #[error("invalid input `{param}`: {reason}")]
    InvalidInput {
        /// Parameter name.
        param: &'static str,
        /// Why it was rejected.
        reason: String,
    },
    /// The backend does not implement this operation.
    #[error("operation `{op}` is not supported by the {backend} kernel")]
    Unsupported {
        /// Operation name.
        op: &'static str,
        /// Backend name.
        backend: String,
    },
    /// An id does not name a live entity.
    #[error("unknown {kind} id {id}")]
    NotFound {
        /// Entity kind.
        kind: &'static str,
        /// The id.
        id: u64,
    },
    /// The backend returned an error.
    #[error("{op} failed: {message}")]
    Failed {
        /// Operation name.
        op: &'static str,
        /// Backend message.
        message: String,
    },
    /// The backend panicked; the guard caught it and poisoned the kernel.
    #[error("{op}: kernel panic caught by the guard: {message}")]
    Panicked {
        /// Operation name.
        op: &'static str,
        /// Panic payload.
        message: String,
    },
    /// A previous panic left the kernel in an unknown state; rebuild it.
    #[error("the kernel is poisoned by an earlier panic; rebuild it from the document")]
    Poisoned,
    /// The operation produced a result that fails validation.
    #[error("{op} produced an invalid result: {reason}")]
    InvalidResult {
        /// Operation name.
        op: &'static str,
        /// Validation failure.
        reason: String,
    },
    /// The operation was interrupted (budget or user cancel).
    #[error("{op} was interrupted")]
    Interrupted {
        /// Operation name.
        op: &'static str,
    },
}

/// Result alias of the facade.
pub type KernelResult<T> = Result<T, KernelError>;
