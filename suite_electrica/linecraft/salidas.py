"""Salidas de LineCraft: perfil DXF, planilla Excel y memoria de cálculo PDF."""

from __future__ import annotations

import io
from datetime import date
from pathlib import Path

import numpy as np
import pandas as pd
from matplotlib.figure import Figure

from suite_electrica.core.dxf.escritura import asegurar_capa, guardar, nuevo_documento
from suite_electrica.core.units import formato_progresiva
from suite_electrica.linecraft.calculo import Resultado
from suite_electrica.linecraft.conductor import G
from suite_electrica.linecraft.estructuras import linea_conductor
from suite_electrica.topocraft.salidas import _tramos_validos, cota_referencia

CAPAS = {"PERFIL": 3, "POSTES": 4, "CONDUCTOR": 1, "DESPEJE": 6, "TEXTO_LC": 7}


def _curvas_vanos(r: Resultado, desplazar: float = 0.0) -> list[tuple[np.ndarray, np.ndarray]]:
    """Polilíneas (s, z) del conductor en la hipótesis de flecha máxima."""
    p = r.proyecto
    out = []
    for k, v in enumerate(r.vanos):
        ea, eb = r.estructuras[v.a], r.estructuras[v.a + 1]
        e = r.tramo_de_vano(k).estados[p.hip_flecha_maxima]
        s, zc, _ = linea_conductor(p.perfil, ea.progresiva, eb.progresiva, ea.cota_amarre,
                                   eb.cota_amarre, e.H, e.w_resultante, p.modelo,
                                   e.w_vertical / e.w_resultante, paso=2.0)
        out.append((s, zc - desplazar))
    return out


def perfil_dxf(ruta: str | Path, r: Resultado, exageracion: float = 10.0) -> Path:
    """Perfil con terreno, postes, conductor (flecha máxima) y línea de despeje mínimo.

    Coordenadas: x = progresiva (m), y = (cota − cota_ref) × exageración.
    """
    p = r.proyecto
    doc = nuevo_documento()
    msp = doc.modelspace()
    for k, col in CAPAS.items():
        asegurar_capa(doc, k, col)
    ref = cota_referencia(p.perfil)
    Y = lambda z: (np.asarray(z) - ref) * exageracion  # noqa: E731
    h = min(max(p.perfil.longitud / 250, 0.5), 5.0)
    for t in _tramos_validos(p.perfil.progresiva, p.perfil.cota):
        msp.add_lwpolyline(np.column_stack([t[:, 0], Y(t[:, 1])]).tolist(), dxfattribs={"layer": "PERFIL"})
    for e in r.estructuras:
        msp.add_line((e.progresiva, float(Y(e.cota_terreno))), (e.progresiva, float(Y(e.cota_amarre))),
                     dxfattribs={"layer": "POSTES"})
        msp.add_text(f"{e.codigo} {e.tipo}", height=h, rotation=90,
                     dxfattribs={"layer": "TEXTO_LC"}).set_placement(
            (e.progresiva - 0.5 * h, float(Y(e.cota_amarre)) + h))
        msp.add_text(formato_progresiva(e.progresiva), height=h, rotation=90,
                     dxfattribs={"layer": "TEXTO_LC"}).set_placement((e.progresiva + 0.4 * h, -8 * h))
    for capa, desp in (("CONDUCTOR", 0.0), ("DESPEJE", p.distancia_minima_terreno)):
        for s, z in _curvas_vanos(r, desp):
            msp.add_lwpolyline(np.column_stack([s, Y(z)]).tolist(), dxfattribs={"layer": capa})
    msp.add_text(f"COTA REF. {ref:.2f} m   EXAG. V {exageracion:g}x   CONDUCTOR: "
                 f"{p.hip_flecha_maxima}   DESPEJE MIN. {p.distancia_minima_terreno:.2f} m",
                 height=h, dxfattribs={"layer": "TEXTO_LC"}).set_placement((0, -11 * h))
    return guardar(doc, ruta)


def tabla_flechado(r: Resultado, amplitud: float = 20.0, paso: float = 5.0) -> pd.DataFrame:
    """Tabla de tendido (flechado): tiro horizontal y flecha a medio vano por temperatura.

    Condición de templado (EDS en la hipótesis inicial, sin viento ni creep), cambio de estado
    con el vano regulador de cada tramo (forma exacta en tramos de un vano con desnivel), como la
    *Tabla de Flechado* de REDLIN; temperaturas ``t_templado ± amplitud`` cada ``paso`` °C.
    """
    from suite_electrica.linecraft.conductor import Hipotesis, carga_unitaria
    from suite_electrica.linecraft.mecanica import cambio_estado, curva

    p, c = r.proyecto, r.proyecto.conductor
    filas = []
    for k, t in enumerate(r.tramos):
        est = r.estructuras
        z = float(np.mean([e.cota_terreno for e in est[t.inicio:t.fin + 1]]))
        h0 = p.hip(p.hip_inicial, z)
        w = carga_unitaria(c, Hipotesis("templado", h0.temperatura)).vertical
        s1 = p.tiro_inicial_pct / 100 * c.carga_rotura / c.seccion
        unico = t.fin - t.inicio == 1
        temps = np.arange(h0.temperatura - amplitud, h0.temperatura + amplitud + 1e-9, paso)
        for i in range(t.inicio, t.fin):
            v = r.vanos[i]
            fila = {"Tramo": f"{est[t.inicio].codigo}-{est[t.fin].codigo}", "Vano": f"{est[i].codigo}-{est[i + 1].codigo}",
                    "Vano regulador (m)": round(t.vano_regulador, 2), "Vano (m)": round(v.longitud, 2),
                    "Desnivel (m)": round(v.desnivel, 2), "EDS (%)": p.tiro_inicial_pct}
            for T in temps:
                H = cambio_estado(s1, w, h0.temperatura, w, float(T), t.vano_regulador, c,
                                  v.desnivel if unico else 0.0) * c.seccion
                cv = curva(p.modelo, v.longitud, v.desnivel, H, w)
                fila[f"H {T:g} °C (kgf)"] = round(H / G, 2)
                fila[f"Flecha {T:g} °C (m)"] = round(v.desnivel / 2 - float(cv.y(v.longitud / 2)), 3)
            filas.append(fila)
    return pd.DataFrame(filas)


def tablas(r: Resultado) -> dict[str, pd.DataFrame]:
    """Tablas de resultados (unidades en los encabezados)."""
    p = r.proyecto
    est = pd.DataFrame([{
        "Estructura": e.codigo, "Tipo": e.tipo, "Progresiva": formato_progresiva(e.progresiva),
        "Distancia (m)": round(e.progresiva, 2), "Este (m)": round(e.este, 3),
        "Norte (m)": round(e.norte, 3), "Cota terreno (m)": round(e.cota_terreno, 2),
        "Altura amarre (m)": round(e.altura_amarre, 2), "Cota amarre (m)": round(e.cota_amarre, 2),
        "Deflexión (°)": round(e.angulo, 2), "Armado": e.armado, "Poste": e.poste,
        "Vano atrás (m)": round(r.vanos[i - 1].longitud, 2) if i > 0 else None,
        "Vano adelante (m)": round(r.vanos[i].longitud, 2) if i < len(r.vanos) else None,
    } for i, e in enumerate(r.estructuras)])
    vanos = pd.DataFrame([{
        "Vano": f"{r.estructuras[v.a].codigo} - {r.estructuras[v.a + 1].codigo}",
        "Longitud (m)": round(v.longitud, 2), "Desnivel amarres (m)": round(v.desnivel, 2),
        **{f"Flecha {k} (m)": round(f, 3) for k, f in v.flechas.items()},
        f"Flecha medio vano {p.hip_flecha_maxima} (m)": round(v.flecha_medio, 3),
        "Despeje mín. (m)": round(v.despeje, 2), "En progresiva": formato_progresiva(v.s_despeje),
        "Exigido (m)": p.distancia_minima_terreno, "Cumple": "SÍ" if v.cumple else "NO",
    } for v in r.vanos])
    mec = pd.DataFrame([{
        "Tramo": f"{r.estructuras[t.inicio].codigo} - {r.estructuras[t.fin].codigo}",
        "Vano regulador (m)": round(t.vano_regulador, 2), "Hipótesis": e.hipotesis,
        "Temp. (°C)": e.temperatura, "w vertical (N/m)": round(e.w_vertical, 4),
        "w viento (N/m)": round(e.w_viento, 4), "w resultante (N/m)": round(e.w_resultante, 4),
        "Tiro horizontal (N)": round(e.H, 1), "Tiro horizontal (kgf)": round(e.H / G, 2),
        "Tiro máx. soporte (N)": round(e.tiro_maximo, 1), "% rotura": round(e.pct_rotura, 2),
        "Límite (%)": p.tiro_maximo_pct, "Cumple": "SÍ" if e.cumple else "NO",
    } for t in r.tramos for e in t.estados.values()])
    codigo = {e.numero: e.codigo for e in r.estructuras}
    cargas = pd.DataFrame([{
        "Estructura": codigo[c.estructura], "Hipótesis": c.hipotesis,
        "Vano viento (m)": round(c.vano_viento, 2), "Vano peso (m)": round(c.vano_peso, 2),
        "F transversal (N)": round(c.transversal, 1), "F longitudinal (N)": round(c.longitudinal, 1),
        "F vertical (N)": round(c.vertical, 1), "F horizontal (N)": round(c.horizontal, 1),
        "Momento base (N·m)": round(c.momento_base, 1), "Tiro retenida (N)": round(c.tiro_retenida, 1),
    } for c in r.cargas])
    avisos = pd.DataFrame({"Aviso": r.avisos or ["Sin avisos"]})
    res = {"Estructuras": est, "Vanos": vanos, "Mecanico": mec, "Cargas": cargas}
    if r.verificacion is not None:
        res["Verificacion"] = r.verificacion
    if r.metrado is not None:
        res["Metrado"] = r.metrado
    res["Flechado"] = tabla_flechado(r)
    if r.alertas is not None:
        res["Alertas"] = (r.alertas.assign(**{"Crítica": r.alertas["Crítica"].map({True: "SÍ", False: ""})})
                          if len(r.alertas) else pd.DataFrame({"Alerta": ["Sin alertas"]}))
    res["Avisos"] = avisos
    return res


def planilla_excel(ruta: str | Path, r: Resultado) -> Path:
    """Planilla de estructuras, vanos, cálculo mecánico, cargas y avisos."""
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    with pd.ExcelWriter(ruta, engine="openpyxl") as xw:
        for nombre, df in tablas(r).items():
            df.to_excel(xw, sheet_name=nombre, index=False)
    return ruta


def figura_perfil(r: Resultado, ancho_in: float = 10.0, alto_in: float = 3.5,
                  fig: Figure | None = None) -> Figure:
    """Figura del perfil con postes y conductor (memoria y GUI).

    Si se pasa ``fig`` se limpia y se dibuja sobre ella.
    """
    p = r.proyecto
    if fig is None:
        fig = Figure(figsize=(ancho_in, alto_in))
    else:
        fig.clear()
    ax = fig.add_subplot()
    for t in _tramos_validos(p.perfil.progresiva, p.perfil.cota):
        ax.fill_between(t[:, 0], np.nanmin(p.perfil.cota) - 5, t[:, 1], color="#d9c7a3", lw=0)
        ax.plot(t[:, 0], t[:, 1], color="#5a3e1b", lw=0.8)
    for k, ((s, z), (_, zd)) in enumerate(zip(_curvas_vanos(r),
                                             _curvas_vanos(r, p.distancia_minima_terreno))):
        ax.plot(s, z, color="#c0392b", lw=0.8, label=f"Conductor ({p.hip_flecha_maxima})" if k == 0 else None)
        ax.plot(s, zd, color="#8e44ad", lw=0.5, ls="--",
                label=f"Despeje mínimo {p.distancia_minima_terreno:.2f} m" if k == 0 else None)
    for e in r.estructuras:
        ax.plot([e.progresiva] * 2, [e.cota_terreno, e.cota_amarre], color="k", lw=1.2)
        ax.annotate(e.codigo, (e.progresiva, e.cota_amarre), fontsize=6, ha="center",
                    xytext=(0, 3), textcoords="offset points")
    ax.set_xlabel("Progresiva (m)", fontsize=7)
    ax.set_ylabel("Cota (m)", fontsize=7)
    ax.tick_params(labelsize=6)
    ax.grid(True, lw=0.3, color="0.85")
    ax.set_xlim(0, p.perfil.longitud)
    ax.legend(fontsize=6, loc="best")
    fig.tight_layout()
    return fig


def memoria_pdf(ruta: str | Path, r: Resultado) -> Path:
    """Memoria de cálculo en PDF (reportlab)."""
    from reportlab.lib import colors
    from reportlab.lib.pagesizes import A4, landscape
    from reportlab.lib.styles import getSampleStyleSheet
    from reportlab.lib.units import mm
    from reportlab.platypus import Image, Paragraph, SimpleDocTemplate, Spacer, Table, TableStyle

    p, c = r.proyecto, r.proyecto.conductor
    ruta = Path(ruta)
    ruta.parent.mkdir(parents=True, exist_ok=True)
    st = getSampleStyleSheet()
    pg = landscape(A4)
    doc = SimpleDocTemplate(str(ruta), pagesize=pg, leftMargin=12 * mm, rightMargin=12 * mm,
                            topMargin=12 * mm, bottomMargin=12 * mm,
                            title=f"Memoria de cálculo - {p.nombre}", author="LineCraft")
    ancho = pg[0] - 24 * mm

    def tabla(df: pd.DataFrame, fs: float = 6.5) -> Table:
        datos = [list(df.columns)] + [["" if pd.isna(x) else str(x) for x in fila]
                                      for fila in df.itertuples(index=False)]
        t = Table(datos, repeatRows=1)
        t.setStyle(TableStyle([
            ("FONTSIZE", (0, 0), (-1, -1), fs), ("GRID", (0, 0), (-1, -1), 0.25, colors.grey),
            ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#e8e8e8")),
            ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ]))
        return t

    H, B = st["Heading2"], st["BodyText"]
    B.fontSize = 8
    B.leading = 10
    tb = tablas(r)
    story = [
        Paragraph(f"Memoria de cálculo mecánico — {p.nombre}", st["Title"]),
        Paragraph(f"Generado con LineCraft (Suite Eléctrica) el {date.today().isoformat()}. "
                  f"Modelo de curva: {p.modelo}. Conductores por circuito: {p.n_conductores}.", B),
        Paragraph("Resultado global: " + ("<b>CUMPLE</b>" if r.cumple else "<b>NO CUMPLE</b>")
                  + f" — {len(r.avisos)} aviso(s).", B),
        Paragraph("1. Datos del conductor", H),
        tabla(pd.DataFrame([
            ["Conductor", c.nombre], ["Sección (mm²)", f"{c.seccion * 1e6:.2f}"],
            ["Diámetro (mm)", f"{c.diametro * 1e3:.2f}"], ["Peso (N/m)", f"{c.peso:.4f}"],
            ["Tiro de rotura (N)", f"{c.carga_rotura:.0f}"],
            ["Módulo de elasticidad (MPa)", f"{c.modulo_elasticidad / 1e6:.0f}"],
            ["Coef. dilatación (1/°C)", f"{c.coef_dilatacion:.2e}"], ["Fuente", c.fuente or "—"],
        ], columns=["Parámetro", "Valor"]), 7.5),
        Paragraph("2. Parámetros de diseño y fuentes", H),
        tabla(pd.DataFrame([
            ["Condición inicial", f"{p.hip_inicial}: {p.tiro_inicial_pct:g} % rotura",
             p.fuentes.get("tiro_inicial_pct_rotura", "—")],
            ["Tiro máximo admisible", f"{p.tiro_maximo_pct:g} % rotura",
             p.fuentes.get("tiro_maximo_pct_rotura", "—")],
            ["Distancia mínima al terreno", f"{p.distancia_minima_terreno:.2f} m",
             p.fuentes.get("distancia_minima_terreno_m", "—")],
            ["Hipótesis de flecha máxima", p.hip_flecha_maxima, p.fuentes.get("hipotesis", "—")],
            ["Altura de amarre", f"{p.altura_amarre:.2f} m", "Diseño"],
            ["Vano máximo / mínimo", f"{p.vano_maximo:g} / {p.vano_minimo:g} m", "Diseño"],
            ["Ángulo de retenida", f"{p.angulo_retenida:g}° con la horizontal",
             p.fuentes.get("angulo_retenida_deg", "Diseño")],
        ], columns=["Parámetro", "Valor", "Fuente"]), 7.5),
        Spacer(1, 3 * mm),
        tabla(pd.DataFrame([[h.nombre, h.temperatura, h.presion_viento, h.espesor_hielo * 1e3,
                             h.densidad_hielo,
                             "todas" if h.altitud_min == -float("inf") and h.altitud_max == float("inf")
                             else f"{max(h.altitud_min, 0):g}–{h.altitud_max:g}"] for h in p.hipotesis],
                           columns=["Hipótesis", "Temp. (°C)", "Viento (Pa)", "Hielo (mm)",
                                    "Dens. hielo (kg/m³)", "Altitud (msnm)"]), 7.5),
        Paragraph("3. Fórmulas", H),
        Paragraph("Cambio de estado: σ2²·[σ2 − σ1 + a²·w1²·E/(24·A²·σ1²) + α·E·(t2 − t1)] = "
                  "a²·w2²·E/(24·A²), con a = vano regulador √(Σa³/Σa).", B),
        Paragraph("Catenaria: C = H/w; y(x) = C·[cosh((x − xm)/C) − cosh(xm/C)]; "
                  "xm = a/2 − C·asinh(h/(2C·sinh(a/2C))). Parábola: f = w·a²/(8H).", B),
        Paragraph("Cargas unitarias: hielo ρ·g·π·e·(d + e); viento p·(d + 2e). Flecha vertical = "
                  "flecha resultante × w_vertical/w_resultante.", B),
        Paragraph("Cargas en estructura (n conductores): transversal n·[(H_atrás + H_adelante)·sen(θ/2) + "
                  "w_viento·a_viento·cos(θ/2)]; longitudinal n·|H_adelante − H_atrás|·cos(θ/2); "
                  "retenida F_h/cos(φ). Detalle y fuentes en docs/linecraft.md.", B),
        Paragraph("4. Perfil", H),
    ]
    buf = io.BytesIO()
    figura_perfil(r).savefig(buf, format="png", dpi=150)
    buf.seek(0)
    story.append(Image(buf, width=ancho, height=ancho * 0.35))
    titulos = {"Estructuras": "Estructuras", "Mecanico": "Cálculo mecánico por tramo",
               "Vanos": "Vanos, flechas y distancias de seguridad", "Cargas": "Cargas en estructuras",
               "Verificacion": "Verificación de postes y retenidas", "Metrado": "Metrado",
               "Alertas": "Alertas de diseño (exceso sobre el permisible)",
               "Flechado": "Tabla de flechado (tendido, sin viento)", "Avisos": "Avisos"}
    orden = ["Estructuras", "Mecanico", "Vanos", "Cargas", "Verificacion", "Alertas", "Flechado", "Metrado",
             "Avisos"]
    for k, clave in enumerate([c for c in orden if c in tb], start=5):
        story += [Paragraph(f"{k}. {titulos[clave]}", H), tabla(tb[clave])]
    doc.build(story)
    return ruta
