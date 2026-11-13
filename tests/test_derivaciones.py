"""Derivaciones: ramal calculado aparte y cargas sumadas en la estructura de derivación."""

import json
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from suite_electrica.linecraft import cli
from suite_electrica.linecraft.calculo import calcular
from suite_electrica.linecraft.derivaciones import calcular_red, cargar_red, eje_longitudinal

EJ = Path(__file__).resolve().parents[1] / "examples" / "linecraft"


def _proyecto_con_ramal(tmp_path, desde="E-03", largo=180.0):
    cfg = json.loads((EJ / "proyecto.json").read_text(encoding="utf-8"))
    base = (EJ / "proyecto.json").parent
    for k in ("puntos", "eje"):
        cfg["perfil"][k] = str((base / cfg["perfil"][k]).resolve())
    cfg["conductor"]["catalogo"] = str((base / cfg["conductor"]["catalogo"]).resolve())
    cfg["estructuras"] = str((base / cfg["estructuras"]).resolve()) if cfg.get("estructuras") else None
    ruta = tmp_path / "p.json"
    ruta.write_text(json.dumps(cfg), encoding="utf-8")
    principal, _, _ = cargar_red(ruta)
    r = calcular(principal)
    i = next(k for k, e in enumerate(r.estructuras) if e.codigo == desde)
    e = r.estructuras[i]
    u = eje_longitudinal(r, i)
    perp = np.array([-u[1], u[0]])
    pts = np.array([[e.este, e.norte], [e.este, e.norte] + largo * perp])
    pd.DataFrame(pts, columns=["este", "norte"]).to_csv(tmp_path / "ramal.csv", index=False)
    cfg["derivaciones"] = [{"nombre": "Ramal A", "desde": desde,
                            "perfil": {**cfg["perfil"], "eje": str(tmp_path / "ramal.csv")}}]
    ruta.write_text(json.dumps(cfg), encoding="utf-8")
    return ruta, r


def test_cargas_en_derivacion(tmp_path):
    ruta, r_sola = _proyecto_con_ramal(tmp_path)
    red = calcular_red(*cargar_red(ruta)[:2])
    assert set(red.ramales) == {"Ramal A"}
    d = red.derivaciones
    assert set(d["Estructura"]) == {"E-03"} and d["β (°)"].iloc[0] == pytest.approx(90, abs=1)
    num = next(e.numero for e in r_sola.estructuras if e.codigo == "E-03")
    antes = {c.hipotesis: c for c in r_sola.cargas if c.estructura == num}
    ramal0 = {c.hipotesis: c for c in red.ramales["Ramal A"].cargas if c.estructura == 1}
    for c in (c for c in red.principal.cargas if c.estructura == num):
        a, rr = antes[c.hipotesis], ramal0[c.hipotesis]
        # a 90°: el tiro del ramal es transversal para la principal
        assert c.transversal == pytest.approx(a.transversal + rr.longitudinal + rr.transversal, rel=1e-6)
        assert c.longitudinal == pytest.approx(a.longitudinal, abs=1e-6 * rr.longitudinal + 1e-9)
        assert c.vertical == pytest.approx(a.vertical + rr.vertical)
        assert c.tiro_retenida > 0  # la derivación exige retenida


def test_cli_y_errores(tmp_path):
    ruta, _ = _proyecto_con_ramal(tmp_path)
    out = tmp_path / "salidas"
    cli.main(["calcular", str(ruta), "--out", str(out)])
    assert (out / "derivaciones.xlsx").exists() and (out / "Ramal_A" / "planilla_estructuras.xlsx").exists()
    cfg = json.loads(ruta.read_text(encoding="utf-8"))
    cfg["derivaciones"][0]["desde"] = "E-02"  # el eje del ramal no empieza ahí
    ruta.write_text(json.dumps(cfg), encoding="utf-8")
    with pytest.raises(ValueError, match="empieza a"):
        calcular_red(*cargar_red(ruta)[:2])
