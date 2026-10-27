# Kubit — Investigación de materiales, procesos y reglas DFM (semilla del catálogo)

> **Estado:** borrador de investigación v0.1 · 2026-10-26
> **Alcance:** familias de materiales para mobiliario/millwork (prioridad) y familias adyacentes (metal, vidrio, compuestos, impresión 3D, tapicería).
> **Aviso importante:** todo lo que aparece aquí es **orientativo / de asistencia al diseño**, no constituye certificación, cálculo estructural ni cumplimiento normativo. Los valores de fabricantes son "típicos" y casi siempre vienen con la advertencia explícita de no usarse como base de diseño (p. ej. [F44], [F59]). Cada regla del linter debe presentarse al usuario como "aviso" con su fuente y permitir que el taller la sobrescriba con sus propios parámetros.

**Convención de citas:** cada valor numérico lleva un enlace `[Fnn]` a la lista de **Fuentes** al final. Cuando no se encontró fuente verificable se marca **⚠ típico-sin-fuente** = *"valor típico de industria, sin fuente verificada — validar"*.

---

## 0. Resumen ejecutivo

1. **Hallazgo de licencias más importante:** la fuente abierta más sólida para madera es el **USDA FPL *Wood Handbook*** (ed. 2021, FPL-GTR-282 [F2][F3]); como obra de empleados del gobierno de EE. UU. no está sujeta a copyright según **17 U.S.C. §105** [F1]. El PDF no incluye una declaración explícita de dominio público [F2], así que hay que (a) citarlo siempre y (b) revisar figuras o tablas reproducidas de terceros antes de copiarlas. En cambio, **MatWeb prohíbe expresamente** redistribuir, exportar en masa, publicar una base derivada o usar su contenido para entrenar IA [F11]. Hay hojas "de fabricante" que en realidad son copias de MatWeb/ASM (p. ej. [F59]): **no se deben ingerir**.
2. Los **hechos numéricos aislados** no tienen copyright en EE. UU. (*Feist*, [F14]), y en México la LFDA protege las bases de datos como compilaciones, pero "no se extenderá a los datos" [F12]. En la UE, en cambio, existe un **derecho sui generis** de 15 años sobre bases con inversión sustancial [F13]. Estrategia recomendada: **re-derivar cada valor de fuentes primarias citables** (gobierno, normas que los resumen públicamente, fichas técnicas de fabricante citadas fila por fila) y **nunca copiar una tabla completa** de un tercero.
3. Para el MVP de mobiliario hay datos **muy buenos** sobre tableros (ANSI A208.1 [F15], fichas Arauco México [F18]), herrajes (System 32 [F24], Blum [F25][F26], Accuride [F27]), HPL (Wilsonart [F32][F33]) y movimiento de madera (Wood Handbook [F4]). Los datos son **débiles** en: EMC por región de México, propiedades de maderas mexicanas (pino, parota, cedro, encino), límites de normas ASTM de piedra (de pago) y radios mínimos de cubrecanto por máquina.
4. Se proponen **45 reglas iniciales** (sección C), cada una parametrizada y con su fuente.

---

## 1. Madera maciza (latifoliadas y coníferas)

### 1.1 Formas y medidas de stock
| Concepto | Valor | Fuente |
|---|---|---|
| Sistema de "cuartos" EE. UU. (NHLA): 4/4 seco en bruto ≥ 15/16" (23.8 mm); cepillado S2S ≥ 13/16" (20.6 mm) | | [F10] |
| 5/4: bruto ≥ 1-3/16" (30.2 mm); S2S ≥ 1-1/16" (27 mm) | | [F10] |
| 8/4: bruto ≥ 1-7/8" (47.6 mm); S2S ≥ 1-3/4" (44.5 mm) | | [F10] |
| Anchos y largos aleatorios (lumber de latifoliadas) | ⚠ típico-sin-fuente | — |
| México: el pino se comercializa comúnmente en pulgadas nominales (1", 2"…) y en tablas de 8 ft o 2.44 m | ⚠ típico-sin-fuente | — |

### 1.2 Propiedades relevantes
**Movimiento por humedad (lo más importante para DFM de muebles).** Fórmula del Wood Handbook [F4]:

`ΔD = D_I · C · (M_F − M_I)`

Aquí `C` es `C_T` (tangencial) o `C_R` (radial). La fórmula es válida en el intervalo de 6–14 % de contenido de humedad (CH) y los coeficientes se basan en la dimensión al 10 % [F4].

| Especie | C_R (por 1 % CH) | C_T (por 1 % CH) | Contracción verde→anhidro R / T (%) |
|---|---|---|---|
| Encino rojo (red oak, comercial) | 0.00158 [F4] | 0.00369 [F4] | 4.0 / 8.6 [F7] |
| Encino blanco (white oak) | 0.00180 [F4] | 0.00365 [F4] | 5.6 / 10.5 [F7] |
| Maple duro (sugar maple) | 0.00165 [F4] | 0.00353 [F4] | 4.8 / 9.9 [F7] |
| Cerezo negro | 0.00126 [F4] | 0.00248 [F4] | 3.7 / 7.1 [F7] |
| Nogal negro | 0.00190 [F4] | 0.00274 [F4] | 5.5 / 7.8 [F7] |
| Yellow-poplar | 0.00158 [F4] | 0.00289 [F4] | 4.6 / 8.2 [F7] |
| Douglas-fir (costa) | 0.00165 [F4] | 0.00267 [F4] | — |
| Pino blanco del este | 0.00071 [F4] | 0.00212 [F4] | — |
| Pino ponderosa | 0.00133 [F4] | 0.00216 [F4] | — |
| Caoba (mahogany)† | 0.00172 [F4] | 0.00238 [F4] | — |
| Teca† | 0.00101 [F4] | 0.00186 [F4] | — |

† Para estas especies se asume que la contracción empieza al 22 % de CH y no al 30 % [F4].

- La contracción longitudinal es de solo ~0.1–0.2 % y normalmente se ignora [F6]. En promedio, la contracción tangencial es ~2 veces la radial (8.7 % frente a 5.0 % en la muestra de especies de Purdue [F6]; T/R ≈ 2.4 en encino rojo del sur [F9]). Esto causa **acopamiento** en tablas de corte tangencial (flatsawn).
- **CH recomendado para interiores** (EE. UU., carpintería, muebles y molduras) [F4]:

| Región | CH promedio | Piezas individuales |
|---|---|---|
| Mayor parte de EE. UU. | 8 % | 6–10 % |
| Suroeste seco | 6 % | 4–9 % |
| Costa cálida y húmeda | 11 % | 8–13 % |

- Ejemplo: una cubierta de maple de 32" con ΔCH = 6 % se mueve ≈ 0.6" (15 mm) [F6]. Muchos fabricantes de EE. UU. secan a ~6 % cuando no conocen el destino [F6]. La "regla del 2 %" admite 2 % de movimiento en servicio, lo que equivale a ΔCH ≈ 8 % [F6].
- **Propiedades mecánicas** (probetas pequeñas libres de defectos, rango verde–12 % CH) [F8]:

| Especie | Gravedad específica | MOR (psi) | MOE (×10⁶ psi) | Dureza lateral Janka (lbf) |
|---|---|---|---|---|
| Encino rojo | 0.52–0.69 | 7,400–18,100 | 1.14–2.28 | 930–2,080 |
| Encino blanco | 0.57–0.88 | 7,200–18,400 | 0.88–2.05 | 1,210–2,660 |
| Maple duro | 0.52–0.63 | 7,900–15,800 | 1.33–1.83 | 1,130–2,330 |
| Cerezo | 0.47–0.50 | 8,000–12,300 | 1.31–1.49 | 840–2,240 |
| Nogal negro | 0.51–0.55 | 9,500–14,600 | 1.42–1.68 | 1,220–1,370 |
| Poplar | 0.40–0.42 | 6,000–10,100 | 1.22–1.58 | 790–1,190 |
| Fresno | 0.45–0.60 | 6,000–15,000 | 1.04–1.74 | 860–2,030 |

> Para el catálogo, ingerir directamente las tablas del Wood Handbook 2021 (cap. 4–5 y 13) en lugar de [F8], que no declara su origen. **Brecha:** no se encontraron fuentes abiertas fiables para especies mexicanas (pino *Pinus spp.*, parota, cedro rojo, encino mexicano). Revisar publicaciones de CONAFOR / INIFAP / Universidad de Guadalajara (pendiente).

### 1.3 Procesos
Aserrado, cepillado, moldurado, CNC (router), torneado, ensambles (caja y espiga, dowel, galleta, cola de milano), encolado de tableros alistonados, curvado a vapor, laminado curvo, acabado.

### 1.4 Restricciones DFM
- **No restringir el movimiento transversal a la veta:** fijar cubiertas con ranuras, clips o "figure-8"; los tableros dentro de marcos deben flotar en la ranura. El caso documentado: una cubierta de maple anclada a 22" que pasa de 12 % a 6 % se contrae ~0.42" y se raja [F6].
- La holgura de la ranura en marco y tablero debe ser ≥ ΔW/2 por lado, con ΔW calculado con [F4] (derivado).
- Evitar ensambles a contraveta rígidos (p. ej. un listón encolado perpendicular en toda su longitud) [F6].
- El espesor terminado no puede superar el S2S del grueso nominal comprado (4/4 → 13/16") [F10].
- Veta corta en piezas curvas o estrechas: ⚠ típico-sin-fuente (sin regla numérica; aviso cualitativo).

### 1.5 Modos de falla
Grietas por contracción restringida [F6]; acopamiento en flatsawn [F9]; juntas que se abren cuando la madera se trabaja con más humedad de la de servicio [F6]; fluencia en repisas (la madera maciza fluye menos que el aglomerado) [F31].

---

## 2. Tableros chapados y núcleos (MDF, aglomerado, contrachapado)

### 2.1 Formas y medidas
| Concepto | Valor | Fuente |
|---|---|---|
| Formato México (Arauco melamina) | 1.22 × 2.44 m | [F18] |
| Espesores MDF (Arauco MX) | 5.5, 12, 15, 18, 25 mm | [F18] |
| Espesores aglomerado PBO (Arauco MX) | 12, 16, 19, 28 mm; RH 16 mm | [F18] |
| Formato EE. UU. / APA | 4 × 8 ft ≈ 1220 × 2440 mm; también 1200 × 2400 | [F22] |
| Equivalencias de espesor | 23/32" = 18.0 mm; 3/4" = 19.1 mm; 19/32" = 15.0 mm; 5/8" = 15.9 mm | [F22] |
| Chile (referencia de LatAm) | 1830 × 2500 y 1520 × 2440; 15 y 18 mm | [F23] |
| UE: formatos 2800 × 2070 y similares | ⚠ típico-sin-fuente | — |

### 2.2 Propiedades
**Aglomerado ANSI A208.1-2009** (límites del 5.º percentil inferior) [F15]:

| Grado | MOR (MPa) | MOE (MPa) | IB (MPa) | Tornillo en cara (N) | Tornillo en canto (N) | Expansión lineal máx. |
|---|---|---|---|---|---|---|
| M-1 | 10.0 | 1550 | 0.36 | NS | NS | 0.40 % |
| M-S | 11.0 | 1700 | 0.36 | 800 | 700 | 0.40 % |
| M-2 | 13.0 | 2000 | 0.40 | 900 | 800 | 0.40 % |
| M-3i | 15.0 | 2500 | 0.50 | 1000 | 900 | 0.40 % |
| H-1 | 14.9 | 2160 | 0.81 | 1600 | 1200 | NS |

- Densidad: grados M 640–800 kg/m³; grados H > 800 kg/m³ [F15]. Tolerancias: largo y ancho ±2.0 mm; espesor ±0.20 mm (solo tableros lijados) [F15].
- **MDF grado 155** (ANSI A208.2), valores medios medidos en 5 fabricantes norteamericanos: MOR 20–32 MPa, MOE 2.7–3.5 GPa, IB 0.50–0.87 MPa, tornillo en cara 1200–1540 N, tornillo en canto 1030–1460 N, densidad del núcleo ~670–716 kg/m³ y de la superficie ~950–1126 kg/m³ [F17]. El límite ANSI de tornillo en canto citado es 1001 N [F17]. El texto de ANSI A208.2-2022 está incorporado por referencia en el CFR (copia pública en law.resource.org) [F16], pero **no se revisó**: validar.
- **Arauco México** (normas EN): MDF 15–25 mm de 595–645 kg/m³, IB ≥ 0.55 N/mm², flexión ≥ 23 N/mm², tornillo cara/canto ≥ 850/700 N, hinchamiento 24 h ≤ 12 %. PBO 16–19 mm de 660–700 kg/m³, IB ≥ 0.35, tornillo ≥ 800/700 N, hinchamiento ≤ 35 %. RH PBO hinchamiento ≤ 8 % [F18]. Tolerancias EN 324: espesor ±0.3 mm (MDF) y +0.5/−0.3 mm (PBO) [F18].
- **Formaldehído (EE. UU., TSCA Title VI):** HWPW 0.05 ppm; aglomerado 0.09; MDF 0.11; MDF delgado 0.13 [F82].

### 2.3 Procesos
Seccionadora con incisor, CNC nesting, enchapado (prensa caliente o de membrana), cubrecanto, taladrado en línea (System 32), barnizado o laqueado (MDF).

### 2.4 Restricciones DFM
- **Construcción balanceada:** usar el mismo material, espesor y especie en ambas caras; las chapas sobre MDF/PB deben laminarse con 6–9 % de CH y el mismo espesor y grado en las dos caras [F21]. En HPL, las direcciones de máquina de la cara y del contrabalance deben quedar paralelas [F21]. Acondicionar laminado y sustrato al menos **2 semanas** en las mismas condiciones [F21].
- **Tornillos en MDF:** en cara, ≥ 12 mm del canto y ≥ 25 mm de esquinas; en canto, ≥ 70 mm de esquinas; agujero guía del 85–95 % del diámetro de núcleo, 1 mm más profundo que la inserción [F19]. Weyerhaeuser da valores similares en pulgadas: en cara ≥ 1" de esquinas, en canto ≥ 3" de esquinas, guía ≥ 90 % del diámetro de raíz. Pasar de 0.75" a 1" de profundidad en cara sube la sujeción ~30 % [F20].
- **Tarugos en canto:** diámetro ≤ 50 % del espesor del tablero; barreno 0.005" sobremedida y 1/32"–1/16" más profundo que el tarugo [F20].
- **Grapas:** en cara ≥ 1/2" del canto y ≥ 1" de esquinas; en canto ≥ 3/4" de esquinas, calibre ≤ 14 [F20].
- **Repisas (pandeo):** el ojo nota ~1/32" de flecha por pie. Claros máximos sin pandeo con 20 psf y repisa de 10" de fondo: aglomerado 3/4" = 26" (660 mm); contrachapado 3/4" = 32" (813 mm); contrachapado con un listón de 1-1/2" = 42" [F31]. El aglomerado es el material con más fluencia [F31].
- **Juntas de dilatación** en tramos largos: obligatorias, pero sin valor numérico en [F21].

### 2.5 Modos de falla
Alabeo por desbalance o por CH distinta entre las caras [F21]; hinchamiento de cantos expuestos (PBO hasta 35 % en 24 h [F18]); arranque de tornillos en canto; fluencia de repisas [F31]; rajado por agujero guía insuficiente [F20].

---

## 3. Tableros melamínicos / TFL (baja presión)

- **Stock:** 1.22 × 2.44 m sobre MDF o PBO (Arauco MX), con espesores según el sustrato (§2.1) [F18]. El blanco en 12 mm solo se fabrica en diseños blancos [F18].
- **Propiedades:** las del sustrato (§2.2), más la abrasión Taber (UNE 7265) y la norma de producto UNE-EN 14322 [F18].
- **Procesos:** corte con **incisor**, disco de > 96 dientes de carburo y avance ≤ 25 m/min [F18]; nesting CNC; cubrecanto.
- **DFM:**
  - Uso **solo interior**; en baños o costa usar RH PBO [F18].
  - Cubrecanto en todos los cantos expuestos o en contacto con piso, techo o pared; en zonas húmedas, en **todos** los cantos [F18].
  - La melamina no se puede posformar (laminado de baja presión; ⚠ típico-sin-fuente).
  - Cara y contracara balanceadas (§2.4) [F21]. Los LPL suelen dilatarse más que el PB o el MDF [F21].
- **Fallas:** desportillado del decorativo al cortar sin incisor [F18]; hinchamiento en cantos sin sellar [F18]; alabeo por desbalance [F21].

---

## 4. Laminado de alta presión (HPL)

### 4.1 Stock y grados
| Grado (Wilsonart) | NEMA/ISO | Espesor nominal | Fuente |
|---|---|---|---|
| 107 General Purpose | HGS | 0.048" ± 0.005 (1.22 mm) (ficha 2020) / 0.045" (ficha web) | [F33] / [F32] |
| 335 Vertical | VGP | 0.028" (0.7 mm) | [F32][F33] |
| 350 Postforming | HGP | 0.039" ± 0.005 (0.99 mm) (2020) / 0.036" (web) | [F33] / [F32] |
| Nevamar F39 | HGP | 0.039" (1.0 mm) | [F34] |
| Nevamar F28 | VGP | 0.028" (0.7 mm) | [F34] |

> Las dos fichas de Wilsonart no coinciden (versiones distintas). En el catálogo hay que **fechar la fuente** y versionar el registro.

- **Formatos:** anchos de 36", 48" y 60" (914 / 1219 / 1524 mm); largos de 96", 120" y 144" (2438 / 3048 / 3658 mm) [F32][F33].
- **Estabilidad dimensional** típica (dirección de máquina / transversal): 107 = 0.3/0.7 %; 335 y 350 = 0.5/0.8 %; máximos ISO 1.1/1.4 % [F32].

### 4.2 Procesos
Encolado sobre PB/MDF/contrachapado (cola de contacto, PVA o urea) [F33]; posformado a 163–170 °C durante 20 ± 5 s [F33]; ruteado.

### 4.3 DFM
- **Radios de posformado:** Wilsonart 350, radio de cara 5/16" (7.9 mm) y radio posterior 3/8" (9.5 mm); referencia ISO 4586-3 HGP: 9/16" (14.3 mm) cara y 3/4" (19 mm) posterior [F33]. Nevamar HGP típico 3/8" (10 mm), **mínimo NEMA 5/8" (16 mm)**; VGP típico 1/4" (7 mm), mínimo NEMA 1/2" (13 mm) [F34].
- **Esquinas interiores** de recortes: radio ≥ **1/8" (3.2 mm)**; nunca esquinas a escuadra, porque se agrietan [F32][F33].
- Barrenos para tornillo **sobredimensionados** y avellanados por la cara [F32].
- **No estructural**; el sustrato debe ser de calidad (no yeso ni concreto) [F33].
- Acondicionar cara, contrabalance y sustrato **48 h a ~24 °C y 45–55 % HR** [F32].
- Usar HGS o HGP en horizontales; VGP (0.7 mm) solo en verticales (derivado de la designación de grado [F33]).
- **Fallas:** grietas por esfuerzo en esquinas rectas [F32]; alabeo por contrabalance faltante o con veta cruzada [F21]; delaminación.

---

## 5. Piedra natural (mármol, granito)

- **Stock:** losas de 2 cm (3/4") y 3 cm (1-1/4") [F35][F36]. Tamaños de losa variables por cantera: ⚠ típico-sin-fuente (≈ 2.8–3.2 × 1.6–2.0 m).
- **Propiedades:** las normas ASTM C615 (granito) y C503 (mármol) fijan absorción, densidad, resistencia a compresión, módulo de ruptura, abrasión y flexión [F42], pero **sus límites numéricos están en el texto de pago** y no se verificaron. ⚠ típico-sin-fuente: granito ~2.6–2.7 g/cm³ con absorción ≤ 0.4 %; mármol ~2.6–2.8 g/cm³ con absorción ≤ 0.2 %.
- **Procesos:** corte con disco o waterjet, CNC de puente, pulido de cantos, perforación con broca de corona, encolado de juntas con epoxi o poliéster, varillado (rodding).
- **DFM:**
  - **Voladizo sin soporte:** 2 cm ≤ 6"; 3 cm ≤ 10"; la parte en voladizo ≤ 1/3 del ancho de la cubierta. Para 3 cm el *Dimension Stone Design Manual* (Natural Stone Institute) admite un claro libre de 3 ft entre apoyos. Son **recomendaciones, no norma obligatoria** [F35].
  - **Recortes de tarja/parrilla:** esquinas interiores con radio ≥ 3/8" (9.5 mm) [F36] (dato para cuarzo; usar también para piedra natural ⚠ validar). Cambria pide ≥ 1/2" en tarjas integrales [F37].
  - **Juntas:** evitarlas sobre recortes de calor o en tramos en voladizo ⚠ típico-sin-fuente (para solid surface ver §7: ≥ 300 mm del recorte de la parrilla [F47]).
- **Fallas:** fractura en voladizos o recortes con esquinas vivas; manchas (mármol poroso y sensible a ácidos); grietas en las bandas estrechas junto a la tarja.

---

## 6. Cuarzo de ingeniería

- **Stock:** 2 cm y 3 cm; MSI también ofrece subcubierta de 5/8" [F36]. Formatos "jumbo": ⚠ típico-sin-fuente (≈ 3200 × 1600 mm).
- **Propiedades (Silestone, por familia)** [F38]: densidad 2133–2453 kg/m³; absorción ≤ 0.05 % (EN 14617-1); flexión ≥ 25 MPa (F3) a ≥ 40 MPa (F4); **dilatación térmica 27–45 × 10⁻⁶ /°C** (EN 14617-11), mayor que la del granito; conductividad 1.3 W/m·K; reacción al fuego A2.
- **DFM:**
  - **Voladizo sin soporte (MSI, gabinete de 24"):** 2 cm < 8"; 2 cm con subcubierta < 12"; 3 cm < 16". Más allá se requieren ménsulas cada 24" o patas o paneles [F36]. Hay que tomar el valor **más conservador** entre [F35] y [F36] (6"/10" frente a 8"/16").
  - Esquinas de recortes y esquinas interiores en L/U: radio ≥ 3/8" [F36].
  - Holgura de 1/8" entre tarja o parrilla y el recorte, y entre la cubierta y el muro, por dilatación [F36]. Base plana dentro de 1/16" en 18" [F36].
  - **No exponer a > 250 °F (121 °C)** ni a choque térmico [F36]; el choque térmico no se considera defecto en garantía [F40].
  - **Exterior/UV:** el cuarzo estándar es solo para interior; la radiación UV lo decolora y debilita [F39]. La garantía excluye "cualquier aplicación exterior" (existe una línea Outdoor aparte) [F40].
  - **Salud:** sílice cristalina respirable. Australia prohibió el uso, suministro y fabricación de piedra de ingeniería desde el **1 de julio de 2024** [F41]. En Kubit esto es un atributo informativo de "riesgo de proceso".
- **Fallas:** decoloración en exterior [F39]; fractura por choque térmico [F40]; grietas en recortes con esquinas vivas.

---

## 7. Superficie sólida (acrílica, p. ej. Corian, HIMACS, Krion)

- **Stock (Corian):** 6 mm de 2490 × 760/930 mm (solo vertical); 12 mm de 3658 × 760/930 mm; 19 mm de 3658 × 760 mm [F43].
- **Propiedades (Corian):** densidad 1.7 g/cm³; flexión 71 MPa; módulo de flexión 8.8 GPa; **dilatación 3.9 × 10⁻⁵ /°C**; 22 kg/m² en 12 mm [F44].
- **Procesos:** corte, ruteado, unión con adhesivo invisible, **termoformado**, tarjas integrales.
- **DFM:**
  - **Termoformado:** HIMACS a 150–170 °C, ~1 min por mm. Radio interior mínimo ≈ **30 mm (6 mm), 75 mm (9 mm), 150 mm (12 mm)** [F45]. Krion a ~160 °C, con radio dependiente del color: 20–240 mm en 12 mm y 20–100 mm en 6 mm. Curvado en frío: 900 mm (6 mm) y 1800 mm (12 mm) [F46]. → **El radio depende del color y del grosor:** el catálogo debe tenerlo por SKU.
  - **Recortes:** radio interior ≥ **5 mm** y cantos del recorte con radio ≥ 3 mm [F47].
  - **Parrilla:** juntas ≥ **300 mm** del recorte; recorte ≥ 50 mm del salpicadero y ≥ 60 mm del muro; nunca una junta sobre un elemento de calor; bloques de refuerzo de 150 × 150 mm en las esquinas [F47].
  - **Juntas:** tira de refuerzo de 50–100 mm por debajo [F48]; esquina interior con radio ≥ 5 mm [F48].
  - Acondicionar a 20 ± 5 °C durante 24 h [F47].
- **Fallas:** grietas por calor en las esquinas de la parrilla; ruptura del termoformado bajo el radio mínimo; juntas visibles.

---

## 8. Acero: lámina y perfiles (al carbono, inoxidable 304/316, galvanizado)

### 8.1 Stock
| Calibre | Acero (in/mm) | Galvanizado | Inoxidable | Aluminio |
|---|---|---|---|---|
| 10 | .135 / 3.416 | .138 / 3.510 | .141 / 3.571 | .102 / 2.588 |
| 12 | .105 / 2.657 | .108 / 2.753 | .109 / 2.779 | .081 / 2.052 |
| 14 | .075 / 1.897 | .079 / 1.994 | .078 / 1.984 | .064 / 1.628 |
| 16 | .060 / 1.519 | .064 / 1.613 | .063 / 1.588 | .051 / 1.290 |
| 18 | .048 / 1.214 | .052 / 1.311 | .050 / 1.270 | .040 / 1.024 |
| 20 | .036 / 0.912 | .040 / 1.006 | .038 / 0.953 | .032 / 0.813 |
Fuente: [F52]. **El mismo calibre equivale a espesores distintos según el material**, así que Kubit debe guardar el espesor en mm y el calibre solo como etiqueta.

- **México (Ternium), tubo estructural cuadrado/rectangular ASTM A500 ("PTR"):** de 1" a 5" cuadrado y rectangulares de 1-1/2×1" a 6×4"; calibre 16 = 1.52 mm, 14 = 1.90, 12 = 2.67, 11 = 3.05, 10 = 3.43 mm, hasta 1/4" (6.35 mm). Largo de 6.00 m (6.10 m en espesores > cal. 7) [F53].
- Lámina en hojas de 4 × 8 ft / 4 × 10 ft: ⚠ típico-sin-fuente.

### 8.2 Propiedades
| Material | Fy | Fu | E | Densidad | α (dilatación) | Fuente |
|---|---|---|---|---|---|---|
| ASTM A36 | 250 MPa (36 ksi) | 400 MPa (58 ksi) | 200 GPa | 7850 kg/m³ | 12 × 10⁻⁶ /°C | [F54] |
| Inox 304/304L | 30 ksi (≈ 207 MPa) rep. | 75 ksi rep. | 29 × 10⁶ psi (≈ 200 GPa) | 0.285 lb/in³ (≈ 7.9 g/cm³) | 9.2 µin/in·°F (≈ 16.6 × 10⁻⁶ /°C) | [F55] |
| Inox 316/316L | ≥ 30 ksi | ≥ 75 ksi | 29 × 10⁶ psi | 0.29 lb/in³ | 9.2 µin/in·°F | [F56] |

- Composición del 304: Cr 18–20 %, Ni 8–10.5 % [F55]. El 316 añade **Mo 2–3 %**, que mejora la resistencia a picadura por cloruros [F56]. → Para cocinas de hotel en costa o con contacto salino, sugerir 316.

### 8.3 Procesos
Corte láser, plasma o punzonado; plegado en prensa; soldadura MIG/TIG/punto; galvanizado por inmersión; pintura en polvo; pulido (inox).

### 8.4 DFM (lámina)
- **Radio interior de doblez:** acero dulce ≥ 1T (al aire 1T–2T, según la abertura de la V); aluminio blando (3003/5052) ≈ igual que el acero; **6061/2024 entre 4T y 8T** [F49]. Protolabs usa un radio estándar de 0.030" (0.76 mm) para ≤ 1/8" [F51].
- **Diámetro mínimo de barreno:** ≥ T [F49][F51].
- **Barreno a borde:** ≥ 0.062" (1.6 mm) en ≤ 0.036"; ≥ 0.125" (3.2 mm) en material más grueso [F51].
- **Barreno a doblez:** ≥ **2T desde el inicio del radio** [F49]; Protolabs pide ≥ **4T desde el exterior del doblez** [F51]; SendCutSend recomienda ≥ ½ del ancho del dado (p. ej. 0.315" para acero de 0.104") [F50]. → Usar el **máximo** de los tres criterios por defecto.
- **Pestaña mínima:** ≥ 4T [F49][F51]. Ejemplo: 5052 de 0.250" → pestaña plana mínima de 1.150" [F50].
- **Alivio de doblez:** ≥ T + R [F49].
- **Tolerancias típicas:** ángulo ±1°; barreno a barreno ±0.005"; doblez a barreno ±0.015" [F51].

### 8.5 Soldar galvanizado e inoxidable
- La soldadura de galvanizado libera **humos de zinc** que causan **fiebre de los humos metálicos** (síntomas gripales a las pocas horas) [F58]. OSHA recomienda limpiar los recubrimientos de la superficie, usar extracción local y, si no basta, protección respiratoria [F57].
- La soldadura de inoxidable genera **Cr(VI)**, con PEL OSHA de **5 µg/m³** (8 h) [F57].
- Tras soldar galvanizado se pierde la protección en la zona afectada. Hay que reparar (p. ej. con galvanizado en frío según ASTM A780; ⚠ validar alcance de la norma).
- **Fallas:** corrosión en soldaduras de galvanizado; picadura del 304 en ambiente con cloruros [F56]; grietas en dobleces de 6061 con radios cerrados [F49]; deformación de barrenos cerca del doblez [F50].

---

## 9. Aluminio (lámina y extrusiones 6061/6063; anodizado)

- **Propiedades:**

| Aleación | Fy | Fu | E | ρ | α | Fuente |
|---|---|---|---|---|---|---|
| 6061-T6/T651 | 276 MPa | 310 MPa | 68.3 GPa | 2.70 g/cm³ | 23.6 µm/m·K | [F60] |
| 6063-T5 | 145 MPa | 186 MPa | 68.9 GPa | 2.7 g/cm³ | 23.4 µm/m·°C | [F59] ⚠ (copia de MatWeb/ASM: solo referencia, **no ingerir**) |

- **Extrusión:** las extrusoras típicas producen perfiles de CCD < 12"; se recomienda **CCD < 8"** y ≤ 3 lb/ft [F61]. Paredes uniformes y balanceadas, con transiciones redondeadas [F61][F62]. Esquinas exteriores ≥ 0.020" (0.5 mm) e interiores ≥ 0.015" (0.4 mm) [F62]; Hydro sugiere radios de 0.5–1 mm. Relación ancho:alto de canal ~1:3 (1:4 con radio en la boca), y canales < 3 mm se revisan caso por caso [F63]. Espesor mínimo de pared: **no se encontró valor** (depende de la aleación y del perfil [F63]; ⚠ típico-sin-fuente ≈ 1–1.5 mm en 6063).
- **Anodizado arquitectónico (AAMA 611):** Clase I ≥ 0.7 mil (18 µm) para exterior de alto desempeño; Clase II ≥ 0.4 mil (10 µm) [F64]. MIL-A-8625 Tipo III asume 2.0 mil si no se especifica [F64]. Crecimiento dimensional ~50 % hacia dentro y 50 % hacia fuera: ⚠ típico-sin-fuente.
- **Lámina:** se aplican las reglas de §8.4; el 6061 necesita radios de 4–8T [F49].
- **Fallas:** grietas al doblar 6061-T6; color de anodizado no uniforme entre lotes o aleaciones ⚠; corrosión galvánica en contacto con acero ⚠ típico-sin-fuente.

---

## 10. Fibra de vidrio / FRP

- **Propiedades aproximadas (laminado con resina)** [F68]: mat picado 0.058 lb/in³ (≈ 1.6 g/cm³), 22,000 psi (≈ 152 MPa) y 1.4 × 10⁶ psi (≈ 9.7 GPa); roving tejido 40,000 psi y 2.0 × 10⁶ psi. **No son parámetros de diseño** [F68]. Moldeo por contacto: ~33 % de vidrio y 1578 kg/m³ [F69].
- **Procesos:** moldeo manual por contacto, aspersión, infusión, RTM.
- **DFM (contacto/manual):** **ángulo de salida ≥ 2°** [F69] (2–3° según [F70]); **radio ≥ 6 mm** y **espesor ≥ 3 mm** [F69]; sin límite de tamaño, y adecuado para < 1000 piezas/año [F69].
- **Fallas:** burbujas en esquinas cerradas (el radio debe ser proporcional al espesor [F70]); ampollas por ósmosis ⚠; desprendimiento del gelcoat ⚠.

---

## 11. Impresión 3D (FDM, SLA, SLS/MJF)

| Parámetro (mm) | FDM | SLA | SLS | MJF | Fuente |
|---|---|---|---|---|---|
| Pared mín. soportada | 0.75 (1.0 en [F67]) | 0.4 | 0.5 | 0.7 | [F66] |
| Pared mín. no soportada | 1.0 | 0.6 | 0.6 | 0.7 | [F66] |
| Detalle mínimo | 1.2 (2.0 en [F67]) | 0.5 | 0.5 | 0.5 | [F66] |
| Barreno mínimo | 1 (2.0 en [F67]) | 0.75 | 0.8 | 0.8 | [F66] |
| Holgura para partes móviles | 0.5 | 0.5 | 0.7 | 0.7 | [F66] |
| Ángulo sin soporte | 45° | 19° desde horizontal | n/a | n/a | [F66][F65] |
| Volumen máx. | 900 × 600 × 900 (estándar 350³) | 736 × 635 × 533 | 700 × 380 × 580 | 380 × 284 × 380 | [F66] |

- **SLA (Formlabs Form 2):** voladizo horizontal ≤ 3 mm; puente de 21 mm (viga de 5 × 3 mm); grabado ≥ 0.4 mm; relieve ≥ 0.1 mm; **barreno de drenaje ≥ 3.5 mm** en cavidades cerradas [F65].
- **FDM:** texto ≥ 14 pt; relieve ≥ 1 mm; pin ≥ 3 mm [F67]. **Anisotropía en Z** (menor resistencia entre capas): ⚠ típico-sin-fuente.
- **SLS/MJF:** barrenos de escape para el polvo en piezas huecas, ⚠ típico-sin-fuente (≈ ≥ 5 mm, 2 por cavidad).
- **Fallas:** delaminación entre capas (FDM); copas de succión y blowouts en SLA sin drenaje [F65]; piezas que se fusionan por holgura insuficiente [F65][F66]; alabeo en piezas planas grandes ⚠.

---

## 12. Vidrio (recocido, templado, laminado)

- **Propiedades del vidrio flotado sodocálcico** [F72]: 2500 kg/m³; E ≈ 71.7 GPa; ν ≈ 0.22; α ≈ 8.3 × 10⁻⁶ /°C; ablandamiento a 715–729 °C.
- **Templado vs termoendurecido (ASTM C1048):** el termoendurecido tiene compresión superficial de 3,500–7,500 psi (24–52 MPa), ~2× la resistencia del recocido. El **templado tiene ≥ 10,000 psi (69 MPa)** en superficie y ≥ 9,700 psi en canto, ~4× la del recocido [F73].
- **Regla dura:** todo corte, barreno, muesca y canteado **se hace antes del tratamiento térmico**; el vidrio templado no se puede cortar ni barrenar después [F73]. Es la regla clave de **orden en la cadena de procesos** de Kubit.
- **Barrenos en vidrio tratado térmicamente (Viracon)** [F71]:
  - Diámetro ≥ espesor + 1/16" y ≥ 1/4" (6 mm); diámetro ≤ 1/3 de la dimensión menor.
  - Barreno a borde ≥ max(6 mm, 2t); barreno a barreno ≥ max(10 mm, 2t); **barreno a esquina ≥ 6.5t**.
  - Máximo 10 barrenos por pieza; ancho mínimo de la pieza de 8" (203 mm) para templar con barrenos.
  - Recortes no circulares con radio ≥ t y ≥ 1/2".
- **Laminado:** PVB de 0.38, 0.76 y 1.52 mm; configuraciones de 3+3 a 10+10 mm; formato máximo de 3210 × 6000 mm. Se corta en mesa especial para laminado [F74].
- **Vidrio de seguridad:** en EE. UU., CPSC 16 CFR 1201 y ANSI Z97.1 [F73]. En **México es obligatoria la NOM-146-SCFI-2016** (Clase I templado y Clase II inastillable/laminado; cubre canceles, puertas, barandales, fachadas…) y exige marcado permanente en el templado [F75].
- Espesores comerciales en México (3, 4, 6, 9, 12, 19 mm) y hoja de 2.60 × 3.60 m: ⚠ típico-sin-fuente.
- **Fallas:** rotura espontánea por inclusiones de NiS ⚠; rotura por barreno cerca del borde o de la esquina [F71]; intento de modificar vidrio ya templado [F73].

---

## 13. Tapicería (espumas, telas, estimación de metraje)

- **Espuma de poliuretano flexible:** densidades de 0.8–6.0 pcf; en muebles normalmente 0.9–2.5 pcf; espuma HC de 1.8–2.2 pcf [F77]. IFD25 de 7 a 45 lbf (supersuave 7–10) [F77]. **Factor de soporte** IFD65/IFD25 ≈ 1.8–2.6 [F77]. Para asientos, la PFA indica mejor desempeño con **densidad de polímero ≥ 1.8 pcf** (≈ 29 kg/m³) y una pérdida de IFD ≤ 6 lb en el roller shear para espuma de 4" [F76]. El IFD se mide al 25 % de compresión con un plato de 50 in² sobre una muestra de 15×15×4" o mayor [F76].
- **Rangos de IFD por uso** (asiento, respaldo, brazo): la tabla de [F77] no fue legible. ⚠ típico-sin-fuente: asiento ≈ 30–45 lbf; respaldo ≈ 15–25 lbf.
- **Telas:** ancho estándar de **54"** (137 cm) [F78]. Abrasión mínima ACT: tráfico bajo ≥ 15,000 ciclos Wyzenbeek / 20,000 Martindale; tráfico alto ≥ 30,000 / 40,000 [F80].
- **Metraje (fundas, tela lisa de 54")** [F79]: sofá de 3 cojines = 18 yd; sofá de 2 cojines = 16 yd; loveseat de 2 cojines = 13 yd; sillón orejero = 11 yd; slipper chair = 4 yd; otomana = 3½ yd. **Sumar ~15 % si hay que casar el patrón** [F78]. Para Kubit, mejor calcular el metraje con el **nesting de patrones desarrollados** sobre el ancho del rollo, sumando el rapport del dibujo.
- **Flamabilidad EE. UU.:** **16 CFR 1640** (adopta CA TB 117-2013, sin combustión lenta) para tapizado interior fabricado desde el 25 de junio de 2021. Etiqueta permanente obligatoria desde el 25 de junio de 2022 [F81].
- **Fallas:** espuma de baja densidad que pierde firmeza [F76][F77]; tela con abrasión insuficiente para hospitalidad [F80]; tela que no alcanza por el rapport del patrón [F78].

---

## A. Fuentes de datos abiertas / redistribuibles y sus licencias

| Fuente | Estado legal | Uso en Kubit (CC-BY-4.0) |
|---|---|---|
| **USDA FPL Wood Handbook** (2021 FPL-GTR-282; 2010 GTR-190; 1999 GTR-113) [F2][F3][F4][F5] | Obra del gobierno de EE. UU. → sin copyright en EE. UU. (17 U.S.C. §105 [F1]). El PDF no trae declaración explícita [F2]. | ✅ **Fuente principal para madera.** Citar edición, capítulo y tabla. Revisar si alguna figura es de terceros. |
| Otras publicaciones de USDA FS / Treesearch, NIST, OSHA, EPA, CPSC (p. ej. [F57][F82][F81]) | Ídem, obras federales [F1] | ✅ Citar. |
| Extensión universitaria (Purdue [F6], NCSU [F9]) | Con copyright de la universidad | ⚠ Citar el dato y re-derivarlo; no copiar las tablas. |
| Fichas de fabricantes (Arauco, Wilsonart, Blum, Corian, Silestone…) | Con copyright | ⚠ Los **datos numéricos aislados** pueden citarse como hechos (*Feist* [F14]; LFDA art. 107: la protección "no se extenderá a los datos" [F12]). No copiar el documento ni su maquetación. Guardar la URL y la fecha por registro. |
| **MatWeb** | Prohíbe reproducir, redistribuir, ponerlo a disposición de terceros, exportar en masa, crear bases derivadas públicas o > 500 materiales, y **entrenar IA** [F11] | ❌ **No usar como fuente de ingestión.** Ojo con las hojas "de fabricante" derivadas de MatWeb/ASM [F59]. |
| Normas (ANSI, ASTM, EN, NEMA, AWI, BIFMA) | Con copyright y de pago | ❌ No reproducir el texto. ✅ Referenciar por número y título; usar valores solo cuando un resumen público los publique (p. ej. A208.1 en hud.gov [F15], A208.2 incorporada por referencia en law.resource.org [F16]). |
| Wikipedia (p. ej. System 32 [F24]) | CC BY-SA (share-alike) | ⚠ El texto **no es compatible** con publicar bajo CC-BY puro. Usar solo como pista y citar la fuente primaria. |
| Calculadoras y blogs (Engineering Toolbox, Wood Database, woodbin) | Con copyright | ⚠ Solo como referencia cruzada. |
| UE: derecho sui generis sobre bases de datos (Dir. 96/9/CE) | 15 años sobre bases con "inversión sustancial"; prohíbe extraer partes sustanciales o extraer de forma sistemática [F13] | ⚠ No extraer en masa de bases europeas (p. ej. catálogos de proveedores de la UE). |

**Recomendaciones de licencia del catálogo:**
1. Cada registro debe llevar `source_url`, `source_title`, `retrieved_at`, `license_basis` (`us-gov-pd` | `fact-citation` | `own-measurement`) y `confidence` (`verified` | `typical-unverified`).
2. Las mediciones propias de Evita/talleres (p. ej. extracción de tornillos en tablero mexicano) son la vía más limpia para publicar bajo CC-BY.
3. Mantener una lista negra de dominios (MatWeb y sus espejos, copias piratas de normas ASTM).

---

## B. Organismos y normas de referencia (solo nombres y alcance)

| Norma / organismo | Alcance | Fuente |
|---|---|---|
| **AWI** – ANSI/AWI 100 (Submittals), 200 (Care & Storage), 300 (Materials), 0620 (Finish Carpentry/Installation), 0641 (Architectural Wood Casework). Sustituyen al libro AWS (AWI/AWMAC/WI). Grados estéticos Economy/Custom/Premium y duty levels 1–4 (por defecto Custom + nivel 3) | Carpintería arquitectónica y casework | [F83] |
| **ANSI/KCMA A161.1** | Desempeño de gabinetes de cocina (14 pruebas: estructurales, puertas, cajones y acabado; p. ej. 600 lb en gabinete alto y 25,000 ciclos) | [F85] |
| **ANSI/BIFMA** X5.1 (sillas), X5.4 (lounge), X5.5 (escritorios y mesas), X5.9 (almacenamiento), X6.5 (home office), X7.1 (emisiones), e3 (sustentabilidad), X10.1 (ergonomía) | Mobiliario de oficina, contract y hospitalidad | [F84] |
| **ANSI A208.1 / A208.2** (Composite Panel Association) | Aglomerado / MDF | [F15][F16] |
| **NEMA LD3 / ISO 4586 / EN 438** | HPL | [F34][F33] |
| **EN 14322, EN 323/319/310/320/317/324** | Tableros melamínicos y sus ensayos | [F18] |
| **EPA TSCA Title VI** (40 CFR 770) / CARB | Emisión de formaldehído en tableros (obligatorio para vender en EE. UU.) | [F82] |
| **ASTM C1048**, ANSI Z97.1, **CPSC 16 CFR 1201** | Vidrio termoendurecido y templado; vidrio de seguridad | [F73] |
| **NOM-146-SCFI-2016** | Vidrio de seguridad en construcción (México, obligatoria) | [F75] |
| **ASTM C615 / C503** | Granito / mármol | [F42] |
| **EN 14617** | Piedra aglomerada (cuarzo): métodos de ensayo | [F38] |
| **AAMA 611**, MIL-A-8625 | Anodizado | [F64] |
| **ASTM A36, A500, A513, A653** | Acero estructural, tubo y galvanizado | [F54][F53] |
| **16 CFR 1640 (TB 117-2013)**, ACT Guidelines | Flamabilidad de tapizados; desempeño de textiles contract | [F81][F80] |
| **NOM mexicanas para tableros o muebles** | **No se encontró** una NOM obligatoria específica para tableros de madera o muebles. Arauco MX cita normas EN, no NMX [F18]. Brecha a investigar (NMX de ONNCCE, PROFECO). | — |

---

## C. Propuesta de 45 reglas DFM iniciales (MVP mobiliario/millwork)

**Tipos:** `attr` = atributo del material o proceso; `geom` = geometría de una pieza; `asm` = ensamble o relación entre piezas.
**Severidad:** `error` (no fabricable o incumple una norma obligatoria) · `warn` (riesgo alto) · `info` (recomendación).
Todos los parámetros son **valores por defecto que el taller puede sobrescribir**.

| ID | Familia | Tipo | Sev. | Mensaje corto | Parámetros por defecto | Fuente |
|---|---|---|---|---|---|---|
| PNL-001 | Tablero | geom | error | La pieza no cabe en el formato de tablero | stock = {1220×2440 (MX/US), 1830×2500}; refilado = 10 mm/lado ⚠ | [F18][F22][F23] |
| PNL-002 | Tablero | attr | warn | Espesor no disponible en el catálogo del proveedor | MDF {5.5,12,15,18,25}; PBO {12,16,19,28} mm | [F18] |
| PNL-003 | Tablero | geom | warn | Dirección de veta/decorado incompatible con la dimensión larga | veta_requerida = true; largo > ancho_stock | ⚠ típico-sin-fuente |
| PNL-004 | Tablero | attr | warn | Construcción desbalanceada: cara ≠ contracara | comparar material, espesor y dirección de máquina | [F21] |
| PNL-005 | Tablero | attr | warn | Chapa sobre MDF/PB fuera del CH de laminado | CH ∈ [6, 9] % | [F21] |
| PNL-006 | Melamina/PB | attr | warn | Zona húmeda con sustrato no RH | zona ∈ {baño, costa, tarja} → sustrato RH | [F18] |
| PNL-007 | Melamina/PB | attr | warn | Canto expuesto sin cubrecanto | todos los visibles o en contacto con piso/muro; en zona húmeda, todos | [F18] |
| PNL-008 | MDF/PB | geom | warn | Tornillo en cara muy cerca del canto o de la esquina | canto ≥ 12 mm; esquina ≥ 25 mm (US: ≥ 1") | [F19][F20] |
| PNL-009 | MDF/PB | geom | warn | Tornillo en canto muy cerca de la esquina | ≥ 70 mm (US: ≥ 3") | [F19][F20] |
| PNL-010 | MDF/PB | attr | info | Diámetro del agujero guía fuera de rango | 85–95 % del diámetro de núcleo; profundidad = inserción + 1 mm | [F19] |
| PNL-011 | MDF/PB | geom | warn | Tarugo demasiado grueso para el tablero | Ø_tarugo ≤ 0.5·t | [F20] |
| PNL-012 | MDF/PB | attr | info | Ajuste del barreno de tarugo | +0.005" de diámetro; +1/32"–1/16" de profundidad | [F20] |
| PNL-013 | Tablero | asm | warn | Repisa con riesgo de pandeo | flecha ≤ 1/32"/ft; claro máx. 20 psf: PB 3/4" = 26", ply 3/4" = 32" | [F31] |
| PNL-014 | Tablero | geom | error | Radio interior menor que el radio de la fresa CNC | r_int ≥ Ø_fresa/2; Ø_fresa por defecto = 6 mm ⚠ | ⚠ típico-sin-fuente |
| PNL-015 | Tablero | asm | warn | Ranura/dado sin tolerancia de espesor | ancho ≥ t_nom + tol (MDF ±0.3; PBO +0.5/−0.3 mm) | [F18] |
| PNL-016 | Tablero | attr | error | Mercado US sin certificación TSCA Title VI | HWPW 0.05, PB 0.09, MDF 0.11, MDF delgado 0.13 ppm | [F82] |
| PNL-017 | Tablero | attr | info | Grado de aglomerado por debajo del uso estructural | repisas/costados → ≥ M-2 (tornillo cara 900 N / canto 800 N) | [F15] |
| HPL-001 | HPL | geom | error | Esquina interior de recorte sin radio | r ≥ 3.2 mm (1/8") | [F32][F33] |
| HPL-002 | HPL | geom | error | Radio de posformado menor que el mínimo del grado | HGP: típ. 10 mm, mín. NEMA 16 mm; VGP: típ. 7, mín. 13 mm | [F34][F33] |
| HPL-003 | HPL | attr | warn | Grado vertical (VGP 0.7 mm) en superficie horizontal | horizontal → HGS/HGP | [F33] |
| HPL-004 | HPL | attr | warn | HPL sin contrabalance o con dirección de máquina cruzada | backer = true; dir_máquina ∥ | [F21] |
| HPL-005 | HPL | attr | info | Acondicionar antes de laminar | 48 h, 24 °C, 45–55 % HR | [F32] |
| EB-001 | Cubrecanto | geom | error | Radio interior menor que el mínimo de la enchapadora | r ≥ 25 mm + espesor del canto (máquina de contorno de ref.) | [F29] |
| EB-002 | Cubrecanto | geom | warn | Esquina interior a 90° con cubrecanto continuo | requiere radio (EB-001) o canteo manual | ⚠ típico-sin-fuente |
| EB-003 | Cubrecanto | attr | warn | Espesor de canto fuera del rango de la máquina | 0.5–3.0 mm (canto ≤ 30 mm de alto); 0.5–1.0 mm (≤ 60 mm) | [F29] |
| EB-004 | Cubrecanto | attr | warn | Canto de PVC junto a una fuente de calor | Vicat PVC ≈ 72 °C; ABS ≈ 85 °C | [F30] |
| EB-005 | Cubrecanto | geom | info | Ancho del canto insuficiente para el refilado | ancho ≥ t_tablero + 2 mm ⚠ | ⚠ típico-sin-fuente |
| HW-001 | Herraje | geom | error | Barrenado System 32 fuera de patrón | paso 32 mm; Ø 5 mm; prof. 12–14 mm; retiro 37 mm | [F24] |
| HW-002 | Herraje | geom | error | Cazoleta de bisagra fuera de rango | Ø 35 mm; prof. 13 mm; distancia al canto 3–7 mm | [F25] |
| HW-003 | Herraje | geom | warn | Puerta demasiado delgada para la cazoleta | t_puerta ≥ 13 mm + remanente ⚠ (por defecto 16 mm) | [F25] + ⚠ |
| HW-004 | Herraje | asm | warn | Número de bisagras insuficiente para alto o peso de la puerta | 2 ≤ 900 mm; 3 ≤ 1600; 4 ≤ 2000 ⚠ | ⚠ típico-sin-fuente |
| HW-005 | Herraje | asm | error | Cajón con corredera oculta mal dimensionado | ancho interior del cajón = interior del mueble − 42 mm; lateral ≤ 16 mm; rebaje de fondo 13 mm | [F26] |
| HW-006 | Herraje | asm | error | Profundidad interior insuficiente para la corredera | prof ≥ perfil + retiro + 3 mm (retiro = t_frente + 5) | [F26] |
| HW-007 | Herraje | asm | warn | Altura de cajón excesiva | h_cajón ≤ hueco − 20 mm | [F26] |
| HW-008 | Herraje | asm | error | Holgura lateral para corredera de bolas | 12.7 mm (+0.79/−0) | [F27] |
| HW-009 | Herraje | geom | warn | Excéntrico Minifix mal posicionado | distancia de apriete 24 o 34 mm; Ø caja 15 mm (Minifix 15) | [F28] |
| SW-001 | Madera | asm | error | Madera maciza restringida a contraveta | ΔW = W·C_T·ΔCH; error si fijación rígida y ΔW > 1.5 mm ⚠ umbral | [F4] |
| SW-002 | Madera | attr | warn | CH especificado fuera del rango de servicio de la región | 8 % (6–10); seco 6 % (4–9); costa 11 % (8–13) | [F4] |
| SW-003 | Madera | asm | warn | Tablero en marco sin holgura de expansión | prof. ranura − inserción ≥ ΔW/2 por lado | [F4][F6] |
| SW-004 | Madera | attr | warn | Espesor terminado mayor que el S2S del grueso nominal | 4/4 → 13/16"; 5/4 → 1-1/16"; 8/4 → 1-3/4" | [F10] |
| SW-005 | Madera | geom | info | Tabla ancha de corte tangencial: riesgo de acopamiento | T/R ≈ 2; aviso si ancho > 150 mm ⚠ | [F6][F9] |
| CT-001 | Cubierta piedra/cuarzo | asm | warn | Voladizo sin soporte excesivo | 2 cm ≤ 6"; 3 cm ≤ 10"; voladizo ≤ 1/3 del ancho (conservador) | [F35][F36] |
| CT-002 | Cubiertas | geom | error | Esquina de recorte sin radio | cuarzo/piedra ≥ 9.5 mm; solid surface ≥ 5 mm; HPL ≥ 3.2 mm | [F36][F47][F32] |
| CT-003 | Cuarzo | attr | error | Cuarzo estándar en exterior/UV | exposición = exterior → error salvo línea outdoor | [F39][F40] |
| CT-004 | Solid surface | geom | error | Junta o recorte demasiado cerca de la parrilla | junta ≥ 300 mm; recorte ≥ 50 mm del salpicadero, ≥ 60 mm del muro | [F47] |
| GL-001 | Vidrio | attr | error | Operación de maquinado después del templado | orden: corte/barreno/canto → templado | [F73] |
| GL-002 | Vidrio | geom | error | Barreno en vidrio templado fuera de regla | Ø ≥ max(t + 1.6 mm, 6 mm); Ø ≤ 1/3 dim. mín.; borde ≥ max(6, 2t); entre barrenos ≥ max(10, 2t); esquina ≥ 6.5t | [F71] |
| GL-003 | Vidrio | attr | error | Vidrio no de seguridad en puerta, cancel o barandal | MX: NOM-146; US: 16 CFR 1201 / Z97.1 | [F75][F73] |
| UP-001 | Tapicería | attr | warn | Espuma de asiento de baja densidad | densidad de polímero ≥ 1.8 pcf (29 kg/m³) | [F76] |

> Son 49 filas; el núcleo del MVP son las **45** de tablero, HPL, cubrecanto, herrajes, madera y cubiertas. Las reglas GL y UP pueden ir en un segundo paquete. Las reglas marcadas ⚠ deben validarse con el taller (Evita) antes de activarlas como `error`.

**Esquema sugerido de regla (datos):**
```yaml
id: PNL-008
family: [mdf, particleboard]
check: geom            # attr | geom | asm
severity: warn
message_es: "Tornillo en cara a {dist} mm del canto; mínimo recomendado {min_edge} mm."
params: { min_edge_mm: 12, min_corner_mm: 25 }
units_display: [mm, in]
sources:
  - { id: F19, url: "https://mdf-info.eu/using-mdf/fixing/screwing", retrieved: 2026-10-26 }
confidence: verified   # verified | typical-unverified
disclaimer: assistance-only
```

---

## D. Brechas honestas y siguientes pasos

1. **Maderas mexicanas y EMC en México:** no hay datos verificados. Pedir a CONAFOR/INIFAP/UdeG o medir en Evita (Guadalajara: clima templado-seco, probablemente cercano al rango "seco" de [F4]; hipótesis sin verificar).
2. **Límites de ASTM C615/C503 y de ANSI A208.2:** no verificados (normas de pago). Leer la copia IBR de A208.2 [F16] y extraer solo los valores.
3. **Radios de cubrecanto:** solo hay una máquina de referencia [F29]. Pedir el dato de la enchapadora real del taller (Homag/Biesse/SCM).
4. **Número de bisagras por puerta, fresas CNC por defecto, refilado y holgura de canto:** sin fuente. Tomar los estándares internos de Evita.
5. **Espesor mínimo de pared en extrusión de aluminio:** sin fuente abierta (AEC remite a su manual de pago [F61]).
6. **Tamaños de losa (piedra/cuarzo) y hojas de vidrio en México:** sin fuente primaria.
7. **IFD por uso en tapicería:** la tabla de [F77] no fue legible; buscar la edición vigente de PFA InTouch.
8. **Wood Handbook 2021:** la descarga directa por capítulos está bloqueada para este agente. Se usó la edición 1999 (cap. 12) [F4]. Verificar que los coeficientes de 2021 (cap. 13 [F5]) no cambiaron.

---

## Fuentes

- **[F1]** 17 U.S.C. §105 — Subject matter of copyright: United States Government works. https://www.govinfo.gov/content/pkg/USCODE-2020-title17/html/USCODE-2020-title17-chap1-sec105.htm
- **[F2]** Forest Products Laboratory (2021). *Wood handbook — wood as an engineering material*, FPL-GTR-282 (front matter). https://www.fpl.fs.usda.gov/documnts/fplgtr/fplgtr282/front_matter_fpl_gtr282.pdf
- **[F3]** USDA Treesearch 62200 — Wood handbook 2021. https://research.fs.usda.gov/treesearch/62200
- **[F4]** Simpson, W.T. (1999). Cap. 12 "Drying and control of moisture content and dimensional changes", *Wood Handbook* FPL-GTR-113 (copia alojada por SPIB). https://blog.spib.org/wp-content/uploads/2021/07/ch12.pdf
- **[F5]** Bergman, R. (2021). Cap. 13, *Wood Handbook* FPL-GTR-282 (Treesearch 62261). https://research.fs.usda.gov/treesearch/62261
- **[F6]** Purdue Extension FNR-163, wood movement in furniture. https://www.extension.purdue.edu/extmedia/FNR/FNR-163.pdf
- **[F7]** Engineers Edge — Shrinkage values of domestic hardwoods (cita Wood Handbook 1987). https://www.engineersedge.com/civil_engineering/shrinkage_values_of_domestic_hardwoods_9997.htm
- **[F8]** AHMI / hardwoodinfo — Species guide: strength & mechanical properties. https://www.hardwoodinfo.com/wp-content/uploads/professional/species-guide/Species_Guide_Stength_Mech_Properties.pdf
- **[F9]** NC State Extension — Using the differential shrinkage and cupping of lumber spreadsheet. https://content.ces.ncsu.edu/using-the-differential-shrinkage-and-the-cupping-of-lumber-spreadsheet
- **[F10]** Wengert, G. (2014). "Correct thickness for 4/4 and 8/4 lumber", Woodworking Network (cita NHLA Rules). https://woodworkingnetwork.com/knowledge-center/wood-dr/correct-thickness-44-and-84-lumber
- **[F11]** MatWeb — Terms of Use. https://apm.matweb.com/reference/terms.aspx
- **[F12]** Ley Federal del Derecho de Autor (México), art. 107. https://mley.mx/LFDA/articulo/107/
- **[F13]** Berkman Center — The EU Database Directive: brief overview. https://cyber.harvard.edu/ilaw/Contract/europesumm.html
- **[F14]** *Feist Publications v. Rural Telephone Service*, 499 U.S. 340 (1991). https://www.law.cornell.edu/supremecourt/text/499/340
- **[F15]** ANSI A208.1-2009 Particleboard (CPA; copia en HUD). https://hud.gov/sites/dfiles/Housing/documents/ANSI_A208.1-09.pdf
- **[F16]** ANSI/CPA A208.2-2022 MDF (incorporada por referencia; no revisada). https://law.resource.org/pub/us/cfr/ibr/inc/ansi/ansi.cpa.a208.2.2022.pdf
- **[F17]** Dettmer et al., *BioResources* 10(3) 6032 — Property comparison of North American MDF. https://bioresources.cnr.ncsu.edu/BioRes_10/BioRes_10_3_6032_Dettmer_S_Prop_Compar_North_American_Manuf_MDF_Pt2_7080.pdf
- **[F18]** Arauco México — Ficha técnica Arauco Melamina. https://tablered.mx.arauco.com/fichas/arauco-melamina.pdf
- **[F19]** mdf-info.eu (EPF) — Screwing MDF. https://mdf-info.eu/using-mdf/fixing/screwing
- **[F20]** Weyerhaeuser — MDF Tip Sheet. https://res.cloudinary.com/amerhart/image/upload/Documents/Product/Weyerhaeuser%20MDF%20Tip%20Sheet
- **[F21]** Composite Panel Association — Technical Bulletin: Minimizing warp in laminated panels. https://www.compositepanel.org/wp-content/uploads/24-CPA-0059_Print_Technical-Bulletin_Minimizing-Warp-in-LPB_F.pdf
- **[F22]** APA / Tolko — Understanding the APA Trademark (2015). https://tolko.com/wp-content/uploads/2018/10/Understanding-the-APA-Trademark-March-2015.pdf
- **[F23]** Sodimac Chile — Melaminas (formatos de referencia LatAm). https://www.sodimac.cl/sodimac-cl/category/scat934763/Melaminas
- **[F24]** Wikipedia — 32 mm cabinetmaking system (CC BY-SA; solo referencia). https://en.wikipedia.org/wiki/32_mm_cabinetmaking_system
- **[F25]** Blum B71M2690B spec sheet (vía Würth Baer). https://xcart.wurthlac.com/skin/common_files/files/specs/B71M2690B_SpecSheet.pdf
- **[F26]** Blum TANDEM plus BLUMOTION 562H — Installation instructions. https://quikdrawers.com/Product_Instructions/Blum%20562H%20installation%20Instructions.pdf
- **[F27]** Accuride 3932 technical data. https://woodworkinstitute.com/wp-content/uploads/2024/07/Accuride-3932-tech-data.pdf
- **[F28]** Häfele UK catálogo — Minifix drilling (23C3-38). https://files.hafele.co.uk/CatalogFiles/www/23C3-38.pdf
- **[F29]** Brandt KTD72 contour edgebander — especificaciones. https://www.rtmachine.com/product-detail/?id=16023
- **[F30]** REHAU — Edgeband technical data sheet. https://www.rehau.com/downloads/886866/edgeband-technical-data-sheet.pdf
- **[F31]** WoodBin — Shelf design guidelines. https://woodbin.com/?p=1129
- **[F32]** Wilsonart — Basic laminate technical data (web). https://images.wilsonart.com/media/Technical_Resources/en/basiclaminatetechdata.pdf
- **[F33]** Wilsonart Laminate Technical Data, rev. 2020-09-15 (vía Richelieu). https://static.richelieu.com/documents/docsGr/101/641/6/1016416/1543182.pdf
- **[F34]** Nevamar HPL / NEMA LD3 (vía Richelieu). https://static.richelieu.com/documents/docsGr/105/241/4/1052414/1471351.pdf
- **[F35]** CTaSC — Maximum cantilever overhang of natural stone. https://ctasc.com/expert-answers/maximum-cantilever-overhang-natural-stone-side-cabinet/
- **[F36]** MSI — Q Quartz installation guide. https://www.msisurfaces.com/quartz-countertops/quartz-installation-guide/
- **[F37]** Cambria SOP 9.2.2.N — Sink drawing and fabrication requirements. https://www.cambriausa.com/content/dam/cusa/sales-marketing-collateral/sops/sop-9.2.2n-sink-drawing-and-fabrication-requirements.pdf
- **[F38]** Cosentino — Silestone technical data sheets. https://worktoplibrary.co.uk/wp-content/uploads/2022/05/Silestone-technical-data-sheets-all-colours.pdf
- **[F39]** Caesarstone Canada — Quartz countertops for outdoor kitchens. https://www.caesarstone.ca/blog/quartz-countertops-for-outdoor-kitchens/
- **[F40]** Caesarstone US — Indoor Surfaces Residential Lifetime Warranty (jun. 2025). https://studio.caesarstoneus.com/wp-content/uploads/2025/06/CSIndoorSurfaces-Residential-Lifetime-WarrantyJune2025.pdf
- **[F41]** Australian Government DEWR — Ban on engineered stone. https://www.dewr.gov.au/newsroom/articles/want-know-more-about-ban-use-supply-and-manufacture-engineered-stone-under-work-health-and-safety
- **[F42]** ASTM C615/C615M-23 — Granite Dimension Stone (resumen). https://store.astm.org/c0615_c0615m-23.html
- **[F43]** DuPont Corian — Product information (K-24016-NA). https://images.thdstatic.com/catalog/pdfImages/37/3766a531-291b-472e-9a4b-99256cbd2bd2.pdf
- **[F44]** DuPont Corian — Technical data sheet EMEA (K-26829). https://leeuwerik.nl/amfile/file/download/file/119/product/19084
- **[F45]** LX Hausys — HIMACS thermoforming FAQ. https://himacs-fabrication.lxhausys.com/docs/faqs-himacs-thermoforming
- **[F46]** Krion — In-depth understanding of thermoforming. https://www.krion.com/us/news/indepth-understanding-of-krion-thermoforming?amp=1
- **[F47]** LX Hausys — HIMACS 8. Cutting & cutouts. https://himacs-fabrication.lxhausys.com/docs/8-cutting-cutouts
- **[F48]** LX Hausys — HIMACS drop edges & reinforcement FAQ. https://himacs-fabrication.lxhausys.com/docs/faqs-himacs-drop-edges-downturns-design-fabrication-reinforcement
- **[F49]** The Fabricator — Design tips for bend relief, small holes, hole distortion and minimum flange widths. https://www.thefabricator.com/article/bending/design-tips-for-sheet-metal-bend-relief-small-holes-hole-distortion-near-bends-and-minimum-flange-widths
- **[F50]** SendCutSend — Bend deformation guidelines. https://sendcutsend.com/guidelines/bend-deformation/
- **[F51]** Protolabs — Sheet metal tolerances one-pager. https://protolabs.com/media/roognect/sheet-metal-tolerances-one-pager-2.pdf
- **[F52]** Wevolver — Sheet metal thickness standards & gauge charts. https://www.wevolver.com/article/sheet-metal-thickness-standards-gauge-charts
- **[F53]** Ternium México — Guía de tubería y perfiles. https://mx.ternium.com/media/ivvpcgti/guia-tuberia-y-perfiles.pdf
- **[F54]** BeamDimensions — ASTM A36 properties. https://beamdimensions.com/materials/Steel/American_Standard_(ASTM)/ASTM_A36
- **[F55]** Rolled Alloys — 304/304L data sheet. https://www.rolledalloys.ca/wp-content/uploads/304-304L_stainless-steel-data-sheet-rolled-alloys.pdf
- **[F56]** Rolled Alloys — 316/316L data sheet. https://www.rolledalloys.com/wp-content/uploads/316-316L_Stainless_DataSheet_RolledAlloys.pdf
- **[F57]** OSHA Fact Sheet — Controlling hazardous fume and gases during welding (vía eLCOSH). https://elcosh.org/document/4201/d001488/OSHA%2BFact%2BSheet%253A%2BControlling%2BHazardous%2BFume%2Band%2BGases%2BDuring%2BWelding.html
- **[F58]** WorkSafe NZ — Welding fume fever alert. https://worksafe.govt.nz/dmsdocument/1739-welding-fume-fever
- **[F59]** Quickparts — Aluminium 6063-T5 (derivado de MatWeb/ASM; **no ingerir**). https://quickparts.com/wp-content/uploads/2024/05/Aluminium-6063-T5.pdf
- **[F60]** Protolabs — CNC material data sheet Aluminum 6061-T651/T6. https://www.protolabs.com/media/h54d0gnp/2026-026-cnc-material-data-sheet_aluminum-6061-t651_t6.pdf
- **[F61]** Aluminum Extruders Council — Extrusion design considerations. https://members.aec.org/page/extrusion-design-considerations
- **[F62]** Aluminum Extruders Council — Extrusion design tips. https://aec.org/extrusion-design-tips
- **[F63]** Hydro — Extrusion quick guide for designing with aluminium. https://www.shapesbyhydro.com/en/industries/extrusion-quick-guide-for-designing-with-aluminium/
- **[F64]** Products Finishing — What is the correct anodizing specification? (2010). https://pfonline.com/articles/what-is-the-correct-anodizing-specification
- **[F65]** Formlabs — Design specs (Form 2). https://formlabs.com/support/Design-Specs/
- **[F66]** Xometry — Quick design reference for 3D printing. https://xometry.pro/en/3d-printing-design-guide/quick-guide/
- **[F67]** Universität Zürich AMF — Design guideline FDM. https://www.amf.uzh.ch/dam/jcr:fd1ee024-1a98-488b-89ec-ffd85befa786/Design-Guideline-FDM.pdf
- **[F68]** Fibre Glast — Physical properties of laminates. https://www.fibreglast.com/blogs/learning-center/physical-properties-of-laminates
- **[F69]** BFG International — Contact moulding / hand lay-up. https://bfginternational.com/what-we-do/contact-moulding
- **[F70]** Formlabs — Composite part manufacturing white paper. https://formlabs.com/support/Composite-part-manufacturing/
- **[F71]** Viracon — Holes (fabrication guidelines). https://www.viracon.com/holes/
- **[F72]** National Glass Association FM05-12 (2023) — Physical & mechanical properties of soda lime float glass. https://www.glass.org/sites/default/files/2023-07/FM05-12_2023_Physical_Mechanical_Properties_Typical_Soda_Lime_Float_Glass.pdf
- **[F73]** National Glass Association FB45-14 (2019) — Recommended applications for heat-treated glass. https://glass.org/sites/default/files/2021-08/FB45-14_2019_Recommended_Applications_for_Heat-Treated_Glass_0521_0.pdf
- **[F74]** Şişecam — Laminated glass. https://www.sisecam.com/en/flat-glass-product-catalog/sisecam-laminated-glass
- **[F75]** NOM-146-SCFI-2016 — Productos de vidrio de seguridad para la construcción. https://www.normasoficiales.mx/nom/nom-146-scfi-2016
- **[F76]** Polyurethane Foam Association — InTouch v4.2. https://www.pfa.org/wp-content/uploads/2022/05/InTouchv4.2newpart2.pdf
- **[F77]** American Chemistry Council — The furniture industry's guide to today's flexible foam. https://www.americanchemistry.com/content/download/4872/file/The-Furniture-Industrys-Guide-to-Todays-Flexible-Foam.pdf
- **[F78]** Kovi Fabrics — Upholstery fabric yardage calculator. https://kovifabrics.com/pages/yardage_calculator
- **[F79]** Fabric Depot — Upholstery charts. https://fabricdepot.com/pages/upholstery-charts
- **[F80]** Architex — ACT performance guidelines. https://www.architex-ljh.com/resources/act-performance-guidelines/
- **[F81]** SGS SafeGuardS 052/21 — US CPSC upholstered furniture flammability standard (16 CFR 1640). https://www.sgs.com/en/news/2021/04/safeguards-05221-us-cpsc-issues-upholstered-furniture-flammability-standard-direct-final-rule
- **[F82]** US EPA — TSCA Title VI small entity compliance guide for mills (2018). https://www.epa.gov/sites/default/files/2018-06/documents/small_entity_compliance_guide_for_mills_june_2018.pdf
- **[F83]** CSI — Changes to architectural woodwork standards (AWI), 2019. https://www.csiresources.org/blogs/julia-hall-csi-ccca-cdt1/2019/07/26/changes-to-architectural-woodwork-standards-AWI
- **[F84]** BIFMA — Standards short descriptions. https://www.bifma.org/page/StandardsShortDesc
- **[F85]** KCMA — ROI of durable cabinets (ANSI/KCMA A161.1). https://kcma.org/resources/roi-durable-cabinets-a161-1

<!-- Definiciones de enlaces de referencia -->
[F1]: https://www.govinfo.gov/content/pkg/USCODE-2020-title17/html/USCODE-2020-title17-chap1-sec105.htm
[F2]: https://www.fpl.fs.usda.gov/documnts/fplgtr/fplgtr282/front_matter_fpl_gtr282.pdf
[F3]: https://research.fs.usda.gov/treesearch/62200
[F4]: https://blog.spib.org/wp-content/uploads/2021/07/ch12.pdf
[F5]: https://research.fs.usda.gov/treesearch/62261
[F6]: https://www.extension.purdue.edu/extmedia/FNR/FNR-163.pdf
[F7]: https://www.engineersedge.com/civil_engineering/shrinkage_values_of_domestic_hardwoods_9997.htm
[F8]: https://www.hardwoodinfo.com/wp-content/uploads/professional/species-guide/Species_Guide_Stength_Mech_Properties.pdf
[F9]: https://content.ces.ncsu.edu/using-the-differential-shrinkage-and-the-cupping-of-lumber-spreadsheet
[F10]: https://woodworkingnetwork.com/knowledge-center/wood-dr/correct-thickness-44-and-84-lumber
[F11]: https://apm.matweb.com/reference/terms.aspx
[F12]: https://mley.mx/LFDA/articulo/107/
[F13]: https://cyber.harvard.edu/ilaw/Contract/europesumm.html
[F14]: https://www.law.cornell.edu/supremecourt/text/499/340
[F15]: https://hud.gov/sites/dfiles/Housing/documents/ANSI_A208.1-09.pdf
[F16]: https://law.resource.org/pub/us/cfr/ibr/inc/ansi/ansi.cpa.a208.2.2022.pdf
[F17]: https://bioresources.cnr.ncsu.edu/BioRes_10/BioRes_10_3_6032_Dettmer_S_Prop_Compar_North_American_Manuf_MDF_Pt2_7080.pdf
[F18]: https://tablered.mx.arauco.com/fichas/arauco-melamina.pdf
[F19]: https://mdf-info.eu/using-mdf/fixing/screwing
[F20]: https://res.cloudinary.com/amerhart/image/upload/Documents/Product/Weyerhaeuser%20MDF%20Tip%20Sheet
[F21]: https://www.compositepanel.org/wp-content/uploads/24-CPA-0059_Print_Technical-Bulletin_Minimizing-Warp-in-LPB_F.pdf
[F22]: https://tolko.com/wp-content/uploads/2018/10/Understanding-the-APA-Trademark-March-2015.pdf
[F23]: https://www.sodimac.cl/sodimac-cl/category/scat934763/Melaminas
[F24]: https://en.wikipedia.org/wiki/32_mm_cabinetmaking_system
[F25]: https://xcart.wurthlac.com/skin/common_files/files/specs/B71M2690B_SpecSheet.pdf
[F26]: https://quikdrawers.com/Product_Instructions/Blum%20562H%20installation%20Instructions.pdf
[F27]: https://woodworkinstitute.com/wp-content/uploads/2024/07/Accuride-3932-tech-data.pdf
[F28]: https://files.hafele.co.uk/CatalogFiles/www/23C3-38.pdf
[F29]: https://www.rtmachine.com/product-detail/?id=16023
[F30]: https://www.rehau.com/downloads/886866/edgeband-technical-data-sheet.pdf
[F31]: https://woodbin.com/?p=1129
[F32]: https://images.wilsonart.com/media/Technical_Resources/en/basiclaminatetechdata.pdf
[F33]: https://static.richelieu.com/documents/docsGr/101/641/6/1016416/1543182.pdf
[F34]: https://static.richelieu.com/documents/docsGr/105/241/4/1052414/1471351.pdf
[F35]: https://ctasc.com/expert-answers/maximum-cantilever-overhang-natural-stone-side-cabinet/
[F36]: https://www.msisurfaces.com/quartz-countertops/quartz-installation-guide/
[F37]: https://www.cambriausa.com/content/dam/cusa/sales-marketing-collateral/sops/sop-9.2.2n-sink-drawing-and-fabrication-requirements.pdf
[F38]: https://worktoplibrary.co.uk/wp-content/uploads/2022/05/Silestone-technical-data-sheets-all-colours.pdf
[F39]: https://www.caesarstone.ca/blog/quartz-countertops-for-outdoor-kitchens/
[F40]: https://studio.caesarstoneus.com/wp-content/uploads/2025/06/CSIndoorSurfaces-Residential-Lifetime-WarrantyJune2025.pdf
[F41]: https://www.dewr.gov.au/newsroom/articles/want-know-more-about-ban-use-supply-and-manufacture-engineered-stone-under-work-health-and-safety
[F42]: https://store.astm.org/c0615_c0615m-23.html
[F43]: https://images.thdstatic.com/catalog/pdfImages/37/3766a531-291b-472e-9a4b-99256cbd2bd2.pdf
[F44]: https://leeuwerik.nl/amfile/file/download/file/119/product/19084
[F45]: https://himacs-fabrication.lxhausys.com/docs/faqs-himacs-thermoforming
[F46]: https://www.krion.com/us/news/indepth-understanding-of-krion-thermoforming?amp=1
[F47]: https://himacs-fabrication.lxhausys.com/docs/8-cutting-cutouts
[F48]: https://himacs-fabrication.lxhausys.com/docs/faqs-himacs-drop-edges-downturns-design-fabrication-reinforcement
[F49]: https://www.thefabricator.com/article/bending/design-tips-for-sheet-metal-bend-relief-small-holes-hole-distortion-near-bends-and-minimum-flange-widths
[F50]: https://sendcutsend.com/guidelines/bend-deformation/
[F51]: https://protolabs.com/media/roognect/sheet-metal-tolerances-one-pager-2.pdf
[F52]: https://www.wevolver.com/article/sheet-metal-thickness-standards-gauge-charts
[F53]: https://mx.ternium.com/media/ivvpcgti/guia-tuberia-y-perfiles.pdf
[F54]: https://beamdimensions.com/materials/Steel/American_Standard_(ASTM)/ASTM_A36
[F55]: https://www.rolledalloys.ca/wp-content/uploads/304-304L_stainless-steel-data-sheet-rolled-alloys.pdf
[F56]: https://www.rolledalloys.com/wp-content/uploads/316-316L_Stainless_DataSheet_RolledAlloys.pdf
[F57]: https://elcosh.org/document/4201/d001488/OSHA%2BFact%2BSheet%253A%2BControlling%2BHazardous%2BFume%2Band%2BGases%2BDuring%2BWelding.html
[F58]: https://worksafe.govt.nz/dmsdocument/1739-welding-fume-fever
[F59]: https://quickparts.com/wp-content/uploads/2024/05/Aluminium-6063-T5.pdf
[F60]: https://www.protolabs.com/media/h54d0gnp/2026-026-cnc-material-data-sheet_aluminum-6061-t651_t6.pdf
[F61]: https://members.aec.org/page/extrusion-design-considerations
[F62]: https://aec.org/extrusion-design-tips
[F63]: https://www.shapesbyhydro.com/en/industries/extrusion-quick-guide-for-designing-with-aluminium/
[F64]: https://pfonline.com/articles/what-is-the-correct-anodizing-specification
[F65]: https://formlabs.com/support/Design-Specs/
[F66]: https://xometry.pro/en/3d-printing-design-guide/quick-guide/
[F67]: https://www.amf.uzh.ch/dam/jcr:fd1ee024-1a98-488b-89ec-ffd85befa786/Design-Guideline-FDM.pdf
[F68]: https://www.fibreglast.com/blogs/learning-center/physical-properties-of-laminates
[F69]: https://bfginternational.com/what-we-do/contact-moulding
[F70]: https://formlabs.com/support/Composite-part-manufacturing/
[F71]: https://www.viracon.com/holes/
[F72]: https://www.glass.org/sites/default/files/2023-07/FM05-12_2023_Physical_Mechanical_Properties_Typical_Soda_Lime_Float_Glass.pdf
[F73]: https://glass.org/sites/default/files/2021-08/FB45-14_2019_Recommended_Applications_for_Heat-Treated_Glass_0521_0.pdf
[F74]: https://www.sisecam.com/en/flat-glass-product-catalog/sisecam-laminated-glass
[F75]: https://www.normasoficiales.mx/nom/nom-146-scfi-2016
[F76]: https://www.pfa.org/wp-content/uploads/2022/05/InTouchv4.2newpart2.pdf
[F77]: https://www.americanchemistry.com/content/download/4872/file/The-Furniture-Industrys-Guide-to-Todays-Flexible-Foam.pdf
[F78]: https://kovifabrics.com/pages/yardage_calculator
[F79]: https://fabricdepot.com/pages/upholstery-charts
[F80]: https://www.architex-ljh.com/resources/act-performance-guidelines/
[F81]: https://www.sgs.com/en/news/2021/04/safeguards-05221-us-cpsc-issues-upholstered-furniture-flammability-standard-direct-final-rule
[F82]: https://www.epa.gov/sites/default/files/2018-06/documents/small_entity_compliance_guide_for_mills_june_2018.pdf
[F83]: https://www.csiresources.org/blogs/julia-hall-csi-ccca-cdt1/2019/07/26/changes-to-architectural-woodwork-standards-AWI
[F84]: https://www.bifma.org/page/StandardsShortDesc
[F85]: https://kcma.org/resources/roi-durable-cabinets-a161-1
