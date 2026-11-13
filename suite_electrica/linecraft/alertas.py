"""Alertas de diseño con exceso porcentual sobre el valor permisible.

Cada alerta tiene ``E = (P* − P) / P`` (``P*`` magnitud evaluada, ``P`` máximo
permitido con su factor de seguridad); solo se reporta si ``E > 0``. Por
elemento (vano o estructura) se marca como *crítica* la de mayor ``E``.

Códigos:

======  ==================================================================
DMS     Distancia mínima de seguridad al terreno (vano).
C       Tiro del conductor sobre el máximo (% de rotura) en el tramo.
H       Separación horizontal entre fases a medio vano.
V       Tiro horizontal EDS sobre el límite por vibraciones eólicas (tramo).
T / L   Poste sin retenida: momento de vuelco; T si predomina la carga
        transversal, L si la longitudinal.
R       Retenida: tiro por retenida sobre su carga de rotura / FS.
S↓      Compresión vertical (conductores + componente vertical de retenida)
        sobre la carga vertical máxima del poste.
S↑      Vano peso negativo en aislador rígido (el conductor tira hacia arriba).
I↑ I↓   Aislador rígido: ángulo de salida del conductor fuera de [θmin, θmax].
I→      Aislador rígido: fuerza por fase sobre la carga máxima de la espiga.
======  ==================================================================

Los parámetros en 0 (y columnas de catálogo vacías) desactivan la verificación
correspondiente: no se asumen valores normativos.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import pandas as pd

from suite_electrica.linecraft.calculo import Resultado
from suite_electrica.linecraft.mecanica import curva

COLUMNAS = ["Elemento", "Código", "Exceso (%)", "Detalle", "Crítica"]


@dataclass
class ConfigAlertas:
    """Límites opcionales (0 = no verificar)."""

    fs_poste: float = 0.0
    fs_retenida: float = 0.0
    tension_kV: float = 0.0
    sep_k_tension: float = 0.0
    """Separación mínima ``D = k_U · U + k_f · √f`` (m); coeficiente de U (m/kV)."""
    sep_k_flecha: float = 0.0
    """Coeficiente de √flecha (m^0.5)."""
    angulo_salida_min: float = 0.0
    """Ángulo mínimo de salida en aislador rígido (°, negativo hacia abajo)."""
    angulo_salida_max: float = 0.0
    tiro_max_espiga: float = 0.0
    """Carga máxima por espiga (N)."""
    eds_max_pct: float = 0.0
    """Tiro EDS máximo por vibraciones eólicas (% de rotura)."""


def _exceso(evaluado: float, permitido: float) -> float:
    return 100 * (evaluado - permitido) / permitido


def _pendiente_salida(c, extremo: str) -> float:
    """Ángulo (°) del conductor al salir del soporte hacia el vano; + hacia arriba."""
    dx = min(0.01, c.a / 1000)
    if extremo == "a":
        return math.degrees(math.atan((float(c.y(dx)) - float(c.y(0.0))) / dx))
    return math.degrees(math.atan((float(c.y(c.a - dx)) - float(c.y(c.a))) / dx))


def alertas(r: Resultado, cat=None, cfg: ConfigAlertas | None = None) -> pd.DataFrame:
    """Tabla de alertas (columnas ``COLUMNAS``), ordenada de mayor a menor exceso."""
    cfg = cfg or ConfigAlertas()
    p, est = r.proyecto, r.estructuras
    filas: list[tuple[str, str, float, str]] = []

    def add(elem: str, cod: str, e: float, det: str) -> None:
        if e > 1e-9:
            filas.append((elem, cod, round(e, 1), det))

    # --- vanos y tramos
    hf = p.hip_flecha_maxima
    for v in r.vanos:
        nombre = f"Vano {est[v.a].codigo}-{est[v.a + 1].codigo}"
        add(nombre, "DMS", _exceso(p.distancia_minima_terreno, v.despeje) if v.despeje > 0 else math.inf,
            f"despeje {v.despeje:.2f} m < {p.distancia_minima_terreno:.2f} m en {v.s_despeje:.1f} m")
        if cfg.tension_kV and (cfg.sep_k_tension or cfg.sep_k_flecha) and cat is not None:
            seps = [cat.armado(est[i].armado) for i in (v.a, v.a + 1)]
            seps = [s["separacion_fases_m"] for s in seps if s is not None and not pd.isna(s["separacion_fases_m"])]
            if seps:
                actual = min(seps)
                req = cfg.sep_k_tension * cfg.tension_kV + cfg.sep_k_flecha * math.sqrt(max(v.flechas[hf], 0))
                add(nombre, "H", _exceso(req, actual), f"separación {actual:.2f} m < {req:.2f} m requerida")
    for t in r.tramos:
        nombre = f"Tramo {est[t.inicio].codigo}-{est[t.fin].codigo}"
        for s in t.estados.values():
            add(nombre, "C", _exceso(s.pct_rotura, p.tiro_maximo_pct),
                f"{s.hipotesis}: tiro {s.pct_rotura:.1f} % > {p.tiro_maximo_pct:g} %")
        if cfg.eds_max_pct:
            eds = 100 * t.estados[p.hip_inicial].H / p.conductor.carga_rotura  # tiro horizontal
            add(nombre, "V", _exceso(eds, cfg.eds_max_pct),
                f"EDS {eds:.1f} % > {cfg.eds_max_pct:g} % (vibraciones eólicas)")

    # --- estructuras: postes y retenidas (reusa la verificación de catálogos)
    ver = r.verificacion if r.verificacion is not None else pd.DataFrame()
    cargas = pd.DataFrame([vars(c) for c in r.cargas])
    phi = math.radians(p.angulo_retenida)
    numero = {x.codigo: x.numero for x in est}

    def valor(fila: pd.Series, col: str) -> float | None:
        v = pd.to_numeric(fila.get(col), errors="coerce")
        return float(v) if not pd.isna(v) and v > 0 else None

    for _, fila in ver.iterrows():
        cod_e = fila["Estructura"]
        if cfg.fs_poste and (fs := valor(fila, "FS poste")):
            c = cargas[cargas.estructura == numero[cod_e]].sort_values("horizontal").iloc[-1]
            add(cod_e, "T" if c.transversal >= c.longitudinal else "L", _exceso(cfg.fs_poste, fs),
                f"FS poste {fs:.2f} < {cfg.fs_poste:g} ({c.hipotesis})")
        if cfg.fs_retenida and (fsr := valor(fila, "FS retenida")):
            add(cod_e, "R", _exceso(cfg.fs_retenida, fsr), f"FS retenida {fsr:.2f} < {cfg.fs_retenida:g}")

    # --- estructuras: verticales y aisladores
    for i, e in enumerate(est):
        ce = cargas[cargas.estructura == e.numero]
        poste = cat.poste(e.poste) if cat is not None and e.poste else None
        if poste is not None and "carga_vertical_max_daN" in poste and not pd.isna(poste["carga_vertical_max_daN"]):
            comp = (ce.vertical + ce.tiro_retenida * math.sin(phi)).max()
            add(e.codigo, "S↓", _exceso(comp, poste["carga_vertical_max_daN"] * 10),
                f"compresión {comp:.0f} N > {poste['carga_vertical_max_daN'] * 10:.0f} N")
        arm = cat.armado(e.armado) if cat is not None and e.armado else None
        if arm is None or str(arm.get("aislador", "")).upper() != "RIGIDO":
            continue
        # ponytail: S↑ sin magnitud permisible (no hay dato de arranque del soporte): exceso ∞,
        # solo en aislador rígido; en anclaje/suspensión queda como aviso de ``calculo``.
        if len(ce) and ce.vano_peso.min() < 0:
            c = ce.sort_values("vano_peso").iloc[0]
            add(e.codigo, "S↑", math.inf, f"vano peso {c.vano_peso:.1f} m en {c.hipotesis} (arrancamiento)")
        n_ais = arm["aisladores_por_fase"] if not pd.isna(arm.get("aisladores_por_fase", math.nan)) else 1
        if cfg.tiro_max_espiga and len(ce):
            f_fase = (ce.horizontal / p.n_conductores).max()
            lim = cfg.tiro_max_espiga * n_ais
            add(e.codigo, "I→", _exceso(f_fase, lim), f"fuerza por fase {f_fase:.0f} N > {lim:.0f} N")
        if cfg.angulo_salida_min or cfg.angulo_salida_max:
            angs = []
            for k, extremo in ((i - 1, "b"), (i, "a")):
                if 0 <= k < len(r.vanos):
                    v = r.vanos[k]
                    for s in r.tramo_de_vano(k).estados.values():
                        angs.append(_pendiente_salida(curva(p.modelo, v.longitud, v.desnivel, s.H, s.w_vertical),
                                                      extremo))
            if angs:
                hi, lo = max(angs), min(angs)
                if cfg.angulo_salida_max:
                    add(e.codigo, "I↑", 100 * (hi - cfg.angulo_salida_max) / abs(cfg.angulo_salida_max),
                        f"ángulo de salida {hi:.1f}° > {cfg.angulo_salida_max:g}° (desamarre)")
                if cfg.angulo_salida_min:
                    add(e.codigo, "I↓", 100 * (cfg.angulo_salida_min - lo) / abs(cfg.angulo_salida_min),
                        f"ángulo de salida {lo:.1f}° < {cfg.angulo_salida_min:g}°")

    df = pd.DataFrame(filas, columns=COLUMNAS[:4]).sort_values("Exceso (%)", ascending=False, kind="stable")
    df["Crítica"] = ~df["Elemento"].duplicated()
    return df.reset_index(drop=True)
