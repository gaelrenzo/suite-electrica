import math

import numpy as np
import pytest

from suite_electrica.linecraft.conductor import G, Conductor, Hipotesis, carga_unitaria
from suite_electrica.linecraft.mecanica import (
    Catenaria, Parabola, cambio_estado, curva, vano_regulador,
)

# AAAC 35 mm² del proyecto Capachica (memoria_calculo_mecanico.md, sección 1)
AAAC35 = Conductor("AAAC 35", 34.36e-6, 7.5e-3, 0.0945 * G, 1040 * G, 6000 * G * 1e6, 23e-6)


def test_cambio_estado_sin_cambio_devuelve_mismo_esfuerzo():
    s1 = 50e6
    assert cambio_estado(s1, 1.0, 15, 1.0, 15, 100, AAAC35) == pytest.approx(s1, rel=1e-9)


@pytest.mark.parametrize("w_kgf_m, t2, tiro_kgf", [
    (0.2008, 10, 253.62),   # I viento máximo
    (0.3434, -5, 358.25),   # II hielo
    (0.0945, -15, 310.81),  # III temperatura mínima
    (0.0945, 35, 126.83),   # IV temperatura máxima
])
def test_validacion_capachica(w_kgf_m, t2, tiro_kgf):
    """Reproduce la tabla de hipótesis del proyecto Capachica (vano 115.59 m, EDS 18 %)."""
    s1 = 187.20 * G / AAAC35.seccion
    s2 = cambio_estado(s1, 0.0945 * G, 15, w_kgf_m * G, t2, 115.59, AAAC35)
    assert s2 * AAAC35.seccion / G == pytest.approx(tiro_kgf, abs=0.05)


def test_cambio_estado_balance_de_longitudes():
    """Verificación independiente: ΔL geométrico = ΔL térmico + ΔL elástico (parábola)."""
    a, w1, w2, t1, t2 = 150.0, 1.0, 2.5, 15.0, -5.0
    s1 = 40e6
    s2 = cambio_estado(s1, w1, t1, w2, t2, a, AAAC35)
    A, E, al = AAAC35.seccion, AAAC35.modulo_elasticidad, AAAC35.coef_dilatacion
    geom = a**3 * w2**2 / (24 * (s2 * A) ** 2) - a**3 * w1**2 / (24 * (s1 * A) ** 2)
    fisico = a * al * (t2 - t1) + a * (s2 - s1) / E
    assert geom == pytest.approx(fisico, rel=1e-6)


def test_mas_temperatura_menos_tiro():
    s1 = 40e6
    t = [cambio_estado(s1, 1, 15, 1, x, 120, AAAC35) for x in (-10, 15, 40)]
    assert t[0] > t[1] > t[2]


def test_vano_regulador():
    assert vano_regulador([100, 200]) == pytest.approx(math.sqrt(9e6 / 300))
    assert vano_regulador([80, 80, 80]) == pytest.approx(80)
    with pytest.raises(ValueError):
        vano_regulador([])


def test_catenaria_nivelada():
    a, H, w = 120.0, 2000.0, 1.0
    c = Catenaria(a, 0, H, w)
    C = H / w
    assert c.x_vertice == pytest.approx(a / 2)
    assert c.flecha == pytest.approx(C * (math.cosh(a / (2 * C)) - 1))
    assert c.tiro_maximo == pytest.approx(H + w * c.flecha)
    assert c.longitud == pytest.approx(2 * C * math.sinh(a / (2 * C)))
    assert c.flecha == pytest.approx(Parabola(a, 0, H, w).flecha, rel=1e-3)


@pytest.mark.parametrize("modelo", ["catenaria", "parabola"])
def test_curva_pasa_por_soportes(modelo):
    c = curva(modelo, 110.0, -25.0, 1500.0, 0.93)
    assert c.y(0) == pytest.approx(0, abs=1e-9)
    assert c.y(110.0) == pytest.approx(-25.0, abs=1e-9)
    x = np.linspace(0, 110, 2001)
    assert c.flecha == pytest.approx(np.max(-25 * x / 110 - c.y(x)), abs=1e-4)
    assert c.y(c.x_vertice) <= c.y(np.clip(c.x_vertice + np.array([-1, 1]), -1e9, 1e9)).min() + 1e-9


def test_catenaria_inclinada_aprox_parabola():
    cat, par = Catenaria(100, 20, 3000, 1.0), Parabola(100, 20, 3000, 1.0)
    assert cat.x_vertice == pytest.approx(par.x_vertice, rel=0.01)  # h/a = 0.2
    assert cat.flecha == pytest.approx(par.flecha, rel=0.03)


def test_flecha_parabolica_capachica():
    assert Parabola(115.59, 0, 126.83 * G, 0.0945 * G).flecha == pytest.approx(1.244, abs=1e-3)


def test_cargas_unitarias_capachica():
    viento = Hipotesis("I", 10, presion_viento=0.0042 * 75**2 * G)
    assert carga_unitaria(AAAC35, viento).resultante / G == pytest.approx(0.2008, abs=2e-4)
    hielo = Hipotesis("II", -5, 0.0042 * 37.5**2 * G, 0.006, 900)
    assert carga_unitaria(AAAC35, hielo).resultante / G == pytest.approx(0.3434, abs=5e-4)
    with pytest.raises(ValueError):
        carga_unitaria(AAAC35, Hipotesis("x", 0, 0, 0.006, 0))


def test_flecha_medio_vano_y_maxima():
    # Criterio REDLIN: flecha a medio vano. En vano a nivel coincide con la máxima;
    # en vano inclinado la máxima (paralela a la cuerda) es mayor.
    from suite_electrica.linecraft.mecanica import Catenaria
    c0 = Catenaria(300.0, 0.0, 1200.0, 1.0)
    assert c0.flecha == pytest.approx(-float(c0.y(150.0)), rel=1e-12)
    c = Catenaria(300.0, 60.0, 1200.0, 1.0)
    f_medio = 30.0 - float(c.y(150.0))
    assert c.flecha > f_medio and c.flecha == pytest.approx(f_medio, rel=0.03)


def test_cambio_estado_inclinado():
    from suite_electrica.linecraft.conductor import Conductor
    from suite_electrica.linecraft.mecanica import cambio_estado
    c = Conductor("AAAC 35", 34.36e-6, 7.5e-3, 0.094 * 9.80665, 1056.12 * 9.80665, 6206.12 * 9.80665e6, 2.3e-5)
    s1 = 0.18 * c.carga_rotura / c.seccion
    # sin cambio de estado devuelve σ1
    assert cambio_estado(s1, c.peso, 25, c.peso, 25, 120, c, desnivel=30) == pytest.approx(s1, rel=1e-6)
    # desnivel → 0 converge a la cúbica de vano a nivel
    nivel = cambio_estado(s1, c.peso, 25, c.peso, 40, 300, c)
    assert cambio_estado(s1, c.peso, 25, c.peso, 40, 300, c, desnivel=1e-3) == pytest.approx(nivel, rel=2e-3)
    # vano inclinado: al calentar el tiro cae más que en vano a nivel (mismo sentido que REDLIN)
    incl = cambio_estado(s1, c.peso, 25, c.peso, 40, 300, c, desnivel=150)
    assert incl < nivel


def test_deformacion_creep_alcoa():
    from dataclasses import replace

    from suite_electrica.linecraft.conductor import Conductor, curva
    from suite_electrica.linecraft.mecanica import KG_MM2, _deformacion, cambio_estado, deformacion_creep
    base = Conductor("X", 34.36e-6, 7.5e-3, 0.094 * 9.80665, 1056.12 * 9.80665, 6206.12 * KG_MM2, 2.3e-5)
    s1, w = 0.18 * base.carga_rotura / base.seccion, base.peso
    assert deformacion_creep(base, s1, w, 25, 200) == 0.0  # sin curva de creep
    assert curva("0;55,6; 25.4;-85.8;0") == (0.0, 55.6, 25.4, -85.8, 0.0) and curva(None) == ()
    # curva inicial lineal con el módulo E (sin término inicial) y creep lineal más blando
    c = replace(base, curva_inicial=(0.0, 62.0612), curva_creep=(0.0, 38.29))
    de = deformacion_creep(c, s1, w, 25, 200)
    assert de > 0
    # punto fijo: el esfuerzo final cumple Δε = ε_c(σf) − σf/E
    sf = cambio_estado(s1, w, 25, w, 25 + de / c.coef_dilatacion, 200, c) / KG_MM2
    assert de == pytest.approx(_deformacion(c.curva_creep, sf) / 100 - sf / 6206.12, rel=1e-6)
    assert sf < s1 / KG_MM2  # el creep afloja el conductor
