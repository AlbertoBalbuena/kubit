# Kubit — Arquitectura

Estado: borrador v0.1 · Decisiones razonadas en `docs/adr/` · Evidencia en `docs/research/tecnologia-rust-cad.md`

---

## 1. Vista general

```
┌──────────────────────────────────────────────────────────────────────┐
│ Apps: kubit (escritorio) · kubit-cli (headless, MCP) · kubit-web     │
├──────────────────────────────────────────────────────────────────────┤
│ L5  kubit-ui          egui/eframe + egui_dock, viewport wgpu propio  │
├──────────────────────────────────────────────────────────────────────┤
│ L4  kubit-engine      registro de comandos, sesión, undo/redo,       │
│                       selección, eventos                             │
│     kubit-mcp         servidor MCP (rmcp) generado del registro      │
├──────────────────────────────────────────────────────────────────────┤
│ L3  kubit-fab   cuerpos de fabricación, despiece, desarrollos,       │
│                 nesting, costos                                      │
│     kubit-dfm   motor de reglas DFM                                  │
│     kubit-sim   analítico → FEA                                      │
│     kubit-drawings  proyecciones, HLR, cotas                         │
│     kubit-io    STEP, STL, 3MF, OBJ, glTF, DXF, SVG, PDF, CSV        │
├──────────────────────────────────────────────────────────────────────┤
│ L2  kubit-doc         documento: parámetros, componentes, partes,    │
│                       cuerpos, timeline, nombres persistentes        │
│     kubit-catalog     tipos del catálogo, carga, validación,         │
│                       consulta, caché offline                        │
├──────────────────────────────────────────────────────────────────────┤
│ L1  kubit-kernel-api  traits Kernel/Solid/Face/Edge + OpHistory      │
│     kubit-kernel-*    backends (monstertruck, solidcore, …) + guard  │
│     kubit-sketch      croquis 2D + solver de restricciones           │
│     kubit-mesh        teselado, cuerpos de malla                     │
│     kubit-render      renderer wgpu (sin ventanas)                   │
├──────────────────────────────────────────────────────────────────────┤
│ L0  kubit-math        vectores, tolerancias, unidades, expresiones   │
└──────────────────────────────────────────────────────────────────────┘
```

**Reglas de capas** (las verifica `cargo xtask layers`):

- Un crate solo depende de crates de capas inferiores.
- Nada por debajo de L5 depende de egui, eframe, winit ni rfd.
- wgpu solo entra en `kubit-render` y en L5.
- Solo los crates `kubit-kernel-*` tocan una librería de kernel.
- Todo lo que está por debajo de L4 compila para `wasm32-unknown-unknown`; lo verifica `cargo xtask wasm` en CI.

Esta organización se inspira en SolidCraft (Apache-2.0). Lo que se adapte de ahí se registra en `ATTRIBUTION.md`.

## 2. Kernel geométrico

- **Fachada propia**: `kubit-kernel-api` define las operaciones:
  - construcción: extrude, revolve, sweep, loft;
  - booleanas;
  - fillet, chamfer, shell;
  - edición directa: offset/move/delete face;
  - split;
  - consultas: masa, distancia, normales.
- **Historia obligatoria**: cada operación devuelve `OpHistory { generated, modified, deleted }`. Si el backend no la expone, la fachada la reconstruye con matching geométrico.
- **Guard**: cada llamada pasa por `catch_unwind`, un timeout y una validación posterior (sólido cerrado, orientable). Un fallo es un `KernelError` recuperable, nunca un crash.
- **Backends previstos** (los define el spike de la Fase 0 → ADR-0002):
  1. `monstertruck` como principal inicial.
  2. `solidcore` en spike paralelo; candidato a principal a mediano plazo.
  3. OCCT solo como proceso o módulo WASM separado, para importar o reparar STEP difíciles y como oráculo de pruebas. Nunca va enlazado estático (LGPL).
- **Corpus dorado**: el mismo conjunto de pruebas corre contra todos los backends.

## 3. Modelo de documento (`kubit-doc`)

```
Document
├── units, tolerances, catalog_snapshot (materiales fijados por id@versión)
├── Parameters        nombre, expresión, unidad (siempre se guarda la expresión)
├── Components        ensambles: instancias, transformaciones, joints
│   └── Parts
│       └── Bodies
│           ├── kind: Solid | Panel | SolidWood | Profile | SheetMetal
│           │         | Slab | Glass | Printed | Soft | Mesh
│           ├── fab_intent: ManufacturingIntent (ver MANUFACTURING-MODEL.md)
│           └── geometry: B-rep derivado (caché) o fuente semántica
├── Timeline          features = comandos registrados con params y referencias
└── Annotations       planos, cotas, notas
```

**Cuerpos semánticos.** Un `Panel` no es "un sólido cualquiera". Es un contorno 2D más un espesor tomado del stock, una dirección de veta, cantos con cubrecanto y operaciones de maquinado (barrenos, ranuras, cajas). El B-rep se **deriva** de esa descripción. Lo mismo pasa con `Profile` (sección de catálogo + trayectoria + cortes de extremo) y `SheetMetal` (el desarrollo plano es la fuente de verdad, con dobleces que tienen radio y factor K). Así, el despiece, el DFM y la salida CNC son robustos y no dependen de interpretar geometría arbitraria. `Solid` queda para formas libres.

**Nombres persistentes.** Ver la §6 de la investigación de tecnología:

- `TopoName` generativo en `kubit-doc`.
- Referencias como consultas semánticas ("tapa final del Extrude#3").
- Resolución puntuada con fallback geométrico y reparación guiada en la UI.
- Nunca se cambia una referencia en silencio.

## 4. Comandos: una sola fuente de verdad (`kubit-engine`)

Todo comportamiento visible es un `CommandSpec`:

```rust
CommandSpec {
    id: "solid.extrude",            // <área>.<cosa>[.<variante>]
    label, icon, shortcut, toolbar_group,
    params: JsonSchema,             // validados, números finitos y acotados
    enabled: fn(&Session) -> Enablement,
    run: fn(&mut Session, Params) -> Result<CommandOutcome, CommandError>,
    mutates: bool,                  // permisos MCP: solo lectura vs mutación
}
```

- La UI (toolbar, menú contextual, paleta Cmd/Ctrl+K), la CLI, los scripts y **MCP** llaman los mismos comandos.
- La ejecución programática nunca abre diálogos.
- Cada comando mutante es una transacción con undo/redo y queda en el timeline con su **autor** (usuario o agente).

## 5. Interacción (modelo híbrido)

Resumen; el detalle está en la §7.3 de la investigación:

1. **Un solo modo.** Se trabaja en directo y el historial siempre se registra. El timeline está colapsado por defecto.
2. **Selección → menú contextual** con 4 a 6 acciones válidas, más la paleta de comandos.
3. **Manipuladores en el viewport con entrada numérica inline.** Se aceptan expresiones con unidades (`=ancho/2`, `3/4 in`, `18mm + kerf`). No hay diálogos modales.
4. **Parámetros creados en línea**: `=alto_zoclo = 100 mm`.
5. **Croquis sobre cara con inferencias** y colores de grados de libertad (DOF). El diagnóstico dice qué restricción sobra.
6. **Cotas sugeridas (grises) vs bloqueadas.** Mirror, pattern y offset crean restricciones compuestas.
7. **Errores accionables en el timeline**, con botones de reparación.
8. **Badges DFM en el viewport** y un panel de issues con quick-fixes.
9. **Acciones de la IA visibles en el timeline**, con opción de deshacerlas y un diff antes/después.

## 6. Catálogo de materiales (`kubit-catalog`)

- **Fuente**: un repositorio aparte, `kubit-catalog` (datos CC-BY-4.0), con archivos TOML validados por JSON Schema en CI.
- **Distribución**:
  - Releases versionados se publican en Cloudflare: Worker para la API, D1 para índices y consultas, R2 para texturas y paquetes.
  - La app trae un snapshot embebido, funciona offline y sincroniza cuando hay red.
- **Capas de datos**: catálogo oficial → catálogo de la organización o taller → overrides del usuario. Esto cubre precios, stock local y reglas propias.
- **Reproducibilidad**: el documento fija `material_id@version`. Un archivo viejo se abre igual aunque el catálogo cambie, y la app ofrece actualizar.

## 7. Motor DFM (`kubit-dfm`)

- **Reglas como datos.** Cada regla tiene id, familias, tipo de chequeo, severidad, parámetros, mensajes es/en, fuentes, `confidence` y disclaimer.
- **Tipos de chequeo**:
  - `attr`: atributos del material o proceso.
  - `geom`: chequeos geométricos implementados en Rust y **parametrizados** por datos (radio interior mínimo, distancia barreno-canto, pared mínima, tamaño vs stock, radio de doblez…).
  - `asm`: relaciones entre piezas (holguras, herrajes, movimiento de madera).
  - `struct`: derivados de `kubit-sim`.
- **Ejecución incremental**: solo se reevalúan los cuerpos afectados por el último cambio, para que el linter funcione en vivo.
- **Quick-fixes**: una regla puede proponer un comando corrector, por ejemplo `body.set_thickness 18`.

## 8. MCP (`kubit-mcp`)

- **SDK**: `rmcp` (oficial). Transporte stdio en escritorio (`kubit-cli mcp`) y Streamable HTTP en modo servidor o web.
- **Tools** generadas automáticamente del registro de comandos, más tools de consulta: `inspect_document`, `measure`, `body_topology`, `dfm_report`, `cutlist`, `screenshot`, `catalog_search`.
- **Resources**: documento, parámetros, catálogo.
- **Seguridad**: modo solo lectura vs mutación. Las acciones de archivo piden confirmación.

## 9. Formato de archivo `.kubit`

Contenedor ZIP con:

- `manifest.json`: versión de formato y de la app.
- `document.json`: determinista y diffable.
- `catalog-snapshot.json`: los materiales usados, fijados.
- `cache/`: B-rep y mallas, que nunca reemplazan al documento paramétrico.
- `thumbnail.png`.

Opción "Guardar como JSON" para git y diffs. Cada versión del formato tiene su migración y su prueba.

## 10. Plataformas

| Plataforma | Cuándo | Notas |
|---|---|---|
| macOS (Apple Silicon) | Fase 0 | Plataforma de desarrollo principal |
| Windows / Linux | Fase 0 (CI) / Fase 1 (instaladores) | Builds en CI desde el día 1 |
| Web (WASM + WebGPU) | Fase 6 | Las capas bajas ya compilan a wasm32 desde la Fase 0 |

## 11. Calidad y seguridad

- `#![forbid(unsafe_code)]`, salvo interop nativo justificado.
- Clippy prohíbe `unwrap/expect/panic/todo` en crates de producción.
- Números de entrada validados: finitos y acotados.
- **Pruebas**:
  - unitarias;
  - **oráculo** con valores analíticos;
  - fuzzing/proptest con entradas hostiles (criterio: cero panics);
  - regresión de nombres persistentes;
  - un test de regresión por cada bug.
- **`cargo deny`** con lista blanca de licencias (Apache-2.0, MIT, BSD, ISC, Zlib; MPL-2.0 con revisión). Bloquea GPL, LGPL y AGPL en el core.
- **Determinismo**: no se itera sobre `HashMap` en caminos que afectan la geometría (se usa `IndexMap`/`BTreeMap`).
- **Unidades internas**: milímetros y radianes en `f64`. Conversión solo en UI e IO; se muestra en mm o en pulgadas con fracciones.
