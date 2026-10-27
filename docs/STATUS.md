# Estado — Fase 0

Actualizado: 2026-10-26 23:55 CST (pausa hasta las 03:00). Coordina la sesión principal; cada sesión paralela trabaja en su worktree y su rama `claude/*`, y la principal integra en `main`.

## Hecho (en `main`)
- Repositorio en GitHub (`AlbertoBalbuena/kubit`), workspace de producto con capas (`layers.toml`) y lints que prohíben `unsafe`, `unwrap`, `expect`, `panic`…
- `cargo xtask` (`ci`, `layers`, `wasm`, `deny`), `deny.toml`, CI de GitHub Actions (macOS, Windows, Linux + DCO en PRs), LICENSE, NOTICE, ATTRIBUTION, CONTRIBUTING (DCO), CODE_OF_CONDUCT, SECURITY. **`cargo xtask ci` en verde en local.**
- `kubit-math`: unidades (mm/in con fracciones), tolerancias, parser de expresiones propio con análisis dimensional y detección de ciclos.
- `kubit-mesh`: malla con etiqueta de cara, volumen, área y watertightness.
- `kubit-kernel-api`: trait `Kernel`, `Guarded` (validación de entradas, captura de panics con envenenamiento, validación de resultados, reconstrucción de historia), `OpHistory`, y `testing::BoxKernel` (kernel exacto de cajas para tests).
- `kubit-doc`: documento v0 (parámetros + timeline con autor), nombres persistentes v0 (`F1:+Z`, `F3:g2`, `F1:+Z/1`) y contenedor `.kubit` v1 determinista. Round-trip probado byte a byte.
- `kubit-engine`: registro de `CommandSpec` con JSON Schema, `Session::execute`, transacciones, undo/redo, replay al abrir y 13 comandos de Fase 0.
- `kubit-io`: STL binario/ASCII.
- `kubit-cli`: `run`, `export`, `exec`, `commands`. `kubit-cli run examples/box.json --out box.kubit --out box.stl` funciona. STEP espera al backend real.
- Spike de kernels: harness, corpus de 16 pruebas con oráculos analíticos, runner por subproceso, corpus STEP de NIST (restaurable con `fixtures/step/fetch-nist.sh`).

## En curso (sesiones pausadas; retoman a las 03:00)
| Sesión | Rama | Alcance |
|---|---|---|
| adapter-arris | `claude/spike-arris-*` | `spikes/kernel/backends/arris`, `wasm/arris` |
| adapter-monstertruck | `claude/spike-monstertruck-*` | `backends/monstertruck`, `wasm/monstertruck` |
| adapter-truck | `claude/spike-truck-*` | `backends/truck`, `wasm/truck` |
| adapter-solidcore | `claude/spike-solidcore-*` | `backends/solidcore`, `wasm/solidcore` |
| kubit-render | `claude/kubit-render-*` | `crates/kubit-render` |
| kubit-mcp | `claude/kubit-mcp-*` | `crates/kubit-mcp` |

## Siguiente
1. Integrar los 4 adaptadores, correr `spike all` completo y las sondas wasm → `results/`.
2. Escribir **ADR-0002** con la tabla 16 × 4 y la recomendación; crear `kubit-kernel-<elegido>`; conectar CLI/engine al backend real (STEP).
3. Integrar render y MCP; `kubit-cli mcp` y `snapshot`.
4. `kubit-ui` (egui + egui_dock + viewport) y `apps/kubit`: caja, push/pull, undo/redo desde la UI; verificación con captura.
5. Pruebas de salida de Fase 0: STEP re-import conserva volumen; casillas del ROADMAP.

## Bloqueos y riesgos
- El guard no puede atrapar panics en `wasm32-unknown-unknown` (allí los panics abortan).
- Los commits anteriores a esta pausa no llevan `Signed-off-by` (DCO); desde ahora sí.
- La ventana de autorización de push/merge a `main` es de 8 horas a partir del 26-oct 23:20 CST.
