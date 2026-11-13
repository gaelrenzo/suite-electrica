# Suite Eléctrica

Suite libre (MIT) para ingeniería eléctrica de distribución en Perú.

| Módulo | Alcance | Estado |
|---|---|---|
| **TopoCraft** | Topografía: limpieza de puntos, TIN, curvas de nivel, perfil longitudinal | Fase 1 — MVP |
| **LineCraft** | Líneas primarias MT: postes, cálculo mecánico, despejes, cargas y retenidas | Fase 2 — MVP |
| **GridCraft** | Redes primarias/secundarias: cargas, caída de tensión | Fase 3 — pendiente |
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

## Estructura

```
suite_electrica/
  core/      motor compartido (geo: UTM, TIN, perfil; dxf; norms; units)
  topocraft/ linecraft/ gridcraft/ gui/
data/      catálogos CSV     examples/  datos sintéticos
tests/     pytest            docs/      fundamentos de cálculo
```

## Licencia

MIT © Renzo Mamani. CADCraft se usa solo como visor externo de DXF.
