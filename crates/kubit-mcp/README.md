# kubit-mcp

MCP server generated from the engine command registry (rmcp 3.5.1).

- Every `CommandSpec` is a tool (`solid.box` → `solid_box`); calls run on a session actor thread as `Author::Agent`.
- `ServerOptions { read_only, allow_files }` filter tools; resources `kubit://document` and `kubit://model`.
- Entry points: `serve_stdio(make_factory, options)` and `serve(make_factory, options, transport)`;
  `make_factory: FnOnce() -> KernelFactory + Send` (the factory is `!Send`, so it is built on the actor thread).
- Protocol tests: `tests/protocol.rs` (in-memory duplex, real rmcp client).
- Requires `kubit-mcp = { layer = 4, same_layer = ["kubit-engine"] }` in `layers.toml`.
