# ADR-0005 — Solver de croquis propio (`kubit-sketch`)

- Estado: **Propuesto** (implementado en el hito 1.4a; pendiente de revisión del mantenedor)
- Fecha: 2026-10-27
- Relacionado: ADR-0003 (fachada y documento), `docs/research/tecnologia-rust-cad.md` §2

## Contexto
El croquis es la entrada de casi todo el modelado paramétrico. Lo que distingue a un buen sketcher es el UX:
- que la geometría no salte al arrastrar;
- que diga cuántos grados de libertad quedan y dónde;
- que diga **qué** restricción sobra o choca, en lugar de un "Failed to solve".

Opciones revisadas en la investigación:

| Opción | Problema |
|---|---|
| **slvs** (SolveSpace) | GPL-3.0, prohibido |
| **planegcs** (FreeCAD) | LGPL y código de FreeCAD; prohibido, ni como referencia |
| **ezpz** 0.2.29 | MIT, apto. Su diagnóstico y su arrastre los define otro producto, y depende de `faer` |

## Decisión
**Solver propio** en el crate `kubit-sketch` (L1, compila a wasm, sin dependencias de UI ni de kernel). ezpz queda como referencia de lectura; si se adapta código, se registra en `ATTRIBUTION.md`. Por ahora no se adaptó nada.

1. **Modelo** (`model.rs`):
   - **Entidades:** punto, línea, círculo y arco antihorario (centro, inicio, fin).
   - **Restricciones** (18): coincidente, horizontal, vertical, paralela, perpendicular, tangente, igual, punto sobre línea, punto sobre círculo, punto medio y fijar.
   - **Cotas:** distancia, distancia horizontal y vertical, longitud, radio, diámetro y ángulo.
   - **Identificadores y orden:** IDs crecientes que nunca se reutilizan, guardados en `IndexMap` (determinista y en orden de inserción).
   - **Cotas de referencia:** se miden pero no se imponen.
   - **Validación:** cada edición valida tipos y números (finitos, |x| ≤ 1e6).
2. **Ecuaciones** (`equations.rs`):
   - **Incógnitas:** las coordenadas de cada punto y el radio de cada círculo. El arco deriva su radio de sus puntos y lleva una ecuación interna para que el fin quede en su círculo.
   - **Residuos:** se escalan a milímetros, así que una sola tolerancia (1e-9 mm) sirve para todos. Los de ángulo se multiplican por la longitud media de las líneas.
3. **Jacobiano exacto** con diferenciación automática hacia adelante (`dual.rs`, números duales sobre ≤ 9 variables locales). Cada restricción se escribe una sola vez, sin derivadas a mano ni diferencias finitas.
4. **Resolución** (`solve.rs`):
   - descomposición en componentes conexos (union-find), y en cada uno Levenberg-Marquardt sobre `nalgebra` 0.35 (Apache-2.0, ya presente vía arris), con amortiguamiento adaptativo;
   - el amortiguamiento da pasos de norma mínima, así que un croquis subrestringido se mueve lo menos posible.
5. **Arrastre:**
   - primero un solve con ecuaciones blandas: el punto arrastrado hacia el cursor (peso 1) y el resto hacia donde estaba (peso 0.01);
   - después un solve de las restricciones duras solas, para que el resultado sea exacto.
6. **Diagnóstico:**
   - DOF = variables − rango(J), con SVD;
   - las restricciones se agregan en orden: la que no sube el rango es **redundante** (si se cumple) o **en conflicto** (si no se cumple). Se nombra la última que se agregó, que es la que el usuario puso de más;
   - la libertad por entidad (`Fully`, `Under(n)` u `Over`) sale del espacio nulo del jacobiano restringido a sus variables;
   - arriba de 400 variables se omite el diagnóstico por restricción, porque cuesta O(n³) por paso; el conteo de DOF sigue siendo exacto.
7. **Perfiles** (`profile.rs`):
   - las líneas y arcos que comparten puntos (o tienen puntos coincidentes) forman un grafo, y cada parte donde todos los nodos tienen grado 2 es un lazo cerrado;
   - cada círculo es un lazo por sí solo;
   - el anidamiento por contención define exterior y agujeros;
   - el id de una región es el menor id de entidad de su lazo exterior;
   - el área es exacta: polígono más segmentos circulares;
   - la geometría de construcción se ignora;
   - las ramificaciones y las cadenas abiertas no forman perfil en v1.
8. **Formato de texto** (`text.rs`) para fixtures y reportes de errores (`point p1 0 0`, `length l1 30`, `angle l1 l2 60 ref`).

## Consecuencias
- **Ventajas:**
  - el diagnóstico y el arrastre son nuestros y se pueden afinar para el UX;
  - no hay dependencias nuevas pesadas;
  - el croquis se guarda en los `params` de su feature como JSON (serde), así que el formato `.kubit` no cambia.
- **Límites conocidos de v1:**
  - **Tangencia:** es una raíz doble cuando ya la implican otras restricciones, así que converge solo a ~√tolerancia (unos 1e-5 mm). Es aceptable para fabricación; en v2 conviene detectarla y tratarla como redundante.
  - **Solo componentes conexos:** todavía no hay descomposición en clusters rígidos. Los croquis grandes pueden ser lentos de diagnosticar.
  - **Splines y elipses:** quedan fuera.
- **Pruebas:**
  - oráculos: triángulo 3-4-5, rectángulo con DOF 0, arco de empalme tangente, círculo inscrito, ángulo de 60°;
  - diagnóstico: restricción redundante y cota en conflicto;
  - arrastre con mínima perturbación;
  - perfiles: rectángulo con agujero, ranura con arcos (área exacta) y construcción;
  - proptest con números hostiles, con cero panics.
