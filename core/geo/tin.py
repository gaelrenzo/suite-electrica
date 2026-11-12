"""Modelo TIN (red irregular de triángulos) por triangulación de Delaunay.

Fórmulas (ver ``docs/topocraft.md``):

* Interpolación lineal en el triángulo mediante coordenadas baricéntricas
  ``z = l1*z1 + l2*z2 + l3*z3`` con ``l1 + l2 + l3 = 1``.
* Curvas de nivel por intersección plano-arista ("marching triangles"):
  ``t = (L - z0) / (z1 - z0)``, ``P = P0 + t (P1 - P0)``.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass

import numpy as np
from numpy.typing import ArrayLike, NDArray
from scipy.spatial import Delaunay, QhullError


@dataclass(frozen=True)
class Curva:
    """Curva de nivel: polilínea 2D a una cota constante."""

    cota: float
    """Cota de la curva (m)."""
    xy: NDArray[np.float64]
    """Vértices (k, 2) en coordenadas este/norte (m)."""
    cerrada: bool

    @property
    def longitud(self) -> float:
        """Longitud en planta (m)."""
        p = np.vstack([self.xy, self.xy[:1]]) if self.cerrada else self.xy
        return float(np.hypot(*np.diff(p, axis=0).T).sum())


class TIN:
    """Superficie TIN construida a partir de puntos (este, norte, cota).

    Las coordenadas se trasladan internamente al centroide para conservar
    precisión numérica con coordenadas UTM grandes.
    """

    def __init__(self, xyz: ArrayLike, max_lado: float | None = None) -> None:
        """Triangula los puntos.

        Args:
            xyz: arreglo (n, 3) con este, norte y cota en metros. Los pares (x, y)
                deben ser únicos (use ``topocraft.limpieza`` antes).
            max_lado: si se indica, descarta triángulos con algún lado mayor (m);
                útil para no interpolar en bordes cóncavos del levantamiento.
        """
        pts = np.asarray(xyz, dtype=float)
        if pts.ndim != 2 or pts.shape[1] != 3 or len(pts) < 3:
            raise ValueError("Se requieren al menos 3 puntos (n, 3)")
        self.xyz: NDArray[np.float64] = pts
        self.origen: NDArray[np.float64] = pts[:, :2].mean(axis=0)
        self._xy_local = pts[:, :2] - self.origen
        try:
            self._delaunay = Delaunay(self._xy_local)
        except QhullError as exc:
            raise ValueError("No se puede triangular: puntos colineales o insuficientes") from exc
        self.triangulos: NDArray[np.int64] = self._delaunay.simplices.astype(np.int64)

        activos = np.ones(len(self.triangulos), dtype=bool)
        if max_lado is not None:
            p = self._xy_local[self.triangulos]  # (m, 3, 2)
            lados = np.linalg.norm(p - np.roll(p, -1, axis=1), axis=2)
            activos = lados.max(axis=1) <= max_lado
        self.activos: NDArray[np.bool_] = activos

        # Aristas únicas de los triángulos activos y aristas de cada triángulo.
        tri = self.triangulos[activos]
        e = np.sort(np.concatenate([tri[:, [0, 1]], tri[:, [1, 2]], tri[:, [2, 0]]]), axis=1)
        self.aristas, inv = np.unique(e, axis=0, return_inverse=True)
        self._aristas_tri: NDArray[np.int64] = inv.ravel().reshape(3, -1).T

    @property
    def z(self) -> NDArray[np.float64]:
        """Cotas de los vértices (m)."""
        return self.xyz[:, 2]

    @property
    def triangulos_activos(self) -> NDArray[np.int64]:
        """Índices (m, 3) de los triángulos usados."""
        return self.triangulos[self.activos]

    def cota(self, x: ArrayLike, y: ArrayLike) -> NDArray[np.float64]:
        """Interpola la cota (m) en los puntos dados; NaN fuera del TIN."""
        x = np.atleast_1d(np.asarray(x, dtype=float))
        y = np.atleast_1d(np.asarray(y, dtype=float))
        p = np.column_stack([x - self.origen[0], y - self.origen[1]])
        s = self._delaunay.find_simplex(p, tol=1e-9)
        t = self._delaunay.transform[s]  # (k, 3, 2)
        b = np.einsum("ijk,ik->ij", t[:, :2], p - t[:, 2])
        bary = np.column_stack([b, 1.0 - b.sum(axis=1)])
        z = (bary * self.z[self.triangulos[s]]).sum(axis=1)
        fuera = (s < 0) | ~self.activos[s]
        z[fuera] = np.nan
        return z

    def segmentos_aristas(self) -> tuple[NDArray[np.float64], NDArray[np.float64]]:
        """Extremos (ne, 2) inicial y final de cada arista, en coordenadas absolutas."""
        return self.xyz[self.aristas[:, 0], :2], self.xyz[self.aristas[:, 1], :2]

    def curvas_nivel(self, cotas: ArrayLike) -> list[Curva]:
        """Calcula curvas de nivel para las cotas indicadas.

        Cada vértice se clasifica como "arriba" si ``z >= L``; así cada triángulo
        tiene 0 o 2 aristas cortadas y los segmentos se encadenan por arista
        compartida, lo que da polilíneas topológicamente continuas.
        """
        p0 = self._xy_local[self.aristas[:, 0]]
        p1 = self._xy_local[self.aristas[:, 1]]
        z0 = self.z[self.aristas[:, 0]]
        z1 = self.z[self.aristas[:, 1]]
        curvas: list[Curva] = []
        for nivel in np.atleast_1d(np.asarray(cotas, dtype=float)):
            arriba = self.z >= nivel
            corta = arriba[self.aristas[:, 0]] != arriba[self.aristas[:, 1]]
            if not corta.any():
                continue
            t = np.zeros(len(self.aristas))
            t[corta] = (nivel - z0[corta]) / (z1[corta] - z0[corta])
            puntos = p0 + t[:, None] * (p1 - p0) + self.origen

            tc = corta[self._aristas_tri]
            pares = self._aristas_tri[tc.sum(axis=1) == 2]
            mask = tc[tc.sum(axis=1) == 2]
            pares = pares[mask].reshape(-1, 2)
            for cadena, cerrada in _encadenar(pares):
                xy = _sin_repetidos(puntos[cadena])
                if len(xy) < 2:
                    continue
                c = Curva(float(nivel), xy, cerrada and len(xy) >= 3)
                if c.longitud > 1e-6:
                    curvas.append(c)
        return curvas


def _encadenar(pares: NDArray[np.int64]) -> list[tuple[list[int], bool]]:
    """Une pares de aristas (grafo de grado <= 2) en cadenas abiertas o cerradas."""
    adj: dict[int, list[int]] = defaultdict(list)
    for a, b in pares.tolist():
        adj[a].append(b)
        adj[b].append(a)
    visitado: set[int] = set()
    cadenas: list[tuple[list[int], bool]] = []

    def recorrer(inicio: int) -> list[int]:
        cadena = [inicio]
        visitado.add(inicio)
        actual = inicio
        while True:
            siguientes = [n for n in adj[actual] if n not in visitado]
            if not siguientes:
                return cadena
            actual = siguientes[0]
            visitado.add(actual)
            cadena.append(actual)

    for nodo in [n for n, v in adj.items() if len(v) == 1]:  # cadenas abiertas (borde)
        if nodo not in visitado:
            cadenas.append((recorrer(nodo), False))
    for nodo in adj:  # lazos cerrados
        if nodo not in visitado:
            cadenas.append((recorrer(nodo), True))
    return cadenas


def _sin_repetidos(xy: NDArray[np.float64], tol: float = 1e-9) -> NDArray[np.float64]:
    """Elimina vértices consecutivos coincidentes."""
    if len(xy) < 2:
        return xy
    keep = np.ones(len(xy), dtype=bool)
    keep[1:] = np.hypot(*np.diff(xy, axis=0).T) > tol
    return xy[keep]


def niveles(zmin: float, zmax: float, equidistancia: float) -> NDArray[np.float64]:
    """Cotas múltiplo de la equidistancia comprendidas en [zmin, zmax] (m)."""
    if equidistancia <= 0:
        raise ValueError("La equidistancia debe ser positiva")
    k0 = int(np.ceil(zmin / equidistancia - 1e-9))
    k1 = int(np.floor(zmax / equidistancia + 1e-9))
    return np.round(np.arange(k0, k1 + 1) * equidistancia, 6)


def es_mayor(cota: float, equidistancia: float, cada: int) -> bool:
    """True si la curva de cota dada es "mayor" (cada ``cada`` equidistancias)."""
    k = round(cota / equidistancia)
    return k % cada == 0
