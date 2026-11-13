"""Momento de vuelco del poste y FS (criterio de REDLIN)."""

import math
from pathlib import Path

import pandas as pd
import pytest

from suite_electrica.documento import Documento
from suite_electrica.linecraft.calculo import Cargas
from suite_electrica.linecraft.catalogos import factor_seguridad_poste, momentos_poste

POSTE = pd.Series({"longitud_m": 12.0, "empotramiento_m": 1.8, "diam_punta_m": 0.137, "diam_tierra_m": 0.229,
                   "mor_MPa": 55.2, "carga_nominal_daN": math.nan, "dist_aplicacion_m": math.nan})


def test_momentos_a_mano():
    c = Cargas(1, "H", 100, 50, transversal=150.0, longitudinal=200.0, vertical=0, horizontal=0,
               momento_base=0, tiro_retenida=0, viento_conductor=100.0, presion_viento=231.77)
    alt = [10.0, 10.0, 9.0]  # Σh = 29
    m = momentos_poste(c, alt, POSTE, None)
    assert m["MVC"] == pytest.approx(100 / 3 * 29)
    assert m["MTB"] == pytest.approx(50 / 3 * 29)
    assert m["MTD"] == pytest.approx(200 / 3 * 29)
    mve = 231.77 * 10.2**2 * (2 * 0.137 + 0.229) / 6
    assert m["MVE"] == pytest.approx(mve)
    assert m["MRN"] == pytest.approx(math.hypot(150 / 3 * 29, 200 / 3 * 29) + mve)
    assert momentos_poste(c, alt, POSTE, "todo")["MRN"] == pytest.approx(mve)
    # retenida en la bisectriz: solo toma el tiro por ángulo (MTB)
    assert momentos_poste(c, alt, POSTE, "angulo")["MRN"] == pytest.approx(math.hypot(100 / 3 * 29, 200 / 3 * 29) + mve)
    # retenida longitudinal: toma el desbalance (MTD)
    assert momentos_poste(c, alt, POSTE, "longitudinal")["MRN"] == pytest.approx(150 / 3 * 29 + mve)
    fs, _ = factor_seguridad_poste(POSTE, m["MRN"])
    assert fs == pytest.approx(55.2 / (m["MRN"] / (math.pi * 0.229**3 / 32) / 1e6))
    tronco = POSTE.copy()
    tronco["mor_MPa"], tronco["carga_nominal_daN"], tronco["dist_aplicacion_m"] = math.nan, 400, 0.15
    fs2, _ = factor_seguridad_poste(tronco, 10050.0)
    assert fs2 == pytest.approx(4000 / (10050 / (12 - 1.8 - 0.15)))


def test_verificacion_usa_momentos():
    ej = Path(__file__).resolve().parents[1] / "examples"
    d = Documento()
    d.importar_puntos(ej / "colina_gaussiana.csv")
    d.importar_eje(ej / "eje.dxf")
    d.calcular()
    pst = d.catalogos.postes
    pst["empotramiento_m"], pst["diam_punta_m"], pst["diam_tierra_m"] = 1.8, 0.16, 0.34
    d.catalogos.armados["alturas_fases_m"] = "0.1;0.1;1.0"
    r = d.calcular()
    v = r.verificacion
    sin_ret = v[v["Retenidas"] == 0]
    assert len(sin_ret) and sin_ret["MRN (N·m)"].notna().all() and (sin_ret["MRN (N·m)"] > 0).all()
    con_ret = v[v["Retenidas"] > 0]
    assert con_ret["MRN (N·m)"].notna().all()  # con retenida queda el viento sobre el poste
