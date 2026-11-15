//! Autodiseño de conductores BT (`red::autodisenar`). Catálogo y red sintéticos (K y R de
//! prueba, no de catálogo comercial).

use std::collections::HashMap;

use suite_motor::red::{autodisenar, calcular, ConductorRed, Nodo, Red};

fn cond(f: &str, k: f64, imax: Option<f64>, fases: Option<f64>) -> ConductorRed {
    ConductorRed {
        formacion: f.into(),
        k_ohm_km: Some(k),
        r_ohm_km: Some(k * 0.55),
        x_ohm_km: None,
        k_ap_ohm_km: None,
        r_ap_ohm_km: None,
        corriente_max_a: imax,
        fuente: "prueba".into(),
        fases,
    }
}

fn nodo(n: &str, p: &str, l: f64, c: &str, lotes: f64) -> Nodo {
    Nodo { nodo: n.into(), padre: p.into(), longitud_m: l, conductor: c.into(), lotes, ..Default::default() }
}

fn red(conductores: Vec<ConductorRed>, nodos: Vec<Nodo>, caida: f64) -> Red {
    Red {
        nombre: "auto".into(),
        sistema: "3F".into(),
        tension_v: 380.0,
        nodos,
        conductores,
        calificacion_kw_lote: 0.6,
        fs_sp: 1.0,
        fp_sp: 1.0,
        fs_ce: 1.0,
        kw_lampara: 0.0,
        fs_ap: 1.0,
        fp_ap: 1.0,
        tension_ap_v: None,
        tension_fn_v: None,
        caida_max_pct: caida,
        perdidas_max_pct: 0.0,
        factor_carga: 0.0,
        transformador_kva: 0.0,
        factor_sobrecarga: 1.0,
        tabla_fs: Vec::new(),
        fuentes: HashMap::new(),
    }
}

fn catalogo() -> Vec<ConductorRed> {
    // orden del archivo distinto del de K, a propósito
    vec![cond("C25", 2.3, Some(90.0), None), cond("C16", 3.5, Some(60.0), None), cond("C50", 1.2, Some(250.0), None), cond("C35", 1.6, Some(150.0), None)]
}

fn troncal() -> Vec<Nodo> {
    let mut v = vec![nodo("SED", "", 0.0, "", 0.0)];
    let mut padre = "SED".to_string();
    for k in 1..=6 {
        let n = format!("{k:02}");
        v.push(nodo(&n, &padre, 70.0, "C50", 8.0));
        padre = n;
    }
    v.push(nodo("R1", "02", 60.0, "C50", 6.0));
    v.push(nodo("R2", "R1", 60.0, "C50", 6.0));
    v
}

#[test]
fn cumple_y_es_minimo_local() {
    let base = red(catalogo(), troncal(), 5.0);
    let a = autodisenar(&base, true).unwrap();
    assert!(a.resultado.cumple(), "{:?}", a.resultado.alertas);
    assert!(a.avisos.is_empty(), "{:?}", a.avisos);
    assert!(!a.cambios.is_empty());
    // el extremo de la troncal sale con la formación más chica y la salida de la SED no
    assert_eq!(a.red.nodos[6].conductor, "C16");
    assert_ne!(a.red.nodos[1].conductor, "C16");
    minimo_local(&a.red);
    // partir de la actual (todo C50) también cumple y es mínimo local (puede ser otro)
    let b = autodisenar(&base, false).unwrap();
    assert!(b.resultado.cumple());
    minimo_local(&b.red);
}

/// Achicar cualquier tramo un paso hace que la red no cumpla.
fn minimo_local(red: &Red) {
    let orden = ["C16", "C25", "C35", "C50"];
    for i in 1..red.nodos.len() {
        let p = orden.iter().position(|c| *c == red.nodos[i].conductor).unwrap();
        if p == 0 {
            continue;
        }
        let mut r = red.clone();
        r.nodos[i].conductor = orden[p - 1].into();
        assert!(!calcular(&r).unwrap().cumple(), "{} se podía achicar", r.nodos[i].nodo);
    }
}

#[test]
fn corriente_maxima_y_caso_imposible() {
    // el C35 admite 150 A: con mucha carga en la salida no alcanza y pasa a C50
    let mut nodos = troncal();
    nodos[1].lotes = 120.0;
    let a = autodisenar(&red(catalogo(), nodos, 50.0), true).unwrap();
    assert!(a.resultado.cumple(), "{:?}", a.resultado.alertas);
    let t = a.resultado.tramos.iter().find(|t| t.nodo == "01").unwrap();
    assert_eq!(a.red.nodos[1].conductor, "C50", "I = {:.1} A", t.i_a);
    // caída imposible: avisa y no cumple
    let b = autodisenar(&red(catalogo(), troncal(), 0.05), true).unwrap();
    assert!(!b.resultado.cumple());
    assert!(b.avisos.iter().any(|x| x.contains("No se puede corregir")), "{:?}", b.avisos);
}

#[test]
fn no_mezcla_fases() {
    let cat = vec![
        cond("3x16", 3.5, None, Some(3.0)),
        cond("3x25", 2.3, None, Some(3.0)),
        cond("1x16", 3.6, None, Some(1.0)),
    ];
    let nodos = vec![
        nodo("SED", "", 0.0, "", 0.0),
        nodo("01", "SED", 200.0, "3x16", 20.0),
        nodo("02", "01", 150.0, "1x16", 4.0),
    ];
    let a = autodisenar(&red(cat, nodos, 4.0), true).unwrap();
    assert_eq!(a.red.nodos[2].conductor, "1x16");
    assert!(a.red.nodos[1].conductor.starts_with('3'));
}
