"""CLI de GridCraft: ``gridcraft calcular red.json --out salidas`` y ``gridcraft nuevo carpeta``."""

from __future__ import annotations

import argparse
import shutil
import sys
from collections.abc import Sequence
from importlib.resources import files
from pathlib import Path

import pandas as pd

from suite_electrica.core.dxf.escritura import asegurar_capa, guardar, nuevo_documento
from suite_electrica.gridcraft.red import COLS_NODOS, ResultadoRed, calcular, cargar

PLANTILLAS = {"red.json": "red_proyecto.json", "nodos.csv": "red_nodos.csv",
              "conductores_bt.csv": "conductores_bt.csv"}


def planilla_excel(ruta: str | Path, r: ResultadoRed) -> Path:
    """Hojas Resumen, CaidaTension, Alertas y Avisos."""
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    with pd.ExcelWriter(ruta) as xw:
        pd.DataFrame({"Parámetro": list(r.resumen), "Valor": list(r.resumen.values())}).to_excel(
            xw, sheet_name="Resumen", index=False)
        r.tramos.to_excel(xw, sheet_name="CaidaTension", index=False)
        (r.alertas if len(r.alertas) else pd.DataFrame({"Alerta": ["Sin alertas"]})).to_excel(
            xw, sheet_name="Alertas", index=False)
        pd.DataFrame({"Aviso": r.avisos or ["Sin avisos"]}).to_excel(xw, sheet_name="Avisos", index=False)
    return ruta


def plano_dxf(ruta: str | Path, r: ResultadoRed, alto_texto: float = 2.0) -> Path | None:
    """Plano de la red (capas RED, NODOS, TEXTO_RED) si los nodos tienen este/norte."""
    n = r.red.nodos.reindex(columns=COLS_NODOS)
    xy = n[["este", "norte"]].apply(pd.to_numeric, errors="coerce")
    if xy.isna().any().any():
        return None
    pos = dict(zip(n["nodo"].astype(str), xy.itertuples(index=False, name=None)))
    doc = nuevo_documento()
    for capa, color in (("RED", 1), ("NODOS", 3), ("TEXTO_RED", 7), ("ALERTAS_RED", 1)):
        asegurar_capa(doc, capa, color)
    msp = doc.modelspace()
    malos = set(r.alertas["Elemento"])
    for t in r.tramos.to_dict("records"):
        a, b = pos[str(t["Padre"])], pos[str(t["Nodo"])]
        msp.add_line(a, b, dxfattribs={"layer": "RED"})
        m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
        msp.add_text(f"{t['Formación']} {t['Long. (m)']:.0f}m", height=alto_texto * 0.7,
                     dxfattribs={"layer": "TEXTO_RED"}).set_placement(m)
    for nodo, (x, y) in pos.items():
        msp.add_circle((x, y), alto_texto * 0.6, dxfattribs={"layer": "ALERTAS_RED" if nodo in malos else "NODOS"})
        fila = r.tramos[r.tramos["Nodo"] == nodo]
        txt = nodo + (f" ({fila['%ΔV'].iloc[0]:.2f}%)" if len(fila) else "")
        msp.add_text(txt, height=alto_texto, dxfattribs={"layer": "TEXTO_RED"}).set_placement(
            (x + alto_texto, y + alto_texto))
    return guardar(doc, ruta)


def cmd_calcular(args: argparse.Namespace) -> int:
    r = calcular(cargar(args.proyecto))
    out = Path(args.out)
    print(f"{r.red.nombre}: {len(r.tramos)} tramos, sistema {r.red.sistema} {r.red.tension_V:g} V")
    for k, v in r.resumen.items():
        print(f"  {k}: {v}")
    print(f"  Resultado: {'CUMPLE' if r.cumple else 'NO CUMPLE'}")
    for a in r.alertas.to_dict("records"):
        print(f"  ALERTA {a['Código']} {a['Elemento']}: {a['Exceso (%)']} % — {a['Detalle']}")
    for a in r.avisos:
        print(f"  AVISO: {a}")
    print(planilla_excel(out / "caida_tension.xlsx", r))
    if (d := plano_dxf(out / "plano_red.dxf", r)) is not None:
        print(d)
    return 0 if r.cumple else 3


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
    print(f"Edite nodos.csv, conductores_bt.csv y red.json (valores y fuentes) y ejecute:\n"
          f"  gridcraft calcular {destino / 'red.json'} --out {destino / 'salidas'}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """Punto de entrada. Código 3 si la red no cumple."""
    ap = argparse.ArgumentParser(prog="gridcraft", description="GridCraft: redes de distribución MT/BT")
    sub = ap.add_subparsers(dest="comando", required=True)
    c = sub.add_parser("calcular", help="demandas, caída de tensión y pérdidas")
    c.add_argument("proyecto", help="red .json")
    c.add_argument("--out", default="salidas")
    c.set_defaults(func=cmd_calcular)
    n = sub.add_parser("nuevo", help="crear carpeta con plantillas")
    n.add_argument("carpeta")
    n.set_defaults(func=cmd_nuevo)
    args = ap.parse_args(argv)
    try:
        return args.func(args)
    except (ValueError, KeyError, OSError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
