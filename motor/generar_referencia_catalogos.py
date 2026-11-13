"""Referencia de asignación de armados/postes, verificación y metrado (Python -> tests/catalogos_auto.txt).

Usa el caso ``auto`` de LineCraft y la plantilla de catálogos, con geometría de poste añadida
para ejercitar el momento de vuelco (valores ilustrativos, no de catálogo).
"""

import io
import math
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))

from suite_electrica.linecraft import catalogos as cm  # noqa: E402
from suite_electrica.linecraft import proyecto as pr  # noqa: E402
from suite_electrica.linecraft.calculo import calcular  # noqa: E402

POSTES = """codigo,material,longitud_m,carga_nominal_daN,empotramiento_m,carga_vertical_max_daN,diam_punta_m,diam_tierra_m,mor_MPa,dist_aplicacion_m,fuente
CAC 12/300,Concreto,12,300,1.3,,0.16,0.34,,0.15,prueba
CAC 12/400,Concreto,12,400,1.3,,0.16,0.34,,0.15,prueba
CAC 13/400,Concreto,13,400,1.4,,0.16,0.355,,0.15,prueba
M 12/C6,Madera,12,,1.8,,0.137,0.229,55.2,,prueba
"""


def f(x) -> str:
    if x is None or (isinstance(x, float) and math.isnan(x)):
        return "-"
    return "inf" if x == math.inf else f"{x:.17e}"


def main() -> None:
    import pandas as pd
    p = pr.cargar(RAIZ / "examples" / "linecraft" / "proyecto.json")
    r = calcular(p)
    cat = cm.Catalogos.plantilla()
    cat.postes = cm.normalizar("postes", pd.read_csv(io.StringIO(POSTES)))
    fm = cm._fuerzas_maximas(r)
    for e in r.estructuras:  # mismo algoritmo que Documento.asignar_catalogos
        e.armado = cat.elegir_armado(e.tipo, e.angulo)
        fh, mb, _ = fm.get(e.numero, (0.0, 0.0, 0.0))
        e.poste = cm.elegir_poste(cat, e, fh, mb, 2.0)
    r.estructuras[2].poste = "M 12/C6"  # un poste de madera para el FS por MOR
    ver = cm.verificar(r, cat, 2.0, 2.0)
    met = cm.metrado(r, cat, 3.0)
    L = ["postes " + line for line in POSTES.strip().splitlines()]
    for e in r.estructuras:
        L.append(f"asig {e.armado} {e.poste.replace(' ', '_')}")
    L.append("== resultados")
    for v in ver.to_dict("records"):
        num = lambda k: f(v[k]) if isinstance(v[k], (int, float)) else "-"  # noqa: E731
        L.append(f"ver {v['Estructura']} {num('F horizontal máx. (N)')} {num('F punta equiv. (N)')} {num('MRN (N·m)')} "
                 f"{num('FS poste')} {v['Retenidas']} {num('Tiro por retenida (N)')} {num('FS retenida')} "
                 f"{v['Cumple']}")
    for m in met.to_dict("records"):
        L.append(f"met {m['Ítem']}|{m['Código']}|{m['Descripción']}|{m['Unidad']}|{f(float(m['Cantidad']))}")
    out = Path(__file__).parent / "tests" / "catalogos_auto.txt"
    out.write_text("\n".join(L) + "\n", encoding="utf-8")
    print(f"{len(ver)} verificaciones, {len(met)} renglones de metrado -> {out}")


if __name__ == "__main__":
    main()
