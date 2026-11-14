//! Cruces y distancias de seguridad: el conductor a flecha máxima sobre el obstáculo, contra la
//! distancia mínima que ingresa el usuario; se guardan en el `.sel`.

use std::path::PathBuf;

use suite_motor::documento::Documento;
use suite_motor::geo::interp;
use suite_motor::salidas::curvas_vanos;

#[test]
fn cruces_verificados_y_guardados() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    let base = doc.calcular().unwrap();
    assert!(base.cruces.is_empty());
    let (r, p) = (&base.resultado, &base.proyecto);
    let s = (r.estructuras[0].progresiva + r.estructuras[1].progresiva) / 2.0;
    let (xs, zs) = &curvas_vanos(r, p, 0.0)[0];
    let libre = interp(s, xs, zs) - p.perfil.cota_en(s);
    // un obstáculo 1 m por debajo del límite y otro 1 m por encima
    let fila = |alto: f64, nombre: &str| vec![format!("{s}"), nombre.into(), format!("{alto}"), "2.5".into(), "CNE (tabla del cruce)".into()];
    doc.cruces = vec![fila(libre - 3.5, "camino"), fila(libre - 1.5, "cerco"), vec![String::new(); 5]];
    let c = doc.calcular().unwrap();
    assert_eq!(c.cruces.len(), 2);
    assert!(c.cruces[0].cumple && (c.cruces[0].distancia_m - 3.5).abs() < 1e-9);
    assert!(!c.cruces[1].cumple && (c.cruces[1].distancia_m - 1.5).abs() < 1e-9);
    assert_eq!(c.alertas.iter().filter(|a| a.codigo == "DISTANCIA_SEGURIDAD").count(), 1);
    assert!(!c.cumple());

    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cruces.sel");
    doc.guardar(Some(&tmp)).unwrap();
    assert_eq!(Documento::abrir(&tmp).unwrap().cruces, doc.cruces[..2].iter().cloned().chain([vec![String::new(); 5]]).collect::<Vec<_>>());

    doc.cruces = vec![vec!["99999".into(), "fuera".into(), "1".into(), "1".into(), String::new()]];
    assert!(doc.calcular().unwrap_err().contains("fuera de la línea"));
}
