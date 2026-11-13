"""Lectura y limpieza de puntos topográficos.

Formato estándar (DataFrame y CSV): ``punto, este, norte, cota, descripcion``
con coordenadas UTM y cotas en metros.
"""

from __future__ import annotations

import unicodedata
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.spatial import cKDTree

from suite_electrica.core.dxf.lectura import leer_puntos

COLUMNAS = ["punto", "este", "norte", "cota", "descripcion"]
_ALIAS = {
    "punto": "punto", "pto": "punto", "id": "punto", "nombre": "punto", "p": "punto",
    "este": "este", "x": "este", "e": "este", "easting": "este",
    "norte": "norte", "y": "norte", "n": "norte", "northing": "norte",
    "cota": "cota", "z": "cota", "elevacion": "cota", "altura": "cota",
    "descripcion": "descripcion", "desc": "descripcion", "d": "descripcion", "codigo": "descripcion",
}


def _normalizar(texto: str) -> str:
    t = unicodedata.normalize("NFKD", str(texto)).encode("ascii", "ignore").decode()
    return t.strip().lower()


def _a_float(serie: pd.Series) -> pd.Series:
    if not pd.api.types.is_numeric_dtype(serie):
        serie = serie.astype(str).str.strip().str.replace(",", ".", regex=False)
    return pd.to_numeric(serie, errors="coerce")


def leer_csv(ruta: str | Path, requeridas: tuple[str, ...] = ("este", "norte", "cota")) -> pd.DataFrame:
    """Lee un CSV de puntos (con o sin encabezado; separador ``,``, ``;`` o tabulador).

    Sin encabezado se asume PENZD (punto, este, norte, cota, descripción); si las
    magnitudes indican PNEZD (estación total) se intercambian este y norte.
    Con 2 columnas se asume ``este, norte``.
    Columnas no requeridas ausentes quedan vacías (NaN).
    """
    with open(ruta, encoding="utf-8-sig") as fh:
        linea = fh.readline()
    sep = ";" if ";" in linea else "\t" if "\t" in linea else ","
    crudo = pd.read_csv(ruta, sep=sep, dtype=str, header=None, skipinitialspace=True,
                        encoding="utf-8-sig", keep_default_na=False)
    primera = [_normalizar(v) for v in crudo.iloc[0].tolist()]
    if sum(v in _ALIAS for v in primera) >= min(3, len(requeridas)):
        crudo.columns = [_ALIAS.get(v, v) for v in primera]
        crudo = crudo.iloc[1:].reset_index(drop=True)
    elif crudo.shape[1] == 2:
        crudo.columns = ["este", "norte"]
    elif crudo.shape[1] == 3 and "cota" not in requeridas:
        crudo.columns = ["punto", "este", "norte"]
    else:
        crudo.columns = COLUMNAS[: crudo.shape[1]] + [f"extra{i}" for i in range(crudo.shape[1] - 5)]
    faltan = set(requeridas) - set(crudo.columns)
    if faltan:
        raise ValueError(f"Faltan columnas en {ruta}: {sorted(faltan)}")
    df = pd.DataFrame({
        "punto": crudo["punto"].astype(str).str.strip() if "punto" in crudo
        else [str(i + 1) for i in range(len(crudo))],
        "este": _a_float(crudo["este"]),
        "norte": _a_float(crudo["norte"]),
        "cota": _a_float(crudo["cota"]) if "cota" in crudo else np.nan,
        "descripcion": crudo["descripcion"].fillna("").astype(str).str.strip()
        if "descripcion" in crudo else "",
    })
    # ponytail: autodetección PNEZD (estación total, TOPOMAGIC). El Este UTM siempre
    # está en ~[160 000, 834 000] m y el Norte del hemisferio sur > 1 000 000 m; falla
    # solo cerca del ecuador en el hemisferio norte (Norte < 1e6). Ahí usar encabezado.
    if df["este"].median() > 1e6 > df["norte"].median():
        df[["este", "norte"]] = df[["norte", "este"]].to_numpy()
    return df.reset_index(drop=True)


def leer_eje_csv(ruta: str | Path) -> np.ndarray:
    """Lee un eje (n, 2) desde CSV con columnas este, norte (encabezado opcional)."""
    return leer_csv(ruta, ("este", "norte"))[["este", "norte"]].to_numpy()


def leer_dxf(ruta: str | Path, capa: str | None = None) -> pd.DataFrame:
    """Lee puntos de un DXF (POINT y vértices de polilíneas 3D)."""
    xyz = leer_puntos(ruta, capa)
    return pd.DataFrame({
        "punto": [str(i + 1) for i in range(len(xyz))],
        "este": xyz[:, 0], "norte": xyz[:, 1], "cota": xyz[:, 2], "descripcion": "DXF",
    })


def leer(ruta: str | Path) -> pd.DataFrame:
    """Lee puntos desde .csv/.txt, .dxf, .kml, .gpx, .xls/.xlsx (RLT UTM) o TXT MapSource."""
    from suite_electrica.topocraft import importar
    ext = Path(ruta).suffix.lower()
    if ext == ".dxf":
        return leer_dxf(ruta)
    if ext in {".kml", ".gpx", ".xls", ".xlsx"} or (ext == ".txt" and importar.es_mapsource(ruta)):
        return importar.importar(ruta)[0]
    if ext in {".csv", ".txt"}:
        return leer_csv(ruta)
    raise ValueError(f"Formato no soportado: {ext}")


def guardar_csv(df: pd.DataFrame, ruta: str | Path) -> None:
    """Guarda puntos en el formato estándar (3 decimales)."""
    df[COLUMNAS].to_csv(ruta, index=False, float_format="%.3f")


@dataclass
class ReporteLimpieza:
    """Resultado de la validación. Listas con nombres de punto."""

    n_entrada: int = 0
    n_salida: int = 0
    invalidos: list[str] = field(default_factory=list)
    """Filas con coordenadas o cota no numéricas (eliminadas)."""
    duplicados: list[str] = field(default_factory=list)
    """Mismo (x, y, z) que un punto anterior (eliminados)."""
    conflictos: list[str] = field(default_factory=list)
    """Mismo (x, y) con cota distinta (eliminados; se conserva el primero)."""
    nombres_repetidos: list[str] = field(default_factory=list)
    atipicos: list[str] = field(default_factory=list)
    """Cotas atípicas detectadas (eliminadas solo si se pidió)."""

    def resumen(self) -> str:
        """Texto breve para consola."""
        def fila(t: str, v: list[str]) -> str:
            extra = f" ({', '.join(v[:10])}{'…' if len(v) > 10 else ''})" if v else ""
            return f"  {t}: {len(v)}{extra}"
        return "\n".join([
            f"Puntos leídos: {self.n_entrada}  ->  usados: {self.n_salida}",
            fila("inválidos", self.invalidos),
            fila("duplicados", self.duplicados),
            fila("conflictos de cota (mismo E,N)", self.conflictos),
            fila("nombres repetidos", self.nombres_repetidos),
            fila("cotas atípicas", self.atipicos),
        ])


def detectar_atipicos(
    xyz: np.ndarray, k: int = 8, factor: float = 5.0, tol_abs: float = 0.5, max_iter: int = 20
) -> np.ndarray:
    """Índices de cotas atípicas (picos o pozos aislados).

    Para cada punto se ajusta por mínimos cuadrados un plano ``z = a dx + b dy + c``
    a sus ``k`` vecinos más cercanos (sin incluirlo). Con el residuo del punto
    ``r = z - c`` y el error típico del ajuste de los vecinos
    ``s = sqrt(sum(e_j^2) / (k - 3))``, el punto es atípico si
    ``|r| > max(tol_abs, factor * s)`` y su ``|r|`` es máximo entre sus vecinos.
    Se itera retirando los atípicos hallados. ``s`` crece con la curvatura del
    terreno, por lo que cerros y quebradas suaves no se marcan.
    """
    activos = np.arange(len(xyz))
    hallados: list[int] = []
    for _ in range(max_iter):
        kk = min(k, len(activos) - 1)
        if kk < 4:
            break
        p = xyz[activos]
        _, idx = cKDTree(p[:, :2]).query(p[:, :2], k=kk + 1)
        vec = idx[:, 1:]
        dx = p[vec, 0] - p[:, None, 0]
        dy = p[vec, 1] - p[:, None, 1]
        a = np.stack([dx, dy, np.ones_like(dx)], axis=2)  # (n, k, 3)
        zv = p[vec, 2]
        coef = np.einsum("nij,nj->ni", np.linalg.pinv(a), zv)
        e = zv - np.einsum("nij,nj->ni", a, coef)
        s = np.sqrt((e**2).sum(axis=1) / (kk - 3))
        r = p[:, 2] - coef[:, 2]
        candidato = np.abs(r) > np.maximum(tol_abs, factor * s)
        maximo_local = np.abs(r) >= np.abs(r[vec]).max(axis=1)
        nuevos = np.flatnonzero(candidato & maximo_local)
        if len(nuevos) == 0:
            break
        hallados.extend(activos[nuevos].tolist())
        activos = np.delete(activos, nuevos)
    return np.array(sorted(hallados), dtype=int)


def limpiar(
    df: pd.DataFrame,
    tol_xy: float = 0.001,
    tol_z: float = 0.001,
    quitar_atipicos: bool = False,
    **kw_atipicos: float,
) -> tuple[pd.DataFrame, ReporteLimpieza]:
    """Valida y limpia los puntos.

    Args:
        df: puntos en formato estándar.
        tol_xy: tolerancia (m) para considerar dos puntos en la misma posición.
        tol_z: diferencia de cota (m) por debajo de la cual es un duplicado exacto.
        quitar_atipicos: si True, elimina las cotas atípicas detectadas.
        **kw_atipicos: parámetros de :func:`detectar_atipicos`.
    """
    rep = ReporteLimpieza(n_entrada=len(df))
    d = df.copy().reset_index(drop=True)
    malos = d[["este", "norte", "cota"]].isna().any(axis=1)
    rep.invalidos = d.loc[malos, "punto"].tolist()
    d = d[~malos].reset_index(drop=True)

    rep.nombres_repetidos = sorted(set(d.loc[d["punto"].duplicated(), "punto"]))

    clave = list(zip(np.round(d["este"] / tol_xy).astype(np.int64),
                     np.round(d["norte"] / tol_xy).astype(np.int64)))
    primero: dict[tuple[int, int], int] = {}
    quitar = np.zeros(len(d), dtype=bool)
    for i, c in enumerate(clave):
        if c in primero:
            quitar[i] = True
            dz = abs(d.at[i, "cota"] - d.at[primero[c], "cota"])
            (rep.duplicados if dz <= tol_z else rep.conflictos).append(d.at[i, "punto"])
        else:
            primero[c] = i
    d = d[~quitar].reset_index(drop=True)

    idx = detectar_atipicos(d[["este", "norte", "cota"]].to_numpy(), **kw_atipicos)
    rep.atipicos = d.loc[idx, "punto"].tolist()
    if quitar_atipicos and len(idx):
        d = d.drop(index=idx).reset_index(drop=True)
    rep.n_salida = len(d)
    return d, rep
