"""Panel genérico para módulos con proyecto JSON (GridCraft, PresuCraft): calcular y ver tablas."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any

import pandas as pd
from PySide6.QtWidgets import (
    QFileDialog, QFormLayout, QGroupBox, QHBoxLayout, QLabel, QPushButton, QTabWidget, QVBoxLayout, QWidget,
)

from suite_electrica.gui.widgets import SelectorArchivo, vista_tabla

if TYPE_CHECKING:
    from suite_electrica.gui.app import VentanaPrincipal


@dataclass
class Modulo:
    titulo: str
    calcular: Callable[[Path], Any]
    """Ruta JSON -> resultado."""
    tablas: Callable[[Any], dict[str, pd.DataFrame]]
    estado: Callable[[Any], str]
    exportar: Callable[[Any, Path], list[Path]]
    """Escribe salidas en la carpeta y devuelve las rutas."""
    nuevo: Callable[[Path], Any]
    """Copia plantillas a una carpeta."""
    archivo_json: str
    con_dxf: bool = True


def _tablas_dict(d: dict[str, float]) -> pd.DataFrame:
    return pd.DataFrame({"Parámetro": list(d), "Valor": list(d.values())})


def modulo_grid() -> Modulo:
    from suite_electrica.gridcraft import cli
    from suite_electrica.gridcraft.red import calcular, cargar

    def exportar(r: Any, out: Path) -> list[Path]:
        rutas = [cli.planilla_excel(out / "caida_tension.xlsx", r)]
        if (d := cli.plano_dxf(out / "plano_red.dxf", r)) is not None:
            rutas.append(d)
        return rutas

    return Modulo(
        "GridCraft · Redes", lambda p: calcular(cargar(p)),
        lambda r: {"Resumen": _tablas_dict(r.resumen), "CaidaTension": r.tramos, "Alertas": r.alertas,
                   "Avisos": pd.DataFrame({"Aviso": r.avisos})},
        lambda r: (f"{'CUMPLE' if r.cumple else 'NO CUMPLE'} — %ΔV máx. {r.resumen['%ΔV máx.']:.2f} %, "
                   f"M.D. {r.resumen['M.D. total (kW)']:.2f} kW, {len(r.alertas)} alerta(s)"),
        exportar, lambda c: cli.main(["nuevo", str(c)]), "red.json")


def modulo_presu() -> Modulo:
    from suite_electrica.presucraft import cli
    from suite_electrica.presucraft.presupuesto import calcular, cargar, excel

    def estado(r: Any) -> str:
        t = r.resumen.set_index("Rubro").loc["TOTAL"]
        return f"Total {r.presupuesto.moneda} {t['Total']:,.2f} (CD {t['Costo directo']:,.2f})"

    return Modulo(
        "PresuCraft · Presupuesto", lambda p: calcular(cargar(p)),
        lambda r: {"Resumen": r.resumen, "Suministro": r.suministro, "Montaje": r.montaje,
                   "PlanillaUC": r.planilla_uc, "Avisos": pd.DataFrame({"Aviso": r.avisos})},
        estado, lambda r, out: [excel(out / "presupuesto.xlsx", r)],
        lambda c: cli.main(["nuevo", str(c)]), "presupuesto.json", con_dxf=False)


class PanelJSON(QWidget):
    """Elegir proyecto JSON, calcular en segundo plano y mostrar las tablas del resultado."""

    def __init__(self, ventana: VentanaPrincipal, modulo: Modulo, extra: list[tuple[str, Callable]] = ()) -> None:
        super().__init__()
        self.v, self.m = ventana, modulo
        self.resultado: Any = None
        self.exportados: list[Path] = []
        self.selector = SelectorArchivo("Proyecto (*.json)")
        self.lbl = QLabel("Sin calcular")
        self.lbl.setWordWrap(True)
        self.lbl.setStyleSheet("font-size: 15px; font-weight: bold;")
        g = QGroupBox("Proyecto")
        f = QFormLayout(g)
        f.addRow("Archivo", self.selector)
        fila = QHBoxLayout()
        botones = [("Nuevo desde plantilla…", self.nuevo), ("▶ Calcular", self.calcular),
                   ("Exportar a carpeta…", self.exportar_dialogo)]
        if modulo.con_dxf:
            botones.append(("Abrir DXF", self.abrir_dxf))
        for texto, fn in [*botones, *extra]:
            b = QPushButton(texto)
            b.clicked.connect(lambda _=False, fn=fn: fn())
            fila.addWidget(b)
        f.addRow(fila)
        self.tabs = QTabWidget()
        lay = QVBoxLayout(self)
        lay.addWidget(g)
        lay.addWidget(self.lbl)
        lay.addWidget(self.tabs, 1)

    def ruta(self) -> Path:
        t = self.selector.texto()
        if not t:
            raise ValueError("Elija el archivo .json del proyecto")
        return Path(t)

    def nuevo(self, carpeta: str | Path | None = None) -> None:
        carpeta = carpeta or QFileDialog.getExistingDirectory(self, "Carpeta del nuevo proyecto")
        if carpeta:
            self.m.nuevo(Path(carpeta))
            self.selector.campo.setText(str(Path(carpeta) / self.m.archivo_json))
            self.v.estado(f"Plantillas copiadas en {carpeta}; edite los CSV y calcule")

    def calcular(self) -> None:
        try:
            ruta = self.ruta()
        except ValueError as exc:
            self.v.error(str(exc))
            return
        self.v.ejecutar(f"{self.m.titulo}: calculando…", self.m.calcular, (ruta,), self.mostrar)

    def mostrar(self, r: Any) -> None:
        self.resultado = r
        self.lbl.setText(self.m.estado(r))
        self.tabs.clear()
        for nombre, df in self.m.tablas(r).items():
            vista, modelo = vista_tabla()
            modelo.poner(df)
            self.tabs.addTab(vista, nombre)
        self.v.estado(f"{self.m.titulo}: {self.m.estado(r)}")

    def exportar(self, carpeta: str | Path) -> list[Path]:
        if self.resultado is None:
            raise ValueError("Calcule primero")
        self.exportados = self.m.exportar(self.resultado, Path(carpeta))
        self.v.estado("Exportado: " + ", ".join(p.name for p in self.exportados))
        return self.exportados

    def exportar_dialogo(self) -> None:
        c = QFileDialog.getExistingDirectory(self, "Carpeta de salida")
        if c:
            try:
                self.exportar(c)
            except (ValueError, OSError) as exc:
                self.v.error(str(exc))

    def abrir_dxf(self) -> None:
        dxf = [p for p in self.exportados if p.suffix.lower() == ".dxf"]
        if not dxf:
            self.v.error("Exporte primero (este módulo genera DXF solo si hay coordenadas)")
            return
        self.v.abrir_dxf(dxf[0])
