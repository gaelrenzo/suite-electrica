"""Mecánica del conductor: cambio de estado, catenaria, parábola y vano regulador.

Convención: vano de longitud horizontal ``a`` (m) entre el soporte A (x = 0) y
el B (x = a), desnivel ``h = z_B - z_A`` (m), tiro horizontal ``H`` (N), carga
unitaria ``w`` (N/m). Las alturas ``y(x)`` se miden respecto del soporte A.
"""

from __future__ import annotations

import math
from collections.abc import Sequence
from typing import Literal, Protocol

import numpy as np
from numpy.typing import ArrayLike, NDArray

from suite_electrica.linecraft.conductor import Conductor

Modelo = Literal["catenaria", "parabola"]


def cambio_estado(
    sigma1: float, w1: float, t1: float, w2: float, t2: float, vano: float, c: Conductor,
    desnivel: float = 0.0,
) -> float:
    """Esfuerzo horizontal final ``σ2`` (Pa) por la ecuación de cambio de estado.

    ``σ2² [σ2 − σ1 + a² w1² E / (24 A² σ1²) + α E (t2 − t1)] = a² w2² E / (24 A²)``

    Resulta de igualar la variación de longitud parabólica del conductor
    ``a³ w² / (24 H²)`` con la dilatación térmica y la deformación elástica.
    La cúbica tiene una única raíz real positiva.

    Con ``desnivel ≠ 0`` (vano único inclinado) se usa la forma exacta: longitud de
    catenaria ``L = sqrt(h² + (2C sinh(a/2C))²)`` y longitud sin tensión
    ``L0 = L / (1 + T_m / EA)`` con ``T_m = H sqrt(1 + (h/a)²)``; se resuelve
    ``L0(σ2) = L0(σ1) (1 + α Δt)``. Validado contra REDLIN (ver docs/linecraft.md).
    """
    if sigma1 <= 0 or vano <= 0:
        raise ValueError("σ1 y el vano deben ser positivos")
    if desnivel:
        return _cambio_estado_inclinado(sigma1, w1, t1, w2, t2, vano, desnivel, c)
    a, A, E = vano, c.seccion, c.modulo_elasticidad
    k = a * a * w1 * w1 * E / (24 * A * A * sigma1 * sigma1) - sigma1 + c.coef_dilatacion * E * (t2 - t1)
    m = a * a * w2 * w2 * E / (24 * A * A)
    raices = np.roots([1.0, k, 0.0, -m])
    reales = raices[(np.abs(raices.imag) < 1e-6 * np.abs(raices).max()) & (raices.real > 0)].real
    if len(reales) == 0:
        raise ArithmeticError("Sin solución positiva en la ecuación de cambio de estado")
    return float(reales.max())


def _cambio_estado_inclinado(sigma1: float, w1: float, t1: float, w2: float, t2: float,
                             a: float, h: float, c: Conductor) -> float:
    from scipy.optimize import brentq

    A, EA = c.seccion, c.modulo_elasticidad * c.seccion
    k = math.hypot(1.0, h / a)

    def l0(H: float, w: float) -> float:
        C = H / w
        return math.hypot(h, 2 * C * math.sinh(a / (2 * C))) / (1 + H * k / EA)

    H1 = sigma1 * A
    objetivo = l0(H1, w1) * (1 + c.coef_dilatacion * (t2 - t1))
    # ponytail: cota inferior C = a/40 (evita overflow de sinh); por debajo el vano es un "colgajo".
    return brentq(lambda H: l0(H, w2) - objetivo, w2 * a / 40, 100 * H1 + 1e5, xtol=1e-6) / A


def vano_regulador(vanos: Sequence[float]) -> float:
    """Vano regulador (equivalente) de un tramo de tensado: ``sqrt(Σa³ / Σa)`` (m)."""
    a = np.asarray(vanos, dtype=float)
    if len(a) == 0 or (a <= 0).any():
        raise ValueError("Vanos deben ser positivos")
    return float(np.sqrt((a**3).sum() / a.sum()))


class Curva(Protocol):
    """Interfaz común de catenaria y parábola."""

    a: float
    h: float
    H: float
    w: float

    def y(self, x: ArrayLike) -> NDArray[np.float64]: ...
    @property
    def x_vertice(self) -> float: ...
    @property
    def flecha(self) -> float: ...
    @property
    def tiro_maximo(self) -> float: ...


class Catenaria:
    """Catenaria entre soportes a distinto nivel; parámetro ``C = H / w`` (m)."""

    def __init__(self, a: float, h: float, H: float, w: float) -> None:
        self.a, self.h, self.H, self.w = a, h, H, w
        self.C = H / w
        self._xm = a / 2 - self.C * math.asinh(h / (2 * self.C * math.sinh(a / (2 * self.C))))

    @property
    def x_vertice(self) -> float:
        """Abscisa del punto más bajo respecto de A (puede caer fuera del vano)."""
        return self._xm

    def y(self, x: ArrayLike) -> NDArray[np.float64]:
        """Altura del conductor respecto de A: ``C [cosh((x − xm)/C) − cosh(xm/C)]``."""
        x = np.asarray(x, dtype=float)
        return self.C * (np.cosh((x - self._xm) / self.C) - math.cosh(self._xm / self.C))

    @property
    def flecha(self) -> float:
        """Máxima distancia vertical entre la cuerda y el conductor (m)."""
        x = min(max(self._xm + self.C * math.asinh(self.h / self.a), 0.0), self.a)
        return float(self.h * x / self.a - self.y(x))

    @property
    def tiro_maximo(self) -> float:
        """Tiro en el soporte más alejado del vértice: ``H cosh(x/C)`` (N)."""
        return self.H * math.cosh(max(abs(self._xm), abs(self.a - self._xm)) / self.C)

    @property
    def longitud(self) -> float:
        """Longitud del conductor: ``sqrt(h² + (2C sinh(a/2C))²)`` (m)."""
        return math.hypot(self.h, 2 * self.C * math.sinh(self.a / (2 * self.C)))


class Parabola:
    """Aproximación parabólica: ``y = h x / a − w x (a − x) / (2H)``."""

    def __init__(self, a: float, h: float, H: float, w: float) -> None:
        self.a, self.h, self.H, self.w = a, h, H, w

    @property
    def x_vertice(self) -> float:
        """Abscisa del punto más bajo: ``a/2 − H h / (w a)``."""
        return self.a / 2 - self.H * self.h / (self.w * self.a)

    def y(self, x: ArrayLike) -> NDArray[np.float64]:
        x = np.asarray(x, dtype=float)
        return self.h * x / self.a - self.w * x * (self.a - x) / (2 * self.H)

    @property
    def flecha(self) -> float:
        """``f = w a² / (8 H)`` (m)."""
        return self.w * self.a**2 / (8 * self.H)

    @property
    def tiro_maximo(self) -> float:
        """``H sqrt(1 + y'²)`` en el soporte de mayor pendiente (N)."""
        p = max(abs(self.h / self.a - self.w * self.a / (2 * self.H)),
                abs(self.h / self.a + self.w * self.a / (2 * self.H)))
        return self.H * math.sqrt(1 + p * p)


def curva(modelo: Modelo, a: float, h: float, H: float, w: float) -> Catenaria | Parabola:
    """Crea la curva del conductor según el modelo."""
    if modelo == "catenaria":
        return Catenaria(a, h, H, w)
    if modelo == "parabola":
        return Parabola(a, h, H, w)
    raise ValueError(f"Modelo desconocido: {modelo}")
