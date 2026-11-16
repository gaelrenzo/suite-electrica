//! Amortiguadores por vano: la cantidad por conductor sale de la tabla del proyecto según la
//! longitud de cada vano y se suma al metrado; se guarda en el `.sel`.

use std::path::PathBuf;

use suite_motor::documento::Documento;

#[test]
fn metrado_de_amortiguadores() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    let base = doc.calcular().unwrap();
    assert!(!base.metrado.iter().any(|f| f.codigo == "AMORT"), "sin tabla no se metran");
    let n = base.proyecto.n_conductores as f64;
    let largos: Vec<f64> = base.resultado.vanos.iter().map(|v| v.longitud).collect();
    // dos rangos: hasta 100 m → 0; hasta 1000 m → 2 por conductor
    doc.amortiguadores = vec![
        vec!["1000".into(), "2".into(), "Fabricante X".into()],
        vec!["100".into(), "0".into(), "Fabricante X".into()],
    ];
    let c = doc.calcular().unwrap();
    let esperado: f64 = largos.iter().map(|l| if *l <= 100.0 { 0.0 } else { 2.0 * n }).sum();
    let f = c.metrado.iter().find(|f| f.codigo == "AMORT").unwrap();
    assert_eq!(f.cantidad, esperado);
    assert!(!c.resultado.avisos.iter().any(|a| a.contains("amortiguadores")), "con fuente no avisa");
    // guardar y abrir
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("amort.sel");
    doc.guardar(Some(&tmp)).unwrap();
    assert_eq!(Documento::abrir(&tmp).unwrap().amortiguadores, doc.amortiguadores);
    // un vano más largo que la tabla: error claro
    doc.amortiguadores = vec![vec!["10".into(), "1".into(), String::new()]];
    assert!(doc.calcular().unwrap_err().contains("supera"));
}
