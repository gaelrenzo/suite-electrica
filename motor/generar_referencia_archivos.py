"""Archivos de prueba por formato (válidos y problemáticos) y lo que Python lee de ellos.

Crea ``motor/tests/archivos/*`` (sintéticos, publicables) y ``motor/tests/archivos_esperado.txt``
con, por archivo: puntos leídos, rutas, perfil tabular, reporte de limpieza o el error esperado.

Uso: ``python motor/generar_referencia_archivos.py``
"""

import math
import sys
from pathlib import Path

import ezdxf
import numpy as np
import pandas as pd

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ))

from suite_electrica.core.dxf.lectura import leer_polilinea  # noqa: E402
from suite_electrica.topocraft import datos  # noqa: E402
from suite_electrica.topocraft import importar as im  # noqa: E402

A = Path(__file__).parent / "tests" / "archivos"
E0, N0 = 390_000.0, 8_248_000.0


def f(x) -> str:
    return "nan" if x is None or (isinstance(x, float) and math.isnan(x)) else f"{float(x):.17e}"


def malla(n: int = 12, paso: float = 15.0, semilla: int = 3) -> pd.DataFrame:
    rng = np.random.default_rng(semilla)
    g = np.array([(i, j) for i in range(n) for j in range(n)], dtype=float) * paso
    xy = g + rng.uniform(-4, 4, g.shape)
    z = 3800 + 0.04 * xy[:, 0] - 0.02 * xy[:, 1] + 6 * np.exp(-((xy[:, 0] - 80) ** 2 + (xy[:, 1] - 90) ** 2) / 3000)
    return pd.DataFrame({"punto": [f"P{k + 1}" for k in range(len(g))], "este": E0 + xy[:, 0],
                         "norte": N0 + xy[:, 1], "cota": z, "descripcion": "TN"})


def crear() -> None:
    A.mkdir(parents=True, exist_ok=True)
    m = malla()
    # --- CSV
    m.rename(columns={"este": "X", "norte": "Y", "cota": "Elevación"}).to_csv(A / "csv_encabezado.csv", index=False)
    m[["punto", "norte", "este", "cota", "descripcion"]].to_csv(A / "csv_pnezd.csv", index=False, header=False)
    with open(A / "csv_puntoycoma.csv", "w", encoding="utf-8") as fh:
        for r in m.head(20).itertuples():
            fh.write(f"{r.punto};{r.este:.3f};{r.norte:.3f};{r.cota:.3f};{r.descripcion}\n".replace(".", ","))
    p = m.copy()
    p.loc[len(p)] = ["D1", p.este[5], p.norte[5], p.cota[5], "duplicado"]
    p.loc[len(p)] = ["C1", p.este[6], p.norte[6], p.cota[6] + 2.0, "conflicto"]
    p.loc[len(p)] = ["V1", E0 + 33.3, N0 + 44.4, math.nan, "cota vacía"]
    p.loc[len(p)] = ["Z1", 39_000.0, N0 + 10.0, 3800.0, "este con un dígito menos"]
    p.loc[len(p)] = ["MM", E0 + 61.0, N0 + 52.0, 3_801_234.0, "cota en mm"]
    p.loc[70, "cota"] += 9.0  # pico atípico
    p.loc[len(p)] = ["P10", E0 + 170.0, N0 + 3.0, 3806.0, "nombre repetido"]
    p.to_csv(A / "csv_problemas.csv", index=False)
    with open(A / "csv_problemas.csv", "a", encoding="utf-8") as fh:
        fh.write("B1,,8248100,3801,campos vacíos\n")
    g = m.head(30).copy()
    g["este"], g["norte"] = -70.0 + (g.este - E0) * 1e-5, -15.8 + (g.norte - N0) * 1e-5
    g.to_csv(A / "csv_grados.csv", index=False)
    # --- DXF
    doc = ezdxf.new("R2010")
    msp = doc.modelspace()
    for r in m.head(60).itertuples():
        msp.add_point((r.este, r.norte, r.cota), dxfattribs={"layer": "PUNTOS"})
    msp.add_polyline3d([(r.este, r.norte, r.cota) for r in m.iloc[60:90].itertuples()], dxfattribs={"layer": "BORDE"})
    lw = msp.add_lwpolyline([(r.este, r.norte) for r in m.iloc[90:100].itertuples()], dxfattribs={"layer": "CURVA"})
    lw.dxf.elevation = 3805.0
    msp.add_point((m.este[0], m.norte[0], m.cota[0]), dxfattribs={"layer": "PUNTOS"})  # duplicado
    doc.saveas(A / "puntos.dxf")
    doc = ezdxf.new("R2010")
    msp = doc.modelspace()
    msp.add_lwpolyline([(E0, N0), (E0 + 100, N0 + 5)], dxfattribs={"layer": "OTRA"})
    msp.add_lwpolyline([(E0 + 10, N0 + 20), (E0 + 90, N0 + 60), (E0 + 160, N0 + 150)], dxfattribs={"layer": "EJE"})
    doc.saveas(A / "eje.dxf")
    doc = ezdxf.new("R2010")
    doc.modelspace().add_line((0, 0), (1, 1))
    doc.saveas(A / "sin_puntos.dxf")
    # --- KML
    (A / "puntos.kml").write_text("""<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document><Folder>
<Placemark><name>V-0</name><Point><coordinates>-70.0012,-15.8015,3850.5</coordinates></Point></Placemark>
<Placemark><name>SinCota</name><Point><coordinates>-70.0020,-15.8010,0</coordinates></Point></Placemark>
<Placemark><name>Ruta1,AB</name><LineString><coordinates>
 -70.0,-15.8,0 -70.001,-15.801,0 -70.002,-15.8005,3851
</coordinates></LineString></Placemark>
<Placemark><name>Ruta2</name><LineString><coordinates>-70.003,-15.802 -70.004,-15.803</coordinates></LineString></Placemark>
</Folder></Document></kml>""", encoding="utf-8")
    # --- GPX
    (A / "puntos.gpx").write_text("""<?xml version="1.0"?><gpx xmlns="http://www.topografix.com/GPX/1/1" version="1.1">
<wpt lat="-15.8" lon="-70.0"><ele>3850.5</ele><name>AN-0</name><cmt>poste</cmt></wpt>
<wpt lat="-15.801" lon="-70.0005"><name>SinEle</name><desc>sin cota</desc></wpt>
<rte><name>R1</name><rtept lat="-15.8" lon="-70.0"><ele>3850</ele></rtept>
<rtept lat="-15.81" lon="-70.0"><ele>3860</ele></rtept></rte>
<trk><name>T1</name><trkseg><trkpt lat="-15.805" lon="-70.001"><ele>3855</ele></trkpt>
<trkpt lat="-15.806" lon="-70.002"/></trkseg></trk></gpx>""", encoding="utf-8")
    # --- MapSource
    cab = "Grid\tUTM\nDatum\tWGS 84\n\nHeader\tName\tDescription\tType\tPosition\tAltitude\n\n"
    (A / "mapsource.txt").write_text(cab + "".join(
        f"Waypoint\t{r.punto}\t{r.descripcion}\tUser Waypoint\t18 L {r.este:.0f} {r.norte:.0f}\t{r.cota:.0f} m\t\n"
        for r in m.head(15).itertuples()) + "Waypoint\tSA\tsin altitud\tUser Waypoint\t18 L 390500 8248500\t\t\n",
        encoding="latin-1")
    (A / "mapsource_grados.txt").write_text(
        "Grid\tLat/Lon hddd.ddddd°\n\nWaypoint\t001\td\tUser Waypoint\tS15.80000 W70.00000\t3850 m\t\n", encoding="latin-1")
    # --- Excel RLT (UTM y acumulada/ángulo)
    utm = [["RLT", 3, "Formato:", "UTM", None, None, None, None, None],
           ["Nombre de Ruta", "Zona", "Banda", "X", "Y", "Altitud", "Vértice", "Comentario", "Superficie"],
           ["L1", 19, "L", E0, N0, 3810.0, "P-0", None, None],
           [None, 19, "L", E0, N0 + 50, 3811.0, None, "TN", None],
           [None, 19, "L", E0, N0 + 100, None, None, "sin altitud", None],
           [None, 19, "L", E0, N0 + 100, 3812.0, "V-1", None, None],
           [None, 19, "L", E0 + 100, N0 + 100, 3813.0, "V-2", None, None],
           ["L2", 19, "L", E0 + 300, N0, 3820.0, "Q-0", None, None],
           [None, 19, "L", E0 + 300, N0 + 80, 3822.0, "Q-1", None, None]]
    pd.DataFrame(utm).to_excel(A / "rlt_utm.xlsx", header=False, index=False)
    acu = [["RLT", 3, "Formato:", "ACUMULADA/ANGULO"],
           ["Nombre de Ruta", "Acumulada", "Altitud", "Ángulo", "Vértice"],
           ["L1", 0, 3810, "0°", "P-0"], [None, 50, 3811, None, None],
           [None, 100, 3812, "-21°17'44\"", "V-1"], [None, 150, 3815.5, None, None],
           [None, 230, 3813, "35.5", "V-2"], [None, 300, 3814, None, "V-3"],
           ["L2", 0, 3900, "0°", "Q-0"], [None, 40, 3901, None, "Q-1"]]
    pd.DataFrame(acu).to_excel(A / "rlt_acumulada.xlsx", header=False, index=False)
    # --- TPG
    (A / "perfil.tpg").write_text("Acumulada(m)\tCota(m)\tAtributo\tAngulo(°g)\tEtiqueta\n"
                                  "0\t100\t0\t0\tvA\n50\t101\t0\t0\t-\n100\t102\t0\t30\tvB\n"
                                  "150.5\t99.25\t0\t0\t-\n200\t99\t0\t-12°30'\tvC\n260\t98\t0\t0\tvD\n",
                                  encoding="latin-1")
    (A / "perfil_sin_encabezado.tpg").write_text("0\t2489.8\t0\t0°\tvSPA-0\tSPA-0\n21.1\t2491.34\t0\t0°\t-\t0\n"
                                                 "33.52\t2492.37\t0\t15°\tvSPA-1\t0\n60\t2490\t0\t0°\tvSPA-2\t0\n",
                                                 encoding="latin-1")


def volcar_puntos(L: list[str], nombre: str, df: pd.DataFrame) -> None:
    for r in df.itertuples(index=False):
        L.append(f"pto {nombre} {r.punto}|{f(r.este)}|{f(r.norte)}|{f(r.cota)}|{r.descripcion}")


def main() -> None:
    crear()
    L: list[str] = []

    def caso(nombre: str, fn) -> None:
        try:
            fn(nombre)
        except ValueError as exc:  # sin la carpeta local en el mensaje
            L.append(f"error {nombre} {str(exc).replace(str(A) + chr(92), '').replace(str(A) + '/', '')}")

    def leer_y_limpiar(nombre: str) -> None:
        df = datos.leer(A / nombre)
        volcar_puntos(L, nombre, df)
        limpio, rep = datos.limpiar(df)
        for k in ("invalidos", "duplicados", "conflictos", "nombres_repetidos", "atipicos", "fuera_zona",
                  "cotas_fuera_rango"):
            L.append(f"rep {nombre} {k} {','.join(getattr(rep, k))}")
        L.append(f"rep {nombre} n_salida {rep.n_salida}")

    def rutas(nombre: str) -> None:
        df, rs = im.importar(A / nombre)
        volcar_puntos(L, nombre, df)
        for n, xyz in rs:
            L.append(f"ruta {nombre} {n.replace(' ', '_')} " + " ".join(f"{f(a)},{f(b)},{f(c)}" for a, b, c in xyz))

    def tabular(nombre: str) -> None:
        p, eje = im.leer_perfil_tabular(A / nombre)
        for s, z, t in zip(p.progresiva, p.cota, p.tipo):
            L.append(f"perfil {nombre} {f(s)} {f(z)} {t}")
        L.append(f"eje {nombre} " + " ".join(f"{f(a)},{f(b)}" for a, b in eje))

    for n in ("csv_encabezado.csv", "csv_pnezd.csv", "csv_puntoycoma.csv", "csv_problemas.csv", "csv_grados.csv",
              "puntos.dxf", "sin_puntos.dxf", "mapsource.txt", "mapsource_grados.txt"):
        caso(n, leer_y_limpiar)
    for n in ("puntos.kml", "puntos.gpx", "rlt_utm.xlsx"):
        caso(n, rutas)
    for n in ("rlt_acumulada.xlsx", "perfil.tpg", "perfil_sin_encabezado.tpg"):
        caso(n, tabular)
    eje = leer_polilinea(A / "eje.dxf")
    L.append("polilinea eje.dxf " + " ".join(f"{f(a)},{f(b)}" for a, b in eje))
    for a in ("-21°17'44\"", "0°", "-", " 1.5E+0001", "35,25", "12°30'"):
        v = im.angulo_grados(a)
        L.append(f"angulo {a.strip().replace(' ', '_') or '_'} {f(v)}")
    (Path(__file__).parent / "tests" / "archivos_esperado.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print(len(L), "líneas;", sum(x.startswith("error") for x in L), "errores esperados")
    print("\n".join(x for x in L if x.startswith(("error", "rep csv_problemas"))))


if __name__ == "__main__":
    main()
