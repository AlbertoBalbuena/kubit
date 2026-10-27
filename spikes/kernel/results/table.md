# Resultados del spike de kernels

Generado por `spike report` a partir de `results.json`. Leyenda: ✅ correcto · ⚠️ terminó pero el resultado es inválido o el volumen se sale de tolerancia · ❌ error limpio · N/E operación no expuesta · 💥 panic · ☠️ crash del proceso · ⏱ timeout · ↷ omitido (el caso fuente falló). εV = peor error relativo de volumen; tiempo = mediana de los casos correctos.

## Resumen: 16 pruebas × backends

| # | Prueba | monstertruck | truck | solidcore | arris |
|---|---|---|---|---|---|
| 1 | Caja − cilindro (pasante, ciego) | ✅2 · εV≤6.9e-6 · 89.0 ms | ⚠️2 · εV≤1.0e-5 · 204.0 ms | ✅2 · εV≤5.9e-10 · 10.5 ms | ✅2 · εV≤1.3e-16 · 10.6 ms |
| 2 | Booleana coplanar (cara, arista) | ❌2 | ❌2 | ✅1 ⚠️1 · εV≤0 · 4.2 ms | ⚠️1 ❌1 · εV≤0 · 241 µs |
| 3 | Booleana tangente (cilindro, esfera) | ✅1 ❌1 · εV≤1.4e-5 · 34.4 ms | ⚠️1 💥1 · εV≤7.0e-6 · 2.8 ms | ✅1 ⚠️1 · εV≤4.7e-9 · 14.0 ms | ❌2 |
| 4 | Unión de 100 cilindros | ❌1 | ⏱1 | ✅1 · εV≤1.9e-9 · 1.19 s | ✅1 · εV≤1.6e-15 · 642.6 ms |
| 5 | Fillet (arista, cadena, vértice, variable) | ✅2 ⚠️1 ❌1 · εV≤3.3e-6 · 1.8 ms | N/E4 | ⚠️4 · εV≤7.6e-8 · 15.8 ms | ✅3 N/E1 · εV≤2.7e-16 · 410 µs |
| 6 | Fillet después de booleana | ❌1 | N/E1 | ⚠️1 · εV≤3.9e-10 · 17.3 ms | ✅1 · εV≤0 · 15.8 ms |
| 7 | Chaflán (d-d, d-ángulo) | ✅1 N/E1 · εV≤2.4e-16 · 1.4 ms | N/E2 | ⚠️1 N/E1 · εV≤2.4e-16 · 5.2 ms | ✅1 N/E1 · εV≤0 · 113 µs |
| 8 | Shell (caja, cilindro con fillet) | N/E2 | N/E2 | ✅2 · εV≤3.6e-8 · 48.6 ms | ✅2 · εV≤1.0e-15 · 15.6 ms |
| 9 | Push/pull de cara fileteada | N/E2 | N/E2 | N/E2 | ✅2 · εV≤1.9e-16 · 5.6 ms |
| 10 | Sweep hélice / loft | ✅2 N/E1 · εV≤6.3e-5 · 1.2 ms | ✅2 N/E1 · εV≤4.6e-5 · 39 µs | ✅2 ⚠️1 · εV≤2.2e-6 · 10.0 ms | ✅3 · εV≤6.1e-13 · 1.5 ms |
| 11 | Revolve tocando el eje | ✅2 · εV≤2.3e-4 · 1.5 ms | ⚠️2 · εV≤2.3e-4 · 33 µs | ✅2 · εV≤4.1e-8 · 2.4 ms | ✅2 · εV≤4.3e-16 · 85 µs |
| 12 | STEP round-trip + import NIST | RT ✅9 ❌1 ↷10 · εV≤1.9e-4 / import ✅6 ⚠️1 ❌3 · 121.1 ms | RT ✅2 ⚠️2 ⏱3 ↷13 · εV≤3.0e0 / import ❌10 | RT N/E17 ↷3 / import ⚠️6 ❌4 · 2.90 s | RT ✅17 ⚠️1 ↷2 · εV≤6.1e-13 / import ✅5 ⚠️4 ❌1 · 7.87 s |
| 13 | Teselado watertight | ✅3 · 1.4 ms | ✅1 ⚠️2 · 29 µs | ✅3 · 1.3 ms | ✅3 · 49 µs |
| 14 | Build wasm32 | ✅ 3044 KiB (799 KiB gz) · 6/18 ok, 1 invalid, 5 N/E, 6 failed, 0 trap in node | ✅ 1463 KiB (339 KiB gz) · 3/18 ok, 11 N/E, 2 error, 1 trap, 1 timeout in node | ✅ 16041 KiB (4948 KiB gz) · 15/18 ok, 3 N/E, 0 failed, 0 trap in node | ✅ 4591 KiB (1438 KiB gz) · 13/18 ok, 2 N/E, 3 failed, 0 trap in node |
| 15 | Hostiles (proptest) en 1, 5, 9 | 1: ⏱ · 5: 4💥/256 · 9: N/E | 1: ⏱ · 5: N/E · 9: N/E | 1: 0💥/256 · 5: 0💥/256 · 9: N/E | 1: 0💥/256 · 5: 0💥/256 · 9: 0💥/256 |
| 16 | Historia en 1, 5, 9 | 1a: no; 5a: no; 9a: N/E (sin resultado) | 1a: no; 5a: N/E (sin resultado); 9a: N/E (sin resultado) | 1a: no; 5a: no; 9a: N/E (sin resultado) | 1a: nativa g6/m4/d10; 5a: nativa g9/m10/d3; 9a: nativa g0/m35/d0 |

## Detalle por sub-caso

| Caso | Backend | Resultado | Tiempo | Volumen (método) | εV | εA | Válido | V/E/F/L/S | Mensaje |
|---|---|---|---|---|---|---|---|---|---|
| 1a | arris | ✅ | 10.6 ms | 227433.629 (native) | 1.3e-16 | 6.8e-16 | sí b0 nm0 mo0 χ✓ | 10/15/7/9/1 |  |
| 1b | arris | ✅ | 7.5 ms | 233716.815 (native) | 1.2e-16 | 5.6e-16 | sí b0 nm0 mo0 χ✓ | 10/15/8/9/1 |  |
| 2a | arris | ⚠️ | 241 µs | 480000.000 (native) | 0 | 3.2e-16 | sí b0 nm0 mo0 χ✓ | 12/20/10/10/1 |  |
| 2b | arris | ❌ |  |  |  |  |  |  | fuse: degenerate result: the result's shells would touch along an edge or at a vertex, which a solid does not hold (+e6) |
| 3a | arris | ❌ |  |  |  |  |  |  | fuse: degenerate result: the faces touch along a curve interior to both result faces (+f1, +f7) |
| 3b | arris | ❌ |  |  |  |  |  |  | fuse: no closed form for +f1 (plane surface) against +v8 (point) |
| 4 | arris | ✅ | 642.6 ms | 500530.965 (native) | 1.6e-15 |  | sí b0 nm0 mo0 χ✓ | 208/312/206/306/1 |  |
| 5a | arris | ✅ | 236 µs | 239463.495 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 5b | arris | ✅ | 6.7 ms | 108418.763 (native) | 2.7e-16 |  | sí b0 nm0 mo0 χ✓ | 12/20/10/10/1 |  |
| 5c | arris | ✅ | 410 µs | 238947.916 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 13/21/10/10/1 |  |
| 5d | arris | N/E |  |  |  |  |  |  | variable-radius fillet (ops::fillet takes one constant radius) |
| 6 | arris | ✅ | 15.8 ms | 227377.285 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 11/17/8/10/1 |  |
| 7a | arris | ✅ | 113 µs | 238750.000 (native) | 0 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 7b | arris | N/E |  |  |  |  |  |  | distance–angle chamfer (ops::chamfer is equal-distance only) |
| 8a | arris | ✅ | 536 µs | 35712.000 (native) | 1.0e-15 |  | sí b0 nm0 mo0 χ✓ | 16/24/11/12/1 |  |
| 8b | arris | ✅ | 15.6 ms | 11159.956 (native) | 9.8e-16 |  | sí b0 nm0 mo0 χ✓ | 6/10/7/8/1 |  |
| 9a | arris | ✅ | 5.6 ms | 298926.991 (native) | 1.9e-16 |  | sí b0 nm0 mo0 χ✓ | 16/24/10/10/1 |  |
| 9b | arris | ✅ | 5.6 ms | 179356.194 (native) | 1.6e-16 |  | sí b0 nm0 mo0 χ✓ | 16/24/10/10/1 |  |
| 10a | arris | ✅ | 98.0 ms | 4737.410 (native) | 6.1e-13 |  | sí b0 nm0 mo0 | 2/3/3/3/1 |  |
| 10b | arris | ✅ | 118 µs | 21991.149 (native) | 1.7e-16 |  | sí b0 nm0 mo0 χ✓ | 2/3/3/3/1 |  |
| 10c | arris | ✅ | 1.5 ms | 6626.756 (native) |  |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 11a | arris | ✅ | 85 µs | 33510.322 (native) | 2.2e-16 |  | sí b0 nm0 mo0 χ✓ | 2/1/1/1/1 |  |
| 11b | arris | ✅ | 62 µs | 12566.371 (native) | 4.3e-16 |  | sí b0 nm0 mo0 χ✓ | 2/2/2/2/1 |  |
| nist_ctc_01_asme1_ap242-e1.stp | arris | ✅ | 1.32 s | 14644823.556 (native) |  |  | sí b0 nm0 mo0 | 206/318/117/140/1 |  |
| nist_ctc_02_asme1_ap242-e2.stp | arris | ⚠️ | 60.13 s | 47106575.750 (native) |  |  | no b0 nm46 mo112 | 876/1416/637/753/3 |  |
| nist_ctc_03_asme1_ap242-e2.stp | arris | ✅ | 451.1 ms | 332122.908 (native) |  |  | sí b0 nm0 mo0 | 236/354/120/154/1 |  |
| nist_ctc_04_asme1_ap242-e1.stp | arris | ⚠️ | 16.76 s | 17519780.335 (native) |  |  | no b42 nm0 mo0 | 664/1110/484/587/2 |  |
| nist_ctc_05_asme1_ap242-e1.stp | arris | ✅ | 10.24 s | 12694903.247 (native) |  |  | sí b0 nm0 mo0 | 251/380/156/201/1 |  |
| nist_ftc_06_asme1_ap242-e2.stp | arris | ✅ | 1.04 s | 3323557.193 (native) |  |  | sí b0 nm0 mo0 | 310/476/187/226/1 |  |
| nist_ftc_07_asme1_ap242-e2.stp | arris | ⚠️ | 31.51 s | 1726248.291 (native) |  |  | no b6 nm0 mo0 | 588/871/306/355/3 |  |
| nist_ftc_08_asme1_ap242-e1-tg.stp | arris | ❌ | 13.2 ms |  |  |  |  |  | 2 solids in file: 0 read (0 sheets), 2 refused: #11436: TESSELLATED_SOLID is outside the subset the reader maps; #25216: TESSELLATED_SHELL i |
| nist_ftc_10_asme1_ap242-e2.stp | arris | ⚠️ | 7.87 s | 188318.069 (native) |  |  | no b76 nm1780 mo3528 | 708/957/282/361/13 |  |
| nist_ftc_11_asme1_ap242-e2.stp | arris | ✅ | 604.1 ms | 5135.076 (native) |  |  | sí b0 nm0 mo0 | 74/112/42/46/1 |  |
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
| 13a | arris | ✅ | 49 µs | 33510.322 (native) |  |  | sí b0 nm0 mo0 dev0.0088 χ✓ | 2/1/1/1/1 |  |
| 13b | arris | ✅ | 42 µs | 33510.322 (native) |  |  | sí b0 nm0 mo0 dev0.0846 χ✓ | 2/1/1/1/1 |  |
| 13c | arris | ✅ | 13.0 ms | 227433.629 (native) |  |  | sí b0 nm0 mo0 χ✓ | 10/15/7/9/1 |  |
| 1a | monstertruck | ✅ | 89.0 ms | 227435.209 (mesh) | 6.9e-6 | 1.7e-5 | sí b0 nm0 mo0 χ✓ | 12/18/8/10/1 |  |
| 1b | monstertruck | ✅ | 32.1 ms | 233717.582 (mesh) | 3.3e-6 | 2.5e-6 | sí b0 nm0 mo0 χ✓ | 12/18/9/10/1 |  |
| 2a | monstertruck | ❌ |  |  |  |  |  |  | or: invalid output shell for `or`: This shell is not oriented and closed.. |
| 2b | monstertruck | ❌ |  |  |  |  |  |  | or: invalid output shell for `or`: This shell is not oriented and closed.. |
| 3a | monstertruck | ✅ | 34.4 ms | 271412.117 (mesh) | 1.4e-5 |  | sí b0 nm0 mo0 | 12/18/10/10/2 |  |
| 3b | monstertruck | ❌ |  |  |  |  |  |  | or: invalid output shell for `or`: This shell is not oriented and closed.. |
| 4 | monstertruck | ❌ |  |  |  |  |  |  | or: invalid output shell for `or`: This shell is not oriented and closed.. |
| 5a | monstertruck | ✅ | 1.7 ms | 239462.696 (mesh) | 3.3e-6 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 5b | monstertruck | ⚠️ | 2.0 ms |  |  |  | no χ✓ | 10/15/7/7/1 |  |
| 5c | monstertruck | ❌ |  |  |  |  |  |  | fillet result is not a solid: This shell is not oriented and closed. |
| 5d | monstertruck | ✅ | 1.8 ms | 239627.532 (mesh) | 2.1e-6 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 6 | monstertruck | ❌ |  |  |  |  |  |  | fillet_edges returned Ok on 2 edge(s) but added no face (8 faces before and after) |
| 7a | monstertruck | ✅ | 1.4 ms | 238750.000 (mesh) | 2.4e-16 |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 7b | monstertruck | N/E |  |  |  |  |  |  | distance–angle chamfer: FilletProfile::Chamfer is equal-setback only |
| 8a | monstertruck | N/E |  |  |  |  |  |  | shell/hollow solid: no B-rep shell operation (surface offsets only) |
| 8b | monstertruck | N/E |  |  |  |  |  |  | shell/hollow solid: no B-rep shell operation (surface offsets only) |
| 9a | monstertruck | N/E |  |  |  |  |  |  | push/pull: no face offset/move operation |
| 9b | monstertruck | N/E |  |  |  |  |  |  | push/pull: no face offset/move operation |
| 10a | monstertruck | N/E |  |  |  |  |  |  | sweep along a helix: no B-rep sweep along a curve (surface-only sweep_rail) |
| 10b | monstertruck | ✅ | 1.2 ms | 21989.766 (mesh) | 6.3e-5 |  | sí b0 nm0 mo0 χ✓ | 4/6/4/4/1 |  |
| 10c | monstertruck | ✅ | 968 µs | 16234.964 (mesh) |  |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 11a | monstertruck | ✅ | 1.5 ms | 33502.495 (mesh) | 2.3e-4 |  | sí b0 nm0 mo0 χ✓ | 6/12/8/8/1 |  |
| 11b | monstertruck | ✅ | 719 µs | 12564.580 (mesh) | 1.4e-4 |  | sí b0 nm0 mo0 χ✓ | 6/12/8/8/1 |  |
| nist_ctc_01_asme1_ap242-e1.stp | monstertruck | ⚠️ | 662.6 ms | 14645321.317 (mesh) |  |  | no b0 nm32 mo64 | 206/318/117/140/1 |  |
| nist_ctc_02_asme1_ap242-e2.stp | monstertruck | ✅ | 257.2 ms |  |  |  | sí | 870/1410/635/750/1 |  |
| nist_ctc_03_asme1_ap242-e2.stp | monstertruck | ✅ | 147.6 ms | 332139.726 (mesh) |  |  | sí b0 nm0 mo0 | 236/354/120/154/1 |  |
| nist_ctc_04_asme1_ap242-e1.stp | monstertruck | ✅ | 121.1 ms |  |  |  | sí | 662/1108/483/586/1 |  |
| nist_ctc_05_asme1_ap242-e1.stp | monstertruck | ❌ | 32.6 ms |  |  |  |  |  | no solid read (1 manifold_solid_brep): #1205: extract: Two same vertices cannot construct an edge. |
| nist_ftc_06_asme1_ap242-e2.stp | monstertruck | ✅ | 106.0 ms |  |  |  | sí | 310/476/187/226/1 |  |
| nist_ftc_07_asme1_ap242-e2.stp | monstertruck | ✅ | 72.7 ms |  |  |  | sí | 582/865/304/353/1 |  |
| nist_ftc_08_asme1_ap242-e1-tg.stp | monstertruck | ❌ | 95.3 ms |  |  |  |  |  | no solid read (0 manifold_solid_brep):  |
| nist_ftc_10_asme1_ap242-e2.stp | monstertruck | ✅ | 99.1 ms |  |  |  | sí | 560/799/264/331/1 |  |
| nist_ftc_11_asme1_ap242-e2.stp | monstertruck | ❌ | 96.0 ms |  |  |  |  |  | no solid read (1 manifold_solid_brep): #1299: extract: Two same vertices cannot construct an edge. |
| 12rt:10a | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: sweep along a helix: no B-rep sweep along a curve (surface-only sweep_rail) |
| 12rt:10b | monstertruck | ✅ |  | 21989.766 (mesh) | 6.3e-5 (RT 1.7e-16) |  | sí b0 nm0 mo0 χ✓ | 4/6/4/4/1 |  |
| 12rt:10c | monstertruck | ✅ |  | 16234.964 (mesh) |  (RT 5.6e-16) |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 12rt:11a | monstertruck | ✅ |  | 33507.291 (mesh) | 9.0e-5 (RT 1.4e-4) |  | sí b0 nm0 mo0 χ✓ | 6/12/8/8/1 |  |
| 12rt:11b | monstertruck | ✅ |  | 12564.031 (mesh) | 1.9e-4 (RT 4.4e-5) |  | sí b0 nm0 mo0 χ✓ | 6/12/8/8/1 |  |
| 12rt:1a | monstertruck | ✅ |  |  |  |  | sí χ✓ | 12/18/8/10/1 |  |
| 12rt:1b | monstertruck | ✅ |  |  |  |  | sí χ✓ | 12/18/9/10/1 |  |
| 12rt:2a | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: failed: or: invalid output shell for `or`: This shell is not oriented and closed.. |
| 12rt:4 | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: failed: or: invalid output shell for `or`: This shell is not oriented and closed.. |
| 12rt:5a | monstertruck | ✅ |  | 239462.696 (mesh) | 3.3e-6 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 12rt:5b | monstertruck | ❌ |  |  |  |  |  |  | Table::from_step: STEP parser error: Error while tokenizing STEP input 0: at line 7, in Tag: DATA; ^   |
| 12rt:5c | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: failed: fillet result is not a solid: This shell is not oriented and closed. |
| 12rt:5d | monstertruck | ✅ |  | 239627.532 (mesh) | 2.1e-6 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 12rt:6 | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: failed: fillet_edges returned Ok on 2 edge(s) but added no face (8 faces before and after) |
| 12rt:7a | monstertruck | ✅ |  | 238750.000 (mesh) | 2.4e-16 (RT 0) |  | sí b0 nm0 mo0 χ✓ | 10/15/7/7/1 |  |
| 12rt:7b | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: distance–angle chamfer: FilletProfile::Chamfer is equal-setback only |
| 12rt:8a | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: shell/hollow solid: no B-rep shell operation (surface offsets only) |
| 12rt:8b | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: shell/hollow solid: no B-rep shell operation (surface offsets only) |
| 12rt:9a | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: push/pull: no face offset/move operation |
| 12rt:9b | monstertruck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: push/pull: no face offset/move operation |
| 13a | monstertruck | ✅ | 1.4 ms | 33470.596 (mesh) |  |  | sí b0 nm0 mo0 dev0.0143 χ✓ | 6/12/8/8/1 |  |
| 13b | monstertruck | ✅ | 1.4 ms | 33453.192 (mesh) |  |  | sí b0 nm0 mo0 dev0.0775 χ✓ | 6/12/8/8/1 |  |
| 13c | monstertruck | ✅ | 53.0 ms | 227440.750 (mesh) |  |  | sí b0 nm0 mo0 χ✓ | 12/18/8/10/1 |  |
| 1a | solidcore | ✅ | 10.5 ms | 227433.630 (native) | 5.9e-10 | 1.6e-6 | sí χ✓ | 10/15/7/9/1 |  |
| 1b | solidcore | ✅ | 7.0 ms | 233716.815 (native) | 3.7e-10 | 3.0e-7 | sí χ✓ | 10/15/8/9/1 |  |
| 2a | solidcore | ✅ | 4.2 ms | 480000.000 (native) | 0 | 0 | sí χ✓ | 8/12/6/6/1 |  |
| 2b | solidcore | ⚠️ | 3.2 ms | 480000.000 (native) | 0 |  | no | 14/23/12/12/2 |  |
| 3a | solidcore | ⚠️ | 8.0 ms | 271415.925 (native) | 4.7e-9 |  | no | 11/17/9/9/2 |  |
| 3b | solidcore | ✅ | 14.0 ms | 244188.790 (native) | 3.3e-10 |  | sí | 10/13/7/8/2 |  |
| 4 | solidcore | ✅ | 1.19 s | 500530.964 (native) | 1.9e-9 |  | sí χ✓ | 208/312/206/306/1 |  |
| 5a | solidcore | ⚠️ | 4.7 ms | 239463.495 (native) | 8.8e-11 |  | no χ✓ | 10/15/7/7/1 |  |
| 5b | solidcore | ⚠️ | 37.1 ms | 108418.763 (native) | 2.3e-9 |  | no χ✓ | 12/20/10/10/1 |  |
| 5c | solidcore | ⚠️ | 14.0 ms | 238947.916 (native) | 4.1e-11 |  | no χ✓ | 13/21/10/10/1 |  |
| 5d | solidcore | ⚠️ | 15.8 ms | 239628.005 (native) | 7.6e-8 |  | no χ✓ | 10/15/7/7/1 |  |
| 6 | solidcore | ⚠️ | 17.3 ms | 227377.284 (native) | 3.9e-10 |  | no χ✓ | 11/17/8/10/1 |  |
| 7a | solidcore | ⚠️ | 5.2 ms | 238750.000 (native) | 2.4e-16 |  | no χ✓ | 10/15/7/7/1 |  |
| 7b | solidcore | N/E |  |  |  |  |  |  | distance–angle chamfer: fillet_edges/chamfer_fillet take one distance (equal setback); no asymmetric or angle chamfer in the public API |
| 8a | solidcore | ✅ | 27.3 ms | 35712.000 (native) | 8.1e-16 |  | sí χ✓ | 16/24/11/12/1 |  |
| 8b | solidcore | ✅ | 48.6 ms | 11159.956 (native) | 3.6e-8 |  | sí χ✓ | 6/10/7/8/1 |  |
| 9a | solidcore | N/E |  |  |  |  |  |  | push/pull: no public face offset/move (only low-level delete_face / replace_surface) |
| 9b | solidcore | N/E |  |  |  |  |  |  | push/pull: no public face offset/move (only low-level delete_face / replace_surface) |
| 10a | solidcore | ⚠️ | 122.8 ms | 4737.400 (native) | 2.2e-6 |  | sí | 2/3/3/3/1 |  |
| 10b | solidcore | ✅ | 3.9 ms | 21991.149 (native) | 8.0e-10 |  | sí χ✓ | 2/3/3/3/1 |  |
| 10c | solidcore | ✅ | 10.0 ms | 7339.682 (native) |  |  | sí χ✓ | 8/12/6/6/1 |  |
| 11a | solidcore | ✅ | 2.4 ms | 33510.322 (native) | 3.4e-9 |  | sí χ✓ | 2/1/1/1/1 |  |
| 11b | solidcore | ✅ | 1.3 ms | 12566.371 (native) | 4.1e-8 |  | sí χ✓ | 2/2/2/2/1 |  |
| nist_ctc_01_asme1_ap242-e1.stp | solidcore | ⚠️ | 607.1 ms | 14660345.770 (native) |  |  | no | 206/318/117/140/1 |  |
| nist_ctc_02_asme1_ap242-e2.stp | solidcore | ❌ | 1.85 s |  |  |  |  |  | step_brep::import: topology::brep::p8: upstream status 1001 |
| nist_ctc_03_asme1_ap242-e2.stp | solidcore | ⚠️ | 593.2 ms | 440750.772 (native) |  |  | no | 236/354/120/154/1 |  |
| nist_ctc_04_asme1_ap242-e1.stp | solidcore | ⚠️ | 18.17 s | 267749169.141 (native) |  |  | no | 664/1110/484/587/1 |  |
| nist_ctc_05_asme1_ap242-e1.stp | solidcore | ⚠️ | 7.48 s |  |  |  | no | 239/331/193/238/49 |  |
| nist_ftc_06_asme1_ap242-e2.stp | solidcore | ⚠️ | 2.90 s | 3310583.851 (native) |  |  | no | 310/476/187/226/1 |  |
| nist_ftc_07_asme1_ap242-e2.stp | solidcore | ❌ | 131.6 ms |  |  |  |  |  | step_brep::import: topology::brep::p8: upstream status 1001 |
| nist_ftc_08_asme1_ap242-e1-tg.stp | solidcore | ❌ | 15.4 ms |  |  |  |  |  | step_brep::import returned no bodies |
| nist_ftc_10_asme1_ap242-e2.stp | solidcore | ❌ | 142.3 ms |  |  |  |  |  | step_brep::import: topology::brep::p8: upstream status 1001 |
| nist_ftc_11_asme1_ap242-e2.stp | solidcore | ⚠️ | 2.45 s |  |  |  | no | 72/104/48/52/6 |  |
| 12rt:10a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:10b | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:10c | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:11a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:11b | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:1a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:1b | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:2a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:4 | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:5a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:5b | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:5c | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:5d | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:6 | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:7a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:7b | solidcore | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: distance–angle chamfer: fillet_edges/chamfer_fillet take one distance (equal setback); no  |
| 12rt:8a | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:8b | solidcore | N/E |  |  |  |  |  |  | solidcore has no STEP export of a B-rep (step-export writes the neutral step-import model, planar single-shell only) |
| 12rt:9a | solidcore | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: push/pull: no public face offset/move (only low-level delete_face / replace_surface) |
| 12rt:9b | solidcore | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: push/pull: no public face offset/move (only low-level delete_face / replace_surface) |
| 13a | solidcore | ✅ | 1.3 ms | 33510.322 (native) |  |  | sí χ✓ | 2/1/1/1/1 |  |
| 13b | solidcore | ✅ | 1.2 ms | 33510.322 (native) |  |  | sí χ✓ | 2/1/1/1/1 |  |
| 13c | solidcore | ✅ | 3.7 ms | 227433.630 (native) |  |  | sí χ✓ | 10/15/7/9/1 |  |
| 1a | truck | ⚠️ | 204.0 ms | 227435.895 (mesh) | 1.0e-5 | 1.9e-6 | no b6 nm1 mo2 χ✓ | 14/21/9/11/1 |  |
| 1b | truck | ⚠️ | 112.8 ms | 233717.955 (mesh) | 4.9e-6 | 3.0e-6 | no b9 nm0 mo0 χ✓ | 14/21/10/11/1 |  |
| 2a | truck | ❌ |  |  |  |  |  |  | truck_shapeops::or returned None |
| 2b | truck | ❌ |  |  |  |  |  |  | truck_shapeops::or returned None |
| 3a | truck | 💥 |  |  |  |  |  |  | This shell is not oriented and closed. at /Users/alberto/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/truck-topology-0.6.0/src/lib.r |
| 3b | truck | ⚠️ | 2.8 ms | 244187.086 (mesh) | 7.0e-6 |  | no b0 nm6 mo12 | 13/21/12/12/2 |  |
| 4 | truck | ⏱ |  |  |  |  |  |  | killed after 300s; last stderr:  |
| 5a | truck | N/E |  |  |  |  |  |  | truck has no fillet |
| 5b | truck | N/E |  |  |  |  |  |  | truck has no fillet |
| 5c | truck | N/E |  |  |  |  |  |  | truck has no fillet |
| 5d | truck | N/E |  |  |  |  |  |  | truck has no fillet |
| 6 | truck | N/E |  |  |  |  |  |  | truck has no fillet |
| 7a | truck | N/E |  |  |  |  |  |  | truck has no chamfer |
| 7b | truck | N/E |  |  |  |  |  |  | truck has no chamfer |
| 8a | truck | N/E |  |  |  |  |  |  | truck has no shell/hollow |
| 8b | truck | N/E |  |  |  |  |  |  | truck has no shell/hollow |
| 9a | truck | N/E |  |  |  |  |  |  | truck has no fillet and no face offset/move |
| 9b | truck | N/E |  |  |  |  |  |  | truck has no fillet and no face offset/move |
| 10a | truck | N/E |  |  |  |  |  |  | truck has no sweep along a curve (only tsweep/rsweep/homotopy) |
| 10b | truck | ✅ | 37 µs | 21990.139 (mesh) | 4.6e-5 |  | sí b0 nm0 mo0 χ✓ | 6/9/5/5/1 |  |
| 10c | truck | ✅ | 39 µs | 16235.172 (mesh) |  |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 11a | truck | ⚠️ | 33 µs | 33502.525 (mesh) | 2.3e-4 |  | no b0 nm6 mo12 χ✓ | 5/9/6/6/1 |  |
| 11b | truck | ⚠️ | 16 µs | 12565.798 (mesh) | 4.6e-5 |  | no b0 nm6 mo12 χ✓ | 5/9/6/6/1 |  |
| nist_ctc_01_asme1_ap242-e1.stp | truck | ❌ | 1.1 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ctc_02_asme1_ap242-e2.stp | truck | ❌ | 2.7 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ctc_03_asme1_ap242-e2.stp | truck | ❌ | 2.1 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ctc_04_asme1_ap242-e1.stp | truck | ❌ | 50.7 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ctc_05_asme1_ap242-e1.stp | truck | ❌ | 396 µs |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ftc_06_asme1_ap242-e2.stp | truck | ❌ | 4.1 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ftc_07_asme1_ap242-e2.stp | truck | ❌ | 39.0 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ftc_08_asme1_ap242-e1-tg.stp | truck | ❌ | 186 µs |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ftc_10_asme1_ap242-e2.stp | truck | ❌ | 58.7 ms |  |  |  |  |  | Table::from_step: STEP parse failed |
| nist_ftc_11_asme1_ap242-e2.stp | truck | ❌ | 589 µs |  |  |  |  |  | Table::from_step: STEP parse failed |
| 12rt:10a | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no sweep along a curve (only tsweep/rsweep/homotopy) |
| 12rt:10b | truck | ✅ |  | 21990.139 (mesh) | 4.6e-5 (RT 2.8e-10) |  | sí b0 nm0 mo0 χ✓ | 6/9/5/5/1 |  |
| 12rt:10c | truck | ✅ |  | 16235.172 (mesh) |  (RT 6.9e-10) |  | sí b0 nm0 mo0 χ✓ | 8/12/6/6/1 |  |
| 12rt:11a | truck | ⚠️ |  | -68078.887 (mesh) | 3.0e0 (RT 3.0e0) |  | no b1080 nm786 mo2082 χ✓ | 5/9/6/6/1 |  |
| 12rt:11b | truck | ⚠️ |  | -25130.223 (mesh) | 3.0e0 (RT 3.0e0) |  | no b1074 nm3 mo6 χ✓ | 5/9/6/6/1 |  |
| 12rt:1a | truck | ⏱ |  |  |  |  |  |  | killed after 300s; last stderr: Lookup failed for #573 |
| 12rt:1b | truck | ⏱ |  |  |  |  |  |  | killed after 300s; last stderr: Lookup failed for #364 |
| 12rt:2a | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: failed: truck_shapeops::or returned None |
| 12rt:4 | truck | ⏱ |  |  |  |  |  |  | killed after 300s; last stderr:  |
| 12rt:5a | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet |
| 12rt:5b | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet |
| 12rt:5c | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet |
| 12rt:5d | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet |
| 12rt:6 | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet |
| 12rt:7a | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no chamfer |
| 12rt:7b | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no chamfer |
| 12rt:8a | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no shell/hollow |
| 12rt:8b | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no shell/hollow |
| 12rt:9a | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet and no face offset/move |
| 12rt:9b | truck | ↷ |  |  |  |  |  |  | source case did not produce a solid: not exposed: truck has no fillet and no face offset/move |
| 13a | truck | ⚠️ | 29 µs | 33460.450 (mesh) |  |  | no b0 nm6 mo12 dev0.0428 χ✓ | 5/9/6/6/1 |  |
| 13b | truck | ⚠️ | 24 µs | 33131.144 (mesh) |  |  | no b0 nm6 mo12 dev0.3703 χ✓ | 5/9/6/6/1 |  |
| 13c | truck | ✅ | 196.8 ms | 227442.551 (mesh) |  |  | sí b0 nm0 mo0 χ✓ | 14/21/9/11/1 |  |

## Hostiles (prueba 15)

| Familia | Backend | Total | OK | Error limpio | N/E | Panics | Primer panic |
|---|---|---|---|---|---|---|---|
| 15:1 | arris | 256 | 0 | 256 | 0 | 0 |  |
| 15:5 | arris | 256 | 0 | 256 | 0 | 0 |  |
| 15:9 | arris | 256 | 0 | 256 | 0 | 0 |  |
| 15:1 | monstertruck | — | — | — | — | ⏱ | killed after 300s; last stderr: HOSTILE 30 [2.2250738585072014e-308, 82.91988993964058, -1.0, 1000000000.0] |
| 15:5 | monstertruck | 256 | 0 | 252 | 0 | 4 | [196.89693547573484, 1000000000.0, 175.2291554115495, 0.0]: The end point Vector4 [NaN, NaN, NaN, 1.0] of the first curve is different from the start point Vect |
| 15:9 | monstertruck | 256 | 0 | 0 | 256 | 0 |  |
| 15:1 | solidcore | 256 | 0 | 256 | 0 | 0 |  |
| 15:5 | solidcore | 256 | 0 | 256 | 0 | 0 |  |
| 15:9 | solidcore | 256 | 0 | 0 | 256 | 0 |  |
| 15:1 | truck | — | — | — | — | ⏱ | killed after 300s; last stderr: HOSTILE 6 [83.53883526641387, -0.0, NaN, 71.09495333397464] |
| 15:5 | truck | 256 | 0 | 0 | 256 | 0 |  |
| 15:9 | truck | 256 | 0 | 0 | 256 | 0 |  |
