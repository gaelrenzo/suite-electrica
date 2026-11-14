# Diferencias entre las salidas de Rust y las de Python

Etapa 4 de `docs/PLAN_RUST.md`. Las salidas de Python (`motor/tests/salidas_python/`, generadas
por `motor/generar_referencia_salidas.py`) son la referencia. Cómo se verifica:

```bash
cd motor
cargo test --test salidas            # Excel celda a celda, DXF entidad a entidad
python comparar_salidas.py           # ezdxf audita los DXF de Rust; texto de los PDF con pypdf
```

## Casos comparados

| Caso | Entrada | Salidas |
|---|---|---|
| `linea_auto` | `examples/linecraft/proyecto.json` + catálogo de prueba (armados y postes asignados, verificación, metrado, alertas) | planilla (9 hojas), perfil DXF, memoria PDF |
| `linea_manual` | `examples/linecraft/proyecto_manual.json` (como la CLI) | planilla (7 hojas), perfil DXF, memoria PDF |
| `topo` | colina sintética irregular (semilla 7), TIN, curvas cada 2,5 m | plano DXF, perfil DXF, perfil Excel, perfil PDF a escala |
| `red` | `examples/gridcraft/red.json` | planilla (4 hojas), plano DXF |
| `presu` | `examples/presucraft/presupuesto.json` | planilla (5 hojas) |

En los casos de línea, Rust usa el perfil del volcado de Python (`tests/linea_*.txt`) para
dibujar exactamente el mismo terreno; la malla regular de ejemplo admite otra triangulación
(ver el plan, «Triangulación»).

## Resultado

* **Excel:** 2 698 celdas iguales; mismas hojas en el mismo orden, mismos encabezados (con
  unidades), mismos redondeos y textos («SÍ»/«NO», «—», «Sin avisos», progresivas `km+mmm.dd`).
  Los montos de PresuCraft se aceptan a ±0,01 (orden de suma, ver el plan).
* **DXF:** mismas capas y colores ACI; mismas entidades, en el mismo orden, con la misma
  geometría (1e-6 m) y los mismos textos, alturas y giros. ezdxf abre los DXF de Rust y su
  auditoría no encuentra errores.
* **PDF:** mismo tamaño de página (A4 apaisado en la memoria; tamaño a escala en el perfil);
  todas las palabras y números del PDF de Python están en el de Rust (salvo la fecha).

## Diferencias intencionales

| Salida | Python | Rust | Motivo |
|---|---|---|---|
| DXF: versión | R2010 (AC1024), ezdxf | **R12 (AC1009)** ASCII | Escritor propio, sin handles ni objetos; lo abren AutoCAD, BricsCAD, LibreCAD, QGIS y ezdxf |
| DXF: polilíneas | LWPOLYLINE (elevación en código 38) | **POLYLINE 2D** + VERTEX (elevación en el encabezado y en cada vértice) | LWPOLYLINE no existe en R12; misma geometría y elevación. Los lectores de la suite (Python y Rust) leen ambas |
| DXF: textos no ASCII | UTF-8 | `\U+XXXX` | Convención de R12 (página ANSI_1252); AutoCAD y ezdxf los decodifican |
| DXF: estilos y capas extra | ezdxf crea estilos, tipos de línea y la capa `Defpoints` | solo `0`, las capas del plano, `CONTINUOUS` y `STANDARD` | Nada de eso se usa en los planos |
| Plano TopoCraft: orden de los triángulos | orden de qhull | orden de spade | Mismos triángulos (se comparan como conjunto) |
| Plano TopoCraft: inicio de las curvas y rótulo de cota | según el recorrido de qhull | según el recorrido de spade | Misma geometría de cada curva; el rótulo de las curvas mayores va en el punto medio de la curva, que depende de dónde empieza: mismo texto, altura y capa, siempre sobre su curva |
| Memoria PDF: fuente | Helvetica (reportlab) | **DejaVu Sans** embebida | Helvetica no tiene σ, α, Σ, θ, φ, √: en Python las fórmulas de la sección 3 salen con huecos; en Rust se leen completas |
| Memoria PDF: figura del perfil | imagen PNG de matplotlib (150 ppp) | **dibujo vectorial** | Nítido a cualquier zoom; mismos elementos (terreno, conductor, despeje mínimo, postes con código, ejes, leyenda); las marcas de los ejes las elige cada programa |
| Memoria PDF: paginación | reportlab corta las tablas y repite el encabezado | igual (corta y repite), con otra altura de fila | Rust ocupa 1 página menos en los dos casos; el contenido es el mismo |
| Memoria PDF: tablas anchas | reportlab las deja salir del margen | se reduce la letra hasta que caben (mín. 3 pt) | La tabla de flechado tiene 24 columnas |
| Perfil PDF a escala: marcas de los ejes | `MaxNLocator` de matplotlib | pasos redondos 1-2-2,5-5 (~8 en cota, ~6 en progresiva) | Solo la cuadrícula; escalas, banda de progresivas y cotas, y títulos iguales |
| Perfil PDF a escala: marcas del eje X | marcas sin número bajo el perfil | sin marcas (la banda ya muestra las progresivas) | Estética |

Cualquier otra diferencia en una salida es un error y debe corregirse o agregarse a esta tabla
con su motivo en el mismo commit.
