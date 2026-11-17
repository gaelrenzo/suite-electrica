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
| 6 | Validación final contra REDLIN con los scripts de `examples/validacion` portados, `.exe` y retiro de Python | — | ⏳ referencia congelada (`python-referencia-v1`); `comparar-redlin` portado (= Python en los 5 modos); `.exe` publicado (release `v0.2.0`; antes `v0.1.0-rust`); falta probarlo en otra PC y retirar Python |

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

1. ✅ Plantillas movidas a `data/plantillas/` (17/11): Rust las incluye desde ahí con `include_str!` y la versión Python las lee con `suite_electrica/_plantillas.py` (102 pruebas Python y 66 Rust pasan).
2. ✅ Portar `examples/validacion/comparar_redlin.py`: binario `comparar-redlin`
   (`motor/src/bin/comparar-redlin.rs`), mismos 5 modos y argumentos. Con los reportes del
   ejemplo de REDLIN (`SER-ejemplo.len`, exportados en local; no se distribuyen) imprime **las
   mismas cifras** que Python: catenaria (f y L con error 0,0000 m, vp ≤ 0,0098 m), tiro vs
   temperatura (1280 puntos, mediana 0,001 %), estado final con creep, cargas (FT, FL, Fv) y
   momentos/FS de postes.
   `cargo run --release --bin comparar-redlin -- PlanillaEstTotal.xls` (ver los demás modos en el
   archivo).
3. Probar el `.exe` en otra PC: descargarlo del *release*
   [`v0.2.0`](https://github.com/gaelrenzo/suite-electrica/releases/tag/v0.2.0)
   (se incluye `SHA256SUMS.txt`).
4. Confirmar con el autor.

## Funciones nuevas (solo en Rust)

Desde `python-referencia-v1`, las funciones nuevas se escriben solo en Rust y no cambian los
resultados de referencia. Si un cambio los altera a propósito, se explica aquí.

- **Cruces y distancias de seguridad (LineCraft):** tabla de cruces en el `.sel` (clave `cruces`,
  solo si hay; columnas progresiva, descripción, altura del obstáculo, distancia mínima y fuente).
  Al calcular se compara la cota del conductor a flecha máxima con la del obstáculo; los que no
  cumplen dan una alerta `DISTANCIA_SEGURIDAD` y se dibujan en rojo en el perfil; hoja «Cruces» en
  la planilla y el Excel. La suite no trae distancias propias: el usuario cita la tabla del CNE que
  aplique (sin fuente → aviso PENDIENTE_REVISION). Prueba: `tests/cruces.rs`.
- **Interfaz:**
  - Archivo → Abrir reciente (8 últimos `.sel`, redes y presupuestos; lista en
    `%APPDATA%\SuiteElectrica\recientes.txt`) y Guardar todo. Abrir acepta también JSON (red o
    presupuesto, según el contenido).
  - Ctrl+S y F6 actúan sobre la pestaña activa (proyecto, red o presupuesto).
  - Ayuda → Guía rápida (flujo de trabajo, atajos y advertencia sobre valores normativos).
  - Al cerrar se avisa de cambios sin guardar en el proyecto, la red y las tablas del presupuesto
    (antes solo del proyecto: los cambios de la red se perdían sin aviso).
- **Planos de entrega (DXF):** los DXF exportados desde la app llevan lámina en la capa LAMINA:
  marco ISO A3 (o A1 si en A3 pasaría de 1:5000), margen de encuadernación y cajetín (proyecto,
  plano, autor, escala, formato, fecha y código de lámina PT-01, PT-02, PP-01, RS-01), a la escala
  normalizada que encuadra el dibujo. `dxf::construir_*` devuelve el dibujo sin guardar; las
  funciones de siempre (`plano_topo`, `perfil_linea`…) siguen dando la salida igual a Python.
  Prueba: `dxf::prueba_lamina` (escala, formato, encuadre y fecha).
- **Vista en planta de LineCraft:** selector «Perfil | Planta». La planta muestra curvas de nivel,
  puntos, eje y estructuras; clic selecciona, doble clic agrega una estructura en la progresiva del
  punto proyectado sobre el eje y arrastrar la mueve a lo largo del eje (`geo::proyectar_en_eje`,
  con prueba unitaria).
- **Editores de tablas en la app (sin tocar CSV a mano):** un mismo editor (`app/tablas.rs`) con
  búsqueda, edición de celdas, agregar/duplicar/eliminar filas y filas virtualizadas.
  - PresuCraft: pestañas «Datos» (parámetros y fuentes del JSON; suministros, unidades, montaje,
    metrado, APU y agrupación FP) y «Resultados». Calcular guarda lo editado; también Guardar y
    Descartar cambios. Botones para crear las tablas de APU y de agrupación FP. Suministros
    muestra la columna `iu` aunque el archivo no la tenga.
  - `catalogos::ArchivoCsv` lee y escribe el CSV conservando los comentarios `#` (prueba unitaria
    de ida y vuelta).
  - LineCraft: ventana «Editar catálogos…» (postes, armados, retenidas, materiales), guardados en el
    `.sel`.
  - `suite-electrica.exe presupuesto.json` abre el presupuesto directo en PresuCraft.
- **GridCraft gráfico (editor en planta, como REDCAD):** la red se dibuja y edita en la app, sin
  preparar CSV a mano.
  - Herramientas: seleccionar/mover (el vano se recalcula con las coordenadas), agregar poste
    (cuelga del seleccionado; el primero es la SED) y eliminar (Supr; no borra postes con
    derivaciones). Deshacer con Ctrl+Z.
  - Panel de propiedades: nombre, padre, conductor, vano, lotes, cargas, coordenadas y estructura
    del metrado (armado, soporte, PAT, caja, pastoral, retenida, acometidas); parámetros de la red y
    tabla de conductores.
  - Fondo: un DXF (LINE/POLYLINE/LWPOLYLINE, `entrada::lineas_dxf`) y/o la topografía del proyecto.
  - Tramos coloreados por %ΔV (verde ≤ 60 % del límite, ámbar ≤ límite, rojo si lo excede).
  - `red::guardar` escribe el JSON (conserva claves que no conoce) y los CSV de nodos y conductores;
    `suite-electrica.exe red.json` abre la red directo en GridCraft. Prueba: `tests/red_guardar.rs`.

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
  el cálculo completo con una línea 1Φ, una 2Φ con neutro y una 3Φ con neutro. Biposte en H:
  ver «Otras brechas».
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

  Pruebas: unitarias en `apu.rs` y `tests/presu_apu.rs`.
* **Materiales de los armados BT (Norma DGE RD 023-2003, láminas 001-006).** La plantilla
  `materiales_bt.csv` tiene E1 a E6 y sus variantes /S, solo con cantidades fijas: sin los ítems
  «s.req.» ni el poste. `red::materiales` multiplica el metrado de armados por esa tabla (E'n usa
  los de En), y la planilla de GridCraft agrega la hoja «Materiales».

  La plantilla incluye también las láminas 007, 008, 012, 014, 015 y 016, con códigos del
  metrado como claves:
  - acometidas corta y larga;
  - puesta a tierra en poste de concreto (PAT-Concreto);
  - retenidas inclinada (RI-BT; RI-MT es la misma sobre poste de MT) y vertical (RV-BT);
  - pastoral con luminaria (Pastorales-Concreto).

  La lámina 013 (puesta a tierra en madera) no está en el PDF de la norma.

  **Validación:** `comparar-redcad PlanillaRS.xls PlanillaMateriales.xlsx --materiales`, contra
  la planilla de materiales de REDCAD (todas las estructuras del ejemplo).
  - **18 de 22 materiales con cantidad fija coinciden exactos:**
    - armados: grapas de suspensión (59) y de anclaje (73), pernos con gancho (59) y con ojal
      (57), tuercas ojal (16) y cajas de derivación (7);
    - retenidas: varillas, arandelas de anclaje, bloques, pernos angulares y conectores de doble
      vía (58 de cada uno), mordazas (116) y alambre de entorchar (174 m);
    - puesta a tierra: electrodos y conectores de bronce (23) y conductor de cobre de 16 mm²
      (288 m);
    - alumbrado: luminarias (25).
  - **Difieren por reglas de REDCAD que no están en las láminas:**
    - portalíneas (14 frente a 116) y conductor N2XY (8,4 m frente a 53,4 m), que REDCAD también
      agrega en acometidas, alumbrado y las variantes /S («según requerimiento»);
    - cable de acero (580 frente a 584 m; 4 m más, probablemente en la retenida sobre poste de MT);
    - cajas portamedidor (87 frente a 3; REDCAD lleva las conexiones domiciliarias aparte);
    - fleje (7 frente a 2): no se identificó la regla.
* Lo que no se valida en PresuCraft: el transporte por
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

### Compilar y probar en Windows

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\mingw64\bin;" + $env:Path   # Rust GNU + MinGW-w64 portable
cd motor
cargo test --release                                  # todas las pruebas
cargo build --release --bin suite-electrica           # target\release\suite-electrica.exe
```

Abrir directo un archivo: `suite-electrica.exe proyecto.sel`, `suite-electrica.exe red.json` o
`suite-electrica.exe presupuesto.json` (red o presupuesto se distinguen por el contenido).

Comparar con los programas comerciales (reportes exportados de ellos; no se suben al repositorio):

```powershell
.\target\release\comparar-redlin.exe  ...              # LineCraft vs REDLIN
.\target\release\comparar-redcad.exe PlanillaRS.xls CaidaTension.xls          # caída de tensión
.\target\release\comparar-redcad.exe PlanillaRS.xls MetradoRS.xls --metrado   # metrado
.\target\release\comparar-redcad.exe PlanillaRS.xls PlanillaMateriales.xlsx --materiales
.\target\release\comparar-presio.exe ... --apu | --fp                          # PRESIO
```

### Hecho tras la referencia (detalle)

1. ✅ **Cruces en la app:** revisado en pantalla el 16/11 (marcas en el perfil con su distancia,
   verde si cumple, y hoja «Cruces»); la verificación pasa `tests/cruces.rs`.
2. **GridCraft:**
   - ✅ asignar la misma estructura a varios postes a la vez: Mayús+clic arma la selección; el
     panel copia solo los campos marcados (armado, soporte, PAT, caja, pastoral, retenida,
     conectores, acometidas) con «Aplicar a N postes» y Ctrl+Z (`red::asignar_estructura`,
     prueba `tests/red_asignar.rs`). Revisado en pantalla el 16/11;
   - ✅ simbología en el plano y en el DXF (`red::simbolos_red`, `dxf::simbologia_red`, prueba
     `tests/red_simbolos.rs`): SED (cuadrado), retenida con anclaje contra la resultante de los
     vanos (fuera del ángulo, en prolongación en terminales, perpendicular en alineamiento; varias
     a ±12,5°), pastoral con luminaria al lado opuesto (nL si hay más de una lámpara) y tipo de
     poste (soporte del metrado). En el DXF van en las capas SIMB_* con leyenda (capa LEYENDA);
     `construir_plano_red` no cambia (paridad con Python). En la app: casilla «Simbología».
     Es solo dibujo, no norma. Revisado en pantalla el 16/11.
3. ✅ **Cimentaciones** (`motor/src/cimentacion.rs`, prueba `tests/cimentaciones.rs`): tabla
   `cimentaciones.csv` junto a `red.json` o al `.sel`, una fila por código de soporte/poste
   (forma cuadrada o circular, dimensión, profundidad, solado, diámetro de base, relleno con
   concreto o compactado, fuente). Por poste: excavación A·h, solado A·s, concreto o relleno
   A·(h−s) − poste empotrado, eliminación = excavación − relleno. La plantilla trae solo valores
   de **EJEMPLO**; se avisa mientras la fuente diga EJEMPLO/PENDIENTE, si un soporte no tiene fila
   o falta el diámetro de base.
   - GridCraft: hoja «Cimentaciones» (también en el Excel exportado), botón «Cimentaciones…»
     (crea/abre el CSV) y «Nueva red» copia la plantilla.
   - PresuCraft: «Metrado desde GridCraft/LineCraft» agrega CIM-EXC, CIM-SOL, CIM-CON, CIM-REL
     y CIM-ELI (m³) al rubro RS o LP; sus precios van en `montaje.csv` (si faltan, PresuCraft
     avisa «costo 0»).
   - LineCraft: hoja «Cimentaciones» en la planilla.
4. **Release:** ✅ `v0.2.4` (17/11/2026: valores normativos citados de la DGE/MEM-DEP 501 y el CNE 2011). Antes `v0.2.3` (17/11/2026: cuelgue con nubes grandes y franja SRTM, editor de puntos, invertir eje, amortiguadores, revisión del diseño, rehacer, varias redes). Antes `v0.2.2` (17/11/2026: retenidas de biposte y transporte por origen como REDLIN y PRESIO, alerta H/w). Antes `v0.2.1` y `v0.2.0`. `v0.1.0-rust` se volvió a subir
   varias veces con el mismo nombre (último build: `4f13cf6`); desde ahora cada build publicado
   lleva un número de versión nuevo y el README enlaza a `releases/latest`.
5. **Brechas con ESOLUTIONS de la primera lista:** ✅ todas tienen una primera versión.
   - ✅ **Catastro y faja de servidumbre** (`motor/src/catastro.rs`, prueba `tests/catastro.rs`):
     botón «Catastro (DXF)…» en TopoCraft (predios = polilíneas cerradas; nombre = texto dentro).
     Por predio: progresivas y longitud del eje dentro, área de faja y % del predio; lista en el
     panel, predios dibujados en la planta y `predios_afectados.xlsx` al exportar. El ancho de faja
     (11 m de **EJEMPLO**) se edita en el panel y se cita en «Fuentes» (`ancho_servidumbre_m`).
     Se guarda en el `.sel` (clave `catastro`, solo si hay predios). Área por muestreo de 0,5 m:
     en los vértices del eje hay una cuña de error (anotado en el código con `ponytail:`).
   - ✅ **Terminal a media línea** (doble remate; prueba `tests/terminal_intermedio.rs`): una
     estructura FIN con vanos a ambos lados ya no toma la diferencia de tiros (como un anclaje, casi
     cero), sino el tiro completo del lado mayor, porque cada lado se ancla por separado (al tender
     o si se corta el otro lado). Lleva 2 retenidas por defecto (una por lado) y cada una se
     verifica con el tiro completo de su lado. Cambiar el tipo de una estructura devuelve sus
     retenidas a «auto» si tenía 0 o 1. Los extremos no cambian (paridad con Python intacta).
     Además: `cambiar_estructura` con un índice inexistente devuelve un error en vez de cerrar el
     programa. No se comparó con REDLIN.
   - ✅ **Red primaria (MT)** con el mismo motor de GridCraft (prueba `tests/red_primaria.rs`, que
     verifica corriente, ΔV y %ΔV del primer tramo contra el cálculo a mano): botón «Nueva red
     primaria…» con las plantillas `red_mt_proyecto.json` (22,9 kV, 3F), `red_mt_nodos.csv` (punto de
     alimentación y subestaciones con su demanda en «Carga especial», kW) y `conductores_mt.csv`
     (AAAC 25-70 mm² de **EJEMPLO**: r = ρ/S con la aleación 6201 y x por fórmula con DMG = 1,20 m;
     reemplazar por la ficha del fabricante y la DMG real). No se comparó con REDCAD.
   - ✅ **Relieve SRTM** (`entrada::leer_hgt`, `Documento::importar_srtm`, prueba `tests/srtm.rs`):
     botón «Relieve SRTM (.hgt)…» en TopoCraft. Lee la hoja (1201² o 3601² enteros big-endian; la
     esquina sale del nombre, p. ej. `S16W070.hgt`), descarta vacíos, convierte a UTM de la zona del
     proyecto y se queda con una franja de 300 m alrededor del eje (hay que importar antes el eje).
     Reemplaza los puntos y anota en «Fuentes» que es SRTM, solo para anteproyecto. La hoja la
     descarga el usuario (p. ej. de EarthExplorer del USGS); la suite no descarga nada.
   - ✅ **Transporte por rutas** (como PRESIO; prueba `tests/presu_transporte.rs`): tabla opcional
     `transporte.csv` (ruta, tramo, vía, distancia_km, costo_t_km, precio_kg, fuente) enlazada en el
     JSON con la clave `transporte`. Flete por kg de cada ruta = Σ precio_kg + Σ distancia × costo
     por t·km / 1000; cada suministro usa el de su ruta (ver la validación con PRESIO); hoja «Transporte» en los resultados y el Excel; aviso por cada tramo sin
     fuente. En la app: «Crear ruta de transporte» en Parámetros y pestaña «Transporte (ruta)».
     Validado con PRESIO el 17/11 (ver punto 3 de Pendientes).
   - GridCraft: revisado en pantalla el 16/11 la selección múltiple (Mayús+clic, «Aplicar a 3
     postes» con armado y retenida), la simbología (SED, luminarias, retenidas) y el metrado.
     Se corrigió el redondeo del metrado (0.11500000000000002 km → 0.115).
   - ✅ **Biposte en H** (Norma DGE RD 024-2003, láminas 027-030; prueba `tests/biposte_h.rs`):
     7 armados (PSH-3/3N, PA1H-3/3N, PRH-3/3N, PA2H-3) con geometría y materiales transcritos de
     las láminas, y columna nueva `postes` en `armados.csv` (2 en los H). Cada poste toma la mitad
     de las cargas de los conductores (MVC, MTB, MTD) y todo el viento sobre sí mismo
     (`catalogos::momentos_por_poste`); también en el criterio simplificado y en la elección de
     poste por MRN. Es conservador: no cuenta el efecto pórtico de las crucetas. El metrado cuenta
     dos postes por estructura H. La elección automática no los toma (están al final): se asignan
     a mano en la tabla de estructuras. Un `.sel` existente necesita «Cargar catálogos de la
     plantilla». No se comparó con REDLIN.
   - ✅ **Autodiseño BT** (`red::autodisenar`, prueba `tests/red_autodiseno.rs`, botón
     «Autodiseñar conductores» en GridCraft; Mayús+clic parte de las formaciones actuales; Ctrl+Z
     deshace): a cada tramo, la formación más chica (mayor K) que cumple %ΔV SP/AP, corriente y
     pérdidas, solo entre formaciones con las mismas fases y con o sin hilo de AP. Voraz (agranda
     el tramo del camino a la SED que más baja la caída) y pasada final que achica mientras
     cumpla: da un mínimo local, no necesariamente el de menor costo. No compara con REDCAD.

### Pendientes

1. **Retirar Python.** Bloquean: probar `suite-electrica.exe` en otra PC sin Python y la
   confirmación explícita del autor. (✅ Plantillas ya movidas a `data/plantillas`.)
2. ✅ **Revisar en pantalla el relieve SRTM** (17/11, hoja real S16W070 de Capachica con un eje
   de 8 km): encontró dos defectos, corregidos:
   - **la app se colgaba sin memoria** al procesar la nube: `detectar_atipicos` buscaba los 8
     vecinos de cada punto comparando con todos (O(n²); 37 627 puntos). Ahora usa una grilla
     espacial con el mismo resultado exacto (prueba `grilla_igual_a_fuerza_bruta` contra la fuerza
     bruta, con empates y duplicados): limpiar 0,2 s, TIN 0,06 s, curvas 0,3 s;
   - se importaba el **rectángulo** que contiene al eje (37 627 puntos) y no la franja de 300 m que
     dice el mensaje: ahora se conservan solo los puntos a ≤ 300 m del eje (5 490). ✅ Revisados el 17/11: cimentaciones (cantidades = cálculo a mano), autodiseño,
   red primaria y catastro. Esa revisión encontró y corrigió (v0.2.1):
   - el autodiseño no cambiaba nada con catálogos sin `k_ohm_km` (K calculada de r y x, como el
     ejemplo): ahora usa la misma K que el cálculo (prueba `catalogo_con_k_calculada`);
   - los CSV guardados por Excel como «CSV UTF-8» (con BOM) no se leían bien (la red salía con
     10 raíces): `catalogos::leer_csv` quita el BOM;
   - la plantilla de red primaria trae coordenadas de ejemplo para verse en planta y exportar DXF.
3. **Comparar con el software comercial** (la demo de 18 días corre desde el 13/11, vence ≈ 01/12):
   - ✅ **Terminal a media línea, con REDLIN** (17/11). En una copia de `SER-ejemplo.len` se cambió
     el armado de la estructura 6 de la Troncal (PS1-3L, suspensión) a PTV-3 (terminal) editando el
     archivo: el `.len` es una serialización MFC sin cifrar y cada nodo guarda en el byte 53 de su
     registro el índice del armado primario (posición en el catálogo + 1; se dedujo cruzando los
     registros con `PlanillaEstTotal.xls`). REDLIN lo abrió y se exportaron sus planillas. Resultado:
     REDLIN reporta como «longitudinal» solo el desbalance (49 kg-f), pero **verifica el poste con el
     tiro completo del lado mayor** (257 kg-f): MRN 68 494,6 N·m frente a 68 810,8 de la suite
     (+0,46 %), FS 0,953 frente a 0,948. Es la regla de la suite (`tests/terminal_intermedio.rs`).
     REDLIN no agrega retenidas por el cambio de armado (la suite propone 2, una por lado).
   - ✅ **Biposte en H, con REDLIN** (17/11): `comparar-redlin --postes` ahora incluye las estructuras
     de 2 postes con armado H (las «2x…», dos postes independientes, quedan fuera). Las 5 PRH-3 del
     ejemplo coinciden (mediana 0,102 %, máx. 0,675 %) tras una regla nueva deducida de REDLIN: en un
     biposte las retenidas solo toman toda la carga con 2 por poste; con menos toman la longitudinal y
     la del ángulo queda en los postes (`catalogos::toma_biposte`, prueba `retenidas_de_biposte`).
     Antes la suite suponía que 2 retenidas tomaban todo en un H en ángulo: MRN −84 %, no
     conservador.
   - **Abierto: terminales TS y suspensiones PS con retenida** (diferencias que ya estaban en la
     comparación base: TS n=31 mediana 45 %, PS n=7 mediana 123 %). Análisis del 17/11 por
     estructura: en las TS sin ángulo el MRN de REDLIN queda entre «la retenida toma la
     longitudinal» y «sin retenida» (una, la fila 55, coincide −0,02 %): REDLIN deja en el poste el
     momento de cada fase por su altura sobre el anclaje de la retenida (brazo implícito 0,24 a 4 m
     según la estructura, coherente con armados verticales). En las TS con ángulo REDLIN da menos:
     la retenida también toma parte de la transversal. Modelarlo exige las alturas de fase y de
     anclaje de la retenida de cada armado, que el reporte de REDLIN no trae por separado; no se
     cambió el motor para no introducir un error sin evidencia suficiente.
     Actualización (17/11): en la base de REDLIN (`BDS.xls`, hoja Armado) los TS son de tipo
     **TERMINAL-DERIVACIÓN**: el poste lleva la línea principal y el arranque de una derivación. En
     su reporte, cuando la retenida toma todo, MRN = MVC + MVE exacto (fila 55, igual que la suite);
     en las demás TS el momento residual es una fracción variable de su MTD (0,26 a 0,54), que
     depende del ángulo entre la derivación y la línea principal, no de las alturas del armado. Para
     compararlas hace falta la geometría de cada derivación (no está en la planilla); el motor de
     derivaciones de la suite (`derivaciones.rs`) sí combina ambas cargas.
   - `COMPARAR_DETALLE=texto` hace que `comparar-redlin --postes` imprima una línea por estructura
     cuyo armado contiene el texto (postes, retenida, FL, MRN y FS de REDLIN y de la suite).
   - ✅ **Transporte por rutas, con PRESIO** (17/11). PRESIO no usa una ruta única en t·km: su
     hoja «Flete T» da un flete por kg para cada origen o ruta y cada suministro tiene su origen;
     transporte = peso × flete de su origen. La suite se cambió a ese modelo (`Suministro.ruta`,
     columna `ruta` u `origen` en `suministros.csv`; `transporte.csv` con `ruta` y `precio_kg`
     directo y/o tramos en t·km; con una sola ruta funciona como antes). `comparar-presio
     Reporte-Presupuesto.xls BaseDatosPRESIO.xlsx --transporte` con el ejemplo de PRESIO: 172
     renglones de LP, RP y RS, 164 iguales y el resto con ≤ 0,005 S/ (redondeo); los 19 renglones
     cuyos suministros no están en la base instalada cierran la diferencia con el total de cada hoja
     (LP: 31 593,21 + 1 566,22 = 33 159,43). Prueba `flete_por_origen_del_suministro`.
   - **No comparables con REDCAD:** REDCAD no tiene autodiseño de conductores (su «autodiseño»
     distribuye estructuras y suministros desde los lotes, manual cap. 6.1) y su reporte de caída de
     tensión es solo de baja tensión, así que la red primaria y el autodiseño de la suite quedan
     validados solo con sus pruebas (cálculo a mano en `tests/red_primaria.rs`).
4. **Valores normativos** (17/11, verificados en los PDF que entregó el autor):
   - ✅ Faja de servidumbre 11 m: CNE Suministro 2011, Regla 219.B.2 y Tabla 219 (20–36 kV), con
     la salvedad de 219.B.4 (si el desplazamiento por viento da más, el mayor).
   - ✅ Conductores de línea AAAC 25-95: DGE/MEM-DEP 501 §4.2.2 (n.° y Ø de alambres, Ø exterior,
     masa, E final 60 760 N/mm², rotura 295,8 N/mm² × sección). La ficha Nexans de 34,4 mm² que se
     entregó es AS 1531 (Nueva Zelanda, otra aleación, 8,18 kN ≈ 238 N/mm²) y **no** se usó: la DGE
     §4.2.1 exige ASTM B398/B399 o IEC 1089 (la ficha Nexans de 39,25 mm², ASTM B399, da 317 N/mm²,
     coherente). El AAAC 35 de la plantilla pasó de E 58 860 a 60 760 N/mm² y de 10,199 a 10,164 kN.
   - ✅ Conductores de red primaria: DGE/MEM-DEP 501 Cuadro N° 6.1, R a 40 °C (la norma usa la
     resistencia a la temperatura de operación, §6.2.1) y X1 con DMG 1,20 m. `tests/red_primaria.rs`
     comprueba que el %ΔV de la suite es el de la fórmula de la norma (§6.2.2, ΔV % = K1·P·L con
     K1 = 2,387e-4 del Cuadro N° 6.2 para 35 mm²).
   - ✅ EDS inicial 18 % (final 15 %, referencial sin amortiguadores): DGE §4.3.1. Tiro máximo:
     **60 %** del de rotura para AAAC (DGE §4.3.2; antes la plantilla tenía 40 % sin fuente).
   - Pendientes por ser del proyecto, no genéricos: temperaturas, viento y hielo de las hipótesis
     (la DGE §4.4 fija las seis hipótesis; los valores salen de la zona del CNE), la distancia mínima
     al terreno (CNE Regla 232, según la superficie y el tipo de cruce) y las cimentaciones (la DGE
     pide calcularlas para el terreno y las cargas del proyecto: la tabla queda de EJEMPLO).
   - La prueba Python `test_nuevo_proyecto_desde_plantilla` cuenta ahora los parámetros que siguen
     PENDIENTE en la plantilla (antes suponía que lo estaban todos).
5. **Brechas con ESOLUTIONS según sus manuales** (también en la hoja de ruta del README):
   - TopoCraft: ✅ numeración y validación de viviendas, separar/unir/empalmar rutas y puntos sobre
     el eje (18/11, `tests/rutas_viviendas.rs`, revisado en pantalla). Pendiente: ortofotos georreferenciadas como fondo;
     edición de rutas (proyectar, unir); códigos de campo (CTU). ✅ Exportar KML y puntos (18/11,
     `motor/src/kml.rs`, prueba `tests/kml.rs` que reimporta ambos): al exportar TopoCraft escribe
     `puntos.csv` y `linea.kml` (eje, estructuras con su armado y predios); GridCraft escribe
     `red.kml` (tramos y postes) en la zona UTM del proyecto abierto. ✅ Invertir eje (17/11): botón
     en TopoCraft; las progresivas de estructuras y cruces pasan a L − s y siguen en el mismo
     punto del terreno (`Documento::invertir_eje`, prueba `invertir_eje_conserva_ubicaciones`). ✅ Edición de puntos
     (17/11): ventana «Editar puntos…» con la tabla (buscar, editar, agregar, eliminar; «Aplicar
     cambios» valida cada número) y transformación de la nube: traslación ΔE/ΔN/ΔZ y rotación
     alrededor del centroide, opcionalmente también del eje (`Documento::transformar_puntos`,
     prueba `tests/puntos_transformar.rs`). Revisado en pantalla: 2 601 puntos girados 30° con
     el eje, perfil sin cambios.
   - LineCraft: ✅ alertas de cruceta (Cr), vano peso máximo (P↓) y arrancamiento en suspensión (18/11):
     columnas nuevas `vano_peso_max_m` y `carga_max_cruceta_n` en el catálogo de armados (vacías = no
     se verifica; el dato es del fabricante). El arrancamiento (S↑) se revisa con aislador rígido o en
     estructuras de suspensión (antes solo rígido; las cadenas de anclaje trabajan horizontales) y,
     con vano peso máximo, su exceso es % de ese máximo como en REDLIN. Paridad con Python intacta.
     Prueba `tests/alertas_armado.rs`.
   - LineCraft: ✅ amortiguadores por vano (17/11; tabla «Amortiguadores por vano…» en el `.sel`
     con vano_hasta_m, cantidad_por_conductor y fuente del fabricante; el vano usa la primera fila
     que lo cubre y el total entra al metrado como AMORT, también hacia PresuCraft; prueba
     `tests/amortiguadores.rs`); verificación de crucetas y
     esquemas de aisladores; ◐ planos con plantilla del usuario (18/11: `plantilla_lamina.dxf` junto al
      proyecto reemplaza el marco y cajetín propios en todos los planos; faltan los bloques INSERT). ✅ Vibraciones eólicas, criterio H/w
     de CIGRE TB 273 (prueba `tests/vibraciones_hw.rs`): parámetro «H/w máx.» en las alertas
     (0 = no verificar; el límite según la categoría de terreno lo pone el usuario con su fuente,
     `hw_max_m`); alerta «V» por tramo con H/w en la hipótesis inicial y L·D/m del vano mayor para
     decidir amortiguadores.
   - GridCraft: ◐ autodistribución de la red BT desde los lotes (18/11; `red::autodistribuir`, prueba
     `tests/red_autodistribucion.rs`): botón «Autodistribuir desde lotes…» con lotes del catastro del
     proyecto o de un DXF (centro de cada polilínea cerrada, o puntos). Cobertura voraz (cada lote a
     ≤ acometida larga de un poste; 15/30 m de las láminas 007/008 de la DGE RD 023-2003), árbol de
     menor longitud hasta la SED (poste seleccionado o centro de los lotes) y postes intermedios en
     vanos > máximo; cada poste lleva sus lotes y acometidas C/L. Revisado en pantalla (40 lotes,
     10 postes, cumple). No sigue calles: las líneas pueden cruzar predios. ✅ Capas (18/11): menú «Capas ▾»
     (fondo DXF, topografía, longitud de vanos, nombres y %ΔV, retenidas, alumbrado, SED y tipo de
     poste); queda abierto para marcar varias. Revisado en pantalla. ✅ Varias redes juntas
     (17/11): «Metrado de varias redes (RS)…» en PresuCraft suma el metrado y las cimentaciones de
     varios `red.json` en el rubro RS (`presupuesto::sumar_metrado_csv`, prueba
     `sumar_metrado_de_varias_redes`). ✅ Rehacer (17/11): botón y Ctrl+Y; deshacer guarda el estado para rehacerlo y
     cada edición nueva vacía la pila (revisado en pantalla: autodiseño → deshacer → rehacer). ✅ Revisión automática del diseño (17/11, `red::revisar`, hoja «Revisión»
     en la app y el Excel; prueba `tests/red_revision.rs`): nombres repetidos, postes sin
     coordenadas, vano distinto de la distancia en el plano (> 2 m y > 5 %), postes sin armado,
     lámparas sin pastoral, lotes sin acometidas y fines de línea sin retenida (las tres últimas
     solo si el diseño ya usa ese dato). No usa valores normativos.

Reglas que se mantienen: no copiar datos de software comercial (los reportes de REDLIN, REDCAD y
PRESIO solo se usan localmente para comparar); todo valor normativo lleva su fuente; cada función
nueva deja una prueba que falle si se rompe; `git pull` antes de editar y commits cortos en español.
