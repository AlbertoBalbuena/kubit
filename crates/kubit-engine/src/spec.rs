//! `CommandSpec` and the registry: the single source of truth for behaviour
//! (CLAUDE.md rule 1). UI, CLI, scripts and MCP all call `Session::execute`.

use crate::error::CommandError;
use crate::session::Session;
use indexmap::IndexMap;
use serde::Serialize;

/// Whether a command can run now, and why not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Enablement {
    /// Can run.
    Enabled,
    /// Cannot run; the reason is shown as a tooltip / returned to MCP.
    Disabled(&'static str),
}

/// What a command returns: a one-line summary and structured data.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CommandOutcome {
    /// Short English summary (CLI prints it; MCP returns it as text).
    pub summary: String,
    /// Structured result (MCP `structuredContent`, CLI `--json`).
    pub data: serde_json::Value,
}

impl CommandOutcome {
    /// Summary-only outcome.
    pub fn text(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            data: serde_json::Value::Null,
        }
    }
}

/// Signature of a command implementation.
pub type RunFn = fn(&mut Session, serde_json::Value) -> Result<CommandOutcome, CommandError>;

/// One registered command.
#[derive(Clone, Copy)]
pub struct CommandSpec {
    /// `<area>.<thing>[.<variant>]`, e.g. `solid.box`.
    pub id: &'static str,
    /// i18n key of the label, e.g. `cmd.solid.box`; the UI translates it.
    pub label: &'static str,
    /// English description for CLI help and MCP tool descriptions.
    pub description: &'static str,
    /// Icon name in the UI icon set.
    pub icon: Option<&'static str>,
    /// Keyboard shortcut, e.g. `"Ctrl+Z"` (UI maps Ctrl to Cmd on macOS).
    pub shortcut: Option<&'static str>,
    /// Toolbar group, if it appears in the toolbar.
    pub toolbar_group: Option<&'static str>,
    /// Changes the document or the session (MCP read-only mode hides these).
    pub mutates: bool,
    /// Reads or writes files (MCP asks for confirmation).
    pub file_access: bool,
    /// JSON Schema of the parameters.
    pub params_schema: fn() -> serde_json::Value,
    /// Whether it can run in the current session state.
    pub enabled: fn(&Session) -> Enablement,
    /// The implementation. Never opens dialogs.
    pub run: RunFn,
}

impl std::fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandSpec")
            .field("id", &self.id)
            .field("mutates", &self.mutates)
            .finish_non_exhaustive()
    }
}

/// Always enabled.
pub fn always(_: &Session) -> Enablement {
    Enablement::Enabled
}

/// Ordered registry of commands.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    commands: IndexMap<&'static str, CommandSpec>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a command; a later registration with the same id replaces it.
    pub fn register(&mut self, spec: CommandSpec) {
        self.commands.insert(spec.id, spec);
    }

    /// Looks a command up.
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.commands.get(id)
    }

    /// Every command, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &CommandSpec> {
        self.commands.values()
    }

    /// Number of commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether it is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// JSON Schema of `T` as a value (helper for `params_schema`).
pub fn schema_of<T: schemars::JsonSchema>() -> serde_json::Value {
    schemars::schema_for!(T).to_value()
}
