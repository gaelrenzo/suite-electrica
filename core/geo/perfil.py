"""Perfil longitudinal del terreno a lo largo de un eje (polilínea en planta).

El terreno de un TIN es lineal por tramos a lo largo del eje, con quiebres solo
en los cruces con aristas del TIN. Incluyendo esos cruces, la interpolación
lineal entre puntos del perfil reproduce exactamente la superficie TIN.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from numpy.typing import ArrayLike, NDArray

from core.geo.tin import TIN

VERTICE = "V"
"""Vértice del eje."""
ESTACION = "E"
"""Estación a paso regular."""
QUIEBRE = "T"
"""Cruce con arista del TIN (quiebre de pendiente)."""
_PRIORIDAD = {VERTICE: 0, ESTACION: 1, QUIEBRE: 2}


@dataclass(frozen=True)
class Perfil:
    """Perfil longitudinal. Todas las magnitudes en metros."""

    progresiva: NDArray[np.float64]
    este: NDArray[np.float64]
    norte: NDArray[np.float64]
    cota: NDArray[np.float64]
    """Cota de terreno (NaN si el punto cae fuera del TIN)."""
    tipo: NDArray[np.str_]
    """Tipo de punto: ``V`` vértice, ``E`` estación, ``T`` quiebre del TIN."""

    @property
    def longitud(self) -> float:
        """Longitud total del eje (m)."""
        return float(self.progresiva[-1])

    def cota_en(self, s: ArrayLike) -> NDArray[np.float64]:
        """Cota de terreno interpolada en progresivas arbitrarias (m)."""
        return np.interp(np.asarray(s, dtype=float), self.progresiva, self.cota)

    def principales(self) -> "Perfil":
        """Subconjunto con solo vértices y estaciones (sin quiebres)."""
        m = self.tipo != QUIEBRE
        return Perfil(self.progresiva[m], self.este[m], self.norte[m], self.cota[m], self.tipo[m])


def progresivas_eje(eje_xy: ArrayLike) -> NDArray[np.float64]:
    """Progresiva acumulada (m) de cada vértice del eje."""
    p = np.asarray(eje_xy, dtype=float)[:, :2]
    return np.concatenate([[0.0], np.cumsum(np.hypot(*np.diff(p, axis=0).T))])


def punto_en_progresiva(eje_xy: ArrayLike, s: ArrayLike) -> NDArray[np.float64]:
    """Coordenadas (k, 2) en planta de las progresivas ``s`` sobre el eje."""
    p = np.asarray(eje_xy, dtype=float)[:, :2]
    acum = progresivas_eje(p)
    s = np.asarray(s, dtype=float)
    return np.column_stack([np.interp(s, acum, p[:, 0]), np.interp(s, acum, p[:, 1])])


def _cruces_aristas(eje: NDArray[np.float64], tin: TIN) -> NDArray[np.float64]:
    """Progresivas donde el eje cruza aristas del TIN."""
    a0, a1 = tin.segmentos_aristas()
    acum = progresivas_eje(eje)
    resultado: list[NDArray[np.float64]] = []
    for i in range(len(eje) - 1):
        p, q = eje[i], eje[i + 1]
        r = q - p
        largo = float(np.hypot(*r))
        if largo == 0:
            continue
        # Filtro por caja envolvente
        lo, hi = np.minimum(p, q), np.maximum(p, q)
        m = (np.maximum(a0, a1) >= lo).all(axis=1) & (np.minimum(a0, a1) <= hi).all(axis=1)
        b0, d = a0[m], a1[m] - a0[m]
        den = r[0] * d[:, 1] - r[1] * d[:, 0]
        ok = np.abs(den) > 1e-12
        w = b0[ok] - p
        t = (w[:, 0] * d[ok, 1] - w[:, 1] * d[ok, 0]) / den[ok]
        u = (w[:, 0] * r[1] - w[:, 1] * r[0]) / den[ok]
        sel = (t >= 0) & (t <= 1) & (u >= 0) & (u <= 1)
        resultado.append(acum[i] + t[sel] * largo)
    return np.concatenate(resultado) if resultado else np.empty(0)


def perfil_longitudinal(tin: TIN, eje_xy: ArrayLike, paso: float = 20.0) -> Perfil:
    """Calcula el perfil del terreno a lo largo del eje.

    Args:
        tin: superficie del terreno.
        eje_xy: vértices (n, 2) del eje en planta (m).
        paso: separación de estaciones regulares (m).
    """
    eje = np.asarray(eje_xy, dtype=float)[:, :2]
    if len(eje) < 2:
        raise ValueError("El eje necesita al menos 2 vértices")
    if paso <= 0:
        raise ValueError("El paso debe ser positivo")
    acum = progresivas_eje(eje)
    longitud = float(acum[-1])
    estaciones = np.arange(0.0, longitud, paso)
    cruces = _cruces_aristas(eje, tin)

    s = np.concatenate([acum, estaciones, cruces])
    tipos = np.concatenate([
        np.full(len(acum), VERTICE), np.full(len(estaciones), ESTACION), np.full(len(cruces), QUIEBRE)
    ])
    orden = np.lexsort(([_PRIORIDAD[t] for t in tipos], s))
    s, tipos = s[orden], tipos[orden]
    keep = np.ones(len(s), dtype=bool)
    keep[1:] = np.diff(s) > 1e-6  # conserva el de mayor prioridad en coincidencias
    s, tipos = s[keep], tipos[keep]

    xy = punto_en_progresiva(eje, s)
    z = tin.cota(xy[:, 0], xy[:, 1])
    return Perfil(s, xy[:, 0], xy[:, 1], z, tipos)
