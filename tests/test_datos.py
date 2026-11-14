import numpy as np
import pandas as pd

from suite_electrica.topocraft import datos
from suite_electrica.topocraft import sinteticos as st


def test_csv_con_encabezado_y_acentos(tmp_path):
    f = tmp_path / "p.csv"
    f.write_text("Punto,Este,Norte,Cota,Descripción\n1,390000.0,8248000.0,3820.5,TN\n2,390010,8248000,3821,BM\n",
                 encoding="utf-8")
    df = datos.leer(f)
    assert list(df.columns) == datos.COLUMNAS
    assert df.cota.tolist() == [3820.5, 3821.0]
    assert df.descripcion.tolist() == ["TN", "BM"]


def test_csv_punto_y_coma_coma_decimal_sin_encabezado(tmp_path):
    f = tmp_path / "p.txt"
    f.write_text("A1;390000,25;8248000,50;3820,125;TN\nA2;390010;8248000;3821;\n", encoding="utf-8")
    df = datos.leer(f)
    assert df.punto.tolist() == ["A1", "A2"]
    assert df.este.iloc[0] == 390000.25 and df.cota.iloc[0] == 3820.125


def test_limpieza_duplicados_conflictos_invalidos():
    df = pd.DataFrame({
        "punto": ["1", "2", "3", "4", "1"],
        "este": [390000.0, 390000.0, 390000.0004, 390010.0, 390020.0],
        "norte": [8248000.0] * 5,
        "cota": [100.0, 100.0, 101.0, np.nan, 100.0],
        "descripcion": "",
    })
    limpio, rep = datos.limpiar(df)
    assert rep.invalidos == ["4"]
    assert rep.duplicados == ["2"]
    assert rep.conflictos == ["3"]
    assert rep.nombres_repetidos == ["1"]
    assert len(limpio) == rep.n_salida == 2
    assert "Puntos leídos: 5" in rep.resumen()


def test_atipico_detectado_solo_el_pico():
    df = st.malla(st.plano_inclinado(), 10)
    i = 1300
    df.loc[i, "cota"] += 50.0
    limpio, rep = datos.limpiar(df, quitar_atipicos=True)
    assert rep.atipicos == [df.punto[i]]
    assert len(limpio) == len(df) - 1


def test_picos_y_pozos_de_5m():
    df = st.malla(st.colina_gaussiana(), 10)
    df.loc[200, "cota"] -= 5.0
    df.loc[1300, "cota"] += 5.0  # cima de la colina
    _, rep = datos.limpiar(df)
    assert sorted(rep.atipicos) == sorted([df.punto[200], df.punto[1300]])


def test_sin_falsos_atipicos_en_terreno_suave():
    for sup in (st.plano_inclinado(), st.colina_gaussiana(), st.colina_gaussiana(60, 40),
                st.cono(), st.cono(100, 0.6)):
        _, rep = datos.limpiar(st.malla(sup, 10))
        assert rep.atipicos == []


def test_csv_ida_y_vuelta(tmp_path):
    df = st.malla(st.colina_gaussiana(), 50)
    datos.guardar_csv(df, tmp_path / "x.csv")
    df2 = datos.leer(tmp_path / "x.csv")
    assert np.allclose(df.cota, df2.cota, atol=5e-4)


def test_eje_csv_con_y_sin_encabezado(tmp_path):
    (tmp_path / "a.csv").write_text("este,norte\n1,2\n3,4\n")
    (tmp_path / "b.csv").write_text("1;2\n3;4\n")
    for f in ("a.csv", "b.csv"):
        assert datos.leer_eje_csv(tmp_path / f).tolist() == [[1, 2], [3, 4]]


def test_csv_pnezd_estacion_total(tmp_path):
    # Formato de estación total / TOPOMAGIC: punto, norte, este, cota, descripción.
    p = tmp_path / "et.csv"
    p.write_text("1,8437165.837,797638.292,3783.341,.K-0\n2,8437177.219,797656.193,3785.041,J\n")
    df = datos.leer_csv(p)
    assert df["este"].tolist() == [797638.292, 797656.193]
    assert df["norte"].iloc[0] == 8437165.837


def test_unidades_y_zona():
    import pytest
    df = st.malla(st.plano_inclinado(), 50)
    df.loc[0, ["este", "norte"]] = [39000.0, 8248000.0]  # este con un dígito de menos
    df.loc[1, "norte"] = -5.0
    df.loc[2, "cota"] = 3820000.0  # mm
    limpio, rep = datos.limpiar(df)
    assert rep.fuera_zona == [df.punto[0], df.punto[1]]
    assert rep.cotas_fuera_rango == [df.punto[2]]
    assert len(limpio) == len(df) - 2 and "fuera de zona UTM: 2" in rep.resumen()
    geo = df.assign(este=-70.0 + df.este * 1e-6, norte=-15.8 + df.norte * 1e-8)
    with pytest.raises(ValueError, match="geográficas"):
        datos.limpiar(geo)
