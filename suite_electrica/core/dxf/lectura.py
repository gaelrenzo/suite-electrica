"""Lectura de puntos y polilíneas desde DXF."""

from __future__ import annotations

from pathlib import Path

import ezdxf
import numpy as np
from numpy.typing import NDArray


def _filtro(capa: str | None) -> str:
    return f'[layer=="{capa}"]' if capa else ""


def leer_puntos(ruta: str | Path, capa: str | None = None) -> NDArray[np.float64]:
    """Lee puntos 3D (m) de entidades POINT y vértices de POLYLINE/LWPOLYLINE.

    Returns:
        arreglo (n, 3) con este, norte, cota. Puede contener duplicados.
    """
    msp = ezdxf.readfile(ruta).modelspace()
    f = _filtro(capa)
    pts: list[tuple[float, float, float]] = []
    for e in msp.query(f"POINT{f}"):
        v = e.dxf.location
        pts.append((v.x, v.y, v.z))
    for e in msp.query(f"POLYLINE{f}"):
        pts.extend((v.x, v.y, v.z) for v in e.points())
    for e in msp.query(f"LWPOLYLINE{f}"):
        z = e.dxf.elevation
        pts.extend((x, y, z) for x, y in e.get_points("xy"))
    if not pts:
        raise ValueError(f"No se encontraron puntos en {ruta}")
    return np.array(pts, dtype=float)


def leer_polilinea(ruta: str | Path, capa: str | None = None) -> NDArray[np.float64]:
    """Lee la polilínea de eje (n, 2) en planta.

    Si no se indica capa, busca primero en la capa ``EJE`` y luego en cualquiera.
    Toma la primera LWPOLYLINE/POLYLINE encontrada.
    """
    msp = ezdxf.readfile(ruta).modelspace()
    capas = [capa] if capa else ["EJE", None]
    for c in capas:
        for e in msp.query(f"LWPOLYLINE POLYLINE{_filtro(c)}"):
            if e.dxftype() == "LWPOLYLINE":
                xy = np.array(list(e.get_points("xy")), dtype=float)
                cerrada = e.closed
            else:
                xy = np.array([(v.x, v.y) for v in e.points()], dtype=float)
                cerrada = e.is_closed
            if cerrada and len(xy):
                xy = np.vstack([xy, xy[:1]])
            if len(xy) >= 2:
                return xy
    raise ValueError(f"No se encontró polilínea de eje en {ruta}")
