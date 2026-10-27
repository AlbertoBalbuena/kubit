//! Command errors. All recoverable; none opens a dialog.

use kubit_kernel_api::KernelError;
use thiserror::Error;

/// Error returned by `Session::execute`.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CommandError {
    /// No command with that id.
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    /// Parameters do not match the command's schema or bounds.
    #[error("invalid parameters for `{command}`: {message}")]
    InvalidParams {
        /// Command id.
        command: &'static str,
        /// What is wrong.
        message: String,
    },
    /// The command is disabled in the current state.
    #[error("`{command}` is not available now: {reason}")]
    Disabled {
        /// Command id.
        command: &'static str,
        /// Why.
        reason: &'static str,
    },
    /// A face / body reference does not resolve.
    #[error("reference not found: {0}")]
    Reference(String),
    /// The kernel failed (already guarded: never a crash).
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// A feature in the document has an unknown command (newer file?).
    #[error("feature {feature} uses unknown command `{command}`")]
    UnknownFeature {
        /// Feature id.
        feature: String,
        /// Command id.
        command: String,
    },
    /// File system error.
    #[error("file error: {0}")]
    Io(String),
    /// File format error.
    #[error("format error: {0}")]
    Format(String),
}
