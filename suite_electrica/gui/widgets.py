"""Componentes Qt reutilizables."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import pandas as pd
from matplotlib.backends.backend_qtagg import FigureCanvasQTAgg, NavigationToolbar2QT
from matplotlib.figure import Figure
from PySide6.QtCore import QAbstractTableModel, QModelIndex, QObject, QRunnable, Qt, Signal
from PySide6.QtWidgets import (
    QFileDialog, QHBoxLayout, QHeaderView, QLineEdit, QPushButton, QTableView, QVBoxLayout, QWidget,
)


class ModeloTabla(QAbstractTableModel):
    """Modelo de solo lectura sobre un DataFrame."""

    def __init__(self, df: pd.DataFrame | None = None) -> None:
        super().__init__()
        self.df = df if df is not None else pd.DataFrame()

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
        if role == Qt.ItemDataRole.DisplayRole:
            return "" if pd.isna(v) else str(v)
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
        lay.addWidget(NavigationToolbar2QT(self.canvas, self))
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
