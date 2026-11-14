//! Etapa 1e: alertas, tabla de flechado y derivaciones — Rust vs Python.

mod comun;

use std::collections::BTreeMap;

use suite_motor::alertas::{alertas, ConfigAlertas};
use suite_motor::catalogos::{elegir_poste, fuerzas_maximas, verificar, Catalogos};
use suite_motor::derivaciones::combinar;
use suite_motor::flechado::tabla_flechado;
use suite_motor::linea::calcular;
use suite_motor::mecanica::Modelo;

#[test]
fn alertas_todas_las_clases() {
    let caso = comun::leer(include_str!("linea_auto.txt"));
    let mut r = calcular(&caso.p).unwrap();
    let postes: Vec<&str> = include_str!("catalogos_auto.txt").lines().filter_map(|l| l.strip_prefix("postes ")).collect();
    let base = Catalogos::plantilla();
    let mut cat = Catalogos::desde_csv(&postes.join("\n"), "codigo\n", "codigo\n", "armado\n").unwrap();
    cat.armados = base.armados;
    cat.retenidas = base.retenidas;
    let fm = fuerzas_maximas(&r);
    for e in r.estructuras.iter_mut() {
        e.armado = cat.elegir_armado(e.tipo, e.angulo);
        let (fh, mb, _) = fm.get(&e.numero).copied().unwrap_or((0.0, 0.0, 0.0));
        e.poste = elegir_poste(&cat, e, fh, mb, 2.0);
    }
    r.estructuras[2].poste = "M 12/C6".into();
    let ver = verificar(&r, &caso.p, &cat, 2.0, 2.0, None, &BTreeMap::new());
    let cfg = ConfigAlertas {
        fs_poste: 4.0,
        fs_retenida: 3.0,
        tension_kv: 22.9,
        sep_k_tension: 0.04,
        sep_k_flecha: 0.3,
        angulo_salida_min: -20.0,
        angulo_salida_max: 5.0,
        tiro_max_espiga: 2000.0,
        eds_max_pct: 10.0,
    };
    let a = alertas(&r, &caso.p, Some(&cat), &ver, &cfg);
    let esperado: Vec<Vec<&str>> =
        include_str!("alertas_auto.txt").lines().map(|l| l.split_whitespace().collect()).collect();
    assert_eq!(a.len(), esperado.len(), "rust: {a:#?}");
    for (x, e) in a.iter().zip(&esperado) {
        assert_eq!(x.elemento, e[1].replace('_', " "));
        assert_eq!(x.codigo, e[2], "{}", x.elemento);
        let ex = comun::n(e[3]);
        assert!(x.exceso == ex || (x.exceso - ex).abs() < 1e-9, "{} {}: rust {} python {ex}", x.elemento, x.codigo, x.exceso);
        assert_eq!(x.critica, e[4] == "True", "{} {}", x.elemento, x.codigo);
    }
}

fn comparar_flechado(texto_linea: &str, texto_ref: &str, parabola: bool) {
    let mut caso = comun::leer(texto_linea);
    if parabola {
        caso.p.modelo = Modelo::Parabola;
    }
    let r = calcular(&caso.p).unwrap();
    let t = tabla_flechado(&r, &caso.p, 20.0, 5.0).unwrap();
    let filas: Vec<&str> = texto_ref.lines().collect();
    assert_eq!(t.len(), filas.len());
    for (f, l) in t.iter().zip(filas) {
        let p: Vec<&str> = l.split_whitespace().collect();
        assert_eq!((f.tramo.as_str(), f.vano.as_str()), (p[1], p[2]));
        assert_eq!(f.valores.len(), p.len() - 3);
        for ((tt, h, fl), v) in f.valores.iter().zip(&p[3..]) {
            let q: Vec<f64> = v.split(':').map(comun::n).collect();
            assert!((tt - q[0]).abs() < 1e-9);
            assert!((h - q[1]).abs() <= 0.0051, "{} {tt}°C H: rust {h} python {}", f.vano, q[1]);
            assert!((fl - q[2]).abs() <= 0.00051, "{} {tt}°C flecha: rust {fl} python {}", f.vano, q[2]);
        }
    }
}

#[test]
fn flechado() {
    comparar_flechado(include_str!("linea_auto.txt"), include_str!("flechado_auto.txt"), false);
    comparar_flechado(include_str!("linea_manual.txt"), include_str!("flechado_manual.txt"), true);
}

#[test]
fn derivacion() {
    let principal = comun::leer(include_str!("linea_auto.txt"));
    let ramal = comun::leer(include_str!("linea_ramal.txt"));
    let mut rp = calcular(&principal.p).unwrap();
    let rr = calcular(&ramal.p).unwrap();
    let filas = combinar(&mut rp, &principal.p, &rr, "Ramal A", "E-03").unwrap();
    let texto = include_str!("derivacion.txt");
    let der: Vec<Vec<&str>> = texto.lines().filter(|l| l.starts_with("der ")).map(|l| l.split_whitespace().collect()).collect();
    assert_eq!(filas.len(), der.len());
    let cerca = |o: f64, e: &str, etq: &str| {
        let e = comun::n(e);
        assert!((o - e).abs() <= 1e-6 * e.abs().max(1.0), "{etq}: rust {o} python {e}");
    };
    // la tabla de Python está redondeada a 1 decimal
    let r1 = |o: f64, e: &str, etq: &str| {
        let e = comun::n(e);
        assert!((o - e).abs() <= 0.051, "{etq}: rust {o} python {e}");
    };
    for (f, d) in filas.iter().zip(&der) {
        assert_eq!(f.hipotesis, d[1]);
        r1(f.beta, d[2], "beta");
        r1(f.mas_longitudinal, d[3], "+L");
        r1(f.mas_transversal, d[4], "+T");
        r1(f.mas_vertical, d[5], "+V");
        r1(f.horizontal_total, d[6], "Fh");
    }
    let num = rp.estructuras.iter().find(|e| e.codigo() == "E-03").unwrap().numero;
    let cargas: Vec<_> = rp.cargas.iter().filter(|c| c.estructura == num).collect();
    for (c, l) in cargas.iter().zip(texto.lines().filter(|l| l.starts_with("carga "))) {
        let t: Vec<&str> = l.split_whitespace().collect();
        assert_eq!(c.hipotesis, t[1]);
        cerca(c.transversal, t[2], "T");
        cerca(c.longitudinal, t[3], "L");
        cerca(c.vertical, t[4], "V");
        cerca(c.horizontal, t[5], "H");
        cerca(c.tiro_retenida, t[6], "ret");
    }
}
