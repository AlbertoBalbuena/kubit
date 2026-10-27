# ADR-0004 — Nombres de arista v0 y aristas B-rep en la malla

- Estado: **Propuesto** (implementado en el hito 1.2; pendiente de revisión del mantenedor)
- Fecha: 2026-10-27
- Relacionado: ADR-0003 (nombres persistentes v0 de cara), ADR-0002 (backend arris)

## Contexto
Fillet y chamfer trabajan sobre aristas, y en el hito 1.2 el usuario debe poder seleccionarlas en el viewport, guardarlas en un script y reencontrarlas después de reconstruir el documento. El ADR-0003 solo nombra caras. Hacen falta tres cosas:

1. que la malla traiga las aristas B-rep reales, para dibujarlas y hacer picking (las aristas por ángulo de `feature_edges` no saben qué arista del kernel son);
2. que la fachada diga qué caras se tocan en cada arista;
3. un nombre de arista que sobreviva a las reconstrucciones.

## Decisiones

### 1. Aristas en `kubit_mesh::TriMesh`
- `TriMesh` gana `edges: Vec<(EdgeTag, Vec<[f64; 3]>)>`: una polilínea por arista B-rep, en el orden de aristas del kernel. `EdgeTag(u64)` es el `EdgeId` del kernel, igual que `FaceTag` es el `FaceId`.
- El campo tiene `#[serde(default)]`, así que una malla sin aristas (STL o hecha a mano) sigue siendo válida y deserializa igual.
- `TriMesh::validate` rechaza polilíneas con menos de dos puntos o con coordenadas no finitas (`MeshDefect::BadEdge`). El guard ya valida toda malla que sale de `tessellate`.
- arris llena las polilíneas desde `arris_mesh::TriMesh::edges()` y `edge_polyline()`. Los puntos son vértices de la propia malla, así que la arista coincide exactamente con el borde de los triángulos.

### 2. `Kernel::edge_faces(body, edge) -> [FaceId; 2]`
- Devuelve las dos caras de `body` que se tocan en `edge`. Una costura (*seam*) devuelve la misma cara dos veces. La implementación por defecto devuelve `Unsupported`, como el resto de la fachada.
- **Recibe el cuerpo, no solo la arista.** arris comparte las entidades que una operación no cambia entre el cuerpo de entrada y el de salida. Medido: después de un push/pull de la cara superior de una caja, las cuatro aristas inferiores siguen siendo las mismas, pero sus caras laterales son nuevas. Sin el cuerpo, la respuesta sería ambigua (devolvería las caras del cuerpo viejo). El plan aprobado decía `edge_faces(EdgeId)`; el parámetro extra es la corrección mínima.
- En arris sale de `Model::edge_uses`, filtrado por las caras del cuerpo.

### 3. Nombre de arista v0
- Una arista se nombra por los `TopoName` de sus dos caras vecinas, **ordenados** y unidos con `|`: `F1:+X|F1:+Z`. El orden es el de `TopoName` (número de feature; dentro de una feature, primero los roles de primitiva, luego las caras generadas y luego los pedazos). Al parsear se aceptan las dos caras en cualquier orden, y al mostrar siempre salen ordenadas.
- El modelo del motor calcula `BodyState::edges: BTreeMap<EdgeName, EdgeId>` cada vez que reconstruye un cuerpo, a partir de los nombres de cara ya propagados. No hay historia de aristas aparte: si los nombres de cara son estables, los de arista también.
- Una arista queda **sin nombre** cuando:
  - alguna de sus caras no tiene nombre;
  - el backend no sabe responder para esa arista;
  - otra arista del mismo cuerpo produce el mismo nombre (dos caras que se tocan en más de una arista, por ejemplo después de un corte que parte una cara en U).

  Si el backend no implementa `edge_faces`, ninguna arista tiene nombre y el modelado sigue funcionando. Un panic, un kernel envenenado o una interrupción sí hacen fallar la feature, igual que en el resto del motor.

### 4. Selección y comandos
- `selection.set` acepta `{ faces: [...], edges: [...] }`. Los dos campos son opcionales, así que los scripts viejos (`{ faces: [...] }`) siguen funcionando. La selección no se guarda en el `.kubit`, así que el formato no cambia.
- `Session::edge_name(body, EdgeTag)` y `Session::body_edges(body, chord)` son el equivalente de `face_name` y `body_mesh` para aristas.
- `doc.inspect` reporta `selected_edges` y el número de aristas con nombre de cada cuerpo.

## Consecuencias
- El renderer puede dibujar las aristas reales y hacer picking sobre ellas. `feature_edges` queda como respaldo para mallas sin aristas.
- Los nombres de arista heredan los límites de los nombres de cara (ADR-0003): si una cara cambia de nombre, sus aristas también.
- Las aristas ambiguas no se pueden seleccionar por nombre en v0. La v1 de nombres (Fase 1, "Nombres persistentes v1") debe desambiguarlas, por ejemplo con un ordinal geométrico (`F1:+X|F2:g0#1`) o con los vértices extremos, y debe nombrar vértices.
- `edge_faces` recorre las caras del cuerpo por cada arista, así que nombrar las aristas de un cuerpo cuesta O(aristas × caras). Es suficiente para las piezas de la Fase 1; si se vuelve un cuello de botella, la fachada puede ganar una consulta por lote.
