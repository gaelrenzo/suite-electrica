# Plan de migración a Rust

Decisión (2026-11-13): reescribir **toda** la Suite Eléctrica en Rust para distribuirla como un
solo ejecutable, sin instalar Python. La versión Python se mantiene como **referencia** hasta
que la versión Rust la iguale; luego se retira.

## Principio de validación

Nada se da por portado sin comparación numérica:

1. Un script de Python (`motor/generar_referencia*.py`) calcula casos con la versión Python, que
   está validada contra REDLIN 2023 (ver `docs/linecraft.md`, sección 8), y los guarda en
   `motor/tests/*.txt`.
2. Las pruebas de Rust (`motor/tests/*.rs`) leen las mismas entradas y deben reproducir los
   resultados (tolerancia relativa 1e-9 en fórmulas cerradas, 1e-6 donde hay iteraciones).
3. `cargo test` en `motor/` debe pasar siempre antes de subir cambios.
4. Redondeos: Rust usa `numerico::redondear` (formato decimal correctamente redondeado), que
   reproduce `round()` de Python; los montos que caen justo en medio céntimo pueden diferir
   en ±0,01 porque pandas suma por pares.

## Herramientas

* Rust estable con toolchain **GNU** (`x86_64-pc-windows-gnu`, sin Visual Studio), instalado con
  rustup en `%USERPROFILE%\.cargo\bin`.
* **MinGW-w64 portátil** (WinLibs GCC 16.2 UCRT, ZIP verificado por SHA-256) en `C:\mingw64`: los
  crates de Windows (`windows-sys`, `getrandom`, usados por `calamine` y la interfaz) necesitan su
  `dlltool`/`as`. Para compilar: `$env:Path = "$env:USERPROFILE\.cargo\bin;C:\mingw64\bin;" + $env:Path`.
* DXF: lector ASCII propio (POINT, POLYLINE/VERTEX, LWPOLYLINE con elevación, código 38); el crate
  `dxf` se descartó porque no lee la elevación de LWPOLYLINE. DXF binario no se admite.
* Crates aprobados: `eframe`/`egui` + `egui_plot` (interfaz), `serde`/`serde_json` (proyectos),
  `csv`, `calamine` (leer Excel), `rust_xlsxwriter` (escribir Excel), `dxf`, `quick-xml`
  (KML/GPX), `spade` (TIN y curvas), `printpdf` (memoria PDF), `rfd` (diálogos). La conversión
  UTM se programa sin PROJ.
* DXF de salida: escritor propio R12 (`dxf.rs`). PDF: `printpdf` sin funciones por defecto
  (sin el motor HTML) y la fuente DejaVu Sans embebida (`motor/assets/fonts`, licencia libre en
  `LICENSE_DEJAVU`).

## Etapas

| # | Etapa | Módulos Rust | Estado |
|---|---|---|---|
| 1a | Mecánica del conductor | `conductor`, `mecanica`, `numerico` | ✅ 674 casos = Python |
| 1b | Perfil y eje | `geo` | ✅ |
| 1c | Cálculo de línea (ubicación automática, tramos, vanos, cargas, altitud, estados por vano) | `linea` | ✅ 4 proyectos = Python |
| 1d | Catálogos DGE, verificación con momentos de poste, metrado | `catalogos` | ✅ = Python |
| 1e | Alertas, tabla de flechado, derivaciones | `alertas`, `flechado`, `derivaciones` | ✅ = Python |
| 2 | GridCraft (caída de tensión) y PresuCraft (presupuesto) | `red`, `presupuesto` | ✅ = Python (montos ±0,01 por orden de suma) |
| 3 | Archivos: CSV, JSON (.sel), Excel, DXF, KML/GPX, MapSource, TPG, UTM, TIN, curvas de nivel, perfil | `entrada`, `proyectos`, `utm`, `tin` | ✅ 14 archivos de prueba (válidos y problemáticos) = Python; `.sel` ida y vuelta Python↔Rust; `proyecto.json` → mismo cálculo |
| 4 | Salidas: planillas Excel, planos DXF (perfil, red), memoria PDF | `salidas`, `dxf`, `pdf` | ✅ 5 casos: Excel = Python (2 698 celdas), DXF = Python (entidad a entidad, auditoría ezdxf sin errores), PDF con todo el texto de Python; diferencias en `docs/DIFERENCIAS_RUST.md` |
| 5 | Interfaz egui: TopoCraft, LineCraft (perfil interactivo, planilla editable), GridCraft, PresuCraft | binario `suite-electrica` | pendiente |
| 6 | Validación final contra REDLIN con los scripts de `examples/validacion` portados, `.exe` y retiro de Python | — | pendiente |

## Criterios de aceptación por etapa

**Etapa 3 (archivos).** Pruebas por formato (CSV, DXF, KML, GPX, MapSource, Excel RLT, TPG) con
archivos de ejemplo válidos **y** casos problemáticos: duplicados, cotas atípicas, campos vacíos,
unidades incorrectas (p. ej. grados en vez de metros, cotas en mm) y coordenadas fuera de zona.
Para cada caso problemático se fija el comportamiento esperado (error claro o aviso) y Rust debe
dar el mismo resultado que Python. Los proyectos `.sel` y `.json` deben conservar todos los datos
al guardarse y volver a abrirse (prueba de ida y vuelta, también entre Python y Rust).

**Etapa 4 (salidas).** Planillas Excel, planos DXF y memoria PDF comparados con las salidas de
Python en contenido, unidades, capas, nombres de hojas y columnas, redondeos y avisos. Toda
diferencia visual intencional (tipografía, maquetación del PDF, colores) se documenta en
`docs/DIFERENCIAS_RUST.md`.

**Etapa 5 (interfaz y .exe).** La interfaz se construye sobre los flujos completos de cada módulo
(importar → calcular → revisar → exportar). El `.exe` se prueba en una computadora limpia, sin
Python ni herramientas de desarrollo instaladas.

**Etapa 6 (retiro de Python).** Antes de retirar Python se fija una **versión de referencia**:
casos de entrada y resultados esperados congelados en `motor/tests/`, con las diferencias aceptadas
registradas (ver abajo). Las versiones futuras de Rust deben mantener esos resultados; un cambio
intencional en un resultado exige actualizar la referencia en el mismo commit y explicarlo.

## Diferencias aceptadas y condiciones de las comparaciones

* **Montos (PresuCraft):** ±0,01 cuando un total cae justo en medio céntimo; pandas suma por pares
  y Rust en orden, y esa diferencia ínfima decide el redondeo.
* **Triangulación (TIN):** la coincidencia exacta con qhull (triángulos, aristas, cotas, curvas y
  perfil) está demostrada con la colina sintética de `generar_referencia_tin.py`: malla de 20 m
  con desplazamiento aleatorio uniforme ±6 m (semilla 7), sin puntos cocirculares. En **mallas
  regulares** (cuatro puntos cocirculares) la triangulación de Delaunay no es única: qhull y spade
  pueden elegir diagonales distintas, ambas válidas; las cotas interpoladas pueden diferir en esas
  celdas (no en superficies planas).
* **UTM:** error < 10 µm frente a pyproj (PROJ) demostrado en 243 puntos generados por
  `generar_referencia_utm.py`: para cada zona 17S, 18S y 19S, una grilla de 9 longitudes entre
  meridiano central ±3,4° y 9 latitudes entre −18,5° y −0,2° (WGS84). Condiciones: pyproj con
  `Transformer.from_crs(4326, 327xx, always_xy=True)`; Rust con series de Krüger hasta n⁶; además
  la ida y vuelta (UTM → geográficas) reproduce lon/lat con error < 1e-9°.

## Etapa 3: comportamiento ante archivos problemáticos (igual en Python y Rust)

| Caso | Comportamiento |
|---|---|
| Duplicado (mismo E, N, z) | se quita y se lista en *duplicados* |
| Conflicto (mismo E, N, otra z) | se quita (queda el primero) y se lista en *conflictos* |
| Campo vacío o no numérico en E, N o z | se quita y se lista en *inválidos* |
| Cota atípica (pico o pozo aislado) | se lista en *atípicos*; se quita solo si se pide |
| E fuera de 100 000–900 000 m o N fuera de 0–10 000 000 m | se quita y se lista en *fuera de zona UTM* |
| Cota fuera de −500…7000 m (mm, cm, pies) | aviso en *cotas fuera de rango*, no se quita |
| Coordenadas en grados (mayoría con E y N dentro de ±180 / ±90) | error: importar como KML/GPX o convertir |
| DXF sin puntos; MapSource sin `Grid UTM` | error con el nombre del archivo |
| `.sel` de otro formato o de versión más nueva | error claro |

Archivos de prueba: `motor/tests/archivos/` (sintéticos, generados por
`generar_referencia_archivos.py`); esperado: `motor/tests/archivos_esperado.txt`.

**`.sel`:** Rust lo trata como JSON genérico (conserva claves y orden). Verificación cruzada:
`python motor/generar_referencia_sel.py` → `cargo test --test proyectos` →
`python motor/generar_referencia_sel.py --verificar` (Python abre lo guardado por Rust sin cambios).

**`proyecto.json` con malla regular** (`examples/colina_gaussiana.csv`, 10 m): por las diagonales
distintas, el perfil de Rust tiene 258 puntos y el de Python 232; la cota del terreno sobre el eje
difiere como máximo **8,5 cm** (muestreo cada 1 m), pero el resultado de ingeniería coincide: mismas
estructuras en la misma progresiva (Δ = 0,000 m), mismos tipos y despejes con |Δ| ≤ **1,3 mm**.

## Cómo continuar

```bash
cd motor
cargo test                       # pruebas Rust vs Python
python generar_referencia.py     # regenerar referencias si cambia la versión Python
```

Al portar un módulo: (1) leer el `.py` vigente, (2) portarlo sin cambiar fórmulas, (3) generar
casos de referencia en Python, (4) prueba Rust que los reproduzca, (5) actualizar esta tabla.
