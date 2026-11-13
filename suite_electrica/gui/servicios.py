"""Lógica de la app sin dependencias de Qt."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import numpy as np
import pandas as pd
from matplotlib.figure import Figure

from suite_electrica.core.geo.perfil import Perfil
from suite_electrica.core.geo.tin import es_mayor
from suite_electrica.core.units import formato_progresiva
from suite_electrica.documento import Documento, Topografia
from suite_electrica.linecraft import salidas as lc_salidas
from suite_electrica.linecraft.calculo import Resultado
from suite_electrica.topocraft import salidas as topo_salidas


def exportar_topo(doc: Documento, carpeta: str | Path) -> list[Path]:
    """Escribe plano.dxf y, si hay eje, perfil.pdf/.xlsx/.dxf."""
    c = Path(carpeta)
    c.mkdir(parents=True, exist_ok=True)
    t, p = doc.topografia(), doc.topo
    out = [topo_salidas.plano_dxf(c / "plano.dxf", t.puntos, t.tin, t.curvas, p.equidistancia,
                                  p.mayor_cada, doc.eje)]
    if t.perfil is not None:
        topo_salidas.perfil_pdf(c / "perfil.pdf", t.perfil)
        topo_salidas.perfil_excel(c / "perfil.xlsx", t.perfil, {"Proyecto": doc.nombre})
        topo_salidas.perfil_dxf(c / "perfil.dxf", t.perfil)
        out += [c / "perfil.pdf", c / "perfil.xlsx", c / "perfil.dxf"]
    return out


def exportar_linea(r: Resultado, carpeta: str | Path) -> list[Path]:
    """Escribe perfil DXF, planilla Excel y memoria PDF."""
    c = Path(carpeta)
    return [lc_salidas.perfil_dxf(c / "perfil_linea.dxf", r),
            lc_salidas.planilla_excel(c / "planilla_estructuras.xlsx", r),
            lc_salidas.memoria_pdf(c / "memoria_calculo.pdf", r)]


def tabla_perfil(perfil: Perfil) -> pd.DataFrame:
    return pd.DataFrame({
        "Progresiva": [formato_progresiva(s) for s in perfil.progresiva],
        "Este (m)": perfil.este.round(3), "Norte (m)": perfil.norte.round(3),
        "Cota (m)": perfil.cota.round(3), "Tipo": perfil.tipo,
    })


def dibujar_plano(fig: Figure, t: Topografia, doc: Documento) -> None:
    """Planta: puntos, curvas mayores/menores y eje."""
    fig.clear()
    ax = fig.add_subplot()
    ax.scatter(t.puntos.este, t.puntos.norte, s=2, c="0.55", lw=0)
    for c in t.curvas:
        mayor = es_mayor(c.cota, doc.topo.equidistancia, doc.topo.mayor_cada)
        xy = np.vstack([c.xy, c.xy[:1]]) if c.cerrada else c.xy
        ax.plot(xy[:, 0], xy[:, 1], color="#8b5a2b" if mayor else "#d2a679", lw=0.9 if mayor else 0.4)
    if doc.eje is not None:
        ax.plot(doc.eje[:, 0], doc.eje[:, 1], color="#c0392b", lw=1.5)
    ax.set_aspect("equal")
    ax.ticklabel_format(useOffset=False, style="plain")
    ax.tick_params(labelsize=7)
    ax.set_title(f"{len(t.puntos)} puntos · {len(t.curvas)} curvas (eq. {doc.topo.equidistancia:g} m)",
                 fontsize=9)
    fig.tight_layout()


def dibujar_perfil_topo(fig: Figure, perfil: Perfil) -> None:
    fig.clear()
    ax = fig.add_subplot()
    zmin = np.nanmin(perfil.cota)
    ax.fill_between(perfil.progresiva, zmin - 2, perfil.cota, color="#d9c7a3")
    ax.plot(perfil.progresiva, perfil.cota, color="#5a3e1b", lw=1)
    v = perfil.tipo == "V"
    ax.plot(perfil.progresiva[v], perfil.cota[v], "o", color="#c0392b", ms=4)
    ax.set_xlabel("Progresiva (m)", fontsize=8)
    ax.set_ylabel("Cota (m)", fontsize=8)
    ax.tick_params(labelsize=7)
    ax.grid(True, lw=0.3, color="0.85")
    ax.set_xlim(0, perfil.longitud)
    fig.tight_layout()


def abrir_externo(ruta: str | Path, programa: str | None = None) -> None:
    """Abre un archivo con ``programa`` (p. ej. CADCraft) o con la app del sistema."""
    ruta = str(Path(ruta).resolve())
    if programa:
        subprocess.Popen([programa, ruta])
    elif sys.platform.startswith("win"):
        os.startfile(ruta)  # type: ignore[attr-defined]
    elif sys.platform == "darwin":
        subprocess.Popen(["open", ruta])
    else:
        subprocess.Popen(["xdg-open", ruta])
