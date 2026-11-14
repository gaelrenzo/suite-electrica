"""Referencias de la etapa 1e: alertas, tabla de flechado y derivaciones (Python -> motor/tests/).

* ``alertas_auto.txt``: alertas del caso de ``catalogos_auto.txt`` con todos los límites activos.
* ``flechado_<caso>.txt``: tabla de flechado de ``linea_auto`` y ``linea_altitud_porvano``.
* ``linea_ramal.txt`` + ``derivacion.txt``: ramal perpendicular en E-03 y cargas combinadas.
"""

import io
import json
import math
import sys
from dataclasses import replace
from pathlib import Path

import numpy as np
import pandas as pd

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))
sys.path.insert(0, str(Path(__file__).parent))

from generar_referencia_catalogos import POSTES  # noqa: E402
from generar_referencia_linea import volcar  # noqa: E402
from suite_electrica.linecraft import catalogos as cm  # noqa: E402
from suite_electrica.linecraft import proyecto as pr  # noqa: E402
from suite_electrica.linecraft.alertas import ConfigAlertas, alertas  # noqa: E402
from suite_electrica.linecraft.calculo import calcular  # noqa: E402
from suite_electrica.linecraft.derivaciones import calcular_red, cargar_red, eje_longitudinal  # noqa: E402
from suite_electrica.linecraft.salidas import tabla_flechado  # noqa: E402

EJ = RAIZ / "examples" / "linecraft"
SAL = Path(__file__).parent / "tests"
CFG = ConfigAlertas(fs_poste=4.0, fs_retenida=3.0, tension_kV=22.9, sep_k_tension=0.04, sep_k_flecha=0.3,
                    angulo_salida_min=-20, angulo_salida_max=5, tiro_max_espiga=2000, eds_max_pct=10)


def f(x: float) -> str:
    return "inf" if x == math.inf else f"{x:.17e}"


def ref_alertas() -> None:
    p = pr.cargar(EJ / "proyecto.json")
    r = calcular(p)
    cat = cm.Catalogos.plantilla()
    cat.postes = cm.normalizar("postes", pd.read_csv(io.StringIO(POSTES)))
    fm = cm._fuerzas_maximas(r)
    for e in r.estructuras:
        e.armado = cat.elegir_armado(e.tipo, e.angulo)
        fh, mb, _ = fm.get(e.numero, (0.0, 0.0, 0.0))
        e.poste = cm.elegir_poste(cat, e, fh, mb, 2.0)
    r.estructuras[2].poste = "M 12/C6"
    r.verificacion = cm.verificar(r, cat, 2.0, 2.0)
    a = alertas(r, cat, CFG)
    L = [f"alerta {x['Elemento'].replace(' ', '_')} {x['Código']} {f(float(x['Exceso (%)']))} {x['Crítica']}"
         for x in a.to_dict("records")]
    (SAL / "alertas_auto.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(f"alertas_auto: {len(a)} alertas, códigos {sorted(set(a['Código']))}")


def ref_flechado(nombre: str, p) -> None:
    t = tabla_flechado(calcular(p))
    L = []
    for fila in t.to_dict("records"):
        vals = [f"{k.split()[1]}:{fila[k]!r}:{fila['Flecha ' + k.split()[1] + ' °C (m)']!r}"
                for k in fila if k.startswith("H ")]
        L.append(f"fila {fila['Tramo']} {fila['Vano']} " + " ".join(vals))
    (SAL / f"flechado_{nombre}.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(f"flechado_{nombre}: {len(t)} vanos")


def ref_derivacion() -> None:
    cfg = json.loads((EJ / "proyecto.json").read_text(encoding="utf-8"))
    for k in ("puntos", "eje"):
        cfg["perfil"][k] = str((EJ / cfg["perfil"][k]).resolve())
    cfg["conductor"]["catalogo"] = str((EJ / cfg["conductor"]["catalogo"]).resolve())
    tmp = SAL / "_tmp_der"
    tmp.mkdir(exist_ok=True)
    ruta = tmp / "p.json"
    ruta.write_text(json.dumps(cfg), encoding="utf-8")
    principal, _, _ = cargar_red(ruta)
    r = calcular(principal)
    i = next(k for k, e in enumerate(r.estructuras) if e.codigo == "E-03")
    e, u = r.estructuras[i], eje_longitudinal(r, i)
    pts = np.array([[e.este, e.norte], [e.este - 180 * u[1], e.norte + 180 * u[0]]])
    pd.DataFrame(pts, columns=["este", "norte"]).to_csv(tmp / "ramal.csv", index=False)
    cfg["derivaciones"] = [{"nombre": "Ramal A", "desde": "E-03",
                            "perfil": {**cfg["perfil"], "eje": str(tmp / "ramal.csv")}}]
    ruta.write_text(json.dumps(cfg), encoding="utf-8")
    pri, ramales, _ = cargar_red(ruta)
    volcar("ramal", ramales[0][2])
    red = calcular_red(pri, ramales)
    L = [f"der {x['Hipótesis']} {f(x['β (°)'])} {f(x['+Longitudinal (N)'])} {f(x['+Transversal (N)'])} "
         f"{f(x['+Vertical (N)'])} {f(x['Horizontal total (N)'])}" for x in red.derivaciones.to_dict("records")]
    num = next(e.numero for e in red.principal.estructuras if e.codigo == "E-03")
    for c in red.principal.cargas:
        if c.estructura == num:
            L.append(f"carga {c.hipotesis} {f(c.transversal)} {f(c.longitudinal)} {f(c.vertical)} "
                     f"{f(c.horizontal)} {f(c.tiro_retenida)}")
    (SAL / "derivacion.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    for x in tmp.iterdir():
        x.unlink()
    tmp.rmdir()
    print(f"derivacion: {len(red.derivaciones)} filas")


def main() -> None:
    ref_alertas()
    ref_flechado("auto", pr.cargar(EJ / "proyecto.json"))
    man = pr.cargar(EJ / "proyecto_manual.json")
    ref_flechado("manual", replace(man, modelo="parabola"))
    ref_derivacion()


if __name__ == "__main__":
    main()
