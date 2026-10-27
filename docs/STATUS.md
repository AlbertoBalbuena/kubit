# Estado — Fase 0

Actualizado: 2026-10-27 03:40 CST. Coordina la sesión principal; las sesiones paralelas trabajan en worktrees y ramas `claude/*`, y la principal integra en `main`.

## Hecho (en `main`)
- **Spike de kernels** con 4 backends (monstertruck, truck, solidcore, arris), corpus de 16 pruebas con oráculos, STEP de NIST y sondas wasm. Resultados en `spikes/kernel/results/`. **ADR-0002** (aceptado): backend inicial **arris 0.9.1**.
- **ADR-0003** (propuesto): fachada del kernel, guard, nombres persistentes v0 y formato `.kubit` v1.
- Workspace con capas, `cargo xtask ci` (fmt, clippy, test, layers, wasm, deny), CI en 3 sistemas, archivos de comunidad.
- `kubit-math`, `kubit-mesh`, `kubit-kernel-api` (+ `Guarded`, `OpHistory`), **`kubit-kernel-arris`**, `kubit-doc` + `.kubit` v1, `kubit-engine` (13 comandos), `kubit-io` (STL), `kubit-render` (wgpu sin ventana, picking, snapshots), `kubit-mcp` (tools generadas del registro).
- `kubit-cli`: `run`, `export`, `exec`, `commands`, **`mcp`**, **`snapshot`**. `kubit-cli run examples/box.json --out box.step` funciona sobre arris; la prueba de aceptación reimporta el STEP y conserva los 300 000 mm³.
- `kubit-ui` + `apps/kubit`: dock, barra de herramientas del registro, viewport con picking de caras, caja, push/pull, deshacer/rehacer, tema claro/oscuro, i18n es/en, `--screenshot`.

## Pendiente de verificar por el mantenedor
- Navegación con ratón en ventana real (órbita, pan, zoom, selección). Solo se verificó con capturas fuera de pantalla.
- Revisión y aprobación de ADR-0003 (ADR-0002 aceptado el 2026-10-27).

## Deuda conocida
- Los mensajes de error del motor se muestran en inglés dentro de un prefijo traducido.
- La fusión de caras coplanares tras booleanas, el fillet de radio variable y el chaflán d-ángulo quedan para implementarse sobre arris (ADR-0002).
- Los commits anteriores al 26-oct 23:55 no llevan `Signed-off-by` (DCO).

## Bloqueos y riesgos
- El guard no atrapa panics en `wasm32-unknown-unknown` (allí abortan).
- Booleanas con contacto tangente o por arista fallan limpiamente en arris (ADR-0002, riesgos).
