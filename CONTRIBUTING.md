# Contributing / Contribuir

## English
Thanks for helping. Read `CLAUDE.md` and `docs/` (vision, architecture, roadmap) first; work only
inside the current roadmap phase.

- **DCO**: sign every commit with `git commit -s` (adds `Signed-off-by:`). CI rejects PRs with unsigned
  commits. Code is Apache-2.0; catalog data is CC-BY-4.0.
- **Before opening a PR** run `cargo xtask ci` (fmt, clippy `-D warnings`, tests, layers, wasm, deny).
- **Golden rules** (details in `CLAUDE.md`):
  1. Everything is a command (`CommandSpec` in `kubit-engine`); UI, CLI and MCP call the same ones.
  2. Respect the layers (`cargo xtask layers`); the kernel is touched only by `kubit-kernel-*`.
  3. No `unwrap`/`expect`/`panic` in production code; errors via `Result` and `thiserror`.
  4. Millimetres and radians internally; deterministic output (no `HashMap` iteration in geometry).
  5. Manufacturing knowledge is catalog data (TOML + JSON Schema), not Rust constants.
  6. Every feature has tests; geometry has analytic oracle tests; every bug has a regression test.
  7. Code, comments, commits and PRs in English; `docs/` in Spanish; UI text via i18n.
- **Licenses**: only Apache-2.0, MIT, BSD, ISC, Zlib dependencies. No GPL/LGPL/AGPL in the core.
- **Clean room**: do not read or copy code from FreeCAD, SolveSpace, OCCT or Evita Studio when
  writing core code. Code adapted from permitted sources must be recorded in `ATTRIBUTION.md`.
- Use conventional commits (`feat(engine): ...`). Ask first before changing the `.kubit` format,
  the catalog schema, `kubit-kernel-api`, `CommandSpec` or an ADR.

## Español
Lee `CLAUDE.md` y `docs/` antes de empezar y trabaja solo dentro de la fase actual del roadmap.

- **DCO**: firma cada commit con `git commit -s`. CI rechaza PRs con commits sin firma.
- **Antes de un PR**: `cargo xtask ci` en verde.
- **Reglas de oro**: todo es un comando; respeta las capas; cero panics en producción; mm y radianes
  internos y resultados deterministas; la fabricación son datos del catálogo; pruebas en cada
  cambio (oráculo en geometría, regresión en bugs); código en inglés, docs en español.
- **Licencias**: solo Apache-2.0, MIT, BSD, ISC, Zlib; nada de GPL/LGPL/AGPL en el core.
- **Clean room**: no leas ni copies código de FreeCAD, SolveSpace, OCCT ni Evita Studio para el core.
  El código adaptado de fuentes permitidas se registra en `ATTRIBUTION.md`.
