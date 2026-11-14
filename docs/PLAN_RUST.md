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
| 5 | Interfaz egui: TopoCraft, LineCraft (perfil interactivo, planilla editable), GridCraft, PresuCraft | `documento`, binario `suite-electrica` | ✅ documento `.sel` = Python (flujo completo de la app, 230 líneas); `.exe` de 22 MB probado sin Python; falta probarlo en otra PC |
| 6 | Validación final contra REDLIN con los scripts de `examples/validacion` portados, `.exe` y retiro de Python | — | ⏳ referencia congelada (`python-referencia-v1`); `comparar-redlin` portado (= Python en los 5 modos); `.exe` publicado (release `v0.1.0-rust`); falta probarlo en otra PC y retirar Python |

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

## Etapa 5: aplicación de escritorio

`motor/src/documento.rs` es el port de `documento.py` (modelo de la app): el mismo `.sel`, la
topografía en caché, el cálculo completo (ubicación automática, catálogos, verificación,
metrado, alertas y avisos) y la edición de estructuras con deshacer. `tests/documento.rs`
reproduce el flujo de `generar_referencia_documento.py`: calcular, agregar, mover, cambiar tipo
y altura, quitar el poste, eliminar, deshacer y asignar catálogos. Coincide línea a línea.

La interfaz (`motor/src/app/`, egui + egui_plot + rfd) tiene las cuatro pestañas de la versión
PySide6:

* **TopoCraft:** importar puntos y eje, parámetros, procesar, reporte de limpieza, planta
  (puntos, curvas, eje) y perfil; exporta `plano.dxf`, `perfil.pdf`, `perfil.xlsx`, `perfil.dxf`.
* **LineCraft:** parámetros, conductor, hipótesis y fuentes editables; Calcular (F6), ubicar
  automático, estructuras mínimas, asignar catálogos, deshacer (Ctrl+Z). En el perfil: clic
  selecciona, arrastrar mueve, doble clic agrega, clic derecho abre el menú y Supr elimina.
  Planilla editable (tipo, armado, poste, altura, retenidas) y pestañas de resultados. Exporta
  `perfil_linea.dxf`, `planilla_estructuras.xlsx` y `memoria_calculo.pdf`.
* **GridCraft / PresuCraft:** abrir o crear desde plantillas (sin sobrescribir), calcular,
  revisar hojas y exportar. «Metrado desde LineCraft» escribe `metrado.csv` del presupuesto.

Compilar el ejecutable (Windows, toolchain GNU):

```powershell
cd motor
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\mingw64\bin;" + $env:Path
cargo build --release --bin suite-electrica   # -> target\release\suite-electrica.exe
```

**Prueba sin herramientas de desarrollo (2026-11-14).** Windows Sandbox no existe en Windows 11
Home (solo Pro/Enterprise), así que la PC limpia se aproximó así:

1. **Dependencias del `.exe`** (`objdump -p`): solo DLL del sistema (kernel32, user32, gdi32,
   dxgi, dwrite, opengl32, ole32, UCRT `api-ms-win-crt-*` incluida en Windows 10/11…). No hay
   libgcc, libstdc++, vcruntime ni Python.
2. **Ejecución aislada:** el `.exe` solo, copiado a una carpeta vacía y lanzado con
   `PATH=C:\Windows\System32;C:\Windows`. Abre un `.sel`, calcula con F6 y muestra planta,
   perfil y resultados. Los módulos cargados fuera de `C:\Windows` son el propio `.exe` y dos
   *hooks* inyectados por programas de captura (OBS, TikTok LIVE Studio), que no son
   dependencias.
3. **Uso con ratón** (clics y arrastre automatizados): cambiar de pestaña, calcular y arrastrar
   E-02 de 0+120.00 a 0+155.87 con recálculo.

**Pendiente:** abrir el `.exe` en otra computadora sin Python (por ejemplo, una del laboratorio).
El render usa wgpu (DirectX 12 / Vulkan); en equipos sin GPU, DirectX usa WARP.

## Etapa 6: versión de referencia congelada

La etiqueta git **`python-referencia-v1`** marca el último estado con la versión Python completa
(sin cambios desde `c41525b`) y todos los casos de referencia. Los casos y resultados esperados
están en `motor/tests/` y se generan con `motor/generar_referencia*.py`:

| Referencia | Contenido |
|---|---|
| `referencia.txt` | mecánica del conductor: 674 casos |
| `linea_*.txt`, `catalogos_auto.txt`, `alertas_auto.txt`, `flechado_*.txt`, `derivacion.txt` | cálculo de línea, catálogos, alertas, flechado y derivaciones |
| `red_*.txt`, `presu_*.txt` | GridCraft y PresuCraft |
| `utm.txt`, `tin.txt` | 243 puntos UTM; TIN, curvas y perfil |
| `archivos/` + `archivos_esperado.txt` | 14 archivos de entrada (válidos y problemáticos) |
| `archivos/proyecto_python.sel` | ida y vuelta del `.sel` |
| `archivos/documento_irregular.sel` + `documento.txt` | flujo completo de la app |
| `salidas_python/` | planillas, DXF y PDF de 5 casos |

Las diferencias aceptadas están en «Diferencias aceptadas» (arriba) y en
`docs/DIFERENCIAS_RUST.md`. **Regla:** `cargo test` debe pasar siempre. Si un cambio intencional
altera un resultado, en el mismo commit se actualiza el archivo de referencia y se explica el
motivo; los archivos de referencia ya no se regeneran con Python.

Antes de borrar `suite_electrica/`, hay que:

1. Mover `suite_electrica/plantillas/` (Rust las incluye con `include_str!`) a `data/plantillas/`.
2. ✅ Portar `examples/validacion/comparar_redlin.py`: binario `comparar-redlin`
   (`motor/src/bin/comparar-redlin.rs`), mismos 5 modos y argumentos. Con los reportes del
   ejemplo de REDLIN (`SER-ejemplo.len`, exportados en local; no se distribuyen) imprime **las
   mismas cifras** que Python: catenaria (f y L con error 0,0000 m, vp ≤ 0,0098 m), tiro vs
   temperatura (1280 puntos, mediana 0,001 %), estado final con creep, cargas (FT, FL, Fv) y
   momentos/FS de postes.
   `cargo run --release --bin comparar-redlin -- PlanillaEstTotal.xls` (ver los demás modos en el
   archivo).
3. Probar el `.exe` en otra PC: descargarlo del *release*
   [`v0.1.0-rust`](https://github.com/gaelrenzo/suite-electrica/releases/tag/v0.1.0-rust)
   (se incluye `SHA256SUMS.txt`).
4. Confirmar con el autor.

## Funciones nuevas (solo en Rust)

Desde `python-referencia-v1`, las funciones nuevas se escriben solo en Rust y no cambian los
resultados de referencia. Si un cambio los altera a propósito, se explica aquí.

* **Elección de poste por momento de vuelco** (`catalogos::elegir_poste_mrn`, la usan «Ubicar
  automático» y «Asignar catálogos»). Cuando el poste tiene geometría (diámetros, empotramiento
  y MOR o carga nominal), se elige el de menor momento resistente en la línea de tierra que
  cumple FS(MRN) ≥ FS mínimo en todas las hipótesis. Es el mismo cálculo de la verificación, que
  ya coincide con REDLIN (`comparar-redlin --postes`). También se aplica a estructuras con
  retenida, según la carga que ésta tome. El criterio anterior usaba la fuerza equivalente en la
  punta, sin el viento sobre el poste (MVE), y aceptaba sin verificar cualquier poste con
  retenida. Sin geometría (como en la plantilla actual) se usa ese criterio anterior, así que
  los resultados de referencia no cambian. `tests/postes_mrn.rs` comprueba que el poste elegido
  cumple y que ningún poste más chico cumple. Con el catálogo de prueba y FS 4, E-02, E-03 y
  E-04 pasan de CAC 12/300 (que no cumplía por momentos) a CAC 12/400 (FS de 4,48 a 5,00).
  `elegir_poste` sigue igual, por la paridad con Python.
* **Armados DGE monofásicos, bifásicos y trifásicos con neutro** (plantilla `armados.csv` y
  `materiales.csv`). Se agregan 43 armados de la Norma DGE RD 024-2003, transcritos de sus
  láminas: PS1, PA1, PA2, PA3, PR3, PTV y PTH, en las versiones 1Φ con retorno por tierra
  (`-0`), 1Φ con neutro corrido (`-1N`), 2Φ (`-2`, `-2N`, variantes `L`) y 3Φ con neutro (`-3N`,
  `-3NL`). Se completan también los materiales de PS1-3L, PA1-3L y PR3-3L, que comparten columna
  con el armado base en su lámina. Las columnas nuevas `fases` y `neutro` se combinan con el
  sistema del proyecto («Sistema» en LineCraft, guardado en el `.sel`):
  `Catalogos::elegir_armado_sistema` elige solo entre los armados de ese sistema. En los armados
  con neutro, `distancia_punta_amarre_m` llega hasta el neutro (el conductor más bajo, que manda
  en el despeje), y `alturas_fases_m` lista solo las fases. Python ignora las columnas nuevas y
  sigue eligiendo los trifásicos, que están primero en el archivo, así que la referencia no
  cambia. Un `.sel` guarda sus propios catálogos: el botón «Cargar catálogos de la plantilla»
  actualiza un proyecto existente, y si el catálogo no tiene armados del sistema se avisa.
  `tests/armados_dge.rs` comprueba la elección, que cada armado tenga geometría y materiales, y
  el cálculo completo con una línea 1Φ, una 2Φ con neutro y una 3Φ con neutro. **Pendiente:**
  biposte en H (láminas 027-030), porque el modelo verifica un solo poste por estructura.
* **GridCraft con fases por tramo (criterio de REDCAD).** El catálogo de conductores admite una
  columna nueva, `fases` (1, 2 o 3), y el JSON de la red un parámetro nuevo, `tension_fn_V`
  (220 V en 440/220 V; por defecto V/√3). Si alguna formación declara fases, todas deben
  hacerlo, y cada tramo se calcula con las suyas:
  - corriente: I = P/(√3·V_ff), P/V_ff o P/V_fn, con la tensión real del extremo emisor;
  - caída: ΔV = K·I·L, acumulada en V fase-neutro, con %ΔV sobre la tensión fase-neutro;
  - pérdidas: fases·I²·R·L, y en alumbrado I²·R·L con su propia tensión.

  Sin la columna, el cálculo es el anterior (paridad con Python).

  **Validación:** `comparar-redcad CaidaTension.xls` (binario `motor/src/bin/comparar-redcad.rs`)
  lee la «Planilla de Caída de Tensión» exportada de REDCAD 2023. El ejemplo «Urbano-Rural» tiene
  4 subestaciones (380/220 V trifásica y 440/220 V) y 106 tramos con formaciones 3x16, 2x16 y
  1x16, con y sin conductor de alumbrado. El programa reconstruye el árbol por las tensiones y
  compara nodo por nodo:
  - potencia acumulada (SP y AP): diferencia 0;
  - corriente: ≤ 0,05 A (REDCAD redondea a 0,1 A);
  - caída por tramo: ≤ 0,019 V;
  - tensión: ≤ 0,037 V;
  - %ΔV: ≤ 0,06 % (REDCAD redondea a 0,1 %);
  - pérdidas: ≤ 0,33 W.

  Dos criterios de REDCAD que el comparador refleja: la fila «Tablero» muestra totales, y una
  formación sin conductor de alumbrado tiene K_AP = 0 (la lámpara no carga la fase).
  `tests/red_fases.rs` fija el modo con una red sintética calculada a mano. Los datos de REDCAD
  no se distribuyen: se exportan en local (`C:\esolutions\_comparacion\redcad`).
* **Metrado de GridCraft (criterio de REDCAD).** El CSV de nodos admite columnas opcionales
  (descritas en la plantilla `red_nodos.csv`): armado, soporte, puesta a tierra, caja, pastoral,
  retenidas, conectores y acometidas. `red::metrado` cuenta:
  - armados, expandiendo multiplicadores y sumas («2E3/S», «E3+E3/S»);
  - soportes, PAT, retenidas, cajas, pastorales y puntos de iluminación;
  - conectores, por tipo y por servicio particular o alumbrado;
  - acometidas: cortas, largas y extra largas, 1φ o 3φ, y muretes;
  - conductor en km por formación (suma de vanos sin recargo).

  La planilla de GridCraft agrega la hoja «Metrado» y PresuCraft tiene «Metrado desde GridCraft»
  (rubro RS). Los dos botones de metrado reemplazan solo las filas de su rubro en `metrado.csv`.
  **Validación:** `comparar-redcad PlanillaRS.xls MetradoRS.xls --metrado` arma los nodos desde
  la «Planilla de Redes Secundarias» de REDCAD y compara con su «Metrado de Redes Secundarias»:
  **33 de 33 renglones iguales** (12 armados, soportes, PAT, retenidas BT/MT, cajas, pastorales,
  iluminación, 3 conectores, 5 acometidas y 5 conductores en km). Prueba: `tests/red_metrado.rs`.
* **PresuCraft: APU y fórmula polinómica (criterio de PRESIO)** (`motor/src/apu.rs`).
  - **APU:**
    - cuadrilla: cantidad = cuadrilla · 8 h / rendimiento;
    - parcial = cantidad · precio;
    - `%mo` = % de la mano de obra;
    - `%` = % de lo anterior de su sección, o de la base indicada;
    - secciones: mano de obra, materiales, equipos, maquinaria y gastos varios;
    - redondeo de Excel (la mitad hacia arriba) a 2 decimales.
  - **Fórmula polinómica (DS 011-79-VC):**
    - incidencias por Índice Unificado, redondeadas a 3 decimales;
    - el residuo va al índice de mayor parcial;
    - agrupación preliminar y monomios;
    - avisos si hay más de 8 monomios, uno por debajo de 5 % o la suma no es 1.
  - **Uso** (claves opcionales de `presupuesto.json`):
    - `"apu": "apu.csv"`: si una partida coincide con una UC de montaje, su precio sale del APU;
    - columna `iu` en suministros y `"fp_agrupacion": "fp_agrupacion.csv"` (columnas `iu`,
      `similar`, `monomio`): la fórmula reparte suministro, montaje desglosado por insumo,
      transporte (IU 32) y gastos generales con utilidad (IU 39);
    - la planilla agrega las hojas «APU» y «FormulaPolinomica».

  **Validación:** `comparar-presio` con los reportes del ejemplo de PRESIO 2023.
  - `APU.xlsx --apu`: **102 de 102 partidas**, con 671 cantidades, 823 parciales, 386 subtotales y
    102 totales iguales.
  - `Presupuesto.xlsx --fp`: **las 3 fórmulas** (líneas primarias, redes primarias y redes
    secundarias), con incidencias, grupos y coeficientes iguales.

  Pruebas: unitarias en `apu.rs` y `tests/presu_apu.rs`. Lo que no se valida: el transporte por
  ruta de PRESIO, que necesita los pesos y tarifas de su base de datos. PresuCraft sigue usando
  peso × flete por kg.

## Cómo continuar

```bash
cd motor
cargo test                       # pruebas Rust vs Python
python generar_referencia.py     # regenerar referencias si cambia la versión Python
```

Al portar un módulo: (1) leer el `.py` vigente, (2) portarlo sin cambiar fórmulas, (3) generar
casos de referencia en Python, (4) prueba Rust que los reproduzca, (5) actualizar esta tabla.
