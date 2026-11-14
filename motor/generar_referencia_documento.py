"""Flujo completo de la app con el documento .sel (Python -> tests/documento*.txt).

``tests/archivos/documento_irregular.sel``: puntos irregulares de la colina (semilla 7, TIN
único) + eje, catálogos de plantilla. Se calcula como la app (ubicación automática, armados y
postes, verificación, metrado, alertas, avisos) y luego se edita: agregar, mover, cambiar tipo
y altura, eliminar, deshacer y recalcular.
"""

import math
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))
sys.path.insert(0, str(Path(__file__).parent))

import pandas as pd  # noqa: E402

from generar_referencia_salidas import puntos_topo  # noqa: E402
from suite_electrica.documento import Documento  # noqa: E402

AQUI = Path(__file__).parent / "tests"
SEL = AQUI / "archivos" / "documento_irregular.sel"


def f(x) -> str:
    if x is None or (isinstance(x, float) and math.isnan(x)):
        return "-"
    return "inf" if x == math.inf else f"{float(x):.17e}"


def volcar(doc: Documento, r, etiqueta: str) -> list[str]:
    L = [f"== {etiqueta} cumple={r.cumple}"]
    for e in doc.estructuras.itertuples(index=False):
        L.append(f"est {f(e.progresiva_m)} {e.tipo or '-'} {e.armado or '-'} {(e.poste or '-').replace(' ', '_')} "
                 f"{f(e.altura_amarre_m)} {f(e.retenidas)}")
    for e in r.estructuras:
        L.append(f"res {e.codigo} {e.tipo} {f(e.progresiva)} {f(e.altura_amarre)} {f(e.angulo)}")
    for v in r.verificacion.to_dict("records"):
        L.append(f"ver {v['Estructura']} {v['Cumple']}")
    for a in r.alertas.to_dict("records"):
        L.append(f"alerta {a['Elemento'].replace(' ', '_')} {a['Código']} {f(a['Exceso (%)'])}")
    for m in r.metrado.to_dict("records"):
        L.append(f"met {m['Ítem']}|{m['Código']}|{m['Unidad']}|{f(m['Cantidad'])}")
    for a in r.avisos:
        L.append(f"aviso {a}")
    return L


def main() -> None:
    xyz, eje = puntos_topo()
    doc = Documento()
    doc.nombre = "Documento de prueba (puntos irregulares)"
    doc.puntos = pd.DataFrame({"punto": [str(i + 1) for i in range(len(xyz))], "este": xyz[:, 0],
                               "norte": xyz[:, 1], "cota": xyz[:, 2], "descripcion": ""})
    doc.eje = eje
    doc.topo.equidistancia = 2.5
    doc.linea.tension_kV = 22.9
    doc.linea.eds_max_pct = 18
    doc.fuentes["eds_max_pct"] = "EJEMPLO: prueba"
    doc.cambio_topografia()
    doc.guardar(SEL)

    L = volcar(doc, doc.calcular(), "calculo")
    s_mid = (doc.estructuras["progresiva_m"].iloc[1] + doc.estructuras["progresiva_m"].iloc[2]) / 2
    i = doc.agregar_estructura(float(s_mid))
    L.append(f"agregar {f(s_mid)} {i}")
    s = doc.mover_estructura(i, float(s_mid) + 7.3)
    L.append(f"mover {i} {f(s)}")
    doc.cambiar_estructura(i, "tipo", "ANCLAJE")
    doc.cambiar_estructura(i, "altura_amarre_m", 12.0)
    doc.cambiar_estructura(1, "poste", "")
    L += volcar(doc, doc.calcular(), "editado")
    doc.eliminar_estructura(i)
    doc.deshacer()
    L.append(f"deshacer {len(doc.estructuras)}")
    L += volcar(doc, doc.asignar_catalogos(solo_vacios=True), "asignado")
    (AQUI / "documento.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(len(L), "líneas")


if __name__ == "__main__":
    main()
