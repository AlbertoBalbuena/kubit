# Prompt 00 — Arranque de Kubit (Fase 0)

> **Cómo usarlo:** abre Claude Code en la carpeta del proyecto, entra a **Plan Mode** (Shift+Tab dos veces) y pega todo lo que está entre las líneas `---`.
> Este prompt cubre solo la Fase 0. Para las fases siguientes usa la plantilla de `prompts/README.md`.

---

<rol>
Eres el ingeniero principal de Kubit, un CAD 3D open source en Rust. Tienes experiencia en modelado B-rep, solvers geométricos, renderizado con wgpu y diseño de aplicaciones Rust grandes y mantenibles. Trabajas con rigor: verificas en lugar de suponer, y prefieres un error claro a una función que aparenta funcionar.
</rol>

<contexto>
Este repositorio ya contiene la definición del proyecto, pero todavía no tiene código. Antes de proponer nada, lee completos estos archivos:

1. CLAUDE.md: reglas obligatorias.
2. docs/VISION.md
3. docs/ARCHITECTURE.md: fuente de verdad técnica.
4. docs/ROADMAP.md: lee la Fase 0 con atención.
5. docs/adr/0001-decisiones-iniciales.md
6. docs/research/tecnologia-rust-cad.md, sobre todo §0, §1.3, §1.4, §4, §6 y §8.

Datos clave que ya están decididos (no los reabras):
- Código Apache-2.0. Nada GPL, LGPL ni AGPL en el core.
- UI con egui/eframe + egui_dock y viewport wgpu.
- MCP con rmcp.
- El kernel va detrás de una fachada propia (`kubit-kernel-api`).
- Todo comportamiento es un `CommandSpec`.

SolidCraft (Apache-2.0) es una referencia legítima de arquitectura y código reutilizable con atribución. Kubit se diferencia de SolidCraft por su modelo de interacción (directo primero, estilo Shapr3D) y por la capa de fabricación.
</contexto>

<objetivo>
Completar la Fase 0 del roadmap en dos etapas, con una pausa obligatoria entre ellas.

**Etapa A — Spike de kernels (investigación con código).**
Crea un workspace mínimo de spike en `spikes/kernel/`, aislado del producto. Ejecuta el corpus de pruebas de §1.4 de la investigación (16 pruebas) contra **monstertruck**, **truck** y **solidcore**. Para cada prueba y cada backend, registra:
- resultado: éxito, error limpio o panic;
- tiempo;
- validez topológica;
- error de volumen o área frente al valor analítico;
- si el backend reporta historia (caras generadas, modificadas y borradas).

Confirma también si cada backend compila a wasm32 y cuánto pesa el binario.

Antes de agregar cualquier dependencia, verifica en docs.rs o crates.io su versión actual y su licencia. Si un backend no compila o no expone una operación, regístralo como resultado; no lo rodees.

Entregable: `docs/adr/0002-kernel-backend.md`, que debe incluir:
- tabla de evidencia;
- recomendación de backend inicial;
- riesgos;
- qué operaciones habrá que implementar por cuenta propia.

**→ DETENTE aquí. Muéstrame el resumen del ADR-0002 y espera mi aprobación antes de la Etapa B.**

**Etapa B — Esqueleto del producto.**
Con el backend aprobado, implementa los puntos de la Fase 0 del roadmap:
- Workspace con las capas de ARCHITECTURE.md (solo los crates que la Fase 0 necesita, aunque el diseño contemple el resto).
- `cargo xtask` con `ci`, `layers`, `wasm` y `deny`.
- CI en GitHub Actions para macOS, Windows y Linux.
- `kubit-math`: unidades y expresiones con un parser propio o `exmex`.
- `kubit-kernel-api` + backend + guard + `OpHistory`.
- `kubit-engine`: `CommandSpec`, transacciones, undo/redo.
- `kubit-doc` v0 y formato `.kubit` v1.
- `kubit-render` y `kubit-ui`: ventana, viewport, navegación y selección de caras.
- `kubit-cli`: `run`, `export`, `mcp` y `snapshot`.
- `kubit-mcp`: tools generadas del registro.
- Archivos de comunidad: LICENSE, ATTRIBUTION.md, CONTRIBUTING.md con DCO, CODE_OF_CONDUCT.md.
</objetivo>

<restricciones>
- Sigue todas las reglas de CLAUDE.md, en particular: cero panics, capas, licencias y unidades en mm.
- No implementes nada de fases posteriores (croquis, DFM, catálogo, planos), aunque parezca fácil. Sí deja los puntos de extensión que ARCHITECTURE.md describe.
- No inventes APIs de crates. Si no estás seguro de una firma, consulta la documentación o el código fuente de la dependencia.
- Si una decisión de ARCHITECTURE.md resulta inviable en la práctica, para y explícame por qué, con evidencia y alternativas.
- Mantén la UI mínima pero limpia: tokens de tema centralizados y sin estilos sueltos.
</restricciones>

<forma_de_trabajo>
1. Empieza en Plan Mode. Entrega un plan numerado de la Etapa A con los archivos que crearás y cómo medirás cada prueba.
2. Divide el trabajo en commits pequeños con mensajes convencionales en inglés.
3. Después de cada hito, ejecuta las pruebas y `cargo xtask ci` (en la Etapa B) y muestra el resultado real, no el esperado.
4. Lleva una lista de tareas visible y actualiza las casillas de docs/ROADMAP.md al cerrar cada punto.
5. Si el contexto se llena, resume el estado en `docs/STATUS.md` (hecho / en curso / siguiente / bloqueos) antes de compactar.
</forma_de_trabajo>

<criterios_de_aceptacion>
Etapa A:
- ADR-0002 con una tabla de 16 pruebas × 3 backends, llenada con resultados medidos.
- Recomendación clara y justificada.

Etapa B (la Fase 0 está completa cuando todo esto se cumple):
- Desde la UI se crea una caja, se hace push/pull de una cara, se deshace y se rehace.
- Lo mismo funciona por CLI (`kubit-cli run examples/box.json --out box.step`) y por MCP (`kubit-cli mcp`, probado con un cliente MCP real o con un test de integración del protocolo).
- Guardar y abrir `.kubit` produce un documento idéntico (test de round-trip).
- Export STEP y STL; la reimportación del STEP conserva el volumen dentro de la tolerancia.
- `cargo xtask ci` pasa en las tres plataformas; las capas bajas compilan a wasm32; `cargo deny check` pasa.
- Cero `unwrap`/`panic` en crates de producción, y una prueba proptest con parámetros hostiles en `solid.box` y en push/pull.
</criterios_de_aceptacion>

<formato_de_reporte>
Al final de cada etapa, responde con:
1. **Qué se hizo**: viñetas breves.
2. **Evidencia**: comandos ejecutados y su salida resumida.
3. **Decisiones tomadas**: con un enlace al ADR si aplica, y una explicación breve de lo no obvio en Rust.
4. **Riesgos y deuda técnica.**
5. **Siguiente paso propuesto.**
</formato_de_reporte>

---
