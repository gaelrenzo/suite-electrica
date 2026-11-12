"""Genera los terrenos sintéticos de prueba (cotas exactas conocidas) y el eje.

Uso:  python examples/generar_ejemplos.py
"""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from suite_electrica.core.dxf.escritura import asegurar_capa, guardar, nuevo_documento  # noqa: E402
from suite_electrica.topocraft import datos, sinteticos as st  # noqa: E402

AQUI = Path(__file__).parent


def main() -> None:
    for sup in (st.plano_inclinado(), st.cono(), st.colina_gaussiana()):
        ruta = AQUI / f"{sup.nombre}.csv"
        datos.guardar_csv(st.malla(sup, paso=10.0), ruta)
        print(ruta)
    doc = nuevo_documento()
    asegurar_capa(doc, "EJE", 1)
    doc.modelspace().add_lwpolyline(st.eje_ejemplo().tolist(), dxfattribs={"layer": "EJE"})
    print(guardar(doc, AQUI / "eje.dxf"))


if __name__ == "__main__":
    main()
