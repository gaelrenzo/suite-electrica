//! Elección de poste por momento de vuelco (MRN, criterio de REDLIN): el poste elegido cumple
//! `FS ≥ fs_poste` en la verificación y ningún poste más chico del catálogo habría cumplido.
//! (Función nueva de la versión Rust; la fórmula de MRN/FS está validada contra REDLIN con
//! `comparar-redlin --postes`.)

mod comun;

use std::collections::BTreeMap;

use suite_motor::catalogos::{elegir_poste, elegir_poste_mrn, fs_momentos, fuerzas_maximas, verificar, Catalogos};
use suite_motor::linea::calcular;

/// Momento resistente en la línea de tierra (N·m).
fn resistente(q: &suite_motor::catalogos::Poste) -> f64 {
    match (q.mor_mpa, q.diam_tierra_m) {
        (Some(m), Some(d)) => m * 1e6 * std::f64::consts::PI * d.powi(3) / 32.0,
        _ => q.carga_nominal_dan.unwrap() * 10.0 * (q.longitud_m.unwrap() - q.empotramiento_m.unwrap() - q.dist_aplicacion_m.unwrap_or(0.0)),
    }
}

#[test]
fn poste_minimo_que_cumple_por_momentos() {
    assert_eq!(caso(2.0), 0, "con FS 2 el catálogo de prueba es holgado: mismo poste que antes");
    assert!(caso(4.0) > 0, "con FS 4 el criterio por momentos debe cambiar algún poste");
}

fn caso(fs: f64) -> usize {
    let caso = comun::leer(include_str!("linea_auto.txt"));
    let p = caso.p;
    let mut r = calcular(&p).unwrap();
    let postes: Vec<&str> = include_str!("catalogos_auto.txt").lines().filter_map(|l| l.strip_prefix("postes ")).collect();
    let base = Catalogos::plantilla();
    let mut cat = Catalogos::desde_csv(&postes.join("\n"), "codigo\n", "codigo\n", "armado\n").unwrap();
    cat.armados = base.armados;
    cat.retenidas = base.retenidas;
    let fm = fuerzas_maximas(&r);
    let mut cambios = 0;
    for i in 0..r.estructuras.len() {
        let e = r.estructuras[i].clone();
        let armado = cat.elegir_armado(e.tipo, e.angulo);
        let arm = cat.armado(&armado);
        let cargas: Vec<_> = r.cargas.iter().filter(|c| c.estructura == e.numero).collect();
        let (fh, mb, _) = fm.get(&e.numero).copied().unwrap_or((0.0, 0.0, 0.0));
        let nuevo = elegir_poste_mrn(&cat, &e, &cargas, arm, p.n_conductores, fh, mb, fs);
        let viejo = elegir_poste(&cat, &e, fh, mb, fs);
        assert!(!nuevo.is_empty(), "{}: algún poste del catálogo cumple", e.codigo());
        let elegido = cat.poste(&nuevo).unwrap();
        let fs_el = fs_momentos(&e, &cargas, elegido, arm, p.n_conductores);
        assert!(fs_el >= fs, "{}: {nuevo} FS {fs_el}", e.codigo());
        // mínimo: ningún poste con menor momento resistente cumple
        for q in cat.postes.iter().filter(|q| q.codigo != nuevo) {
            let cumple = fs_momentos(&e, &cargas, q, arm, p.n_conductores) >= fs && q.longitud_m.map_or(true, |l| e.altura_amarre < l);
            assert!(!(cumple && resistente(q) < resistente(elegido)), "{}: {} cumple y es menor que {nuevo}", e.codigo(), q.codigo);
        }
        if nuevo != viejo {
            cambios += 1;
            println!("FS {fs} {} {}: punta → {viejo}, momentos → {nuevo} (FS {fs_el:.2})", e.codigo(), e.tipo.como_str());
        }
        r.estructuras[i].armado = armado;
        r.estructuras[i].poste = nuevo;
    }
    let ver = verificar(&r, &p, &cat, fs, fs, None, &BTreeMap::new());
    for v in &ver {
        assert!(!v.observacion.contains("FS poste"), "{}: {}", v.estructura, v.observacion);
        assert!(v.fs_poste.is_some_and(|x| x >= fs), "{}: FS {:?}", v.estructura, v.fs_poste);
    }
    println!("FS {fs}: {cambios} estructura(s) cambian de poste respecto del criterio de fuerza en la punta");
    cambios
}
