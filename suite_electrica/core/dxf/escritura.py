"""Utilidades para crear documentos DXF compatibles con AutoCAD y CADCraft."""

from __future__ import annotations

from pathlib import Path

import ezdxf
from ezdxf import units
from ezdxf.document import Drawing


def nuevo_documento() -> Drawing:
    """Documento DXF R2010 con unidades en metros y estilos estándar."""
    doc = ezdxf.new("R2010", setup=True)
    doc.units = units.M
    doc.header["$MEASUREMENT"] = 1  # métrico
    return doc


def asegurar_capa(doc: Drawing, nombre: str, color: int = 7) -> None:
    """Crea la capa si no existe (color ACI)."""
    if nombre not in doc.layers:
        doc.layers.add(nombre, color=color)


def guardar(doc: Drawing, ruta: str | Path) -> Path:
    """Audita y guarda el documento; lanza RuntimeError si la auditoría falla."""
    auditor = doc.audit()
    if auditor.has_errors:
        raise RuntimeError(f"DXF inválido: {[e.message for e in auditor.errors]}")
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    doc.saveas(ruta)
    return ruta
