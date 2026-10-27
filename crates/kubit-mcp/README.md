# kubit-mcp

MCP server generated from the engine command registry (rmcp 3.5.1).

**Done:** session actor thread (`SessionHandle`), tool generation (`.` → `_`, reverse map, collision check),
`ServerOptions { read_only, allow_files }`, resources `kubit://document` / `kubit://model`,
`serve` / `serve_stdio`. Builds and passes clippy `-D warnings`.

**Next:** protocol integration tests over an in-memory duplex (box → push/pull → undo/redo, read-only mode,
invalid call → `isError`, agent author in timeline, hostile args); then `cargo xtask ci` (incl. deny for rmcp).

**Note:** `serve*` take `FnOnce() -> KernelFactory + Send` because `KernelFactory` is `!Send` and must be built on the actor thread.
