"""Presupuesto de obras eléctricas por unidades constructivas (UC), estilo PRESIO.

Tablas (CSV editables, sin precios por defecto):

* ``suministros.csv``: ``codigo, descripcion, unidad, precio, peso_kg, fuente``.
* ``unidades.csv``: ``uc, suministro, cantidad`` (composición de cada UC).
* ``montaje.csv``: ``uc, descripcion, unidad, precio, fuente`` (partida de montaje por UC).
* ``metrado.csv``: ``rubro, uc, cantidad`` (rubro: LP, RP, RS, SED, CD, AP…).

Por rubro::

    Suministro  = Σ_UC n_UC · Σ_s q_s · precio_s
    Montaje     = Σ_UC n_UC · precio_montaje_UC
    Transporte  = Σ_UC n_UC · Σ_s q_s · peso_s · flete_por_kg
    CD          = Suministro + Montaje + Transporte
    GG = CD · gg%,  Utilidad = CD · util%,  Subtotal = CD + GG + Utilidad
    IGV = Subtotal · igv%,  Total = Subtotal + IGV
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd

COLS = {
    "suministros": ["codigo", "descripcion", "unidad", "precio", "peso_kg", "fuente"],
    "unidades": ["uc", "suministro", "cantidad"],
    "montaje": ["uc", "descripcion", "unidad", "precio", "fuente"],
    "metrado": ["rubro", "uc", "cantidad"],
}
NUM = {"precio", "peso_kg", "cantidad"}


def normalizar(nombre: str, df: pd.DataFrame) -> pd.DataFrame:
    df = df.copy().reindex(columns=COLS[nombre])
    for c in df.columns:
        df[c] = (pd.to_numeric(df[c], errors="coerce") if c in NUM
                 else df[c].fillna("").astype(str).str.strip())
    return df


@dataclass
class Presupuesto:
    nombre: str
    suministros: pd.DataFrame
    unidades: pd.DataFrame
    montaje: pd.DataFrame
    metrado: pd.DataFrame
    flete_por_kg: float = 0.0
    gastos_generales_pct: float = 0.0
    utilidad_pct: float = 0.0
    igv_pct: float = 0.0
    moneda: str = "S/"
    fuentes: dict[str, str] = field(default_factory=dict)

    def __post_init__(self) -> None:
        for n in COLS:
            setattr(self, n, normalizar(n, getattr(self, n)))


@dataclass
class ResultadoPresupuesto:
    presupuesto: Presupuesto
    resumen: pd.DataFrame
    """Por rubro + TOTAL: suministro, montaje, transporte, CD, GG, utilidad, subtotal, IGV, total."""
    suministro: pd.DataFrame
    """Materiales consolidados por rubro: cantidad, precio, parcial, peso."""
    montaje: pd.DataFrame
    planilla_uc: pd.DataFrame
    """Por rubro y UC: cantidad y costo unitario/total de suministro, montaje y transporte."""
    avisos: list[str]


def calcular(p: Presupuesto) -> ResultadoPresupuesto:
    avisos: list[str] = []
    sm, uc, mo, me = p.suministros.set_index("codigo"), p.unidades, p.montaje, p.metrado
    if sm.index.duplicated().any():
        raise ValueError(f"Suministros repetidos: {sorted(set(sm.index[sm.index.duplicated()]))}")
    me = me[me["cantidad"].fillna(0) != 0]
    sin_def = sorted(set(me["uc"]) - set(uc["uc"]) - set(mo["uc"]))
    if sin_def:
        avisos.append(f"UC del metrado sin composición ni montaje (costo 0): {', '.join(sin_def)}")
    falta_s = sorted(set(uc["suministro"]) - set(sm.index))
    if falta_s:
        raise ValueError(f"Suministros usados en unidades.csv y no definidos: {falta_s}")
    sin_precio = sorted(set(sm.index[sm["precio"].isna()]) & set(uc["suministro"]))
    if sin_precio:
        avisos.append(f"Suministros sin precio (costo 0): {', '.join(sin_precio)}")
    for nombre, df in (("suministros", p.suministros), ("montaje", p.montaje)):
        pend = df[(df["fuente"] == "") | df["fuente"].str.upper().str.contains("PENDIENTE|EJEMPLO")]
        if len(pend):
            avisos.append(f"PENDIENTE_REVISION: precios de {nombre} sin fuente verificada ({len(pend)} filas)")
    for k in ("flete_por_kg", "gastos_generales_pct", "utilidad_pct", "igv_pct"):
        f = p.fuentes.get(k, "")
        if getattr(p, k) and (not f or "PENDIENTE" in f.upper() or "EJEMPLO" in f.upper()):
            avisos.append(f"PENDIENTE_REVISION: '{k}' -> {f or 'sin fuente'}")

    # Materiales por rubro: metrado (rubro, uc, n) × composición (uc, suministro, q)
    mat = me.merge(uc, on="uc", suffixes=("_uc", ""))
    mat["cant"] = mat["cantidad_uc"] * mat["cantidad"]
    mat = mat.groupby(["rubro", "suministro"], as_index=False)["cant"].sum()
    mat = mat.join(sm[["descripcion", "unidad", "precio", "peso_kg"]], on="suministro")
    mat["parcial"] = mat["cant"] * mat["precio"].fillna(0)
    mat["peso"] = mat["cant"] * mat["peso_kg"].fillna(0)
    mat["transporte"] = mat["peso"] * p.flete_por_kg

    mon = me.merge(mo, on="uc", how="inner")
    mon["parcial"] = mon["cantidad"] * mon["precio"].fillna(0)

    # Planilla por UC (costos unitarios)
    cu = uc.join(sm[["precio", "peso_kg"]], on="suministro")
    cu = (cu.assign(s=cu["cantidad"] * cu["precio"].fillna(0), w=cu["cantidad"] * cu["peso_kg"].fillna(0))
          .groupby("uc")[["s", "w"]].sum())
    pl = me.join(cu, on="uc").join(mo.groupby("uc")["precio"].sum().rename("m"), on="uc").fillna(
        {"s": 0.0, "w": 0.0, "m": 0.0})
    planilla = pd.DataFrame({
        "Rubro": pl["rubro"], "UC": pl["uc"], "Cantidad": pl["cantidad"],
        "Suministro unit.": pl["s"].round(2), "Montaje unit.": pl["m"].round(2),
        "Peso unit. (kg)": pl["w"].round(3), "Transporte unit.": (pl["w"] * p.flete_por_kg).round(2),
        "Total": (pl["cantidad"] * (pl["s"] + pl["m"] + pl["w"] * p.flete_por_kg)).round(2)})

    rubros = list(dict.fromkeys(me["rubro"]))
    filas = []
    for r in rubros + ["TOTAL"]:
        sel = (lambda d: d) if r == "TOTAL" else (lambda d, r=r: d[d["rubro"] == r])
        s, t, m = sel(mat)["parcial"].sum(), sel(mat)["transporte"].sum(), sel(mon)["parcial"].sum()
        cd = s + m + t
        gg, ut = cd * p.gastos_generales_pct / 100, cd * p.utilidad_pct / 100
        sub = cd + gg + ut
        igv = sub * p.igv_pct / 100
        filas.append({"Rubro": r, "Suministro": s, "Montaje": m, "Transporte": t, "Costo directo": cd,
                      "Gastos generales": gg, "Utilidad": ut, "Subtotal": sub, "IGV": igv, "Total": sub + igv})
    resumen = pd.DataFrame(filas).round(2)
    suministro = mat.rename(columns={"rubro": "Rubro", "suministro": "Código", "descripcion": "Descripción",
                                     "unidad": "Unidad", "cant": "Cantidad", "precio": "Precio",
                                     "parcial": "Parcial", "peso": "Peso (kg)", "transporte": "Transporte"})[
        ["Rubro", "Código", "Descripción", "Unidad", "Cantidad", "Precio", "Parcial", "Peso (kg)", "Transporte"]]
    montaje = mon.rename(columns={"rubro": "Rubro", "uc": "UC", "descripcion": "Partida", "unidad": "Unidad",
                                  "cantidad": "Cantidad", "precio": "Precio", "parcial": "Parcial"})[
        ["Rubro", "UC", "Partida", "Unidad", "Cantidad", "Precio", "Parcial"]]
    return ResultadoPresupuesto(p, resumen, suministro.round(3), montaje.round(2), planilla, avisos)


def metrado_linecraft(metrado: pd.DataFrame, rubro: str = "LP") -> pd.DataFrame:
    """Convierte el metrado de LineCraft (columna ``Código``) a ``rubro, uc, cantidad``.

    Solo el conductor pasa de m a km (UC = nombre del conductor, cantidad en km); los
    materiales de los armados conservan su unidad.
    """
    cond = metrado["Ítem"] == "Conductor"
    m = metrado[(metrado["Código"] != "") & ~(cond & (metrado["Unidad"] != "m"))]
    cant = np.where((m["Ítem"] == "Conductor") & (m["Unidad"] == "m"), m["Cantidad"] / 1000, m["Cantidad"])
    return pd.DataFrame({"rubro": rubro, "uc": m["Código"].to_numpy(), "cantidad": cant})


def cargar(ruta: str | Path) -> Presupuesto:
    """``presupuesto.json`` con rutas relativas a los cuatro CSV y los parámetros."""
    ruta = Path(ruta)
    cfg = json.loads(ruta.read_text(encoding="utf-8"))
    tablas = {n: pd.read_csv(ruta.parent / cfg.pop(n), comment="#", skipinitialspace=True, dtype=str)
              for n in COLS}
    cfg.setdefault("nombre", ruta.stem)
    return Presupuesto(**tablas, **cfg)


def excel(ruta: str | Path, r: ResultadoPresupuesto) -> Path:
    """Hojas Resumen, Suministro, Montaje, PlanillaUC y Avisos."""
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    with pd.ExcelWriter(ruta) as xw:
        r.resumen.to_excel(xw, sheet_name="Resumen", index=False)
        r.suministro.to_excel(xw, sheet_name="Suministro", index=False)
        r.montaje.to_excel(xw, sheet_name="Montaje", index=False)
        r.planilla_uc.to_excel(xw, sheet_name="PlanillaUC", index=False)
        pd.DataFrame({"Aviso": r.avisos or ["Sin avisos"]}).to_excel(xw, sheet_name="Avisos", index=False)
    return ruta
