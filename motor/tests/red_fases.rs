//! GridCraft con fases por tramo (modo REDCAD), red sintética calculada a mano:
//! I = P/(√3·V_ff), P/V_ff o P/V_fn con la tensión del extremo emisor; ΔV = K·I·L (en V
//! fase-neutro: ΔV_ff·V_fn/V_ff para 2 y 3 fases); pérdidas = fases·I²·R·L; AP: I = P/(V·fp),
//! pérdidas I²·R·L. Validado contra REDCAD con `comparar-redcad` (ver docs/PLAN_RUST.md).

use std::collections::HashMap;

use suite_motor::red::{calcular, ConductorRed, Nodo, Red};

fn cond(f: &str, fases: f64, k: f64, r: f64, k_ap: f64) -> ConductorRed {
    ConductorRed {
        formacion: f.into(),
        k_ohm_km: Some(k),
        r_ohm_km: Some(r),
        x_ohm_km: None,
        k_ap_ohm_km: Some(k_ap),
        r_ap_ohm_km: Some(if k_ap > 0.0 { r } else { 0.0 }),
        corriente_max_a: None,
        fuente: "prueba".into(),
        fases: Some(fases),
    }
}

fn nodo(n: &str, p: &str, l: f64, c: &str, lotes: f64, ce: f64, lamp: f64) -> Nodo {
    Nodo { nodo: n.into(), padre: p.into(), longitud_m: l, conductor: c.into(), lotes, kw_ce: ce, lamparas: lamp, ..Default::default() }
}

#[test]
fn red_mixta_trifasica_bifasica_monofasica() {
    let vff = 380.0;
    let vfn = vff / 3f64.sqrt();
    let red = Red {
        nombre: "prueba".into(),
        sistema: "3F".into(),
        tension_v: vff,
        nodos: vec![
            nodo("BT", "", 0.0, "", 0.0, 0.0, 0.0),
            nodo("T", "BT", 2.0, "2F", 0.0, 0.0, 0.0),
            nodo("A", "T", 40.0, "3F", 4.0, 1.5, 1.0),
            nodo("B", "A", 60.0, "1F", 2.0, 0.0, 1.0),
            nodo("C", "A", 30.0, "2F", 3.0, 0.0, 0.0),
        ],
        conductores: vec![cond("3F", 3.0, 3.553, 1.91, 3.535), cond("2F", 2.0, 4.098, 1.91, 3.53), cond("1F", 1.0, 3.53, 1.91, 0.0)],
        calificacion_kw_lote: 0.2,
        fs_sp: 1.0,
        fp_sp: 1.0,
        fs_ce: 1.0,
        kw_lampara: 0.06,
        fs_ap: 1.0,
        fp_ap: 0.9,
        tension_ap_v: None,
        tension_fn_v: None,
        caida_max_pct: 0.0,
        perdidas_max_pct: 0.0,
        factor_carga: 0.0,
        transformador_kva: 0.0,
        factor_sobrecarga: 1.0,
        tabla_fs: Vec::new(),
        fuentes: HashMap::new(),
    };
    let r = calcular(&red).unwrap();
    let t = |n: &str| r.tramos.iter().find(|x| x.nodo == n).unwrap();
    // a mano
    let (pb, pc) = (0.4, 0.6);
    let pa = 0.8 + 1.5 + pb + pc;
    // T (2 fases desde bornes)
    let i_t = pa * 1e3 / vff;
    let dv_t = 4.098 * i_t * 0.002 * vfn / vff;
    // A (3 fases desde T)
    let v_t = vfn - dv_t;
    let i_a = pa * 1e3 / (3f64.sqrt() * vff * v_t / vfn);
    let dv_a = 3.553 * i_a * 0.040 * vfn / vff;
    let v_a = v_t - dv_a;
    // B (1 fase desde A), C (2 fases desde A)
    let i_b = pb * 1e3 / v_a;
    let dv_b = 3.53 * i_b * 0.060;
    let i_c = pc * 1e3 / (vff * v_a / vfn);
    let dv_c = 4.098 * i_c * 0.030 * vfn / vff;
    let cerca = |o: f64, e: f64, d: f64, q: &str| assert!((o - e).abs() <= d, "{q}: {o} vs {e}");
    cerca(t("A").p_acum_kw, pa, 1e-9, "P acum A");
    for (n, i, dv, acum, perd) in [
        ("T", i_t, dv_t, dv_t, 2.0 * i_t * i_t * 1.91 * 0.002),
        ("A", i_a, dv_a, dv_t + dv_a, 3.0 * i_a * i_a * 1.91 * 0.040),
        ("B", i_b, dv_b, dv_t + dv_a + dv_b, i_b * i_b * 1.91 * 0.060),
        ("C", i_c, dv_c, dv_t + dv_a + dv_c, 2.0 * i_c * i_c * 1.91 * 0.030),
    ] {
        cerca(t(n).i_a, i, 0.005, &format!("I {n}"));
        cerca(t(n).dv_tramo_v, dv, 0.0005, &format!("ΔV {n}"));
        cerca(t(n).dv_acum_v, acum, 0.0005, &format!("ΔV acum {n}"));
        cerca(t(n).dv_pct, 100.0 * acum / vfn, 0.0005, &format!("%ΔV {n}"));
        cerca(t(n).perdidas_w, perd, 0.005, &format!("pérdidas {n}"));
    }
    // alumbrado: B sin conductor de AP (K = 0): su lámpara no carga la fase ni da caída
    let ap_a = t("A").ap.unwrap();
    cerca(ap_a.0, 0.12, 1e-9, "P AP A");
    let i_ap_a = 120.0 / ((vfn - 3.53 * (0.12e3 / (vfn * 0.9)) * 0.002) * 0.9);
    cerca(ap_a.1, i_ap_a, 0.005, "I AP A");
    assert_eq!(t("B").ap.unwrap().2, t("A").ap.unwrap().2, "sin caída de AP en un tramo sin conductor de AP");
    // modo por tramo: si una formación declara fases, todas deben hacerlo
    let mut mala = red.clone();
    mala.conductores[2].fases = None;
    assert!(calcular(&mala).unwrap_err().contains("sin 'fases'"));
}
