# Resultados del spike de kernels

Generado por `spike report` a partir de `results.json`. Leyenda: ✅ correcto · ⚠️ terminó pero el resultado es inválido o el volumen se sale de tolerancia · ❌ error limpio · N/E operación no expuesta · 💥 panic · ☠️ crash del proceso · ⏱ timeout · ↷ omitido (el caso fuente falló). εV = peor error relativo de volumen; tiempo = mediana de los casos correctos.

## Resumen: 16 pruebas × backends

| # | Prueba | monstertruck | truck | solidcore | arris |
|---|---|---|---|---|---|
| 1 | Caja − cilindro (pasante, ciego) | — | — | — | ✅2 · εV≤1.3e-16 · 9.4 ms |
| 2 | Booleana coplanar (cara, arista) | — | — | — | ⚠️1 ❌1 · εV≤0 · 299 µs |
| 3 | Booleana tangente (cilindro, esfera) | — | — | — | ❌2 |
| 4 | Unión de 100 cilindros | — | — | — | ✅1 · εV≤1.6e-15 · 665.8 ms |
| 5 | Fillet (arista, cadena, vértice, variable) | — | — | — | ✅3 N/E1 · εV≤2.7e-16 · 400 µs |
| 6 | Fillet después de booleana | — | — | — | ✅1 · εV≤0 · 14.8 ms |
| 7 | Chaflán (d-d, d-ángulo) | — | — | — | ✅1 N/E1 · εV≤0 · 133 µs |
| 8 | Shell (caja, cilindro con fillet) | — | — | — | ✅2 · εV≤1.0e-15 · 16.9 ms |
| 9 | Push/pull de cara fileteada | — | — | — | ✅2 · εV≤1.9e-16 · 5.1 ms |
| 10 | Sweep hélice / loft | — | — | — | ✅3 · εV≤6.1e-13 · 2.7 ms |
| 11 | Revolve tocando el eje | — | — | — | ✅2 · εV≤4.3e-16 · 101 µs |
| 12 | STEP round-trip + import NIST | RT — / import — | RT — / import — | RT — / import — | RT ✅17 ⚠️1 ↷2 · εV≤6.1e-13 / import — |
| 13 | Teselado watertight | — | — | — | ✅3 · 79 µs |
| 14 | Build wasm32 | — | — | — | — |
| 15 | Hostiles (proptest) en 1, 5, 9 |  |  |  | 1: 0💥/256 · 5: 0💥/256 · 9: 0💥/256 |
| 16 | Historia en 1, 5, 9 | — | — | — | 1a: nativa g6/m4/d10; 5a: nativa g9/m10/d3; 9a: nativa g0/m35/d0 |

## Detalle por sub-caso

| Caso | Backend | Resultado | Tiempo | Volumen (método) | εV | εA | Válido | V/E/F/L/S | Mensaje |
|---|---|---|---|---|---|---|---|---|---|
| 1a | arris | ✅ | 9.4 ms | 227433.629 (native) | 1.3e-16 | 6.8e-16 | sí b0 nm0 mo0 χ✓ | 10/15/7/9/1 |  |
| 1b | arris | ✅ | 6.8 ms | 233716.815 (native) | 1.2e-16 | 5.6e-16 | sí b0 nm0 mo0 χ✓ | 10/15/8/9/1 |  |
| 2a | arris | ⚠️ | 299 µs | 480000.000 (native) | 0 | 3.2e-16 | sí b0 nm0 mo0 χ✓ | 12/20/10/10/1 |  |
| 2b | arris | ❌ |  |  |  |  |  |  | fuse: degenerate result: the result's shells would touch along an edge or at a vertex, which a solid does not hold (+e6) |
| 3a | arris | ❌ |  |  |  |  |  |  | fuse: degenerate result: the faces touch along a curve interior to both result faces (+f1, +f7) |
| 3b | arris | ❌ |  |  |  |  |  |  | fuse: no closed form for +f1 (plane surface) against +v8 (point) |
| 4 | arris | ✅ | 665.8 ms | 500530.965 (native) | 1.6e-15 |  | sí b0 nm0 mo0 χ✓ | 208/312/206/306/1 |  |
| 5a | arris | ✅ | 175 µs | 239463.495 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 5b | arris | ✅ | 5.4 ms | 108418.763 (native) | 2.7e-16 |  | sí b0 nm0 mo0 χ✓ | 12/20/10/10/1 |  |
| 5c | arris | ✅ | 400 µs | 238947.916 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 13/21/10/10/1 |  |
| 5d | arris | N/E |  |  |  |  |  |  | variable-radius fillet (ops::fillet takes one constant radius) |
| 6 | arris | ✅ | 14.8 ms | 227377.285 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 11/17/8/10/1 |  |
| 7a | arris | ✅ | 133 µs | 238750.000 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 7b | arris | N/E |  |  |  |  |  |  | distance–angle chamfer (ops::chamfer is equal-distance only) |
| 8a | arris | ✅ | 430 µs | 35712.000 (native) | 1.0e-15 |  | sí b0 nm0 mo0 χ✓ | 16/24/11/12/1 |  |
| 8b | arris | ✅ | 16.9 ms | 11159.956 (native) | 9.8e-16 |  | sí b0 nm0 mo0 χ✓ | 6/10/7/8/1 |  |
| 9a | arris | ✅ | 5.0 ms | 298926.991 (native) | 1.9e-16 |  | sí b0 nm0 mo0 χ✓ | 16/24/10/10/1 |  |
| 9b | arris | ✅ | 5.1 ms | 179356.194 (native) | 1.6e-16 |  | sí b0 nm0 mo0 χ✓ | 16/24/10/10/1 |  |
| 10a | arris | ✅ | 184.0 ms | 4737.410 (native) | 6.1e-13 |  | sí b0 nm0 mo0 | 2/3/3/3/1 |  |
| 10b | arris | ✅ | 177 µs | 21991.149 (native) | 1.7e-16 |  | sí b0 nm0 mo0 χ✓ | 2/3/3/3/1 |  |
| 10c | arris | ✅ | 2.7 ms | 6626.756 (native) |  |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 11a | arris | ✅ | 101 µs | 33510.322 (native) | 2.2e-16 |  | sí b0 nm0 mo0 χ✓ | 2/1/1/1/1 |  |
| 11b | arris | ✅ | 86 µs | 12566.371 (native) | 4.3e-16 |  | sí b0 nm0 mo0 χ✓ | 2/2/2/2/1 |  |
| 12rt:10a | arris | ✅ |  | 4737.410 (native) | 6.1e-13 (RT 3.8e-16) |  | sí b0 nm0 mo0 | 2/3/3/3/1 |  |
| 12rt:10b | arris | ✅ |  | 21991.149 (native) | 0 (RT 1.7e-16) |  | sí b0 nm0 mo0 χ✓ | 2/3/3/3/1 |  |
| 12rt:10c | arris | ✅ |  | 6626.756 (native) |  (RT 2.0e-13) |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 12rt:11a | arris | ✅ |  | 33510.322 (native) | 2.2e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 2/1/1/1/1 |  |
| 12rt:11b | arris | ✅ |  | 12566.371 (native) | 4.3e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 2/2/2/2/1 |  |
| 12rt:1a | arris | ✅ |  | 227433.629 (native) | 1.3e-16 (RT 0) | 6.8e-16 | sí b0 nm0 mo0 χ✓ | 10/15/7/9/1 |  |
| 12rt:1b | arris | ✅ |  | 233716.815 (native) | 1.2e-16 (RT 0) | 5.6e-16 | sí b0 nm0 mo0 χ✓ | 10/15/8/9/1 |  |
| 12rt:2a | arris | ⚠️ |  | 480000.000 (native) | 0 (RT 0) | 3.2e-16 | sí b0 nm0 mo0 χ✓ | 12/20/10/10/1 |  |
| 12rt:4 | arris | ✅ |  | 500530.965 (native) | 1.6e-15 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 208/312/206/306/1 |  |
| 12rt:5a | arris | ✅ |  | 239463.495 (native) | 0 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 12rt:5b | arris | ✅ |  | 108418.763 (native) | 2.7e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 12/20/10/10/1 |  |
| 12rt:5c | arris | ✅ |  | 238947.916 (native) | 0 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 13/21/10/10/1 |  |
| 12rt:5d | arris | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: variable-radius fillet (ops::fillet takes one constant radius) |
| 12rt:6 | arris | ✅ |  | 227377.285 (native) | 0 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 11/17/8/10/1 |  |
| 12rt:7a | arris | ✅ |  | 238750.000 (native) | 0 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 12rt:7b | arris | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: distance–angle chamfer (ops::chamfer is equal-distance only) |
| 12rt:8a | arris | ✅ |  | 35712.000 (native) | 1.0e-15 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 16/24/11/12/1 |  |
| 12rt:8b | arris | ✅ |  | 11159.956 (native) | 9.8e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 6/10/7/8/1 |  |
| 12rt:9a | arris | ✅ |  | 298926.991 (native) | 1.9e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 16/24/10/10/1 |  |
| 12rt:9b | arris | ✅ |  | 179356.194 (native) | 1.6e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 16/24/10/10/1 |  |
| 13a | arris | ✅ | 79 µs | 33510.322 (native) |  |  | sí b0 nm0 mo0 dev0.0088 χ✓ | 2/1/1/1/1 |  |
| 13b | arris | ✅ | 66 µs | 33510.322 (native) |  |  | sí b0 nm0 mo0 dev0.0846 χ✓ | 2/1/1/1/1 |  |
| 13c | arris | ✅ | 18.3 ms | 227433.629 (native) |  |  | sí b0 nm0 mo0 χ✓ | 10/15/7/9/1 |  |

## Hostiles (prueba 15)

| Familia | Backend | Total | OK | Error limpio | N/E | Panics | Primer panic |
|---|---|---|---|---|---|---|---|
| 15:1 | arris | 256 | 0 | 256 | 0 | 0 |  |
| 15:5 | arris | 256 | 0 | 256 | 0 | 0 |  |
| 15:9 | arris | 256 | 0 | 256 | 0 | 0 |  |
