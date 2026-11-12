"""Transformaciones WGS84 geográficas <-> UTM para las zonas del Perú (17S, 18S, 19S)."""

from __future__ import annotations

from functools import lru_cache

import numpy as np
from numpy.typing import ArrayLike, NDArray
from pyproj import Transformer

ZONAS_EPSG: dict[str, int] = {"17S": 32717, "18S": 32718, "19S": 32719}
"""Códigos EPSG de WGS84 / UTM hemisferio sur usados en el Perú."""


def epsg_utm(zona: str) -> int:
    """Devuelve el EPSG de una zona UTM peruana ("17S", "18S" o "19S")."""
    clave = zona.strip().upper()
    if clave not in ZONAS_EPSG:
        raise ValueError(f"Zona UTM no soportada: {zona!r}. Use una de {sorted(ZONAS_EPSG)}")
    return ZONAS_EPSG[clave]


def zona_desde_longitud(lon_deg: float) -> str:
    """Zona UTM (hemisferio sur) que contiene la longitud dada en grados."""
    return f"{int((lon_deg + 180.0) // 6.0) + 1}S"


@lru_cache(maxsize=8)
def _transformador(origen: int, destino: int) -> Transformer:
    return Transformer.from_crs(origen, destino, always_xy=True)


def geograficas_a_utm(
    lon_deg: ArrayLike, lat_deg: ArrayLike, zona: str
) -> tuple[NDArray[np.float64], NDArray[np.float64]]:
    """Convierte longitud/latitud WGS84 (grados) a este/norte UTM (m)."""
    e, n = _transformador(4326, epsg_utm(zona)).transform(np.asarray(lon_deg), np.asarray(lat_deg))
    return np.asarray(e, dtype=float), np.asarray(n, dtype=float)


def utm_a_geograficas(
    este_m: ArrayLike, norte_m: ArrayLike, zona: str
) -> tuple[NDArray[np.float64], NDArray[np.float64]]:
    """Convierte este/norte UTM (m) a longitud/latitud WGS84 (grados)."""
    lon, lat = _transformador(epsg_utm(zona), 4326).transform(np.asarray(este_m), np.asarray(norte_m))
    return np.asarray(lon, dtype=float), np.asarray(lat, dtype=float)
