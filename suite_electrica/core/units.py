"""Unidades y formatos. Todo el código trabaja en SI; aquí solo hay factores y formatos."""

from __future__ import annotations

M: float = 1.0
"""Metro (unidad base de longitud)."""
CM: float = 0.01
MM: float = 0.001
KM: float = 1000.0


def formato_progresiva(s_m: float, decimales: int = 2) -> str:
    """Formatea una progresiva en metros como ``km+mmm.dd`` (p. ej. 1250.5 m -> ``1+250.50``).

    Args:
        s_m: progresiva en metros (>= 0).
        decimales: decimales de la parte en metros.
    """
    if s_m < 0:
        raise ValueError(f"Progresiva negativa: {s_m}")
    s = round(s_m, decimales)
    km = int(s // 1000)
    resto = round(s - km * 1000, decimales)
    if resto >= 1000:
        km += 1
        resto = round(resto - 1000, decimales)
    ancho = 3 + (decimales + 1 if decimales > 0 else 0)
    return f"{km}+{resto:0{ancho}.{decimales}f}"
