"""Paneles de TopoCraft y LineCraft."""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any

from PySide6.QtGui import QFont
from PySide6.QtWidgets import (
    QCheckBox, QDoubleSpinBox, QFileDialog, QFormLayout, QGroupBox, QHBoxLayout, QLabel,
    QMessageBox, QPlainTextEdit, QPushButton, QSpinBox, QSplitter, QTabWidget, QVBoxLayout, QWidget,
)

from suite_electrica.gui import servicios as sv
from suite_electrica.gui.widgets import Lienzo, SelectorArchivo, vista_tabla
from suite_electrica.linecraft import salidas as lc_salidas
from suite_electrica.linecraft.cli import crear_proyecto

if TYPE_CHECKING:
    from suite_electrica.gui.app import VentanaPrincipal


def _num(minimo: float, maximo: float, valor: float, decimales: int | None = 2,
         sufijo: str = "", prefijo: str = "", especial: str = "") -> QDoubleSpinBox | QSpinBox:
    """Campo numérico (QSpinBox si ``decimales`` es None)."""
    w = QSpinBox() if decimales is None else QDoubleSpinBox()
    if decimales is not None:
        w.setDecimals(decimales)
    w.setRange(minimo, maximo)
    w.setValue(valor)
    w.setSuffix(sufijo)
    w.setPrefix(prefijo)
    if especial:
        w.setSpecialValueText(especial)
    return w


def _boton(texto: str, accion: Callable[[], Any], principal: bool = False) -> QPushButton:
    b = QPushButton(texto)
    b.clicked.connect(accion)
    if principal:
        b.setStyleSheet("font-weight: bold; padding: 6px;")
    return b


class PanelTopo(QWidget):
    """TopoCraft: puntos → TIN, curvas, perfil."""

    def __init__(self, ventana: VentanaPrincipal) -> None:
        super().__init__()
        self.v = ventana
        self.resultado: sv.ResultadoTopo | None = None
        self.exportados: list[Path] = []

        self.puntos = SelectorArchivo("Puntos (*.csv *.txt *.dxf)")
        self.eje = SelectorArchivo("Eje (*.dxf *.csv)")
        self.eq = _num(0.1, 100, 1.0, sufijo=" m")
        self.mayor = _num(1, 20, 5, None)
        self.paso = _num(1, 1000, 20, 1, sufijo=" m")
        self.max_lado = _num(0, 10000, 0, 1, sufijo=" m", especial="sin límite")
        self.atipicos = QCheckBox("Quitar cotas atípicas")
        self.esc_h = _num(0, 100000, 0, None, prefijo="1:", especial="auto")
        self.esc_v = _num(0, 100000, 0, None, prefijo="1:", especial="auto")

        datos = QGroupBox("Datos")
        f1 = QFormLayout(datos)
        f1.addRow("Puntos", self.puntos)
        f1.addRow("Eje (opcional)", self.eje)
        params = QGroupBox("Parámetros")
        f2 = QFormLayout(params)
        f2.addRow("Equidistancia", self.eq)
        f2.addRow("Curva mayor cada", self.mayor)
        f2.addRow("Paso del perfil", self.paso)
        f2.addRow("Lado máx. triángulo", self.max_lado)
        f2.addRow(self.atipicos)
        f2.addRow("Escala H perfil", self.esc_h)
        f2.addRow("Escala V perfil", self.esc_v)

        izq = QWidget()
        li = QVBoxLayout(izq)
        li.addWidget(datos)
        li.addWidget(params)
        li.addWidget(_boton("▶ Procesar", self.procesar, True))
        li.addWidget(_boton("Exportar DXF / PDF / Excel…", self.exportar_dialogo))
        li.addWidget(_boton("Abrir plano en visor DXF", self.abrir_plano))
        li.addStretch()

        self.tabs = QTabWidget()
        self.plano = Lienzo()
        self.perfil = Lienzo()
        self.tabla, self.modelo = vista_tabla()
        self.registro = QPlainTextEdit()
        self.registro.setReadOnly(True)
        self.registro.setFont(QFont("monospace"))
        self.tabs.addTab(self.plano, "Plano")
        self.tabs.addTab(self.perfil, "Perfil")
        self.tabs.addTab(self.tabla, "Tabla perfil")
        self.tabs.addTab(self.registro, "Registro")

        sp = QSplitter()
        sp.addWidget(izq)
        sp.addWidget(self.tabs)
        sp.setStretchFactor(1, 1)
        QHBoxLayout(self).addWidget(sp)

    def param(self) -> sv.ParamTopo:
        return sv.ParamTopo(
            puntos=self.puntos.texto(), eje=self.eje.texto() or None, equidistancia=self.eq.value(),
            mayor_cada=self.mayor.value(), paso=self.paso.value(),
            max_lado=self.max_lado.value() or None, quitar_atipicos=self.atipicos.isChecked(),
            esc_h=self.esc_h.value() or None, esc_v=self.esc_v.value() or None)

    def log(self, texto: str) -> None:
        self.registro.appendPlainText(texto)

    def procesar(self) -> None:
        p = self.param()
        if not p.puntos:
            QMessageBox.warning(self, "TopoCraft", "Elija el archivo de puntos.")
            return
        self.v.ejecutar("Procesando topografía…", sv.procesar_topo, (p,), self._mostrar)

    def _mostrar(self, r: sv.ResultadoTopo) -> None:
        self.resultado = r
        self.log(r.reporte.resumen())
        self.log(f"TIN: {len(r.tin.triangulos_activos)} triángulos; {len(r.curvas)} curvas de nivel")
        sv.dibujar_plano(self.plano.figura, r)
        self.plano.redibujar()
        if r.perfil is not None:
            sv.dibujar_perfil_topo(self.perfil.figura, r.perfil)
            self.perfil.redibujar()
            self.modelo.poner(sv.tabla_perfil(r.perfil))
            self.log(f"Perfil: {r.perfil.longitud:.2f} m, {len(r.perfil.progresiva)} puntos")
        self.tabs.setCurrentWidget(self.plano)
        self.v.estado("Topografía procesada")

    def exportar_dialogo(self) -> None:
        if self.resultado is None:
            QMessageBox.information(self, "TopoCraft", "Primero procese los datos.")
            return
        carpeta = QFileDialog.getExistingDirectory(self, "Carpeta de salida",
                                                   str(Path(self.resultado.param.puntos).parent))
        if carpeta:
            self.exportar(carpeta)

    def exportar(self, carpeta: str | Path) -> None:
        assert self.resultado is not None
        self.v.ejecutar("Exportando…", sv.exportar_topo, (self.resultado, carpeta), self._exportado)

    def _exportado(self, rutas: list[Path]) -> None:
        self.exportados = rutas
        for r in rutas:
            self.log(f"Exportado: {r}")
        self.v.estado(f"{len(rutas)} archivo(s) exportados")

    def abrir_plano(self) -> None:
        dxf = [r for r in self.exportados if r.name == "plano.dxf"]
        if not dxf:
            QMessageBox.information(self, "TopoCraft", "Primero exporte el plano.")
            return
        self.v.abrir_dxf(dxf[0])


class PanelLinea(QWidget):
    """LineCraft: proyecto JSON → cálculo, tablas y salidas."""

    TABLAS = ("Estructuras", "Vanos", "Mecanico", "Cargas", "Avisos")

    def __init__(self, ventana: VentanaPrincipal) -> None:
        super().__init__()
        self.v = ventana
        self.ruta: Path | None = None
        self.resultado = None
        self.exportados: list[Path] = []

        self.lbl_ruta = QLabel("Sin proyecto")
        self.lbl_ruta.setWordWrap(True)
        self.lbl_estado = QLabel("")
        self.lbl_estado.setStyleSheet("font-size: 16px; font-weight: bold;")
        izq = QWidget()
        li = QVBoxLayout(izq)
        g = QGroupBox("Proyecto")
        lg = QVBoxLayout(g)
        lg.addWidget(self.lbl_ruta)
        lg.addWidget(_boton("Nuevo proyecto…", self.nuevo_dialogo))
        lg.addWidget(_boton("Abrir proyecto…", self.abrir_dialogo))
        lg.addWidget(_boton("Guardar cambios", self.guardar))
        li.addWidget(g)
        li.addWidget(_boton("▶ Calcular", self.calcular, True))
        li.addWidget(self.lbl_estado)
        li.addWidget(_boton("Exportar DXF / Excel / PDF…", self.exportar_dialogo))
        li.addWidget(_boton("Abrir perfil en visor DXF", self.abrir_perfil))
        li.addWidget(_boton("Abrir memoria PDF", self.abrir_memoria))
        li.addStretch()

        self.tabs = QTabWidget()
        self.editor = QPlainTextEdit()
        self.editor.setFont(QFont("monospace"))
        self.lienzo = Lienzo()
        self.tabs.addTab(self.editor, "Proyecto (JSON)")
        self.tabs.addTab(self.lienzo, "Perfil")
        self.modelos = {}
        for t in self.TABLAS:
            vista, self.modelos[t] = vista_tabla()
            self.tabs.addTab(vista, "Mecánico" if t == "Mecanico" else t)

        sp = QSplitter()
        sp.addWidget(izq)
        sp.addWidget(self.tabs)
        sp.setStretchFactor(1, 1)
        QHBoxLayout(self).addWidget(sp)

    # -- archivo
    def nuevo_dialogo(self) -> None:
        carpeta = QFileDialog.getExistingDirectory(self, "Carpeta del nuevo proyecto")
        if carpeta:
            self.nuevo(carpeta)

    def nuevo(self, carpeta: str | Path) -> None:
        creados, _ = crear_proyecto(carpeta)
        self.abrir(Path(carpeta) / "proyecto.json")
        QMessageBox.information(
            self, "Nuevo proyecto",
            f"Plantillas creadas en {carpeta}:\n" + "\n".join(p.name for p in creados)
            + "\n\nCopie puntos.csv y eje.dxf a esa carpeta, ajuste el JSON (valores y fuentes) "
              "y pulse Calcular.")

    def abrir_dialogo(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Abrir proyecto", "", "Proyecto LineCraft (*.json)")
        if r:
            self.abrir(r)

    def abrir(self, ruta: str | Path) -> None:
        self.ruta = Path(ruta)
        self.editor.setPlainText(self.ruta.read_text(encoding="utf-8"))
        self.editor.document().setModified(False)
        self.lbl_ruta.setText(f"{self.ruta.parent.name}/{self.ruta.name}")
        self.lbl_ruta.setToolTip(str(self.ruta))
        self.v.recordar_proyecto(self.ruta)
        self.tabs.setCurrentWidget(self.editor)
        self.v.estado(f"Proyecto abierto: {self.ruta.name}")

    def guardar(self) -> bool:
        if self.ruta is None:
            return False
        try:
            json.loads(self.editor.toPlainText())
        except json.JSONDecodeError as e:
            QMessageBox.critical(self, "JSON inválido", f"Línea {e.lineno}, columna {e.colno}: {e.msg}")
            return False
        self.ruta.write_text(self.editor.toPlainText(), encoding="utf-8")
        self.editor.document().setModified(False)
        self.v.estado("Proyecto guardado")
        return True

    # -- cálculo
    def calcular(self) -> None:
        if self.ruta is None:
            QMessageBox.information(self, "LineCraft", "Abra o cree un proyecto.")
            return
        cambiado = self.editor.toPlainText() != self.ruta.read_text(encoding="utf-8")
        if cambiado and not self.guardar():
            return
        self.v.ejecutar("Calculando línea…", sv.calcular_linea, (self.ruta,), self._mostrar)

    def _mostrar(self, r: Any) -> None:
        self.resultado = r
        for nombre, df in lc_salidas.tablas(r).items():
            self.modelos[nombre].poner(df)
        lc_salidas.figura_perfil(r, fig=self.lienzo.figura)
        self.lienzo.redibujar()
        ok = r.cumple
        self.lbl_estado.setText(("✔ CUMPLE" if ok else "✘ NO CUMPLE") + f"\n{len(r.avisos)} aviso(s)")
        self.lbl_estado.setStyleSheet(
            f"font-size: 16px; font-weight: bold; color: {'#1e8449' if ok else '#c0392b'};")
        self.tabs.setCurrentWidget(self.lienzo)
        self.v.estado(f"{len(r.estructuras)} estructuras, {len(r.vanos)} vanos — "
                      + ("CUMPLE" if ok else "NO CUMPLE"))

    def exportar_dialogo(self) -> None:
        if self.resultado is None or self.ruta is None:
            QMessageBox.information(self, "LineCraft", "Primero calcule.")
            return
        carpeta = QFileDialog.getExistingDirectory(self, "Carpeta de salida", str(self.ruta.parent / "salidas"))
        if carpeta:
            self.exportar(carpeta)

    def exportar(self, carpeta: str | Path) -> None:
        self.v.ejecutar("Exportando…", sv.exportar_linea, (self.resultado, carpeta), self._exportado)

    def _exportado(self, rutas: list[Path]) -> None:
        self.exportados = rutas
        self.v.estado("Exportado: " + ", ".join(r.name for r in rutas))

    def _buscar(self, nombre: str) -> Path | None:
        r = [p for p in self.exportados if p.name == nombre]
        if not r:
            QMessageBox.information(self, "LineCraft", "Primero exporte las salidas.")
            return None
        return r[0]

    def abrir_perfil(self) -> None:
        if (p := self._buscar("perfil_linea.dxf")) is not None:
            self.v.abrir_dxf(p)

    def abrir_memoria(self) -> None:
        if (p := self._buscar("memoria_calculo.pdf")) is not None:
            self.v.abrir(p)
