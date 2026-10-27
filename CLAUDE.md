# CLAUDE.md — Kubit

Kubit es un CAD 3D directo y paramétrico, open source, en Rust, con validación de fabricación real (DFM) y MCP nativo. Tiene la rapidez de Shapr3D y la profundidad de Fusion 360, y además sabe de materiales y procesos.

## Lee antes de trabajar
1. `docs/VISION.md`: qué estamos construyendo y qué NO.
2. `docs/ARCHITECTURE.md`: capas, crates y reglas. **Fuente de verdad técnica.**
3. `docs/ROADMAP.md`: la fase actual y sus criterios de salida. Trabaja solo dentro de la fase actual.
4. `docs/MANUFACTURING-MODEL.md`: cuando toques materiales, fabricación o DFM.
5. `docs/adr/`: decisiones tomadas. No las contradigas sin proponer un ADR nuevo.
6. `docs/research/`: evidencia, versiones de crates y licencias. Verifica versiones actuales antes de agregar dependencias.

## Reglas de oro
1. **Todo es un comando.** El comportamiento visible es un `CommandSpec` en `kubit-engine`. UI, CLI, scripts y MCP llaman los mismos comandos. La ejecución programática nunca abre diálogos.
2. **Respeta las capas.** Un crate solo depende de capas inferiores. Nada bajo L5 usa egui, eframe, winit ni rfd; wgpu solo en `kubit-render` y L5. Todo bajo L4 compila a `wasm32-unknown-unknown`. `cargo xtask layers` debe pasar.
3. **Kernel aislado.** Solo los crates `kubit-kernel-*` tocan la librería del kernel. Cada llamada pasa por el guard (`catch_unwind` → `KernelError`), y cada operación devuelve `OpHistory`.
4. **Cero panics en producción.** Clippy prohíbe `unwrap`, `expect`, `panic`, `todo`, `unimplemented` y `unreachable` en crates de producción (en tests sí se permiten). `unsafe_code = "forbid"`. Errores con `Result` y `thiserror`. Los números de entrada deben ser finitos y estar acotados.
5. **Unidades.** Internamente se trabaja en milímetros y radianes, en `f64`. La conversión a mm o pulgadas con fracciones ocurre solo en UI e IO.
6. **Determinismo.** El mismo documento produce la misma geometría. No se itera sobre `HashMap` en caminos que afectan la geometría o el archivo (usa `BTreeMap`/`IndexMap`). Los IDs deben ser estables.
7. **La fabricación es datos.** Materiales, stock, procesos y reglas viven en el catálogo (TOML + JSON Schema), nunca hardcodeados. En Rust solo van los chequeos genéricos parametrizados.
8. **Asistencia, no certificación.** Todo resultado DFM o de simulación muestra su fuente y su nivel de confianza.
9. **Licencias.** Solo se permiten dependencias Apache-2.0, MIT, BSD, ISC o Zlib; MPL-2.0 requiere revisión. **Prohibido** GPL, LGPL y AGPL en el core (ojo: `evalexpr` ≥13 es AGPL, `slvs` es GPL). No leas código de FreeCAD, SolveSpace u OCCT para escribir código del core. El código adaptado de SolidCraft, truck, monstertruck o solidcore se registra en `ATTRIBUTION.md` con fuente, commit y licencia. `cargo deny check` debe pasar.
10. **Clean room.** Nunca uses ni consultes código de Evita Studio.
11. **Idioma.** Código, identificadores, comentarios, mensajes de commit y textos de PR en inglés. Docs en `docs/` en español. Textos de UI vía i18n (es, en).

## Forma de trabajar
- Usa **Plan Mode** para cualquier cambio que toque más de un archivo o una API pública. Presenta el plan y espera aprobación.
- **Pregunta antes** de cambiar el formato `.kubit`, el esquema del catálogo, la API de `kubit-kernel-api`, el `CommandSpec` o un ADR.
- Trabaja en incrementos pequeños y verificables. Haz un commit por unidad lógica, con mensajes convencionales (`feat(engine): …`).
- Cada feature lleva pruebas unitarias. La geometría lleva pruebas **oráculo** (valores analíticos: volumen, área, centro de masa). Cada bug lleva su test de regresión. Las entradas hostiles se prueban con proptest y el criterio es cero panics.
- Verifica la UI con capturas (`kubit-cli snapshot` o el comando `ui.screenshot`) antes de dar algo por terminado.
- Antes de terminar una tarea: `cargo xtask ci` en verde (fmt, clippy `-D warnings`, tests, layers, wasm, deny).
- Al cerrar una tarea, actualiza las casillas de `docs/ROADMAP.md`. Si tomaste una decisión de arquitectura, escribe un ADR (`docs/adr/NNNN-titulo.md`).
- Si algo del plan resulta inviable (el kernel no soporta una operación, por ejemplo), dilo con evidencia y propone alternativas. No lo simules.
- El mantenedor es diseñador industrial y desarrollador. Explica brevemente las decisiones de Rust que no sean obvias (lifetimes, traits, concurrencia) en el resumen de cada tarea.

## Comandos (se crean en la Fase 0)
```bash
cargo xtask ci               # todo lo que corre CI
cargo run -p kubit           # app de escritorio
cargo run -p kubit-cli -- mcp            # servidor MCP por stdio
cargo run -p kubit-cli -- run examples/box.json --out box.step
```
