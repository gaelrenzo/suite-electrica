"""Referencias de la etapa 2: GridCraft (red) y PresuCraft (presupuesto) -> motor/tests/.

Cada archivo lleva los parámetros usados y los resultados de la versión Python; las tablas de
entrada (CSV) son las de ``examples/`` (o van incrustadas con el prefijo ``csv <tabla> <línea>``).
"""

import math
import sys
from dataclasses import replace
from pathlib import Path

import pandas as pd

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))

from suite_electrica.gridcraft.red import calcular as calc_red  # noqa: E402
from suite_electrica.gridcraft.red import cargar as cargar_red  # noqa: E402
from suite_electrica.presucraft.presupuesto import Presupuesto  # noqa: E402
from suite_electrica.presucraft.presupuesto import calcular as calc_presu  # noqa: E402
from suite_electrica.presucraft.presupuesto import cargar as cargar_presu  # noqa: E402

SAL = Path(__file__).parent / "tests"


def f(x) -> str:
    if x is None or (isinstance(x, float) and math.isnan(x)):
        return "nan"
    return f"{float(x):.17e}"


def red(nombre: str, r) -> None:
    res = calc_red(r)
    L = [f"param {k} {getattr(r, k)}" for k in (
        "sistema", "tension_V", "calificacion_kw_lote", "fs_sp", "fp_sp", "fs_ce", "kw_lampara", "fs_ap", "fp_ap",
        "caida_max_pct", "perdidas_max_pct", "factor_carga", "transformador_kva", "factor_sobrecarga")]
    L.append(f"param tension_ap_V {r.tension_ap_V or 0}")
    L += [f"tabla_fs {u} {v}" for u, v in r.tabla_fs]
    L += [f"fuente {k} {v.replace(' ', '_')}" for k, v in r.fuentes.items()]
    for k, v in (("nodos", r.nodos), ("conductores", r.conductores)):
        L += [f"csv {k} {line}" for line in v.to_csv(index=False).strip().splitlines()]
    L.append("== resultados")
    for t in res.tramos.to_dict("records"):
        ap = [t.get(k, math.nan) for k in ("P acum. AP (kW)", "I AP (A)", "%ΔV AP", "Pérdidas AP (W)")]
        L.append("tramo " + " ".join([t["Nodo"], t["Padre"]] + [f(t[k]) for k in (
            "Long. (m)", "P SP (kW)", "P acum. (kW)", "K (Ω/km)", "I (A)", "ΔV tramo (V)", "ΔV acum. (V)", "%ΔV",
            "Pérdidas (W)")] + [f(x) for x in ap]))
    L += [f"resumen {k.replace(' ', '_')} {f(v)}" for k, v in res.resumen.items()]
    L += [f"alerta {a['Elemento'].replace(' ', '_')} {a['Código']} {f(a['Exceso (%)'])}" for a in res.alertas.to_dict("records")]
    L.append(f"avisos {len(res.avisos)}")
    (SAL / f"red_{nombre}.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(f"red_{nombre}: {len(res.tramos)} tramos, {len(res.alertas)} alertas")


def presu(nombre: str, p: Presupuesto) -> None:
    res = calc_presu(p)
    L = [f"param {k} {getattr(p, k)}" for k in ("flete_por_kg", "gastos_generales_pct", "utilidad_pct", "igv_pct")]
    L += [f"fuente {k} {v.replace(' ', '_')}" for k, v in p.fuentes.items()]
    for k in ("suministros", "unidades", "montaje", "metrado"):
        L += [f"csv {k} {line}" for line in getattr(p, k).to_csv(index=False).strip().splitlines()]
    L.append("== resultados")
    for x in res.resumen.to_dict("records"):
        L.append(f"resumen {x['Rubro']} " + " ".join(f(x[k]) for k in (
            "Suministro", "Montaje", "Transporte", "Costo directo", "Gastos generales", "Utilidad", "Subtotal",
            "IGV", "Total")))
    for x in res.suministro.to_dict("records"):
        L.append(f"sum {x['Rubro']} {x['Código']} {f(x['Cantidad'])} {f(x['Parcial'])} {f(x['Peso (kg)'])} "
                 f"{f(x['Transporte'])}")
    for x in res.planilla_uc.to_dict("records"):
        L.append(f"uc {x['Rubro']}|{x['UC']}|{f(x['Suministro unit.'])}|{f(x['Montaje unit.'])}|{f(x['Total'])}")
    L += [f"aviso {a}" for a in res.avisos]
    (SAL / f"presu_{nombre}.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(f"presu_{nombre}: {len(res.resumen)} rubros+total, {len(res.avisos)} avisos")


def main() -> None:
    base = cargar_red(RAIZ / "examples" / "gridcraft" / "red.json")
    red("ejemplo", base)
    cond = base.conductores.copy()
    cond["corriente_max_A"] = [12.0, 8.0, 5.0]
    cond.loc[0, "k_ohm_km"] = 1.5
    red("alertas", replace(base, sistema="1F", tension_V=220, conductores=cond, tabla_fs=[(10, 0.7), (30, 0.45)],
                            caida_max_pct=0.3, perdidas_max_pct=0.1, transformador_kva=5, factor_sobrecarga=1.2,
                            tension_ap_V=230))
    p = cargar_presu(RAIZ / "examples" / "presucraft" / "presupuesto.json")
    presu("ejemplo", p)
    me = pd.concat([p.metrado, pd.DataFrame([{"rubro": "RP", "uc": "PS-1", "cantidad": 3},
                                             {"rubro": "RP", "uc": "UX", "cantidad": 1},
                                             {"rubro": "RS", "uc": "RET 3/8", "cantidad": 0}])], ignore_index=True)
    sm = p.suministros.copy()
    sm.loc[sm["codigo"] == "CRU-1", "precio"] = math.nan
    presu("variante", replace(p, metrado=me, suministros=sm, flete_por_kg=0.45))


if __name__ == "__main__":
    main()
