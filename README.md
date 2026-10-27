# Kubit

**Diseña lo que sí se puede fabricar.**

Kubit es una herramienta open source de modelado 3D directo y paramétrico escrita en Rust. Combina la rapidez de un modelador directo con la precisión de un CAD paramétrico, y valida en vivo que lo diseñado se pueda fabricar con materiales y procesos reales: tableros, madera sólida, laminados, piedra, cuarzo, solid surface, acero, aluminio, vidrio, impresión 3D, tapicería y más.

> Estado: en definición (Fase 0). Todavía no hay código.

## Documentación
- [Visión](docs/VISION.md): qué es Kubit, para quién y por qué.
- [Arquitectura](docs/ARCHITECTURE.md): crates, capas, kernel, comandos, MCP y formato de archivo.
- [Modelo de fabricación](docs/MANUFACTURING-MODEL.md): materiales, stock, procesos y reglas DFM.
- [Roadmap](docs/ROADMAP.md): fases y criterios de salida.
- [Decisiones (ADR)](docs/adr/).
- Investigación: [materiales y DFM](docs/research/materiales-dfm.md) · [tecnología Rust para CAD](docs/research/tecnologia-rust-cad.md).

## Building
Requires a stable Rust toolchain (`rustup`), the wasm target (`rustup target add wasm32-unknown-unknown`)
and [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) (`cargo install --locked cargo-deny`).

```bash
cargo xtask ci       # fmt, clippy, tests, layers, wasm, deny (what CI runs)
cargo xtask layers   # dependency-layer rules from layers.toml
cargo xtask wasm     # build the wasm-capable crates for wasm32-unknown-unknown
cargo xtask deny     # licenses, bans, advisories, sources
```

Contributions need a DCO sign-off (`git commit -s`); see [CONTRIBUTING.md](CONTRIBUTING.md).

## Licencia
- Código: Apache-2.0.
- Datos del catálogo de materiales: CC-BY-4.0.

Las validaciones de fabricación y simulación de Kubit son **asistencia de diseño, no certificación** de ingeniería.
