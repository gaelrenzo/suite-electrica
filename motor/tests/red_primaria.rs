//! Red primaria (MT) con el motor de GridCraft: la plantilla de 22,9 kV se calcula y la caída del
//! primer tramo coincide con el cálculo a mano ΔV = I·(r cos φ + x sen φ)·L, % de V/√3.

use std::path::PathBuf;

use suite_motor::red::{calcular, cargar};

#[test]
fn plantilla_mt_y_caida_a_mano() {
    let pl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data").join("plantillas");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("red_mt");
    std::fs::create_dir_all(&dir).unwrap();
    for (de, a) in [("red_mt_proyecto.json", "red.json"), ("red_mt_nodos.csv", "nodos.csv"), ("conductores_mt.csv", "conductores_mt.csv")] {
        std::fs::copy(pl.join(de), dir.join(a)).unwrap();
    }
    let red = cargar(&dir.join("red.json")).unwrap();
    let r = calcular(&red).unwrap();
    assert!(r.alertas.is_empty(), "{:?}", r.alertas);
    // tramo PMI -> D1: lleva toda la carga (22,5 + 45 + 13,5 + 22,5 = 103,5 kW, fp 0,9), AAAC 35, 1,5 km
    let t = r.tramos.iter().find(|t| t.nodo == "D1").unwrap();
    let (p, v, fp) = (103.5e3, 22_900.0, 0.9_f64);
    let i = p / (3f64.sqrt() * v * fp);
    let k = 0.9371 * fp + 0.4591 * (1.0 - fp * fp).sqrt();
    let dv = i * k * 1.5;
    assert!((t.i_a - i).abs() < 0.01, "I {} vs {i}", t.i_a);
    assert!((t.dv_tramo_v - dv).abs() < 0.01, "ΔV {} vs {dv}", t.dv_tramo_v);
    assert!((t.dv_pct - 100.0 * dv / (v / 3f64.sqrt())).abs() < 0.001);
    // la caída crece aguas abajo y queda muy por debajo del 5 % con estas cargas
    let max = r.tramos.iter().map(|t| t.dv_pct).fold(0.0, f64::max);
    assert!(max > t.dv_pct && max < 1.0, "{max}");
}
