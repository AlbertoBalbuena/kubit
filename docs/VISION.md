# Kubit — Visión del producto

> **Diseña lo que sí se puede fabricar.**
> Modelado 3D directo y paramétrico, open source, con materiales y procesos de fabricación reales.

Estado: borrador v0.1 · Autor: Alberto Balbuena · Octubre 2026

---

## 1. El problema

Los CAD actuales separan **diseñar** de **fabricar**:

- **Shapr3D** es rápido e intuitivo, pero el material es casi solo apariencia.
- **Fusion 360** es preciso y potente: parámetros, restricciones, simulación, CAM. Pero es pesado, cerrado, por suscripción, y su modelo de materiales tampoco "sabe" de tableros, chapas, placas de piedra o calibres de lámina.

El resultado es conocido en cualquier taller. El diseño llega a producción y aparecen los problemas:

- espesores que no existen;
- cubiertas más largas que la placa;
- perforaciones que revientan el canto;
- dobleces imposibles;
- vidrio templado que "hay que perforar después".

El costo de esos errores se paga en el taller, no en el CAD.

## 2. La propuesta

Kubit es un modelador 3D que combina tres cosas:

1. **La velocidad y facilidad de Shapr3D.** Manipulación directa, menos diálogos, flujo continuo.
2. **La precisión y profundidad paramétrica de Fusion 360.** Parámetros con expresiones, restricciones, historial, ensambles, planos.
3. **Conocimiento real de fabricación.** Cada cuerpo sabe de qué material es, de qué stock sale y con qué procesos se fabrica. Un linter de fabricación (DFM) lo valida mientras diseñas.

Además, la IA es ciudadana de primera clase: **todo lo que se puede hacer en la UI se puede hacer por MCP**.

## 3. Para quién

| Usuario | Qué necesita de Kubit |
|---|---|
| Diseñador industrial / de mobiliario | Modelar rápido y saber que su diseño es fabricable |
| Talleres de carpintería, millwork, herrería, marmolería | Cut lists, despieces, desarrollos, planos de taller |
| Makers / impresión 3D | Validación de impresión, export directo |
| Estudiantes (metodología ARD) | Modelador gratuito integrado a su proceso de diseño |
| Agentes de IA | Una API completa (MCP) para modelar, validar y documentar |

## 4. Lo mejor de cada mundo

| Tomamos de Shapr3D | Tomamos de Fusion 360 | Lo que Kubit agrega |
|---|---|---|
| Herramienta contextual según la selección | Tabla de parámetros con expresiones y unidades | **Intención de fabricación** por cuerpo |
| Push/pull directo sobre caras | Timeline (historial) editable | **Cuerpos de fabricación** (tablero, perfil, lámina, placa, vidrio…) |
| Entrada numérica al vuelo mientras arrastras | Restricciones y cotas en sketch | **Linter DFM** en vivo con correcciones sugeridas |
| Sketch directo sobre caras, inferencias | Ensambles con juntas | Catálogo de materiales abierto y actualizable |
| Pocos diálogos modales, flujo continuo | Planos asociativos | Cut list, nesting y costos nativos |
| Interfaz limpia, curva de aprendizaje corta | Simulación | MCP nativo: la IA puede hacer todo |

**Modelo híbrido: "directo primero, con historial detrás".** El usuario trabaja como en Shapr3D (empuja, mueve, corta) y cada acción queda registrada como una operación paramétrica editable en el timeline, como en Fusion. El timeline es opcional y plegable: quien no lo necesita no lo ve.

## 5. Diferenciadores clave

1. **Intención de fabricación.** Al crear un cuerpo eliges "melamina blanca 16 mm, CNC + canteado". A partir de ahí:
   - el espesor sale del stock real;
   - las esquinas interiores respetan el radio de la fresa;
   - la veta tiene dirección;
   - el canto tiene cubrecanto.
2. **Cuerpos de fabricación tipados**: Tablero, Madera sólida, Perfil, Lámina metálica, Placa (piedra/cuarzo/solid surface), Vidrio, Pieza impresa, Pieza blanda (tapicería) y Sólido genérico. Cada tipo conoce su representación de manufactura: despiece, desarrollo, corte o malla.
3. **Linter DFM.** Errores, advertencias y sugerencias en vivo, con fuente citada y corrección rápida ("ajustar a 18 mm", "agregar unión a la cubierta").
4. **Las reglas son datos, no código.** Carpinteros, herreros y marmoleros pueden contribuir reglas y materiales sin saber Rust.
5. **Salida al taller.** Planos, cut list, BOM, nesting, desarrollos DXF y costos.
6. **IA nativa.** Servidor MCP generado automáticamente del mismo registro de comandos que usa la UI.

## 6. Principios de producto

1. **Rapidez antes que botones.** Si una acción necesita un diálogo, primero buscamos cómo hacerla directa.
2. **Precisión siempre disponible.** Todo arrastre acepta un número exacto. Todo número acepta una expresión.
3. **Fabricación primero.** Un diseño que no se puede fabricar es un diseño incompleto.
4. **Honestidad.** Kubit asiste, no certifica. Las validaciones citan su fuente y se presentan como ayuda, no como garantía estructural.
5. **Offline-first.** Funciona sin internet. La nube (catálogo, biblioteca) suma, pero no es requisito.
6. **Abierto.** Código Apache-2.0, datos CC-BY-4.0, formatos documentados.
7. **Determinista.** El mismo archivo produce siempre la misma geometría.
8. **IA como usuario.** Si un humano lo puede hacer, un agente también, con el mismo comando.

## 7. Ecosistema (visión a largo plazo)

- **Kubit**: la aplicación de escritorio (macOS, Windows, Linux) y, después, la web (WASM + WebGPU).
- **Kubit Catálogo**: base de materiales, stock, procesos y reglas. Vive como repositorio abierto (TOML), se publica en Cloudflare (Workers + D1 + R2) y la app la sincroniza con versiones. Los archivos guardan la versión del material usada, así que un diseño se abre igual aunque el catálogo cambie.
- **Kubit Hub** (fase posterior): biblioteca para compartir modelos, tipo 3D Warehouse, pero mejor:
  - componentes **paramétricos** con parámetros expuestos;
  - licencia por modelo;
  - insignia de "validado para fabricación".
- **Integración ARD**: Kubit es el modelador del toolkit ARD y cubre principalmente las fases *Spark*, *Proof* y *Make*. Más adelante se suman datos antropométricos y el cuadro biomecánico.

## 8. Lo que Kubit NO es (por ahora)

- No es un render fotorrealista. Exporta glTF/USD a D5, Blender, etc.
- No es BIM ni CAD arquitectónico.
- No es un CAM completo. Genera DXF y salidas para CNC; los postprocesadores de G-code vienen después.
- No sustituye el cálculo estructural certificado.
- No es iPad-first, aunque la interacción se diseña pensando en tableta y pluma para el futuro.

## 9. Nombre

**Kubit** a secas: es corto, sirve como comando (`kubit`) y como prefijo de crates (`kubit-*`), y no se confunde con "Evita Studio". Los productos del ecosistema se nombran *Kubit Catálogo* y *Kubit Hub*.

⚠️ **Pendiente: revisar la marca.** *kubit GmbH* (Dresde) fue una empresa de software CAD/levantamiento comprada por FARO en 2015, y Sandia tiene *CUBIT* (mallado). Ambas están en la misma industria. Antes de invertir en marca o dominio, hay que buscar en IMPI, USPTO y EUIPO. Mientras tanto, "Kubit" es el nombre de trabajo; renombrar crates al inicio es barato.
