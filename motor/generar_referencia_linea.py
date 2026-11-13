"""Casos de referencia del cálculo completo de una línea (Python -> motor/tests/linea_*.txt).

Cada archivo trae las entradas (perfil, eje, conductor, hipótesis, parámetros y estructuras
manuales) y los resultados de ``linecraft.calculo.calcular`` para comparar con Rust.

Uso: ``python motor/generar_referencia_linea.py``
"""

import math
import sys
from dataclasses import replace
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))

from suite_electrica.linecraft import proyecto as pr  # noqa: E402
from suite_electrica.linecraft.calculo import calcular  # noqa: E402
from suite_electrica.linecraft.conductor import Hipotesis  # noqa: E402

EJ = RAIZ / "examples" / "linecraft"
SAL = Path(__file__).parent / "tests"


def f(x: float) -> str:
    return "nan" if isinstance(x, float) and math.isnan(x) else f"{x:.17e}"


def volcar(nombre: str, p) -> None:
    r = calcular(p)
    c = p.conductor
    L = [f"conductor {f(c.seccion)} {f(c.diametro)} {f(c.peso)} {f(c.carga_rotura)} "
         f"{f(c.modulo_elasticidad)} {f(c.coef_dilatacion)}",
         "curva_inicial " + " ".join(map(f, c.curva_inicial)),
         "curva_creep " + " ".join(map(f, c.curva_creep))]
    for h in p.hipotesis:
        L.append(f"hip {h.nombre} {f(h.temperatura)} {f(h.presion_viento)} {f(h.espesor_hielo)} "
                 f"{f(h.densidad_hielo)} {f(h.altitud_min)} {f(h.altitud_max)}")
    for k in ("n_conductores", "modelo", "hip_inicial", "tiro_inicial_pct", "hip_flecha_maxima", "tiro_maximo_pct",
              "distancia_minima_terreno", "altura_amarre", "vano_maximo", "vano_minimo", "angulo_retenida",
              "condicion_final", "vanos_independientes"):
        v = getattr(p, k)
        L.append(f"param {k} {f(v) if isinstance(v, float) else v}")
    for k, v in p.fuentes.items():
        L.append(f"fuente {k} {v.replace(' ', '_')}")
    for i in range(len(p.perfil.progresiva)):
        L.append(f"perfil {f(p.perfil.progresiva[i])} {f(p.perfil.este[i])} {f(p.perfil.norte[i])} "
                 f"{f(p.perfil.cota[i])} {p.perfil.tipo[i]}")
    for x, y in p.eje[:, :2]:
        L.append(f"eje {f(x)} {f(y)}")
    if p.estructuras_manual is not None:
        for row in p.estructuras_manual.to_dict("records"):
            t = row.get("tipo")
            alt = row.get("altura_amarre_m", math.nan)
            L.append(f"manual {f(float(row['progresiva_m']))} {t if isinstance(t, str) and t.strip() else '-'} "
                     f"{f(float(alt)) if alt == alt else '-'}")
    L.append("== resultados")
    for e in r.estructuras:
        L.append(f"est {f(e.progresiva)} {f(e.este)} {f(e.norte)} {f(e.cota_terreno)} {f(e.altura_amarre)} "
                 f"{e.tipo} {f(e.angulo)}")
    for t in r.tramos:
        L.append(f"tramo {t.inicio} {t.fin} {f(t.vano_regulador)}")
        for s in t.estados.values():
            L.append(f"estado {s.hipotesis} {f(s.temperatura)} {f(s.H)} {f(s.tiro_maximo)} {f(s.pct_rotura)}")
    for v in r.vanos:
        L.append(f"vano {v.a} {v.tramo} {f(v.longitud)} {f(v.desnivel)} {f(v.despeje)} {f(v.s_despeje)} "
                 f"{f(v.flecha_medio)} " + " ".join(f"{k}={f(x)}" for k, x in v.flechas.items()))
    for c_ in r.cargas:
        L.append(f"carga {c_.estructura} {c_.hipotesis} " + " ".join(map(f, (
            c_.vano_viento, c_.vano_peso, c_.transversal, c_.longitudinal, c_.vertical, c_.horizontal,
            c_.momento_base, c_.tiro_retenida, c_.viento_conductor, c_.presion_viento))))
    L.append(f"avisos {len(r.avisos)}")
    (SAL / f"linea_{nombre}.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(f"linea_{nombre}: {len(r.estructuras)} estructuras, {len(r.vanos)} vanos, {len(r.cargas)} cargas")


def main() -> None:
    base = pr.cargar(EJ / "proyecto.json")
    volcar("auto", base)
    volcar("manual", pr.cargar(EJ / "proyecto_manual.json"))
    creep = replace(base, conductor=replace(base.conductor, curva_inicial=(0.05, 50.0, 30.0, -80.0, 0.0),
                                            curva_creep=(0.0, 40.0)))
    volcar("creep", creep)
    man = pr.cargar(EJ / "proyecto_manual.json")
    z = float(sum(man.perfil.cota[~(man.perfil.cota != man.perfil.cota)]) / len(man.perfil.cota))
    hips = []
    for h in man.hipotesis:
        hips += [replace(h, altitud_max=z), replace(h, temperatura=h.temperatura + 7, altitud_min=z)]
    volcar("altitud_porvano", replace(man, hipotesis=hips, vanos_independientes=True, modelo="parabola",
                                      conductor=creep.conductor))


if __name__ == "__main__":
    main()
