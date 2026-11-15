//! Cimentaciones por poste (`cimentacion`): volúmenes calculados a mano y avisos.

use suite_motor::cimentacion::{leer, metrado, volumenes, PLANTILLA};

const TABLA: &str = "soporte,forma,dimension_m,profundidad_m,solado_m,diam_base_m,relleno,fuente
8/200,cuadrada,0.6,1.3,0.1,0.25,compactado,Plano CIM-01 del proyecto
CAC 12/300,circular,1.0,2.0,0.2,0.3,concreto,EJEMPLO
9/300,cuadrada,0.8,1.5,0,,concreto,Plano CIM-01
";

fn cerca(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn volumenes_a_mano() {
    let t = leer(TABLA).unwrap();
    // 8/200: A = 0,36; exc = 0,468; solado = 0,036; poste = π·0,25²/4·1,2 = 0,058905;
    // relleno = 0,36·1,2 − 0,058905 = 0,373095; eliminación = 0,468 − 0,373095
    let p = std::f64::consts::PI * 0.0625 / 4.0 * 1.2;
    let v = volumenes(&t[0]);
    for (a, b) in v.iter().zip([0.468, 0.036, 0.0, 0.432 - p, 0.468 - (0.432 - p)]) {
        assert!(cerca(*a, b), "{v:?}");
    }
    // circular de 1 m: A = π/4; concreto = A·1,8 − π·0,09/4·1,8; nada vuelve al hoyo
    let a = std::f64::consts::PI / 4.0;
    let v = volumenes(&t[1]);
    assert!(cerca(v[0], 2.0 * a) && cerca(v[1], 0.2 * a) && cerca(v[2], 1.8 * a * (1.0 - 0.09)) && v[3] == 0.0 && cerca(v[4], v[0]));
}

#[test]
fn metrado_y_avisos() {
    let t = leer(TABLA).unwrap();
    let postes = vec![("8/200".to_string(), 10.0), ("CAC 12/300".to_string(), 2.0), ("8/400".to_string(), 3.0), ("9/300".into(), 1.0)];
    let (f, avisos) = metrado(&postes, &t);
    let q = |c: &str| f.iter().find(|x| x.codigo == c).map(|x| x.cantidad);
    let exc = 10.0 * 0.468 + 2.0 * 2.0 * std::f64::consts::PI / 4.0 + 0.96;
    assert!(cerca(q("CIM-EXC").unwrap(), (exc * 1000.0).round() / 1000.0));
    assert!(f.iter().all(|x| x.unidad == "m3"));
    assert_eq!(avisos.len(), 3, "{avisos:?}");
    assert!(avisos[0].contains("8/400"));
    assert!(avisos[1].contains("CAC 12/300") && !avisos[1].contains("8/200"));
    assert!(avisos[2].contains("9/300"));
    // sin postes con cimentación: sin filas
    assert!(metrado(&[("X".into(), 1.0)], &t).0.is_empty());
}

#[test]
fn plantilla_y_errores() {
    let t = leer(PLANTILLA).unwrap();
    assert!(!t.is_empty() && t.iter().all(|c| c.fuente.contains("EJEMPLO")), "la plantilla no trae valores normativos");
    assert!(leer("soporte,forma,dimension_m,profundidad_m\nA,triangular,1,1\n").is_err());
    assert!(leer("soporte,dimension_m\nA,1\n").unwrap_err().contains("profundidad_m"));
    assert!(leer("soporte,dimension_m,profundidad_m,solado_m\nA,1,0.5,0.6\n").is_err());
}
