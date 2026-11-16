"""Plantillas compartidas con la versión Rust: ``data/plantillas`` en la raíz del repositorio."""

from pathlib import Path

DIR = Path(__file__).resolve().parents[1] / "data" / "plantillas"


def plantillas() -> Path:
    """Carpeta de plantillas (CSV y JSON)."""
    return DIR
