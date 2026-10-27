# Kubit — Modelo de fabricación

Este documento describe el diferenciador de Kubit: cómo se representan los materiales, el stock, los procesos y las reglas DFM.

- Datos semilla y fuentes: `docs/research/materiales-dfm.md`.
- Todos los valores son **asistencia, no certificación**.

---

## 1. Conceptos

```
Material ──< StockForm ──< (proveedor/región, precio opcional)
   │
   └── Properties (densidad, módulo, resistencia, movimiento por humedad/temperatura, anisotropía)

Process ── capabilities + constraints (parámetros)
ProcessChain = [Process] ordenados (p. ej. corte → canteado → barrenado; corte → barreno → templado)

ManufacturingIntent (por cuerpo) = material@ver + stock + process_chain + finish + grain_dir + edge_bands + exposure

Rule ── aplica a (familias, procesos, tipos de cuerpo) ── check ── severidad ── params ── fuentes
```

## 2. Cuerpos de fabricación

| `kind` | Fuente de verdad | Salidas típicas | Familias |
|---|---|---|---|
| `Panel` | contorno 2D + espesor de stock + maquinados | despiece, cut list, nesting de tableros, DXF por pieza | MDF, aglomerado, triplay, melamina, enchapados, HPL sobre sustrato |
| `SolidWood` | sección nominal/S2S + longitud + uniones | lista de madera (pies-tabla), despiece | latifoliadas, coníferas |
| `Profile` | sección de catálogo + trayectoria + cortes de extremo | lista de corte de perfiles, ángulos de inglete | PTR, ángulo, canal, solera, tubo, extrusiones de aluminio |
| `SheetMetal` | desarrollo plano + dobleces (radio, factor K) | desarrollo DXF, tabla de dobleces | acero al carbón, inoxidable, galvanizado, aluminio |
| `Slab` | contorno + espesor de placa + recortes + uniones | plantillas, mapa de placas y vetas | mármol, granito, cuarzo, solid surface |
| `Glass` | contorno + espesor + tipo + barrenos y cantos | orden de corte a vidriería | recocido, templado, laminado |
| `Printed` | B-rep o malla + orientación de impresión | STL/3MF | FDM, SLA, SLS/MJF |
| `Soft` | volumen aproximado + especificación de espuma y tela | consumo de espuma y tela | tapicería |
| `Solid` | B-rep libre | STEP | genérico, cuando no hay otro tipo |

Un cuerpo puede **convertirse** entre tipos cuando tiene sentido. Por ejemplo, un `Solid` prismático "reconocido como" `Panel`: si el reconocimiento falla, se explica por qué.

## 3. Esquema de datos (TOML, validado con JSON Schema)

Formato elegido: **TOML**, por ser nativo de Rust, sin ambigüedades de tipos y fácil de leer en PRs. Ver ADR-0001.

### 3.1 Material

```toml
[material]
id = "mdf.standard"
version = "1.0.0"
family = "mdf"                     # familia controlada (enum)
name = { es = "MDF estándar", en = "Standard MDF" }
anisotropic = false

[material.properties]              # rangos: [min, típico, max] + fuente
density_kg_m3 = { range = [680, 740, 800], source = "F18" }
moisture_movement = { note = "…", source = "…" }

[[material.sources]]
id = "F18"
url = "https://…"
retrieved = "2026-10-26"
license_note = "dato citado; ficha técnica de fabricante"
```

### 3.2 Stock

```toml
[[stock]]
id = "mdf.standard.sheet.1220x2440"
material = "mdf.standard"
form = "sheet"                     # sheet | board | profile | slab | roll | filament | block
size_mm = [1220, 2440]
thicknesses_mm = [5.5, 12, 15, 18, 25]
thickness_tolerance_mm = { plus = 0.3, minus = 0.3 }
regions = ["MX", "US"]
```

### 3.3 Proceso

```toml
[process]
id = "cnc.router.3axis"
name = { es = "Router CNC 3 ejes", en = "3-axis CNC router" }
applies_to = ["Panel", "SolidWood", "Slab"]

[process.params]                   # el taller puede sobrescribir cualquiera
default_tool_diameter_mm = 6.0     # ⚠ típico, validar con el taller
max_bed_mm = [1530, 3100]
```

### 3.4 Regla DFM

```toml
[rule]
id = "PNL-008"
families = ["mdf", "particleboard"]
body_kinds = ["Panel"]
check = "geom"                      # attr | geom | asm | struct
impl = "screw_edge_distance"        # chequeo Rust parametrizado
severity = "warn"                   # error | warn | info
params = { min_edge_mm = 12, min_corner_mm = 25 }
message = { es = "Tornillo en cara a {dist} mm del canto; mínimo recomendado {min_edge_mm} mm.",
            en = "Face screw {dist} mm from edge; recommended minimum {min_edge_mm} mm." }
quick_fix = { command = "feature.move_to_min_distance" }
confidence = "verified"             # verified | typical-unverified
sources = ["F19", "F20"]
disclaimer = "assistance-only"
```

**Reglas de contribución del catálogo**:

- Todo valor numérico lleva fuente o `confidence = "typical-unverified"`.
- Una regla `typical-unverified` nunca puede tener severidad `error` hasta que se valide.
- No se ingieren datos de fuentes no redistribuibles (MatWeb, por ejemplo) ni texto CC BY-SA (Wikipedia). Ver §A de la investigación.
- El Wood Handbook (USDA, obra del gobierno de EE. UU.) es una fuente principal segura para madera, siempre citada.

## 4. Familias y cobertura por fase

| Fase | Familias | Reglas semilla |
|---|---|---|
| 2 — Kubit Wood | tableros (MDF, aglomerado, triplay), melamina, enchapados, HPL, madera sólida, cubrecanto, herrajes (System 32, bisagras, correderas, Minifix) | ~45 (PNL, HPL, EB, HW, SW) |
| 4 — Metal | lámina (acero al carbón, inoxidable, galvanizado, aluminio), perfiles estructurales y extrusiones | doblez, barreno-doblez, soldadura de galvanizado, etc. |
| 5 — Superficies | mármol, granito, cuarzo, solid surface, vidrio | CT-001…004, GL-001…003 |
| 7 — Otros | impresión 3D, fibra de vidrio, tapicería | voladizos, pared mínima, desmoldeo, densidad de espuma |

## 5. Pendientes de validación con el taller

La investigación dejó brechas que se cierran mejor con datos reales de producción que con internet:

- Radios mínimos de la enchapadora real (Homag/Biesse/SCM).
- Fresa CNC por defecto y refilado.
- Número de bisagras por alto y peso de puerta.
- Tamaños de placa de piedra, cuarzo y vidrio disponibles en México.
- Maderas mexicanas y humedad de equilibrio en Guadalajara.

Estos datos van en una capa de catálogo de **taller** (overrides), no en el catálogo público, salvo que se puedan publicar.
