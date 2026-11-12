"""Conductores, hipótesis de carga y cargas unitarias (SI)."""

from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path

import pandas as pd

G: float = 9.80665
"""Aceleración de la gravedad estándar (m/s²)."""


@dataclass(frozen=True)
class Conductor:
    """Propiedades del conductor en SI."""

    nombre: str
    seccion: float
    """Sección real (m²)."""
    diametro: float
    """Diámetro exterior (m)."""
    peso: float
    """Peso propio (N/m)."""
    carga_rotura: float
    """Tiro de rotura nominal (N)."""
    modulo_elasticidad: float
    """Módulo de elasticidad final (Pa)."""
    coef_dilatacion: float
    """Coeficiente de dilatación lineal (1/°C)."""
    fuente: str = ""


def leer_catalogo(ruta: str | Path) -> dict[str, Conductor]:
    """Lee un catálogo CSV de conductores (líneas ``#`` son comentarios).

    Columnas: ``nombre, seccion_mm2, diametro_mm, masa_kg_m, carga_rotura_kN,
    modulo_elasticidad_MPa, coef_dilatacion_1_C, fuente``.
    """
    df = pd.read_csv(ruta, comment="#", skipinitialspace=True)
    df = df.fillna({"fuente": ""})
    return {
        str(r.nombre).strip(): Conductor(
            nombre=str(r.nombre).strip(),
            seccion=r.seccion_mm2 * 1e-6,
            diametro=r.diametro_mm * 1e-3,
            peso=r.masa_kg_m * G,
            carga_rotura=r.carga_rotura_kN * 1e3,
            modulo_elasticidad=r.modulo_elasticidad_MPa * 1e6,
            coef_dilatacion=float(r.coef_dilatacion_1_C),
            fuente=str(r.fuente),
        )
        for r in df.itertuples(index=False)
    }


@dataclass(frozen=True)
class Hipotesis:
    """Condición climática de cálculo."""

    nombre: str
    temperatura: float
    """Temperatura del conductor (°C)."""
    presion_viento: float = 0.0
    """Presión de viento sobre el área proyectada (Pa)."""
    espesor_hielo: float = 0.0
    """Espesor radial del manguito de hielo (m)."""
    densidad_hielo: float = 0.0
    """Densidad del hielo (kg/m³); obligatoria si hay hielo."""


@dataclass(frozen=True)
class CargaUnitaria:
    """Cargas por unidad de longitud (N/m)."""

    vertical: float
    viento: float

    @property
    def resultante(self) -> float:
        """Carga resultante ``sqrt(vertical² + viento²)`` (N/m)."""
        return math.hypot(self.vertical, self.viento)


def carga_unitaria(c: Conductor, h: Hipotesis) -> CargaUnitaria:
    """Cargas unitarias sobre el conductor.

    * hielo: ``w_h = ρ g π e (d + e)``
    * viento: ``w_v = p (d + 2e)``
    """
    e = h.espesor_hielo
    if e > 0 and h.densidad_hielo <= 0:
        raise ValueError(f"Hipótesis {h.nombre}: falta densidad del hielo")
    hielo = h.densidad_hielo * G * math.pi * e * (c.diametro + e) if e > 0 else 0.0
    return CargaUnitaria(c.peso + hielo, h.presion_viento * (c.diametro + 2 * e))
