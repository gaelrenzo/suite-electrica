"""Salidas de referencia de Python (Excel, DXF, PDF) -> motor/tests/salidas_python/.

Casos (las mismas entradas que usa ``cargo test --test salidas``):

* ``linea_auto``: ``examples/linecraft/proyecto.json`` con armados/postes asignados (catálogo
  de prueba de ``generar_referencia_catalogos``), verificación, metrado y alertas (``CFG``).
* ``linea_manual``: ``proyecto_manual.json`` como lo exporta la CLI (sin catálogos).
* ``topo``: colina sintética de ``generar_referencia_tin`` (puntos irregulares, semilla 7):
  plano con TIN y curvas cada 2,5 m (mayor cada 5), perfil DXF/Excel/PDF.
* ``red``: ``examples/gridcraft/red.json`` (planilla y plano).
* ``presu``: ``examples/presucraft/presupuesto.json``.

Luego ``python motor/comparar_salidas.py`` compara estos archivos con los de Rust.
"""

import io
import sys
from pathlib import Path

import numpy as np
import pandas as pd

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))
sys.path.insert(0, str(Path(__file__).parent))

from generar_referencia_1e import CFG  # noqa: E402
from generar_referencia_catalogos import POSTES  # noqa: E402
from suite_electrica.core.geo.perfil import perfil_longitudinal  # noqa: E402
from suite_electrica.core.geo.tin import TIN, niveles  # noqa: E402
from suite_electrica.gridcraft import cli as gcli  # noqa: E402
from suite_electrica.gridcraft import red as gred  # noqa: E402
from suite_electrica.linecraft import catalogos as cm  # noqa: E402
from suite_electrica.linecraft import proyecto as pr  # noqa: E402
from suite_electrica.linecraft import salidas as ls  # noqa: E402
from suite_electrica.linecraft.alertas import ConfigAlertas, alertas  # noqa: E402
from suite_electrica.linecraft.calculo import calcular  # noqa: E402
from suite_electrica.presucraft import presupuesto as pp  # noqa: E402
from suite_electrica.topocraft import salidas as ts  # noqa: E402
from suite_electrica.topocraft import sinteticos as st  # noqa: E402

EJ = RAIZ / "examples"
OUT = Path(__file__).parent / "tests" / "salidas_python"


def puntos_topo() -> tuple[np.ndarray, np.ndarray]:
    """Mismos puntos y eje que ``generar_referencia_tin`` (semilla 7)."""
    rng = np.random.default_rng(7)
    sup = st.colina_gaussiana()
    df = st.malla(sup, 20.0)
    xy = df[["este", "norte"]].to_numpy() + rng.uniform(-6, 6, (len(df), 2))
    z = sup(xy[:, 0], xy[:, 1]) if callable(sup) else np.array([sup.cota(*p) for p in xy])
    eje = np.array([[xy[:, 0].min() + 40, xy[:, 1].min() + 50], [xy[:, 0].mean(), xy[:, 1].mean() + 13],
                    [xy[:, 0].max() - 35, xy[:, 1].max() - 60]])
    return np.column_stack([xy, z]), eje


def linea(nombre: str, json: str, catalogos: bool) -> None:
    p = pr.cargar(EJ / "linecraft" / json)
    r = calcular(p)
    if catalogos:
        cat = cm.Catalogos.plantilla()
        cat.postes = cm.normalizar("postes", pd.read_csv(io.StringIO(POSTES)))
        fm = cm._fuerzas_maximas(r)
        for e in r.estructuras:
            e.armado = cat.elegir_armado(e.tipo, e.angulo)
            fh, mb, _ = fm.get(e.numero, (0.0, 0.0, 0.0))
            e.poste = cm.elegir_poste(cat, e, fh, mb, 2.0)
        r.estructuras[2].poste = "M 12/C6"
        r.verificacion = cm.verificar(r, cat, 2.0, 2.0)
        r.metrado = cm.metrado(r, cat, 3.0)
        r.alertas = alertas(r, cat, CFG)
    else:
        r.alertas = alertas(r, None, ConfigAlertas(**p.alertas))
    ls.planilla_excel(OUT / f"{nombre}_planilla.xlsx", r)
    ls.perfil_dxf(OUT / f"{nombre}_perfil.dxf", r, 10.0)
    ls.memoria_pdf(OUT / f"{nombre}_memoria.pdf", r)


def topo() -> None:
    xyz, eje = puntos_topo()
    tin = TIN(xyz)
    curvas = tin.curvas_nivel(niveles(xyz[:, 2].min(), xyz[:, 2].max(), 2.5))
    pts = pd.DataFrame({"punto": [str(i + 1) for i in range(len(xyz))], "este": xyz[:, 0], "norte": xyz[:, 1],
                        "cota": xyz[:, 2], "descripcion": ""})
    ts.plano_dxf(OUT / "topo_plano.dxf", pts, tin, curvas, 2.5, 5, eje, 1.0)
    pf = perfil_longitudinal(tin, eje, 20.0)
    ts.perfil_dxf(OUT / "topo_perfil.dxf", pf, 10.0)
    ts.perfil_excel(OUT / "topo_perfil.xlsx", pf)
    ts.perfil_pdf(OUT / "topo_perfil.pdf", pf)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    linea("linea_auto", "proyecto.json", True)
    linea("linea_manual", "proyecto_manual.json", False)
    topo()
    red = gred.cargar(EJ / "gridcraft" / "red.json")
    rr = gred.calcular(red)
    gcli.planilla_excel(OUT / "red_planilla.xlsx", rr)
    gcli.plano_dxf(OUT / "red_plano.dxf", rr)
    pp.excel(OUT / "presu.xlsx", pp.calcular(pp.cargar(EJ / "presucraft" / "presupuesto.json")))
    print("\n".join(sorted(f.name for f in OUT.iterdir())))


if __name__ == "__main__":
    main()
