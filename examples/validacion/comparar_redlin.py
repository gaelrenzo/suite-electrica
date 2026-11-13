"""Compara la catenaria de LineCraft con la "Planilla de Estructuras" exportada por REDLIN.

Uso (requiere REDLIN para generar el .xls; no se distribuyen datos de REDLIN)::

    REDLIN: Reportes > Reportar todas las derivaciones > Planilla de Estructuras -> PlanillaEstTotal.xls
    python examples/validacion/comparar_redlin.py PlanillaEstTotal.xls

Con el parámetro de catenaria C, el vano y el desnivel de amarres (despejado de la
longitud de catenaria que reporta REDLIN) recalcula flecha a medio vano, longitud,
vano peso y vano viento, e imprime el error máximo de cada uno.
"""

import math
import sys
from pathlib import Path

import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from suite_electrica.linecraft.mecanica import Catenaria  # noqa: E402

COLS = {1: "der", 2: "n", 10: "desn", 14: "vad", 18: "vp", 19: "vv", 50: "C", 51: "L", 52: "f"}


def leer(ruta: str) -> pd.DataFrame:
    crudo = pd.read_excel(ruta, header=None).iloc[2:]
    d = pd.DataFrame({k: crudo[i] for i, k in COLS.items()})
    for c in d.columns[1:]:
        d[c] = pd.to_numeric(d[c], errors="coerce")
    return d


def comparar(d: pd.DataFrame) -> pd.DataFrame:
    filas = []
    for der, g in d.groupby("der", sort=False):
        g = g.sort_values("n").reset_index(drop=True)
        ultimo = len(g) - 1
        h = []  # desnivel de amarres despejado de L = sqrt(h² + (2C sinh(a/2C))²)
        for i, r in g.iterrows():
            ok = i < ultimo and r.vad > 0 and r.C > 0
            s = 2 * r.C * math.sinh(r.vad / (2 * r.C)) if ok else 0
            h.append(math.copysign(math.sqrt(max(r.L**2 - s**2, 0)), r.desn) if ok else math.nan)
        for i, r in g.iterrows():
            f = {"der": der, "n": r.n, "vv_R": r.vv,
                 "vv_LC": ((g.vad[i - 1] if i else 0) + (r.vad if i < ultimo else 0)) / 2}
            if not math.isnan(h[i]):
                c = Catenaria(r.vad, h[i], r.C, 1.0)
                f.update(f_R=r.f, f_LC=h[i] / 2 - float(c.y(r.vad / 2)), L_R=r.L, L_LC=c.longitud)
                if i and not math.isnan(h[i - 1]):
                    ca = Catenaria(g.vad[i - 1], h[i - 1], g.C[i - 1], 1.0)
                    f.update(vp_R=r.vp, vp_LC=g.vad[i - 1] - ca.x_vertice + c.x_vertice)
            filas.append(f)
    return pd.DataFrame(filas)


def conductores(ruta_suministros: str) -> dict:
    """Conductores de la hoja Conductor de Suministros.XLS (kg, mm, kg/mm², 1/°C) en SI."""
    from suite_electrica.linecraft.conductor import G, Conductor
    c = pd.read_excel(ruta_suministros, "Conductor", header=None).iloc[4:]
    out = {}
    for r in c.itertuples(index=False):
        try:
            out[str(r[1])] = Conductor(str(r[1]), float(r[8]) * 1e-6, float(r[7]) * 1e-3, float(r[5]) * G,
                                       float(r[6]) * G, float(r[12]) * G * 1e6, float(r[11]),
                                       curva_inicial=tuple(float(v) for v in r[15:20]),
                                       curva_creep=tuple(float(v) for v in r[20:25]))
        except (TypeError, ValueError):
            pass
    return out


def comparar_flechado(ruta_tabla: str, ruta_suministros: str) -> pd.DataFrame:
    """Tiro H por temperatura de la Tabla de Flechado vs ``cambio_estado`` (desde 25 °C, sin viento).

    En tramos de un vano se pasa el desnivel (forma exacta); en tramos de varios vanos, el vano regulador.
    """
    from suite_electrica.linecraft.conductor import G
    from suite_electrica.linecraft.mecanica import cambio_estado
    cond = conductores(ruta_suministros)
    df = pd.read_excel(ruta_tabla, header=None)
    temps = [float(t) for t in df.iloc[1, 11:] if pd.notna(t)]
    t_ref = float(str(df.iat[0, 0]).split("Temperatura:")[1].split("°")[0].split("░")[0])
    filas = []
    for i in range(2, len(df) - 2):
        r = df.iloc[i]
        if not str(r[10]).startswith("Flecha") or r[8] not in cond:
            continue
        c, ar, a, h = cond[r[8]], float(r[4]), float(r[5]), float(r[6])
        H = {t: float(df.iat[i + 2, 11 + k]) * G for k, t in enumerate(temps)}
        for t in temps:
            hl = cambio_estado(H[t_ref] / c.seccion, c.peso, t_ref, c.peso, t, ar, c,
                               h if a == ar else 0.0) * c.seccion
            filas.append({"conductor": c.nombre, "a_r": ar, "T": t, "H_R": H[t] / G, "H_LC": hl / G})
    return pd.DataFrame(filas)


def hipotesis_por_altitud(ruta_config: str) -> list[tuple[float, float, dict]]:
    """[(z_min, z_max, {nombre: (temperatura, presión de viento Pa)})] de la hoja Hipotesis."""
    h = pd.read_excel(ruta_config, "Hipotesis", header=None)
    rangos = []
    for s in range(2, h.shape[1] - 11, 12):
        z0, z1 = pd.to_numeric(h.iat[1, s + 2], errors="coerce"), pd.to_numeric(h.iat[1, s + 3], errors="coerce")
        if pd.isna(z0):
            continue
        hips = {str(h.iat[i, 0]): (float(h.iat[i, s]), float(h.iat[i, s + 11]) * (float(h.iat[i, s + 8]) / 3.6) ** 2)
                for i in range(4, h.shape[0]) if pd.notna(h.iat[i, 0])}
        rangos.append((float(z0), float(z1), hips))
    return rangos


def comparar_final(ruta_planilla: str, ruta_tabla: str, ruta_suministros: str, ruta_config: str,
                   hip_templado: str = "Templado", hip_final: str = "Máximo Esfuerzo") -> pd.DataFrame:
    """C (= H/w) en la hipótesis final con creep: Planilla de Estructuras vs LineCraft.

    Estado inicial (EDS de la Tabla de Flechado) con el vano regulador del tramo; alargamiento
    por creep con ``deformacion_creep`` y cambio de estado a la hipótesis final del rango de altitud.
    """
    from suite_electrica.linecraft.conductor import Hipotesis, carga_unitaria
    from suite_electrica.linecraft.mecanica import cambio_estado, deformacion_creep
    cond, rangos = conductores(ruta_suministros), hipotesis_por_altitud(ruta_config)
    tf = pd.read_excel(ruta_tabla, header=None)
    tramos = {(str(r[1]), int(r[2])): r for r in (tf.iloc[i] for i in range(2, len(tf)))
              if str(r[10]).startswith("Flecha")}
    filas = []
    for r in pd.read_excel(ruta_planilla, header=None).iloc[2:].itertuples(index=False):
        t = tramos.get((str(r[1]), int(r[2]) + 1))
        Cr = pd.to_numeric(r[50], errors="coerce")
        if t is None or not Cr > 0 or t[8] not in cond:
            continue
        c, ar, eds, h, a, z = cond[t[8]], float(t[4]), float(t[9]), float(t[6]), float(t[5]), float(t[7])
        hips = next(hp for z0, z1, hp in rangos if z0 <= z < z1 or z >= rangos[-1][0])
        (t0, _), (t2, p2) = hips[hip_templado], next(v for k, v in hips.items() if k[:5] == hip_final[:5])
        w0, w2 = c.peso, carga_unitaria(c, Hipotesis("f", t2, p2)).resultante
        hh = h if abs(a - ar) < 0.01 else 0.0
        s1 = eds / 100 * c.carga_rotura / c.seccion
        dt = deformacion_creep(c, s1, w0, t0, ar, hh) / c.coef_dilatacion
        C_lc = cambio_estado(s1, w0, t0, w2, t2 + dt, ar, c, hh) * c.seccion / w2
        filas.append({"vanos_tramo": "1" if hh or abs(a - ar) < 0.01 else "varios", "z": z,
                      "C_R": Cr, "C_LC": C_lc})
    return pd.DataFrame(filas)


if __name__ == "__main__":
    if len(sys.argv) == 6:  # planilla tabla suministros configuracion --final
        f = comparar_final(*sys.argv[1:5])
        f["err"] = 100 * (f.C_LC / f.C_R - 1)
        print(f"Estado final con creep (C en máximo esfuerzo): {len(f)} vanos  "
              f"|error| mediana {f.err.abs().median():.2f} %")
        print(f.groupby("vanos_tramo")["err"].agg(
            lambda e: f"n={len(e)} mediana {e.abs().median():.2f} %  p90 {e.abs().quantile(.9):.2f} %").to_string())
        sys.exit()
    if len(sys.argv) == 4:  # comparar_redlin.py TablaFlechado.XLS Suministros.XLS --flechado
        f = comparar_flechado(sys.argv[1], sys.argv[2])
        e = 100 * (f.H_LC / f.H_R - 1)
        print(f"Tiro H vs temperatura: {len(f)} puntos  |error| mediana {e.abs().median():.3f} %  "
              f"p95 {e.abs().quantile(.95):.3f} %  máx {e.abs().max():.2f} %")
        for lim in (20, 50):
            m = f.a_r >= lim
            print(f"  vanos ≥ {lim} m: p95 {e[m].abs().quantile(.95):.3f} %  máx {e[m].abs().max():.3f} %")
        sys.exit()
    t = comparar(leer(sys.argv[1]))
    for nombre in ("vv", "f", "L", "vp"):
        m = t[[f"{nombre}_R", f"{nombre}_LC"]].dropna()
        e = (m.iloc[:, 1] - m.iloc[:, 0]).abs()
        print(f"{nombre:3s} n={len(m):4d}  error máx. {e.max():8.4f} m  mediana {e.median():.4f} m")
    malos = t[(t.vv_LC - t.vv_R).abs() > 0.01]
    if len(malos):
        print("Vano viento distinto (inicio de derivación: REDLIN excluye el vano de enlace):")
        print(malos[["der", "n", "vv_R", "vv_LC"]].to_string(index=False))
