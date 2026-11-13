"""Líneas con derivaciones (ramales): cálculo de cada ramal y cargas combinadas en el punto de derivación.

En ``proyecto.json``::

    "derivaciones": [
      {"nombre": "Ramal A", "desde": "E-05", "perfil": {...}, "estructuras": "ramal_a.csv"}
    ]

Cada ramal hereda los parámetros de la línea principal (conductor, hipótesis, EDS…) y puede
sobrescribir cualquiera. Su eje debe empezar en la estructura ``desde`` de la principal
(tolerancia ``TOL_DERIVACION``). Su primera estructura es la misma estructura física: sus
cargas (tiro del primer vano del ramal, viento y peso) se suman a las de la principal.

Combinación en planta con β = ángulo entre el ramal y el eje longitudinal de la principal:

* longitudinal += |L_ramal · cos β|,   transversal += |L_ramal · sen β| + T_ramal
* vertical += V_ramal;  momento y retenida se recalculan con la horizontal resultante.

ponytail: suma de magnitudes (conservadora): no se conoce el sentido del desbalance de la
principal ni del viento; un cálculo vectorial por hipótesis exigiría signos en ``Cargas``.
"""

from __future__ import annotations

import copy
import json
import math
from dataclasses import dataclass, field, replace
from pathlib import Path

import numpy as np
import pandas as pd

from suite_electrica.linecraft.calculo import Resultado, calcular
from suite_electrica.linecraft.proyecto import Proyecto, desde_cfg

TOL_DERIVACION = 2.0
"""Distancia máxima (m) entre el inicio del eje del ramal y la estructura de derivación."""


@dataclass
class ResultadoRed:
    principal: Resultado
    ramales: dict[str, Resultado] = field(default_factory=dict)
    derivaciones: pd.DataFrame = field(default_factory=pd.DataFrame)
    """Por punto de derivación e hipótesis: β y cargas agregadas a la principal."""

    @property
    def cumple(self) -> bool:
        return self.principal.cumple and all(r.cumple for r in self.ramales.values())


def _unitario(v: np.ndarray) -> np.ndarray:
    n = float(np.hypot(*v))
    return v / n if n else v


def eje_longitudinal(r: Resultado, i: int) -> np.ndarray:
    """Dirección (unitaria) del eje longitudinal de la estructura ``i``: bisectriz de los vanos."""
    est = r.estructuras
    p = np.array([[e.este, e.norte] for e in est])
    d = [_unitario(p[i] - p[i - 1])] if i > 0 else []
    d += [_unitario(p[i + 1] - p[i])] if i < len(est) - 1 else []
    return _unitario(sum(d))


def combinar(principal: Resultado, ramal: Resultado, nombre: str, desde: str) -> pd.DataFrame:
    """Suma las cargas de la primera estructura del ramal a la estructura ``desde``."""
    idx = {e.codigo: i for i, e in enumerate(principal.estructuras)}
    if desde not in idx:
        raise ValueError(f"Derivación {nombre}: la principal no tiene la estructura {desde}")
    i = idx[desde]
    e = principal.estructuras[i]
    r0 = ramal.estructuras[0]
    dist = math.hypot(r0.este - e.este, r0.norte - e.norte)
    if dist > TOL_DERIVACION:
        raise ValueError(f"Derivación {nombre}: el eje empieza a {dist:.1f} m de {desde} "
                         f"(máx. {TOL_DERIVACION} m)")
    u_p = eje_longitudinal(principal, i)
    u_r = _unitario(np.array([ramal.estructuras[1].este - r0.este, ramal.estructuras[1].norte - r0.norte]))
    beta = math.degrees(math.acos(float(np.clip(u_p @ u_r, -1, 1))))
    phi = math.radians(principal.proyecto.angulo_retenida)
    cr = {c.hipotesis: c for c in ramal.cargas if c.estructura == r0.numero}
    filas = []
    for k, c in enumerate(principal.cargas):
        if c.estructura != e.numero or c.hipotesis not in cr:
            continue
        d = cr[c.hipotesis]
        dl = abs(d.longitudinal * math.cos(math.radians(beta)))
        dtr = abs(d.longitudinal * math.sin(math.radians(beta))) + d.transversal
        lon, tra = c.longitudinal + dl, c.transversal + dtr
        fh = math.hypot(tra, lon)
        principal.cargas[k] = replace(
            c, longitudinal=lon, transversal=tra, vertical=c.vertical + d.vertical, horizontal=fh,
            momento_base=fh * e.altura_amarre,
            tiro_retenida=fh / math.cos(phi) if c.tiro_retenida or d.tiro_retenida else 0.0)
        filas.append({"Estructura": desde, "Ramal": nombre, "Hipótesis": c.hipotesis, "β (°)": round(beta, 1),
                      "+Longitudinal (N)": round(dl, 1), "+Transversal (N)": round(dtr, 1),
                      "+Vertical (N)": round(d.vertical, 1), "Horizontal total (N)": round(fh, 1)})
    return pd.DataFrame(filas)


def calcular_red(principal: Proyecto, ramales: list[tuple[str, str, Proyecto]]) -> ResultadoRed:
    """Calcula la principal y cada ramal ``(nombre, desde, proyecto)`` y combina las cargas."""
    rp = calcular(principal)
    out = ResultadoRed(rp)
    tablas = []
    for nombre, desde, p in ramales:
        rr = calcular(p)
        out.ramales[nombre] = rr
        tablas.append(combinar(rp, rr, nombre, desde))
        rp.avisos.append(f"{desde}: derivación '{nombre}' — cargas del ramal sumadas (ver hoja Derivaciones)")
    out.derivaciones = pd.concat(tablas, ignore_index=True) if tablas else pd.DataFrame()
    return out


def cargar_red(ruta: str | Path) -> tuple[Proyecto, list[tuple[str, str, Proyecto]], dict]:
    """Proyecto principal y ramales desde ``proyecto.json`` (los ramales heredan la configuración)."""
    ruta = Path(ruta)
    cfg = json.loads(ruta.read_text(encoding="utf-8"))
    ders = cfg.pop("derivaciones", None) or []
    principal = desde_cfg(copy.deepcopy(cfg), ruta.parent, ruta.stem)
    ramales = []
    for k, d in enumerate(ders, start=1):
        sub = copy.deepcopy(cfg)
        sub["estructuras"] = None
        sub.update({key: v for key, v in d.items() if key not in ("desde", "nombre")})
        nombre = str(d.get("nombre", f"Ramal {k}"))
        sub["nombre"] = nombre
        ramales.append((nombre, str(d["desde"]), desde_cfg(sub, ruta.parent, nombre)))
    return principal, ramales, cfg
