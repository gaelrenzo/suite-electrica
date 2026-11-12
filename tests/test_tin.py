import numpy as np
import pytest

from conftest import tin_de
from core.geo.tin import TIN, es_mayor, niveles
from topocraft import sinteticos as st


def test_triangulo_calculado_a_mano():
    # z = 100 + 0.2 x + 0.1 y  ->  en (2, 3): 100 + 0.4 + 0.3 = 100.7
    tin = TIN([[0, 0, 100], [10, 0, 102], [0, 10, 101]])
    assert tin.cota(2, 3)[0] == pytest.approx(100.7, abs=1e-9)
    assert np.isnan(tin.cota(20, 20)[0])


def test_plano_se_interpola_exacto(plano):
    _, tin = tin_de(plano)
    rng = np.random.default_rng(1)
    x = st.E0 + rng.uniform(0, st.LADO, 500)
    y = st.N0 + rng.uniform(0, st.LADO, 500)
    assert np.max(np.abs(tin.cota(x, y) - plano.cota(x, y))) < 1e-6


def test_colineales_error():
    with pytest.raises(ValueError):
        TIN([[0, 0, 1], [1, 1, 2], [2, 2, 3]])


def test_max_lado_descarta_triangulos():
    pts = [[0, 0, 0], [10, 0, 0], [0, 10, 0], [10, 10, 0], [200, 5, 0]]
    tin = TIN(pts, max_lado=50)
    assert tin.activos.sum() < len(tin.triangulos)
    assert np.isnan(tin.cota(100, 5)[0])
    assert not np.isnan(tin.cota(3, 3)[0])


def test_niveles_y_mayores():
    assert niveles(3820.3, 3824.0, 1).tolist() == [3821, 3822, 3823, 3824]
    assert niveles(0.0, 1.0, 0.25).tolist() == [0, 0.25, 0.5, 0.75, 1.0]
    assert es_mayor(3825, 1, 5) and not es_mayor(3823, 1, 5)
    assert es_mayor(3830, 2, 5) and not es_mayor(3828, 2, 5)
    with pytest.raises(ValueError):
        niveles(0, 1, 0)


def test_curvas_plano_estan_a_su_cota(plano):
    df, tin = tin_de(plano)
    cotas = niveles(df.cota.min(), df.cota.max(), 1.0)
    curvas = tin.curvas_nivel(cotas)
    assert {c.cota for c in curvas} <= set(cotas.tolist())
    assert len({c.cota for c in curvas}) >= len(cotas) - 2  # extremos pueden ser un punto
    for c in curvas:
        assert np.max(np.abs(plano.cota(c.xy[:, 0], c.xy[:, 1]) - c.cota)) < 1e-6
        assert not c.cerrada


def test_curvas_cono_son_circulos_cerrados():
    sup = st.cono(altura=50, pendiente=0.2)
    _, tin = tin_de(sup)
    xc, yc = st.E0 + st.LADO / 2, st.N0 + st.LADO / 2
    for c in tin.curvas_nivel([st.Z0 + 45, st.Z0 + 30, st.Z0 + 10]):
        r_esperado = (st.Z0 + 50 - c.cota) / 0.2
        r = np.hypot(c.xy[:, 0] - xc, c.xy[:, 1] - yc)
        assert c.cerrada
        # error de discretización del TIN (malla 10 m): < 2 % del radio
        assert np.max(np.abs(r - r_esperado)) < 0.02 * r_esperado + 0.5
        assert c.longitud == pytest.approx(2 * np.pi * r_esperado, rel=0.02)


def test_una_curva_por_cota_en_colina(colina):
    _, tin = tin_de(colina)
    curvas = tin.curvas_nivel([st.Z0 + 5, st.Z0 + 20])
    assert len(curvas) == 2 and all(c.cerrada for c in curvas)
