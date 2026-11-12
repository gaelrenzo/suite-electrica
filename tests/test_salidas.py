import ezdxf
import numpy as np
import pandas as pd
import pytest

from conftest import tin_de
from core.dxf.lectura import leer_polilinea, leer_puntos
from core.geo.perfil import perfil_longitudinal
from core.geo.tin import niveles
from core.units import formato_progresiva
from topocraft import salidas
from topocraft import sinteticos as st
from topocraft.cli import main


def _audit(ruta):
    doc = ezdxf.readfile(ruta)
    assert not doc.audit().has_errors
    return doc


def test_formato_progresiva():
    assert formato_progresiva(0) == "0+000.00"
    assert formato_progresiva(1250.5) == "1+250.50"
    assert formato_progresiva(999.999) == "1+000.00"
    assert formato_progresiva(20, 0) == "0+020"
    with pytest.raises(ValueError):
        formato_progresiva(-1)


def test_plano_dxf_capas_y_puntos(tmp_path, colina):
    df, tin = tin_de(colina, 25)
    curvas = tin.curvas_nivel(niveles(df.cota.min(), df.cota.max(), 1))
    ruta = salidas.plano_dxf(tmp_path / "plano.dxf", df, tin, curvas, 1, 5, st.eje_ejemplo())
    doc = _audit(ruta)
    capas = {l.dxf.name for l in doc.layers}
    assert {"PUNTOS", "TIN", "CURVAS_MAYORES", "CURVAS_MENORES", "EJE"} <= capas
    msp = doc.modelspace()
    assert len(msp.query("POINT")) == len(df)
    assert len(msp.query("3DFACE")) == len(tin.triangulos_activos)
    assert len(msp.query('LWPOLYLINE[layer=="CURVAS_MAYORES"]')) > 0
    assert doc.units == ezdxf.units.M
    xyz = leer_puntos(ruta, capa="PUNTOS")
    assert np.allclose(np.sort(xyz[:, 2]), np.sort(df.cota))
    assert np.allclose(leer_polilinea(ruta), st.eje_ejemplo())


def test_perfil_pdf_excel_dxf(tmp_path, colina):
    _, tin = tin_de(colina, 20)
    pf = perfil_longitudinal(tin, st.eje_ejemplo(), 20)
    eh, ev = salidas.perfil_pdf(tmp_path / "p.pdf", pf)
    assert (tmp_path / "p.pdf").read_bytes()[:4] == b"%PDF"
    assert pf.longitud / eh * 1000 <= 900 and ev == eh // 10
    salidas.perfil_excel(tmp_path / "p.xlsx", pf)
    tabla = pd.read_excel(tmp_path / "p.xlsx", sheet_name="Perfil")
    assert np.allclose(tabla["Cota terreno (m)"], pf.cota, atol=5e-4)
    assert tabla["Progresiva"].iloc[0] == "0+000.00"
    doc = _audit(salidas.perfil_dxf(tmp_path / "p.dxf", pf))
    assert "PERFIL" in {l.dxf.name for l in doc.layers}


def test_escala_automatica():
    assert salidas.escala_automatica(400) == 500
    assert salidas.escala_automatica(1500) == 2000
    assert salidas.escala_automatica(10_000) == 20000


def test_cli_curvas_y_perfil(tmp_path, colina, capsys):
    csv = tmp_path / "pts.csv"
    st.malla(colina, 20).to_csv(csv, index=False)
    eje_dxf = tmp_path / "eje.dxf"
    doc = ezdxf.new()
    doc.layers.add("EJE")
    doc.modelspace().add_lwpolyline(st.eje_ejemplo().tolist(), dxfattribs={"layer": "EJE"})
    doc.saveas(eje_dxf)

    assert main(["curvas", str(csv), "--eq", "1", "--out", str(tmp_path / "plano.dxf")]) == 0
    _audit(tmp_path / "plano.dxf")
    outs = [str(tmp_path / f"perfil.{e}") for e in ("pdf", "xlsx", "dxf")]
    assert main(["perfil", str(csv), "--eje", str(eje_dxf), "--out", *outs]) == 0
    assert all((tmp_path / f"perfil.{e}").stat().st_size > 0 for e in ("pdf", "xlsx", "dxf"))
    assert main(["validar", str(csv)]) == 0
    assert main(["curvas", str(tmp_path / "no.csv"), "--out", str(tmp_path / "x.dxf")]) == 1
    assert "Puntos leídos" in capsys.readouterr().out
