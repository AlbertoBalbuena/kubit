//! MCP server generated from the command registry (L4).
//!
//! Layer and rules: `docs/ARCHITECTURE.md`.
//!
//! Every registered `CommandSpec` becomes an MCP tool; nothing here knows
//! about individual commands (CLAUDE.md rule 1). The engine `Session` is
//! `!Send`, so it lives on a dedicated actor thread ([`SessionHandle`]) and
//! the async MCP handler talks to it over channels.
//!
//! # Tool names
//!
//! The MCP spec allows `.` in tool names, but the common LLM tool APIs
//! restrict names to `[A-Za-z0-9_-]{1,64}`. Command ids therefore map `.` to
//! `_` (`solid.box` → `solid_box`); the reverse map is kept in the catalog and
//! a collision after mapping is a startup error.
//!
//! # Modes
//!
//! * `read_only`: tools whose command `mutates` the document are neither
//!   listed nor callable.
//! * `allow_files`: tools whose command touches the file system
//!   (`file_access`) are hidden and refused unless this is set.
//!
//! Resources `kubit://document` (document JSON) and `kubit://model`
//! (`doc.inspect` data) are always readable.

mod actor;
mod server;

pub use actor::{Catalog, SessionHandle, ToolEntry};
pub use server::{KubitServer, serve, serve_stdio};

/// Which commands the server exposes. Both default to the restrictive value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServerOptions {
    /// Hide and refuse commands that mutate the document.
    pub read_only: bool,
    /// Expose commands that read or write files on the host.
    pub allow_files: bool,
}

impl ServerOptions {
    /// Whether a command with these flags is exposed under these options.
    pub fn exposes(&self, mutates: bool, file_access: bool) -> bool {
        !(self.read_only && mutates) && (self.allow_files || !file_access)
    }
}

/// Errors starting or running the MCP server.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// The session actor could not start or has stopped.
    #[error("session actor unavailable: {0}")]
    Actor(String),
    /// Two command ids map to the same tool name.
    #[error("tool name collision: `{0}` and `{1}` both map to `{2}`")]
    NameCollision(String, String, String),
    /// The MCP handshake or transport failed.
    #[error("MCP transport error: {0}")]
    Transport(String),
}
