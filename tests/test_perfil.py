import numpy as np
import pytest

from conftest import tin_de
from core.geo.perfil import ESTACION, QUIEBRE, VERTICE, perfil_longitudinal, progresivas_eje
from topocraft import sinteticos as st


def cota_manual(tin, x, y):
    """Interpolación 'a mano': busca el triángulo por fuerza bruta y resuelve el plano."""
    for t in tin.triangulos_activos:
        p = tin.xyz[t]
        a, b, c = p[:, :2] - [x, y]
        cr = [u[0] * v[1] - u[1] * v[0] for u, v in ((a, b), (b, c), (c, a))]
        if all(v >= -1e-9 for v in cr) or all(v <= 1e-9 for v in cr):
            rel = p[:, :2] - p[0, :2]
            m = np.column_stack([rel, np.ones(3)])
            coef = np.linalg.solve(m, p[:, 2])
            return coef[0] * (x - p[0, 0]) + coef[1] * (y - p[0, 1]) + coef[2]
    return np.nan


def test_progresivas_y_tipos(plano):
    _, tin = tin_de(plano)
    eje = st.eje_ejemplo()
    pf = perfil_longitudinal(tin, eje, paso=20)
    acum = progresivas_eje(eje)
    assert pf.longitud == pytest.approx(acum[-1])
    assert np.all(np.diff(pf.progresiva) > 0)
    for s in acum:
        assert np.min(np.abs(pf.progresiva[pf.tipo == VERTICE] - s)) < 1e-9
    est = pf.progresiva[pf.tipo == ESTACION]
    assert np.allclose(np.mod(est, 20), 0) or np.allclose(np.mod(est + 1e-9, 20), 0, atol=1e-6)
    assert (pf.tipo == QUIEBRE).sum() > 0


def test_perfil_plano_exacto(plano):
    _, tin = tin_de(plano)
    pf = perfil_longitudinal(tin, st.eje_ejemplo(), paso=20)
    assert np.max(np.abs(pf.cota - plano.cota(pf.este, pf.norte))) < 1e-6


def test_criterio_aceptacion_vs_interpolacion_manual(colina):
    """Criterio de la especificación: error < 1 cm contra interpolación manual."""
    _, tin = tin_de(colina, paso=20)
    pf = perfil_longitudinal(tin, st.eje_ejemplo(), paso=25)
    manual = np.array([cota_manual(tin, x, y) for x, y in zip(pf.este, pf.norte)])
    assert np.max(np.abs(pf.cota - manual)) < 0.01
    # El TIN aproxima la colina real dentro del error de discretización
    assert np.max(np.abs(pf.cota - colina.cota(pf.este, pf.norte))) < 0.5


def test_perfil_lineal_entre_quiebres(colina):
    """Con los cruces de aristas incluidos, entre puntos del perfil el TIN es lineal."""
    _, tin = tin_de(colina, paso=20)
    pf = perfil_longitudinal(tin, st.eje_ejemplo(), paso=50)
    sm = (pf.progresiva[1:] + pf.progresiva[:-1]) / 2
    from core.geo.perfil import punto_en_progresiva
    xy = punto_en_progresiva(st.eje_ejemplo(), sm)
    assert np.max(np.abs(tin.cota(xy[:, 0], xy[:, 1]) - pf.cota_en(sm))) < 1e-6


def test_eje_fuera_del_tin(plano):
    _, tin = tin_de(plano)
    eje = np.array([[st.E0 - 100, st.N0 + 100], [st.E0 + 100, st.N0 + 100]])
    pf = perfil_longitudinal(tin, eje, paso=10)
    assert np.isnan(pf.cota[pf.progresiva < 99]).all()
    assert not np.isnan(pf.cota[pf.progresiva > 101]).any()


def test_validaciones(plano):
    _, tin = tin_de(plano)
    with pytest.raises(ValueError):
        perfil_longitudinal(tin, [[0, 0]], 10)
    with pytest.raises(ValueError):
        perfil_longitudinal(tin, st.eje_ejemplo(), 0)
