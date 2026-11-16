<p align="center">
  <img src="docs/img/banner.svg" alt="Suite Eléctrica" width="100%">
</p>

<p align="center">
  <b>Suite libre para ingeniería eléctrica de distribución en Perú</b><br>
  Topografía, líneas primarias, redes de distribución y presupuestos en un solo ejecutable, sin instalar nada.
</p>

<p align="center">
  <a href="https://github.com/gaelrenzo/suite-electrica/releases/latest/download/suite-electrica.exe"><img alt="Descargar para Windows" src="https://img.shields.io/badge/Descargar-Windows%20.exe-2ea44f?style=for-the-badge&logo=windows"></a>
</p>

<p align="center">
  <a href="LICENSE"><img alt="Licencia MIT" src="https://img.shields.io/badge/licencia-MIT-blue"></a>
  <a href="https://github.com/gaelrenzo/suite-electrica/releases"><img alt="Versión" src="https://img.shields.io/badge/versión-0.2.2%20(prueba)-orange"></a>
  <a href="https://github.com/gaelrenzo/suite-electrica/actions/workflows/rust.yml"><img alt="Pruebas Rust" src="https://github.com/gaelrenzo/suite-electrica/actions/workflows/rust.yml/badge.svg"></a>
  <a href="https://github.com/gaelrenzo/suite-electrica/actions/workflows/pruebas.yml"><img alt="Pruebas Python" src="https://github.com/gaelrenzo/suite-electrica/actions/workflows/pruebas.yml/badge.svg"></a>
  <img alt="Hecho en Rust" src="https://img.shields.io/badge/hecho%20en-Rust-b7410e?logo=rust">
</p>

---

## ¿Qué es?

Suite Eléctrica reúne en una sola aplicación de escritorio el flujo completo de un proyecto de
electrificación: **del levantamiento topográfico al presupuesto**. Es una alternativa libre a las
herramientas comerciales que se usan en Perú, construida a partir de la normativa pública
(CNE Suministro 2011, normas DGE/MINEM) y validada número a número contra ellas.

| | Módulo | Hace | Equivale a |
|---|---|---|---|
| 🗺️ | **TopoCraft** | Limpieza de puntos, TIN, curvas de nivel, perfil longitudinal, relieve SRTM | TOPOMAGIC |
| ⚡ | **LineCraft** | Líneas primarias MT: ubicación de postes, cálculo mecánico, despejes, cargas, retenidas, flechado | REDLIN |
| 🔌 | **GridCraft** | Redes BT/MT en planta: caída de tensión, pérdidas, autodiseño de conductores, metrado | REDCAD |
| 💰 | **PresuCraft** | Presupuesto por unidades constructivas, APU, fórmula polinómica, transporte | PRESIO |

<p align="center">
  <a href="#descargas">Descargas</a> ·
  <a href="#capturas">Así se ve</a> ·
  <a href="#comparacion">Comparación</a> ·
  <a href="#flujo">Flujo de trabajo</a> ·
  <a href="#hoja-de-ruta">Hoja de ruta</a> ·
  <a href="#faq">Preguntas frecuentes</a> ·
  <a href="#desarrolladores">Desarrolladores</a>
</p>

<a name="descargas"></a>

## ⬇️ Descargas (Windows 10/11)

| Programa | Descarga | Tamaño |
|---|---|---|
| **Suite Eléctrica** (app principal) | [suite-electrica.exe](https://github.com/gaelrenzo/suite-electrica/releases/latest/download/suite-electrica.exe) | 22,9 MB |
| Comparador con REDLIN | [comparar-redlin.exe](https://github.com/gaelrenzo/suite-electrica/releases/latest/download/comparar-redlin.exe) | 1,4 MB |
| Comparador con REDCAD | [comparar-redcad.exe](https://github.com/gaelrenzo/suite-electrica/releases/latest/download/comparar-redcad.exe) | 1,6 MB |
| Comparador con PRESIO | [comparar-presio.exe](https://github.com/gaelrenzo/suite-electrica/releases/latest/download/comparar-presio.exe) | 1,4 MB |
| Sumas de verificación | [SHA256SUMS.txt](https://github.com/gaelrenzo/suite-electrica/releases/latest/download/SHA256SUMS.txt) | — |

Descargar y abrir: **no necesita instalación ni Python**. Versión de prueba 0.1.0 ·
[todas las versiones](https://github.com/gaelrenzo/suite-electrica/releases).

> Windows puede mostrar el aviso de SmartScreen (ejecutable sin firmar):
> *Más información → Ejecutar de todas formas*.

**Requisitos:** Windows 10 u 11 de 64 bits. La interfaz usa DirectX 12 o Vulkan; en equipos sin
tarjeta gráfica, Windows lo emula (WARP). Para verificar la descarga:
`Get-FileHash suite-electrica.exe` en PowerShell y comparar con `SHA256SUMS.txt`.

<a name="capturas"></a>

## 🖼️ Así se ve

Salidas reales generadas por la suite con los datos de ejemplo del repositorio.

<p align="center">
  <img src="docs/img/perfil-linea.png" alt="Perfil de una línea primaria con postes, conductor a flecha máxima y despeje mínimo" width="100%"><br>
  <sub><b>LineCraft</b> · perfil de la línea: estructuras ubicadas automáticamente, conductor a flecha máxima y curva de despeje mínimo.</sub>
</p>

<table>
  <tr>
    <td width="50%" align="center">
      <img src="docs/img/curvas-nivel.png" alt="Curvas de nivel y eje de la línea"><br>
      <sub><b>TopoCraft</b> · curvas de nivel (mayores y menores) desde el TIN, con el eje de la línea.</sub>
    </td>
    <td width="50%" align="center">
      <img src="docs/img/memoria.png" alt="Primera página de la memoria de cálculo en PDF"><br>
      <sub><b>Memoria de cálculo PDF</b> · cada parámetro lleva su fuente; lo no verificado se marca EJEMPLO.</sub>
    </td>
  </tr>
</table>

<a name="comparacion"></a>

## 📊 Comparación

| | Suite Eléctrica | Suite comercial (4 programas) |
|---|---|---|
| Tamaño | **1 archivo de 23 MB** | 4 instaladores, ~320 MB |
| Instalación | No necesita | Instalador por programa + activación |
| Licencia | **Libre (MIT), sin vencimiento** | De pago; la demo dura 18 días |
| Código fuente | Abierto, con pruebas automáticas | Cerrado |
| Fórmulas | Documentadas en `docs/` con su fuente | Según el manual de usuario |
| Formatos de salida | DXF R12, Excel, PDF | DXF, Excel, reportes propios |

La suite comercial tiene todavía funciones que aquí faltan: ver la [hoja de ruta](#hoja-de-ruta).

## ✨ Por qué usarla

- **Un solo `.exe` de 23 MB.** Los cuatro programas comerciales equivalentes pesan unos 320 MB.
- **Validada contra el software comercial.** Mismas cifras que REDLIN en catenaria, tiros, cargas
  y momentos de postes. Caída de tensión de REDCAD con diferencias ≤ 0,04 V y metrado 33/33.
  APU de PRESIO 102/102 y las 3 fórmulas polinómicas iguales.
- **Sin valores inventados.** Cada valor normativo lleva su fuente; lo que no la tiene se marca
  `PENDIENTE` o `EJEMPLO` y la app avisa.
- **Formatos de siempre.** Lee CSV, DXF, KML, GPX, MapSource, Excel RLT y TPG. Entrega planos
  DXF con lámina y cajetín, planillas Excel y memorias de cálculo en PDF.
- **Libre (MIT).** Úsala, estúdiala y mejórala.

<a name="flujo"></a>

## 🧭 Flujo de trabajo

```
 Puntos topográficos ─► TopoCraft ─► perfil ─► LineCraft ─► estructuras ─┐
                                                                         ├─► PresuCraft ─► presupuesto
                       Plano de la localidad ─► GridCraft ─► metrado ────┘
```

1. **TopoCraft:** importa los puntos, revisa el reporte de limpieza y genera curvas y perfil.
2. **LineCraft:** ubica postes automáticamente (o con el ratón sobre el perfil o la planta),
   calcula con F6 y revisa alertas, despejes y cruces.
3. **GridCraft:** dibuja la red en planta, calcula caída de tensión y autodiseña conductores.
4. **PresuCraft:** importa el metrado de LineCraft y GridCraft y calcula el presupuesto.

Guía paso a paso: [`docs/GUIA_RAPIDA.md`](docs/GUIA_RAPIDA.md).

## 📐 Fundamentos de cálculo

| Módulo | Documento |
|---|---|
| TopoCraft | [`docs/topocraft.md`](docs/topocraft.md) |
| LineCraft | [`docs/linecraft.md`](docs/linecraft.md) |
| GridCraft | [`docs/gridcraft.md`](docs/gridcraft.md) |
| PresuCraft | [`docs/presucraft.md`](docs/presucraft.md) |

Estado de la migración a Rust y validación contra los comerciales:
[`docs/PLAN_RUST.md`](docs/PLAN_RUST.md).

<a name="hoja-de-ruta"></a>

## 🗺️ Hoja de ruta

Lo que falta para estar a la par de la suite comercial, según sus manuales de usuario.

**TopoCraft**
- [x] Importar CSV, DXF, KML, GPX, MapSource, Excel RLT y TPG
- [x] TIN, curvas de nivel, perfil longitudinal y relieve SRTM
- [x] Catastro: predios afectados por la faja de servidumbre
- [ ] Numeración y validación de viviendas
- [ ] Ortofotos georreferenciadas como fondo
- [x] Edición de puntos (tabla, trasladar, rotar)
- [x] Invertir el sentido del eje
- [ ] Edición de rutas (proyectar, unir)
- [ ] Códigos topográficos de campo (CTU)

**LineCraft**
- [x] Cálculo mecánico, cargas, momentos de postes y alertas (igual a REDLIN)
- [x] Cruces y distancias de seguridad, biposte en H, cimentaciones
- [x] Terminal a media línea
- [x] Vibraciones eólicas: criterio H/w (CIGRE 273)
- [x] Amortiguadores por vano (tabla del fabricante, al metrado)
- [ ] Verificación de crucetas y esquemas de aisladores
- [ ] Planos con plantilla de bloques

**GridCraft**
- [x] Editor en planta, caída de tensión, metrado y materiales (igual a REDCAD)
- [x] Autodiseño de conductores BT, simbología, red primaria 22,9 kV
- [ ] Autodistribución de estructuras BT desde los lotes del catastro
- [x] Revisión automática del diseño
- [x] Rehacer (Ctrl+Y)
- [ ] Capas
- [ ] Reportes de varias redes juntas

**PresuCraft**
- [x] Presupuesto, APU y fórmula polinómica (igual a PRESIO)
- [x] Transporte por rutas

<a name="faq"></a>

## ❓ Preguntas frecuentes

**¿Puedo usarla en un proyecto real?**
Sí, pero revisa los avisos. Los catálogos de ejemplo traen valores marcados `EJEMPLO` o
`PENDIENTE_REVISION`; reemplázalos por los de tu proyecto con su fuente (CNE, norma DGE, ficha
del fabricante) antes de firmar una memoria.

**¿Abre los archivos de los programas comerciales (`.len`, `.red`, `.top`)?**
No: son formatos cerrados. Sí lee lo que esos programas exportan (Excel RLT, TPG, DXF, KML).

**¿Funciona en Linux o macOS?**
El ejecutable publicado es para Windows. La versión Python funciona en Linux; el código Rust
debería compilar con `cargo build --release`, pero aún no se ha probado fuera de Windows.

**Encontré un error en un cálculo.**
Abre un [issue](https://github.com/gaelrenzo/suite-electrica/issues) con el archivo de entrada y el
resultado esperado (y su fuente). Cada corrección deja una prueba que falla si el error vuelve.

---

<a name="desarrolladores"></a>

## 🛠️ Para desarrolladores

### Compilar el ejecutable (Rust)

```powershell
cd motor
cargo test --release                          # pruebas (Rust = referencia Python)
cargo build --release --bin suite-electrica   # target\release\suite-electrica.exe
```

En Windows se usa el toolchain GNU con MinGW-w64; detalles en [`docs/PLAN_RUST.md`](docs/PLAN_RUST.md).

### Versión Python (referencia)

* **Windows 11**: instalar Python 3.11+ (con "Add to PATH") y ejecutar `instalar.bat`;
  crea el acceso directo **Suite Electrica** (app) y los comandos.
* **Linux**: `python3 -m pip install -e ".[dev,gui]"` y abrir con `suite-electrica`.

Pruebas: `python -m pytest -q`.

<details>
<summary><b>Línea de comandos: TopoCraft</b></summary>

Entrada: CSV `punto,este,norte,cota,descripcion` (UTM WGS84, m) —también `;`,
coma decimal o sin encabezado (orden PENZD o PNEZD, autodetectado)— o DXF con POINT / polilíneas 3D.
También importa **KML** (Google Earth), **GPX** (GPS), **TXT de MapSource** y
**Excel RLT** formato UTM (puntos + rutas como eje), y perfiles tabulares
**TPG** / Excel *ACUMULADA/ANGULO* (en `proyecto.json`: `"perfil": {"tabla": "ruta.tpg"}`).

```bash
topocraft validar puntos.csv [--out limpio.csv]
topocraft curvas  puntos.csv --eq 1 [--mayor-cada 5] [--eje eje.dxf] --out plano.dxf
topocraft perfil  puntos.csv --eje eje.dxf [--paso 20] [--esc-h 2000 --esc-v 200] \
                  --out perfil.pdf perfil.xlsx perfil.dxf
```

Opciones comunes: `--quitar-atipicos`, `--max-lado M` (descarta triángulos largos).

Capas DXF: `PUNTOS`, `TIN`, `CURVAS_MAYORES`, `CURVAS_MENORES`, `EJE`, `PERFIL`.
Unidades del dibujo: metros.

Ejemplos:

```bash
python examples/generar_ejemplos.py
python -m suite_electrica.topocraft curvas examples/colina_gaussiana.csv --eq 1 --eje examples/eje.dxf --out examples/salidas/plano.dxf
python -m suite_electrica.topocraft perfil examples/colina_gaussiana.csv --eje examples/eje.dxf --out examples/salidas/perfil.pdf examples/salidas/perfil.xlsx
```
</details>

<details>
<summary><b>Línea de comandos: LineCraft</b></summary>

Proyecto en JSON (ver `examples/linecraft/proyecto.json`): perfil (puntos + eje, o
Excel de TopoCraft), conductor del catálogo, hipótesis, condición inicial (EDS),
límites y fuentes de cada valor normativo.

```bash
linecraft nuevo mi_linea                      # plantilla de proyecto
linecraft calcular mi_linea/proyecto.json --out mi_linea/salidas
python -m suite_electrica.linecraft calcular examples/linecraft/proyecto.json --out examples/salidas/linecraft
```

Salidas: `perfil_linea.dxf` (capas PERFIL, POSTES, CONDUCTOR, DESPEJE, TEXTO_LC),
`planilla_estructuras.xlsx` (Estructuras, Vanos, Mecanico, Cargas, Avisos) y
`memoria_calculo.pdf`. Código de salida 3 si el diseño no cumple.
</details>

<details>
<summary><b>Línea de comandos: GridCraft</b></summary>

Red radial en `nodos.csv` (padre → nodo) + catálogo de conductores + `red.json`.

```bash
gridcraft nuevo mi_red
gridcraft calcular mi_red/red.json --out mi_red/salidas
python -m suite_electrica.gridcraft calcular examples/gridcraft/red.json --out examples/salidas/gridcraft
```

Salidas: `caida_tension.xlsx` (Resumen, CaidaTension, Alertas, Avisos) y
`plano_red.dxf` (capas RED, NODOS, TEXTO_RED, ALERTAS_RED). Código 3 si no cumple.
</details>

<details>
<summary><b>Línea de comandos: PresuCraft</b></summary>

```bash
presucraft nuevo mi_presupuesto
presucraft metrado mi_linea.sel --rubro LP --out mi_presupuesto/metrado.csv
presucraft calcular mi_presupuesto/presupuesto.json --out mi_presupuesto/salidas
```
</details>

### Estructura del repositorio

```
motor/            versión Rust: app egui, cálculo, pruebas y comparadores
suite_electrica/  versión Python (referencia congelada en python-referencia-v1)
  core/  topocraft/  linecraft/  gridcraft/  presucraft/  gui/
data/             catálogos CSV        examples/  datos sintéticos
tests/            pytest               docs/      fundamentos de cálculo
```

### Contribuir

1. `git pull` antes de editar; commits cortos en español.
2. Ningún valor normativo sin fuente; lo que no la tiene se marca `PENDIENTE` o `EJEMPLO`.
3. Cada función nueva deja una prueba; `cargo test --release` debe pasar antes de subir.
4. No se copian datos ni código de software comercial (sus reportes solo se usan en local para comparar).

## ⚖️ Aviso y licencia

TOPOMAGIC, REDLIN, REDCAD y PRESIO son marcas de sus dueños. Suite Eléctrica es una
implementación propia a partir de la normativa pública y la documentación de usuario: no copia
datos ni código de esos programas. Sus reportes solo se usan en local para comparar resultados.

Normativa base: Código Nacional de Electricidad (Suministro 2011), normas DGE/MINEM.
No se inventan valores normativos (ver [`data/README.md`](data/README.md)).

MIT © Renzo Mamani. CADCraft se usa solo como visor externo de DXF.
