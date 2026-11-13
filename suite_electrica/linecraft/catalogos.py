"""Catálogos de postes, armados, retenidas y materiales; asignación, verificación y metrado.

Los catálogos son tablas editables por el usuario; no contienen valores
normativos inventados (las filas sin fuente se marcan PENDIENTE).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from importlib.resources import files
from pathlib import Path

import numpy as np
import pandas as pd

from suite_electrica.linecraft.calculo import Resultado
from suite_electrica.linecraft.conductor import G
from suite_electrica.linecraft.estructuras import ANCLAJE, FIN, Estructura
from suite_electrica.linecraft.mecanica import Catenaria

COLUMNAS: dict[str, list[str]] = {
    "postes": ["codigo", "material", "longitud_m", "carga_nominal_daN", "empotramiento_m", "fuente"],
    "armados": ["codigo", "descripcion", "funcion", "angulo_max_deg", "distancia_punta_amarre_m", "fuente"],
    "retenidas": ["codigo", "descripcion", "carga_rotura_kN", "fuente"],
    "materiales": ["armado", "material", "unidad", "cantidad", "fuente"],
}
NUMERICAS = {"longitud_m", "carga_nominal_daN", "empotramiento_m", "angulo_max_deg",
             "distancia_punta_amarre_m", "carga_rotura_kN", "cantidad"}


def normalizar(nombre: str, df: pd.DataFrame) -> pd.DataFrame:
    """Asegura columnas y tipos de un catálogo."""
    df = df.copy()
    for c in COLUMNAS[nombre]:
        if c not in df:
            df[c] = np.nan if c in NUMERICAS else ""
    df = df[COLUMNAS[nombre]]
    for c in df.columns:
        if c in NUMERICAS:
            df[c] = pd.to_numeric(df[c], errors="coerce")
        else:
            df[c] = df[c].fillna("").astype(str).str.strip()
    return df.reset_index(drop=True)


@dataclass
class Catalogos:
    """Tablas de catálogo (DataFrames con las columnas de ``COLUMNAS``)."""

    postes: pd.DataFrame = field(default_factory=lambda: normalizar("postes", pd.DataFrame()))
    armados: pd.DataFrame = field(default_factory=lambda: normalizar("armados", pd.DataFrame()))
    retenidas: pd.DataFrame = field(default_factory=lambda: normalizar("retenidas", pd.DataFrame()))
    materiales: pd.DataFrame = field(default_factory=lambda: normalizar("materiales", pd.DataFrame()))

    @classmethod
    def plantilla(cls) -> Catalogos:
        """Catálogos por defecto (``suite_electrica/plantillas``)."""
        base = files("suite_electrica.plantillas")
        tablas = {}
        for nombre in COLUMNAS:
            with base.joinpath(f"{nombre}.csv").open("r", encoding="utf-8") as fh:
                tablas[nombre] = normalizar(nombre, pd.read_csv(fh, comment="#", skipinitialspace=True))
        return cls(**tablas)

    @classmethod
    def desde_carpeta(cls, carpeta: str | Path) -> Catalogos:
        """Lee ``postes.csv``, ``armados.csv``… de una carpeta (los ausentes, de la plantilla)."""
        c = cls.plantilla()
        for nombre in COLUMNAS:
            ruta = Path(carpeta) / f"{nombre}.csv"
            if ruta.exists():
                setattr(c, nombre, normalizar(nombre, pd.read_csv(ruta, comment="#", skipinitialspace=True)))
        return c

    def tabla(self, nombre: str) -> pd.DataFrame:
        return getattr(self, nombre)

    def avisos(self) -> list[str]:
        """Filas de catálogo sin fuente o marcadas PENDIENTE."""
        av = []
        for nombre in ("postes", "armados", "retenidas"):
            df = self.tabla(nombre)
            pend = df[(df["fuente"] == "") | df["fuente"].str.upper().str.contains("PENDIENTE")]
            if len(pend):
                av.append(f"PENDIENTE_REVISION: catálogo de {nombre} sin fuente verificada "
                          f"({', '.join(pend['codigo'].astype(str))})")
        return av

    # -- consultas
    def poste(self, codigo: str) -> pd.Series | None:
        f = self.postes[self.postes["codigo"] == codigo]
        return None if f.empty else f.iloc[0]

    def armado(self, codigo: str) -> pd.Series | None:
        f = self.armados[self.armados["codigo"] == codigo]
        return None if f.empty else f.iloc[0]

    def altura_amarre(self, poste: str, armado: str) -> float | None:
        """``longitud − empotramiento − distancia punta–amarre`` si todo está definido (m)."""
        p, a = self.poste(poste), self.armado(armado)
        if p is None or a is None:
            return None
        vals = (p["longitud_m"], p["empotramiento_m"], a["distancia_punta_amarre_m"])
        if any(pd.isna(v) for v in vals):
            return None
        return float(vals[0] - vals[1] - vals[2])

    def elegir_armado(self, tipo: str, angulo: float) -> str:
        """Primer armado cuya función incluye el tipo y admite la deflexión."""
        for r in self.armados.itertuples(index=False):
            funciones = {f.strip().upper() for f in str(r.funcion).split("|")}
            if tipo in funciones and (pd.isna(r.angulo_max_deg) or abs(angulo) <= r.angulo_max_deg):
                return str(r.codigo)
        return ""


def _fuerzas_maximas(r: Resultado) -> dict[int, tuple[float, float, float]]:
    """Por estructura: (F horizontal máx. N, momento base máx. N·m, tiro retenida máx. N)."""
    out: dict[int, tuple[float, float, float]] = {}
    for c in r.cargas:
        f, m, t = out.get(c.estructura, (0.0, 0.0, 0.0))
        out[c.estructura] = (max(f, c.horizontal), max(m, c.momento_base), max(t, c.tiro_retenida))
    return out


def lleva_retenida(e: Estructura) -> bool:
    """Estructuras de fin, anclaje o con deflexión llevan retenida."""
    return e.tipo in (FIN, ANCLAJE) or abs(e.angulo) > 0.01


def verificar(r: Resultado, cat: Catalogos, fs_poste: float, fs_retenida: float,
              retenida: str | None = None, n_retenidas: dict[int, int] | None = None) -> pd.DataFrame:
    """Verifica postes (sin retenida) y retenidas (con retenida) en la hipótesis más desfavorable.

    * Poste: fuerza equivalente en la punta ``F = M_base / h_libre`` con
      ``h_libre = longitud − empotramiento`` (si no hay empotramiento, ``F = F_h``);
      ``FS = carga nominal / F``.
    * Retenida: ``FS = carga de rotura / (T / n)``; la retenida toma toda la carga horizontal.
    """
    fm = _fuerzas_maximas(r)
    ret = cat.retenidas[cat.retenidas["codigo"] == retenida] if retenida else cat.retenidas.head(1)
    ret = None if ret.empty else ret.iloc[0]
    filas = []
    for e in r.estructuras:
        fh, mb, tr = fm.get(e.numero, (0.0, 0.0, 0.0))
        fila: dict[str, object] = {"Estructura": e.codigo, "Tipo": e.tipo, "Armado": e.armado or "—",
                                   "Poste": e.poste or "—", "F horizontal máx. (N)": round(fh, 1)}
        obs, ok = [], True
        if lleva_retenida(e):
            n = (n_retenidas or {}).get(e.numero, 1) or 1
            fila["Retenidas"] = n
            fila["Tiro por retenida (N)"] = round(tr / n, 1)
            if ret is not None and not pd.isna(ret["carga_rotura_kN"]):
                fs = ret["carga_rotura_kN"] * 1e3 / (tr / n) if tr > 0 else math.inf
                fila["FS retenida"] = round(fs, 2) if math.isfinite(fs) else "—"
                if fs < fs_retenida:
                    ok = False
                    obs.append(f"FS retenida {fs:.2f} < {fs_retenida:g}")
            else:
                obs.append("Sin retenida en catálogo")
        else:
            fila["Retenidas"] = 0
            p = cat.poste(e.poste) if e.poste else None
            if p is not None and not pd.isna(p["carga_nominal_daN"]):
                libre = p["longitud_m"] - p["empotramiento_m"] if not pd.isna(p["empotramiento_m"]) else None
                f_punta = mb / libre if libre else fh
                fs = p["carga_nominal_daN"] * 10 / f_punta if f_punta > 0 else math.inf
                fila["F punta equiv. (N)"] = round(f_punta, 1)
                fila["FS poste"] = round(fs, 2) if math.isfinite(fs) else "—"
                if fs < fs_poste:
                    ok = False
                    obs.append(f"FS poste {fs:.2f} < {fs_poste:g}")
            elif e.poste:
                ok = False
                obs.append("Poste no está en catálogo")
        if not e.poste:
            ok = False
            obs.append("Sin poste asignado (ningún poste del catálogo cumple o falta asignar)")
        if e.poste and (pp := cat.poste(e.poste)) is not None and not pd.isna(pp["longitud_m"]):
            if e.altura_amarre >= pp["longitud_m"]:
                ok = False
                obs.append("Altura de amarre ≥ longitud del poste")
        fila["Cumple"] = "SÍ" if ok else "NO"
        fila["Observación"] = "; ".join(obs)
        filas.append(fila)
    cols = ["Estructura", "Tipo", "Armado", "Poste", "F horizontal máx. (N)", "F punta equiv. (N)",
            "FS poste", "Retenidas", "Tiro por retenida (N)", "FS retenida", "Cumple", "Observación"]
    return pd.DataFrame(filas).reindex(columns=cols)


def elegir_poste(cat: Catalogos, e: Estructura, fh: float, mb: float, fs_poste: float) -> str:
    """Poste de menor carga nominal (y luego menor longitud) que cumple FS y altura."""
    cand = cat.postes.dropna(subset=["carga_nominal_daN"]).sort_values(["carga_nominal_daN", "longitud_m"])
    for p in cand.itertuples(index=False):
        if not pd.isna(p.longitud_m) and e.altura_amarre >= p.longitud_m:
            continue
        if lleva_retenida(e):
            return str(p.codigo)
        libre = p.longitud_m - p.empotramiento_m if not pd.isna(p.empotramiento_m) else None
        f_punta = mb / libre if libre else fh
        if f_punta <= 0 or p.carga_nominal_daN * 10 / f_punta >= fs_poste:
            return str(p.codigo)
    return ""


def metrado(r: Resultado, cat: Catalogos, desperdicio_pct: float = 0.0,
            retenida: str | None = None, n_retenidas: dict[int, int] | None = None) -> pd.DataFrame:
    """Metrado básico: postes, armados, retenidas, materiales y conductor.

    Conductor: longitud de catenaria por vano en la hipótesis inicial (EDS) × n
    conductores × (1 + desperdicio).
    """
    p = r.proyecto
    filas: list[dict[str, object]] = []

    def agregar(item: str, desc: str, und: str, cant: float) -> None:
        filas.append({"Ítem": item, "Descripción": desc, "Unidad": und, "Cantidad": cant})

    for cod, n in pd.Series([e.poste for e in r.estructuras if e.poste]).value_counts().sort_index().items():
        pp = cat.poste(cod)
        agregar("Poste", f"{cod}" + (f" — {pp['material']}" if pp is not None else ""), "und", int(n))
    conteo_arm = pd.Series([e.armado for e in r.estructuras if e.armado]).value_counts().sort_index()
    for cod, n in conteo_arm.items():
        a = cat.armado(cod)
        agregar("Armado", f"{cod}" + (f" — {a['descripcion']}" if a is not None else ""), "und", int(n))
    n_ret = sum((n_retenidas or {}).get(e.numero, 1) or 1 for e in r.estructuras if lleva_retenida(e))
    if n_ret:
        cod = retenida or (cat.retenidas["codigo"].iloc[0] if len(cat.retenidas) else "Retenida")
        agregar("Retenida", str(cod), "und", n_ret)
    mats: dict[tuple[str, str], float] = {}
    for cod, n in conteo_arm.items():
        for m in cat.materiales[cat.materiales["armado"] == cod].itertuples(index=False):
            if not pd.isna(m.cantidad):
                mats[(m.material, m.unidad)] = mats.get((m.material, m.unidad), 0.0) + m.cantidad * n
    for (mat, und), cant in sorted(mats.items()):
        agregar("Material", mat, und, round(cant, 2))
    ini = p.hip_inicial
    largo = 0.0
    for k, v in enumerate(r.vanos):
        e = r.tramo_de_vano(k).estados[ini]
        largo += Catenaria(v.longitud, v.desnivel, e.H, e.w_resultante).longitud
    total = largo * p.n_conductores * (1 + desperdicio_pct / 100)
    agregar("Conductor", f"{p.conductor.nombre} ({p.n_conductores} × {largo:.1f} m, "
                         f"desperdicio {desperdicio_pct:g} %)", "m", round(total, 1))
    agregar("Conductor", f"{p.conductor.nombre} — masa", "kg", round(total * p.conductor.peso / G, 1))
    return pd.DataFrame(filas)
