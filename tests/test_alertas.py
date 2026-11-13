"""Alertas tipo REDLIN: exceso E = (P* − P)/P, códigos y alerta crítica por elemento."""

import math
from pathlib import Path

import pytest

from suite_electrica.documento import Documento
from suite_electrica.linecraft.alertas import _pendiente_salida
from suite_electrica.linecraft.mecanica import Parabola

EJ = Path(__file__).resolve().parents[1] / "examples"


@pytest.fixture
def doc():
    d = Documento()
    d.importar_puntos(EJ / "colina_gaussiana.csv")
    d.importar_eje(EJ / "eje.dxf")
    d.calcular()
    return d


def test_sin_limites_solo_arrancamiento(doc):
    # sin límites configurados solo queda S↑: el ejemplo tiene vano peso negativo en
    # suspensiones con aislador pin (PS1-3, rígido según la lámina DGE) -> problema real
    r = doc.calcular()
    assert set(r.alertas["Código"]) == {"S↑"} and not r.cumple


def test_pendiente_salida_parabola():
    c = Parabola(100.0, 0.0, 1000.0, 1.0)  # y'(0) = −w a / (2H) = −0.05
    assert _pendiente_salida(c, "a") == pytest.approx(math.degrees(math.atan(-0.05)), abs=1e-3)
    assert _pendiente_salida(c, "b") == pytest.approx(_pendiente_salida(c, "a"), abs=1e-3)


def test_eds_vibraciones(doc):
    doc.linea.eds_max_pct = 10
    r = doc.calcular()
    v = r.alertas[r.alertas["Código"] == "V"]
    assert len(v) == len(r.tramos)
    assert v["Exceso (%)"].iloc[0] == pytest.approx(80.0, abs=0.1)  # EDS 18 % vs 10 %
    assert not r.cumple
    assert any("eds_max_pct" in a for a in r.avisos)  # sin fuente -> aviso


def test_fs_poste_codigo_t_o_l(doc):
    doc.linea.fs_poste = 50
    r = doc.calcular()
    tl = r.alertas[r.alertas["Código"].isin(["T", "L"])]
    assert len(tl) and (tl["Exceso (%)"] > 0).all()


def test_separacion_y_aislador_rigido(doc):
    arm = doc.catalogos.armados
    arm["separacion_fases_m"] = 0.2
    arm.loc[arm["codigo"] == "PS1-3", "aislador"] = "RIGIDO"
    doc.linea.tension_kV = 22.9
    doc.linea.sep_k_tension = 0.01
    doc.linea.angulo_salida_min, doc.linea.angulo_salida_max = -20, 5
    r = doc.calcular()
    cods = set(r.alertas["Código"])
    assert "H" in cods
    h = r.alertas[r.alertas["Código"] == "H"].iloc[0]
    assert h["Exceso (%)"] == pytest.approx(100 * (0.229 / 0.2 - 1), abs=0.1)
    # E-02 (suspensión) tiene vano peso negativo -> conductor sale hacia arriba
    assert {"I↑", "S↑"} <= set(r.alertas[r.alertas["Elemento"] == "E-02"]["Código"])
    # una sola alerta crítica por elemento, la de mayor exceso
    crit = r.alertas[r.alertas["Crítica"]]
    assert crit["Elemento"].is_unique
    for el, g in r.alertas.groupby("Elemento"):
        assert g["Crítica"].iloc[0] and g["Exceso (%)"].iloc[0] == g["Exceso (%)"].max()
