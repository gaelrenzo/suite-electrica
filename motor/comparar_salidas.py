"""Compara las salidas de Rust (motor/target/tmp/salidas_rust) con las de Python (tests/salidas_python).

Complementa ``cargo test --test salidas`` (que compara Excel y DXF celda a celda y entidad a
entidad) con lo que solo se puede verificar desde Python:

* DXF de Rust: ezdxf los abre y su auditoría no encuentra errores.
* Excel de Rust: openpyxl los abre; mismos nombres de hoja y encabezados.
* PDF: mismo tamaño de página; toda palabra o número de la versión Python aparece en la de
  Rust, salvo la fecha de generación. Se compara como conjunto: la paginación es distinta y
  reportlab repite el encabezado de las tablas cortadas entre páginas. pypdf a veces pega textos
  vecinos de matplotlib en un solo token: se acepta si se parte en palabras de Rust.

Uso: ``cargo test --test salidas`` y luego ``python motor/comparar_salidas.py``.
"""

import re
import sys
from collections import Counter
from pathlib import Path

import ezdxf
import openpyxl
from pypdf import PdfReader

AQUI = Path(__file__).parent
PY = AQUI / "tests" / "salidas_python"
RS = AQUI / "target" / "tmp" / "salidas_rust"
FECHA = re.compile(r"\d{4}-\d{2}-\d{2}\.?")


def palabras(ruta: Path) -> tuple[Counter, list[tuple[float, float]]]:
    r = PdfReader(str(ruta))
    texto = " ".join(p.extract_text() or "" for p in r.pages)
    tok = [t for t in texto.split() if not FECHA.fullmatch(t)]
    return Counter(tok), [(round(float(p.mediabox.width), 1), round(float(p.mediabox.height), 1)) for p in r.pages]


def partible(t: str, vocab: set[str]) -> bool:
    """True si ``t`` es una concatenación de palabras de ``vocab``."""
    ok = [True] + [False] * len(t)
    for i in range(1, len(t) + 1):
        ok[i] = any(ok[j] and t[j:i] in vocab for j in range(i))
    return ok[-1]


def main() -> int:
    fallas = 0
    for f in sorted(RS.glob("*.dxf")):
        doc = ezdxf.readfile(f)
        aud = doc.audit()
        n = len(doc.modelspace())
        print(f"DXF  {f.name}: {n} entidades, versión {doc.dxfversion}, auditoría: "
              f"{len(aud.errors)} errores, {len(aud.fixes)} correcciones")
        fallas += bool(aud.has_errors)
    for f in sorted(RS.glob("*.xlsx")):
        a, b = openpyxl.load_workbook(f, read_only=True), openpyxl.load_workbook(PY / f.name, read_only=True)
        igual = a.sheetnames == b.sheetnames and all(
            [c.value for c in next(a[h].iter_rows(max_row=1))] == [c.value for c in next(b[h].iter_rows(max_row=1))]
            for h in a.sheetnames)
        print(f"XLSX {f.name}: hojas {a.sheetnames} {'= Python' if igual else '≠ PYTHON'}")
        fallas += not igual
    for f in sorted(RS.glob("*.pdf")):
        (tr, pr), (tp, pp) = palabras(f), palabras(PY / f.name)
        faltan = Counter({k: v for k, v in tp.items() if k not in tr and not partible(k, set(tr))})
        extra = sum((tr - tp).values())
        perfil = f.name.startswith("topo_")
        if perfil:  # escalas de los ejes: matplotlib elige sus marcas; no forman parte del contenido
            faltan = Counter({k: v for k, v in faltan.items() if not re.fullmatch(r"-?\d+(\.0)?", k)})
        print(f"PDF  {f.name}: {len(pr)} pág. {pr[0]} (Python {len(pp)} pág. {pp[0]} mm→pt); "
              f"{sum(tp.values())} palabras de Python, faltan {sum(faltan.values())}; {extra} adicionales en Rust")
        if faltan:
            print("     faltan:", dict(list(faltan.items())[:20]))
        ok = not faltan and pr[0] == pp[0] and (perfil or len(pr) >= 1)
        fallas += not ok
    print("RESULTADO:", "OK" if not fallas else f"{fallas} diferencias")
    return 1 if fallas else 0


if __name__ == "__main__":
    sys.exit(main())
