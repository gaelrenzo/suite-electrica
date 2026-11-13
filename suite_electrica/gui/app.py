"""Ventana principal de la Suite Eléctrica: un documento ``.sel`` y pestañas por módulo."""

from __future__ import annotations

import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

from PySide6.QtCore import QSettings, QThreadPool, QUrl
from PySide6.QtGui import QAction, QCloseEvent, QDesktopServices, QKeySequence
from PySide6.QtWidgets import (
    QApplication, QFileDialog, QLabel, QMainWindow, QMessageBox, QProgressBar, QTabWidget,
)

from suite_electrica import __version__
from suite_electrica.documento import EXTENSION, Documento
from suite_electrica.gui import servicios as sv
from suite_electrica.gui.paneles import PanelLinea, PanelTopo
from suite_electrica.gui.widgets import Tarea

REPO = "https://github.com/gaelrenzo/suite-electrica"
FILTRO = f"Proyecto Suite Eléctrica (*{EXTENSION})"


class VentanaPrincipal(QMainWindow):
    """Ventana con menús, documento único y ejecución en segundo plano."""

    def __init__(self, sincrono: bool = False) -> None:
        super().__init__()
        self.sincrono = sincrono
        self.ajustes = QSettings("SuiteElectrica", "SuiteElectrica")
        self._tareas: set[Tarea] = set()
        self.ultimo_error = ""
        self.doc = Documento()
        self.resize(1320, 840)

        self.topo = PanelTopo(self)
        self.linea = PanelLinea(self)
        self.modulos = QTabWidget()
        self.modulos.addTab(self.topo, "TopoCraft · Topografía")
        self.modulos.addTab(self.linea, "LineCraft · Línea MT")
        self.modulos.addTab(QLabel("GridCraft (fase 3) aún no está disponible."), "GridCraft · Redes")
        self.modulos.setTabEnabled(2, False)
        self.setCentralWidget(self.modulos)

        self.progreso = QProgressBar()
        self.progreso.setRange(0, 0)
        self.progreso.setVisible(False)
        self.progreso.setMaximumWidth(160)
        self.statusBar().addPermanentWidget(self.progreso)
        self._menus()
        self.poner_documento(self.doc)
        self.estado("Listo — importe puntos y eje en TopoCraft o abra un proyecto (Ctrl+O)")

    # ------------------------------------------------------------------ menús
    def _menus(self) -> None:
        def accion(menu: Any, texto: str, fn: Callable[[], Any], atajo: str | None = None) -> QAction:
            a = QAction(texto, self)
            a.triggered.connect(lambda _=False: fn())
            if atajo:
                a.setShortcut(QKeySequence(atajo))
            menu.addAction(a)
            return a

        m = self.menuBar().addMenu("&Archivo")
        accion(m, "Nuevo proyecto", self.nuevo, "Ctrl+N")
        accion(m, "Abrir…", self.abrir_dialogo, "Ctrl+O")
        accion(m, "Guardar", self.guardar, "Ctrl+S")
        accion(m, "Guardar como…", self.guardar_como, "Ctrl+Shift+S")
        self.menu_recientes = m.addMenu("Recientes")
        m.addSeparator()
        accion(m, "Importar puntos topográficos…", self._importar_puntos)
        accion(m, "Importar eje de línea…", self._importar_eje)
        accion(m, "Importar proyecto LineCraft (.json)…", self._importar_json)
        m.addSeparator()
        accion(m, "Salir", self.close, "Ctrl+Q")

        m = self.menuBar().addMenu("&Editar")
        accion(m, "Deshacer cambio de estructuras", self.linea.deshacer, "Ctrl+Z")

        m = self.menuBar().addMenu("&Calcular")
        accion(m, "Procesar topografía", self._procesar_topo, "F5")
        accion(m, "Calcular línea", self._calcular_linea, "F6")
        accion(m, "Ubicación automática de estructuras", self.linea.ubicar_automatico)
        accion(m, "Asignar armados y postes", self.linea.asignar_catalogos)

        m = self.menuBar().addMenu("&Herramientas")
        accion(m, "Configurar visor DXF (CADCraft)…", self.configurar_cad)
        accion(m, "Usar visor DXF predeterminado del sistema", self._cad_sistema)

        m = self.menuBar().addMenu("A&yuda")
        accion(m, "Guía rápida", self._guia, "F1")
        accion(m, "Acerca de", self._acerca)
        self._actualizar_recientes()

    # ------------------------------------------------------------------ documento
    def poner_documento(self, doc: Documento) -> None:
        self.doc = doc
        self.topo.cargar_documento()
        self.linea.cargar_documento()
        self.cambiado()

    def cambiado(self) -> None:
        """Actualiza el título (nombre, archivo y * si hay cambios sin guardar)."""
        archivo = self.doc.ruta.name if self.doc.ruta else "sin guardar"
        marca = " *" if self.doc.modificado else ""
        self.setWindowTitle(f"{self.doc.nombre} — {archivo}{marca} · Suite Eléctrica {__version__}")

    def _confirmar_descartar(self) -> bool:
        if not self.doc.modificado or self.sincrono:
            return True
        r = QMessageBox.question(
            self, "Cambios sin guardar", "¿Guardar los cambios del proyecto actual?",
            QMessageBox.StandardButton.Save | QMessageBox.StandardButton.Discard
            | QMessageBox.StandardButton.Cancel)
        if r == QMessageBox.StandardButton.Save:
            return self.guardar()
        return r == QMessageBox.StandardButton.Discard

    def nuevo(self) -> None:
        if self._confirmar_descartar():
            self.poner_documento(Documento())
            self.modulos.setCurrentWidget(self.topo)

    def abrir_dialogo(self) -> None:
        if not self._confirmar_descartar():
            return
        r, _ = QFileDialog.getOpenFileName(self, "Abrir proyecto", "", FILTRO)
        if r:
            self.abrir_proyecto(r)

    def abrir_proyecto(self, ruta: str | Path) -> None:
        try:
            doc = Documento.abrir(ruta)
        except (ValueError, OSError, KeyError) as e:
            self.error(f"No se pudo abrir {ruta}: {e}")
            return
        self.poner_documento(doc)
        self._recordar(doc.ruta)
        if len(doc.estructuras):
            self.modulos.setCurrentWidget(self.linea)
            self.linea.recalcular()
        self.estado(f"Proyecto abierto: {doc.ruta}")

    def guardar(self, ruta: str | Path | None = None) -> bool:
        if ruta is None and self.doc.ruta is None:
            return self.guardar_como()
        try:
            r = self.doc.guardar(ruta)
        except (ValueError, OSError) as e:
            self.error(f"No se pudo guardar: {e}")
            return False
        self._recordar(r)
        self.cambiado()
        self.estado(f"Guardado: {r}")
        return True

    def guardar_como(self) -> bool:
        sugerido = str(self.doc.ruta or Path.home() / f"{self.doc.nombre}{EXTENSION}")
        r, _ = QFileDialog.getSaveFileName(self, "Guardar proyecto", sugerido, FILTRO)
        return self.guardar(r) if r else False

    def _importar_puntos(self) -> None:
        self.modulos.setCurrentWidget(self.topo)
        self.topo.importar_puntos_dialogo()

    def _importar_eje(self) -> None:
        self.modulos.setCurrentWidget(self.topo)
        self.topo.importar_eje_dialogo()

    def _importar_json(self) -> None:
        if not self._confirmar_descartar():
            return
        r, _ = QFileDialog.getOpenFileName(self, "Proyecto LineCraft", "", "Proyecto LineCraft (*.json)")
        if r:
            self.importar_json(r)

    def importar_json(self, ruta: str | Path) -> None:
        try:
            doc = Documento.desde_linecraft_json(ruta)
        except (ValueError, OSError, KeyError) as e:
            self.error(f"No se pudo importar: {e}")
            return
        self.poner_documento(doc)
        self.modulos.setCurrentWidget(self.linea)
        self.linea.recalcular()

    def closeEvent(self, ev: QCloseEvent) -> None:  # noqa: N802
        if self._confirmar_descartar():
            ev.accept()
        else:
            ev.ignore()

    # ------------------------------------------------------------------ utilidades para paneles
    def estado(self, texto: str) -> None:
        self.statusBar().showMessage(texto)

    def ejecutar(self, mensaje: str, fn: Callable[..., Any], args: tuple, al_terminar: Callable[[Any], None]) -> None:
        """Ejecuta ``fn`` en segundo plano (o en línea si ``sincrono``) y llama ``al_terminar``."""
        self.estado(mensaje)
        if self.sincrono:
            try:
                r = fn(*args)
            except Exception as exc:  # noqa: BLE001
                self.error(f"{type(exc).__name__}: {exc}")
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
        t.senales.error.connect(lambda e: (fin(), self.error(e)))
        QThreadPool.globalInstance().start(t)

    def error(self, texto: str) -> None:
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
            self.error(f"No se pudo abrir {ruta}: {exc}")

    def configurar_cad(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Ejecutable de CADCraft u otro visor DXF")
        if r:
            self.ajustes.setValue("visor_dxf", r)
            self.estado(f"Visor DXF: {r}")

    def _cad_sistema(self) -> None:
        self.ajustes.remove("visor_dxf")
        self.estado("Visor DXF: predeterminado del sistema")

    def _recordar(self, ruta: Path | None) -> None:
        if ruta is None:
            return
        lista = [str(ruta)] + [r for r in self.recientes() if r != str(ruta)]
        self.ajustes.setValue("recientes_sel", lista[:8])
        self._actualizar_recientes()

    def recientes(self) -> list[str]:
        v = self.ajustes.value("recientes_sel", [])
        return [v] if isinstance(v, str) else list(v or [])

    def _actualizar_recientes(self) -> None:
        self.menu_recientes.clear()
        for r in self.recientes():
            if Path(r).exists():
                a = self.menu_recientes.addAction(r)
                a.triggered.connect(lambda _=False, r=r: self._confirmar_descartar() and self.abrir_proyecto(r))

    def _procesar_topo(self) -> None:
        self.modulos.setCurrentWidget(self.topo)
        self.topo.procesar()

    def _calcular_linea(self) -> None:
        self.modulos.setCurrentWidget(self.linea)
        self.linea.recalcular()

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
            "Los valores normativos y de catálogo son datos del proyecto: verifique cada fuente.")


def main(argv: list[str] | None = None) -> int:
    """Inicia la app. Argumento opcional: proyecto ``.sel`` o ``.json`` de LineCraft."""
    argv = sys.argv if argv is None else argv
    app = QApplication(argv)
    app.setApplicationName("Suite Eléctrica")
    app.setStyle("Fusion")
    v = VentanaPrincipal()
    if len(argv) > 1:
        if argv[1].lower().endswith(EXTENSION):
            v.abrir_proyecto(argv[1])
        elif argv[1].lower().endswith(".json"):
            v.importar_json(argv[1])
    v.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
