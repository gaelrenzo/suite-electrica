"""Red radial de distribución (BT o MT): demandas, caída de tensión y pérdidas.

Método de *sumatoria de potencias* (red radial, cargas balanceadas):

* Demanda por nodo: ``P_SP = lotes · CE_SP · fs_SP``, ``P_CE = kW_CE · fs_CE``,
  ``P_AP = lámparas · kW_lámpara · fs_AP``. ``fs_SP`` puede depender del número
  total de abonados (tabla escalonada).
* Potencia acumulada de cada tramo = suma de las demandas aguas abajo.
* Corriente ``I = P / (k · V · cos φ)``; ``k = √3`` en trifásico, 1 en monofásico.
* Caída en el tramo ``ΔV = K · I · L / 1000`` con ``K`` (Ω/km) del catálogo;
  si falta, ``K = c (r cos φ + x sen φ)`` con ``c = √3`` (3φ) o 2 (1φ, ida y vuelta).
  ``ΔV`` queda referida a ``V`` (fase-fase en 3φ; fase-neutro o fase-fase en 1φ).
* Pérdidas ``n_f · I² · r · L`` (``n_f`` = 3 en 3φ, 2 en 1φ); energía con el factor
  de pérdidas ``fp = 0,3 FC + 0,7 FC²`` (aproximación de Buller–Woodrow).

El alumbrado público (AP) se calcula aparte sobre el hilo de AP (columna
``k_ap_ohm_km`` del catálogo); si un conductor no la tiene, el AP se suma al SP.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd

SISTEMAS = {"3F": (math.sqrt(3), 3, math.sqrt(3)), "1F": (1.0, 2, 2.0)}
"""Sistema -> (k de corriente, conductores con pérdidas, c de K desde r y x)."""

COLS_NODOS = ["nodo", "padre", "longitud_m", "conductor", "lotes", "kw_ce", "lamparas", "este", "norte"]
COLS_COND = ["formacion", "k_ohm_km", "r_ohm_km", "x_ohm_km", "k_ap_ohm_km", "r_ap_ohm_km",
             "corriente_max_A", "fuente"]
PARAMETROS_NORMATIVOS = ("calificacion_kw_lote", "fs_sp", "caida_max_pct", "conductores")


@dataclass
class Red:
    """Datos de una red radial. Potencias en kW, tensiones en V, longitudes en m."""

    nombre: str
    sistema: str
    """``3F`` o ``1F``."""
    tension_V: float
    """Tensión de referencia de la caída (3φ: fase-fase)."""
    nodos: pd.DataFrame
    conductores: pd.DataFrame
    calificacion_kw_lote: float
    fs_sp: float
    fp_sp: float = 0.9
    fs_ce: float = 1.0
    kw_lampara: float = 0.0
    fs_ap: float = 1.0
    fp_ap: float = 1.0
    tension_ap_V: float | None = None
    """Tensión del circuito de AP (por defecto la fase-neutro: V/√3 en 3φ)."""
    caida_max_pct: float = 0.0
    """0 = no verificar."""
    perdidas_max_pct: float = 0.0
    factor_carga: float = 0.0
    """FC para pérdidas de energía (0 = no calcular kWh)."""
    transformador_kva: float = 0.0
    factor_sobrecarga: float = 1.0
    tabla_fs: list[tuple[int, float]] = field(default_factory=list)
    """``[(abonados, fs)]``: fs para el número de abonados ≥ cada umbral."""
    fuentes: dict[str, str] = field(default_factory=dict)

    def fs_efectivo(self) -> float:
        n = int(self.nodos["lotes"].sum())
        fs = self.fs_sp
        for umbral, v in sorted(self.tabla_fs):
            if n >= umbral:
                fs = v
        return fs

    def avisos_fuentes(self) -> list[str]:
        av = []
        for k in PARAMETROS_NORMATIVOS:
            f = self.fuentes.get(k, "").strip()
            if not f or "PENDIENTE" in f.upper() or "EJEMPLO" in f.upper():
                av.append(f"PENDIENTE_REVISION: '{k}' -> {f or 'sin fuente'}")
        return av


@dataclass
class ResultadoRed:
    red: Red
    tramos: pd.DataFrame
    """Por nodo (tramo padre→nodo): potencias, corriente, ΔV, %ΔV acumulado, pérdidas."""
    resumen: dict[str, float]
    avisos: list[str]
    alertas: pd.DataFrame

    @property
    def cumple(self) -> bool:
        return self.alertas.empty


def _normalizar(df: pd.DataFrame, cols: list[str], texto: set[str]) -> pd.DataFrame:
    df = df.copy()
    for c in cols:
        if c not in df:
            df[c] = "" if c in texto else np.nan
    for c in cols:
        if c in texto:
            df[c] = df[c].fillna("").astype(str).str.strip()
        else:
            df[c] = pd.to_numeric(df[c], errors="coerce")
    return df


def _orden_topologico(nodos: pd.DataFrame) -> list[int]:
    """Índices de filas de la raíz a las hojas; valida árbol con una sola raíz."""
    raices = nodos.index[nodos["padre"] == ""].tolist()
    if len(raices) != 1:
        raise ValueError(f"La red debe tener una sola raíz (padre vacío); hay {len(raices)}")
    if nodos["nodo"].duplicated().any():
        raise ValueError(f"Nodos repetidos: {sorted(set(nodos.loc[nodos['nodo'].duplicated(), 'nodo']))}")
    hijos: dict[str, list[int]] = {}
    for i, p in nodos["padre"].items():
        if p:
            hijos.setdefault(p, []).append(i)
    faltan = set(hijos) - set(nodos["nodo"])
    if faltan:
        raise ValueError(f"Padres inexistentes: {sorted(faltan)}")
    orden, pila = [], [raices[0]]
    while pila:
        i = pila.pop()
        orden.append(i)
        pila.extend(reversed(hijos.get(nodos.at[i, "nodo"], [])))
    if len(orden) != len(nodos):
        raise ValueError("La red tiene ciclos o nodos desconectados de la raíz")
    return orden


def _k(cond: pd.Series, sistema: str, fp: float, ap: bool) -> float:
    k, r, x = ((cond["k_ap_ohm_km"], cond["r_ap_ohm_km"], 0.0) if ap
               else (cond["k_ohm_km"], cond["r_ohm_km"], cond["x_ohm_km"]))
    if not pd.isna(k):
        return float(k)
    if pd.isna(r):
        return math.nan
    c = 2.0 if ap else SISTEMAS[sistema][2]
    return c * (r * fp + (0 if pd.isna(x) else x) * math.sqrt(1 - fp * fp))


def calcular(red: Red) -> ResultadoRed:
    """Flujo radial por sumatoria de potencias."""
    if red.sistema not in SISTEMAS:
        raise ValueError(f"Sistema {red.sistema!r}; use {sorted(SISTEMAS)}")
    n = _normalizar(red.nodos, COLS_NODOS, {"nodo", "padre", "conductor"}).reset_index(drop=True)
    cat = _normalizar(red.conductores, COLS_COND, {"formacion", "fuente"}).set_index("formacion")
    for c in ("lotes", "kw_ce", "lamparas", "longitud_m"):
        n[c] = n[c].fillna(0.0)
    orden = _orden_topologico(n)
    avisos = red.avisos_fuentes()
    sin_cat = sorted(set(n.loc[n["padre"] != "", "conductor"]) - set(cat.index))
    if sin_cat:
        raise ValueError(f"Conductores no definidos en el catálogo: {sin_cat}")
    pend = cat[(cat["fuente"] == "") | cat["fuente"].str.upper().str.contains("PENDIENTE")]
    if len(pend):
        avisos.append(f"PENDIENTE_REVISION: catálogo de conductores sin fuente ({', '.join(pend.index)})")

    fs = red.fs_efectivo()
    n["P_SP"] = n["lotes"] * red.calificacion_kw_lote * fs + n["kw_ce"] * red.fs_ce
    n["P_AP"] = n["lamparas"] * red.kw_lampara * red.fs_ap
    k_i, n_f, _ = SISTEMAS[red.sistema]
    v_ap = red.tension_ap_V or (red.tension_V / math.sqrt(3) if red.sistema == "3F" else red.tension_V)
    ap_separado = {f: not pd.isna(cat.at[f, "k_ap_ohm_km"]) or not pd.isna(cat.at[f, "r_ap_ohm_km"])
                   for f in cat.index}

    # Acumulado aguas abajo (recorrido hojas -> raíz)
    pos = {n.at[i, "nodo"]: i for i in n.index}
    acum_sp, acum_ap = n["P_SP"].to_numpy(float).copy(), n["P_AP"].to_numpy(float).copy()
    for i in reversed(orden):
        p = n.at[i, "padre"]
        if p:
            acum_sp[pos[p]] += acum_sp[i]
            acum_ap[pos[p]] += acum_ap[i]
    n["Pacum_SP"], n["Pacum_AP"] = acum_sp, acum_ap

    filas, dv_sp, dv_ap = {}, {}, {}
    for i in orden:
        nodo, padre = n.at[i, "nodo"], n.at[i, "padre"]
        if not padre:
            dv_sp[nodo] = dv_ap[nodo] = 0.0
            continue
        c = cat.loc[n.at[i, "conductor"]]
        L = n.at[i, "longitud_m"] / 1000
        sep = ap_separado[n.at[i, "conductor"]]
        p_sp = acum_sp[i] + (0.0 if sep else acum_ap[i])
        I = p_sp * 1e3 / (k_i * red.tension_V * red.fp_sp)
        K = _k(c, red.sistema, red.fp_sp, ap=False)
        if math.isnan(K):
            raise ValueError(f"Conductor {c.name}: falta k_ohm_km o r_ohm_km")
        dv = K * I * L
        r = c["r_ohm_km"]
        perd = n_f * I * I * r * L if not pd.isna(r) else math.nan  # r Ω/km · L km = Ω
        dv_sp[nodo] = dv_sp[padre] + dv
        fila = {"Nodo": nodo, "Padre": padre, "Long. (m)": round(L * 1e3, 2),
                "Formación": c.name, "Lotes": n.at[i, "lotes"], "P SP (kW)": round(n.at[i, "P_SP"], 3),
                "P acum. (kW)": round(p_sp, 3), "K (Ω/km)": round(K, 4), "I (A)": round(I, 2),
                "ΔV tramo (V)": round(dv, 3), "ΔV acum. (V)": round(dv_sp[nodo], 3),
                "%ΔV": round(100 * dv_sp[nodo] / red.tension_V, 3), "Pérdidas (W)": round(perd, 2),
                "I máx. (A)": c["corriente_max_A"]}
        if sep:
            I_ap = acum_ap[i] * 1e3 / (v_ap * red.fp_ap)
            K_ap = _k(c, red.sistema, red.fp_ap, ap=True)
            dv_ap[nodo] = dv_ap[padre] + K_ap * I_ap * L
            r_ap = c["r_ap_ohm_km"]
            fila.update({"P acum. AP (kW)": round(acum_ap[i], 3), "I AP (A)": round(I_ap, 2),
                         "%ΔV AP": round(100 * dv_ap[nodo] / v_ap, 3),
                         "Pérdidas AP (W)": round(2 * I_ap**2 * r_ap * L, 2) if not pd.isna(r_ap) else math.nan})
        else:
            dv_ap[nodo] = dv_ap[padre]
        filas[i] = fila
    tramos = pd.DataFrame([filas[i] for i in orden if i in filas])

    raiz = orden[0]
    md = float(acum_sp[raiz] + acum_ap[raiz])
    perd_kw = float(np.nansum(tramos.get("Pérdidas (W)", pd.Series(dtype=float)))
                    + np.nansum(tramos.get("Pérdidas AP (W)", pd.Series(dtype=float)))) / 1e3
    resumen = {"Abonados": int(n["lotes"].sum()), "Factor de simultaneidad SP": fs,
               "M.D. SP (kW)": round(float(acum_sp[raiz]), 3), "M.D. AP (kW)": round(float(acum_ap[raiz]), 3),
               "M.D. total (kW)": round(md, 3),
               "Potencia aparente (kVA)": round(acum_sp[raiz] / red.fp_sp + acum_ap[raiz] / red.fp_ap, 3),
               "Pérdidas (kW)": round(perd_kw, 4),
               "Pérdidas (% M.D.)": round(100 * perd_kw / md, 3) if md else 0.0,
               "%ΔV máx.": round(float(tramos["%ΔV"].max()), 3) if len(tramos) else 0.0}
    if "%ΔV AP" in tramos:
        resumen["%ΔV AP máx."] = round(float(tramos["%ΔV AP"].max()), 3)
    if red.factor_carga:
        fper = 0.3 * red.factor_carga + 0.7 * red.factor_carga**2
        resumen["Factor de pérdidas"] = round(fper, 4)
        resumen["Energía perdida (kWh/año)"] = round(perd_kw * fper * 8760, 1)
        resumen["Energía vendida (kWh/año)"] = round(md * red.factor_carga * 8760, 1)

    # Alertas (exceso E = (P* − P)/P, mismo criterio que LineCraft)
    al: list[tuple[str, str, float, str]] = []
    def add(elem: str, cod: str, ev: float, perm: float, det: str) -> None:
        if perm > 0 and ev > perm + 1e-9:
            al.append((elem, cod, round(100 * (ev - perm) / perm, 1), det))
    for t in tramos.to_dict("records"):
        if red.caida_max_pct:
            add(t["Nodo"], "DV", t["%ΔV"], red.caida_max_pct, f"%ΔV {t['%ΔV']:.2f} % > {red.caida_max_pct:g} %")
        if not pd.isna(t["I máx. (A)"]):
            add(f"{t['Padre']}-{t['Nodo']}", "I", t["I (A)"], t["I máx. (A)"],
                f"I {t['I (A)']:.1f} A > {t['I máx. (A)']:g} A")
    if red.caida_max_pct and "%ΔV AP" in tramos:
        for nodo, v in zip(tramos["Nodo"], tramos["%ΔV AP"]):
            if not pd.isna(v):
                add(nodo, "DV-AP", v, red.caida_max_pct, f"%ΔV AP {v:.2f} % > {red.caida_max_pct:g} %")
    if red.perdidas_max_pct:
        add(red.nombre, "PERD", resumen["Pérdidas (% M.D.)"], red.perdidas_max_pct,
            f"pérdidas {resumen['Pérdidas (% M.D.)']:.2f} % > {red.perdidas_max_pct:g} %")
    if red.transformador_kva:
        cap = red.transformador_kva * red.factor_sobrecarga
        add(n.at[raiz, "nodo"], "TR", resumen["Potencia aparente (kVA)"], cap,
            f"{resumen['Potencia aparente (kVA)']:.1f} kVA > {cap:.1f} kVA")
    alertas = pd.DataFrame(al, columns=["Elemento", "Código", "Exceso (%)", "Detalle"]).sort_values(
        "Exceso (%)", ascending=False, kind="stable").reset_index(drop=True)
    return ResultadoRed(red, tramos, resumen, avisos, alertas)


def cargar(ruta: str | Path) -> Red:
    """Proyecto JSON: parámetros + ``nodos`` (CSV) + ``conductores`` (CSV), rutas relativas."""
    ruta = Path(ruta)
    cfg = json.loads(ruta.read_text(encoding="utf-8"))
    base = ruta.parent
    leer = lambda k: pd.read_csv(base / cfg.pop(k), comment="#", skipinitialspace=True, dtype={"nodo": str, "padre": str})
    nodos, cond = leer("nodos"), leer("conductores")
    cfg["tabla_fs"] = [tuple(x) for x in cfg.get("tabla_fs", [])]
    cfg.setdefault("nombre", ruta.stem)
    return Red(nodos=nodos, conductores=cond, **cfg)
