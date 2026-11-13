"""Ubicación de estructuras sobre el perfil y verificación de despeje al terreno."""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from numpy.typing import NDArray

from suite_electrica.core.geo.perfil import Perfil, progresivas_eje, punto_en_progresiva
from suite_electrica.linecraft.mecanica import Modelo, curva

FIN = "FIN"
"""Estructura terminal (inicio o fin de línea)."""
ANCLAJE = "ANCLAJE"
"""Anclaje intermedio: separa tramos de tensado."""
ANGULO = "ANGULO"
"""Suspensión o amarre en ángulo (no separa tramos)."""
SUSPENSION = "SUSPENSION"
"""Alineamiento."""
TIPOS = (FIN, ANCLAJE, ANGULO, SUSPENSION)
_TOL = 1e-6
TOL_VERTICE = 0.01
"""Tolerancia (m) para asociar una estructura manual a un vértice del eje."""


@dataclass
class Estructura:
    """Estructura (poste) ubicada sobre el eje. Longitudes en m."""

    numero: int
    progresiva: float
    este: float
    norte: float
    cota_terreno: float
    altura_amarre: float
    """Altura del amarre del conductor más bajo sobre el terreno (m)."""
    tipo: str
    angulo: float = 0.0
    """Ángulo de deflexión del eje (grados, + a la izquierda)."""
    armado: str = ""
    """Código de armado (catálogo)."""
    poste: str = ""
    """Código de poste (catálogo)."""

    @property
    def cota_amarre(self) -> float:
        """Cota del amarre (m)."""
        return self.cota_terreno + self.altura_amarre

    @property
    def codigo(self) -> str:
        """Código ``E-01``, ``E-02``…"""
        return f"E-{self.numero:02d}"


def angulos_eje(eje: NDArray[np.float64]) -> NDArray[np.float64]:
    """Deflexión (grados, + izquierda) en cada vértice del eje; 0 en los extremos."""
    d = np.diff(np.asarray(eje, dtype=float)[:, :2], axis=0)
    rumbo = np.arctan2(d[:, 1], d[:, 0])
    defl = np.degrees((np.diff(rumbo) + np.pi) % (2 * np.pi) - np.pi)
    return np.concatenate([[0.0], defl, [0.0]])


def crear_estructuras(
    perfil: Perfil,
    eje: NDArray[np.float64],
    progresivas: list[float],
    altura_amarre: float | list[float],
    tipos: list[str | None] | None = None,
    armados: list[str] | None = None,
    postes: list[str] | None = None,
) -> list[Estructura]:
    """Crea estructuras en las progresivas dadas, asignando ángulo y tipo.

    Tipo por defecto: FIN en extremos, ANGULO en vértices con deflexión, si no
    SUSPENSION.
    """
    s = np.asarray(progresivas, dtype=float)
    if np.any(np.diff(s) <= 0):
        raise ValueError("Las progresivas de estructuras deben ser crecientes")
    acum = progresivas_eje(eje)
    ang_v = angulos_eje(eje)
    for sv, av in zip(acum[1:-1], ang_v[1:-1]):
        if abs(av) > 0.01 and np.min(np.abs(s - sv)) > TOL_VERTICE:
            raise ValueError(f"El vértice del eje en {sv:.2f} m (deflexión {av:.2f}°) no tiene estructura")
    if abs(s[0]) > TOL_VERTICE or abs(s[-1] - acum[-1]) > TOL_VERTICE:
        raise ValueError(f"Las estructuras deben ir de 0 a {acum[-1]:.2f} m")
    xy = punto_en_progresiva(eje, s)
    z = perfil.cota_en(s)
    if np.isnan(z).any():
        raise ValueError("Hay estructuras fuera del modelo de terreno")
    alturas = altura_amarre if isinstance(altura_amarre, list) else [altura_amarre] * len(s)
    res = []
    for i, si in enumerate(s):
        j = np.flatnonzero(np.abs(acum - si) <= TOL_VERTICE)
        ang = float(ang_v[j[0]]) if len(j) else 0.0
        tipo = tipos[i] if tipos and tipos[i] else (
            FIN if i in (0, len(s) - 1) else ANGULO if abs(ang) > 0.01 else SUSPENSION)
        if tipo not in TIPOS:
            raise ValueError(f"Tipo de estructura desconocido: {tipo}")
        res.append(Estructura(i + 1, float(si), float(xy[i, 0]), float(xy[i, 1]), float(z[i]),
                              float(alturas[i]), tipo, ang,
                              armados[i] if armados else "", postes[i] if postes else ""))
    return res


def linea_conductor(
    perfil: Perfil, s0: float, s1: float, za: float, zb: float, H: float, w: float,
    modelo: Modelo, factor_vertical: float = 1.0, paso: float = 1.0,
) -> tuple[NDArray[np.float64], NDArray[np.float64], NDArray[np.float64]]:
    """Muestrea un vano: progresivas, cota del conductor y cota del terreno (m).

    ``factor_vertical`` = w_vertical / w_resultante proyecta la flecha (de la
    curva con carga resultante) al plano vertical.
    """
    a = s1 - s0
    n = max(int(math.ceil(a / paso)), 2)
    s = np.linspace(s0, s1, n + 1)
    interior = perfil.progresiva[(perfil.progresiva > s0) & (perfil.progresiva < s1)]
    s = np.union1d(s, interior)
    x = s - s0
    c = curva(modelo, a, zb - za, H, w)
    cuerda = (zb - za) * x / a
    zc = za + cuerda - (cuerda - c.y(x)) * factor_vertical
    return s, zc, perfil.cota_en(s)


def despeje_vano(perfil: Perfil, s0: float, s1: float, za: float, zb: float, H: float, w: float,
                 modelo: Modelo, factor_vertical: float = 1.0) -> tuple[float, float]:
    """Despeje mínimo conductor–terreno (m) y su progresiva."""
    s, zc, zt = linea_conductor(perfil, s0, s1, za, zb, H, w, modelo, factor_vertical)
    if np.isnan(zt).any():
        raise ValueError(f"Vano {s0:.1f}-{s1:.1f} m sale del modelo de terreno")
    d = zc - zt
    i = int(np.argmin(d))
    return float(d[i]), float(s[i])


def ubicar_automatico(
    perfil: Perfil,
    eje: NDArray[np.float64],
    altura_amarre: float,
    vano_maximo: float,
    vano_minimo: float,
    despeje_minimo: float,
    H: float,
    w: float,
    modelo: Modelo = "catenaria",
    paso: float = 1.0,
    factor_vertical: float = 1.0,
) -> tuple[list[float], list[str]]:
    """Propone progresivas de estructuras.

    Algoritmo voraz: se fijan estructuras en los vértices del eje; desde cada
    estructura se elige el vano más largo (≤ ``vano_maximo``, sin pasar el
    siguiente vértice) cuyo despeje al terreno con la flecha máxima (``H``, ``w``,
    ``factor_vertical``) sea ≥ ``despeje_minimo``. Si ninguno cumple, se toma el de mayor despeje y se
    emite un aviso.

    Returns:
        (progresivas, avisos)
    """
    forzadas = progresivas_eje(eje)
    L = float(forzadas[-1])
    zt = lambda s: float(perfil.cota_en(s))  # noqa: E731
    res, avisos = [0.0], []
    s0 = 0.0
    while s0 < L - _TOL:
        sig = float(forzadas[forzadas > s0 + _TOL][0])
        lim = min(s0 + vano_maximo, sig)
        piso = min(s0 + vano_minimo, lim)
        cands = np.arange(lim, piso - _TOL, -paso)
        mejor, mejor_d, elegido = None, -math.inf, None
        za = zt(s0) + altura_amarre
        for s in cands:
            d, _ = despeje_vano(perfil, s0, s, za, zt(s) + altura_amarre, H, w, modelo,
                                factor_vertical)
            if d >= despeje_minimo:
                elegido = float(s)
                break
            if d > mejor_d:
                mejor, mejor_d = float(s), d
        if elegido is None:
            elegido = mejor
            avisos.append(f"Vano desde {s0:.2f} m: ningún vano cumple despeje "
                          f"{despeje_minimo:.2f} m (mejor {mejor_d:.2f} m en {elegido:.2f} m)")
        res.append(elegido)
        s0 = elegido
    return res, avisos
