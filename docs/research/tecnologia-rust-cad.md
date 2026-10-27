# Investigación tecnológica: Kubit en Rust

> Kubit: CAD 3D directo + paramétrico, open source (Apache-2.0), en Rust. Escritorio (macOS/Windows/Linux) primero, web (WASM/WebGPU) después. Servidor MCP nativo, validación de fabricación, planos 2D, simulación y nesting.
>
> Fecha de corte: 26 de octubre de 2026. Las versiones y fechas salen de docs.rs, lib.rs, sitios de proyecto y prensa técnica. Lo que no pude verificar va marcado con **(sin verificar)**. GitHub estuvo casi siempre bloqueado por la lista de dominios permitidos, así que el estado de algunos repos se dedujo de fuentes secundarias.

---

## 0. Resumen ejecutivo (opinado)

| Decisión | Recomendación | Por qué |
|---|---|---|
| Kernel B-rep | **Una fachada `trait Kernel` propia.** Primer backend: **monstertruck**, con **truck** como referencia. **solidcore** en observación activa, con un spike a 3 meses. **OCCT** solo como backend opcional, fuera de proceso, para importar y reparar STEP. | Ningún kernel Rust es hoy robusto al nivel de Parasolid u OCCT. La fachada permite cambiar de kernel sin reescribir la app. monstertruck ya trae fillets y STEP de lectura/escritura sobre la base de truck, y es Apache-2.0. |
| Solver de croquis | **Solver propio** (Newton/LM con descomposición de grafo y análisis de grados de libertad), usando **ezpz (MIT)** como referencia o dependencia inicial. **No usar slvs ni SolveSpace** (GPL-3.0). **No usar planegcs** (LGPL, código de FreeCAD). | La licencia y el control del UX de los croquis (inferencia, arrastre estable, diagnóstico de sobre-restricción) son ventaja competitiva. |
| UI | **egui/eframe 0.37 + egui_dock** para la v1, con viewport wgpu propio. Capa UI desacoplada como en SolidCraft para poder migrar. Vigilar iced y Xilem. **Evitar Slint** por su licencia en un proyecto Apache-2.0. | Es la única opción madura con web real (WebGPU/WebGL2), docking, viewport wgpu y alta velocidad de iteración. El pulido tipo Shapr3D se logra con theming y widgets propios. |
| MCP | **rmcp 3.1** (SDK oficial). stdio para escritorio y Streamable HTTP para el modo servidor o web. Las tools se generan desde el registro de comandos. | Es oficial, sigue el spec 2026-07-28 y tiene macros `#[tool]`. |
| Nesting | **jagua-rs (MPL-2.0) + sparrow (MIT)** para nesting irregular. **binpack2d (MIT)** para rectángulos y guillotina, con un algoritmo guillotina propio para tableros de mueble (veta, kerf, cortes pasantes). | Estado del arte académico, en Rust. MPL-2.0 es copyleft débil a nivel de archivo y es compatible con Apache-2.0 si no modificas sus archivos o publicas esos cambios. |
| Naming topológico | **Nombres persistentes por historia** (mapa de elementos al estilo FreeCAD 1.0/OCCT `BRepTools_History`) más **resolución por consulta semántica** al estilo Onshape (`qCreatedBy`, `qCapEntity`), con **fallback geométrico** puntuado y reparación guiada en la UI. | Es la única estrategia que funciona sin depender del kernel. |
| Modelo de interacción | **"Directo por defecto, paramétrico cuando lo pidas":** una línea de tiempo siempre registrada y colapsable, push/pull con entrada numérica inline, menú contextual por selección y sin diálogos modales. Parámetros con expresiones y unidades. | Combina la velocidad de Shapr3D con la precisión de Fusion y evita el choque que vivió Shapr3D al meter historia en 2023. |

**Alertas de licencia (detalle en §8):**
- **evalexpr es AGPL-3.0** desde la v13. Si se usara en el binario, todo Kubit pasaría a AGPL. Hay que evitarlo.
- **slvs/SolveSpace es GPL-3.0.**
- **OCCT es LGPL-2.1** con excepción. En WASM y en enlace estático complica la distribución.
- **Slint** se ofrece como GPL-3.0, Royalty-free con atribución o comercial. No es Apache-compatible en sentido estricto.
- **tritet con TetGen es AGPL**, y **Triangle** tiene licencia no comercial **(sin verificar el texto exacto)**.
- **jagua-rs es MPL-2.0**, aceptable con cuidado.

---

## 1. Kernel B-rep

### 1.1 Matriz de opciones

| Kernel | Licencia | Actividad / última versión | Booleanas | Fillet / chaflán | Shell / offset | Sweep / loft | STEP | wasm32 | Limitaciones conocidas |
|---|---|---|---|---|---|---|---|---|---|
| **truck** (ricosjp) | Apache-2.0 | `truck-modeling` 0.6.0 ([docs.rs](https://docs.rs/truck-modeling)). Lanzamientos espaciados; no confirmé la fecha exacta de 0.6 **(sin verificar)** | `truck-shapeops` (and/or). Funcionan en casos simples. Son frágiles con caras tangentes o coincidentes y aristas sobre aristas. SolidCraft envuelve truck en un guard de panics ([CLAUDE.md de SolidCraft](https://raw.githubusercontent.com/tangency-labs/solidcraft/master/CLAUDE.md)), lo que dice mucho de su robustez | No trae fillets en el núcleo (SolidCraft escribió sus propios blends) | No en el núcleo | extrude, revolve, sweep y homotopía (loft simple) en `builder` | `truck-stepio`: salida sólida; la importación es parcial ([docs.rs](https://docs.rs/truck-stepio)) | Sí. Se diseñó con wgpu y WASM en mente | Robustez de booleanas, ausencia de fillet/shell, panics con entradas degeneradas |
| **monstertruck** (virtualritz, fork de truck) | Apache-2.0 | 0.4.1, 19-sep-2026 ([docs.rs](https://docs.rs/monstertruck)) | `solid::and/or`. Declara que el backend booleano es sustituible | Fillets de bola rodante (`monstertruck-fillet`, aplicados después del CSG). El chaflán no aparece | `OffsetCurve`, `OffsetSurface`, `NormalOffsetField`. No aparece un shell de sólido | extrude y revolve. Loft no listado | Lectura y escritura de STEP AP (partes y ensambles) | No documentado **(sin verificar, probablemente sí al heredar de truck)** | Proyecto de una persona. Healing y ensamble nuevos, sin mucha batalla en producción |
| **solidcore** (tangency-labs, port de SMLib de NVIDIA) | Apache-2.0 (upstream usd-brep de NVIDIA también Apache-2.0) | 0.0.x experimental ([README](https://github.com/tangency-labs/solidcore)) | Portadas. La integración sigue en curso, con caminos diferidos y desajustes conocidos | Fillets y chaflanes portados | Offset/shell portados | Sweeps y skins (loft) portados | Fuera de alcance (los traductores OCCT se excluyeron) | Sí, como objetivo explícito. Advierte que el stack de 1 MB en wasm se corrompe en silencio y pide subirlo a 8 MiB | API inestable. Su gran ventaja es el diff-testing contra C++ con tolerancia 1e-9 y topología idéntica. Es el candidato más serio a medio plazo |
| **Kernel de SolidCraft** (`solidcraft-kernel`) | Apache-2.0 (según el brief; el CLAUDE.md no lo declara) | Muy activo, v0.15.0 hace ~2 semanas ([newreleases](https://newreleases.io/project/github/tangency-labs/solidcraft/release/v0.15.0)) | truck detrás de `guard` (panic → `KernelError::Internal`) | Blends propios | No confirmado | Sweep con raíles guía y superficies (v0.15) | Sí, con fixes recientes de import y export | Todo lo que está por debajo de la capa de apps compila a `wasm32-unknown-unknown` | No es una librería pensada para terceros, sino una capa de producto. La comunidad lo describe como "vibe-coded" y con ~1 semana de vida pública ([Hackaday](https://hackaday.com/2026/10/17/solidcraft-is-parametric-automation-friendly-cad-for-desktop-and-browser/), comentarios no verificados). Falla con STEP grandes según un comentario |
| **opencascade-rs** (mkovaxx) | Bindings con licencia propia no confirmada **(sin verificar)**. OCCT es LGPL-2.1 con excepción | "major work in progress", hobby ([repo](https://github.com/mkovaxx/opencascade-rs)) | Las de OCCT (las más robustas del open source) | Sí (OCCT) | Sí | Sí | Sí (AP203/214/242 vía OCCT) | No soportado | Compila OCCT desde fuente (feature `builtin`) o enlaza dinámicamente. API incompleta. Tiempos de build enormes |
| **cadrum** | MIT (los bindings) + OCCT 7.9.3 (LGPL) | 0.9.0, 10-oct-2026 ([docs.rs](https://docs.rs/crate/cadrum/0.4.3)) | union/subtract/intersect | Sin documentar | No | No | STEP/BRep con colores XDE | No mencionado | Mínimo. Interesante porque trae **SVG con hidden-line removal** |
| **occt-wasm** | Wrapper MIT/Apache. **El binario WASM de OCCT es LGPL-2.1-only** | 5.0.1, 15-oct-2026 ([docs.rs](https://docs.rs/crate/occt-wasm/latest)) | Sí | Sí (modelado OCCT) | Sí | Sí | STEP/STL/glTF, XCAF | Ejecuta OCCT-WASM **dentro de wasmtime**; existe un paquete npm para navegador | Interesante: aísla OCCT como **módulo WASM separado y reemplazable**, lo que facilita cumplir la LGPL. Penaliza rendimiento |
| **Fornjot** | 0BSD/MIT | **Cerrado**: "This project has been shut down. Its goals were never reached" ([README](https://cdn.jsdelivr.net/gh/hannobraun/fornjot@main/README.md)). Última versión `fj` 0.49 | — | — | — | — | — | — | Descartado |

Otros nombres que salen en búsquedas: `chijin` (bindings OCCT, docs.rs) y `foxtrot` de Formlabs, un visor STEP en Rust ([Matt Keeter](https://www.mattkeeter.com/projects/foxtrot)) que solo sirve como referencia de parser y teselado. Ninguno cambia la recomendación.

### 1.2 OCCT y la LGPL: lo que importa de verdad

- **Enlace dinámico en escritorio:** es el escenario que la LGPL-2.1 cubre sin problemas. Kubit puede seguir siendo Apache-2.0 si se distribuyen las `.so/.dylib/.dll` de OCCT, se incluyen su fuente (o una oferta escrita) y su aviso de licencia, y el usuario puede reemplazar la librería. OCCT añade una excepción que relaja algunos requisitos sobre headers y plantillas **(texto de la excepción sin verificar en esta sesión)**.
- **Enlace estático** (lo que hacen `opencascade-rs` con `builtin` y el modelo habitual de Cargo): la LGPL exige permitir el re-enlace. Habría que distribuir los objetos `.o/.rlib` de Kubit o el código fuente completo. Como Kubit es open source, publicar la fuente cumple en la práctica, pero los forks propietarios de terceros heredarían la obligación. Es una fricción real para un proyecto Apache-2.0 que quiere ser "embebible".
- **WASM en navegador:** un único `.wasm` monolítico equivale a un enlace estático. occt-wasm lo resuelve cargando OCCT como **módulo WASM separado**, que el usuario puede sustituir. Si algún día se usa OCCT en la web, ese es el patrón a seguir.
- **Política de contribución:** SolidCraft prohíbe incluso *leer* código GPL/LGPL/AGPL como OCCT, FreeCAD o SolveSpace ([CLAUDE.md](https://raw.githubusercontent.com/tangency-labs/solidcraft/master/CLAUDE.md)). Kubit debería adoptar la misma regla de clean-room para el núcleo y permitir OCCT solo como **plugin o proceso opcional**, no como dependencia del core.

### 1.3 Estrategia recomendada

1. **Crate `kubit-kernel-api`** con traits estables: `Kernel`, `Solid`, `Face`, `Edge`, operaciones (`extrude`, `revolve`, `sweep`, `loft`, `boolean`, `fillet`, `chamfer`, `shell`, `offset_face`, `split`, `move_face`/push-pull, `delete_face`/healing), consultas (masa, distancia, normal, punto en sólido) y **exigencia de historia**: cada operación devuelve `OpHistory { generated, modified, deleted }` por elemento (ver §6). Si un backend no la expone, la fachada la reconstruye por matching geométrico.
2. **Todas las llamadas pasan por un guard**: `catch_unwind`, timeout y validación posterior (sólido cerrado, orientable, sin auto-intersecciones). El fallo se presenta como error recuperable en la timeline, nunca como un crash.
3. **Backend 1: `monstertruck`.** Hoy es el que más cubre sobre la base de truck: fillets, STEP r/w, healing. Contribuir upstream en lugar de hacer otro fork.
4. **Backend 2 (spike en paralelo): `solidcore`.** Si su diff-testing llega a verde en booleanas, fillets y shell, debería pasar a ser el backend principal en 2027. Es el único con una semántica "de kernel industrial" heredada de un código C++ probado.
5. **Backend 3 (opcional, fuera del core): OCCT** como proceso aparte o módulo WASM (patrón occt-wasm) para **importar y reparar STEP difíciles** y como oráculo de pruebas. Nunca enlazado estáticamente al binario principal.
6. **Corpus de pruebas propio** (tests dorados) que se ejecute contra todos los backends. Eso es lo que hace que la fachada sea verdadera y no solo nominal.

### 1.4 Lista concreta de pruebas para el spike (por backend)

Cada prueba mide el resultado (éxito o error limpio frente a panic), el tiempo, la validez topológica (Euler, caras cerradas) y el volumen comparado con un valor analítico.

1. **Caja − cilindro pasante** (agujero simple) y **caja − cilindro ciego**.
2. **Booleana coplanar**: dos cajas que comparten una cara (unión), y una caja que toca otra solo por una arista.
3. **Booleana tangente**: cilindro tangente a una cara plana; esfera tangente a una caja.
4. **Unión de 100 cilindros en patrón** (rendimiento y acumulación de tolerancias).
5. **Fillet de una arista** recta, de una **cadena de aristas tangentes** y de un **vértice con 3 aristas** (esquina de caja). Radio variable si existe.
6. **Fillet después de booleana**: caja con agujero, filete del borde del agujero.
7. **Chaflán** de distancia-distancia y distancia-ángulo.
8. **Shell**: caja vaciada 2 mm con una cara abierta; cilindro con fillet vaciado.
9. **Offset de cara / push-pull directo**: mover una cara de una caja ya fileteada. Es el test clave para el modo directo.
10. **Sweep** de un perfil circular por una hélice (rosca). **Loft** entre un rectángulo y un círculo.
11. **Revolve** de un perfil que toca el eje (degeneración en el polo).
12. **STEP round-trip**: exportar los casos 1-11, reimportarlos y comparar volumen y área. **Importar** 10 STEP reales (McMaster, GrabCAD, uno de impresora 3D grande; el caso que falló en SolidCraft según los comentarios de Hackaday).
13. **Teselado**: tolerancia cordal y angular, sin grietas entre caras (watertight) para exportar STL/3MF.
14. **Build wasm32**: compilar, medir el tamaño del `.wasm` y ejecutar los tests 1-9 en navegador con 8 MiB de stack.
15. **Fuzzing** (cargo-fuzz o proptest) con parámetros aleatorios en 1, 5 y 9: cero panics como criterio de aceptación.
16. **Historia**: en 1, 5 y 9, ¿el backend informa qué caras se generaron, modificaron o borraron?

---

## 2. Solvers de restricciones geométricas 2D

| Opción | Licencia | Estado | Comentario |
|---|---|---|---|
| **ezpz** (KittyCAD/Zoo) | **MIT** | 0.2.29, 11-oct-2026 ([docs.rs](https://docs.rs/ezpz)) | Gauss-Newton con análisis de grados de libertad, formato de texto para pruebas. Restricciones: fijar X/Y, distancia, vertical, paralelo, punto sobre arco, radio… Lo usa Zoo Design Studio en producción. **El mejor punto de partida.** |
| **slvs** (bindings de SolveSpace) | **GPL-3.0** | 0.6.0, jun-2023 ([lib.rs](https://lib.rs/crates/slvs)) | Descartado por licencia y abandono. |
| **planegcs** (solver del Sketcher de FreeCAD) | LGPL-2.1+ como el resto de FreeCAD **(sin verificar el archivo LICENSE)**. Existe un port a WASM (`@salusoft89/planegcs`, [README](https://cdn.jsdelivr.net/npm/@salusoft89/planegcs@1.1.7/README.md)) | Maduro (DogLeg/LM/BFGS) | Mismo problema que OCCT, más C++. Útil solo como referencia conceptual, y ni siquiera eso bajo una política clean-room. |
| **Solver propio de SolidCraft** | Apache-2.0 | Activo | "the sketch solver and the blend operations are our own" ([CLAUDE.md](https://raw.githubusercontent.com/tangency-labs/solidcraft/master/CLAUDE.md)). Se puede estudiar y reutilizar con atribución. |

**Recomendación:** construir `kubit-sketch` con:
- Modelo de variables (puntos, parámetros de curva) y restricciones como residuos con jacobiano analítico.
- **Descomposición del grafo** (componentes y clusters rígidos) para resolver en subproblemas pequeños. Es clave para el rendimiento interactivo y para diagnosticar.
- **Levenberg-Marquardt / DogLeg** con fallback. Durante el arrastre, solve incremental con **mínima perturbación**: se ponderan las variables que el usuario no toca para que la geometría "no salte", que es justo la queja típica de Fusion.
- **Análisis de DOF y redundancias** (rango del jacobiano por QR con pivotaje, usando `faer`) para colorear lo infra o sobre-restringido y explicar *qué* restricción sobra. Fusion falla aquí con mensajes del tipo "Failed to solve" ([foro Autodesk](https://forums.autodesk.com/t5/fusion-support-forum/long-list-of-bugs-issues/m-p/13163496/highlight/true)).
- **Inferencia de restricciones** al dibujar (horizontal, coincidente, tangente, punto medio) como capa de UX separada del solver.
- Empezar envolviendo o forkeando **ezpz** (MIT) mientras se construye la descomposición propia, con las mismas pruebas de texto.

---

## 3. Frameworks de UI

| Framework | Licencia | Estado | Viewport 3D propio | Docking | Texto e IME | Accesibilidad | i18n | Web | Pulido tipo Shapr3D |
|---|---|---|---|---|---|---|---|---|---|
| **egui/eframe** | MIT/Apache | eframe 0.37.0, 15-oct-2026 ([docs.rs](https://docs.rs/crate/eframe/latest)) | Excelente (callbacks wgpu, el mismo `wgpu::Device`) | `egui_dock` 0.21 ([docs.rs](https://docs.rs/crate/egui_dock/0.21.0)), `egui_tiles` | Correcto en escritorio. En móvil y web la edición de texto es más débil (lo admite su propia doc) | AccessKit en escritorio. En web, lector de pantalla experimental | Manual (fluent o similar) | **Sí, real** (WebGPU con fallback a WebGL2) | Inmediato, "programmer-art" por defecto. Se puede pulir con mucho theming propio. Las animaciones son limitadas pero posibles |
| **iced** | MIT | 0.14, 7-dic-2025 ([Phoronix](https://www.phoronix.com/news/Iced-0.14-Rust-GUI-LIbrary)): renderizado reactivo, time-travel debugging, IME, hot reload | Bueno (widget `shader` con wgpu) | Solo `pane_grid` básico | IME desde 0.14 | Limitada **(sin verificar)** | Manual | Sí, menos pulido | Arquitectura Elm limpia, estética mejor de base que egui, ecosistema más pequeño. Lo usa COSMIC (System76) |
| **Slint** | **GPL-3.0 / Royalty-free 2.0 (atribución obligatoria) / comercial** ([términos](https://slint.dev/terms-and-conditions)) | Maduro | Posible (underlay OpenGL/wgpu) | Hay que hacerlo | Bueno | Buena | Integrada (gettext) | Sí | El mejor "look nativo" con DSL declarativo. **Pero:** la Royalty-free prohíbe "exponer las APIs de Slint" y obliga a mostrar atribución. Para un CAD Apache-2.0 que quiere ser librería y plataforma de plugins, es un riesgo. **Descartado.** |
| **Bevy** | MIT/Apache | 0.20, 8-oct-2026 ([bevy.org](https://bevy.org/news/)) | Nativo (es un motor) | Inmaduro | Débil | Parcial | Débil | Sí | Pensado para juegos: el ECS y el render loop continuo son un mal encaje para un CAD de precisión. El editor de Bevy sigue en construcción |
| **Makepad** | MIT/Apache | Activo **(sin verificar la versión)** | Muy bueno (todo es shader) | Sí (lo usa su IDE) | Correcto | Débil | Débil | Sí, excelente rendimiento | El más "bonito y fluido" de base, pero con comunidad pequeña, DSL propio y documentación escasa. Riesgo de bus-factor |
| **Xilem/Masonry** | Apache-2.0 | xilem 0.4.0, oct-2025, "alpha" ([docs.rs](https://docs.rs/crate/xilem/latest)) | Vello/wgpu | No | Bueno (Parley) | AccessKit desde el diseño | — | Parcial | Es el futuro probable de la UI en Rust (Linebender), pero no está listo para un producto en 2026 |
| **Floem** | MIT | Activo (Lapce) **(sin verificar la versión)** | Posible | No | Bueno | Parcial | — | Limitado | Reactivo y con buen rendimiento. Comunidad pequeña |
| **Tauri + frontend web** (React/Svelte + Three.js o WebGPU) | MIT/Apache | Tauri 2.x maduro **(sin verificar la versión exacta)** | El viewport iría en JS (Three.js), o un canvas wgpu en otra ventana, con integración incómoda | Excelente (librerías web) | Excelente | Excelente (DOM) | Excelente | **Trivial** (es web) | El pulido tipo Shapr3D es más fácil con CSS y animaciones web. **Pero** fragmenta la app en dos lenguajes, y el viewport de CAD con selección precisa y millones de triángulos sufre en el puente IPC. Opción válida si el kernel corre en WASM dentro del webview |

**Recomendación:**
- **v1: egui + eframe + egui_dock**, viewport wgpu propio (se comparte con la web) y **design system propio** (tokens, iconos SVG, tipografía Inter o similar, animaciones de 120-200 ms en hovers y paneles). SolidCraft demuestra que llega para un CAD tipo Fusion en escritorio y web.
- **Arquitectura obligatoria:** capa UI delgada y reemplazable. Imitar las capas de SolidCraft (nada por debajo de la UI depende de egui, winit ni wgpu-UI). Toda acción es un **comando** con id, parámetros JSON y documentación; toolbar, paleta, scripts, CLI y MCP llaman al mismo registro.
- **Revisión en 2027:** si Xilem/Masonry madura, o si el pulido en egui toca techo, migrar la UI. El viewport y el motor no se tocan.
- Para **Pencil/táctil** (estilo Shapr3D en iPad): egui soporta touch básico. Para presión y gestos finos harían falta extensiones propias en winit. Lo dejo para fase posterior.

---

## 4. MCP: SDK de Rust

- **rmcp** es "The official Rust SDK for the Model Context Protocol" (repo `modelcontextprotocol/rust-sdk`). Versión **3.1.0**, enlazada al spec **2026-07-28** ([README 3.1.0](https://docs.rs/crate/rmcp/3.1.0/source/README.md)). La rama 3.x salió tras varias betas (`3.0.0-beta.5`).
- **Transportes:** stdio (servidor `stdio`, cliente `TokioChildProcess`) y **Streamable HTTP** (servidor `StreamableHttpService`, cliente `StreamableHttpClientTransport`). El SSE antiguo ya no figura. Admite transportes propios mediante el trait `Transport`.
- **Features:** `server`, `client`, `macros` (`#[tool]`, `#[prompt]`), `schemars` (JSON Schema automático), `auth` (OAuth), `elicitation`.
- **Madurez:** alta para servidores. Es el SDK oficial, va por la tercera versión mayor y tiene changelog. Alternativas comunitarias: `rust-mcp-sdk` 2.0 y `mcpkit`. No las recomiendo frente al oficial.

**Diseño recomendado para Kubit:**
- Las tools MCP se **generan desde el registro de comandos** (como `CommandSpec` en SolidCraft), no a mano. Cada comando expone un JSON Schema (`schemars`), una descripción y ejemplos.
- **Tools de consulta** de alto valor para IA: `describe_model`, `list_features`, `query_faces(filter)`, `measure`, `validate_for_fabrication(process)`, `render_view(png)`, `get_drawing(svg)`.
- **Recursos MCP:** documento actual, parámetros, timeline y reportes de validación.
- **Transporte:** stdio cuando Claude Desktop o Claude Code lanzan `kubit --mcp`; Streamable HTTP en `127.0.0.1` con token por sesión cuando la app de escritorio ya está abierta (el patrón `--control PORT` de SolidCraft). En web, la app actúa como servidor dentro del navegador (vía WebSocket o proxy) **(diseño por validar)**.
- Toda llamada MCP entra en la **misma pila de undo** con autor "agente" y se muestra en la timeline. Un modo "requiere confirmación" cubre las operaciones destructivas.

---

## 5. Crates de soporte

### 5.1 Formatos de archivo

| Formato | Crate recomendado | Licencia | Notas |
|---|---|---|---|
| STEP | Lo del backend (`monstertruck-step`, `truck-stepio`). `ruststep` 0.4.0, sep-2026 ([docs.rs](https://docs.rs/ruststep)) como parser genérico (AP201/AP203) | Apache-2.0 | AP242 y AP214 con colores no están cubiertos por ruststep. Para importar STEP difíciles: OCCT fuera de proceso (§1.2) |
| STL | `stl_io` | MIT **(sin verificar la versión)** | Trivial |
| 3MF | **`lib3mf`** (Rust puro, sin unsafe) 0.1.7, 23-oct-2026 ([docs.rs](https://docs.rs/crate/lib3mf/latest)) | MIT | Core, Materials, Production, Slice, Beam Lattice… Las booleanas solo se validan. Alternativa: `threemf` |
| OBJ | `tobj` (lectura); la escritura es trivial | MIT **(sin verificar)** | |
| glTF | `gltf` (lectura) + escritura propia o `gltf-json` | MIT/Apache **(sin verificar)** | Para AR/visualización |
| DXF | **`acadrust`** 0.6.7, 24-oct-2026: DXF R12–R2018+ ASCII/binario y **DWG R13–R2018 r/w** ([docs.rs](https://docs.rs/crate/acadrust/0.4.1)) | **MPL-2.0** | Muy activo. Alternativa: `dxf` (ixmilia) 0.6.0, nov-2024 ([lib.rs](https://lib.rs/crates/dxf)), licencia no visible en la página (MIT según su repo, **sin verificar**) |
| PDF | **`krilla`** 0.8.2, jun-2026 (el backend PDF de Typst) ([docs.rs](https://docs.rs/crate/krilla/latest)) | MIT/Apache | Vectores, texto con fuentes y gradientes. Ideal para planos |
| SVG | `svg` (escritura) / `usvg`+`resvg` (render) | MIT/Apache | |

### 5.2 Planos 2D y eliminación de líneas ocultas (HLR)

Enfoques de menor a mayor exactitud:
1. **Raster en GPU:** render de aristas con depth test y vectorización. Es rápido pero no sirve para planos de fabricación.
2. **Malla + visibilidad por rayos:** extraer aristas vivas y siluetas del teselado, partirlas en sus intersecciones proyectadas y decidir la visibilidad de cada tramo con raycast contra un BVH de la malla (`parry3d` o `bvh`). Es robusto y sencillo, pero las curvas salen poligonales.
3. **Híbrido (recomendado):** las aristas son **curvas exactas del B-rep** proyectadas (arcos y líneas reales, con siluetas calculadas sobre las superficies). La visibilidad por tramo se decide con raycast contra la malla teselada (BVH), con tolerancia. Así se obtienen curvas exactas para DXF y PDF sin escribir un HLR analítico completo.
4. **HLR analítico B-rep** (como `HLRBRep` de OCCT; cadrum lo expone en SVG): es exacto, pero en Rust habría que escribirlo de cero. Conviene dejarlo como meta a largo plazo.

Para acotar: cotas asociativas ancladas a referencias persistentes (§6), estilos ISO/ANSI y exportación vía `krilla` y `acadrust`.

### 5.3 Nesting y empaquetado

| Necesidad | Crate | Licencia | Notas |
|---|---|---|---|
| Nesting irregular (láser, CNC, tela) | **jagua-rs** 1.1.0, 9-oct-2026 ([docs.rs](https://docs.rs/jagua-rs)): motor de detección de colisiones con modelos strip (`spp`) y bin packing (`bpp`), publicado académicamente ([arXiv 2508.08341](https://arxiv.org/abs/2508.08341v1)) | **MPL-2.0** | Copyleft débil por archivo: usarlo como dependencia sin modificarlo no afecta a Kubit. Si se modifica, se publican esos archivos |
| Optimizador de nesting | **sparrow**: heurística del estado del arte sobre jagua-rs, iguala o mejora todas las mejores soluciones conocidas ([arXiv 2509.13329](https://arxiv.org/pdf/2509.13329v1)). Último push 21-oct-2026 ([gittrend](https://gittrend.io/repo/JeroenGar/sparrow)). Existe una demo web, "Sparrow Studio" ([McNeel forum](https://discourse.mcneel.com/t/sparrow-studio-free-open-source-nesting-in-your-browser/222513)) | **MIT** (según gittrend) | Resuelve strip packing. Para varias hojas finitas hay que adaptar el modelo `bpp`. Su punto débil declarado son las instancias muy homogéneas |
| Rectángulos / guillotina | **binpack2d** 1.0.1, 10-oct-2026, módulos `guillotine` y `maxrects` ([docs.rs](https://docs.rs/binpack2d)) | MIT | Para tableros de melamina o MDF se necesitará un **guillotina propio**: kerf, veta, cortes pasantes por etapas, sobrantes reutilizables. `gdrr-2bp` (mismo autor que jagua-rs) es una referencia académica de 2BP con guillotina |
| Booleanas 2D y offsets (kerf, holguras) | `i_overlay` / `geo` | MIT/Apache **(sin verificar)** | Sirven para offset de piezas, regiones de croquis y validación DFM |

### 5.4 Álgebra lineal, FEA y mallado

| Componente | Crate | Licencia | Estado |
|---|---|---|---|
| Sparse directo | **faer** 0.24.4, 15-sep-2026: Cholesky (LLᵀ), LU y QR dispersos ([docs.rs](https://docs.rs/faer)) | MIT | La mejor opción en Rust puro, y compila a WASM (sin `rayon`) **(sin verificar en wasm)**. El repo se movió a Codeberg |
| Álgebra pequeña y geometría | `nalgebra` / `glam` | Apache/MIT | `nalgebra-sparse` como formato CSR/COO si se prefiere |
| FEM | `fenris` 0.0.33 (2023): "not recommended for general usage", sin estabilidad de API ([docs.rs](https://docs.rs/crate/fenris/latest)) | MIT/Apache | Sirve como referencia, no como dependencia. `gemlab` (MIT) depende de LAPACK/MUMPS en C ([lib.rs](https://lib.rs/crates/gemlab)), así que no sirve para WASM |
| Mallador tetraédrico | `tritet` 3.0.2: Triangle (MIT para el wrapper), TetGen opcional con **AGPL** ([docs.rs](https://docs.rs/crate/tritet/3.0.2)). La licencia propia de Triangle prohíbe la venta comercial sin permiso **(sin verificar el texto)**. Bindings `fTetWild-rs` en [Codeberg](https://codeberg.org/phqb/fTetWild-rs) (fTetWild es MPL-2.0, **sin verificar**) | Mixta | **No hay un mallador tetraédrico Rust puro y maduro.** Opciones: (a) fTetWild como plugin nativo opcional (no en web); (b) mallador Delaunay propio con refinamiento (meses de trabajo); (c) mallado en servidor |

**Recomendación de simulación (MVP):** estática lineal elástica con tetraedros cuadráticos (tet10), ensamblaje propio, `faer` sparse Cholesky y mallado vía fTetWild como plugin nativo. Para la validación de fabricación de muebles (flechas en baldas, cargas en herrajes), un **solver de vigas y placas** analítico/1D-2D da más valor por menos esfuerzo que el FEA 3D completo.

### 5.5 Parámetros, unidades y scripting

| Necesidad | Crate | Licencia | Comentario |
|---|---|---|---|
| Expresiones de parámetros | ~~evalexpr~~ 13.1.0: **AGPL-3.0** ([docs.rs](https://docs.rs/crate/evalexpr/latest)). **Prohibido** | AGPL | Hay que evitarlo |
| | **exmex** 0.21.0, may-2026 ([docs.rs](https://docs.rs/crate/exmex/latest)) | MIT/Apache | Rápido, con derivadas simbólicas (útiles para el solver) |
| | `fasteval` 0.2.4 ([docs.rs](https://docs.rs/crate/fasteval/latest)) | MIT **(sin verificar)** | Abandonado desde hace años |
| | **Recomendado: parser propio** (pest, chumsky o un descenso recursivo de unas 500 líneas) | — | Las expresiones de CAD necesitan **unidades dimensionales** (`=ancho/2 + 3 mm`), referencias a parámetros y medidas (`d12`, `Body1.height`), detección de ciclos y mensajes de error localizados. Ningún crate cubre todo eso |
| Unidades | `uom` 0.38.0, feb-2026 ([docs.rs](https://docs.rs/crate/uom/latest)) | MIT/Apache | Comprobación en compilación. Buena opción dentro del motor. Para los valores del usuario en tiempo de ejecución hace falta un tipo `Quantity { value, dim }` propio (uom es estático) |
| Scripting embebido | **Rhai** 1.27.0, 21-oct-2026, con sandbox y límites de operaciones ([docs.rs](https://docs.rs/crate/rhai/)) | MIT/Apache | Para macros de usuario y features personalizadas. Funciona en WASM |
| Plugins | **wasmtime** v49+ con el Component Model ([newreleases](https://newreleases.io/project/github/bytecodealliance/wasmtime/release/v49.0.0)) | Apache-2.0 | Plugins aislados y multilenguaje con interfaz WIT. En la versión web, los plugins se cargan con el runtime WASM del navegador (jco). Diseñar la interfaz WIT desde el día 1 |

---

## 6. El problema del naming topológico

**Problema:** una feature (fillet, cota, croquis sobre cara) referencia "la cara 7". Al editar algo aguas arriba, el kernel renumera y la referencia salta a otra cara o desaparece.

### Cómo lo resuelven otros

- **FreeCAD 1.0 (nov-2024):** integró el algoritmo de la comunidad, el trabajo de realthunder en LinkStage3 que Ondsel subió a mainline. Cada forma lleva un **mapa de elementos** donde cada subelemento recibe un **nombre mapeado** que codifica su **historia de generación** (de qué operación, de qué elementos de entrada, con qué rol). Las referencias guardan ese nombre y se resuelven tras el recálculo. Solo cubre Sketcher y PartDesign, y los propios mantenedores dicen que "no está completamente resuelto" ([Hackster](https://hackster.io/news/freecad-hits-1-0-after-two-decades-finally-sees-an-end-to-the-toponaming-problem-7e7ac2df3e63), [AlternativeTo](https://alternativeto.net/news/2024/11/freecad-1-0-launches-with-enhanced-ui-ux-built-in-assembly-workbench-and-tnp-fixes/)). Los detalles internos de la codificación no los pude verificar (la wiki estaba bloqueada).
- **Onshape:** las features no guardan índices, guardan **consultas**. En FeatureScript existen `qCreatedBy(featureId, EntityType)`, `qCapEntity`, `qNonCapEntity`, y la selección del usuario se convierte en una consulta evaluada contra la historia de operaciones del kernel (Parasolid). Los usuarios lo describen como muy estable, aunque con inestabilidades en cadenas largas y pérdida de identidad al cruzar contextos como Derive ([foro Onshape](https://forum.onshape.com/discussion/comment/118336)). Los detalles internos no están documentados públicamente **(sin verificar)**.
- **Fusion 360:** sobre ASM (Autodesk Shape Manager), con atributos persistentes en las entidades **(mecanismo interno sin verificar)**. Hay pérdidas de referencias documentadas en timelines largas (1.000-1.500 operaciones). Autodesk recomienda **"convertir a feature directa"** las partes que ya no se editan y evitar proyecciones asociativas ([foro Autodesk](https://forums.autodesk.com/t5/fusion-support-forum/references-repeatedly-lost/m-p/13190131)).
- **Zoo/KCL:** el usuario **etiqueta** aristas y caras en el código (`tag`) y navega relativamente (`getOppositeEdge`, `getNextAdjacentEdge`) ([docs Zoo](https://zoo.dev/docs/kcl-std/types/std-types-TaggedEdge)). Es explícito y robusto, pero solo es amigable para el modo código.

### Enfoque recomendado para Kubit (independiente del kernel)

1. **IDs persistentes generativos** en la capa `doc`, no en el kernel. Cada cara, arista o vértice recibe un `TopoName`, que es un árbol: `{ feature: FeatureId, role: Cap(Start|End) | Side(profile_edge_name) | Fillet(edge_name) | Split(parent, discriminator) | Imported(stable_hash) , sources: [TopoName] }`.
2. **Propagación por historia:** cada operación de la fachada devuelve `generated/modified/deleted`. Si el backend no lo da (truck no lo da), se reconstruye por **matching geométrico** (superficie portadora, normal, centroide y área, adyacencias) entre entrada y salida.
3. **Referencias como consultas semánticas** (al estilo Onshape) cuando sea posible, por ejemplo "la tapa final del Extrude#3" o "las aristas entre la cara X y la cara Y", en lugar de un `TopoName` concreto. La UI genera la consulta más estable a partir del clic.
4. **Resolución puntuada con fallback:** 1) nombre exacto; 2) descendientes (`split` → conjunto); 3) firma geométrica más cercana por encima de un umbral; 4) fallo → **reparación guiada** (la feature queda en amarillo, se resalta el candidato y el usuario lo confirma de un clic). Nunca hay un cambio silencioso de referencia.
5. **Modo directo como cortafuegos:** las operaciones directas (push/pull, mover cara) también se registran en la historia con nombres persistentes. "Congelar historia" (el "convert to DM" de Fusion) permite cortar dependencias largas a propósito.
6. **Test de regresión del naming:** el corpus de §1.4 incluye ediciones aguas arriba (cambiar el perfil de un extrude, de rectángulo a rectángulo con un chaflán extra) y comprueba que el fillet de aguas abajo sigue sobre la arista correcta.

---

## 7. UX: Shapr3D frente a Fusion 360 y el modelo híbrido

### 7.1 Lo que hace a Shapr3D rápido

- **Herramientas contextuales según la selección:** con una cara seleccionada ofrece offset, move, extrude y cut. Si se añade una arista, ofrece revolve o loft. Casi no hay barra de herramientas que recorrer ([AEC Magazine](https://aecmag.com/technology/review-shapr3d/)).
- **Push/pull directo:** arrastrar el glifo de una cara hacia dentro corta y hacia fuera extruye. La misma operación hace booleana y extrusión ([AEC](https://aecmag.com/technology/review-shapr3d/), [soporte Shapr3D](https://support.shapr3d.com/hc/en-us/articles/11883044707484)).
- **Entrada numérica inline:** al seleccionar aparecen cotas sugeridas (longitud, distancia, ángulo). Se teclea el valor y, si se bloquea, se convierte en restricción persistente. No hay diálogos.
- **Inferencia en el croquis:** tangencia, perpendicularidad, concentricidad, paralelismo y simetría se aplican automáticamente.
- **Pencil y táctil:** el dedo navega, el Pencil crea. Doble toque con el dedo alinea la vista a la cara; toque con el Pencil selecciona y ofrece offset; doble toque con el Pencil selecciona el cuerpo.
- **Kernel Parasolid:** las operaciones directas son fiables, cosa que un kernel Rust todavía no garantiza.

**Puntos débiles de Shapr3D** (comunidad): faltan planos 2D buenos (DWG, PDF, acotado automático), ensambles y movimiento, revolve o patrones 3D en algunas versiones y profundidad mecánica ([Capterra](https://capterra.com/p/184498/Shapr3D/reviews/)). Al añadir historia paramétrica (anunciada en jul-2023, [Develop3D](https://develop3d.com/cad/history-based-modelling-added-to-shapr3d/)), parte de sus usuarios se quejó de croquis y cotas por todas partes, de no poder borrar un croquis sin borrar el sólido y de archivos pesados y lentos ([foro Shapr3D](https://discourse.shapr3d.com/t/any-chance-of-a-release-of-the-old-version-as-a-legacy-version/31832)). **Lección para Kubit:** la historia no debe ensuciar el modo directo. Croquis "consumidos" ocultos por defecto, opción de desvincular y colapsar la historia.

### 7.2 Lo que hace a Fusion 360 preciso y potente

- **Timeline** editable con marcador para retroceder, reordenar y suprimir.
- **Parámetros de usuario con expresiones** y unidades, referenciables en cualquier campo.
- **Croquis con restricciones y cotas** completos, proyecciones y geometría de construcción.
- **Joints** (rígido, revoluta, deslizante…), contactos y estudios de movimiento.
- **Planos** asociativos, **CAM** integrado (Manufacture) y **simulación**.

**Puntos débiles de Fusion** ([hilo "Long list of bugs"](https://forums.autodesk.com/t5/fusion-support-forum/long-list-of-bugs-issues/m-p/13163496/highlight/true), [referencias perdidas](https://forums.autodesk.com/t5/fusion-support-forum/references-repeatedly-lost/m-p/13190131)):
- Mensajes opacos ("Failed to solve", "Constraints were removed"), sin decir qué choca.
- Puntos apilados difíciles de seleccionar; no se ve con claridad qué está infra-restringido.
- `mirror` y `offset` generan muchas restricciones sueltas.
- Las proyecciones se rompen al cambiar la fuente; se pierden referencias en timelines largas.
- Crear un parámetro obliga a salir de la tarea; hay diálogos que guardan el valor en lugar de la expresión; no se puede cambiar la unidad de un parámetro.
- La cara bajo el cursor se selecciona en lugar de la deseada.
- Los fallos de la timeline ralentizan aunque estén suprimidos.
- Hay crashes, prompts de upsell y dependencia de la nube.

### 7.3 Modelo de interacción híbrido recomendado para Kubit

1. **Un solo modo, con la historia siempre registrada:** como en Shapr3D, el usuario empieza directo (push/pull, mover cara, fillet al vuelo). La **timeline** está siempre ahí, colapsada en una barra fina inferior, y se expande cuando el usuario la necesita. Nada de "elegir modo" al crear el documento.
2. **Selección → acción:** clic en una cara o arista abre un **menú radial o flotante contextual** con las 4-6 operaciones válidas para esa selección, más la **paleta de comandos** (Cmd/Ctrl+K) con búsqueda difusa sobre el registro de comandos (la misma fuente que MCP).
3. **Manipuladores en el viewport + entrada numérica inline:** al arrastrar aparece la cota viva. Teclear sustituye el valor, y el valor admite **expresiones con unidades** (`=ancho/2`, `3/4 in`, `18mm+kerf`). Enter confirma y Tab pasa a la siguiente cota. **Cero diálogos modales**: los parámetros secundarios van en un panel lateral no bloqueante.
4. **Parámetros creados en línea:** escribir `=nuevo_param = 600 mm` en cualquier campo crea el parámetro sin salir de la tarea (corrige una queja directa de Fusion). Los parámetros guardan siempre la expresión.
5. **Croquis sobre cara con inferencia:** clic en una cara plana y "Sketch" (o empezar a dibujar sobre la cara con la herramienta activa) crea el plano automáticamente. Inferencias visibles con iconos temporales. **Colores de DOF** (azul infra, negro definido, rojo conflicto) y un panel de diagnóstico que dice **qué restricción** sobra y ofrece quitarla.
6. **Restricciones de alto nivel:** `mirror`, `pattern` y `offset` crean una **restricción compuesta** (una sola entidad editable), no 40 restricciones sueltas.
7. **Cotas "sugeridas" frente a "bloqueadas":** las cotas sugeridas (grises) son informativas. Al bloquearlas o teclearlas pasan a ser paramétricas. Así la pantalla no se llena de cotas.
8. **Errores accionables:** cada fallo de kernel o solver se muestra como un nodo amarillo o rojo en la timeline, con la causa en lenguaje claro, la geometría resaltada y botones de reparación (re-seleccionar referencia, suprimir, convertir a directo).
9. **Selección precisa:** ciclo de selección con Tab o clic largo entre las entidades bajo el cursor, filtros rápidos (cara, arista, cuerpo, croquis) y preselección con hover.
10. **Ensambles y joints al estilo Fusion,** pero creados por gesto: arrastrar una cara sobre otra propone un joint con alineación inferida.
11. **Fabricación como ciudadano de primera:** perfiles de proceso (CNC router, láser, impresión 3D, tablero de mueble) que activan validación en vivo (radios mínimos de fresa, espesores de pared, holguras, veta) con badges en el viewport. Nesting y planos en un clic desde la selección.
12. **IA (MCP) visible:** las acciones del agente aparecen en la timeline con su autor, se pueden deshacer y comparar (diff visual antes/después).
13. **Táctil y Pencil (fase 2):** separación dedo-navega / lápiz-crea al estilo Shapr3D en tablets y en la web táctil.

---

## 8. Resumen de licencias (semáforo)

| Componente | Licencia | Veredicto para Kubit Apache-2.0 |
|---|---|---|
| truck, monstertruck, solidcore, rmcp, egui, iced, Bevy, Xilem, faer, uom, Rhai, wasmtime, krilla, exmex, ruststep | Apache/MIT | Verde |
| lib3mf, binpack2d, sparrow, ezpz | MIT | Verde |
| jagua-rs, acadrust | MPL-2.0 | Amarillo. Usar sin modificar o publicar los cambios en esos archivos |
| OCCT (opencascade-rs, cadrum, occt-wasm) | LGPL-2.1 (+excepción) | Naranja. Solo como plugin o proceso dinámico, nunca estático en el core. En web, como módulo WASM separado |
| planegcs | LGPL **(sin verificar)** | Naranja. Evitar |
| Slint | GPL-3.0 / Royalty-free con restricciones | Naranja. Evitar |
| slvs (SolveSpace) | GPL-3.0 | Rojo |
| evalexpr ≥13 | **AGPL-3.0** | Rojo |
| tritet + TetGen | AGPL | Rojo. Triangle es de uso no comercial **(sin verificar)** |

Recomiendo añadir `cargo-deny` en CI con una lista de licencias permitidas (Apache-2.0, MIT, BSD, ISC, Zlib, MPL-2.0 con revisión manual) desde el primer commit.

---

## 9. Próximos pasos sugeridos (8 semanas)

1. **Semana 1-2:** esqueleto de crates (`geom`, `kernel-api`, `kernel-monstertruck`, `sketch`, `doc`, `engine`, `ui-egui`, `mcp`), CI con `cargo-deny` y build wasm32.
2. **Semana 2-4:** spike de kernels (§1.4) sobre monstertruck, truck y solidcore, con un informe comparativo.
3. **Semana 3-5:** prototipo de solver de croquis (ezpz como base) con análisis de DOF y arrastre con mínima perturbación.
4. **Semana 4-6:** viewport wgpu + push/pull con entrada numérica inline + menú contextual por selección.
5. **Semana 6-8:** registro de comandos → servidor MCP rmcp (stdio + HTTP); `TopoName` v0 con tests de regresión de naming.

---

## Fuentes

- [truck-modeling 0.6.0 (docs.rs)](https://docs.rs/truck-modeling)
- [truck-stepio (docs.rs)](https://docs.rs/truck-stepio)
- [monstertruck 0.4.1 (docs.rs)](https://docs.rs/monstertruck)
- [solidcore README (tangency-labs)](https://github.com/tangency-labs/solidcore)
- [SolidCraft CLAUDE.md (arquitectura)](https://raw.githubusercontent.com/tangency-labs/solidcraft/master/CLAUDE.md)
- [SolidCraft v0.15.0 (newreleases)](https://newreleases.io/project/github/tangency-labs/solidcraft/release/v0.15.0)
- [Hackaday: SolidCraft is parametric, automation-friendly CAD (17-oct-2026)](https://hackaday.com/2026/10/17/solidcraft-is-parametric-automation-friendly-cad-for-desktop-and-browser/)
- [opencascade-rs (repo)](https://github.com/mkovaxx/opencascade-rs)
- [cadrum (docs.rs)](https://docs.rs/crate/cadrum/0.4.3)
- [occt-wasm 5.0.1 (docs.rs)](https://docs.rs/crate/occt-wasm/latest)
- [Fornjot README (jsDelivr)](https://cdn.jsdelivr.net/gh/hannobraun/fornjot@main/README.md)
- [Foxtrot (Matt Keeter)](https://www.mattkeeter.com/projects/foxtrot)
- [ezpz 0.2.29 (docs.rs)](https://docs.rs/ezpz)
- [slvs (lib.rs)](https://lib.rs/crates/slvs)
- [planegcs WASM README](https://cdn.jsdelivr.net/npm/@salusoft89/planegcs@1.1.7/README.md)
- [eframe 0.37.0 (docs.rs)](https://docs.rs/crate/eframe/latest)
- [egui_dock 0.21.0 (docs.rs)](https://docs.rs/crate/egui_dock/0.21.0)
- [Iced 0.14 (Phoronix)](https://www.phoronix.com/news/Iced-0.14-Rust-GUI-LIbrary)
- [Slint: términos y licencias](https://slint.dev/terms-and-conditions)
- [Bevy news](https://bevy.org/news/)
- [xilem 0.4.0 (docs.rs)](https://docs.rs/crate/xilem/latest)
- [rmcp 3.1.0 README (docs.rs)](https://docs.rs/crate/rmcp/3.1.0/source/README.md)
- [rust-mcp-sdk 2.0.0](https://docs.rs/crate/rust-mcp-sdk/2.0.0)
- [ruststep 0.4.0 (docs.rs)](https://docs.rs/ruststep)
- [lib3mf 0.1.7 (docs.rs)](https://docs.rs/crate/lib3mf/latest)
- [acadrust (docs.rs)](https://docs.rs/crate/acadrust/0.4.1)
- [dxf (lib.rs)](https://lib.rs/crates/dxf)
- [krilla 0.8.2 (docs.rs)](https://docs.rs/crate/krilla/latest)
- [jagua-rs 1.1.0 (docs.rs)](https://docs.rs/jagua-rs)
- [Gardeyn et al., CDE / jagua-rs (arXiv 2508.08341)](https://arxiv.org/abs/2508.08341v1)
- [Gardeyn et al., sparrow (arXiv 2509.13329)](https://arxiv.org/pdf/2509.13329v1)
- [sparrow (gittrend)](https://gittrend.io/repo/JeroenGar/sparrow)
- [Sparrow Studio (McNeel forum)](https://discourse.mcneel.com/t/sparrow-studio-free-open-source-nesting-in-your-browser/222513)
- [binpack2d 1.0.1 (docs.rs)](https://docs.rs/binpack2d)
- [gdrr-2bp](https://github.com/JeroenGar/gdrr-2bp)
- [faer 0.24.4 (docs.rs)](https://docs.rs/faer)
- [fenris (docs.rs)](https://docs.rs/crate/fenris/latest)
- [gemlab (lib.rs)](https://lib.rs/crates/gemlab)
- [tritet 3.0.2 (docs.rs)](https://docs.rs/crate/tritet/3.0.2)
- [fTetWild-rs (Codeberg)](https://codeberg.org/phqb/fTetWild-rs)
- [evalexpr 13.1.0 (docs.rs)](https://docs.rs/crate/evalexpr/latest)
- [exmex 0.21.0 (docs.rs)](https://docs.rs/crate/exmex/latest)
- [fasteval (docs.rs)](https://docs.rs/crate/fasteval/latest)
- [uom 0.38.0 (docs.rs)](https://docs.rs/crate/uom/latest)
- [Rhai 1.27.0 (docs.rs)](https://docs.rs/crate/rhai/)
- [wasmtime v49 (newreleases)](https://newreleases.io/project/github/bytecodealliance/wasmtime/release/v49.0.0)
- [FreeCAD 1.0 y toponaming (Hackster)](https://hackster.io/news/freecad-hits-1-0-after-two-decades-finally-sees-an-end-to-the-toponaming-problem-7e7ac2df3e63)
- [FreeCAD 1.0 (AlternativeTo)](https://alternativeto.net/news/2024/11/freecad-1-0-launches-with-enhanced-ui-ux-built-in-assembly-workbench-and-tnp-fixes/)
- [Foro Onshape: estabilidad de referencias](https://forum.onshape.com/discussion/comment/118336)
- [Zoo KCL: TaggedEdge](https://zoo.dev/docs/kcl-std/types/std-types-TaggedEdge)
- [Foro Autodesk: References Repeatedly Lost](https://forums.autodesk.com/t5/fusion-support-forum/references-repeatedly-lost/m-p/13190131)
- [Foro Autodesk: Long list of bugs/issues](https://forums.autodesk.com/t5/fusion-support-forum/long-list-of-bugs-issues/m-p/13163496/highlight/true)
- [AEC Magazine: Review Shapr3D](https://aecmag.com/technology/review-shapr3d/)
- [Shapr3D: conceptos de modelado 3D](https://support.shapr3d.com/hc/en-us/articles/11883044707484)
- [Capterra: reseñas de Shapr3D](https://capterra.com/p/184498/Shapr3D/reviews/)
- [Develop3D: historia paramétrica en Shapr3D](https://develop3d.com/cad/history-based-modelling-added-to-shapr3d/)
- [Foro Shapr3D: quejas por la nueva versión](https://discourse.shapr3d.com/t/any-chance-of-a-release-of-the-old-version-as-a-legacy-version/31832)
