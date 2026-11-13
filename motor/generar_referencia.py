"""Genera motor/tests/referencia.txt con resultados de la versión Python (referencia del port a Rust).

Formato por línea: ``caso arg1 arg2 ... = res1 res2 ...`` (números en notación científica).
Conductores: AAAC 35 de la plantilla y una variante con curvas σ(ε) sintéticas para el creep.

Uso: ``python motor/generar_referencia.py``
"""

import itertools
import sys
from dataclasses import replace
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))

from suite_electrica.linecraft.conductor import G, Conductor, Hipotesis, carga_unitaria  # noqa: E402
from suite_electrica.linecraft.mecanica import (  # noqa: E402
    Catenaria, Parabola, _deformacion, cambio_estado, deformacion_creep, vano_regulador)

BASE = Conductor("AAAC 35", 34.36e-6, 7.5e-3, 0.0945 * G, 10.199e3, 58860e6, 23e-6)
# Curvas sintéticas (no de catálogo): solo para comprobar que Rust reproduce a Python.
CREEP = replace(BASE, curva_inicial=(0.05, 50.0, 30.0, -80.0, 0.0), curva_creep=(0.0, 40.0))
CONDS = {"base": BASE, "creep": CREEP}


def f(x: float) -> str:
    return f"{x:.17e}"


def main() -> None:
    lineas = []
    for nombre, c in CONDS.items():
        s1s = [p / 100 * c.carga_rotura / c.seccion for p in (2, 10, 18, 25)]
        for s1, (t1, t2), a, (w1f, w2f), h in itertools.product(
                s1s, [(25, 5), (15, 40), (10, -10)], [15.0, 60.0, 200.0, 650.0],
                [(1.0, 1.0), (1.0, 2.3)], [0.0, 12.0, -45.0]):
            w1, w2 = c.peso * w1f, c.peso * w2f
            r = cambio_estado(s1, w1, t1, w2, t2, a, c, h)
            lineas.append(f"cambio_estado {nombre} {f(s1)} {f(w1)} {t1} {f(w2)} {t2} {a} {h} = {f(r)}")
        if c.curva_creep:
            for p, a, h in itertools.product((10, 15, 18), (80.0, 300.0), (0.0, 20.0)):
                s1 = p / 100 * c.carga_rotura / c.seccion
                r = deformacion_creep(c, s1, c.peso, 25, a, h)
                lineas.append(f"deformacion_creep {nombre} {f(s1)} {f(c.peso)} 25 {a} {h} = {f(r)}")
            for s in (0.5, 3.0, 6.0, 12.0):
                lineas.append(f"deformacion_ini {nombre} {s} = {f(_deformacion(c.curva_inicial, s))}")
                lineas.append(f"deformacion_creep_curva {nombre} {s} = {f(_deformacion(c.curva_creep, s))}")
    for a, h, H, w in itertools.product((30.0, 150.0, 900.0), (0.0, 25.0, -140.0), (500.0, 2500.0), (0.9, 3.1)):
        for nom, cls in (("catenaria", Catenaria), ("parabola", Parabola)):
            cv = cls(a, h, H, w)
            vals = [cv.x_vertice, float(cv.y(a * 0.3)), cv.flecha, h / 2 - float(cv.y(a / 2)), cv.tiro_maximo]
            if nom == "catenaria":
                vals.append(cv.longitud)
            lineas.append(f"curva_{nom} {a} {h} {H} {w} = " + " ".join(map(f, vals)))
    for vs in ([100.0], [80.0, 120.0, 95.0], [10.0, 400.0]):
        lineas.append("vano_regulador " + " ".join(map(str, vs)) + f" = {f(vano_regulador(vs))}")
    for p, e, rho in ((0.0, 0.0, 0.0), (231.77, 0.0, 0.0), (57.92, 0.006, 900.0)):
        cu = carga_unitaria(BASE, Hipotesis("h", 0, p, e, rho))
        lineas.append(f"carga_unitaria {p} {e} {rho} = {f(cu.vertical)} {f(cu.viento)} {f(cu.resultante)}")
    salida = Path(__file__).parent / "tests" / "referencia.txt"
    salida.parent.mkdir(exist_ok=True)
    salida.write_text("\n".join(lineas) + "\n", encoding="utf-8")
    print(f"{len(lineas)} casos -> {salida}")


if __name__ == "__main__":
    main()
