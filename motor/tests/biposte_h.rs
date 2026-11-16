//! Biposte en H (Norma DGE RD 024-2003, láminas 027-030): catálogo, momentos por poste y flujo
//! completo (verificación con la carga repartida y dos postes por estructura en el metrado).

use std::path::PathBuf;

use suite_motor::catalogos::{momentos_por_poste, momentos_poste, n_postes, Catalogos, Poste};
use suite_motor::documento::Documento;
use suite_motor::linea::{Cargas, Tipo};

const H: [&str; 7] = ["PSH-3", "PSH-3N", "PA1H-3", "PA1H-3N", "PRH-3", "PRH-3N", "PA2H-3"];

#[test]
fn catalogo_y_eleccion() {
    let cat = Catalogos::plantilla();
    for c in H {
        let a = cat.armado(c).unwrap_or_else(|| panic!("falta {c}"));
        assert_eq!(a.n_postes(), 2.0, "{c}");
        assert_eq!(a.alturas_fases_m, "0.225;1.425;1.425", "{c}");
        assert_eq!(a.distancia_punta_amarre_m, Some(if a.neutro { 2.625 } else { 1.425 }), "{c}");
        assert!(cat.materiales.iter().filter(|m| m.armado == c).count() >= 8, "{c}: materiales");
    }
    // un armado de un poste
    assert_eq!(n_postes(cat.armado("PS1-3")), 1.0);
    assert_eq!(n_postes(None), 1.0);
    // la elección automática no los toma (se asignan a mano)
    for (f, n, t, a) in [(3, false, Tipo::Suspension, 0.0), (3, true, Tipo::Angulo, 20.0), (3, false, Tipo::Anclaje, 0.0)] {
        assert!(!cat.elegir_armado_sistema(t, a, f, n).contains('H'));
    }
    // cantidades transcritas de las láminas
    let q = |a: &str, cod: &str| cat.materiales.iter().find(|m| m.armado == a && m.material.starts_with(&format!("{cod} "))).and_then(|m| m.cantidad);
    assert_eq!(q("PSH-3N", "a2"), Some(4.0));
    assert_eq!(q("PA1H-3", "a2"), Some(6.0));
    assert_eq!(q("PRH-3N", "k1"), Some(14.0));
    assert_eq!(q("PRH-3", "d1"), Some(20.0));
    assert_eq!(q("PA2H-3", "9"), Some(6.0));
}

#[test]
fn momentos_repartidos() {
    let c = Cargas { transversal: 3000.0, viento_conductor: 1200.0, longitudinal: 800.0, presion_viento: 400.0, ..Default::default() };
    let p = Poste {
        codigo: "M".into(),
        longitud_m: Some(12.0),
        empotramiento_m: Some(1.8),
        diam_punta_m: Some(0.16),
        diam_tierra_m: Some(0.28),
        mor_mpa: Some(55.0),
        ..Default::default()
    };
    let alt = [10.0, 9.0, 9.0];
    let uno = momentos_poste(&c, &alt, &p, None);
    let dos = momentos_por_poste(&c, &alt, &p, None, 2.0);
    for (a, b) in [(uno.mvc, dos.mvc), (uno.mtb, dos.mtb), (uno.mtd, dos.mtd)] {
        assert!((a / 2.0 - b).abs() < 1e-9);
    }
    assert_eq!(uno.mve, dos.mve, "el viento sobre cada poste no se reparte");
    assert!((dos.mrn - ((uno.mvc + uno.mtb).hypot(uno.mtd) / 2.0 + uno.mve)).abs() < 1e-9);
}

#[test]
fn linea_con_biposte() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let base = || {
        let mut doc = Documento::abrir(&sel).unwrap();
        doc.catalogos = Documento::nuevo().catalogos;
        doc.linea.fases = 3;
        doc.linea.neutro = false;
        doc.linea.n_conductores = 3;
        doc.calcular().unwrap();
        for e in doc.estructuras.iter_mut() {
            e.poste = "CAC 12/300".into();
        }
        doc
    };
    let mut simple = base();
    let c1 = simple.calcular().unwrap();
    let mut h = base();
    let susp: Vec<usize> = (0..h.estructuras.len()).filter(|&i| h.estructuras[i].armado == "PS1-3").collect();
    assert!(!susp.is_empty());
    for &i in &susp {
        h.estructuras[i].armado = "PSH-3".into();
    }
    let c2 = h.calcular().unwrap();
    // dos postes por estructura H en el metrado
    let postes = |c: &suite_motor::documento::Calculo| c.metrado.iter().filter(|m| m.item == "Poste").map(|m| m.cantidad).sum::<f64>();
    assert_eq!(postes(&c2), postes(&c1) + susp.len() as f64);
    assert!(c2.metrado.iter().any(|m| m.item == "Armado" && m.codigo == "PSH-3"));
    // cada poste toma la mitad: el FS del poste de la H es mayor
    for &i in &susp {
        let (a, b) = (&c1.verificacion[i], &c2.verificacion[i]);
        if let (Some(f1), Some(f2)) = (a.fs_poste, b.fs_poste) {
            if f1.is_finite() {
                assert!(f2 > 1.5 * f1, "{}: FS {f1} -> {f2}", a.estructura);
            }
        }
        assert!(b.observacion.contains("2 postes"), "{}", b.observacion);
    }
}

#[test]
fn plantillas_con_columnas_fijas() {
    // un campo con coma sin comillas desplaza las columnas (pandas lo rechaza)
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("data").join("plantillas");
    for (archivo, n) in [("materiales.csv", 5), ("armados.csv", 14)] {
        let t = std::fs::read_to_string(base.join(archivo)).unwrap();
        let limpio = t.lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>().join("\n");
        let mut rd = csv::ReaderBuilder::new().flexible(true).from_reader(limpio.as_bytes());
        for (k, r) in rd.records().enumerate() {
            let r = r.unwrap();
            assert!(r.len() == n || (archivo == "armados.csv" && r.len() == n - 1), "{archivo} fila {}: {} campos", k + 2, r.len());
        }
    }
}
