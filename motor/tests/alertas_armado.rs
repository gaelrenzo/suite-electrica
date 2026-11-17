//! Alertas por límites del armado: vano peso máximo (P↓), carga en la cruceta (Cr) y arrancamiento
//! con exceso relativo al vano peso máximo (S↑), como REDLIN. Sin los límites no se verifican.

use std::path::PathBuf;

use suite_motor::documento::Documento;

fn con_limites(doc: &mut Documento, vano_peso: &str, cruceta: &str) {
    let t = doc.catalogos.get_mut("armados").unwrap();
    let (iv, ic) = (
        t.columnas.iter().position(|c| c == "vano_peso_max_m").unwrap(),
        t.columnas.iter().position(|c| c == "carga_max_cruceta_n").unwrap(),
    );
    for f in &mut t.filas {
        f.resize(t.columnas.len(), String::new());
        f[iv] = vano_peso.into();
        f[ic] = cruceta.into();
    }
}

#[test]
fn alertas_de_vano_peso_y_cruceta() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    doc.calcular().unwrap();
    // catálogos de la plantilla (como «Cargar catálogos de la plantilla») y armados asignados
    doc.catalogos = Documento::nuevo().catalogos;
    let base = doc.asignar_catalogos(false).unwrap();
    let cuenta = |c: &suite_motor::documento::Calculo, k: &str| c.alertas.iter().filter(|a| a.codigo == k).count();
    assert_eq!((cuenta(&base, "P↓"), cuenta(&base, "Cr")), (0, 0), "sin límites no se verifica");

    // límites holgados: no alertan
    con_limites(&mut doc, "10000", "1000000");
    let c = doc.calcular().unwrap();
    assert_eq!((cuenta(&c, "P↓"), cuenta(&c, "Cr")), (0, 0));

    // límites mínimos: alertan todas las estructuras con armado y vano peso positivo
    con_limites(&mut doc, "1", "1");
    let c = doc.calcular().unwrap();
    let con_armado = c.resultado.estructuras.iter().filter(|e| !e.armado.is_empty()).count();
    assert!(cuenta(&c, "Cr") >= con_armado - 1 && cuenta(&c, "Cr") > 0, "Cr {} de {con_armado}", cuenta(&c, "Cr"));
    assert!(cuenta(&c, "P↓") > 0);
    let a = c.alertas.iter().find(|a| a.codigo == "P↓").unwrap();
    assert!(a.detalle.contains("del armado") && a.exceso > 0.0);
    // si hay arrancamiento, su exceso ahora es finito (% del vano peso máximo)
    for a in c.alertas.iter().filter(|a| a.codigo == "S↑") {
        assert!(a.exceso.is_finite(), "{a:?}");
    }
}
