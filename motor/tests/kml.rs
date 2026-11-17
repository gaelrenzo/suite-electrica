//! KML y CSV exportados se vuelven a leer con los importadores de la suite: mismas coordenadas.

use std::path::PathBuf;

use suite_motor::entrada::{leer, leer_kml, Punto};
use suite_motor::kml::{escribir_puntos_csv, Kml};

#[test]
fn kml_y_csv_ida_y_vuelta() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("kml");
    std::fs::create_dir_all(&dir).unwrap();
    let eje = vec![[410716.36, 8270488.93], [413187.20, 8269060.40], [417484.11, 8266421.48]];
    let mut k = Kml::nuevo("Línea <prueba> & Cía", "19S");
    k.linea("Eje", &eje, "ff0000ff", 3.0).unwrap();
    k.punto("E-01", eje[0], "PTV-3").unwrap();
    k.poligono("P-001", &[[411000.0, 8270000.0], [411100.0, 8270000.0], [411100.0, 8270100.0]]).unwrap();
    let ruta = k.guardar(&dir.join("linea.kml")).unwrap();
    let t = std::fs::read_to_string(&ruta).unwrap();
    assert!(t.contains("Línea &lt;prueba&gt; &amp; Cía"));
    let (pts, rutas) = leer_kml(&ruta, Some("19S")).unwrap();
    let r = rutas.iter().find(|r| r.0 == "Eje").unwrap();
    assert_eq!(r.1.len(), 3);
    for (a, b) in r.1.iter().zip(&eje) {
        assert!((a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01, "{a:?} vs {b:?}");
    }
    assert!(pts.iter().any(|p| p.punto == "E-01" && (p.este - eje[0][0]).abs() < 0.01));

    let pts = vec![
        Punto { punto: "P1".into(), este: 390000.5, norte: 8248000.25, cota: 3820.002, descripcion: "TN, borde".into() },
        Punto { punto: "P2".into(), este: 390010.0, norte: 8248010.0, cota: 3821.0, descripcion: String::new() },
    ];
    let c = escribir_puntos_csv(&dir.join("puntos.csv"), &pts).unwrap();
    let otra = leer(&c).unwrap();
    assert_eq!(otra.len(), 2);
    assert_eq!((otra[0].este, otra[0].norte, otra[0].cota), (390000.5, 8248000.25, 3820.002));
    assert_eq!(otra[0].descripcion, "TN, borde");
}
