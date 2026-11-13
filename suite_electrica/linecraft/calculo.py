"""Cálculo completo de una línea: estructuras, tramos de tensado, vanos y cargas.

Hipótesis simplificadoras (documentadas en ``docs/linecraft.md``):

* La ecuación de cambio de estado se aplica al vano regulador de cada tramo.
* La flecha vertical es la flecha en el plano de la carga resultante por
  ``w_vertical / w_resultante``.
* Cargas en estructura aplicadas a la altura de amarre; no incluyen viento sobre
  el poste ni el armado (requieren catálogo de estructuras, fase posterior).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

import numpy as np
import pandas as pd

from suite_electrica.linecraft.conductor import carga_unitaria
from suite_electrica.linecraft.estructuras import (
    ANCLAJE, FIN, Estructura, crear_estructuras, despeje_vano, ubicar_automatico,
)
from suite_electrica.linecraft.mecanica import cambio_estado, curva, deformacion_creep, vano_regulador
from suite_electrica.linecraft.proyecto import Proyecto


@dataclass
class Estado:
    """Estado del conductor en un tramo para una hipótesis (por conductor)."""

    hipotesis: str
    temperatura: float
    w_vertical: float
    w_viento: float
    w_resultante: float
    H: float
    """Tiro horizontal (N)."""
    tiro_maximo: float
    """Tiro máximo en soportes del tramo (N)."""
    pct_rotura: float
    cumple: bool


@dataclass
class Tramo:
    """Tramo de tensado entre estructuras de anclaje/fin (índices inclusive)."""

    inicio: int
    fin: int
    vano_regulador: float
    estados: dict[str, Estado] = field(default_factory=dict)


@dataclass
class Vano:
    """Vano entre las estructuras ``a`` y ``a + 1``."""

    a: int
    tramo: int
    longitud: float
    desnivel: float
    flechas: dict[str, float]
    """Flecha vertical por hipótesis (m)."""
    despeje: float
    """Despeje mínimo al terreno en la hipótesis de flecha máxima (m)."""
    s_despeje: float
    cumple: bool
    flecha_medio: float = 0.0
    """Flecha vertical a medio vano en la hipótesis de flecha máxima (m); criterio de REDLIN."""


@dataclass
class Cargas:
    """Cargas en una estructura para una hipótesis (todos los conductores, N)."""

    estructura: int
    hipotesis: str
    vano_viento: float
    vano_peso: float
    transversal: float
    longitudinal: float
    vertical: float
    horizontal: float
    momento_base: float
    """Momento a nivel del terreno (N·m) con la carga horizontal en el amarre."""
    tiro_retenida: float
    """Tiro en la retenida (N); 0 si la estructura no la lleva."""


@dataclass
class Resultado:
    """Resultado completo de LineCraft."""

    proyecto: Proyecto
    estructuras: list[Estructura]
    tramos: list[Tramo]
    vanos: list[Vano]
    cargas: list[Cargas]
    avisos: list[str]
    verificacion: pd.DataFrame | None = None
    """Verificación de postes y retenidas (si hay catálogos)."""
    metrado: pd.DataFrame | None = None
    alertas: pd.DataFrame | None = None
    """Alertas con exceso sobre el permisible (``linecraft.alertas``)."""

    def tramo_de_vano(self, i: int) -> Tramo:
        return self.tramos[self.vanos[i].tramo]

    @property
    def cumple(self) -> bool:
        """True si vanos, tramos y (si existe) la verificación de estructuras cumplen."""
        ok = all(v.cumple for v in self.vanos) and all(
            e.cumple for t in self.tramos for e in t.estados.values())
        if self.verificacion is not None and len(self.verificacion):
            ok = ok and bool((self.verificacion["Cumple"] != "NO").all())
        return ok and (self.alertas is None or self.alertas.empty)


def _estados_tramo(p: Proyecto, a_r: float, desnivel: float = 0.0, z: float | None = None,
                   propio: tuple[float, float] | None = None) -> dict[str, tuple[float, object]]:
    """H (N) y carga unitaria por hipótesis.

    Estado inicial (EDS) con el vano regulador ``a_r`` (``desnivel``: tramo de un vano). Los
    estados finales usan el mismo vano, o ``propio = (vano, desnivel)`` si los vanos trabajan
    independientes. ``z``: cota media (msnm) para elegir las hipótesis por altitud.
    """
    c = p.conductor
    hips = {h.nombre: h for h in p.hipotesis_en(z)}
    h1 = hips[p.hip_inicial]
    w1 = carga_unitaria(c, h1).resultante
    sigma1 = p.tiro_inicial_pct / 100 * c.carga_rotura / c.seccion
    dt = 0.0  # temperatura equivalente del alargamiento por creep (estados finales)
    if p.condicion_final == "creep":
        dt = deformacion_creep(c, sigma1, w1, h1.temperatura, a_r, desnivel) / c.coef_dilatacion
    a2, h2 = propio or (a_r, desnivel)
    res = {}
    for h in hips.values():
        cu = carga_unitaria(c, h)
        s2 = sigma1 if h.nombre == p.hip_inicial else cambio_estado(
            sigma1, w1, h1.temperatura, cu.resultante, h.temperatura + dt, a2, c, h2)
        res[h.nombre] = (s2 * c.seccion, cu)
    return res


def _necesita_retenida(e: Estructura) -> bool:
    return e.tipo in (FIN, ANCLAJE) or abs(e.angulo) > 0.01


def calcular(p: Proyecto) -> Resultado:
    """Ejecuta el cálculo completo de la línea."""
    avisos = p.avisos_fuentes()
    c = p.conductor

    # 1. Estructuras
    if p.estructuras_manual is not None:
        m = p.estructuras_manual
        alturas = (m["altura_amarre_m"].fillna(p.altura_amarre).tolist()
                   if "altura_amarre_m" in m else p.altura_amarre)
        tipos = ([t.strip().upper() if isinstance(t, str) and t.strip() else None for t in m["tipo"]]
                 if "tipo" in m else None)
        def texto(col: str) -> list[str] | None:
            if col not in m:
                return None
            return [v.strip() if isinstance(v, str) else "" for v in m[col]]
        est = crear_estructuras(p.perfil, p.eje, m["progresiva_m"].astype(float).tolist(), alturas, tipos,
                                texto("armado"), texto("poste"))
    else:
        a_r = 0.8 * p.vano_maximo
        hf = p.hip(p.hip_flecha_maxima)
        cu_f = carga_unitaria(c, hf)
        for _ in range(8):
            H = _estados_tramo(p, a_r)[hf.nombre][0]
            prog, av = ubicar_automatico(
                p.perfil, p.eje, p.altura_amarre, p.vano_maximo, p.vano_minimo,
                p.distancia_minima_terreno, H, cu_f.resultante, p.modelo,
                factor_vertical=cu_f.vertical / cu_f.resultante)
            nuevo = vano_regulador(np.diff(prog))
            if abs(nuevo - a_r) < 0.5:
                break
            a_r = nuevo
        avisos += av
        est = crear_estructuras(p.perfil, p.eje, prog, p.altura_amarre)

    # 2. Tramos de tensado
    cortes = [i for i, e in enumerate(est) if e.tipo in (FIN, ANCLAJE) or i in (0, len(est) - 1)]
    tramos: list[Tramo] = []
    vanos: list[Vano] = []
    estados_vano: list[dict[str, tuple[float, object]]] = []
    for k in range(len(cortes) - 1):
        i0, i1 = cortes[k], cortes[k + 1]
        longitudes = [est[i + 1].progresiva - est[i].progresiva for i in range(i0, i1)]
        a_r = vano_regulador(longitudes)
        # tramo de un solo vano: cambio de estado exacto con su desnivel de amarres
        h_unico = est[i1].cota_amarre - est[i0].cota_amarre if i1 - i0 == 1 else 0.0
        z = float(np.mean([e.cota_terreno for e in est[i0:i1 + 1]]))
        if p.vanos_independientes and i1 - i0 > 1:
            hs_v = [_estados_tramo(p, a_r, 0.0, z, (est[i + 1].progresiva - est[i].progresiva,
                                                    est[i + 1].cota_amarre - est[i].cota_amarre))
                    for i in range(i0, i1)]
        else:
            hs_v = [_estados_tramo(p, a_r, h_unico, z)] * (i1 - i0)
        hs = hs_v[0]
        temps = {h.nombre: h.temperatura for h in p.hipotesis_en(z)}
        t = Tramo(i0, i1, a_r)
        for nombre in hs:
            tmax, H = max((curva(p.modelo, est[i + 1].progresiva - est[i].progresiva,
                                 est[i + 1].cota_amarre - est[i].cota_amarre, hv[nombre][0],
                                 hv[nombre][1].resultante).tiro_maximo, hv[nombre][0])
                          for i, hv in zip(range(i0, i1), hs_v))
            cu = hs[nombre][1]
            pct = 100 * tmax / c.carga_rotura
            t.estados[nombre] = Estado(nombre, temps[nombre], cu.vertical, cu.viento,
                                       cu.resultante, H, tmax, pct, pct <= p.tiro_maximo_pct + 1e-9)
            if pct > p.tiro_maximo_pct + 1e-9:
                avisos.append(f"Tramo {est[i0].codigo}-{est[i1].codigo}, {nombre}: tiro {pct:.1f} % "
                              f"> {p.tiro_maximo_pct:g} % de rotura")
        tramos.append(t)

        # 3. Vanos
        for i, hs in zip(range(i0, i1), hs_v):
            ea, eb = est[i], est[i + 1]
            a, h = eb.progresiva - ea.progresiva, eb.cota_amarre - ea.cota_amarre
            flechas = {n: curva(p.modelo, a, h, H, cu.resultante).flecha * cu.vertical / cu.resultante
                       for n, (H, cu) in hs.items()}
            Hf, cuf = hs[p.hip_flecha_maxima]
            d, sd = despeje_vano(p.perfil, ea.progresiva, eb.progresiva, ea.cota_amarre, eb.cota_amarre,
                                 Hf, cuf.resultante, p.modelo, cuf.vertical / cuf.resultante)
            ok = d >= p.distancia_minima_terreno - 1e-9
            if not ok:
                avisos.append(f"Vano {ea.codigo}-{eb.codigo}: despeje {d:.2f} m < "
                              f"{p.distancia_minima_terreno:.2f} m en {sd:.1f} m")
            cf = curva(p.modelo, a, h, Hf, cuf.resultante)
            f_medio = (h / 2 - float(cf.y(a / 2))) * cuf.vertical / cuf.resultante
            vanos.append(Vano(i, len(tramos) - 1, a, h, flechas, d, sd, ok, f_medio))
            estados_vano.append(hs)

    # 4. Cargas en estructuras
    cargas: list[Cargas] = []
    n = p.n_conductores
    phi = math.radians(p.angulo_retenida)
    for i, e in enumerate(est):
        arranque: list[str] = []
        atras = vanos[i - 1] if i > 0 else None
        adelante = vanos[i] if i < len(vanos) else None
        mitad = math.radians(abs(e.angulo)) / 2
        # hipótesis y cargas unitarias del vano vecino (ya elegidas por la altitud de su tramo)
        ref = estados_vano[(adelante or atras).a]
        for nombre, (_, cu) in ref.items():
            av = ap = 0.0
            Hb = Ha = 0.0
            for v, lado in ((atras, "b"), (adelante, "a")):
                if v is None:
                    continue
                H, cuv = estados_vano[v.a][nombre]
                xm = curva(p.modelo, v.longitud, v.desnivel, H, cuv.vertical).x_vertice
                av += v.longitud / 2
                if lado == "b":
                    Hb, ap = H, ap + (v.longitud - xm)
                else:
                    Ha, ap = H, ap + xm
            trans = n * ((Hb + Ha) * math.sin(mitad) + cu.viento * av * math.cos(mitad))
            longi = n * abs(Ha - Hb) * math.cos(mitad)
            vert = n * cu.vertical * ap
            fh = math.hypot(trans, longi)
            ret = fh / math.cos(phi) if _necesita_retenida(e) else 0.0
            cargas.append(Cargas(e.numero, nombre, av, ap, trans, longi, vert, fh,
                                 fh * e.altura_amarre, ret))
            if ap < 0:
                arranque.append(f"{nombre} ({ap:.1f} m)")
        if arranque:
            avisos.append(f"{e.codigo}: vano peso negativo (arrancamiento) en " + ", ".join(arranque))
    return Resultado(p, est, tramos, vanos, cargas, avisos)
