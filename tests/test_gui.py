"""Pruebas de la GUI sin pantalla (QT_QPA_PLATFORM=offscreen). Se omiten sin PySide6."""

import os
import shutil
from pathlib import Path

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytest.importorskip("PySide6.QtWidgets")

from PySide6.QtWidgets import QApplication, QMessageBox  # noqa: E402

from suite_electrica.gui import servicios as sv  # noqa: E402
from suite_electrica.gui.app import VentanaPrincipal  # noqa: E402
from suite_electrica.topocraft import sinteticos as st  # noqa: E402

EJEMPLOS = Path(__file__).resolve().parents[1] / "examples"


@pytest.fixture
def ventana(monkeypatch, tmp_path):
    app = QApplication.instance() or QApplication([])
    for nombre in ("information", "warning", "critical"):
        monkeypatch.setattr(QMessageBox, nombre, staticmethod(lambda *a, **k: None))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "cfg"))
    v = VentanaPrincipal(sincrono=True)
    v.ajustes.clear()
    yield v
    v.close()
    app.processEvents()


@pytest.fixture
def puntos(tmp_path):
    ruta = tmp_path / "pts.csv"
    st.malla(st.colina_gaussiana(), 20).to_csv(ruta, index=False)
    return ruta


def test_topocraft_procesar_y_exportar(ventana, puntos, tmp_path):
    p = ventana.topo
    p.puntos.campo.setText(str(puntos))
    p.eje.campo.setText(str(EJEMPLOS / "eje.dxf"))
    p.procesar()
    assert p.resultado is not None and p.resultado.perfil is not None
    assert p.modelo.rowCount() == len(p.resultado.perfil.progresiva)
    assert "Puntos leídos" in p.registro.toPlainText()
    assert p.plano.figura.axes and p.perfil.figura.axes
    p.exportar(tmp_path / "out")
    assert {r.name for r in p.exportados} == {"plano.dxf", "perfil.pdf", "perfil.xlsx", "perfil.dxf"}
    assert all(r.exists() for r in p.exportados)


def test_topocraft_error_se_informa(ventana, tmp_path):
    ventana.topo.puntos.campo.setText(str(tmp_path / "no_existe.csv"))
    ventana.topo.procesar()
    assert "FileNotFoundError" in ventana.ultimo_error


def test_linecraft_nuevo_calcular_exportar(ventana, puntos, tmp_path):
    carpeta = tmp_path / "linea"
    ventana.linea.nuevo(carpeta)
    shutil.copy(puntos, carpeta / "puntos.csv")
    shutil.copy(EJEMPLOS / "eje.dxf", carpeta / "eje.dxf")
    lp = ventana.linea
    assert lp.ruta == carpeta / "proyecto.json"
    assert '"nombre"' in lp.editor.toPlainText()
    lp.editor.setPlainText(lp.editor.toPlainText().replace("NOMBRE DEL PROYECTO", "Prueba GUI"))
    lp.calcular()  # guarda antes de calcular
    assert "Prueba GUI" in (carpeta / "proyecto.json").read_text(encoding="utf-8")
    assert lp.resultado is not None
    assert "CUMPLE" in lp.lbl_estado.text()
    assert lp.modelos["Estructuras"].rowCount() == len(lp.resultado.estructuras)
    lp.exportar(carpeta / "salidas")
    assert {r.name for r in lp.exportados} == {"perfil_linea.dxf", "planilla_estructuras.xlsx",
                                               "memoria_calculo.pdf"}
    assert str(carpeta / "proyecto.json") in ventana._recientes()


def test_json_invalido_no_se_guarda(ventana, tmp_path):
    ventana.linea.nuevo(tmp_path / "x")
    original = (tmp_path / "x" / "proyecto.json").read_text(encoding="utf-8")
    ventana.linea.editor.setPlainText("{ mal json")
    assert ventana.linea.guardar() is False
    assert (tmp_path / "x" / "proyecto.json").read_text(encoding="utf-8") == original


def test_abrir_dxf_usa_visor_configurado(ventana, monkeypatch, tmp_path):
    llamadas = []
    monkeypatch.setattr(sv.subprocess, "Popen", lambda args: llamadas.append(args))
    ventana.ajustes.setValue("visor_dxf", "/opt/cadcraft/cadcraft")
    ventana.abrir_dxf(tmp_path / "a.dxf")
    assert llamadas == [["/opt/cadcraft/cadcraft", str((tmp_path / "a.dxf").resolve())]]
