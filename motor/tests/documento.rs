//! Documento `.sel` y flujo completo de la app: Rust vs Python (`tests/documento.txt`).
//!
//! Mismo `.sel` (`tests/archivos/documento_irregular.sel`), mismo cálculo (ubicación automática,
//! catálogos, verificación, metrado, alertas, avisos) y misma secuencia de edición.

use std::path::PathBuf;

use suite_motor::documento::{Calculo, Documento};

fn f(x: Option<f64>) -> String {
    match x {
        None => "-".into(),
        Some(v) if v.is_nan() => "-".into(),
        Some(v) if v.is_infinite() => "inf".into(),
        Some(v) => format!("{v:.17e}"),
    }
}

fn guion(s: &str) -> String {
    if s.is_empty() { "-".into() } else { s.replace(' ', "_") }
}

fn volcar(doc: &Documento, c: &Calculo, etiqueta: &str) -> Vec<String> {
    let r = &c.resultado;
    let mut l = vec![format!("== {etiqueta} cumple={}", if c.cumple() { "True" } else { "False" })];
    for e in &doc.estructuras {
        l.push(format!("est {} {} {} {} {} {}", f(Some(e.progresiva_m)), guion(&e.tipo), guion(&e.armado), guion(&e.poste),
            f(e.altura_amarre_m), f(e.retenidas)));
    }
    for e in &r.estructuras {
        l.push(format!("res {} {} {} {} {}", e.codigo(), e.tipo.como_str(), f(Some(e.progresiva)), f(Some(e.altura_amarre)), f(Some(e.angulo))));
    }
    for v in &c.verificacion {
        l.push(format!("ver {} {}", v.estructura, if v.cumple { "SÍ" } else { "NO" }));
    }
    for a in &c.alertas {
        l.push(format!("alerta {} {} {}", a.elemento.replace(' ', "_"), a.codigo, f(Some(a.exceso))));
    }
    for m in &c.metrado {
        l.push(format!("met {}|{}|{}|{}", m.item, m.codigo, m.unidad, f(Some(m.cantidad))));
    }
    for a in &r.avisos {
        l.push(format!("aviso {a}"));
    }
    l
}

fn iguales(a: &str, b: &str) -> bool {
    let (ta, tb): (Vec<&str>, Vec<&str>) = (a.split(' ').collect(), b.split(' ').collect());
    ta.len() == tb.len()
        && ta.iter().zip(&tb).all(|(x, y)| {
            if x == y {
                return true;
            }
            // números (también dentro de met a|b|c|n)
            let (px, py): (Vec<&str>, Vec<&str>) = (x.split('|').collect(), y.split('|').collect());
            px.len() == py.len()
                && px.iter().zip(&py).all(|(u, v)| {
                    u == v || match (u.parse::<f64>(), v.parse::<f64>()) {
                        (Ok(p), Ok(q)) => (p - q).abs() <= 1e-6 * q.abs().max(1.0),
                        _ => false,
                    }
                })
        })
}

#[test]
fn flujo_completo_como_la_app() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    let c = doc.calcular().unwrap();
    let mut l = volcar(&doc, &c, "calculo");
    let s_mid = (doc.estructuras[1].progresiva_m + doc.estructuras[2].progresiva_m) / 2.0;
    let i = doc.agregar_estructura(s_mid).unwrap();
    l.push(format!("agregar {} {i}", f(Some(s_mid))));
    let s = doc.mover_estructura(i, s_mid + 7.3).unwrap();
    l.push(format!("mover {i} {}", f(Some(s))));
    doc.cambiar_estructura(i, "tipo", "ANCLAJE").unwrap();
    doc.cambiar_estructura(i, "altura_amarre_m", "12").unwrap();
    doc.cambiar_estructura(1, "poste", "").unwrap();
    let c = doc.calcular().unwrap();
    l.extend(volcar(&doc, &c, "editado"));
    doc.eliminar_estructura(i).unwrap();
    doc.deshacer();
    l.push(format!("deshacer {}", doc.estructuras.len()));
    let c = doc.asignar_catalogos(true).unwrap();
    l.extend(volcar(&doc, &c, "asignado"));

    let esperado: Vec<&str> = include_str!("documento.txt").lines().filter(|x| !x.is_empty()).collect();
    for (k, (a, b)) in l.iter().zip(&esperado).enumerate() {
        assert!(iguales(a, b), "línea {}:\n  rust   {a}\n  python {b}", k + 1);
    }
    assert_eq!(l.len(), esperado.len(), "cantidad de líneas");

    // guardar y reabrir conserva todo; Python lo abre (ver generar_referencia_sel.py)
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("documento_rust.sel");
    doc.guardar(Some(&tmp)).unwrap();
    let otro = Documento::abrir(&tmp).unwrap();
    assert_eq!(otro.a_json(), doc.a_json());
}

#[test]
fn abre_sel_de_python_y_nuevo() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("proyecto_python.sel");
    let doc = Documento::abrir(&sel).unwrap();
    assert_eq!(doc.linea.estado_final_vanos, "por vano");
    assert_eq!(doc.conductor.curva_creep, vec![0.0, 40.0]);
    assert!(doc.puntos[3].cota.is_nan());
    let n = Documento::nuevo();
    assert_eq!(n.hipotesis.len(), 5);
    assert!(n.catalogos_obj().unwrap().postes.len() > 1);
    assert!(n.fuente("factor_seguridad_poste").starts_with("PENDIENTE"));
}
