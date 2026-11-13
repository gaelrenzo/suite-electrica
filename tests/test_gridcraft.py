"""GridCraft: flujo radial por sumatoria de potencias, validaciones y salidas."""

import math
from pathlib import Path

import pandas as pd
import pytest

from suite_electrica.gridcraft import cli
from suite_electrica.gridcraft.red import Red, calcular, cargar

EJ = Path(__file__).resolve().parents[1] / "examples" / "gridcraft"
COND = pd.DataFrame([{"formacion": "A", "r_ohm_km": 1.0, "x_ohm_km": 0.1, "corriente_max_A": 20,
                      "fuente": "prueba"}])


def red(nodos, **kw):
    base = dict(nombre="t", sistema="3F", tension_V=380, nodos=pd.DataFrame(nodos), conductores=COND,
                calificacion_kw_lote=1.0, fs_sp=1.0, fp_sp=0.9,
                fuentes={k: "prueba" for k in ("calificacion_kw_lote", "fs_sp", "caida_max_pct", "conductores")})
    base.update(kw)
    return Red(**base)


LINEA = [{"nodo": "S", "padre": ""},
         {"nodo": "A", "padre": "S", "longitud_m": 100, "conductor": "A", "lotes": 2},
         {"nodo": "B", "padre": "A", "longitud_m": 100, "conductor": "A", "lotes": 3}]


def test_caida_a_mano():
    r = calcular(red(LINEA))
    t = r.tramos.set_index("Nodo")
    K = math.sqrt(3) * (1.0 * 0.9 + 0.1 * math.sqrt(1 - 0.81))
    I_a = 5000 / (math.sqrt(3) * 380 * 0.9)
    I_b = 3000 / (math.sqrt(3) * 380 * 0.9)
    assert t.at["A", "I (A)"] == pytest.approx(I_a, abs=0.01)
    dv = K * I_a * 0.1 + K * I_b * 0.1
    assert t.at["B", "%ΔV"] == pytest.approx(100 * dv / 380, abs=1e-3)
    assert r.resumen["Pérdidas (kW)"] == pytest.approx(3 * (I_a**2 + I_b**2) * 0.1 / 1e3, abs=1e-4)  # redondeo a 4 decimales
    assert r.cumple and r.avisos == []


def test_k_de_catalogo_y_monofasico():
    cond = COND.assign(k_ohm_km=2.5)
    r = calcular(red(LINEA, conductores=cond, sistema="1F", tension_V=220))
    t = r.tramos.set_index("Nodo")
    I_a = 5000 / (220 * 0.9)
    assert t.at["A", "K (Ω/km)"] == 2.5
    assert t.at["A", "ΔV tramo (V)"] == pytest.approx(2.5 * I_a * 0.1, abs=1e-3)


def test_tabla_fs_y_alertas():
    r = calcular(red(LINEA, tabla_fs=[(1, 0.8), (5, 0.5)], caida_max_pct=0.1, transformador_kva=1))
    assert r.resumen["Factor de simultaneidad SP"] == 0.5  # 5 abonados
    assert {"DV", "TR"} <= set(r.alertas["Código"]) and not r.cumple
    assert r.alertas["Exceso (%)"].is_monotonic_decreasing


def test_corriente_maxima():
    nodos = [dict(LINEA[0]), dict(LINEA[1], lotes=20)]
    r = calcular(red(nodos))
    assert "I" in set(r.alertas["Código"])


def test_topologia_invalida():
    with pytest.raises(ValueError, match="una sola raíz"):
        calcular(red([{"nodo": "S", "padre": ""}, {"nodo": "T", "padre": ""}]))
    with pytest.raises(ValueError, match="inexistentes"):
        calcular(red([{"nodo": "S", "padre": ""}, {"nodo": "A", "padre": "X", "conductor": "A"}]))
    with pytest.raises(ValueError, match="catálogo"):
        calcular(red([{"nodo": "S", "padre": ""}, {"nodo": "A", "padre": "S", "conductor": "Z"}]))


def test_ejemplo_cli(tmp_path):
    assert cli.main(["calcular", str(EJ / "red.json"), "--out", str(tmp_path)]) == 0
    assert (tmp_path / "plano_red.dxf").exists()
    hojas = pd.ExcelFile(tmp_path / "caida_tension.xlsx").sheet_names
    assert hojas == ["Resumen", "CaidaTension", "Alertas", "Avisos"]
    r = calcular(cargar(EJ / "red.json"))
    assert r.resumen["M.D. SP (kW)"] == pytest.approx(37 * 0.5 * 0.5 + 1.5)
    assert any("EJEMPLO" in a for a in r.avisos)
    assert cli.main(["nuevo", str(tmp_path / "nueva")]) == 0
    assert (tmp_path / "nueva" / "red.json").exists()
