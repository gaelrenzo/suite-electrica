from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from suite_electrica.documento import Documento
from suite_electrica.linecraft.catalogos import Catalogos, normalizar
from suite_electrica.topocraft import sinteticos as st

EJ = Path(__file__).resolve().parents[1] / "examples"


@pytest.fixture
def doc():
    d = Documento()
    d.importar_puntos(EJ / "colina_gaussiana.csv")
    d.importar_eje(EJ / "eje.dxf")
    return d


def test_sin_datos_errores_claros():
    d = Documento()
    with pytest.raises(ValueError, match="puntos"):
        d.topografia()
    d.puntos = st.malla(st.plano_inclinado(), 50)
    with pytest.raises(ValueError, match="eje"):
        d.proyecto()


def test_calculo_auto_asigna_catalogos(doc):
    r = doc.calcular()
    assert r.cumple
    assert len(doc.estructuras) == len(r.estructuras)
    assert (doc.estructuras["armado"] != "").all() and (doc.estructuras["poste"] != "").all()
    assert set(doc.estructuras.loc[doc.estructuras.tipo == "FIN", "armado"]) == {"PR-1"}
    assert set(r.verificacion["Cumple"]) == {"SÍ"}
    cond = r.metrado[r.metrado["Unidad"] == "m"]["Cantidad"].iloc[0]
    assert cond == pytest.approx(3 * sum(v.longitud for v in r.vanos), rel=0.01)
    assert any("catálogo de postes" in a for a in r.avisos)


def test_edicion_bloqueos_y_deshacer(doc):
    doc.estructuras_minimas()
    assert len(doc.estructuras) == 4
    assert all(doc.bloqueada(i) for i in range(4))
    with pytest.raises(ValueError):
        doc.eliminar_estructura(0)
    with pytest.raises(ValueError):
        doc.mover_estructura(1, 100)
    i = doc.agregar_estructura(150)
    assert i == 1 and not doc.bloqueada(1)
    with pytest.raises(ValueError):
        doc.agregar_estructura(150.5)  # muy cerca
    assert doc.mover_estructura(1, 9999) == pytest.approx(doc.estructuras.at[2, "progresiva_m"] - 1, abs=0.01)
    doc.cambiar_estructura(1, "tipo", "ANCLAJE")
    with pytest.raises(ValueError):
        doc.cambiar_estructura(1, "tipo", "XX")
    r = doc.calcular()
    assert len(r.tramos) == 2  # FIN-ANCLAJE y ANCLAJE-FIN
    doc.eliminar_estructura(1)
    assert len(doc.estructuras) == 4
    assert doc.deshacer() and len(doc.estructuras) == 5  # deshace eliminar
    while doc.deshacer():
        pass
    assert doc.estructuras.empty  # vuelve al estado antes de estructuras mínimas


def test_guardar_y_abrir(doc, tmp_path):
    doc.nombre = "Línea Ñuñoa"
    doc.calcular()
    doc.catalogos.materiales = normalizar("materiales", pd.DataFrame(
        [{"armado": "PS-1", "material": "Aislador X", "unidad": "und", "cantidad": 3}]))
    ruta = doc.guardar(tmp_path / "linea")
    assert ruta.suffix == ".sel" and not doc.modificado
    d2 = Documento.abrir(ruta)
    assert d2.nombre == "Línea Ñuñoa"
    assert d2.estructuras.equals(doc.estructuras)
    assert np.allclose(d2.eje, doc.eje)
    assert d2.puntos[["este", "norte", "cota"]].equals(doc.puntos[["este", "norte", "cota"]])
    r = d2.calcular()
    ais = r.metrado[r.metrado["Descripción"] == "Aislador X"]["Cantidad"].iloc[0]
    assert ais == 3 * (d2.estructuras.armado == "PS-1").sum()
    (tmp_path / "otro.sel").write_text('{"formato": "otro"}')
    with pytest.raises(ValueError):
        Documento.abrir(tmp_path / "otro.sel")


def test_altura_desde_catalogo_y_verificacion_falla(doc):
    doc.catalogos.postes.loc[:, "empotramiento_m"] = 1.3
    doc.catalogos.armados.loc[:, "distancia_punta_amarre_m"] = 0.2
    assert doc.catalogos.altura_amarre("CAC 12/300", "PS-1") == pytest.approx(10.5)
    doc.linea.fs_poste = 50  # exigencia imposible
    r = doc.calcular()
    assert r.estructuras[0].altura_amarre == pytest.approx(10.5)  # FIN: poste asignado
    assert r.estructuras[1].poste == ""  # ningún poste cumple FS 50 -> altura por defecto
    assert not r.cumple
    assert any("Sin poste asignado" in a for a in r.avisos)


def test_importar_linecraft_json():
    d = Documento.desde_linecraft_json(EJ / "linecraft" / "proyecto_manual.json")
    assert len(d.estructuras) == 8 and d.estructuras.at[4, "tipo"] == "ANCLAJE"
    assert d.linea.vano_maximo == 120
    assert not d.calcular().cumple  # el ejemplo manual no cumple a propósito


def test_catalogos_plantilla():
    c = Catalogos.plantilla()
    assert list(c.postes["codigo"]) == ["CAC 12/300", "CAC 12/400", "CAC 13/400"]
    assert c.elegir_armado("ANCLAJE", 10) == "PR-1"
    assert c.elegir_armado("SUSPENSION", 0) == "PS-1"


def test_cli_calcula_sel(doc, tmp_path):
    from suite_electrica.linecraft.cli import main
    ruta = doc.guardar(tmp_path / "p.sel")
    assert main(["calcular", str(ruta), "--out", str(tmp_path / "out")]) == 0
    hojas = pd.read_excel(tmp_path / "out" / "planilla_estructuras.xlsx", sheet_name=None)
    assert {"Verificacion", "Metrado"} <= set(hojas)
    assert len(Documento.abrir(ruta).estructuras) > 0  # guardó la ubicación automática


def test_creep_aumenta_flecha(doc, tmp_path):
    from dataclasses import replace
    r0 = doc.calcular()
    doc.conductor = replace(doc.conductor, curva_inicial=(0.0629, 55.61, 25.38, -85.78),
                            curva_creep=(0.0, 38.29))
    r1 = doc.calcular()
    hf = doc.linea.hip_flecha_maxima
    assert all(b.flechas[hf] > a.flechas[hf] for a, b in zip(r0.vanos, r1.vanos))
    ini = doc.linea.hip_inicial  # la condición inicial (EDS) no cambia
    assert r1.tramos[0].estados[ini].H == pytest.approx(r0.tramos[0].estados[ini].H)
    doc.linea.condicion_final = "inicial"
    r2 = doc.calcular()
    assert r2.vanos[0].flechas[hf] == pytest.approx(r0.vanos[0].flechas[hf])
    d = Documento.abrir(doc.guardar(tmp_path / "c.sel"))  # curvas y condición se guardan
    assert d.conductor.curva_creep == (0.0, 38.29) and d.linea.condicion_final == "inicial"


def test_hipotesis_por_altitud(doc):
    r0 = doc.calcular()
    z = np.mean([e.cota_terreno for e in r0.estructuras])
    hf = doc.linea.hip_flecha_maxima
    base = doc.hipotesis[doc.hipotesis.nombre == hf].iloc[0]
    caliente = base.copy()
    caliente["temperatura_C"] = base.temperatura_C + 30
    # rango bajo con la temperatura original, rango alto (que contiene la línea) más caliente
    bajo, alto = base.copy(), caliente
    bajo["altitud_min_m"], bajo["altitud_max_m"] = np.nan, z - 1e4
    alto["altitud_min_m"], alto["altitud_max_m"] = z - 1e4, np.nan
    otras = doc.hipotesis[doc.hipotesis.nombre != hf]
    doc.hipotesis = pd.concat([otras, pd.DataFrame([bajo, alto])], ignore_index=True)
    r1 = doc.calcular()
    assert all(b.flechas[hf] > a.flechas[hf] for a, b in zip(r0.vanos, r1.vanos))
    assert r1.tramos[0].estados[hf].temperatura == base.temperatura_C + 30


def test_vanos_independientes(doc):
    doc.calcular()
    doc.linea.estado_final_vanos = "por vano"
    r = doc.calcular()
    t = max(r.tramos, key=lambda t: t.fin - t.inicio)
    assert t.fin - t.inicio > 1
    from suite_electrica.linecraft.calculo import _estados_tramo
    p = doc.proyecto()
    hf, ini = doc.linea.hip_flecha_maxima, doc.linea.hip_inicial
    hs = [_estados_tramo(p, t.vano_regulador, 0.0, None,
                         (r.vanos[i].longitud, r.vanos[i].desnivel))[hf][0] for i in range(t.inicio, t.fin)]
    assert max(hs) - min(hs) > 1.0  # cada vano con su propio tiro final
    assert t.estados[ini].H == pytest.approx(_estados_tramo(p, t.vano_regulador)[ini][0])
