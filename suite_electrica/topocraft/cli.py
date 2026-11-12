"""Interfaz de línea de comandos de TopoCraft.

Ejemplos::

    topocraft validar puntos.csv
    topocraft curvas puntos.csv --eq 1 --out plano.dxf
    topocraft perfil puntos.csv --eje eje.dxf --out perfil.pdf perfil.xlsx perfil.dxf
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

import numpy as np
import pandas as pd

from suite_electrica.core.dxf.lectura import leer_polilinea
from suite_electrica.core.geo.perfil import perfil_longitudinal
from suite_electrica.core.geo.tin import TIN, niveles
from suite_electrica.topocraft import datos, salidas


def _cargar(args: argparse.Namespace) -> tuple[pd.DataFrame, TIN]:
    df = datos.leer(args.puntos)
    df, rep = datos.limpiar(df, quitar_atipicos=args.quitar_atipicos)
    print(rep.resumen())
    if rep.atipicos and not args.quitar_atipicos:
        print("  (atípicos conservados; use --quitar-atipicos para eliminarlos)")
    return df, TIN(df[["este", "norte", "cota"]].to_numpy(), max_lado=args.max_lado)


def _leer_eje(ruta: str) -> np.ndarray:
    if Path(ruta).suffix.lower() == ".dxf":
        return leer_polilinea(ruta)
    e = datos.leer_csv(ruta)
    return e[["este", "norte"]].to_numpy()


def cmd_validar(args: argparse.Namespace) -> int:
    df = datos.leer(args.puntos)
    limpio, rep = datos.limpiar(df, quitar_atipicos=args.quitar_atipicos)
    print(rep.resumen())
    if args.out:
        datos.guardar_csv(limpio, args.out)
        print(f"Puntos limpios -> {args.out}")
    return 0


def cmd_curvas(args: argparse.Namespace) -> int:
    df, tin = _cargar(args)
    cotas = niveles(float(df["cota"].min()), float(df["cota"].max()), args.eq)
    curvas = tin.curvas_nivel(cotas)
    eje = _leer_eje(args.eje) if args.eje else None
    ruta = salidas.plano_dxf(args.out, df, None if args.sin_tin else tin, curvas,
                             args.eq, args.mayor_cada, eje, args.texto)
    print(f"{len(tin.triangulos_activos)} triángulos, {len(curvas)} curvas "
          f"(eq. {args.eq:g} m, mayor cada {args.mayor_cada}) -> {ruta}")
    return 0


def cmd_perfil(args: argparse.Namespace) -> int:
    df, tin = _cargar(args)
    eje = _leer_eje(args.eje)
    perfil = perfil_longitudinal(tin, eje, args.paso)
    n_fuera = int(np.isnan(perfil.cota).sum())
    print(f"Eje: {perfil.longitud:.2f} m, {len(perfil.progresiva)} puntos de perfil")
    if n_fuera:
        print(f"  AVISO: {n_fuera} puntos del eje fuera del TIN (cota vacía)")
    for out in args.out:
        ext = Path(out).suffix.lower()
        if ext == ".pdf":
            eh, ev = salidas.perfil_pdf(out, perfil, args.esc_h, args.esc_v)
            print(f"PDF (H 1:{eh}, V 1:{ev}) -> {out}")
        elif ext == ".xlsx":
            salidas.perfil_excel(out, perfil, {"Puntos": args.puntos, "Eje": args.eje,
                                               "Paso estaciones (m)": args.paso})
            print(f"Excel -> {out}")
        elif ext == ".dxf":
            salidas.perfil_dxf(out, perfil, args.exageracion)
            print(f"DXF -> {out}")
        else:
            print(f"Extensión no soportada: {out}", file=sys.stderr)
            return 2
    return 0


def construir_parser() -> argparse.ArgumentParser:
    """Parser de argumentos de ``topocraft``."""
    p = argparse.ArgumentParser(prog="topocraft", description="TopoCraft: topografía para redes eléctricas")
    sub = p.add_subparsers(dest="comando", required=True)

    def comunes(sp: argparse.ArgumentParser) -> None:
        sp.add_argument("puntos", help="CSV (punto,este,norte,cota,descripcion) o DXF")
        sp.add_argument("--quitar-atipicos", action="store_true", help="eliminar cotas atípicas")
        sp.add_argument("--max-lado", type=float, default=None, help="lado máximo de triángulo (m)")

    v = sub.add_parser("validar", help="validar y limpiar puntos")
    v.add_argument("puntos")
    v.add_argument("--quitar-atipicos", action="store_true")
    v.add_argument("--out", help="CSV de salida con puntos limpios")
    v.set_defaults(func=cmd_validar)

    c = sub.add_parser("curvas", help="TIN y curvas de nivel a DXF")
    comunes(c)
    c.add_argument("--eq", type=float, default=1.0, help="equidistancia (m)")
    c.add_argument("--mayor-cada", type=int, default=5, help="curva mayor cada N equidistancias")
    c.add_argument("--eje", help="eje (DXF o CSV) para dibujar en capa EJE")
    c.add_argument("--sin-tin", action="store_true", help="no dibujar triángulos")
    c.add_argument("--texto", type=float, default=1.0, help="altura de texto (m)")
    c.add_argument("--out", required=True, help="DXF de salida")
    c.set_defaults(func=cmd_curvas)

    f = sub.add_parser("perfil", help="perfil longitudinal a PDF/Excel/DXF")
    comunes(f)
    f.add_argument("--eje", required=True, help="eje (DXF con polilínea o CSV este,norte)")
    f.add_argument("--paso", type=float, default=20.0, help="paso de estaciones (m)")
    f.add_argument("--esc-h", type=int, default=None, help="escala horizontal 1:N")
    f.add_argument("--esc-v", type=int, default=None, help="escala vertical 1:N")
    f.add_argument("--exageracion", type=float, default=10.0, help="exageración vertical en DXF")
    f.add_argument("--out", nargs="+", required=True, help="salidas .pdf, .xlsx y/o .dxf")
    f.set_defaults(func=cmd_perfil)
    return p


def main(argv: Sequence[str] | None = None) -> int:
    """Punto de entrada de la CLI."""
    args = construir_parser().parse_args(argv)
    try:
        return args.func(args)
    except (ValueError, OSError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
