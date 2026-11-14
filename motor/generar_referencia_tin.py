"""Referencia del TIN, curvas de nivel y perfil (Python/qhull) -> motor/tests/tin.txt.

Puntos de la colina sintética con desplazamiento aleatorio (semilla fija) para que la
triangulación de Delaunay sea única (sin cuatro puntos cocirculares).
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from suite_electrica.core.geo.perfil import perfil_longitudinal  # noqa: E402
from suite_electrica.core.geo.tin import TIN, niveles  # noqa: E402
from suite_electrica.topocraft import sinteticos as st  # noqa: E402


def f(x: float) -> str:
    return "nan" if x != x else f"{x:.17e}"


def main() -> None:
    rng = np.random.default_rng(7)
    sup = st.colina_gaussiana()
    df = st.malla(sup, 20.0)
    xy = df[["este", "norte"]].to_numpy() + rng.uniform(-6, 6, (len(df), 2))
    z = sup(xy[:, 0], xy[:, 1]) if callable(sup) else np.array([sup.cota(*p) for p in xy])
    xyz = np.column_stack([xy, z])
    L = [f"punto {f(a)} {f(b)} {f(c)}" for a, b, c in xyz]
    for max_lado in (None, 35.0):
        tin = TIN(xyz, max_lado=max_lado)
        tag = "sin" if max_lado is None else "max35"
        L.append(f"tin {tag} {len(tin.triangulos)} {int(tin.activos.sum())} {len(tin.aristas)}")
        q = np.column_stack([rng.uniform(xy[:, 0].min() - 30, xy[:, 0].max() + 30, 300),
                             rng.uniform(xy[:, 1].min() - 30, xy[:, 1].max() + 30, 300)])
        for (a, b), zz in zip(q, tin.cota(q[:, 0], q[:, 1])):
            L.append(f"cota {tag} {f(a)} {f(b)} {f(zz)}")
        for nivel in niveles(z.min(), z.max(), 2.5):
            cs = tin.curvas_nivel([nivel])
            L.append(f"curvas {tag} {f(nivel)} {len(cs)} {f(sum(c.longitud for c in cs))} "
                     f"{sum(c.cerrada for c in cs)}")
        eje = np.array([[xy[:, 0].min() + 40, xy[:, 1].min() + 50], [xy[:, 0].mean(), xy[:, 1].mean() + 13],
                        [xy[:, 0].max() - 35, xy[:, 1].max() - 60]])
        pf = perfil_longitudinal(tin, eje, 20.0)
        L.append(f"eje {tag} " + " ".join(f"{f(a)},{f(b)}" for a, b in eje))
        for s, c, t in zip(pf.progresiva, pf.cota, pf.tipo):
            L.append(f"perfil {tag} {f(s)} {f(c)} {t}")
    (Path(__file__).parent / "tests" / "tin.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(len(L), "líneas")


if __name__ == "__main__":
    main()
