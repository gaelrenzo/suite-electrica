//! Catastro: predios cruzados por el eje y tocados solo por la faja; lectura desde DXF con nombres.

use std::path::PathBuf;

use suite_motor::catastro::{afectados, area, dentro, Predio};

fn rect(nombre: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> Predio {
    Predio { nombre: nombre.into(), xy: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]] }
}

#[test]
fn predios_afectados_por_la_faja() {
    assert!(dentro([1.0, 1.0], &rect("", 0.0, 0.0, 2.0, 2.0).xy) && !dentro([3.0, 1.0], &rect("", 0.0, 0.0, 2.0, 2.0).xy));
    assert_eq!(area(&rect("", 0.0, 0.0, 100.0, 50.0).xy), 5000.0);
    let eje = [[-50.0, 0.0], [150.0, 0.0]];
    let predios = [
        rect("A cruzado", 0.0, -50.0, 100.0, 50.0),
        rect("B solo faja", 0.0, 3.0, 10.0, 20.0),
        rect("C lejos", 0.0, 30.0, 10.0, 40.0),
    ];
    let f = afectados(&eje, &predios, 10.0, 0.5);
    assert_eq!(f.len(), 2, "C no se toca");
    let a = &f[0];
    assert_eq!((a.nombre.as_str(), a.progresiva_ini, a.progresiva_fin), ("A cruzado", 50.0, 150.0));
    assert!((a.longitud_eje_m - 100.0).abs() < 1e-9 && (a.area_faja_m2 - 1000.0).abs() < 1e-6 && a.area_predio_m2 == 10_000.0);
    let b = &f[1];
    assert!(b.longitud_eje_m == 0.0 && b.progresiva_ini.is_nan());
    assert!((b.area_faja_m2 - 20.0).abs() < 1e-6, "faja de y = 3 a 5 por 10 m: {}", b.area_faja_m2);
}

#[test]
fn predios_desde_dxf() {
    let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\nPREDIOS\n90\n4\n70\n1\n10\n0\n20\n0\n10\n100\n20\n0\n10\n100\n20\n50\n10\n0\n20\n50\n\
0\nTEXT\n8\nPREDIOS\n10\n40\n20\n20\n40\n2\n1\nP-001 Quispe\n\
0\nLWPOLYLINE\n8\nPREDIOS\n90\n3\n70\n1\n10\n200\n20\n0\n10\n260\n20\n0\n10\n200\n20\n40\n\
0\nLWPOLYLINE\n8\nEJE\n90\n2\n70\n0\n10\n0\n20\n0\n10\n500\n20\n0\n\
0\nENDSEC\n0\nEOF\n";
    let ruta = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("catastro.dxf");
    std::fs::write(&ruta, dxf).unwrap();
    let p = suite_motor::entrada::predios_dxf(&ruta).unwrap();
    assert_eq!(p.len(), 2, "solo las cerradas");
    assert_eq!(p[0].nombre, "P-001 Quispe");
    assert_eq!(p[1].nombre, "Predio 2");
    assert_eq!(area(&p[1].xy), 1200.0);
}

#[test]
fn catastro_en_el_sel() {
    use suite_motor::documento::Documento;
    let mut doc = Documento::nuevo();
    assert!(doc.predios_afectados().is_err(), "sin eje");
    doc.eje = Some(vec![[-50.0, 0.0], [150.0, 0.0]]);
    doc.catastro = vec![rect("A", 0.0, -50.0, 100.0, 50.0)];
    doc.ancho_servidumbre_m = 6.0;
    assert!((doc.predios_afectados().unwrap()[0].area_faja_m2 - 600.0).abs() < 1e-6);
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("catastro.sel");
    doc.guardar(Some(&tmp)).unwrap();
    let otro = Documento::abrir(&tmp).unwrap();
    assert_eq!((otro.catastro.clone(), otro.ancho_servidumbre_m), (doc.catastro.clone(), 6.0));
    // un proyecto sin catastro no escribe la clave (archivos iguales a los de antes)
    let mut sin = Documento::nuevo();
    sin.guardar(Some(&tmp)).unwrap();
    assert!(!std::fs::read_to_string(&tmp).unwrap().contains("catastro"));
}
