"""PresuCraft: presupuesto por UC (suministro, montaje, transporte, GG, utilidad, IGV)."""

from pathlib import Path

import pandas as pd
import pytest

from suite_electrica.documento import Documento
from suite_electrica.presucraft import cli
from suite_electrica.presucraft.presupuesto import Presupuesto, calcular, cargar, metrado_linecraft

RAIZ = Path(__file__).resolve().parents[1] / "examples"


def test_ejemplo_a_mano(tmp_path):
    r = calcular(cargar(RAIZ / "presucraft" / "presupuesto.json"))
    t = r.resumen.set_index("Rubro").loc["TOTAL"]
    assert t["Suministro"] == pytest.approx(18510)
    assert t["Montaje"] == pytest.approx(9860)
    assert t["Transporte"] == pytest.approx(9387.7 * 0.3, abs=0.01)
    cd = 18510 + 9860 + 2816.31
    assert t["Costo directo"] == pytest.approx(cd, abs=0.01)
    assert t["Total"] == pytest.approx(cd * 1.18 * 1.18, abs=0.05)  # (1 + 10 % + 8 %) · 1.18
    assert any("EJEMPLO" in a for a in r.avisos)
    pl = r.planilla_uc.set_index("UC")
    assert pl.at["PA-1", "Suministro unit."] == 510 and pl.at["PA-1", "Total"] == pytest.approx(2 * (510 + 150 + 32 * 0.3))
    assert cli.main(["calcular", str(RAIZ / "presucraft" / "presupuesto.json"), "--out", str(tmp_path)]) == 0
    assert pd.ExcelFile(tmp_path / "presupuesto.xlsx").sheet_names == [
        "Resumen", "Suministro", "Montaje", "PlanillaUC", "Avisos"]


def test_rubros_y_validaciones():
    sm = pd.DataFrame([{"codigo": "A", "precio": 10, "peso_kg": 1, "fuente": "x"},
                       {"codigo": "B", "fuente": "x"}])
    uc = pd.DataFrame([{"uc": "U1", "suministro": "A", "cantidad": 2}, {"uc": "U2", "suministro": "B", "cantidad": 1}])
    me = pd.DataFrame([{"rubro": "RP", "uc": "U1", "cantidad": 3}, {"rubro": "RS", "uc": "U1", "cantidad": 1},
                       {"rubro": "RS", "uc": "U2", "cantidad": 1}, {"rubro": "RS", "uc": "UX", "cantidad": 1}])
    r = calcular(Presupuesto("t", sm, uc, pd.DataFrame(columns=["uc"]), me))
    res = r.resumen.set_index("Rubro")["Suministro"]
    assert res.to_dict() == {"RP": 60, "RS": 20, "TOTAL": 80}
    assert any("UX" in a for a in r.avisos) and any("sin precio" in a for a in r.avisos)
    with pytest.raises(ValueError, match="no definidos"):
        calcular(Presupuesto("t", sm.iloc[:1], uc, pd.DataFrame(columns=["uc"]), me))


def test_metrado_desde_linecraft():
    d = Documento()
    d.importar_puntos(RAIZ / "colina_gaussiana.csv")
    d.importar_eje(RAIZ / "eje.dxf")
    r = d.calcular()
    m = metrado_linecraft(r.metrado)
    assert set(m["rubro"]) == {"LP"}
    assert {"PS1-3", "PTV-3", "AAAC 35"} <= set(m["uc"])
    km = m.loc[m["uc"] == "AAAC 35", "cantidad"].iloc[0]
    cond = r.metrado[(r.metrado["Ítem"] == "Conductor") & (r.metrado["Unidad"] == "m")]
    assert km == pytest.approx(cond["Cantidad"].iloc[0] / 1000)
    ca = m.loc[m["uc"].str.startswith("ca "), "cantidad"]  # alambre de amarre: sigue en m
    assert len(ca) and (ca > 1).all()
