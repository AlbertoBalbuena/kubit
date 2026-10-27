# ADR-0003 — Fachada del kernel, guard, nombres persistentes v0 y formato `.kubit` v1

- Estado: **Propuesto** (implementado en Fase 0; pendiente de revisión del mantenedor)
- Fecha: 2026-10-27
- Relacionado: ADR-0001 (#6), ADR-0002 (backend inicial)

## Contexto
Ningún kernel B-rep en Rust es tan robusto como Parasolid u OCCT. Algunos tienen panics y casi ninguno tiene historia topológica nativa. El producto necesita tres cosas: cambiar de backend sin reescribir capas superiores, no caerse nunca por un panic del kernel y reproducir la misma geometría desde el mismo documento.

## Decisiones

### 1. Fachada `kubit-kernel-api`
- El trait `Kernel` es *object-safe*, así que se puede usar como `Box<dyn Kernel>`. Cada operación tiene una implementación por defecto que devuelve `KernelError::Unsupported`. Un backend implementa solo lo que de verdad soporta y lo declara en `Capabilities`.
- Los cuerpos, caras y aristas se identifican con IDs opacos (`BodyId`, `FaceId`, `EdgeId`) que asigna el backend. Nada fuera de `kubit-kernel-*` ve tipos del kernel.
- Cada operación devuelve `OpOutput { body, history: OpHistory }`. `OpHistory` registra las entidades generadas, modificadas y borradas, y su origen: `Native` si la reporta el kernel, `Reconstructed` si la reconstruye la fachada. Se guarda en `BTreeMap` para que sea determinista.

### 2. Guard (`Guarded<K>`)
Toda llamada al backend pasa por `Guarded`, que hace cuatro cosas:
1. **Valida las entradas.** Los números deben ser finitos, con |x| ≤ 1e6 mm y longitudes de al menos 1e-6 mm. Las direcciones deben ser unitarias y la tolerancia cordal debe estar en rango. Una entrada fuera de rango se rechaza antes de tocar el kernel.
2. **Atrapa los panics** con `catch_unwind`, los convierte en `KernelError::Panicked` y **envenena** el kernel. Después de un panic no se confía en su estado: el motor crea un kernel nuevo (`KernelFactory`) y reproduce el documento.
3. **Valida el resultado**: el cuerpo debe pasar el *checker* del backend y tener volumen positivo. Si no lo cumple, la operación falla de forma limpia y la transacción se revierte.
4. **Reconstruye la historia** cuando el backend no la da. Empareja las caras de antes y de después por tipo de superficie, normal, centroide y área, con tolerancias.

**Limitación conocida:** en `wasm32-unknown-unknown` los panics abortan y `catch_unwind` no los atrapa. En web, la validación de entradas es la única defensa (ver riesgos en ADR-0002).

### 3. Documento y nombres persistentes v0
- El documento es la fuente de verdad: parámetros con sus expresiones (`IndexMap`, mantiene el orden) y un timeline de `Feature { id, command, params, author, suppressed }`. La geometría **no se guarda**: se reconstruye reproduciendo el timeline.
- Cada nombre persistente (`TopoName`) se deriva de la feature que creó la cara:
  - `F1:+Z`: rol primitivo; para una caja se deriva de la normal.
  - `F3:g2`: cara generada número 2 de la feature 3. Se numera en orden geométrico: centroide y área cuantizados a 1 nm.
  - `F1:+Z/1`: segundo pedazo de `F1:+Z` cuando una operación la parte.

  Los nombres se propagan a través de `OpHistory`.
- v0 cubre lo que hace falta en Fase 0 (caja y push/pull). La v1, con firmas más ricas y pruebas de regresión, es trabajo de la Fase 1.

### 4. Formato `.kubit` v1
- Es un ZIP con entradas *Stored* (sin compresión) y fecha fija (1980-01-01), en orden fijo. El mismo documento produce **los mismos bytes**.
- Entradas:
  - `manifest.json`: formato, versión y generador;
  - `document.json`;
  - `catalog-snapshot.json`;
  - `thumbnail.png`, opcional.
- Al abrir se aplican límites defensivos: 64 MiB por entrada y 4096 entradas como máximo. También se rechazan las versiones futuras.
- No se usa `serde_json::Value` intermedio para el documento, porque ordena las claves y se perdería el orden de los parámetros.

### 5. Motor
- `Session::execute(id, params)` es la única entrada para UI, CLI, scripts y MCP. Los parámetros se validan con el JSON Schema del comando (`schemars`).
- Cada comando que muta es una transacción: si falla, el modelo y el documento quedan como estaban. Los cuerpos huérfanos del kernel se liberan (GC).
- Undo/redo por *snapshots* del documento y del modelo, con límite `UNDO_LIMIT`.

## Consecuencias
- Cambiar de backend significa escribir un crate `kubit-kernel-<x>`. Ni el motor ni el documento cambian.
- La historia reconstruida es heurística y puede equivocarse con caras simétricas o casi coincidentes. Por eso cada operación declara su `source`, y las pruebas de nombres van a la Fase 1.
- Reproducir el timeline al abrir cuesta tiempo en documentos grandes. Un caché de geometría en el `.kubit` queda para más adelante y obligará a subir la versión del formato.
