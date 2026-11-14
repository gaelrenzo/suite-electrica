"""Proyecto .sel hecho en Python (tests/archivos/proyecto_python.sel) y verificación cruzada.

1. ``python motor/generar_referencia_sel.py``: crea el .sel con puntos (uno sin cota), eje,
   estructuras editadas, conductor con curvas de creep, hipótesis por altitud y catálogos.
2. ``cargo test --test proyectos`` (Rust) lo abre y lo guarda en
   ``motor/target/tmp/proyecto_desde_rust.sel``.
3. ``python motor/generar_referencia_sel.py --verificar``: Python abre el archivo guardado por
   Rust y comprueba que ``a_dict()`` sea idéntico al original.
"""

import math
import sys
from dataclasses import replace
from pathlib import Path

import numpy as np

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))

from suite_electrica.documento import Documento  # noqa: E402

ORIGEN = Path(__file__).parent / "tests" / "archivos" / "proyecto_python.sel"
RUST = Path(__file__).parent / "target" / "tmp" / "proyecto_desde_rust.sel"


def crear() -> None:
    d = Documento()
    d.nombre, d.autor = "Línea de prueba Ñuñoa", "R.G.M.G."
    d.importar_puntos(RAIZ / "examples" / "colina_gaussiana.csv")
    d.puntos.loc[3, "cota"] = math.nan  # campo vacío
    d.importar_eje(RAIZ / "examples" / "eje.dxf")
    d.calcular()
    d.cambiar_estructura(1, "armado", "PA1-3L")
    d.conductor = replace(d.conductor, curva_inicial=(0.05, 50.0, 30.0, -80.0), curva_creep=(0.0, 40.0))
    d.linea.estado_final_vanos = "por vano"
    d.linea.eds_max_pct = 18.5
    d.hipotesis.loc[0, "altitud_max_m"] = 4000.0
    d.fuentes["eds_max_pct"] = "prueba de ida y vuelta"
    d.catalogos.postes.loc[0, "diam_punta_m"] = 0.16
    d.guardar(ORIGEN)
    print("creado", ORIGEN)


def verificar() -> None:
    a, b = Documento.abrir(ORIGEN).a_dict(), Documento.abrir(RUST).a_dict()
    assert a == b or _iguales(a, b), "Python ve datos distintos en el .sel guardado por Rust"
    print("OK: Python abre el .sel guardado por Rust con los mismos datos")


def _iguales(x, y) -> bool:
    if isinstance(x, dict):
        return x.keys() == y.keys() and all(_iguales(x[k], y[k]) for k in x)
    if isinstance(x, list):
        return len(x) == len(y) and all(_iguales(p, q) for p, q in zip(x, y))
    if isinstance(x, float) and isinstance(y, float):
        return (np.isnan(x) and np.isnan(y)) or x == y
    return x == y


if __name__ == "__main__":
    verificar() if "--verificar" in sys.argv else crear()
