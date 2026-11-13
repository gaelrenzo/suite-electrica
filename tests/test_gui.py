"""Pruebas de la GUI sin pantalla (QT_QPA_PLATFORM=offscreen). Se omiten sin PySide6."""

import os
from pathlib import Path
from types import SimpleNamespace

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytest.importorskip("PySide6.QtWidgets")

from PySide6.QtCore import Qt  # noqa: E402
from PySide6.QtWidgets import QApplication, QMessageBox  # noqa: E402

from suite_electrica.gui import servicios as sv  # noqa: E402
from suite_electrica.gui.app import VentanaPrincipal  # noqa: E402

EJ = Path(__file__).resolve().parents[1] / "examples"


@pytest.fixture
def ventana(monkeypatch, tmp_path):
    app = QApplication.instance() or QApplication([])
    for nombre in ("information", "warning", "critical"):
        monkeypatch.setattr(QMessageBox, nombre, staticmethod(lambda *a, **k: None))
    v = VentanaPrincipal(sincrono=True)
    v.ajustes.clear()
    yield v
    v.doc.modificado = False
    v.close()
    app.processEvents()


@pytest.fixture
def con_datos(ventana):
    ventana.topo.importar_puntos(EJ / "colina_gaussiana.csv")
    ventana.topo.importar_eje(EJ / "eje.dxf")
    return ventana


def evento(lienzo, s, z, boton=1, doble=False):
    """Evento de ratón falso en coordenadas de datos."""
    ax = lienzo._ax
    x, y = ax.transData.transform((s, z))
    return SimpleNamespace(inaxes=ax, xdata=s, ydata=z, x=x, y=y, button=boton, dblclick=doble)


def test_topocraft(con_datos, tmp_path):
    v = con_datos
    assert "2601 puntos" in v.topo.lbl_datos.text()
    v.topo.procesar()
    assert v.topo.modelo.rowCount() > 0 and v.topo.plano.figura.axes
    v.topo.eq.setValue(2.0)
    assert v.doc.topo.equidistancia == 2.0 and v.doc.modificado
    v.topo.exportar(tmp_path)
    assert {r.name for r in v.topo.exportados} == {"plano.dxf", "perfil.pdf", "perfil.xlsx", "perfil.dxf"}


def test_linea_calcular_y_planilla(con_datos, tmp_path):
    v, lp = con_datos, con_datos.linea
    lp.recalcular()
    assert lp.resultado is not None and "CUMPLE" in lp.lbl_estado.text()
    n = len(v.doc.estructuras)
    assert lp.modelo_planilla.rowCount() == n
    # editar armado desde la planilla
    col = lp.modelo_planilla.df.columns.get_loc("Armado")
    lp.modelo_planilla.setData(lp.modelo_planilla.index(1, col), "PA1-3", Qt.ItemDataRole.EditRole)
    assert v.doc.estructuras.at[1, "armado"] == "PA1-3"
    # mover desde la planilla
    col = lp.modelo_planilla.df.columns.get_loc("Progresiva (m)")
    lp.modelo_planilla.setData(lp.modelo_planilla.index(1, col), "100", Qt.ItemDataRole.EditRole)
    assert v.doc.estructuras.at[1, "progresiva_m"] == 100
    assert lp.modelos["Metrado"].rowCount() > 0 and lp.modelos["Verificacion"].rowCount() == n
    lp.exportar(tmp_path)
    assert {r.name for r in lp.exportados} == {"perfil_linea.dxf", "planilla_estructuras.xlsx",
                                               "memoria_calculo.pdf"}


def test_perfil_interactivo_raton(con_datos):
    v, lp = con_datos, con_datos.linea
    lp.recalcular()
    pi = lp.perfil
    n = len(v.doc.estructuras)
    # doble clic en zona libre -> agrega
    pi.al_presionar(evento(pi, 60.0, 3900.0, doble=True))
    assert len(v.doc.estructuras) == n + 1
    i = int((v.doc.estructuras["progresiva_m"] - 60).abs().idxmin())
    e = lp.resultado.estructuras[i]
    # arrastrar el poste nuevo a 80 m
    pi.al_presionar(evento(pi, e.progresiva, (e.cota_terreno + e.cota_amarre) / 2))
    assert pi._arrastre == i
    pi.al_mover(evento(pi, 80.0, e.cota_terreno))
    pi.al_soltar(evento(pi, 80.0, e.cota_terreno))
    assert v.doc.estructuras.at[i, "progresiva_m"] == pytest.approx(80.0)
    # el inicio (fijo) no se arrastra
    e0 = lp.resultado.estructuras[0]
    pi.al_presionar(evento(pi, e0.progresiva, e0.cota_amarre))
    assert pi._arrastre is None
    # clic derecho -> menú; eliminar
    e = lp.resultado.estructuras[i]
    pi.al_presionar(evento(pi, e.progresiva, e.cota_amarre, boton=3))
    menu = lp.ultimo_menu
    accion = [a for a in menu.actions() if a.text() == "Eliminar estructura"][0]
    assert accion.isEnabled()
    accion.trigger()
    assert len(v.doc.estructuras) == n
    lp.deshacer()
    assert len(v.doc.estructuras) == n + 1


def test_formularios_actualizan_documento(con_datos):
    lp = con_datos.linea
    lp.campos["distancia_minima_terreno"].setValue(12.0)
    assert con_datos.doc.linea.distancia_minima_terreno == 12.0
    lp.recalcular()
    assert "NO CUMPLE" in lp.lbl_estado.text()
    lp.campos_cond["nombre"].setText("Conductor X")
    lp.campos_cond["nombre"].editingFinished.emit()
    assert con_datos.doc.conductor.nombre == "Conductor X"
    lp.hip.agregar_fila()
    assert len(con_datos.doc.hipotesis) == 6
    df = lp.fuentes.df()
    df.loc[0, "Fuente"] = "CNE 2011 Regla X"
    lp._fuentes_cambiadas(df)
    assert con_datos.doc.fuentes[df.loc[0, "Parámetro"]] == "CNE 2011 Regla X"


def test_guardar_abrir_sel(con_datos, tmp_path):
    v = con_datos
    v.linea.recalcular()
    assert v.guardar(tmp_path / "linea.sel")
    assert not v.doc.modificado and "linea.sel" in v.windowTitle()
    n = len(v.doc.estructuras)
    v.nuevo()
    assert v.doc.puntos.empty
    v.abrir_proyecto(tmp_path / "linea.sel")
    assert len(v.doc.estructuras) == n and v.linea.resultado is not None
    assert str(tmp_path / "linea.sel") in v.recientes()


def test_importar_json_y_error(ventana, tmp_path):
    ventana.importar_json(EJ / "linecraft" / "proyecto_manual.json")
    assert len(ventana.doc.estructuras) == 8
    assert "NO CUMPLE" in ventana.linea.lbl_estado.text()
    (tmp_path / "x.sel").write_text("{}")
    ventana.abrir_proyecto(tmp_path / "x.sel")
    assert "No se pudo abrir" in ventana.ultimo_error


def test_abrir_dxf_usa_visor_configurado(ventana, monkeypatch, tmp_path):
    llamadas = []
    monkeypatch.setattr(sv.subprocess, "Popen", lambda args: llamadas.append(args))
    ventana.ajustes.setValue("visor_dxf", "/opt/cadcraft/cadcraft")
    ventana.abrir_dxf(tmp_path / "a.dxf")
    assert llamadas == [["/opt/cadcraft/cadcraft", str((tmp_path / "a.dxf").resolve())]]


def test_gridcraft_y_presucraft(ventana, tmp_path):
    ventana.grid.nuevo(tmp_path / "red")
    ventana.grid.calcular()
    assert "CUMPLE" in ventana.grid.lbl.text() and ventana.grid.tabs.count() == 4
    rutas = ventana.grid.exportar(tmp_path / "red" / "salidas")
    assert {p.name for p in rutas} == {"caida_tension.xlsx", "plano_red.dxf"}

    ventana.presu.nuevo(tmp_path / "presu")
    ventana.presu.calcular()
    assert "Total S/" in ventana.presu.lbl.text()
    assert ventana.metrado_a_presupuesto() is None  # sin línea calculada -> error
    ventana.topo.importar_puntos(EJ / "colina_gaussiana.csv")
    ventana.topo.importar_eje(EJ / "eje.dxf")
    ventana.linea.recalcular()
    ruta = ventana.metrado_a_presupuesto()
    assert ruta.exists() and "AAAC 35" in ruta.read_text(encoding="utf-8")
