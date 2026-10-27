# Kubit — Roadmap

Cada fase tiene un **criterio de salida** verificable. No se abre una fase sin cerrar la anterior, salvo spikes explícitos. Claude Code mantiene las casillas actualizadas.

---

## Fase 0 — Fundaciones
**Objetivo:** el esqueleto correcto, probado de punta a punta con una caja.

- [x] Spike de kernels (monstertruck, truck, solidcore) con el corpus de 16 pruebas → **ADR-0002**
- [x] Workspace Cargo con las capas de `ARCHITECTURE.md`; `cargo xtask` con `ci`, `layers`, `wasm` y `deny`
- [x] CI en GitHub Actions para macOS, Windows y Linux, con build wasm32 de las capas bajas
- [x] `kubit-math`: unidades (mm internos, mm/in con fracciones en display), tolerancias, evaluador de expresiones (propio o `exmex`; **nunca evalexpr**, que es AGPL)
- [x] `kubit-kernel-api` + backend inicial + guard + `OpHistory`
- [x] `kubit-engine`: `CommandSpec`, sesión, transacciones, undo/redo
- [x] `kubit-doc` v0 y formato `.kubit` v1 (guardar y abrir)
- [ ] Ventana egui con viewport wgpu: órbita, pan, zoom y selección de cara
- [x] `kubit-cli` (`run`, `export`, `mcp`) y `kubit-mcp` con tools generadas del registro
- [x] `LICENSE` (Apache-2.0), `ATTRIBUTION.md`, `CONTRIBUTING.md` (DCO), `CODE_OF_CONDUCT.md`

**Salida:** crear una caja desde la UI, desde la CLI y por MCP; push/pull de una cara; undo/redo; guardar, abrir y exportar STEP/STL.

## Fase 1 — Modelado híbrido núcleo
- [x] Viewport: grid adaptativo, ejes X/Y/Z, tríada de orientación, navegación con trackpad y tema consistente
- [x] Aristas B-rep seleccionables (hover, clic, Shift+clic) con nombres persistentes v0 (ADR-0004)
- [x] Operaciones directas: push/pull (arrastre con flecha y multi-cara), redondeo, chaflán, vaciado, cilindro y booleanas; barra contextual, menú de clic derecho y valores en línea con unidades
- [ ] Croquis: líneas, arcos, círculos, rectángulos, splines básicas
- [ ] Solver propio (LM con descomposición de grafo y diagnóstico de DOF), con ezpz (MIT) como referencia
- [ ] Inferencias, cotas sugeridas y bloqueadas, colores de DOF
- [ ] Extrude, revolve, push/pull, booleanas, fillet, chamfer y shell (según lo que permita el kernel)
- [ ] Parámetros con expresiones, creados en línea
- [ ] Timeline: editar, suprimir, reordenar, "congelar historia"
- [ ] Nombres persistentes v1 con pruebas de regresión
- [ ] Menú contextual por selección, paleta de comandos, ciclo de selección
- [ ] Medir; importar STEP
- [ ] i18n es/en

**Salida:** modelar un soporte con agujeros y fillets, cambiar un parámetro y que todo se reconstruya sin perder referencias.

## Fase 2 — Kubit Wood (fabricación de mobiliario) ⭐ MVP público
- [ ] `kubit-catalog` v1 local: esquema TOML + JSON Schema; repo `kubit-catalog` con tableros, madera, cubrecanto y herrajes
- [ ] Cuerpos `Panel` y `SolidWood` con intención de fabricación (stock, veta, cubrecantos)
- [ ] Maquinados: barrenos (System 32), ranuras, rebajes, cajas, cazoletas
- [ ] Componentes, ensambles y juntas básicas
- [ ] Motor DFM v1 incremental con las ~45 reglas semilla, panel de issues, badges y quick-fixes
- [ ] Cut list, BOM, nesting de tableros por guillotina (veta, kerf) y costo estimado
- [ ] Exportar cut list (CSV/XLSX) y DXF por pieza

**Salida:** diseñar un gabinete completo y obtener cut list, nesting, BOM y costo; el linter detecta al menos 10 errores sembrados.

## Fase 3 — Planos
- [ ] Vistas ortogonales e isométricas con eliminación de líneas ocultas (HLR)
- [ ] Cortes, detalles, cotas asociativas, notas, globos con BOM
- [ ] Cajetines y plantillas; export PDF, SVG y DXF
- [ ] Planos de taller automáticos por pieza

## Fase 4 — Metal
- [ ] `SheetMetal`: brida, doblez, alivio, factor K, desarrollo plano DXF
- [ ] `Profile`: catálogo de secciones (PTR, ángulo, canal, extrusiones de aluminio), cortes a inglete, lista de corte
- [ ] Reglas DFM de metal; nesting irregular (jagua-rs/sparrow) para láser y punzonado

## Fase 5 — Simulación analítica + superficies
- [ ] Masa, centro de gravedad, estabilidad al volteo, flecha de repisas (vigas, con creep para MDF/aglomerado)
- [ ] `Slab` y `Glass`: placas, uniones, recortes, voladizos, reglas de templado
- [ ] Mapa de placas y vetas para piedra y cuarzo

## Fase 6 — Nube y web
- [ ] Kubit Catálogo en Cloudflare (Worker + D1 + R2), releases versionados, sincronización offline-first
- [ ] Capas de catálogo de organización/taller y de usuario
- [ ] Build web (WASM + WebGPU)

## Fase 7 — Expansión
- [ ] `Printed` (voladizos, pared mínima, orientación), FRP, `Soft` (tapicería)
- [ ] FEA lineal estático (faer); mallador tetraédrico propio o con licencia compatible
- [ ] Postprocesadores CNC (G-code)
- [ ] **Kubit Hub**: biblioteca de modelos paramétricos compartidos (auth, R2, previews glb, licencias por modelo, insignia "validado")
- [ ] Plugins (WASM/wasmtime), datos antropométricos e integración ARD
- [ ] Táctil y pluma
