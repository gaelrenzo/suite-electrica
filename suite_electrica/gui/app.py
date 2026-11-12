"""Ventana principal de la Suite Eléctrica (lanzador común TopoCraft / LineCraft)."""

from __future__ import annotations

import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

from PySide6.QtCore import QSettings, QThreadPool, QUrl
from PySide6.QtGui import QAction, QDesktopServices, QKeySequence
from PySide6.QtWidgets import (
    QApplication, QFileDialog, QMainWindow, QMessageBox, QProgressBar, QTabWidget,
)

from suite_electrica import __version__
from suite_electrica.gui import servicios as sv
from suite_electrica.gui.paneles import PanelLinea, PanelTopo
from suite_electrica.gui.widgets import Tarea

REPO = "https://github.com/gaelrenzo/suite-electrica"


class VentanaPrincipal(QMainWindow):
    """Ventana con pestañas por módulo, menús y ejecución en segundo plano."""

    def __init__(self, sincrono: bool = False) -> None:
        super().__init__()
        self.sincrono = sincrono
        self.ajustes = QSettings("SuiteElectrica", "SuiteElectrica")
        self._tareas: set[Tarea] = set()
        self.setWindowTitle(f"Suite Eléctrica {__version__}")
        self.resize(1280, 800)

        self.topo = PanelTopo(self)
        self.linea = PanelLinea(self)
        self.modulos = QTabWidget()
        self.modulos.addTab(self.topo, "TopoCraft · Topografía")
        self.modulos.addTab(self.linea, "LineCraft · Línea MT")
        self.modulos.addTab(self._proximamente(), "GridCraft · Redes")
        self.modulos.setTabEnabled(2, False)
        self.setCentralWidget(self.modulos)

        self.progreso = QProgressBar()
        self.progreso.setRange(0, 0)
        self.progreso.setVisible(False)
        self.progreso.setMaximumWidth(160)
        self.statusBar().addPermanentWidget(self.progreso)
        self._menus()
        self.estado("Listo")

    @staticmethod
    def _proximamente() -> Any:
        from PySide6.QtWidgets import QLabel
        return QLabel("GridCraft (fase 3) aún no está disponible.")

    def _menus(self) -> None:
        def accion(menu: Any, texto: str, fn: Callable[[], Any], atajo: str | None = None) -> QAction:
            a = QAction(texto, self)
            a.triggered.connect(fn)
            if atajo:
                a.setShortcut(QKeySequence(atajo))
            menu.addAction(a)
            return a

        m = self.menuBar().addMenu("&Archivo")
        accion(m, "Nuevo proyecto LineCraft…", self._nuevo_linea, "Ctrl+N")
        accion(m, "Abrir proyecto LineCraft…", self._abrir_linea, "Ctrl+O")
        accion(m, "Guardar proyecto", self.linea.guardar, "Ctrl+S")
        self.menu_recientes = m.addMenu("Recientes")
        self._actualizar_recientes()
        m.addSeparator()
        accion(m, "Salir", self.close, "Ctrl+Q")

        m = self.menuBar().addMenu("&Calcular")
        accion(m, "Procesar topografía", self._procesar_topo, "F5")
        accion(m, "Calcular línea", self._calcular_linea, "F6")

        m = self.menuBar().addMenu("&Herramientas")
        accion(m, "Configurar visor DXF (CADCraft)…", self.configurar_cad)
        accion(m, "Usar visor DXF predeterminado del sistema", self._cad_sistema)

        m = self.menuBar().addMenu("A&yuda")
        accion(m, "Guía rápida", self._guia, "F1")
        accion(m, "Acerca de", self._acerca)

    # -- utilidades usadas por los paneles
    def estado(self, texto: str) -> None:
        self.statusBar().showMessage(texto)

    def ejecutar(self, mensaje: str, fn: Callable[..., Any], args: tuple, al_terminar: Callable[[Any], None]) -> None:
        """Ejecuta ``fn`` en segundo plano (o en línea si ``sincrono``) y llama ``al_terminar``."""
        self.estado(mensaje)
        if self.sincrono:
            try:
                r = fn(*args)
            except Exception as exc:  # noqa: BLE001
                self._error(f"{type(exc).__name__}: {exc}")
            else:
                al_terminar(r)
            return
        t = Tarea(fn, *args)
        self._tareas.add(t)
        self.progreso.setVisible(True)

        def fin() -> None:
            self._tareas.discard(t)
            self.progreso.setVisible(bool(self._tareas))

        t.senales.listo.connect(lambda r: (fin(), al_terminar(r)))
        t.senales.error.connect(lambda e: (fin(), self._error(e)))
        QThreadPool.globalInstance().start(t)

    def _error(self, texto: str) -> None:
        self.ultimo_error = texto
        self.estado("Error")
        QMessageBox.critical(self, "Error", texto)

    def abrir(self, ruta: str | Path) -> None:
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(Path(ruta).resolve())))

    def abrir_dxf(self, ruta: str | Path) -> None:
        """Abre el DXF en CADCraft (si está configurado) o con la app del sistema."""
        programa = self.ajustes.value("visor_dxf", "", str) or None
        try:
            sv.abrir_externo(ruta, programa)
        except OSError as exc:
            self._error(f"No se pudo abrir {ruta}: {exc}")

    def configurar_cad(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Ejecutable de CADCraft u otro visor DXF")
        if r:
            self.ajustes.setValue("visor_dxf", r)
            self.estado(f"Visor DXF: {r}")

    def _cad_sistema(self) -> None:
        self.ajustes.remove("visor_dxf")
        self.estado("Visor DXF: predeterminado del sistema")

    def recordar_proyecto(self, ruta: Path) -> None:
        lista = [str(ruta)] + [r for r in self._recientes() if r != str(ruta)]
        self.ajustes.setValue("recientes", lista[:8])
        self._actualizar_recientes()

    def _recientes(self) -> list[str]:
        v = self.ajustes.value("recientes", [])
        return [v] if isinstance(v, str) else list(v or [])

    def _actualizar_recientes(self) -> None:
        self.menu_recientes.clear()
        for r in self._recientes():
            if Path(r).exists():
                a = self.menu_recientes.addAction(r)
                a.triggered.connect(lambda _=False, r=r: self._abrir_linea(r))

    # -- acciones de menú
    def _nuevo_linea(self) -> None:
        self.modulos.setCurrentWidget(self.linea)
        self.linea.nuevo_dialogo()

    def _abrir_linea(self, ruta: str | None = None) -> None:
        self.modulos.setCurrentWidget(self.linea)
        if ruta:
            self.linea.abrir(ruta)
        else:
            self.linea.abrir_dialogo()

    def _procesar_topo(self) -> None:
        self.modulos.setCurrentWidget(self.topo)
        self.topo.procesar()

    def _calcular_linea(self) -> None:
        self.modulos.setCurrentWidget(self.linea)
        self.linea.calcular()

    def _guia(self) -> None:
        local = Path(__file__).resolve().parents[2] / "docs" / "GUIA_RAPIDA.md"
        if local.exists():
            self.abrir(local)
        else:
            QDesktopServices.openUrl(QUrl(f"{REPO}/blob/main/docs/GUIA_RAPIDA.md"))

    def _acerca(self) -> None:
        QMessageBox.about(
            self, "Acerca de",
            f"<b>Suite Eléctrica {__version__}</b><br>TopoCraft · LineCraft · GridCraft<br>"
            "Ingeniería eléctrica de distribución en Perú.<br><br>"
            f"Licencia MIT © Renzo Mamani<br><a href='{REPO}'>{REPO}</a><br><br>"
            "Los valores normativos son entradas del proyecto: verifique cada fuente.")


def main(argv: list[str] | None = None) -> int:
    """Inicia la app. Argumento opcional: proyecto LineCraft (.json) a abrir."""
    argv = sys.argv if argv is None else argv
    app = QApplication(argv)
    app.setApplicationName("Suite Eléctrica")
    app.setStyle("Fusion")
    v = VentanaPrincipal()
    if len(argv) > 1 and argv[1].lower().endswith(".json"):
        v._abrir_linea(argv[1])
    v.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
