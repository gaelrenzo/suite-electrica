"""Compara la catenaria de LineCraft con la "Planilla de Estructuras" exportada por REDLIN.

Uso (requiere REDLIN para generar el .xls; no se distribuyen datos de REDLIN)::

    REDLIN: Reportes > Reportar todas las derivaciones > Planilla de Estructuras -> PlanillaEstTotal.xls
    python examples/validacion/comparar_redlin.py PlanillaEstTotal.xls

Con el parámetro de catenaria C, el vano y el desnivel de amarres (despejado de la
longitud de catenaria que reporta REDLIN) recalcula flecha a medio vano, longitud,
vano peso y vano viento, e imprime el error máximo de cada uno.
"""

import math
import sys
from pathlib import Path

import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from suite_electrica.linecraft.mecanica import Catenaria  # noqa: E402

COLS = {1: "der", 2: "n", 10: "desn", 14: "vad", 18: "vp", 19: "vv", 50: "C", 51: "L", 52: "f"}


def leer(ruta: str) -> pd.DataFrame:
    crudo = pd.read_excel(ruta, header=None).iloc[2:]
    d = pd.DataFrame({k: crudo[i] for i, k in COLS.items()})
    for c in d.columns[1:]:
        d[c] = pd.to_numeric(d[c], errors="coerce")
    return d


def comparar(d: pd.DataFrame) -> pd.DataFrame:
    filas = []
    for der, g in d.groupby("der", sort=False):
        g = g.sort_values("n").reset_index(drop=True)
        ultimo = len(g) - 1
        h = []  # desnivel de amarres despejado de L = sqrt(h² + (2C sinh(a/2C))²)
        for i, r in g.iterrows():
            ok = i < ultimo and r.vad > 0 and r.C > 0
            s = 2 * r.C * math.sinh(r.vad / (2 * r.C)) if ok else 0
            h.append(math.copysign(math.sqrt(max(r.L**2 - s**2, 0)), r.desn) if ok else math.nan)
        for i, r in g.iterrows():
            f = {"der": der, "n": r.n, "vv_R": r.vv,
                 "vv_LC": ((g.vad[i - 1] if i else 0) + (r.vad if i < ultimo else 0)) / 2}
            if not math.isnan(h[i]):
                c = Catenaria(r.vad, h[i], r.C, 1.0)
                f.update(f_R=r.f, f_LC=h[i] / 2 - float(c.y(r.vad / 2)), L_R=r.L, L_LC=c.longitud)
                if i and not math.isnan(h[i - 1]):
                    ca = Catenaria(g.vad[i - 1], h[i - 1], g.C[i - 1], 1.0)
                    f.update(vp_R=r.vp, vp_LC=g.vad[i - 1] - ca.x_vertice + c.x_vertice)
            filas.append(f)
    return pd.DataFrame(filas)


def conductores(ruta_suministros: str) -> dict:
    """Conductores de la hoja Conductor de Suministros.XLS (kg, mm, kg/mm², 1/°C) en SI."""
    from suite_electrica.linecraft.conductor import G, Conductor
    c = pd.read_excel(ruta_suministros, "Conductor", header=None).iloc[4:]
    out = {}
    for r in c.itertuples(index=False):
        try:
            out[str(r[1])] = Conductor(str(r[1]), float(r[8]) * 1e-6, float(r[7]) * 1e-3, float(r[5]) * G,
                                       float(r[6]) * G, float(r[12]) * G * 1e6, float(r[11]))
        except (TypeError, ValueError):
            pass
    return out


def comparar_flechado(ruta_tabla: str, ruta_suministros: str) -> pd.DataFrame:
    """Tiro H por temperatura de la Tabla de Flechado vs ``cambio_estado`` (desde 25 °C, sin viento).

    En tramos de un vano se pasa el desnivel (forma exacta); en tramos de varios vanos, el vano regulador.
    """
    from suite_electrica.linecraft.conductor import G
    from suite_electrica.linecraft.mecanica import cambio_estado
    cond = conductores(ruta_suministros)
    df = pd.read_excel(ruta_tabla, header=None)
    temps = [float(t) for t in df.iloc[1, 11:] if pd.notna(t)]
    t_ref = float(str(df.iat[0, 0]).split("Temperatura:")[1].split("°")[0].split("░")[0])
    filas = []
    for i in range(2, len(df) - 2):
        r = df.iloc[i]
        if not str(r[10]).startswith("Flecha") or r[8] not in cond:
            continue
        c, ar, a, h = cond[r[8]], float(r[4]), float(r[5]), float(r[6])
        H = {t: float(df.iat[i + 2, 11 + k]) * G for k, t in enumerate(temps)}
        for t in temps:
            hl = cambio_estado(H[t_ref] / c.seccion, c.peso, t_ref, c.peso, t, ar, c,
                               h if a == ar else 0.0) * c.seccion
            filas.append({"conductor": c.nombre, "a_r": ar, "T": t, "H_R": H[t] / G, "H_LC": hl / G})
    return pd.DataFrame(filas)


if __name__ == "__main__":
    if len(sys.argv) == 4:  # comparar_redlin.py TablaFlechado.XLS Suministros.XLS --flechado
        f = comparar_flechado(sys.argv[1], sys.argv[2])
        e = 100 * (f.H_LC / f.H_R - 1)
        print(f"Tiro H vs temperatura: {len(f)} puntos  |error| mediana {e.abs().median():.3f} %  "
              f"p95 {e.abs().quantile(.95):.3f} %  máx {e.abs().max():.2f} %")
        for lim in (20, 50):
            m = f.a_r >= lim
            print(f"  vanos ≥ {lim} m: p95 {e[m].abs().quantile(.95):.3f} %  máx {e[m].abs().max():.3f} %")
        sys.exit()
    t = comparar(leer(sys.argv[1]))
    for nombre in ("vv", "f", "L", "vp"):
        m = t[[f"{nombre}_R", f"{nombre}_LC"]].dropna()
        e = (m.iloc[:, 1] - m.iloc[:, 0]).abs()
        print(f"{nombre:3s} n={len(m):4d}  error máx. {e.max():8.4f} m  mediana {e.median():.4f} m")
    malos = t[(t.vv_LC - t.vv_R).abs() > 0.01]
    if len(malos):
        print("Vano viento distinto (inicio de derivación: REDLIN excluye el vano de enlace):")
        print(malos[["der", "n", "vv_R", "vv_LC"]].to_string(index=False))
