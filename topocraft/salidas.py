"""Exportación de TopoCraft: plano DXF, perfil DXF/PDF y tabla Excel."""

from __future__ import annotations

import math
from pathlib import Path

import numpy as np
import pandas as pd
from matplotlib.figure import Figure
from numpy.typing import NDArray

from core.dxf.escritura import asegurar_capa, guardar, nuevo_documento
from core.geo.perfil import Perfil
from core.geo.tin import TIN, Curva, es_mayor
from core.units import formato_progresiva

CAPAS: dict[str, int] = {
    "PUNTOS": 7, "TIN": 8, "CURVAS_MAYORES": 30, "CURVAS_MENORES": 33, "EJE": 1, "PERFIL": 3,
}
"""Capas del proyecto y su color ACI."""

ESCALAS: tuple[int, ...] = (500, 1000, 2000, 2500, 5000, 10000, 20000, 25000, 50000, 100000)
"""Escalas normalizadas para el perfil (denominador)."""


# --------------------------------------------------------------------------- plano
def plano_dxf(
    ruta: str | Path,
    puntos: pd.DataFrame,
    tin: TIN | None = None,
    curvas: list[Curva] | None = None,
    equidistancia: float = 1.0,
    mayor_cada: int = 5,
    eje: NDArray[np.float64] | None = None,
    altura_texto: float = 1.0,
) -> Path:
    """Escribe el plano topográfico en DXF (coordenadas UTM en metros).

    Capas: PUNTOS (punto + rótulo de cota), TIN (3DFACE), CURVAS_MAYORES y
    CURVAS_MENORES (LWPOLYLINE con elevación = cota), EJE.
    """
    doc = nuevo_documento()
    msp = doc.modelspace()
    for nombre, color in CAPAS.items():
        if nombre != "PERFIL":
            asegurar_capa(doc, nombre, color)
    h = altura_texto

    for r in puntos.itertuples(index=False):
        msp.add_point((r.este, r.norte, r.cota), dxfattribs={"layer": "PUNTOS"})
        msp.add_text(f"{r.cota:.2f}", height=h, dxfattribs={"layer": "PUNTOS"}).set_placement(
            (r.este + 0.5 * h, r.norte + 0.2 * h))

    if tin is not None:
        for t in tin.triangulos_activos:
            p = [tuple(tin.xyz[i]) for i in t]
            msp.add_3dface([p[0], p[1], p[2], p[2]], dxfattribs={"layer": "TIN"})

    for c in curvas or []:
        mayor = es_mayor(c.cota, equidistancia, mayor_cada)
        capa = "CURVAS_MAYORES" if mayor else "CURVAS_MENORES"
        msp.add_lwpolyline(c.xy.tolist(), close=c.cerrada,
                           dxfattribs={"layer": capa, "elevation": c.cota})
        if mayor and len(c.xy) >= 2:
            i = len(c.xy) // 2
            a, b = c.xy[i - 1], c.xy[i]
            ang = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
            ang = ang - 180 if ang > 90 else ang + 180 if ang < -90 else ang
            txt = f"{c.cota:g}"
            msp.add_text(txt, height=1.5 * h, rotation=ang, dxfattribs={"layer": capa}).set_placement(
                tuple((a + b) / 2))

    if eje is not None:
        msp.add_lwpolyline(np.asarray(eje)[:, :2].tolist(), dxfattribs={"layer": "EJE"})
    return guardar(doc, ruta)


# --------------------------------------------------------------------------- perfil
def _tramos_validos(s: NDArray[np.float64], z: NDArray[np.float64]) -> list[NDArray[np.float64]]:
    """Divide la línea de terreno en tramos sin NaN."""
    ok = ~np.isnan(z)
    tramos, actual = [], []
    for si, zi, v in zip(s, z, ok):
        if v:
            actual.append((si, zi))
        elif actual:
            tramos.append(np.array(actual))
            actual = []
    if actual:
        tramos.append(np.array(actual))
    return [t for t in tramos if len(t) >= 2]


def cota_referencia(perfil: Perfil, multiplo: float = 5.0, margen: float = 5.0) -> float:
    """Cota de comparación bajo el terreno mínimo, múltiplo de ``multiplo`` (m)."""
    return math.floor((np.nanmin(perfil.cota) - margen) / multiplo) * multiplo


def perfil_dxf(ruta: str | Path, perfil: Perfil, exageracion: float = 10.0) -> Path:
    """Dibuja el perfil en DXF (capa PERFIL).

    Coordenadas de dibujo: x = progresiva (m), y = (cota - cota_ref) * exageración.
    """
    doc = nuevo_documento()
    msp = doc.modelspace()
    asegurar_capa(doc, "PERFIL", CAPAS["PERFIL"])
    at = {"layer": "PERFIL"}
    ref = cota_referencia(perfil)
    L = perfil.longitud
    h = min(max(L / 250, 0.5), 5.0)

    for t in _tramos_validos(perfil.progresiva, perfil.cota):
        msp.add_lwpolyline(np.column_stack([t[:, 0], (t[:, 1] - ref) * exageracion]).tolist(),
                           dxfattribs=at)
    msp.add_line((0, 0), (L, 0), dxfattribs=at)
    msp.add_text(f"COTA REF. {ref:.2f} m   EXAG. V {exageracion:g}x", height=h,
                 dxfattribs=at).set_placement((0, -9 * h))
    p = perfil.principales()
    for s, z in zip(p.progresiva, p.cota):
        ytop = 0.0 if np.isnan(z) else (z - ref) * exageracion
        msp.add_line((s, 0), (s, ytop), dxfattribs=at)
        msp.add_text(formato_progresiva(s), height=h, rotation=90, dxfattribs=at).set_placement(
            (s + 0.4 * h, -7 * h))
        if not np.isnan(z):
            msp.add_text(f"{z:.2f}", height=h, rotation=90, dxfattribs=at).set_placement(
                (s + 0.4 * h, -2.5 * h))
    return guardar(doc, ruta)


def escala_automatica(longitud: float, ancho_max_mm: float = 900.0) -> int:
    """Menor escala normalizada con la que el perfil cabe en ``ancho_max_mm``."""
    for e in ESCALAS:
        if longitud / e * 1000 <= ancho_max_mm:
            return e
    return ESCALAS[-1]


def perfil_pdf(
    ruta: str | Path,
    perfil: Perfil,
    esc_h: int | None = None,
    esc_v: int | None = None,
    titulo: str = "PERFIL LONGITUDINAL",
) -> tuple[int, int]:
    """Gráfico del perfil a escala real en PDF (imprimir al 100 %).

    Si no se dan escalas, se elige H automática y V = H/10.

    Returns:
        (esc_h, esc_v) usadas (denominadores).
    """
    L = perfil.longitud
    esc_h = esc_h or escala_automatica(L)
    esc_v = esc_v or max(esc_h // 10, 1)
    zbot = cota_referencia(perfil)
    ztop = math.ceil((np.nanmax(perfil.cota) + 5) / 5) * 5
    w_mm = L / esc_h * 1000
    h_mm = (ztop - zbot) / esc_v * 1000
    if h_mm < 40:  # altura mínima legible sin alterar la escala
        ztop = zbot + 40 * esc_v / 1000
        h_mm = 40.0

    ml, mr, mt, banda, mb = 30.0, 10.0, 15.0, 40.0, 10.0
    W, H = ml + w_mm + mr, mb + banda + h_mm + mt
    fig = Figure(figsize=(W / 25.4, H / 25.4))
    ax = fig.add_axes((ml / W, (mb + banda) / H, w_mm / W, h_mm / H))
    for t in _tramos_validos(perfil.progresiva, perfil.cota):
        ax.fill_between(t[:, 0], zbot, t[:, 1], color="#d9c7a3", alpha=0.6, lw=0)
        ax.plot(t[:, 0], t[:, 1], color="#5a3e1b", lw=1.0)
    ax.set_xlim(0, L)
    ax.set_ylim(zbot, ztop)
    ax.set_ylabel("Cota (m)", fontsize=7)
    ax.tick_params(labelsize=6)
    ax.grid(True, lw=0.3, color="0.8")
    ax.set_title(f"{titulo}   Esc. H 1:{esc_h}   V 1:{esc_v}", fontsize=8, loc="left")
    ax.tick_params(axis="x", labelbottom=False)

    bd = fig.add_axes((ml / W, mb / H, w_mm / W, banda / H), sharex=ax)
    bd.set_ylim(0, 1)
    bd.set_yticks([])
    bd.tick_params(axis="x", labelbottom=False, length=0)
    bd.axhline(0.5, color="k", lw=0.4)
    fig.text(2 / W, (mb + 0.75 * banda) / H, "PROGRESIVA", fontsize=6, va="center")
    fig.text(2 / W, (mb + 0.25 * banda) / H, "COTA TERRENO", fontsize=6, va="center")
    p = perfil.principales()
    min_sep = 3.5 * esc_h / 1000  # 3.5 mm en papel, en metros de terreno
    ultimo = -math.inf
    for s, z in zip(p.progresiva, p.cota):
        if s - ultimo < min_sep and not math.isclose(s, L):
            continue
        ultimo = s
        bd.axvline(s, color="0.6", lw=0.3)
        bd.text(s, 0.75, formato_progresiva(s), rotation=90, fontsize=5, ha="center", va="center")
        if not np.isnan(z):
            bd.text(s, 0.25, f"{z:.2f}", rotation=90, fontsize=5, ha="center", va="center")
            ax.vlines(s, zbot, z, color="0.7", lw=0.3)
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(ruta)
    return esc_h, esc_v


def perfil_excel(ruta: str | Path, perfil: Perfil, datos: dict[str, object] | None = None) -> Path:
    """Tabla de progresivas en Excel (hojas ``Perfil`` y ``Datos``)."""
    tabla = pd.DataFrame({
        "Progresiva": [formato_progresiva(s) for s in perfil.progresiva],
        "Distancia (m)": perfil.progresiva.round(3),
        "Este (m)": perfil.este.round(3),
        "Norte (m)": perfil.norte.round(3),
        "Cota terreno (m)": perfil.cota.round(3),
        "Tipo": perfil.tipo,
    })
    info = {"Longitud (m)": round(perfil.longitud, 3),
            "Tipo V/E/T": "V vértice del eje, E estación, T quiebre del TIN"}
    info.update(datos or {})
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    with pd.ExcelWriter(ruta, engine="openpyxl") as xw:
        tabla.to_excel(xw, sheet_name="Perfil", index=False)
        pd.DataFrame(list(info.items()), columns=["Dato", "Valor"]).to_excel(
            xw, sheet_name="Datos", index=False)
    return ruta
