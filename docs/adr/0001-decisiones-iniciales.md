# ADR-0001 — Decisiones iniciales del proyecto

- Estado: **Aceptado**
- Fecha: 2026-10-26
- Decide: Alberto Balbuena

## Contexto
Kubit arranca como un proyecto open source de largo plazo: un CAD directo y paramétrico en Rust, con validación de fabricación. Varias decisiones condicionan todo lo demás y conviene fijarlas desde el inicio.

## Decisiones

| # | Decisión | Razón |
|---|---|---|
| 1 | **Nombre: Kubit** (no "Kubit Studio" ni "Kubit Design"). Ecosistema: Kubit Catálogo, Kubit Hub | Corto, sirve como comando y prefijo de crates, no se confunde con Evita Studio. ⚠ Pendiente revisar marca (kubit GmbH/FARO, Sandia CUBIT) |
| 2 | **Código Apache-2.0; datos del catálogo CC-BY-4.0; contribuciones con DCO** | Concesión de patentes, compatible con truck, monstertruck, solidcore y SolidCraft (Apache-2.0) |
| 3 | **Clean room respecto a Evita Studio** | Evita Studio es propiedad de Evita Cabinets. Kubit no copia ni consulta su código |
| 4 | **Política de licencias de dependencias**: nada GPL, LGPL o AGPL en el core. Se verifica con `cargo deny`. OCCT solo como proceso o módulo separado. No se lee código GPL/LGPL (FreeCAD, SolveSpace, OCCT) para escribir código del core | Mantener Kubit embebible y la licencia limpia |
| 5 | **Kubit es el modelador del toolkit ARD** | Integración por archivo `.kubit` y API, con acoplamiento débil |
| 6 | **Kernel detrás de una fachada propia**; backend inicial decidido por spike (ADR-0002) | Ningún kernel Rust tiene hoy la robustez de Parasolid u OCCT; la fachada permite cambiar |
| 7 | **UI: egui/eframe + egui_dock + viewport wgpu**, capa UI reemplazable | Única opción madura con escritorio y web; Slint descartado por licencia |
| 8 | **MCP con rmcp**, tools generadas del registro de comandos | Una sola fuente de verdad para UI, CLI e IA |
| 9 | **Catálogo en TOML + JSON Schema**, en un repo aparte, publicado en Cloudflare | Editable por la comunidad, versionado y offline-first |
| 10 | **Idioma**: código, identificadores, comentarios y commits en inglés; documentación del proyecto en español por ahora; UI con i18n es/en | Alcance internacional del código y velocidad para el mantenedor. Traducir docs más adelante |
| 11 | **Plataformas**: macOS, Windows y Linux desde el día 1 (CI); web en la Fase 6 | Las capas bajas compilan a wasm32 desde el inicio |
| 12 | **Vertical primero: mobiliario y millwork** (Fase 2), luego metal, superficies y el resto | Dominio que el mantenedor conoce; MVP útil más pronto |
| 13 | **Asistencia, no certificación**: toda validación DFM o de simulación lleva fuente y disclaimer | Responsabilidad y honestidad |

## Consecuencias
- Se pueden reutilizar código y patrones de SolidCraft, truck, monstertruck y solidcore, siempre con atribución en `ATTRIBUTION.md`.
- Hay que construir solver de croquis, evaluador de expresiones y nombres persistentes propios. Es más trabajo, pero da control y licencia limpia.
- La revisión de marca puede forzar un cambio de nombre; por eso conviene hacerla antes de la primera release pública.
