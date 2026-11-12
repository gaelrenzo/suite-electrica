# Suite Eléctrica

Suite libre (MIT) para ingeniería eléctrica de distribución en Perú.

| Módulo | Alcance | Estado |
|---|---|---|
| **TopoCraft** | Topografía: limpieza de puntos, TIN, curvas de nivel, perfil longitudinal | Fase 1 — MVP |
| **LineCraft** | Líneas primarias MT: postes, cálculo mecánico, distancias CNE | Fase 2 — pendiente |
| **GridCraft** | Redes primarias/secundarias: cargas, caída de tensión | Fase 3 — pendiente |
| GUI | PySide6, lanzador común, abrir DXF en CADCraft | Fase 4 — pendiente |

Normativa base: Código Nacional de Electricidad (Suministro 2011), normas DGE/MINEM.
No se inventan valores normativos (ver `data/README.md`).

## Instalación

Requiere Python 3.12+.

```bash
python -m pip install -e ".[dev]"     # añadir ,gui para PySide6
python -m pytest -q
```

Sin instalar: `python -m topocraft ...` desde la raíz del repositorio.

## TopoCraft

Entrada: CSV `punto,este,norte,cota,descripcion` (UTM WGS84, m) —también `;`,
coma decimal o sin encabezado (orden PENZD)— o DXF con POINT / polilíneas 3D.

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
python -m topocraft curvas examples/colina_gaussiana.csv --eq 1 --eje examples/eje.dxf --out examples/salidas/plano.dxf
python -m topocraft perfil examples/colina_gaussiana.csv --eje examples/eje.dxf --out examples/salidas/perfil.pdf examples/salidas/perfil.xlsx
```

Fórmulas y fuentes: [`docs/topocraft.md`](docs/topocraft.md).

## Estructura

```
core/      motor compartido (geo: UTM, TIN, perfil; dxf; norms; units)
topocraft/ linecraft/ gridcraft/ gui/
data/      catálogos CSV     examples/  datos sintéticos
tests/     pytest            docs/      fundamentos de cálculo
```

## Licencia

MIT © Renzo Mamani. CADCraft se usa solo como visor externo de DXF.
