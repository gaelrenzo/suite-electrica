"""CLI de LineCraft.

    linecraft nuevo mi_linea/
    linecraft calcular mi_linea/proyecto.json --out mi_linea/salidas
"""

from __future__ import annotations

import argparse
import shutil
import sys
from collections.abc import Sequence
from importlib.resources import files
from pathlib import Path

from suite_electrica.linecraft import proyecto, salidas
from suite_electrica.linecraft.calculo import calcular


def cmd_calcular(args: argparse.Namespace) -> int:
    if str(args.proyecto).lower().endswith(".sel"):
        from suite_electrica.documento import Documento
        doc = Documento.abrir(args.proyecto)
        r = doc.calcular()
        p = r.proyecto
        if doc.modificado:  # se ubicaron estructuras automáticamente
            doc.guardar()
    else:
        from suite_electrica.linecraft.alertas import ConfigAlertas, alertas
        p = proyecto.cargar(args.proyecto)
        r = calcular(p)
        r.alertas = alertas(r, None, ConfigAlertas(**p.alertas))
    out = Path(args.out)
    print(f"{p.nombre}: {len(r.estructuras)} estructuras, {len(r.vanos)} vanos, {len(r.tramos)} tramo(s)")
    for t in r.tramos:
        print(f"  Tramo {r.estructuras[t.inicio].codigo}-{r.estructuras[t.fin].codigo}: "
              f"vano regulador {t.vano_regulador:.2f} m")
    print(f"  Despeje mínimo: {min(v.despeje for v in r.vanos):.2f} m "
          f"(exigido {p.distancia_minima_terreno:.2f} m)")
    print(f"  Resultado: {'CUMPLE' if r.cumple else 'NO CUMPLE'}")
    for a in r.alertas.itertuples(index=False):
        print(f"  ALERTA {a[1]} {a[0]}: {a[2]} % — {a[3]}")
    for a in r.avisos:
        print(f"  AVISO: {a}")
    print(salidas.perfil_dxf(out / "perfil_linea.dxf", r, args.exageracion))
    print(salidas.planilla_excel(out / "planilla_estructuras.xlsx", r))
    print(salidas.memoria_pdf(out / "memoria_calculo.pdf", r))
    return 0 if r.cumple else 3


def crear_proyecto(carpeta: str | Path) -> tuple[list[Path], list[Path]]:
    """Copia las plantillas a ``carpeta`` sin sobrescribir.

    Returns:
        (creados, existentes)
    """
    destino = Path(carpeta)
    destino.mkdir(parents=True, exist_ok=True)
    plantillas = files("suite_electrica.plantillas")
    creados, existentes = [], []
    for nombre in ("proyecto.json", "conductores.csv", "estructuras.csv"):
        ruta = destino / nombre
        if ruta.exists():
            existentes.append(ruta)
            continue
        with plantillas.joinpath(nombre).open("rb") as src, open(ruta, "wb") as dst:
            shutil.copyfileobj(src, dst)
        creados.append(ruta)
    return creados, existentes


def cmd_nuevo(args: argparse.Namespace) -> int:
    destino = Path(args.carpeta)
    creados, existentes = crear_proyecto(destino)
    for r in existentes:
        print(f"Ya existe, no se sobrescribe: {r}")
    for r in creados:
        print(f"Creado: {r}")
    print("Siguiente: copie puntos.csv y eje.dxf a la carpeta, edite proyecto.json (valores y fuentes)\n"
          f"y ejecute: linecraft calcular {destino / 'proyecto.json'} --out {destino / 'salidas'}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """Punto de entrada de la CLI. Código 3 si el diseño no cumple."""
    ap = argparse.ArgumentParser(prog="linecraft", description="LineCraft: líneas primarias MT")
    sub = ap.add_subparsers(dest="comando", required=True)
    c = sub.add_parser("calcular", help="ubicar estructuras y calcular la línea")
    c.add_argument("proyecto", help="proyecto .sel (app) o .json (LineCraft)")
    c.add_argument("--out", default="salidas", help="carpeta de salida")
    c.add_argument("--exageracion", type=float, default=10.0, help="exageración vertical del DXF")
    c.set_defaults(func=cmd_calcular)
    n = sub.add_parser("nuevo", help="crear carpeta de proyecto con plantillas")
    n.add_argument("carpeta")
    n.set_defaults(func=cmd_nuevo)
    args = ap.parse_args(argv)
    try:
        return args.func(args)
    except (ValueError, KeyError, OSError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
