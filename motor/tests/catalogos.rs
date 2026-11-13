//! Asignación de armados/postes, verificación y metrado: Rust vs Python (`tests/catalogos_auto.txt`).

mod comun;

use std::collections::BTreeMap;

use suite_motor::catalogos::{elegir_poste, fuerzas_maximas, metrado, verificar, Catalogos};
use suite_motor::linea::calcular;

fn opt(s: &str) -> Option<f64> {
    if s == "-" { None } else { Some(comun::n(s)) }
}

#[test]
fn verificacion_y_metrado() {
    let mut caso = comun::leer(include_str!("linea_auto.txt"));
    caso.p.conductor.nombre = "AAAC 35".into();
    let mut r = calcular(&caso.p).unwrap();
    let texto = include_str!("catalogos_auto.txt");
    let postes: Vec<&str> = texto.lines().filter_map(|l| l.strip_prefix("postes ")).collect();
    let base = Catalogos::plantilla();
    let mut cat = Catalogos::desde_csv(&postes.join("\n"), "codigo\n", "codigo\n", "armado\n").unwrap();
    cat.armados = base.armados;
    cat.retenidas = base.retenidas;
    cat.materiales = base.materiales;

    // asignación con el mismo algoritmo que Python
    let fm = fuerzas_maximas(&r);
    let asig: Vec<(String, String)> = texto
        .lines()
        .filter_map(|l| l.strip_prefix("asig "))
        .map(|l| {
            let (a, p) = l.split_once(' ').unwrap();
            (a.to_string(), p.replace('_', " "))
        })
        .collect();
    for (i, e) in r.estructuras.iter_mut().enumerate() {
        e.armado = cat.elegir_armado(e.tipo, e.angulo);
        let (fh, mb, _) = fm.get(&e.numero).copied().unwrap_or((0.0, 0.0, 0.0));
        e.poste = elegir_poste(&cat, e, fh, mb, 2.0);
        assert_eq!(e.armado, asig[i].0, "armado E-{:02}", i + 1);
        if i != 2 {
            assert_eq!(e.poste, asig[i].1, "poste E-{:02}", i + 1);
        }
    }
    r.estructuras[2].poste = "M 12/C6".into();

    let ver = verificar(&r, &caso.p, &cat, 2.0, 2.0, None, &BTreeMap::new());
    let met = metrado(&r, &caso.p, &cat, 3.0, None, &BTreeMap::new());
    let mut iv = 0;
    let mut im = 0;
    let cerca = |o: Option<f64>, e: Option<f64>, tol: f64, etq: &str| match (o, e) {
        (None, None) => {}
        (Some(o), Some(e)) => assert!((o - e).abs() <= tol + 1e-9 * e.abs(), "{etq}: rust {o} python {e}"),
        _ => panic!("{etq}: rust {o:?} python {e:?}"),
    };
    let en_res = texto.split("== resultados").nth(1).unwrap();
    for l in en_res.lines().filter(|l| !l.is_empty()) {
        if let Some(rest) = l.strip_prefix("ver ") {
            let t: Vec<&str> = rest.split_whitespace().collect();
            let v = &ver[iv];
            assert_eq!(v.estructura, t[0]);
            cerca(Some(v.f_horizontal), opt(t[1]), 0.051, "F horizontal");
            cerca(v.f_punta.map(|x| (x * 10.0).round() / 10.0), opt(t[2]), 0.051, "F punta");
            cerca(v.mrn, opt(t[3]), 0.051, "MRN");
            cerca(v.fs_poste.filter(|x| x.is_finite()), opt(t[4]), 0.0051, "FS poste");
            assert_eq!(v.retenidas.to_string(), t[5]);
            cerca(v.tiro_por_retenida, opt(t[6]), 0.051, "tiro retenida");
            cerca(v.fs_retenida.filter(|x| x.is_finite()), opt(t[7]), 0.0051, "FS retenida");
            assert_eq!(if v.cumple { "SÍ" } else { "NO" }, t[8], "{}", v.observacion);
            iv += 1;
        } else if let Some(rest) = l.strip_prefix("met ") {
            let t: Vec<&str> = rest.split('|').collect();
            let m = &met[im];
            assert_eq!((m.item.as_str(), m.codigo.as_str(), m.descripcion.as_str(), m.unidad.as_str()), (t[0], t[1], t[2], t[3]));
            cerca(Some(m.cantidad), Some(comun::n(t[4])), 1e-9, &m.descripcion);
            im += 1;
        }
    }
    assert_eq!((iv, im), (ver.len(), met.len()));
}
