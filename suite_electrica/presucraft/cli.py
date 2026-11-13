"""CLI: ``presucraft calcular presupuesto.json``, ``presucraft nuevo carpeta``,
``presucraft metrado proyecto.sel --out metrado.csv`` (metrado de LineCraft)."""

from __future__ import annotations

import argparse
import shutil
import sys
from collections.abc import Sequence
from importlib.resources import files
from pathlib import Path

from suite_electrica.presucraft.presupuesto import calcular, cargar, excel, metrado_linecraft

PLANTILLAS = {"presupuesto.json": "presu_presupuesto.json", "suministros.csv": "presu_suministros.csv",
              "unidades.csv": "presu_unidades.csv", "montaje.csv": "presu_montaje.csv",
              "metrado.csv": "presu_metrado.csv"}


def cmd_calcular(args: argparse.Namespace) -> int:
    r = calcular(cargar(args.proyecto))
    m = r.presupuesto.moneda
    print(r.presupuesto.nombre)
    for f in r.resumen.to_dict("records"):
        print(f"  {f['Rubro']:>6}: suministro {m} {f['Suministro']:,.2f}  montaje {m} {f['Montaje']:,.2f}  "
              f"transporte {m} {f['Transporte']:,.2f}  TOTAL {m} {f['Total']:,.2f}")
    for a in r.avisos:
        print(f"  AVISO: {a}")
    print(excel(Path(args.out) / "presupuesto.xlsx", r))
    return 0


def cmd_nuevo(args: argparse.Namespace) -> int:
    destino = Path(args.carpeta)
    destino.mkdir(parents=True, exist_ok=True)
    base = files("suite_electrica.plantillas")
    for nombre, origen in PLANTILLAS.items():
        ruta = destino / nombre
        if ruta.exists():
            print(f"Ya existe, no se sobrescribe: {ruta}")
            continue
        with base.joinpath(origen).open("rb") as src, open(ruta, "wb") as dst:
            shutil.copyfileobj(src, dst)
        print(f"Creado: {ruta}")
    return 0


def cmd_metrado(args: argparse.Namespace) -> int:
    from suite_electrica.documento import Documento
    r = Documento.abrir(args.proyecto).calcular()
    df = metrado_linecraft(r.metrado, args.rubro)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    df.to_csv(out, index=False)
    print(f"{len(df)} UC -> {out}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="presucraft", description="PresuCraft: metrados y presupuestos")
    sub = ap.add_subparsers(dest="comando", required=True)
    c = sub.add_parser("calcular", help="presupuesto desde presupuesto.json")
    c.add_argument("proyecto")
    c.add_argument("--out", default="salidas")
    c.set_defaults(func=cmd_calcular)
    n = sub.add_parser("nuevo", help="crear carpeta con plantillas")
    n.add_argument("carpeta")
    n.set_defaults(func=cmd_nuevo)
    m = sub.add_parser("metrado", help="metrado de un proyecto LineCraft (.sel) a metrado.csv")
    m.add_argument("proyecto")
    m.add_argument("--rubro", default="LP")
    m.add_argument("--out", default="metrado.csv")
    m.set_defaults(func=cmd_metrado)
    args = ap.parse_args(argv)
    try:
        return args.func(args)
    except (ValueError, KeyError, OSError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
