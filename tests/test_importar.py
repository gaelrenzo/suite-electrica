"""Importación KML, GPX, MapSource TXT, Excel RLT y perfiles TPG / acumulada-ángulo."""

import numpy as np
import pandas as pd
import pytest

from suite_electrica.core.geo.utm import geograficas_a_utm
from suite_electrica.documento import Documento
from suite_electrica.linecraft.estructuras import angulos_eje
from suite_electrica.topocraft import datos
from suite_electrica.topocraft import importar as im

KML = """<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document><Folder>
<Placemark><name>P1</name><Point><coordinates>-70.0,-15.8,3850</coordinates></Point></Placemark>
<Placemark><name>Ruta1</name><LineString><coordinates>
 -70.0,-15.8,0 -70.001,-15.801,0 -70.002,-15.8,0
</coordinates></LineString></Placemark></Folder></Document></kml>"""

GPX = """<?xml version="1.0"?><gpx xmlns="http://www.topografix.com/GPX/1/1" version="1.1">
<wpt lat="-15.8" lon="-70.0"><ele>3850.5</ele><name>AN-0</name><cmt>poste</cmt></wpt>
<rte><name>R1</name><rtept lat="-15.8" lon="-70.0"><ele>3850</ele></rtept>
<rtept lat="-15.81" lon="-70.0"><ele>3860</ele></rtept></rte></gpx>"""


def test_kml_puntos_y_rutas(tmp_path):
    f = tmp_path / "a.kml"
    f.write_text(KML, encoding="utf-8")
    df, rutas = im.importar(f)
    e, n = geograficas_a_utm([-70.0], [-15.8], "19S")
    assert df["punto"].tolist() == ["P1"] and df["cota"].iloc[0] == 3850
    assert df["este"].iloc[0] == pytest.approx(e[0], abs=1e-6)
    assert rutas[0][0] == "Ruta1" and rutas[0][1].shape == (3, 3)
    assert np.isnan(rutas[0][1][:, 2]).all()  # altitud 0 de Google Earth = sin cota


def test_gpx_y_eje_en_documento(tmp_path):
    f = tmp_path / "a.gpx"
    f.write_text(GPX, encoding="utf-8")
    df = datos.leer(f)
    assert df["punto"].tolist() == ["AN-0"] and df["cota"].iloc[0] == 3850.5
    d = Documento()
    d.importar_eje(f)
    assert d.eje.shape == (2, 2)
    assert np.hypot(*(d.eje[1] - d.eje[0])) == pytest.approx(1106, rel=0.01)  # 0.01° lat ≈ 1.1 km
    with pytest.raises(ValueError, match="sin rutas"):
        d.importar_eje(f, "NO_EXISTE")


def test_mapsource_txt(tmp_path):
    f = tmp_path / "m.txt"
    f.write_text("Grid\tUTM\nDatum\tWGS 84\n\nHeader\tName\tDescription\tType\tPosition\tAltitude\n\n"
                 "Waypoint\t001\tdesc\tUser Waypoint\t18 M 199397 9150498\t2737 m\t\n", encoding="latin-1")
    df = datos.leer(f)
    assert df.iloc[0][["este", "norte", "cota"]].tolist() == [199397, 9150498, 2737]


def test_angulo_dms():
    assert im.angulo_grados("-21°17'44\"") == pytest.approx(-(21 + 17 / 60 + 44 / 3600))
    assert im.angulo_grados("0°") == 0 and np.isnan(im.angulo_grados("-"))
    assert im.angulo_grados(" 1.5E+0001") == 15


def test_tpg_reconstruye_deflexion(tmp_path):
    f = tmp_path / "p.tpg"
    f.write_text("Acumulada(m)\tCota(m)\tAtributo\tAngulo(°g)\tEtiqueta\n"
                 "0\t100\t0\t0\tvA\n50\t101\t0\t0\t-\n100\t102\t0\t30\tvB\n200\t99\t0\t0\tvC\n",
                 encoding="latin-1")
    p, eje = im.leer_perfil_tabular(f)
    assert p.longitud == 200 and len(eje) == 3
    assert angulos_eje(eje)[1] == pytest.approx(30, abs=1e-6)
    assert p.cota_en(75) == pytest.approx(101.5)


def test_excel_rlt_utm_y_acumulada(tmp_path):
    utm = [["RLT", 3, "Formato:", "UTM", None, None, None, None, None],
           ["Nombre de Ruta", "Zona", "Banda", "X", "Y", "Altitud", "Vértice", "Comentario", "Superficie"],
           ["L1", 19, "L", 0.0, 0.0, 10.0, "P-0", None, None],
           [None, 19, "L", 0.0, 50.0, 11.0, None, "TN", None],
           [None, 19, "L", 0.0, 100.0, 12.0, "V-1", None, None],
           [None, 19, "L", 100.0, 100.0, 13.0, "V-2", None, None]]
    f = tmp_path / "utm.xlsx"
    pd.DataFrame(utm).to_excel(f, header=False, index=False)
    df, rutas = im.importar(f)
    assert len(df) == 4 and rutas[0][0] == "L1" and len(rutas[0][1]) == 3
    assert angulos_eje(rutas[0][1][:, :2])[1] == pytest.approx(-90)  # giro a la derecha
    acu = [["RLT", 3, "Formato:", "ACUMULADA/ANGULO"],
           ["Nombre de Ruta", "Acumulada", "Altitud", "Ángulo", "Vértice"],
           ["L1", 0, 10, "0°", "P-0"], [None, 50, 11, None, None],
           [None, 100, 12, "-90°0'0\"", "V-1"], [None, 200, 13, None, "V-2"]]
    g = tmp_path / "acu.xlsx"
    pd.DataFrame(acu).to_excel(g, header=False, index=False)
    p, eje = im.leer_perfil_tabular(g)
    assert angulos_eje(eje)[1] == pytest.approx(-90) and p.longitud == 200
