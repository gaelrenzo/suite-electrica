"""Terrenos sintéticos con cota exacta conocida, para pruebas y ejemplos.

Origen por defecto cerca de Puno (UTM WGS84 zona 19S).
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

import numpy as np
import pandas as pd
from numpy.typing import ArrayLike, NDArray

E0: float = 390_000.0
"""Este de la esquina SO de la malla (m)."""
N0: float = 8_248_000.0
"""Norte de la esquina SO de la malla (m)."""
Z0: float = 3_820.0
"""Cota base (m s. n. m.)."""
LADO: float = 500.0
"""Lado de la zona cuadrada (m)."""


@dataclass(frozen=True)
class Superficie:
    """Superficie analítica z = f(este, norte) en metros."""

    nombre: str
    f: Callable[[NDArray[np.float64], NDArray[np.float64]], NDArray[np.float64]]

    def cota(self, x: ArrayLike, y: ArrayLike) -> NDArray[np.float64]:
        """Cota exacta (m)."""
        return self.f(np.asarray(x, dtype=float), np.asarray(y, dtype=float))


def plano_inclinado(gx: float = 0.05, gy: float = -0.02) -> Superficie:
    """Plano z = Z0 + gx (x - E0) + gy (y - N0); pendientes en m/m."""
    return Superficie("plano_inclinado", lambda x, y: Z0 + gx * (x - E0) + gy * (y - N0))


def cono(altura: float = 50.0, pendiente: float = 0.2) -> Superficie:
    """Cono con vértice en el centro: z = Z0 + altura - pendiente * r."""
    xc, yc = E0 + LADO / 2, N0 + LADO / 2
    return Superficie("cono", lambda x, y: Z0 + altura - pendiente * np.hypot(x - xc, y - yc))


def colina_gaussiana(amplitud: float = 30.0, sigma: float = 80.0) -> Superficie:
    """Colina z = Z0 + A exp(-r² / (2 sigma²)) centrada en la zona."""
    xc, yc = E0 + LADO / 2, N0 + LADO / 2
    return Superficie(
        "colina_gaussiana",
        lambda x, y: Z0 + amplitud * np.exp(-((x - xc) ** 2 + (y - yc) ** 2) / (2 * sigma**2)),
    )


def malla(sup: Superficie, paso: float = 10.0, lado: float = LADO) -> pd.DataFrame:
    """Muestrea la superficie en una malla regular y devuelve puntos estándar."""
    v = np.arange(0.0, lado + paso / 2, paso)
    gx, gy = np.meshgrid(E0 + v, N0 + v)
    x, y = gx.ravel(), gy.ravel()
    return pd.DataFrame({
        "punto": [f"P{i + 1}" for i in range(len(x))],
        "este": x, "norte": y, "cota": sup.cota(x, y), "descripcion": "TN",
    })


def eje_ejemplo() -> NDArray[np.float64]:
    """Eje de tres tramos que cruza la zona (m)."""
    return np.array([
        [E0 + 20.0, N0 + 30.0],
        [E0 + 250.0, N0 + 260.0],
        [E0 + 330.0, N0 + 400.0],
        [E0 + 480.0, N0 + 470.0],
    ])
