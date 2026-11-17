//! Carga máxima por espiga desde la rotura del aislador y el FS del proyecto, cada uno con su fuente.

use std::path::PathBuf;

use suite_motor::documento::Documento;

#[test]
fn espiga_por_rotura_y_fs() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    doc.linea.tiro_max_espiga = 1.0; // valor directo: se ignora cuando hay rotura y FS
    doc.linea.rotura_espiga_n = 13000.0;
    doc.linea.fs_espiga = 2.5;
    assert_eq!(doc.linea.tiro_espiga(), 5200.0);
    assert_eq!(doc.linea.config_alertas().tiro_max_espiga, 5200.0);
    // sin fuente del FS: aviso de revisión
    let c = doc.calcular().unwrap();
    assert!(c.resultado.avisos.iter().any(|a| a.contains("'fs_espiga'")), "{:?}", c.resultado.avisos);
    // con FS sin rotura vale el valor directo
    doc.linea.rotura_espiga_n = 0.0;
    assert_eq!(doc.linea.tiro_espiga(), 1.0);
    // se guarda y se relee
    doc.linea.rotura_espiga_n = 13000.0;
    let ruta = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fs_espiga.sel");
    doc.guardar(Some(&ruta)).unwrap();
    let otro = Documento::abrir(&ruta).unwrap();
    assert_eq!((otro.linea.rotura_espiga_n, otro.linea.fs_espiga), (13000.0, 2.5));
}
