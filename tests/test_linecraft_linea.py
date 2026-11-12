import json
import math

import ezdxf
import numpy as np
import pandas as pd
import pytest

from conftest import tin_de
from suite_electrica.core.geo.perfil import perfil_longitudinal
from suite_electrica.linecraft import proyecto, salidas
from suite_electrica.linecraft.calculo import calcular
from suite_electrica.linecraft.cli import main
from suite_electrica.linecraft.estructuras import ANCLAJE, ANGULO, FIN, angulos_eje, ubicar_automatico
from suite_electrica.topocraft import salidas as topo_salidas
from suite_electrica.topocraft import sinteticos as st

CATALOGO = "nombre,seccion_mm2,diametro_mm,masa_kg_m,carga_rotura_kN,modulo_elasticidad_MPa,coef_dilatacion_1_C,fuente\n" \
           "C1,34.36,7.5,0.0945,10.199,58860,23e-6,prueba\n"
EJE_L = np.array([[st.E0 + 20, st.N0 + 50], [st.E0 + 320, st.N0 + 50], [st.E0 + 320, st.N0 + 450]])


def escribir_proyecto(tmp_path, sup, eje, **extra):
    datos = st.malla(sup, 10)
    datos.to_csv(tmp_path / "pts.csv", index=False)
    pd.DataFrame(eje, columns=["este", "norte"]).to_csv(tmp_path / "eje.csv", index=False)
    (tmp_path / "cat.csv").write_text(CATALOGO)
    cfg = {
        "nombre": "Prueba", "perfil": {"puntos": "pts.csv", "eje": "eje.csv", "paso": 5},
        "conductor": {"catalogo": "cat.csv", "nombre": "C1"}, "n_conductores": 3,
        "hipotesis": [{"nombre": "EDS", "temperatura_C": 15},
                      {"nombre": "TMAX", "temperatura_C": 40},
                      {"nombre": "VIENTO", "temperatura_C": 10, "presion_viento_Pa": 230}],
        "condicion_inicial": {"hipotesis": "EDS", "tiro_pct_rotura": 18},
        "hipotesis_flecha_maxima": "TMAX", "tiro_maximo_pct_rotura": 40,
        "distancia_minima_terreno_m": 6.5, "altura_amarre_m": 10.6,
        "vano_maximo_m": 100, "vano_minimo_m": 30,
        "fuentes": {k: "prueba" for k in proyecto.PARAMETROS_NORMATIVOS},
    }
    cfg.update(extra)
    ruta = tmp_path / "proyecto.json"
    ruta.write_text(json.dumps(cfg))
    return ruta


def test_angulos_eje():
    a = angulos_eje(EJE_L)
    assert a.tolist() == pytest.approx([0, 90, 0])
    assert angulos_eje(EJE_L[::-1])[1] == pytest.approx(-90)


def test_ubicacion_automatica_terreno_plano():
    _, tin = tin_de(st.plano_inclinado(0, 0))
    pf = perfil_longitudinal(tin, EJE_L, 5)
    prog, avisos = ubicar_automatico(pf, EJE_L, 10.6, 100, 30, 6.5, 2000.0, 1.0)
    assert not avisos
    assert prog[0] == 0 and prog[-1] == pytest.approx(700)
    assert any(abs(s - 300) < 1e-6 for s in prog)  # vértice forzado
    assert np.diff(prog).max() <= 100 + 1e-9
    assert np.allclose(np.diff(prog), 100)


def test_linea_en_colina_cumple_despeje(tmp_path):
    r = calcular(proyecto.cargar(escribir_proyecto(tmp_path, st.colina_gaussiana(), st.eje_ejemplo())))
    assert all(v.despeje >= 6.5 for v in r.vanos)
    assert all(v.longitud <= 100 + 1e-9 for v in r.vanos)
    assert r.estructuras[0].tipo == FIN and r.estructuras[-1].tipo == FIN
    assert sum(e.tipo == ANGULO for e in r.estructuras) == 2
    assert not any("PENDIENTE" in a for a in r.avisos)


def test_cargas_terreno_plano_manual_con_anclaje(tmp_path):
    (tmp_path / "est.csv").write_text("progresiva_m,tipo\n0,\n100,\n200,\n300,\n400,ANCLAJE\n500,\n600,\n700,\n")
    r = calcular(proyecto.cargar(escribir_proyecto(tmp_path, st.plano_inclinado(0, 0), EJE_L,
                                                   estructuras="est.csv")))
    assert len(r.tramos) == 2
    assert [t.vano_regulador for t in r.tramos] == pytest.approx([100, 100])
    t0 = r.tramos[0]
    assert t0.estados["EDS"].H == pytest.approx(0.18 * 10199, rel=1e-9)
    c = {(x.estructura, x.hipotesis): x for x in r.cargas}
    H = t0.estados["TMAX"].H
    w = r.proyecto.conductor.peso
    # Terminal: tiro longitudinal = n·H; vano peso = mitad del vano en terreno plano
    assert c[(1, "TMAX")].longitudinal == pytest.approx(3 * H)
    assert c[(1, "TMAX")].vano_peso == pytest.approx(50)
    # Ángulo de 90° sin viento: transversal = n·2H·sen(45°); vertical = n·w·a
    assert r.estructuras[3].tipo == ANGULO
    assert c[(4, "TMAX")].transversal == pytest.approx(3 * 2 * H * math.sin(math.pi / 4))
    assert c[(4, "TMAX")].vertical == pytest.approx(3 * w * 100)
    assert c[(4, "TMAX")].tiro_retenida == pytest.approx(c[(4, "TMAX")].horizontal / math.cos(math.pi / 4))
    # Alineamiento con viento: solo viento transversal, sin retenida
    v = c[(2, "VIENTO")]
    assert v.transversal == pytest.approx(3 * 230 * 0.0075 * 100)
    assert v.longitudinal == pytest.approx(0, abs=1e-6) and v.tiro_retenida == 0
    assert r.estructuras[4].tipo == ANCLAJE


def test_avisos_fuentes_y_despeje(tmp_path):
    ruta = escribir_proyecto(tmp_path, st.plano_inclinado(0, 0), EJE_L, fuentes={},
                             distancia_minima_terreno_m=12)
    r = calcular(proyecto.cargar(ruta))
    assert sum("PENDIENTE_REVISION" in a for a in r.avisos) == len(proyecto.PARAMETROS_NORMATIVOS)
    assert not r.cumple


def test_salidas_y_cli(tmp_path):
    ruta = escribir_proyecto(tmp_path, st.colina_gaussiana(), st.eje_ejemplo())
    r = calcular(proyecto.cargar(ruta))
    doc = ezdxf.readfile(salidas.perfil_dxf(tmp_path / "p.dxf", r))
    assert not doc.audit().has_errors
    assert {"POSTES", "CONDUCTOR", "DESPEJE", "PERFIL"} <= {l.dxf.name for l in doc.layers}
    assert len(doc.modelspace().query('LINE[layer=="POSTES"]')) == len(r.estructuras)
    hojas = pd.read_excel(salidas.planilla_excel(tmp_path / "p.xlsx", r), sheet_name=None)
    assert set(hojas) == {"Estructuras", "Vanos", "Mecanico", "Cargas", "Avisos"}
    assert len(hojas["Estructuras"]) == len(r.estructuras)
    assert salidas.memoria_pdf(tmp_path / "m.pdf", r).read_bytes()[:4] == b"%PDF"
    assert main(["calcular", str(ruta), "--out", str(tmp_path / "out")]) == 0
    assert (tmp_path / "out" / "memoria_calculo.pdf").exists()


def test_perfil_desde_excel_topocraft(tmp_path):
    _, tin = tin_de(st.colina_gaussiana())
    pf = perfil_longitudinal(tin, st.eje_ejemplo(), 10)
    topo_salidas.perfil_excel(tmp_path / "perfil.xlsx", pf)
    pf2, eje = topo_salidas.leer_perfil_excel(tmp_path / "perfil.xlsx")
    assert np.allclose(eje, st.eje_ejemplo(), atol=1e-3)
    assert np.allclose(pf2.cota, pf.cota, atol=1e-3)
    ruta = escribir_proyecto(tmp_path, st.colina_gaussiana(), st.eje_ejemplo(),
                             perfil={"excel": "perfil.xlsx"})
    assert calcular(proyecto.cargar(ruta)).cumple


def test_manual_sin_vertice_es_error(tmp_path):
    (tmp_path / "est.csv").write_text("progresiva_m\n0\n150\n450\n700\n")
    with pytest.raises(ValueError, match="vértice"):
        calcular(proyecto.cargar(escribir_proyecto(tmp_path, st.plano_inclinado(0, 0), EJE_L,
                                                   estructuras="est.csv")))


def test_nuevo_proyecto_desde_plantilla(tmp_path):
    destino = tmp_path / "linea"
    assert main(["nuevo", str(destino)]) == 0
    assert {p.name for p in destino.iterdir()} == {"proyecto.json", "conductores.csv", "estructuras.csv"}
    # Con puntos y eje propios, la plantilla se calcula sin editar nada más
    st.malla(st.colina_gaussiana(), 10).to_csv(destino / "puntos.csv", index=False)
    doc = ezdxf.new()
    doc.modelspace().add_lwpolyline(st.eje_ejemplo().tolist())
    doc.saveas(destino / "eje.dxf")
    r = calcular(proyecto.cargar(destino / "proyecto.json"))
    assert len(r.estructuras) >= 2
    assert sum("PENDIENTE_REVISION" in a for a in r.avisos) == len(proyecto.PARAMETROS_NORMATIVOS)
    assert main(["nuevo", str(destino)]) == 0  # no sobrescribe
