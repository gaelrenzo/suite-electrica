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


if __name__ == "__main__":
    t = comparar(leer(sys.argv[1]))
    for nombre in ("vv", "f", "L", "vp"):
        m = t[[f"{nombre}_R", f"{nombre}_LC"]].dropna()
        e = (m.iloc[:, 1] - m.iloc[:, 0]).abs()
        print(f"{nombre:3s} n={len(m):4d}  error máx. {e.max():8.4f} m  mediana {e.median():.4f} m")
    malos = t[(t.vv_LC - t.vv_R).abs() > 0.01]
    if len(malos):
        print("Vano viento distinto (inicio de derivación: REDLIN excluye el vano de enlace):")
        print(malos[["der", "n", "vv_R", "vv_LC"]].to_string(index=False))
