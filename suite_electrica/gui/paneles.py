"""Paneles de TopoCraft y LineCraft sobre el documento de proyecto."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any

import numpy as np
import pandas as pd
from PySide6.QtGui import QCursor, QFont
from PySide6.QtWidgets import (
    QCheckBox, QComboBox, QDoubleSpinBox, QFileDialog, QFormLayout, QGroupBox, QHBoxLayout,
    QHeaderView, QInputDialog, QLabel, QLineEdit, QMenu, QMessageBox, QPlainTextEdit, QPushButton,
    QScrollArea, QSpinBox, QSplitter, QTableView, QTabWidget, QVBoxLayout, QWidget,
)

from suite_electrica.core.units import formato_progresiva
from suite_electrica.documento import FUENTES_EXTRA, ParamLinea, _hip_df, conductor_a_dict, conductor_desde_dict
from suite_electrica.gui import servicios as sv
from suite_electrica.gui.perfil_interactivo import PerfilInteractivo
from suite_electrica.gui.widgets import DelegadoLista, Lienzo, ModeloTabla, TablaEditable, vista_tabla
from suite_electrica.linecraft import catalogos as cat_mod
from suite_electrica.linecraft import salidas as lc_salidas
from suite_electrica.linecraft.conductor import leer_catalogo
from suite_electrica.linecraft.estructuras import TIPOS
from suite_electrica.linecraft.proyecto import PARAMETROS_NORMATIVOS

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
    w.setKeyboardTracking(False)
    return w


def _boton(texto: str, accion: Callable[[], Any], principal: bool = False) -> QPushButton:
    b = QPushButton(texto)
    b.clicked.connect(accion)
    if principal:
        b.setStyleSheet("font-weight: bold; padding: 6px;")
    return b


def _desplazable(w: QWidget) -> QScrollArea:
    sa = QScrollArea()
    sa.setWidgetResizable(True)
    sa.setWidget(w)
    return sa


# ====================================================================== TopoCraft
class PanelTopo(QWidget):
    """TopoCraft: puntos → TIN, curvas, perfil."""

    def __init__(self, ventana: VentanaPrincipal) -> None:
        super().__init__()
        self.v = ventana
        self.exportados: list[Path] = []
        self._cargando = False

        self.lbl_datos = QLabel("Sin puntos ni eje")
        self.lbl_datos.setWordWrap(True)
        self.eq = _num(0.1, 100, 1.0, sufijo=" m")
        self.mayor = _num(1, 20, 5, None)
        self.paso = _num(1, 1000, 20, 1, sufijo=" m")
        self.max_lado = _num(0, 10000, 0, 1, sufijo=" m", especial="sin límite")
        self.atipicos = QCheckBox("Quitar cotas atípicas")
        for w in (self.eq, self.mayor, self.paso, self.max_lado):
            w.valueChanged.connect(self._params)
        self.atipicos.toggled.connect(self._params)

        datos = QGroupBox("Datos del levantamiento")
        ld = QVBoxLayout(datos)
        ld.addWidget(self.lbl_datos)
        ld.addWidget(_boton("Importar puntos (CSV/DXF)…", self.importar_puntos_dialogo))
        ld.addWidget(_boton("Importar eje (DXF/CSV)…", self.importar_eje_dialogo))
        params = QGroupBox("Parámetros")
        f2 = QFormLayout(params)
        f2.addRow("Equidistancia", self.eq)
        f2.addRow("Curva mayor cada", self.mayor)
        f2.addRow("Paso del perfil", self.paso)
        f2.addRow("Lado máx. triángulo", self.max_lado)
        f2.addRow(self.atipicos)

        izq = QWidget()
        li = QVBoxLayout(izq)
        li.addWidget(datos)
        li.addWidget(params)
        li.addWidget(_boton("▶ Procesar  (F5)", self.procesar, True))
        li.addWidget(_boton("Exportar DXF / PDF / Excel…", self.exportar_dialogo))
        li.addWidget(_boton("Abrir plano en visor DXF", self.abrir_plano))
        li.addStretch()

        self.tabs = QTabWidget()
        self.plano = Lienzo()
        self.perfil = Lienzo()
        self.tabla, self.modelo = vista_tabla()
        self.tabla_puntos, self.modelo_puntos = vista_tabla()
        self.registro = QPlainTextEdit()
        self.registro.setReadOnly(True)
        self.registro.setFont(QFont("monospace"))
        self.tabs.addTab(self.plano, "Plano")
        self.tabs.addTab(self.perfil, "Perfil")
        self.tabs.addTab(self.tabla, "Tabla perfil")
        self.tabs.addTab(self.tabla_puntos, "Puntos")
        self.tabs.addTab(self.registro, "Registro")

        sp = QSplitter()
        sp.addWidget(izq)
        sp.addWidget(self.tabs)
        sp.setStretchFactor(1, 1)
        QHBoxLayout(self).addWidget(sp)

    @property
    def doc(self) -> Any:
        return self.v.doc

    def cargar_documento(self) -> None:
        self._cargando = True
        t = self.doc.topo
        self.eq.setValue(t.equidistancia)
        self.mayor.setValue(t.mayor_cada)
        self.paso.setValue(t.paso_perfil)
        self.max_lado.setValue(t.max_lado or 0)
        self.atipicos.setChecked(t.quitar_atipicos)
        self._cargando = False
        self.exportados = []
        self.modelo.poner(pd.DataFrame())
        self.modelo_puntos.poner(self.doc.puntos)
        for lz in (self.plano, self.perfil):
            lz.figura.clear()
            lz.redibujar()
        self._resumen()

    def _resumen(self) -> None:
        d = self.doc
        eje = "sin eje" if d.eje is None else f"eje de {len(d.eje)} vértices"
        self.lbl_datos.setText(f"{len(d.puntos)} puntos · {eje}")

    def _params(self) -> None:
        if self._cargando:
            return
        t = self.doc.topo
        t.equidistancia, t.mayor_cada, t.paso_perfil = self.eq.value(), self.mayor.value(), self.paso.value()
        t.max_lado = self.max_lado.value() or None
        t.quitar_atipicos = self.atipicos.isChecked()
        self.doc.cambio_topografia()
        self.v.cambiado()

    def log(self, texto: str) -> None:
        self.registro.appendPlainText(texto)

    def importar_puntos_dialogo(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Importar puntos", "", "Puntos (*.csv *.txt *.dxf)")
        if r:
            self.importar_puntos(r)

    def importar_puntos(self, ruta: str | Path) -> None:
        try:
            self.doc.importar_puntos(ruta)
        except (ValueError, OSError) as e:
            self.v.error(str(e))
            return
        self.modelo_puntos.poner(self.doc.puntos)
        self.log(f"Importados {len(self.doc.puntos)} puntos de {ruta}")
        self._resumen()
        self.v.cambiado()

    def importar_eje_dialogo(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Importar eje", "", "Eje (*.dxf *.csv)")
        if r:
            self.importar_eje(r)

    def importar_eje(self, ruta: str | Path) -> None:
        try:
            self.doc.importar_eje(ruta)
        except (ValueError, OSError) as e:
            self.v.error(str(e))
            return
        self.log(f"Eje importado de {ruta} ({len(self.doc.eje)} vértices); estructuras reiniciadas")
        self._resumen()
        self.v.cambiado()
        self.v.linea.cargar_documento()

    def procesar(self) -> None:
        self.v.ejecutar("Procesando topografía…", self.doc.topografia, (), self._mostrar)

    def _mostrar(self, t: Any) -> None:
        self.log(t.reporte.resumen())
        self.log(f"TIN: {len(t.tin.triangulos_activos)} triángulos; {len(t.curvas)} curvas de nivel")
        sv.dibujar_plano(self.plano.figura, t, self.doc)
        self.plano.redibujar()
        if t.perfil is not None:
            sv.dibujar_perfil_topo(self.perfil.figura, t.perfil)
            self.perfil.redibujar()
            self.modelo.poner(sv.tabla_perfil(t.perfil))
            self.log(f"Perfil: {t.perfil.longitud:.2f} m, {len(t.perfil.progresiva)} puntos")
        self.tabs.setCurrentWidget(self.plano)
        self.v.estado("Topografía procesada")

    def exportar_dialogo(self) -> None:
        base = str(self.doc.ruta.parent) if self.doc.ruta else ""
        carpeta = QFileDialog.getExistingDirectory(self, "Carpeta de salida", base)
        if carpeta:
            self.exportar(carpeta)

    def exportar(self, carpeta: str | Path) -> None:
        self.v.ejecutar("Exportando…", sv.exportar_topo, (self.doc, carpeta), self._exportado)

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


# ====================================================================== LineCraft
# (atributo, etiqueta, especificación): numérico = (min, max, decimales, sufijo);
# lista = ("opcion", ...); "hip" = nombres de hipótesis; "ret" = retenidas del catálogo.
CAMPOS_LINEA: list[tuple[str, str, tuple]] = [
    ("n_conductores", "Conductores por circuito", (1, 12, None, "")),
    ("modelo", "Modelo de curva", ("catenaria", "parabola")),
    ("hip_inicial", "Condición inicial (EDS)", ("hip",)),
    ("tiro_inicial_pct", "Tiro inicial (% rotura)", (1, 60, 2, " %")),
    ("hip_flecha_maxima", "Hipótesis de flecha máxima", ("hip",)),
    ("tiro_maximo_pct", "Tiro máximo admisible (% rotura)", (1, 100, 2, " %")),
    ("distancia_minima_terreno", "Distancia mínima al terreno", (0, 50, 2, " m")),
    ("altura_amarre", "Altura de amarre por defecto", (1, 60, 2, " m")),
    ("vano_maximo", "Vano máximo (ubicación automática)", (10, 2000, 1, " m")),
    ("vano_minimo", "Vano mínimo (ubicación automática)", (1, 2000, 1, " m")),
    ("angulo_retenida", "Ángulo de retenida (con la horizontal)", (1, 89, 1, " °")),
    ("fs_poste", "Factor de seguridad de postes", (1, 10, 2, "")),
    ("fs_retenida", "Factor de seguridad de retenidas", (1, 10, 2, "")),
    ("retenida", "Retenida (catálogo)", ("ret",)),
    ("desperdicio_pct", "Desperdicio de conductor (metrado)", (0, 20, 1, " %")),
]
CAMPOS_CONDUCTOR = [("nombre", "Nombre", None), ("seccion_mm2", "Sección (mm²)", 4),
                    ("diametro_mm", "Diámetro (mm)", 3), ("masa_kg_m", "Masa (kg/m)", 5),
                    ("carga_rotura_kN", "Tiro de rotura (kN)", 3),
                    ("modulo_elasticidad_MPa", "Módulo de elasticidad (MPa)", 0),
                    ("coef_dilatacion_1_C", "Coef. dilatación (1/°C)", "e"), ("fuente", "Fuente", None)]
COLS_PLANILLA = ["N°", "Código", "Progresiva (m)", "Tipo", "Armado", "Poste", "Altura amarre (m)",
                 "Retenidas", "Cota terreno (m)", "Deflexión (°)", "Vano adelante (m)", "Verificación"]
EDITABLES_PLANILLA = {"Progresiva (m)", "Tipo", "Armado", "Poste", "Altura amarre (m)", "Retenidas"}
_CAMPO_DOC = {"Tipo": "tipo", "Armado": "armado", "Poste": "poste", "Altura amarre (m)": "altura_amarre_m",
              "Retenidas": "retenidas"}


class PanelLinea(QWidget):
    """LineCraft: datos en formularios, perfil interactivo, planilla y resultados."""

    TABLAS = ("Vanos", "Mecanico", "Cargas", "Verificacion", "Metrado", "Avisos")

    def __init__(self, ventana: VentanaPrincipal) -> None:
        super().__init__()
        self.v = ventana
        self.resultado: Any = None
        self.exportados: list[Path] = []
        self._cargando = False

        self.lbl_estado = QLabel("Sin calcular")
        self.lbl_estado.setWordWrap(True)
        self.lbl_estado.setStyleSheet("font-size: 15px; font-weight: bold;")
        izq = QWidget()
        li = QVBoxLayout(izq)
        g = QGroupBox("Estructuras")
        lg = QVBoxLayout(g)
        lg.addWidget(_boton("Ubicación automática", self.ubicar_automatico))
        lg.addWidget(_boton("Solo inicio, fin y vértices", self.estructuras_minimas))
        lg.addWidget(_boton("Asignar armados y postes", self.asignar_catalogos))
        lg.addWidget(_boton("↶ Deshacer  (Ctrl+Z)", self.deshacer))
        li.addWidget(_boton("▶ Calcular  (F6)", self.recalcular, True))
        li.addWidget(self.lbl_estado)
        li.addWidget(g)
        li.addWidget(_boton("Exportar DXF / Excel / PDF…", self.exportar_dialogo))
        li.addWidget(_boton("Abrir perfil en visor DXF", self.abrir_perfil))
        li.addWidget(_boton("Abrir memoria PDF", self.abrir_memoria))
        li.addStretch()

        self.tabs = QTabWidget()
        self.perfil = PerfilInteractivo(self)
        self.tabs.addTab(self.perfil, "Perfil")
        self.tabs.addTab(self._crear_planilla(), "Planilla de estructuras")
        self.tabs.addTab(_desplazable(self._crear_datos()), "Datos")
        self.tabs.addTab(_desplazable(self._crear_conductor()), "Conductor")
        self.hip = TablaEditable(self._hipotesis_cambiadas)
        self.tabs.addTab(self.hip, "Hipótesis")
        self.fuentes = TablaEditable(self._fuentes_cambiadas, filas=False)
        self.tabs.addTab(self.fuentes, "Fuentes")
        self.catalogos = QTabWidget()
        self.tablas_cat: dict[str, TablaEditable] = {}
        for nombre in cat_mod.COLUMNAS:
            t = TablaEditable(lambda df, n=nombre: self._catalogo_cambiado(n, df), csv=True)
            self.tablas_cat[nombre] = t
            self.catalogos.addTab(t, nombre.capitalize())
        self.tabs.addTab(self.catalogos, "Catálogos")
        self.modelos: dict[str, ModeloTabla] = {}
        for t in self.TABLAS:
            vista, self.modelos[t] = vista_tabla()
            self.tabs.addTab(vista, {"Mecanico": "Mecánico", "Verificacion": "Verificación"}.get(t, t))

        sp = QSplitter()
        sp.addWidget(izq)
        sp.addWidget(self.tabs)
        sp.setStretchFactor(1, 1)
        QHBoxLayout(self).addWidget(sp)

    @property
    def doc(self) -> Any:
        return self.v.doc

    # ------------------------------------------------------------------ construcción
    def _crear_planilla(self) -> QWidget:
        self.modelo_planilla = ModeloTabla(editable=EDITABLES_PLANILLA, al_editar=self._planilla_editada)
        vista = QTableView()
        vista.setModel(self.modelo_planilla)
        vista.setAlternatingRowColors(True)
        vista.verticalHeader().setVisible(False)
        vista.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.ResizeToContents)
        cols = COLS_PLANILLA
        vista.setItemDelegateForColumn(cols.index("Tipo"), DelegadoLista(lambda: [""] + list(TIPOS), vista))
        vista.setItemDelegateForColumn(cols.index("Armado"), DelegadoLista(
            lambda: [""] + self.doc.catalogos.armados["codigo"].tolist(), vista))
        vista.setItemDelegateForColumn(cols.index("Poste"), DelegadoLista(
            lambda: [""] + self.doc.catalogos.postes["codigo"].tolist(), vista))
        vista.clicked.connect(lambda idx: self.seleccionar(idx.row(), desde_planilla=True))
        self.vista_planilla = vista
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.addWidget(QLabel("Edite progresiva, tipo, armado, poste, altura (vacío = automática) o "
                             "retenidas; la línea se recalcula al instante."))
        lay.addWidget(vista)
        return w

    def _crear_datos(self) -> QWidget:
        w = QWidget()
        f = QFormLayout(w)
        self.ed_nombre, self.ed_autor = QLineEdit(), QLineEdit()
        self.ed_zona = QComboBox()
        self.ed_zona.addItems(["17S", "18S", "19S"])
        f.addRow("Nombre del proyecto", self.ed_nombre)
        f.addRow("Autor", self.ed_autor)
        f.addRow("Zona UTM (WGS84)", self.ed_zona)
        self.ed_nombre.editingFinished.connect(self._datos_generales)
        self.ed_autor.editingFinished.connect(self._datos_generales)
        self.ed_zona.currentTextChanged.connect(self._datos_generales)
        self.campos: dict[str, QWidget] = {}
        for attr, etiqueta, spec in CAMPOS_LINEA:
            if isinstance(spec[0], str):
                cb = QComboBox()
                if spec[0] not in ("hip", "ret"):
                    cb.addItems(list(spec))
                cb.currentTextChanged.connect(lambda t, a=attr: self._campo_linea(a, t))
                campo: QWidget = cb
            else:
                mn, mx, dec, suf = spec
                campo = _num(mn, mx, getattr(ParamLinea(), attr), dec, sufijo=suf)
                campo.valueChanged.connect(lambda val, a=attr: self._campo_linea(a, val))
            self.campos[attr] = campo
            f.addRow(etiqueta, campo)
        nota = QLabel("Los valores normativos deben citar su fuente en la pestaña «Fuentes». "
                      "Tras cambiar datos, pulse Calcular (F6).")
        nota.setWordWrap(True)
        f.addRow(nota)
        return w

    def _crear_conductor(self) -> QWidget:
        w = QWidget()
        f = QFormLayout(w)
        self.campos_cond: dict[str, QWidget] = {}
        for k, etiqueta, dec in CAMPOS_CONDUCTOR:
            if dec is None or dec == "e":
                ed: QWidget = QLineEdit()
                ed.editingFinished.connect(self._conductor_cambiado)
            else:
                ed = _num(0, 1e7, 0, dec)
                ed.valueChanged.connect(self._conductor_cambiado)
            self.campos_cond[k] = ed
            f.addRow(etiqueta, ed)
        f.addRow(_boton("Cargar desde catálogo CSV…", self._cargar_conductor))
        return w

    # ------------------------------------------------------------------ documento → widgets
    def cargar_documento(self) -> None:
        self._cargando = True
        d = self.doc
        self.ed_nombre.setText(d.nombre)
        self.ed_autor.setText(d.autor)
        self.ed_zona.setCurrentText(d.zona_utm)
        self._llenar_combos()
        for attr, w in self.campos.items():
            val = getattr(d.linea, attr)
            if isinstance(w, QComboBox):
                w.setCurrentText(str(val))
            else:
                w.setValue(val)
        cd = conductor_a_dict(d.conductor)
        for k, ed in self.campos_cond.items():
            if isinstance(ed, QLineEdit):
                ed.setText(f"{cd[k]:.3e}" if k == "coef_dilatacion_1_C" else str(cd[k]))
            else:
                ed.setValue(float(cd[k]))
        self.hip.poner(d.hipotesis)
        claves = list(PARAMETROS_NORMATIVOS) + list(FUENTES_EXTRA)
        self.fuentes.poner(pd.DataFrame({"Parámetro": claves,
                                         "Fuente": [d.fuentes.get(k, "") for k in claves]}))
        self.fuentes.modelo.editable = {"Fuente"}
        for nombre, t in self.tablas_cat.items():
            t.poner(d.catalogos.tabla(nombre))
        self._cargando = False
        self.resultado = None
        self.exportados = []
        self.perfil.seleccion = None
        self.perfil.reiniciar_vista()
        self.perfil.figura.clear()
        self.perfil.redibujar()
        self.lbl_estado.setText("Sin calcular")
        self.lbl_estado.setStyleSheet("font-size: 15px; font-weight: bold;")
        self._llenar_planilla()

    def _llenar_combos(self) -> None:
        nombres = [n for n in self.doc.hipotesis["nombre"].tolist() if n]
        rets = [""] + self.doc.catalogos.retenidas["codigo"].tolist()
        previo = self._cargando
        self._cargando = True
        for attr, opciones in (("hip_inicial", nombres), ("hip_flecha_maxima", nombres), ("retenida", rets)):
            cb = self.campos[attr]
            assert isinstance(cb, QComboBox)
            cb.clear()
            cb.addItems(opciones)
            cb.setCurrentText(str(getattr(self.doc.linea, attr)))
        self._cargando = previo

    # ------------------------------------------------------------------ widgets → documento
    def _marcar(self, texto: str = "Datos modificados: pulse Calcular (F6)") -> None:
        self.v.cambiado()
        if self.resultado is not None:
            self.lbl_estado.setText("⟳ " + texto)
            self.lbl_estado.setStyleSheet("font-size: 13px; font-weight: bold; color: #b9770e;")

    def _datos_generales(self) -> None:
        if self._cargando:
            return
        d = self.doc
        d.nombre = self.ed_nombre.text().strip()
        d.autor = self.ed_autor.text().strip()
        d.zona_utm = self.ed_zona.currentText()
        d.modificado = True
        self.v.cambiado()

    def _campo_linea(self, attr: str, valor: Any) -> None:
        if self._cargando:
            return
        tipo = type(getattr(ParamLinea(), attr))
        setattr(self.doc.linea, attr, tipo(valor))
        self.doc.modificado = True
        self._marcar()

    def _conductor_cambiado(self) -> None:
        if self._cargando:
            return
        datos = {k: (ed.text() if isinstance(ed, QLineEdit) else ed.value())
                 for k, ed in self.campos_cond.items()}
        try:
            datos["coef_dilatacion_1_C"] = float(str(datos["coef_dilatacion_1_C"]).replace(",", "."))
            self.doc.conductor = conductor_desde_dict(datos)
        except ValueError:
            self.v.estado("Coeficiente de dilatación inválido")
            return
        self.doc.modificado = True
        self._marcar()

    def _cargar_conductor(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Catálogo de conductores", "", "CSV (*.csv)")
        if not r:
            return
        try:
            cat = leer_catalogo(r)
        except Exception as e:  # noqa: BLE001
            self.v.error(f"Catálogo inválido: {e}")
            return
        nombre, ok = QInputDialog.getItem(self, "Conductor", "Elija el conductor", list(cat), 0, False)
        if ok:
            self.doc.conductor = cat[nombre]
            self.doc.modificado = True
            self.cargar_documento()
            self._marcar()

    def _hipotesis_cambiadas(self, df: pd.DataFrame) -> None:
        self.doc.hipotesis = _hip_df(df.to_dict(orient="records"))
        self.doc.modificado = True
        self._llenar_combos()
        self._marcar()

    def _fuentes_cambiadas(self, df: pd.DataFrame) -> None:
        self.doc.fuentes = {str(k): str(v) for k, v in zip(df["Parámetro"], df["Fuente"])}
        self.doc.modificado = True
        self._marcar()

    def _catalogo_cambiado(self, nombre: str, df: pd.DataFrame) -> None:
        setattr(self.doc.catalogos, nombre, cat_mod.normalizar(nombre, df))
        self.doc.modificado = True
        if nombre == "retenidas":
            self._llenar_combos()
        self._marcar()

    # ------------------------------------------------------------------ cálculo
    def recalcular(self) -> None:
        """Calcula la línea (síncrono: es rápido) y refresca todo."""
        try:
            r = self.doc.calcular()
        except Exception as e:  # noqa: BLE001
            self.resultado = None
            self.lbl_estado.setText(f"✘ No se pudo calcular:\n{e}")
            self.lbl_estado.setStyleSheet("font-size: 12px; font-weight: bold; color: #c0392b;")
            self.v.estado(f"Error: {e}")
            self._llenar_planilla()
            return
        self.resultado = r
        self.v.cambiado()
        self._refrescar()

    def _refrescar(self) -> None:
        r = self.resultado
        n = len(self.doc.estructuras)
        bloqueadas = [self.doc.bloqueada(i) for i in range(n)]
        if self.perfil.seleccion is not None and self.perfil.seleccion >= n:
            self.perfil.seleccion = None
        self.perfil.dibujar(r, bloqueadas)
        tablas = lc_salidas.tablas(r)
        for t in self.TABLAS:
            self.modelos[t].poner(tablas.get(t, pd.DataFrame()))
        self._llenar_planilla()
        ok = r.cumple
        criticos = [a for a in r.avisos if "PENDIENTE" not in a]
        self.lbl_estado.setText(("✔ CUMPLE" if ok else "✘ NO CUMPLE")
                                + f"\n{len(r.estructuras)} estructuras · {len(r.vanos)} vanos"
                                + f"\n{len(criticos)} aviso(s) técnicos · "
                                  f"{len(r.avisos) - len(criticos)} pendiente(s) de fuente")
        self.lbl_estado.setStyleSheet(
            f"font-size: 14px; font-weight: bold; color: {'#1e8449' if ok else '#c0392b'};")
        self.v.estado(f"{len(r.estructuras)} estructuras, {len(r.vanos)} vanos — "
                      + ("CUMPLE" if ok else "NO CUMPLE"))

    def _llenar_planilla(self) -> None:
        d = self.doc
        r = self.resultado
        ver = {} if r is None or r.verificacion is None else dict(
            zip(r.verificacion["Estructura"], r.verificacion["Cumple"]))
        filas = []
        for i, f in d.estructuras.iterrows():
            e = r.estructuras[i] if r is not None and i < len(r.estructuras) else None
            filas.append({
                "N°": i + 1, "Código": f"E-{i + 1:02d}", "Progresiva (m)": float(f.progresiva_m),
                "Tipo": f.tipo or (e.tipo if e else ""), "Armado": f.armado, "Poste": f.poste,
                "Altura amarre (m)": f.altura_amarre_m, "Retenidas": f.retenidas,
                "Cota terreno (m)": round(e.cota_terreno, 2) if e else np.nan,
                "Deflexión (°)": round(e.angulo, 2) if e else np.nan,
                "Vano adelante (m)": round(r.vanos[i].longitud, 2)
                if r is not None and i < len(r.vanos) else np.nan,
                "Verificación": ver.get(f"E-{i + 1:02d}", ""),
            })
        df = pd.DataFrame(filas, columns=COLS_PLANILLA)
        for c in ("Progresiva (m)", "Altura amarre (m)", "Retenidas", "Cota terreno (m)", "Deflexión (°)",
                  "Vano adelante (m)"):
            df[c] = pd.to_numeric(df[c], errors="coerce").astype(float)
        for c in ("Código", "Tipo", "Armado", "Poste", "Verificación"):
            df[c] = df[c].astype(object)
        self.modelo_planilla.poner(df)

    # ------------------------------------------------------------------ edición de estructuras
    def _aplicar(self, fn: Callable[[], Any]) -> None:
        try:
            fn()
        except ValueError as e:
            self.v.estado(str(e))
            QMessageBox.warning(self, "Estructuras", str(e))
            self._llenar_planilla()
            return
        self.recalcular()

    def _planilla_editada(self, fila: int, col: str, valor: Any) -> None:
        if col == "Progresiva (m)":
            self._aplicar(lambda: self.doc.mover_estructura(fila, float(valor)))
            return
        v = "" if isinstance(valor, float) and np.isnan(valor) else valor
        if col == "Tipo":
            v = str(v).upper()
        self._aplicar(lambda: self.doc.cambiar_estructura(fila, _CAMPO_DOC[col], v))

    def agregar(self, s: float) -> None:
        def f() -> None:
            self.perfil.seleccion = self.doc.agregar_estructura(s)
        self._aplicar(f)

    def mover(self, i: int, s: float) -> None:
        self._aplicar(lambda: self.doc.mover_estructura(i, s))

    def eliminar(self, i: int) -> None:
        self.perfil.seleccion = None
        self._aplicar(lambda: self.doc.eliminar_estructura(i))

    def cambiar(self, i: int, campo: str, valor: Any) -> None:
        self._aplicar(lambda: self.doc.cambiar_estructura(i, campo, valor))

    def seleccionar(self, i: int, desde_planilla: bool = False) -> None:
        self.perfil.seleccion = i
        if self.resultado is not None and len(self.resultado.estructuras) == len(self.doc.estructuras):
            self.perfil.dibujar(self.resultado, [self.doc.bloqueada(k) for k in range(len(self.doc.estructuras))])
        if not desde_planilla:
            self.vista_planilla.selectRow(i)
        e = self.doc.estructuras.iloc[i]
        self.v.estado(f"E-{i + 1:02d}  {formato_progresiva(float(e.progresiva_m))}"
                      + ("  (fija)" if self.doc.bloqueada(i) else ""))

    def menu_estructura(self, i: int) -> QMenu:
        """Menú contextual de una estructura."""
        d = self.doc
        e = d.estructuras.iloc[i]
        m = QMenu(self)
        t = m.addAction(f"E-{i + 1:02d}  ·  {formato_progresiva(float(e.progresiva_m))}")
        t.setEnabled(False)
        m.addSeparator()
        for titulo, campo, opciones in (
                ("Tipo", "tipo", [""] + list(TIPOS)),
                ("Armado", "armado", [""] + d.catalogos.armados["codigo"].tolist()),
                ("Poste", "poste", [""] + d.catalogos.postes["codigo"].tolist())):
            sub = m.addMenu(titulo)
            for op in opciones:
                a = sub.addAction(op or "(automático)")
                a.setCheckable(True)
                a.setChecked(str(e[campo]) == op)
                a.triggered.connect(lambda _=False, c=campo, o=op: self.cambiar(i, c, o))
        m.addAction("Altura de amarre…").triggered.connect(lambda: self._pedir_numero(i, "altura_amarre_m"))
        m.addAction("Número de retenidas…").triggered.connect(lambda: self._pedir_numero(i, "retenidas"))
        m.addSeparator()
        el = m.addAction("Eliminar estructura")
        el.setEnabled(not d.bloqueada(i))
        el.triggered.connect(lambda: self.eliminar(i))
        self.ultimo_menu = m
        if not self.v.sincrono:
            m.exec(QCursor.pos())
        return m

    def _pedir_numero(self, i: int, campo: str) -> None:
        actual = self.doc.estructuras.at[i, campo]
        texto, ok = QInputDialog.getText(self, "Estructura", f"{campo} (vacío = automático):",
                                         text="" if pd.isna(actual) else f"{actual:g}")
        if ok:
            self.cambiar(i, campo, texto.strip().replace(",", "."))

    def ubicar_automatico(self) -> None:
        if len(self.doc.estructuras) and not self.v.sincrono:
            r = QMessageBox.question(self, "Ubicación automática",
                                     "Se reemplazarán las estructuras actuales (puede deshacer). ¿Continuar?")
            if r != QMessageBox.StandardButton.Yes:
                return
        self.perfil.seleccion = None
        self._aplicar(self.doc.ubicar_automatico)

    def estructuras_minimas(self) -> None:
        self.perfil.seleccion = None
        self._aplicar(self.doc.estructuras_minimas)

    def asignar_catalogos(self) -> None:
        self._aplicar(lambda: self.doc.asignar_catalogos(solo_vacios=True))

    def deshacer(self) -> None:
        if self.doc.deshacer():
            self.recalcular()
        else:
            self.v.estado("Nada que deshacer")

    # ------------------------------------------------------------------ salidas
    def exportar_dialogo(self) -> None:
        if self.resultado is None:
            QMessageBox.information(self, "LineCraft", "Primero calcule.")
            return
        base = str(self.doc.ruta.parent / "salidas") if self.doc.ruta else ""
        carpeta = QFileDialog.getExistingDirectory(self, "Carpeta de salida", base)
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
