# Suite Eléctrica

Suite libre (MIT) para ingeniería eléctrica de distribución en Perú.

| Módulo | Alcance | Estado |
|---|---|---|
| **TopoCraft** | Topografía: limpieza de puntos, TIN, curvas de nivel, perfil longitudinal | Fase 1 — MVP |
| **LineCraft** | Líneas primarias MT: postes, cálculo mecánico, despejes, cargas y retenidas | Fase 2 — MVP |
| **GridCraft** | Redes primarias/secundarias: demandas, caída de tensión, pérdidas, transformador | Fase 3 — MVP |
| **PresuCraft** | Presupuesto por unidades constructivas: suministro, montaje, transporte, GG, utilidad, IGV | MVP |
| **App** | PySide6: proyecto `.sel`, perfil interactivo, formularios, catálogos, metrado, CADCraft | Fase 4 — MVP |

Normativa base: Código Nacional de Electricidad (Suministro 2011), normas DGE/MINEM.
No se inventan valores normativos (ver `data/README.md`).

## Instalación

* **Windows 11**: instalar Python 3.11+ (con "Add to PATH") y ejecutar `instalar.bat`;
  crea el acceso directo **Suite Electrica** (app) y los comandos.
* **Linux**: `python3 -m pip install -e ".[dev,gui]"` y abrir con `suite-electrica`.

Paso a paso y flujo de trabajo: [`docs/GUIA_RAPIDA.md`](docs/GUIA_RAPIDA.md).
Pruebas: `python -m pytest -q`.

## TopoCraft

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

### Ejemplos

```bash
python examples/generar_ejemplos.py
python -m suite_electrica.topocraft curvas examples/colina_gaussiana.csv --eq 1 --eje examples/eje.dxf --out examples/salidas/plano.dxf
python -m suite_electrica.topocraft perfil examples/colina_gaussiana.csv --eje examples/eje.dxf --out examples/salidas/perfil.pdf examples/salidas/perfil.xlsx
```

Fórmulas y fuentes: [`docs/topocraft.md`](docs/topocraft.md).

## LineCraft

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

Fórmulas y fuentes: [`docs/linecraft.md`](docs/linecraft.md).

## GridCraft

Red radial en `nodos.csv` (padre → nodo) + catálogo de conductores + `red.json`.

```bash
gridcraft nuevo mi_red
gridcraft calcular mi_red/red.json --out mi_red/salidas
python -m suite_electrica.gridcraft calcular examples/gridcraft/red.json --out examples/salidas/gridcraft
```

Salidas: `caida_tension.xlsx` (Resumen, CaidaTension, Alertas, Avisos) y
`plano_red.dxf` (capas RED, NODOS, TEXTO_RED, ALERTAS_RED). Código 3 si no cumple.
Fórmulas: [`docs/gridcraft.md`](docs/gridcraft.md).

## PresuCraft

```bash
presucraft nuevo mi_presupuesto
presucraft metrado mi_linea.sel --rubro LP --out mi_presupuesto/metrado.csv
presucraft calcular mi_presupuesto/presupuesto.json --out mi_presupuesto/salidas
```

Fórmulas: [`docs/presucraft.md`](docs/presucraft.md).

## Equivalencias con software comercial

| Comercial | Suite Eléctrica |
|---|---|
| TOPOMAGIC | TopoCraft |
| REDLIN | LineCraft |
| REDCAD | GridCraft |
| PRESIO | PresuCraft |

Implementación propia a partir de la normativa pública y la documentación de
usuario; no se copian datos ni código de esos programas.

## Estructura

```
suite_electrica/
  core/      motor compartido (geo: UTM, TIN, perfil; dxf; norms; units)
  topocraft/ linecraft/ gridcraft/ presucraft/ gui/
data/      catálogos CSV     examples/  datos sintéticos
tests/     pytest            docs/      fundamentos de cálculo
```

## Migración a Rust

La suite se está reescribiendo en Rust (carpeta `motor/`) para distribuirla como un solo`n`ejecutable; cada módulo se valida contra la versión Python. Plan y estado: [`docs/PLAN_RUST.md`](docs/PLAN_RUST.md).

## Licencia

MIT © Renzo Mamani. CADCraft se usa solo como visor externo de DXF.
