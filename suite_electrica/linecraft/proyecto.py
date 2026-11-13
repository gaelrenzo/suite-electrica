"""Archivo de proyecto LineCraft (JSON). Rutas relativas al propio archivo.

Todo valor normativo es una entrada del proyecto con su fuente en ``fuentes``;
si falta la fuente o contiene "PENDIENTE"/"EJEMPLO", se emite un aviso que
aparece en la memoria de cálculo.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd

from suite_electrica.core.dxf.lectura import leer_polilinea
from suite_electrica.core.geo.perfil import Perfil, perfil_longitudinal
from suite_electrica.core.geo.tin import TIN
from suite_electrica.linecraft.conductor import Conductor, Hipotesis, leer_catalogo
from suite_electrica.linecraft.mecanica import Modelo

PARAMETROS_NORMATIVOS = (
    "hipotesis", "tiro_inicial_pct_rotura", "tiro_maximo_pct_rotura",
    "distancia_minima_terreno_m", "conductor",
)
"""Claves de ``fuentes`` que deben citar norma o ficha técnica."""


@dataclass
class Proyecto:
    """Datos de entrada de una línea. Unidades SI."""

    nombre: str
    conductor: Conductor
    n_conductores: int
    modelo: Modelo
    hipotesis: list[Hipotesis]
    hip_inicial: str
    tiro_inicial_pct: float
    """Tiro horizontal en la condición inicial (EDS), % del tiro de rotura."""
    hip_flecha_maxima: str
    tiro_maximo_pct: float
    """Límite de tiro máximo en cualquier hipótesis, % del tiro de rotura."""
    distancia_minima_terreno: float
    altura_amarre: float
    vano_maximo: float
    vano_minimo: float
    angulo_retenida: float
    """Ángulo de la retenida respecto de la horizontal (grados)."""
    perfil: Perfil
    eje: np.ndarray
    estructuras_manual: pd.DataFrame | None = None
    fuentes: dict[str, str] = field(default_factory=dict)
    alertas: dict[str, float] = field(default_factory=dict)
    """Límites de ``linecraft.alertas.ConfigAlertas`` (0 o ausente = no verificar)."""

    def hip(self, nombre: str) -> Hipotesis:
        """Busca una hipótesis por nombre."""
        for h in self.hipotesis:
            if h.nombre == nombre:
                return h
        raise KeyError(f"Hipótesis no definida: {nombre}")

    def avisos_fuentes(self) -> list[str]:
        """Parámetros normativos sin fuente verificada."""
        avisos = []
        for k in PARAMETROS_NORMATIVOS:
            f = self.fuentes.get(k, "").strip()
            if not f:
                avisos.append(f"PENDIENTE_REVISION: '{k}' sin fuente normativa")
            elif "PENDIENTE" in f.upper() or "EJEMPLO" in f.upper():
                avisos.append(f"PENDIENTE_REVISION: '{k}' -> {f}")
        return avisos


def _perfil_desde(cfg: dict, base: Path) -> tuple[Perfil, np.ndarray]:
    if "tabla" in cfg:  # perfil REDLIN/TOPOMAGIC: .tpg o Excel ACUMULADA/ANGULO
        from suite_electrica.topocraft.importar import leer_perfil_tabular
        return leer_perfil_tabular(base / cfg["tabla"])
    if "excel" in cfg:
        from suite_electrica.topocraft.salidas import leer_perfil_excel
        return leer_perfil_excel(base / cfg["excel"])
    from suite_electrica.topocraft import datos
    df, _ = datos.limpiar(datos.leer(base / cfg["puntos"]))
    tin = TIN(df[["este", "norte", "cota"]].to_numpy(), max_lado=cfg.get("max_lado"))
    ruta_eje = base / cfg["eje"]
    if ruta_eje.suffix.lower() == ".dxf":
        eje = leer_polilinea(ruta_eje)
    else:
        eje = datos.leer_eje_csv(ruta_eje)
    return perfil_longitudinal(tin, eje, cfg.get("paso", 5.0)), eje


def cargar(ruta: str | Path) -> Proyecto:
    """Lee el proyecto JSON."""
    ruta = Path(ruta)
    base = ruta.parent
    cfg = json.loads(ruta.read_text(encoding="utf-8"))
    cat = leer_catalogo(base / cfg["conductor"]["catalogo"])
    nombre_c = cfg["conductor"]["nombre"]
    if nombre_c not in cat:
        raise ValueError(f"Conductor '{nombre_c}' no está en el catálogo ({sorted(cat)})")
    hips = [Hipotesis(h["nombre"], float(h["temperatura_C"]), float(h.get("presion_viento_Pa", 0)),
                      float(h.get("espesor_hielo_mm", 0)) / 1000, float(h.get("densidad_hielo_kg_m3", 0)))
            for h in cfg["hipotesis"]]
    perfil, eje = _perfil_desde(cfg["perfil"], base)
    manual = None
    if cfg.get("estructuras"):
        manual = pd.read_csv(base / cfg["estructuras"], comment="#", skipinitialspace=True)
    ini = cfg["condicion_inicial"]
    p = Proyecto(
        nombre=cfg.get("nombre", ruta.stem),
        conductor=cat[nombre_c],
        n_conductores=int(cfg.get("n_conductores", 3)),
        modelo=cfg.get("modelo", "catenaria"),
        hipotesis=hips,
        hip_inicial=ini["hipotesis"],
        tiro_inicial_pct=float(ini["tiro_pct_rotura"]),
        hip_flecha_maxima=cfg["hipotesis_flecha_maxima"],
        tiro_maximo_pct=float(cfg["tiro_maximo_pct_rotura"]),
        distancia_minima_terreno=float(cfg["distancia_minima_terreno_m"]),
        altura_amarre=float(cfg["altura_amarre_m"]),
        vano_maximo=float(cfg["vano_maximo_m"]),
        vano_minimo=float(cfg.get("vano_minimo_m", 0.2 * float(cfg["vano_maximo_m"]))),
        angulo_retenida=float(cfg.get("angulo_retenida_deg", 45.0)),
        perfil=perfil,
        eje=eje,
        estructuras_manual=manual,
        fuentes={k: str(v) for k, v in cfg.get("fuentes", {}).items()},
        alertas={k: float(v) for k, v in (cfg.get("alertas") or {}).items()},
    )
    p.hip(p.hip_inicial)
    p.hip(p.hip_flecha_maxima)
    return p
