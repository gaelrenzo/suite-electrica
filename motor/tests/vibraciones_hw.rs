//! Vibraciones eólicas, criterio H/w (CIGRE TB 273): H de la condición de EDS / peso por metro,
//! contra el límite que pone el usuario; 0 = no se verifica.

use std::path::PathBuf;

use suite_motor::documento::Documento;

#[test]
fn alerta_por_parametro_de_catenaria() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    let c = doc.calcular().unwrap();
    let hw = |c: &suite_motor::documento::Calculo| c.alertas.iter().filter(|a| a.detalle.contains("H/w")).count();
    assert_eq!(hw(&c), 0, "sin límite no se verifica");
    // H/w real del primer tramo en la hipótesis inicial
    let h = c.resultado.tramos[0].estado(&c.proyecto.hip_inicial).unwrap().h;
    let real = h / c.proyecto.conductor.peso;
    assert!(real > 100.0 && real < 5000.0, "{real}");

    doc.linea.hw_max_m = real - 50.0;
    let c = doc.calcular().unwrap();
    assert!(hw(&c) >= 1);
    let a = c.alertas.iter().find(|a| a.detalle.contains("H/w")).unwrap();
    assert!(a.detalle.contains("L·D/m") && a.codigo == "V");
    assert!(c.resultado.avisos.iter().any(|x| x.contains("hw_max_m")), "sin fuente: aviso");

    doc.linea.hw_max_m = real + 50.0;
    assert_eq!(hw(&doc.calcular().unwrap()), 0);

    // se guarda en el .sel
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("hw.sel");
    doc.guardar(Some(&tmp)).unwrap();
    assert_eq!(Documento::abrir(&tmp).unwrap().linea.hw_max_m, real + 50.0);
}
