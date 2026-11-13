"""Importación de topografía en los formatos de campo habituales (como TOPOMAGIC/REDLIN).

* ``.kml`` (Google Earth): ``Point`` → puntos, ``LineString`` → rutas.
* ``.gpx`` (GPS): ``wpt`` → puntos; ``rte``/``trk`` → rutas.
* ``.txt`` MapSource (Garmin, Grid UTM): líneas ``Waypoint`` → puntos.
* ``.xls/.xlsx`` "RLT" formato UTM: filas zona/banda/X/Y/altitud/vértice/comentario;
  el nombre de ruta en la primera fila de cada ruta; los vértices forman la ruta.
* ``.xls/.xlsx`` RLT formato ACUMULADA/ANGULO y ``.tpg``: perfiles tabulares
  (progresiva, cota, ángulo en vértices) → :func:`leer_perfil_tabular`.

Geográficas WGS84 se convierten a UTM en la zona de la longitud media.
"""

from __future__ import annotations

import math
import re
import xml.etree.ElementTree as ET
from pathlib import Path

import numpy as np
import pandas as pd

from suite_electrica.core.geo.perfil import ESTACION, VERTICE, Perfil
from suite_electrica.core.geo.utm import geograficas_a_utm, zona_desde_longitud

Ruta = tuple[str, np.ndarray]
"""(nombre, vértices (n, 3) este/norte/cota; cota NaN si no hay)."""

EXT_TOPO = {".kml", ".gpx", ".xls", ".xlsx", ".tpg"}


def _local(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _hijos(nodo: ET.Element, nombre: str) -> list[ET.Element]:
    return [n for n in nodo.iter() if _local(n.tag) == nombre]


def _texto(nodo: ET.Element, nombre: str) -> str:
    for n in nodo:
        if _local(n.tag) == nombre:
            return (n.text or "").strip()
    return ""


def _a_utm(lon: list[float], lat: list[float], zona: str | None) -> tuple[np.ndarray, np.ndarray, str]:
    zona = zona or zona_desde_longitud(float(np.mean(lon)))
    e, n = geograficas_a_utm(lon, lat, zona)
    return e, n, zona


def _puntos_df(nombres, e, n, z, desc) -> pd.DataFrame:
    return pd.DataFrame({"punto": [str(x) for x in nombres], "este": np.asarray(e, float),
                         "norte": np.asarray(n, float), "cota": np.asarray(z, float),
                         "descripcion": [str(x) for x in desc]})


def leer_kml(ruta: str | Path, zona: str | None = None) -> tuple[pd.DataFrame, list[Ruta]]:
    """Puntos y rutas de un KML. Altitud 0 (sin relieve) se toma como cota desconocida."""
    raiz = ET.parse(ruta).getroot()
    pts, geo_rutas = [], []
    for pm in _hijos(raiz, "Placemark"):
        nombre = _texto(pm, "name")
        for geom in pm.iter():
            if _local(geom.tag) not in ("Point", "LineString"):
                continue
            coords = [tuple(map(float, c.split(","))) + (0.0,) * (3 - len(c.split(",")))
                      for c in " ".join(n.text or "" for n in _hijos(geom, "coordinates")).split()]
            if _local(geom.tag) == "Point" and coords:
                pts.append((nombre, *coords[0]))
            elif coords:
                geo_rutas.append((nombre, coords))
    todos = [p[1] for p in pts] + [c[0] for _, cs in geo_rutas for c in cs]
    if not todos:
        return _puntos_df([], [], [], [], []), []
    zona = zona or zona_desde_longitud(float(np.mean(todos)))
    def z(v: float) -> float:
        return math.nan if v == 0 else v
    df = _puntos_df([], [], [], [], [])
    if pts:
        e, n, _ = _a_utm([p[1] for p in pts], [p[2] for p in pts], zona)
        df = _puntos_df([p[0] for p in pts], e, n, [z(p[3]) for p in pts], ["KML"] * len(pts))
    rutas = []
    for nombre, cs in geo_rutas:
        e, n, _ = _a_utm([c[0] for c in cs], [c[1] for c in cs], zona)
        rutas.append((nombre, np.column_stack([e, n, [z(c[2]) for c in cs]])))
    return df, rutas


def leer_gpx(ruta: str | Path, zona: str | None = None) -> tuple[pd.DataFrame, list[Ruta]]:
    """Waypoints (puntos) y rutas/tracks de un GPX."""
    raiz = ET.parse(ruta).getroot()

    def leer(nodos: list[ET.Element]) -> list[tuple[str, float, float, float, str]]:
        out = []
        for w in nodos:
            ele = _texto(w, "ele")
            out.append((_texto(w, "name"), float(w.get("lon")), float(w.get("lat")),
                        float(ele) if ele else math.nan, _texto(w, "cmt") or _texto(w, "desc")))
        return out

    wpt = leer([n for n in raiz if _local(n.tag) == "wpt"])
    grupos = [(_texto(r, "name"), leer(_hijos(r, "rtept"))) for r in raiz if _local(r.tag) == "rte"]
    grupos += [(_texto(t, "name"), leer(_hijos(t, "trkpt"))) for t in raiz if _local(t.tag) == "trk"]
    lons = [w[1] for w in wpt] + [p[1] for _, g in grupos for p in g]
    if not lons:
        return _puntos_df([], [], [], [], []), []
    zona = zona or zona_desde_longitud(float(np.mean(lons)))
    df = _puntos_df([], [], [], [], [])
    if wpt:
        e, n, _ = _a_utm([w[1] for w in wpt], [w[2] for w in wpt], zona)
        df = _puntos_df([w[0] for w in wpt], e, n, [w[3] for w in wpt], [w[4] for w in wpt])
    rutas = []
    for nombre, g in grupos:
        if g:
            e, n, _ = _a_utm([p[1] for p in g], [p[2] for p in g], zona)
            rutas.append((nombre, np.column_stack([e, n, [p[3] for p in g]])))
    return df, rutas


_POS_UTM = re.compile(r"^\s*(\d{1,2})\s*([C-X])\s+([\d.]+)\s+([\d.]+)\s*$", re.I)


def leer_mapsource_txt(ruta: str | Path) -> pd.DataFrame:
    """Waypoints de un TXT exportado por Garmin MapSource con ``Grid UTM``."""
    filas = []
    for linea in Path(ruta).read_text(encoding="latin-1").splitlines():
        c = linea.split("\t")
        if c[0] != "Waypoint" or len(c) < 6:
            continue
        m = _POS_UTM.match(c[4])
        if not m:
            raise ValueError(f"{ruta}: posición no UTM ({c[4]!r}); exporte con Grid UTM")
        alt = re.match(r"\s*(-?[\d.]+)", c[5])
        filas.append((c[1], float(m[3]), float(m[4]), float(alt[1]) if alt else math.nan, c[2]))
    return pd.DataFrame(filas, columns=["punto", "este", "norte", "cota", "descripcion"])


def es_mapsource(ruta: str | Path) -> bool:
    with open(ruta, encoding="latin-1") as fh:
        return fh.readline().startswith("Grid")


def _rlt(ruta: str | Path) -> tuple[str, pd.DataFrame]:
    """(formato, tabla) de un Excel RLT de TOPOMAGIC/REDLIN."""
    crudo = pd.read_excel(ruta, header=None, dtype=object)
    formato = str(crudo.iat[0, 3]).strip().upper()
    t = crudo.iloc[2:].reset_index(drop=True)
    t.columns = [str(c) for c in crudo.iloc[1]]
    return formato, t


def _col(t: pd.DataFrame, *claves: str) -> pd.Series:
    for c in t.columns:
        norm = c.lower().encode("ascii", "ignore").decode()
        if any(k in norm for k in claves):
            return t[c]
    return pd.Series([None] * len(t))


def leer_xls_utm(ruta: str | Path) -> tuple[pd.DataFrame, list[Ruta]]:
    """Excel RLT formato UTM: puntos (todas las filas) y rutas (vértices de cada ruta)."""
    formato, t = _rlt(ruta)
    if formato != "UTM":
        raise ValueError(f"{ruta}: formato RLT '{formato}'; use leer_perfil_tabular")
    nombre = _col(t, "nombre").ffill().fillna("Ruta").astype(str)
    vert = _col(t, "rtice").fillna("").astype(str).str.strip()
    e = pd.to_numeric(_col(t, "x"), errors="coerce").to_numpy()
    n = pd.to_numeric(_col(t, "y"), errors="coerce").to_numpy()
    z = pd.to_numeric(_col(t, "altitud"), errors="coerce").to_numpy()
    com = _col(t, "comentario").fillna("").astype(str)
    df = _puntos_df([v or str(i + 1) for i, v in enumerate(vert)], e, n, z,
                    [c if c != "nan" else "" for c in com])
    rutas = []
    for nom in dict.fromkeys(nombre):
        m = (nombre == nom).to_numpy() & (vert != "").to_numpy()
        if m.sum() >= 2:
            rutas.append((nom, np.column_stack([e[m], n[m], z[m]])))
    return df, rutas


_DMS = re.compile(r"(-?)\s*(\d+(?:\.\d+)?)\D*(?:(\d+(?:\.\d+)?)\D*)?(?:(\d+(?:\.\d+)?)\D*)?$")


def angulo_grados(v: object) -> float:
    """Ángulo en grados desde número o texto ``-21°17'44"`` (NaN si vacío)."""
    if v is None or (isinstance(v, float) and math.isnan(v)):
        return math.nan
    if isinstance(v, (int, float)):
        return float(v)
    s = str(v).strip().replace(",", ".")
    if not s or s == "-":
        return math.nan
    try:
        return float(s)
    except ValueError:
        pass
    m = _DMS.match(s)
    if not m:
        raise ValueError(f"Ángulo no reconocido: {v!r}")
    g = float(m[2]) + float(m[3] or 0) / 60 + float(m[4] or 0) / 3600
    return -g if m[1] else g


def _perfil_de_tabla(prog, cota, ang, es_vertice) -> tuple[Perfil, np.ndarray]:
    """Perfil y eje en planta reconstruido girando el rumbo el ángulo de cada vértice.

    ponytail: el eje queda en un sistema local (inicio en 0,0 con rumbo norte); sirve
    para deflexiones y retenidas, no para planos georreferenciados.
    """
    prog, cota = np.asarray(prog, float), np.asarray(cota, float)
    es_vertice = np.array(es_vertice, bool)
    es_vertice[[0, -1]] = True
    sv = prog[es_vertice]
    av = np.nan_to_num(np.asarray(ang, float)[es_vertice])
    rumbo, xy = 0.0, [np.zeros(2)]
    for k in range(1, len(sv)):
        rumbo -= math.radians(av[k - 1]) if k > 1 else 0.0  # deflexión + = izquierda
        d = sv[k] - sv[k - 1]
        xy.append(xy[-1] + d * np.array([math.sin(rumbo), math.cos(rumbo)]))
    eje = np.array(xy)
    este, norte = np.interp(prog, sv, eje[:, 0]), np.interp(prog, sv, eje[:, 1])
    tipo = np.where(es_vertice, VERTICE, ESTACION)
    return Perfil(prog, este, norte, cota, tipo), eje


def leer_perfil_tabular(ruta: str | Path) -> tuple[Perfil, np.ndarray]:
    """Perfil desde Excel RLT ACUMULADA/ANGULO o ``.tpg`` (progresiva, cota, ángulo, etiqueta).

    En TPG los vértices llevan etiqueta que empieza con ``v`` (p. ej. ``vSPA-0``);
    en el Excel, la columna *Vértice* no vacía.
    """
    ruta = Path(ruta)
    if ruta.suffix.lower() == ".tpg":
        lineas = [l.split("\t") for l in ruta.read_text(encoding="latin-1").splitlines() if l.strip()]
        if lineas and not re.match(r"^\s*-?[\d.]+$", lineas[0][0]):
            lineas = lineas[1:]
        prog = [float(l[0]) for l in lineas]
        cota = [float(l[1]) for l in lineas]
        ang = [angulo_grados(l[3]) if len(l) > 3 else math.nan for l in lineas]
        vert = [len(l) > 4 and l[4].strip().lower().startswith("v") for l in lineas]
        return _perfil_de_tabla(prog, cota, ang, vert)
    formato, t = _rlt(ruta)
    if "ACUMULADA" not in formato:
        raise ValueError(f"{ruta}: formato RLT '{formato}' no es ACUMULADA/ANGULO")
    nombre = _col(t, "nombre")
    if nombre.notna().sum() > 1:  # ponytail: solo la primera ruta del archivo
        fin = int(np.flatnonzero(nombre.notna().to_numpy())[1])
        t = t.iloc[:fin]
    vert = _col(t, "rtice").fillna("").astype(str).str.strip().to_numpy() != ""
    return _perfil_de_tabla(pd.to_numeric(_col(t, "acumulada")), pd.to_numeric(_col(t, "altitud")),
                            [angulo_grados(a) for a in _col(t, "ngulo")], vert)


def importar(ruta: str | Path, zona: str | None = None) -> tuple[pd.DataFrame, list[Ruta]]:
    """Puntos y rutas de KML, GPX, TXT MapSource o Excel RLT UTM."""
    ext = Path(ruta).suffix.lower()
    if ext == ".kml":
        return leer_kml(ruta, zona)
    if ext == ".gpx":
        return leer_gpx(ruta, zona)
    if ext == ".txt" and es_mapsource(ruta):
        return leer_mapsource_txt(ruta), []
    if ext in (".xls", ".xlsx"):
        return leer_xls_utm(ruta)
    raise ValueError(f"Formato no soportado para importar: {ext}")
