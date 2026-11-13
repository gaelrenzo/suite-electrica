"""Componentes Qt reutilizables."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import pandas as pd
from matplotlib.backends.backend_qtagg import FigureCanvasQTAgg, NavigationToolbar2QT
from matplotlib.figure import Figure
from PySide6.QtCore import QAbstractTableModel, QModelIndex, QObject, QRunnable, Qt, Signal
from PySide6.QtWidgets import (
    QComboBox, QFileDialog, QHBoxLayout, QHeaderView, QLineEdit, QPushButton, QStyledItemDelegate,
    QTableView, QVBoxLayout, QWidget,
)


class ModeloTabla(QAbstractTableModel):
    """Modelo sobre un DataFrame; editable si ``editable`` (todas o un conjunto de columnas).

    ``al_editar(fila, columna, valor)`` se llama tras cada edición válida.
    """

    def __init__(self, df: pd.DataFrame | None = None, editable: bool | set[str] = False,
                 al_editar: Callable[[int, str, Any], None] | None = None) -> None:
        super().__init__()
        self.df = df if df is not None else pd.DataFrame()
        self.editable = editable
        self.al_editar = al_editar

    def _col_editable(self, col: str) -> bool:
        return self.editable is True or (isinstance(self.editable, set) and col in self.editable)

    def flags(self, index: QModelIndex) -> Qt.ItemFlag:
        f = super().flags(index)
        if index.isValid() and self._col_editable(str(self.df.columns[index.column()])):
            f |= Qt.ItemFlag.ItemIsEditable
        return f

    def setData(self, index: QModelIndex, value: Any, role: int = Qt.ItemDataRole.EditRole) -> bool:  # noqa: N802
        if role != Qt.ItemDataRole.EditRole or not index.isValid():
            return False
        col = str(self.df.columns[index.column()])
        texto = str(value).strip()
        if pd.api.types.is_numeric_dtype(self.df[col]):
            if texto == "":
                valor: Any = float("nan")
            else:
                try:
                    valor = float(texto.replace(",", "."))
                except ValueError:
                    return False
        else:
            valor = texto
        self.df.iat[index.row(), index.column()] = valor
        self.dataChanged.emit(index, index)
        if self.al_editar:
            self.al_editar(index.row(), col, valor)
        return True

    def poner(self, df: pd.DataFrame) -> None:
        self.beginResetModel()
        self.df = df.reset_index(drop=True)
        self.endResetModel()

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:  # noqa: N802
        return 0 if parent.isValid() else len(self.df)

    def columnCount(self, parent: QModelIndex = QModelIndex()) -> int:  # noqa: N802
        return 0 if parent.isValid() else len(self.df.columns)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole) -> Any:
        if not index.isValid():
            return None
        v = self.df.iat[index.row(), index.column()]
        if role in (Qt.ItemDataRole.DisplayRole, Qt.ItemDataRole.EditRole):
            if pd.isna(v):
                return ""
            if isinstance(v, float):
                return f"{v:.3f}".rstrip("0").rstrip(".")
            return str(v)
        if role == Qt.ItemDataRole.ForegroundRole and v in ("NO",):
            from PySide6.QtGui import QColor
            return QColor("#c0392b")
        if role == Qt.ItemDataRole.TextAlignmentRole and isinstance(v, (int, float)):
            return int(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        return None

    def headerData(self, s: int, o: Qt.Orientation, role: int = Qt.ItemDataRole.DisplayRole) -> Any:  # noqa: N802
        if role != Qt.ItemDataRole.DisplayRole:
            return None
        return str(self.df.columns[s]) if o == Qt.Orientation.Horizontal else str(s + 1)


def vista_tabla() -> tuple[QTableView, ModeloTabla]:
    """Tabla con modelo de DataFrame."""
    m = ModeloTabla()
    v = QTableView()
    v.setModel(m)
    v.setAlternatingRowColors(True)
    v.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.ResizeToContents)
    return v, m


class Lienzo(QWidget):
    """Figura matplotlib con barra de zoom/desplazamiento."""

    def __init__(self) -> None:
        super().__init__()
        self.figura = Figure(figsize=(8, 5))
        self.canvas = FigureCanvasQTAgg(self.figura)
        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        self.barra = NavigationToolbar2QT(self.canvas, self)
        lay.addWidget(self.barra)
        lay.addWidget(self.canvas)

    def redibujar(self) -> None:
        self.canvas.draw_idle()


class SelectorArchivo(QWidget):
    """Campo de ruta con botón «…»."""

    def __init__(self, filtro: str, guardar: bool = False, carpeta: bool = False) -> None:
        super().__init__()
        self.filtro, self.guardar, self.carpeta = filtro, guardar, carpeta
        self.campo = QLineEdit()
        boton = QPushButton("…")
        boton.setFixedWidth(32)
        boton.clicked.connect(self._elegir)
        lay = QHBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.addWidget(self.campo)
        lay.addWidget(boton)

    def _elegir(self) -> None:
        if self.carpeta:
            r = QFileDialog.getExistingDirectory(self, "Elegir carpeta", self.campo.text())
        elif self.guardar:
            r, _ = QFileDialog.getSaveFileName(self, "Guardar", self.campo.text(), self.filtro)
        else:
            r, _ = QFileDialog.getOpenFileName(self, "Abrir", self.campo.text(), self.filtro)
        if r:
            self.campo.setText(r)

    def texto(self) -> str:
        return self.campo.text().strip()


class _Senales(QObject):
    listo = Signal(object)
    error = Signal(str)


class Tarea(QRunnable):
    """Ejecuta ``fn(*args)`` en segundo plano y emite ``listo``/``error``."""

    def __init__(self, fn: Callable[..., Any], *args: Any) -> None:
        super().__init__()
        self.fn, self.args = fn, args
        self.senales = _Senales()

    def run(self) -> None:
        try:
            r = self.fn(*self.args)
        except Exception as exc:  # noqa: BLE001 - se muestra al usuario
            self.senales.error.emit(f"{type(exc).__name__}: {exc}")
        else:
            self.senales.listo.emit(r)


class DelegadoLista(QStyledItemDelegate):
    """Editor de celda con lista desplegable; ``opciones()`` se evalúa al abrir."""

    def __init__(self, opciones: Callable[[], list[str]], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.opciones = opciones

    def createEditor(self, parent: QWidget, option: Any, index: QModelIndex) -> QWidget:  # noqa: N802
        cb = QComboBox(parent)
        cb.addItems(self.opciones())
        return cb

    def setEditorData(self, editor: QWidget, index: QModelIndex) -> None:  # noqa: N802
        assert isinstance(editor, QComboBox)
        editor.setCurrentText(str(index.data(Qt.ItemDataRole.EditRole) or ""))

    def setModelData(self, editor: QWidget, model: Any, index: QModelIndex) -> None:  # noqa: N802
        assert isinstance(editor, QComboBox)
        model.setData(index, editor.currentText(), Qt.ItemDataRole.EditRole)


class TablaEditable(QWidget):
    """Tabla editable con botones para agregar/quitar filas e importar/exportar CSV.

    ``al_cambiar(df)`` recibe el DataFrame completo tras cada cambio.
    """

    def __init__(self, al_cambiar: Callable[[pd.DataFrame], None], filas: bool = True,
                 csv: bool = False) -> None:
        super().__init__()
        self.al_cambiar = al_cambiar
        self.vista = QTableView()
        self.modelo = ModeloTabla(editable=True, al_editar=lambda *_: self._avisar())
        self.vista.setModel(self.modelo)
        self.vista.setAlternatingRowColors(True)
        self.vista.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.ResizeToContents)
        barra = QHBoxLayout()
        if filas:
            for texto, fn in (("+ Fila", self.agregar_fila), ("− Fila", self.quitar_fila)):
                b = QPushButton(texto)
                b.clicked.connect(fn)
                barra.addWidget(b)
        if csv:
            for texto, fn in (("Importar CSV…", self._importar), ("Exportar CSV…", self._exportar)):
                b = QPushButton(texto)
                b.clicked.connect(fn)
                barra.addWidget(b)
        barra.addStretch()
        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.addLayout(barra)
        lay.addWidget(self.vista)

    def poner(self, df: pd.DataFrame) -> None:
        self.modelo.poner(df.copy())

    def df(self) -> pd.DataFrame:
        return self.modelo.df.copy()

    def _avisar(self) -> None:
        self.al_cambiar(self.df())

    def agregar_fila(self) -> None:
        df = self.modelo.df
        fila = {c: (float("nan") if pd.api.types.is_numeric_dtype(df[c]) else "") for c in df.columns}
        self.modelo.poner(pd.concat([df, pd.DataFrame([fila])], ignore_index=True))
        self._avisar()

    def quitar_fila(self) -> None:
        filas = sorted({i.row() for i in self.vista.selectionModel().selectedIndexes()})
        if not filas and len(self.modelo.df):
            filas = [len(self.modelo.df) - 1]
        self.modelo.poner(self.modelo.df.drop(index=filas).reset_index(drop=True))
        self._avisar()

    def _importar(self) -> None:
        r, _ = QFileDialog.getOpenFileName(self, "Importar CSV", "", "CSV (*.csv)")
        if r:
            self.modelo.poner(pd.read_csv(r, comment="#", skipinitialspace=True))
            self._avisar()

    def _exportar(self) -> None:
        r, _ = QFileDialog.getSaveFileName(self, "Exportar CSV", "", "CSV (*.csv)")
        if r:
            self.modelo.df.to_csv(r, index=False)
