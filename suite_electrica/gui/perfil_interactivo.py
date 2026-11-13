"""Perfil interactivo de la línea: agregar, mover y editar estructuras con el ratón.

* Doble clic en zona libre: agrega estructura.
* Clic y arrastrar un poste: lo mueve (inicio, fin y vértices están fijos).
* Clic derecho en un poste: menú de tipo, armado, poste, altura, retenidas y eliminar.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

from suite_electrica.core.units import formato_progresiva
from suite_electrica.gui.widgets import Lienzo
from suite_electrica.linecraft.estructuras import linea_conductor

if TYPE_CHECKING:
    from suite_electrica.linecraft.calculo import Resultado

TOL_PX = 8
"""Tolerancia en píxeles para seleccionar un poste."""


class PerfilInteractivo(Lienzo):
    """Lienzo del perfil con edición por ratón. ``panel`` implementa las acciones."""

    def __init__(self, panel: Any) -> None:
        super().__init__()
        self.panel = panel
        self.resultado: Resultado | None = None
        self.seleccion: int | None = None
        self._arrastre: int | None = None
        self._fantasma = None
        self._ax = None
        self._vista: tuple | None = None
        self.canvas.mpl_connect("button_press_event", self.al_presionar)
        self.canvas.mpl_connect("motion_notify_event", self.al_mover)
        self.canvas.mpl_connect("button_release_event", self.al_soltar)

    # ------------------------------------------------------------------ dibujo
    def dibujar(self, r: Resultado, bloqueadas: list[bool]) -> None:
        self.resultado = r
        if self._ax is not None and self._vista is not None:
            self._vista = (self._ax.get_xlim(), self._ax.get_ylim())
        self.figura.clear()
        ax = self._ax = self.figura.add_subplot()
        p = r.proyecto
        pf = p.perfil
        zmin = float(np.nanmin(pf.cota))
        ax.fill_between(pf.progresiva, zmin - 5, pf.cota, color="#e3d5b8", lw=0)
        ax.plot(pf.progresiva, pf.cota, color="#5a3e1b", lw=0.9)
        for k, v in enumerate(r.vanos):
            ea, eb = r.estructuras[v.a], r.estructuras[v.a + 1]
            e = r.tramo_de_vano(k).estados[p.hip_flecha_maxima]
            s, zc, _ = linea_conductor(pf, ea.progresiva, eb.progresiva, ea.cota_amarre, eb.cota_amarre,
                                       e.H, e.w_resultante, p.modelo, e.w_vertical / e.w_resultante, 2.0)
            color = "#1e8449" if v.cumple else "#c0392b"
            ax.plot(s, zc, color=color, lw=1.4 if v.cumple else 2.2)
            ax.plot(s, zc - p.distancia_minima_terreno, color="#8e44ad", lw=0.5, ls="--")
            if not v.cumple:
                zt = float(pf.cota_en(v.s_despeje))
                ax.plot([v.s_despeje], [zt], "x", color="#c0392b", ms=9, mew=2)
                ax.annotate(f"{v.despeje:.2f} m", (v.s_despeje, zt), color="#c0392b", fontsize=7,
                            xytext=(4, -10), textcoords="offset points")
        ver = {} if r.verificacion is None else dict(zip(r.verificacion["Estructura"], r.verificacion["Cumple"]))
        for i, e in enumerate(r.estructuras):
            sel = i == self.seleccion
            mal = ver.get(e.codigo) == "NO"
            color = "#e67e22" if sel else "#c0392b" if mal else "#1b2631"
            ax.plot([e.progresiva] * 2, [e.cota_terreno, e.cota_amarre], color=color,
                    lw=3.5 if sel else 2.2 if bloqueadas[i] else 1.6, solid_capstyle="butt")
            ax.plot([e.progresiva], [e.cota_amarre], "s" if bloqueadas[i] else "o", color=color, ms=4)
            ax.annotate(f"{e.codigo}\n{e.armado}", (e.progresiva, e.cota_amarre), fontsize=6.5,
                        ha="center", va="bottom", xytext=(0, 4), textcoords="offset points", color=color)
        self._fantasma = ax.axvline(0, color="#e67e22", lw=1, ls=":", visible=False)
        ax.set_xlim(0, pf.longitud)
        ax.set_xlabel("Progresiva (m)", fontsize=8)
        ax.set_ylabel("Cota (m)", fontsize=8)
        ax.tick_params(labelsize=7)
        ax.grid(True, lw=0.3, color="0.88")
        ax.set_title("Doble clic: agregar · Arrastrar: mover · Clic derecho: opciones   "
                     "(■ fijo: inicio/fin/vértice)", fontsize=8, loc="left", color="0.35")
        if self._vista is not None:
            ax.set_xlim(*self._vista[0])
            ax.set_ylim(*self._vista[1])
        self._vista = self._vista or (ax.get_xlim(), ax.get_ylim())
        self.figura.tight_layout()
        self.redibujar()

    def reiniciar_vista(self) -> None:
        self._vista = None

    # ------------------------------------------------------------------ ratón
    def poste_cercano(self, ev: Any) -> int | None:
        """Índice del poste bajo el cursor (en píxeles), o None."""
        if self.resultado is None or self._ax is None or ev.inaxes is not self._ax:
            return None
        mejor, dmin = None, TOL_PX
        for i, e in enumerate(self.resultado.estructuras):
            x0, y0 = self._ax.transData.transform((e.progresiva, e.cota_terreno))
            _, y1 = self._ax.transData.transform((e.progresiva, e.cota_amarre))
            if min(y0, y1) - TOL_PX <= ev.y <= max(y0, y1) + 3 * TOL_PX and abs(ev.x - x0) < dmin:
                mejor, dmin = i, abs(ev.x - x0)
        return mejor

    def _modo_navegacion(self) -> bool:
        """True si está activo el zoom o desplazamiento de la barra."""
        return bool(str(getattr(self.barra, "mode", "")))

    def al_presionar(self, ev: Any) -> None:
        if ev.inaxes is None or self._modo_navegacion():
            return
        i = self.poste_cercano(ev)
        if ev.button == 1:
            if i is None:
                if ev.dblclick and ev.xdata is not None:
                    self.panel.agregar(float(ev.xdata))
                return
            self.seleccion = i
            self.panel.seleccionar(i)
            if not self.panel.doc.bloqueada(i):
                self._arrastre = i
        elif ev.button == 3 and i is not None:
            self.seleccion = i
            self.panel.seleccionar(i)
            self.panel.menu_estructura(i)

    def al_mover(self, ev: Any) -> None:
        if self._arrastre is None or ev.xdata is None or self._fantasma is None:
            return
        self._fantasma.set_xdata([ev.xdata, ev.xdata])
        self._fantasma.set_visible(True)
        self.panel.v.estado(f"Mover a {formato_progresiva(max(ev.xdata, 0))}")
        self.redibujar()

    def al_soltar(self, ev: Any) -> None:
        i, self._arrastre = self._arrastre, None
        if i is None:
            return
        if self._fantasma is not None:
            self._fantasma.set_visible(False)
        if ev.xdata is not None:
            s0 = self.resultado.estructuras[i].progresiva if self.resultado else None
            if s0 is None or abs(float(ev.xdata) - s0) > 0.05:
                self.panel.mover(i, float(ev.xdata))
                return
        self.redibujar()
