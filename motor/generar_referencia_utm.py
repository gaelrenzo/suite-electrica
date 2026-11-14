"""Referencia UTM con pyproj (Python) -> motor/tests/utm.txt: lon lat zona este norte."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from suite_electrica.core.geo.utm import geograficas_a_utm  # noqa: E402

L = []
for zona, lon0 in (("17S", -81.0), ("18S", -75.0), ("19S", -69.0)):
    for dl in np.linspace(-3.4, 3.4, 9):
        for lat in np.linspace(-18.5, -0.2, 9):
            e, n = geograficas_a_utm([lon0 + dl], [lat], zona)
            L.append(f"{lon0 + dl:.12f} {lat:.12f} {zona} {e[0]:.6f} {n[0]:.6f}")
(Path(__file__).parent / "tests" / "utm.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
print(len(L), "puntos")
