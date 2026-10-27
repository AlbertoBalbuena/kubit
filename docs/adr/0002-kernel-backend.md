# ADR-0002 — Backend inicial del kernel

- Estado: **Propuesto**. Ya se usa de forma provisional en `main` (`kubit-kernel-arris`) bajo la autorización de trabajo autónomo; falta que el mantenedor lo apruebe.
- Fecha: 2026-10-27
- Relacionado: ADR-0001 (#6), ADR-0003 (fachada y guard), `spikes/kernel/`

## Contexto
Kubit habla con el kernel solo a través de `kubit-kernel-api` (ADR-0003). Para elegir el primer backend se midieron cuatro candidatos con licencia permisiva y el corpus de 16 pruebas de `docs/research/tecnologia-rust-cad.md` §1.4:

| Backend | Versión medida | Licencia |
|---|---|---|
| monstertruck | 0.4.1 (`monstertruck-*`) | Apache-2.0 |
| truck | `truck-modeling` 0.6.0 / `truck-shapeops` 0.4.0 / `truck-stepio` 0.3.0 | Apache-2.0 |
| solidcore | git `tangency-labs/solidcore` @ `dbaedda` (el 0.0.1 de crates.io está desactualizado) | Apache-2.0 |
| arris | 0.9.1 (`arris`) | MIT OR Apache-2.0 |

## Método
- **Spike aislado** en `spikes/kernel/`, con su propio workspace:
  - un adaptador por backend;
  - corpus de 26 sub-casos con **oráculo analítico** (`harness/src/corpus.rs`; contrato geométrico en `CASES.md`);
  - 10 STEP públicos del NIST (`fixtures/step/fetch-nist.sh`).
- **Cada caso corre en un subproceso**, así que un panic o un abort no contamina a los demás. Clasificación: ok / error limpio / no expuesto / panic / crash / timeout. Límite de 300 s por caso; tiempo = mediana de 5 repeticiones (menos si una pasada tarda más de 10 s). Corrida en serie en un Apple M5, release.
- **Validez:** se combinan tres chequeos:
  - checker del backend (si existe);
  - cascarón cerrado y orientado sobre la malla soldada;
  - Euler-Poincaré.
- **Volumen:** propiedades de masa nativas si el backend las expone; si no, divergencia sobre un teselado fino. En la tabla, "(mesh)" indica el segundo método.
- **Entradas hostiles:** 256 conjuntos deterministas por familia (NaN, ±∞, 0, negativos, 1e-308, 1e9) en los casos 1, 5 y 9.
- **wasm32:** cada adaptador se compila con su sonda (`wasm/<backend>/`) y los casos 1–11 se ejecutan en Node.
- **No se emuló ninguna operación ausente:** se registra como N/E.

Datos crudos: `spikes/kernel/results/results.json` y `wasm.json`. Tabla completa por sub-caso: `spikes/kernel/results/table.md`. API de cada kernel: `spikes/kernel/NOTES.md` y `backends/*/NOTES.md`.

## Evidencia (resumen 16 × 4)
Leyenda: ✅ correcto · ⚠️ terminó, pero el resultado es inválido o se sale de tolerancia · ❌ error limpio · N/E no expuesta · 💥 panic · ⏱ timeout · ↷ omitido porque falló el caso fuente. εV es el peor error relativo de volumen.

| # | Prueba | monstertruck | truck | solidcore | arris |
|---|---|---|---|---|---|
| 1 | Caja − cilindro | ✅2 · εV 6.9e-6 · 89 ms | ⚠️2 · εV 1.0e-5 | ✅2 · εV 5.9e-10 · 10 ms | ✅2 · εV 1.3e-16 · 11 ms |
| 2 | Booleana coplanar (cara / arista) | ❌2 | ❌2 | ✅1 ⚠️1 | ⚠️1 ❌1 |
| 3 | Booleana tangente | ✅1 ❌1 | ⚠️1 💥1 | ✅1 ⚠️1 | ❌2 |
| 4 | Unión de 100 cilindros | ❌ | ⏱ | ✅ 1.19 s | ✅ 0.64 s · εV 1.6e-15 |
| 5 | Fillet (arista, cadena, vértice, variable) | ✅2 ⚠️1 ❌1 | N/E4 | ⚠️4 (checker: inválido) | ✅3 N/E1 (radio variable) |
| 6 | Fillet después de booleana | ❌ | N/E | ⚠️ | ✅ |
| 7 | Chaflán (d-d, d-ángulo) | ✅1 N/E1 | N/E2 | ⚠️1 N/E1 | ✅1 N/E1 (d-ángulo) |
| 8 | Shell | N/E2 | N/E2 | ✅2 | ✅2 · εV 1e-15 |
| 9 | Push/pull de cara con fillets | N/E2 | N/E2 | N/E2 | ✅2 · εV 1.9e-16 · 5.6 ms |
| 10 | Sweep por hélice / loft | ✅2 N/E1 | ✅2 N/E1 | ✅2 ⚠️1 | ✅3 · εV 6.1e-13 |
| 11 | Revolve que toca el eje | ✅2 · εV 2.3e-4 | ⚠️2 | ✅2 · εV 4.1e-8 | ✅2 · εV 4.3e-16 |
| 12 | STEP: round-trip / import NIST | RT ✅9 ❌1 ↷10 / imp ✅6 ⚠️1 ❌3 | RT ✅2 ⚠️2 ⏱3 / imp ❌10 | RT N/E (no exporta B-rep curvo) / imp ⚠️6 ❌4 | RT ✅17 ⚠️1 ↷2 / imp ✅5 ⚠️4 ❌1 |
| 13 | Teselado watertight | ✅3 | ✅1 ⚠️2 | ✅3 (con la malla propia del spike) | ✅3 |
| 14 | wasm32: compila / tamaño gzip / casos en Node | ✅ 799 KiB¹ · 6/18 ok | ✅ 339 KiB · 3/18 ok, 1 trap | ✅ 4.9 MiB · 15/18 ok | ✅ 1.4 MiB · 13/18 ok, 0 traps |
| 15 | Hostiles (768 conjuntos) | 5: **4 💥** · 1: ⏱ | 1: ⏱ | 0 💥 (9: N/E) | **0 💥** en 1, 5 y 9 |
| 16 | Historia | no | no | no | **nativa** (generadas/modificadas/borradas) |

¹ La medida de monstertruck es antes de wasm-bindgen y sin wasm-opt, así que no es directamente comparable. arris y solidcore se midieron después de wasm-bindgen, con LTO.

Notas de lectura:
- **arris 2a** marca ⚠️ solo porque no funde las caras coplanares: queda con 10 caras en vez de 6. El sólido es válido y el volumen es exacto.
- **solidcore** da volúmenes precisos, pero su propio checker marca inválidos los fillets. No tesela B-rep ni exporta STEP de superficies curvas, y no tiene edición directa de caras.
- **arris NIST:** 5 archivos importan válidos. 3 importan con defectos (no-variedad, aristas frontera) que el checker reporta. 1 contiene `TESSELLATED_SOLID`, que el lector no soporta, y lo rechaza limpiamente. Ninguno provoca panic.

## Decisión
**Backend inicial: arris 0.9.1**, en el crate `kubit-kernel-arris`, detrás de `Guarded`.

Razones medidas:
1. Es el **único con push/pull (`offset_faces`)**, que es la operación central de un CAD directo. Además funciona sobre caras con fillets (caso 9).
2. Es el **único con historia nativa** (provenance). Los nombres persistentes salen de datos del kernel, no de heurísticas.
3. Tiene **propiedades de masa exactas** (εV ≈ 1e-15) y checker propio. Eso hace posibles las pruebas oráculo y la validación de resultados del guard.
4. Hace **round-trip STEP AP214** de los 17 casos que exporta, y es el que mejor importa los STEP de NIST.
5. Tiene **cero panics** con entradas hostiles y **cero traps** en wasm. Es lo que más importa en web, donde `catch_unwind` no funciona.
6. Licencia MIT OR Apache-2.0. Compila a wasm32, con 1.4 MiB gzip.

**solidcore** queda como segunda opción: es el mejor en booleanas difíciles (casos 2 a 4). Se reevaluará cuando tenga teselado, exportación STEP y edición de caras. monstertruck y truck quedan descartados por ahora: panics, booleanas frágiles, sin shell ni offset ni historia.

## Riesgos
- **Booleanas degeneradas:** con contacto tangente o por arista, arris devuelve un error limpio en vez de un resultado (2b, 3a, 3b). El usuario verá "no se pudo" en esos casos. Mitigación: mensajes claros y, más adelante, reintentar con una perturbación mínima documentada.
- **Proyecto joven:** arris está en 0.x y tiene poca adopción. Su API puede cambiar entre 0.x. Mitigación: versión fijada, fachada (ADR-0003) y el spike como test de regresión al actualizar.
- **STEP:** todavía no importa sólidos teselados (AP242) y algunos archivos reales salen con defectos de topología. Mitigación: el guard rechaza resultados inválidos; reparación (healing) en una fase posterior.
- **wasm:** los panics abortan y el guard no puede atraparlos. La validación de entradas es la única defensa en web.
- **Rendimiento:** importar STEP grandes es lento (ctc_02: 60 s, ftc_07: 31 s). Debe correr fuera del hilo de UI y poder cancelarse (`interrupt`).

## Operaciones que Kubit implementará por su cuenta
| Operación | Motivo |
|---|---|
| Fillet de radio variable (5d) | arris solo acepta radio constante |
| Chaflán distancia-ángulo (7b) | arris solo hace distancias iguales; se puede derivar como d₁-d₂ con d₂ = d₁·tan θ |
| Fusión de caras coplanares tras booleanas | arris deja caras partidas (2a). Hace falta para nombres limpios y para la DFM |
| Nombres persistentes v1 | sobre la provenance de arris (ADR-0003 §3) |
| Reconstrucción de historia como respaldo | `Guarded` ya la hace para backends sin historia nativa |
| Reparación de STEP importado | arris reporta los defectos pero no los cura |

## Consecuencias
- `apps/kubit` y `apps/kubit-cli` usan `ArrisKernel`. Cambiar de backend significa cambiar solo `backend.rs` y escribir otro `kubit-kernel-<x>`.
- El spike se queda en el repo como banco de regresión: hay que volver a correrlo al subir de versión a arris o al evaluar otro backend.
