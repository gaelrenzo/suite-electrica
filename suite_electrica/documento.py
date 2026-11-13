"""Documento de proyecto único (``.sel``): topografía, línea, catálogos y estructuras.

El archivo es JSON legible (UTF-8). Unidades: SI salvo donde el nombre del
campo indica otra (``_mm``, ``_kN``…), igual que en los catálogos.
"""

from __future__ import annotations

import copy
import json
import math
from dataclasses import asdict, dataclass, field, fields
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

from suite_electrica.core.dxf.lectura import leer_polilinea
from suite_electrica.core.geo.perfil import Perfil, perfil_longitudinal, progresivas_eje
from suite_electrica.core.geo.tin import TIN, Curva, niveles
from suite_electrica.linecraft import catalogos as cat_mod
from suite_electrica.linecraft.alertas import ConfigAlertas, alertas
from suite_electrica.linecraft.calculo import Resultado, calcular
from suite_electrica.linecraft.catalogos import Catalogos
from suite_electrica.linecraft.conductor import G, Conductor, Hipotesis, curva, leer_catalogo
from suite_electrica.linecraft.estructuras import TIPOS, TOL_VERTICE, angulos_eje
from suite_electrica.linecraft.proyecto import PARAMETROS_NORMATIVOS, Proyecto
from suite_electrica.topocraft import datos
from suite_electrica.topocraft.datos import ReporteLimpieza

FORMATO = "suite-electrica"
VERSION = 1
EXTENSION = ".sel"
COLS_EST = ["progresiva_m", "tipo", "altura_amarre_m", "armado", "poste", "retenidas"]
COLS_HIP = ["nombre", "temperatura_C", "presion_viento_Pa", "espesor_hielo_mm", "densidad_hielo_kg_m3",
            "altitud_min_m", "altitud_max_m"]
"""``altitud_min_m``/``altitud_max_m`` vacíos (o 0) = sin límite."""
FUENTES_EXTRA = ("factor_seguridad_poste", "factor_seguridad_retenida")
FUENTES_ALERTAS = ("sep_k_tension", "sep_k_flecha", "angulo_salida_min", "angulo_salida_max",
                   "tiro_max_espiga", "eds_max_pct")
"""Parámetros de alertas que, si se usan (≠ 0), deben citar su fuente en ``fuentes``."""
SEPARACION_MIN = 1.0
"""Separación mínima entre estructuras al editar (m)."""


@dataclass
class ParamTopo:
    """Parámetros topográficos (m)."""

    equidistancia: float = 1.0
    mayor_cada: int = 5
    paso_perfil: float = 20.0
    max_lado: float | None = None
    quitar_atipicos: bool = False


@dataclass
class ParamLinea:
    """Parámetros de diseño de la línea. Los normativos requieren fuente."""

    n_conductores: int = 3
    modelo: str = "catenaria"
    hip_inicial: str = "V_EDS"
    tiro_inicial_pct: float = 18.0
    hip_flecha_maxima: str = "IV_TEMP_MAX"
    tiro_maximo_pct: float = 40.0
    distancia_minima_terreno: float = 6.5
    altura_amarre: float = 10.6
    vano_maximo: float = 120.0
    vano_minimo: float = 30.0
    angulo_retenida: float = 45.0
    condicion_final: str = "creep"
    estado_final_vanos: str = "tramo"
    """``tramo`` (vano regulador) o ``por vano`` (vanos independientes, criterio de REDLIN)."""
    fs_poste: float = 2.0
    fs_retenida: float = 2.0
    retenida: str = ""
    desperdicio_pct: float = 0.0
    # Alertas tipo REDLIN; 0 = no verificar (ver linecraft.alertas).
    tension_kV: float = 0.0
    sep_k_tension: float = 0.0
    sep_k_flecha: float = 0.0
    angulo_salida_min: float = 0.0
    angulo_salida_max: float = 0.0
    tiro_max_espiga: float = 0.0
    eds_max_pct: float = 0.0


@dataclass
class Topografia:
    """Resultado topográfico calculado del documento."""

    puntos: pd.DataFrame
    reporte: ReporteLimpieza
    tin: TIN
    curvas: list[Curva]
    perfil: Perfil | None


def _plantilla_json() -> dict[str, Any]:
    from importlib.resources import files
    return json.loads(files("suite_electrica.plantillas").joinpath("proyecto.json").read_text("utf-8"))


def _conductor_plantilla() -> Conductor:
    from importlib.resources import as_file, files
    with as_file(files("suite_electrica.plantillas").joinpath("conductores.csv")) as ruta:
        return next(iter(leer_catalogo(ruta).values()))


def conductor_a_dict(c: Conductor) -> dict[str, Any]:
    """Conductor en unidades de catálogo."""
    return {"nombre": c.nombre, "seccion_mm2": c.seccion * 1e6, "diametro_mm": c.diametro * 1e3,
            "masa_kg_m": c.peso / G, "carga_rotura_kN": c.carga_rotura / 1e3,
            "modulo_elasticidad_MPa": c.modulo_elasticidad / 1e6,
            "coef_dilatacion_1_C": c.coef_dilatacion, "fuente": c.fuente,
            "curva_inicial": ";".join(f"{v:g}" for v in c.curva_inicial),
            "curva_creep": ";".join(f"{v:g}" for v in c.curva_creep)}


def conductor_desde_dict(d: dict[str, Any]) -> Conductor:
    return Conductor(str(d["nombre"]), float(d["seccion_mm2"]) * 1e-6, float(d["diametro_mm"]) * 1e-3,
                     float(d["masa_kg_m"]) * G, float(d["carga_rotura_kN"]) * 1e3,
                     float(d["modulo_elasticidad_MPa"]) * 1e6, float(d["coef_dilatacion_1_C"]),
                     str(d.get("fuente", "")), curva(d.get("curva_inicial")), curva(d.get("curva_creep")))


def _df_a_json(df: pd.DataFrame) -> list[dict[str, Any]]:
    return [{k: (None if (isinstance(v, float) and math.isnan(v)) else
                 v.item() if isinstance(v, np.generic) else v) for k, v in r.items()}
            for r in df.to_dict(orient="records")]


def _hip_df(registros: list[dict[str, Any]]) -> pd.DataFrame:
    df = pd.DataFrame(registros).reindex(columns=COLS_HIP)
    df["nombre"] = df["nombre"].fillna("").astype(str)
    for c in COLS_HIP[1:]:
        df[c] = pd.to_numeric(df[c], errors="coerce")
        if not c.startswith("altitud"):
            df[c] = df[c].fillna(0.0)
    return df


def _est_df(registros: list[dict[str, Any]] | pd.DataFrame) -> pd.DataFrame:
    df = pd.DataFrame(registros).reindex(columns=COLS_EST)
    df["progresiva_m"] = pd.to_numeric(df["progresiva_m"], errors="coerce")
    df["altura_amarre_m"] = pd.to_numeric(df["altura_amarre_m"], errors="coerce")
    df["retenidas"] = pd.to_numeric(df["retenidas"], errors="coerce")
    for c in ("tipo", "armado", "poste"):
        df[c] = pd.Series([v.strip() if isinstance(v, str) else "" for v in df[c]], index=df.index,
                          dtype=object)
    df["tipo"] = pd.Series([v.upper() for v in df["tipo"]], index=df.index, dtype=object)
    return df.dropna(subset=["progresiva_m"]).sort_values("progresiva_m").reset_index(drop=True)


class Documento:
    """Proyecto completo editable (modelo de la app)."""

    def __init__(self) -> None:
        pl = _plantilla_json()
        self.nombre = "Proyecto nuevo"
        self.autor = ""
        self.zona_utm = "19S"
        self.puntos = pd.DataFrame(columns=datos.COLUMNAS)
        self.eje: np.ndarray | None = None
        self.topo = ParamTopo()
        self.linea = ParamLinea()
        self.conductor = _conductor_plantilla()
        hips = pl["hipotesis"]
        self.hipotesis = _hip_df([{"nombre": h["nombre"], "temperatura_C": h["temperatura_C"],
                                   "presion_viento_Pa": h.get("presion_viento_Pa", 0),
                                   "espesor_hielo_mm": h.get("espesor_hielo_mm", 0),
                                   "densidad_hielo_kg_m3": h.get("densidad_hielo_kg_m3", 0)} for h in hips])
        self.fuentes: dict[str, str] = dict(pl["fuentes"])
        for k in FUENTES_EXTRA:
            self.fuentes.setdefault(k, "PENDIENTE: citar CNE / norma DGE")
        self.estructuras = _est_df([])
        self.catalogos = Catalogos.plantilla()
        self.ruta: Path | None = None
        self.modificado = False
        self._topo: Topografia | None = None
        self._deshacer: list[pd.DataFrame] = []

    # ------------------------------------------------------------------ topografía
    def cambio_topografia(self) -> None:
        """Invalida la topografía calculada (llamar tras cambiar puntos, eje o parámetros)."""
        self._topo = None
        self.modificado = True

    def importar_puntos(self, ruta: str | Path) -> None:
        self.puntos = datos.leer(ruta)
        self.cambio_topografia()

    def importar_eje(self, ruta: str | Path, nombre: str | None = None) -> None:
        """Eje desde DXF/CSV, o la ruta ``nombre`` (por defecto la primera) de KML/GPX/XLS."""
        r = Path(ruta)
        ext = r.suffix.lower()
        if ext in {".kml", ".gpx", ".xls", ".xlsx"}:
            from suite_electrica.topocraft.importar import importar
            rutas = importar(r)[1]
            elegidas = [xyz for n, xyz in rutas if nombre in (None, n)]
            if not elegidas:
                raise ValueError(f"{r.name}: sin rutas" + (f" llamadas {nombre!r}" if nombre else ""))
            self.eje = elegidas[0][:, :2]
        else:
            self.eje = leer_polilinea(r) if ext == ".dxf" else datos.leer_eje_csv(r)
        self.estructuras = _est_df([])
        self.cambio_topografia()

    def topografia(self) -> Topografia:
        """Limpieza, TIN, curvas y perfil (en caché hasta el próximo cambio)."""
        if self._topo is None:
            if self.puntos.empty:
                raise ValueError("El proyecto no tiene puntos topográficos (Importar puntos)")
            t = self.topo
            df, rep = datos.limpiar(self.puntos, quitar_atipicos=t.quitar_atipicos)
            tin = TIN(df[["este", "norte", "cota"]].to_numpy(), max_lado=t.max_lado)
            curvas = tin.curvas_nivel(niveles(float(df.cota.min()), float(df.cota.max()), t.equidistancia))
            perfil = perfil_longitudinal(tin, self.eje, t.paso_perfil) if self.eje is not None else None
            self._topo = Topografia(df, rep, tin, curvas, perfil)
        return self._topo

    # ------------------------------------------------------------------ línea
    def hipotesis_obj(self) -> list[Hipotesis]:
        def lim(v: float, defecto: float) -> float:
            return defecto if pd.isna(v) or (v == 0 and defecto > 0) else float(v)
        return [Hipotesis(str(h.nombre), float(h.temperatura_C), float(h.presion_viento_Pa),
                          float(h.espesor_hielo_mm) / 1000, float(h.densidad_hielo_kg_m3),
                          lim(h.altitud_min_m, -math.inf), lim(h.altitud_max_m, math.inf))
                for h in self.hipotesis.itertuples(index=False) if str(h.nombre).strip()]

    def _estructuras_para_calculo(self) -> pd.DataFrame:
        df = self.estructuras.copy()
        alturas = []
        for r in df.itertuples(index=False):
            if not pd.isna(r.altura_amarre_m):
                alturas.append(float(r.altura_amarre_m))
            else:
                h = self.catalogos.altura_amarre(r.poste, r.armado)
                alturas.append(h if h is not None else self.linea.altura_amarre)
        df["altura_amarre_m"] = alturas
        df["tipo"] = [t if t else None for t in df["tipo"]]
        return df

    def proyecto(self, con_estructuras: bool = True) -> Proyecto:
        """Proyecto de LineCraft equivalente al documento."""
        if self.eje is None:
            raise ValueError("El proyecto no tiene eje de línea (Importar eje)")
        perfil = self.topografia().perfil
        assert perfil is not None
        L = self.linea
        p = Proyecto(
            nombre=self.nombre, conductor=self.conductor, n_conductores=L.n_conductores,
            modelo=L.modelo, hipotesis=self.hipotesis_obj(), hip_inicial=L.hip_inicial,
            tiro_inicial_pct=L.tiro_inicial_pct, hip_flecha_maxima=L.hip_flecha_maxima,
            tiro_maximo_pct=L.tiro_maximo_pct, distancia_minima_terreno=L.distancia_minima_terreno,
            altura_amarre=L.altura_amarre, vano_maximo=L.vano_maximo, vano_minimo=L.vano_minimo,
            angulo_retenida=L.angulo_retenida, perfil=perfil, eje=self.eje,
            estructuras_manual=(self._estructuras_para_calculo()
                                if con_estructuras and len(self.estructuras) else None),
            fuentes=dict(self.fuentes), condicion_final=L.condicion_final,
            vanos_independientes=L.estado_final_vanos == "por vano",
        )
        p.hip(p.hip_inicial)
        p.hip(p.hip_flecha_maxima)
        return p

    def _n_retenidas(self, r: Resultado) -> dict[int, int]:
        out = {}
        if len(self.estructuras) == len(r.estructuras):
            for e, n in zip(r.estructuras, self.estructuras["retenidas"]):
                if not pd.isna(n):
                    out[e.numero] = int(n)
        return out

    def _completar(self, r: Resultado) -> Resultado:
        """Agrega verificación de catálogos, metrado y avisos."""
        L = self.linea
        nret = self._n_retenidas(r)
        r.verificacion = cat_mod.verificar(r, self.catalogos, L.fs_poste, L.fs_retenida,
                                           L.retenida or None, nret)
        r.metrado = cat_mod.metrado(r, self.catalogos, L.desperdicio_pct, L.retenida or None, nret)
        r.avisos += self.catalogos.avisos()
        for k in FUENTES_EXTRA:
            f = self.fuentes.get(k, "").strip()
            if not f or "PENDIENTE" in f.upper() or "EJEMPLO" in f.upper():
                r.avisos.append(f"PENDIENTE_REVISION: '{k}' -> {f or 'sin fuente'}")
        for v in r.verificacion.itertuples(index=False):
            if v.Cumple == "NO":
                r.avisos.append(f"{v.Estructura}: {v.Observación}")
        cfg = ConfigAlertas(**{f.name: getattr(L, f.name) for f in fields(ConfigAlertas)})
        r.alertas = alertas(r, self.catalogos, cfg)
        for k in FUENTES_ALERTAS:
            f = self.fuentes.get(k, "").strip()
            if getattr(L, k) and (not f or "PENDIENTE" in f.upper() or "EJEMPLO" in f.upper()):
                r.avisos.append(f"PENDIENTE_REVISION: '{k}' -> {f or 'sin fuente'}")
        return r

    def calcular(self) -> Resultado:
        """Calcula la línea; si no hay estructuras, primero las ubica automáticamente."""
        if self.estructuras.empty:
            return self.ubicar_automatico()
        return self._completar(calcular(self.proyecto()))

    def _guardar_estructuras(self, r: Resultado) -> None:
        anterior = self.estructuras
        self.estructuras = _est_df([{
            "progresiva_m": round(e.progresiva, 3), "tipo": e.tipo, "altura_amarre_m": np.nan,
            "armado": e.armado, "poste": e.poste,
            "retenidas": 1 if cat_mod.lleva_retenida(e) else np.nan} for e in r.estructuras])
        if not anterior.equals(self.estructuras):
            self._deshacer.append(anterior)
        self.modificado = True

    def ubicar_automatico(self) -> Resultado:
        """Ubicación automática de estructuras + asignación de armados y postes."""
        r = calcular(self.proyecto(con_estructuras=False))
        self._guardar_estructuras(r)
        return self.asignar_catalogos(solo_vacios=True)

    def estructuras_minimas(self) -> None:
        """Solo inicio, fin y vértices del eje (para ubicar el resto a mano)."""
        if self.eje is None:
            raise ValueError("El proyecto no tiene eje de línea")
        acum = progresivas_eje(self.eje)
        self._apilar()
        self.estructuras = _est_df([{"progresiva_m": round(float(s), 3)} for s in acum])
        self.modificado = True

    def asignar_catalogos(self, solo_vacios: bool = True) -> Resultado:
        """Asigna armado (por tipo y deflexión) y poste (menor que cumple) a cada estructura."""
        r = calcular(self.proyecto())
        fm = cat_mod._fuerzas_maximas(r)
        df = self.estructuras.copy()
        for i, e in enumerate(r.estructuras):
            if not (solo_vacios and df.at[i, "armado"]):
                df.at[i, "armado"] = self.catalogos.elegir_armado(e.tipo, e.angulo)
            if not (solo_vacios and df.at[i, "poste"]):
                fh, mb, _ = fm.get(e.numero, (0.0, 0.0, 0.0))
                df.at[i, "poste"] = cat_mod.elegir_poste(self.catalogos, e, fh, mb, self.linea.fs_poste)
        if not df.equals(self.estructuras):
            self._apilar()
            self.estructuras = df
            self.modificado = True
        return self._completar(calcular(self.proyecto()))

    # ------------------------------------------------------------------ edición de estructuras
    def _apilar(self) -> None:
        self._deshacer.append(self.estructuras.copy())
        del self._deshacer[:-50]

    def puede_deshacer(self) -> bool:
        return bool(self._deshacer)

    def deshacer(self) -> bool:
        if not self._deshacer:
            return False
        self.estructuras = self._deshacer.pop()
        self.modificado = True
        return True

    def bloqueada(self, i: int) -> bool:
        """Inicio, fin y vértices del eje no se mueven ni se eliminan."""
        if self.eje is None:
            return False
        s = float(self.estructuras.at[i, "progresiva_m"])
        return bool(np.min(np.abs(progresivas_eje(self.eje) - s)) <= TOL_VERTICE)

    def _limites(self, i: int) -> tuple[float, float]:
        s = self.estructuras["progresiva_m"].to_numpy()
        lo = s[i - 1] + SEPARACION_MIN if i > 0 else 0.0
        hi = s[i + 1] - SEPARACION_MIN if i < len(s) - 1 else float(progresivas_eje(self.eje)[-1])
        return lo, hi

    def agregar_estructura(self, s: float) -> int:
        """Inserta una estructura en la progresiva ``s``; devuelve su índice."""
        if self.eje is None:
            raise ValueError("Sin eje")
        L = float(progresivas_eje(self.eje)[-1])
        prog = self.estructuras["progresiva_m"].to_numpy()
        if not (0 < s < L) or (len(prog) and np.min(np.abs(prog - s)) < SEPARACION_MIN):
            raise ValueError(f"No se puede ubicar una estructura en {s:.2f} m")
        self._apilar()
        fila = pd.DataFrame([{"progresiva_m": round(s, 2)}])
        self.estructuras = _est_df(pd.concat([self.estructuras, fila], ignore_index=True))
        self.modificado = True
        return int(np.flatnonzero(np.isclose(self.estructuras["progresiva_m"], round(s, 2)))[0])

    def mover_estructura(self, i: int, s: float) -> float:
        """Mueve la estructura ``i`` (limitada entre vecinas); devuelve la progresiva final."""
        if self.bloqueada(i):
            raise ValueError("Las estructuras de inicio, fin y vértices no se mueven")
        lo, hi = self._limites(i)
        s = round(min(max(s, lo), hi), 2)
        if s != self.estructuras.at[i, "progresiva_m"]:
            self._apilar()
            self.estructuras.at[i, "progresiva_m"] = s
            self.modificado = True
        return s

    def eliminar_estructura(self, i: int) -> None:
        if self.bloqueada(i):
            raise ValueError("Las estructuras de inicio, fin y vértices no se eliminan")
        self._apilar()
        self.estructuras = self.estructuras.drop(index=i).reset_index(drop=True)
        self.modificado = True

    def cambiar_estructura(self, i: int, campo: str, valor: Any) -> None:
        """Cambia tipo, armado, poste, altura_amarre_m o retenidas."""
        if campo not in COLS_EST or campo == "progresiva_m":
            raise ValueError(f"Campo no editable: {campo}")
        if campo == "tipo" and valor and valor not in TIPOS:
            raise ValueError(f"Tipo inválido: {valor}")
        self._apilar()
        if campo in ("altura_amarre_m", "retenidas"):
            valor = np.nan if valor in ("", None) else float(valor)
        self.estructuras.at[i, campo] = valor
        self.modificado = True

    def angulo_en(self, s: float) -> float:
        """Deflexión del eje si ``s`` coincide con un vértice (grados)."""
        acum = progresivas_eje(self.eje)
        j = np.flatnonzero(np.abs(acum - s) <= TOL_VERTICE)
        return float(angulos_eje(self.eje)[j[0]]) if len(j) else 0.0

    # ------------------------------------------------------------------ archivo
    def a_dict(self) -> dict[str, Any]:
        return {
            "formato": FORMATO, "version": VERSION,
            "nombre": self.nombre, "autor": self.autor, "zona_utm": self.zona_utm,
            "topo": asdict(self.topo), "linea": asdict(self.linea),
            "conductor": conductor_a_dict(self.conductor),
            "hipotesis": _df_a_json(self.hipotesis), "fuentes": self.fuentes,
            "puntos": {"columnas": datos.COLUMNAS,
                       "filas": self.puntos[datos.COLUMNAS].astype(object).where(
                           self.puntos[datos.COLUMNAS].notna(), None).values.tolist()},
            "eje": None if self.eje is None else np.asarray(self.eje)[:, :2].tolist(),
            "estructuras": _df_a_json(self.estructuras),
            "catalogos": {k: _df_a_json(self.catalogos.tabla(k)) for k in cat_mod.COLUMNAS},
        }

    def guardar(self, ruta: str | Path | None = None) -> Path:
        ruta = Path(ruta or self.ruta or "")
        if not ruta.name:
            raise ValueError("Indique dónde guardar")
        if ruta.suffix.lower() != EXTENSION:
            ruta = ruta.with_suffix(EXTENSION)
        ruta.write_text(json.dumps(self.a_dict(), ensure_ascii=False, indent=1), encoding="utf-8")
        self.ruta = ruta
        self.modificado = False
        return ruta

    @classmethod
    def abrir(cls, ruta: str | Path) -> Documento:
        d = json.loads(Path(ruta).read_text(encoding="utf-8"))
        if d.get("formato") != FORMATO:
            raise ValueError(f"{ruta} no es un proyecto de Suite Eléctrica")
        if int(d.get("version", 0)) > VERSION:
            raise ValueError("El archivo es de una versión más nueva de la suite")
        doc = cls()
        doc.nombre, doc.autor, doc.zona_utm = d["nombre"], d.get("autor", ""), d.get("zona_utm", "19S")
        doc.topo = ParamTopo(**{k: v for k, v in d["topo"].items() if k in {f.name for f in fields(ParamTopo)}})
        doc.linea = ParamLinea(**{k: v for k, v in d["linea"].items()
                                  if k in {f.name for f in fields(ParamLinea)}})
        doc.conductor = conductor_desde_dict(d["conductor"])
        doc.hipotesis = _hip_df(d["hipotesis"])
        doc.fuentes = dict(d.get("fuentes", {}))
        pts = pd.DataFrame(d["puntos"]["filas"], columns=d["puntos"]["columnas"])
        for c in ("este", "norte", "cota"):
            pts[c] = pd.to_numeric(pts[c], errors="coerce")
        for c in ("punto", "descripcion"):
            pts[c] = pts[c].fillna("").astype(str)
        doc.puntos = pts
        doc.eje = None if d.get("eje") is None else np.asarray(d["eje"], dtype=float)
        doc.estructuras = _est_df(d.get("estructuras", []))
        doc.catalogos = Catalogos(**{k: cat_mod.normalizar(k, pd.DataFrame(v))
                                     for k, v in d.get("catalogos", {}).items() if k in cat_mod.COLUMNAS})
        doc.ruta, doc.modificado = Path(ruta), False
        return doc

    @classmethod
    def desde_linecraft_json(cls, ruta: str | Path) -> Documento:
        """Importa un ``proyecto.json`` de LineCraft (con puntos y eje, no Excel)."""
        ruta = Path(ruta)
        base = ruta.parent
        cfg = json.loads(ruta.read_text(encoding="utf-8"))
        doc = cls()
        doc.nombre = cfg.get("nombre", ruta.stem)
        pf = cfg["perfil"]
        if "puntos" not in pf:
            raise ValueError("Solo se importan proyectos con 'puntos' y 'eje'")
        doc.importar_puntos(base / pf["puntos"])
        doc.importar_eje(base / pf["eje"])
        doc.topo.paso_perfil = float(pf.get("paso", doc.topo.paso_perfil))
        doc.topo.max_lado = pf.get("max_lado")
        doc.conductor = leer_catalogo(base / cfg["conductor"]["catalogo"])[cfg["conductor"]["nombre"]]
        doc.hipotesis = _hip_df(cfg["hipotesis"])
        ini = cfg["condicion_inicial"]
        L = doc.linea
        L.n_conductores = int(cfg.get("n_conductores", 3))
        L.modelo = cfg.get("modelo", "catenaria")
        L.hip_inicial, L.tiro_inicial_pct = ini["hipotesis"], float(ini["tiro_pct_rotura"])
        L.hip_flecha_maxima = cfg["hipotesis_flecha_maxima"]
        L.tiro_maximo_pct = float(cfg["tiro_maximo_pct_rotura"])
        L.distancia_minima_terreno = float(cfg["distancia_minima_terreno_m"])
        L.altura_amarre = float(cfg["altura_amarre_m"])
        L.vano_maximo = float(cfg["vano_maximo_m"])
        L.vano_minimo = float(cfg.get("vano_minimo_m", 0.2 * L.vano_maximo))
        L.angulo_retenida = float(cfg.get("angulo_retenida_deg", 45))
        doc.fuentes.update({k: str(v) for k, v in cfg.get("fuentes", {}).items()})
        if cfg.get("estructuras"):
            doc.estructuras = _est_df(pd.read_csv(base / cfg["estructuras"], comment="#", skipinitialspace=True))
        doc.ruta, doc.modificado = None, True
        return doc


def copia(doc: Documento) -> Documento:
    """Copia profunda (para cálculos en segundo plano)."""
    return copy.deepcopy(doc)


__all__ = ["Documento", "ParamLinea", "ParamTopo", "Topografia", "PARAMETROS_NORMATIVOS", "EXTENSION"]
