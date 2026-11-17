//! Perfil desde curvas de nivel (TOPOMAGIC §7.17): un DXF que solo trae curvas (polilíneas con
//! elevación) se importa como puntos, se triangula y da el perfil del eje.

use std::path::PathBuf;

use suite_motor::documento::Documento;

#[test]
fn perfil_desde_curvas_de_nivel() {
    // pirámide: cuadrados concéntricos de semilado 100 - 10·k a cota 3800 + 2·k (k = 0..9)
    let mut dxf = String::from("0\nSECTION\n2\nENTITIES\n");
    for k in 0..10 {
        let (a, z) = (100.0 - 10.0 * k as f64, 3800.0 + 2.0 * k as f64);
        dxf.push_str(&format!("0\nLWPOLYLINE\n8\nCURVAS\n90\n4\n70\n1\n38\n{z}\n"));
        for (x, y) in [(-a, -a), (a, -a), (a, a), (-a, a)] {
            dxf.push_str(&format!("10\n{}\n20\n{}\n", 400000.0 + x, 8300000.0 + y));
        }
    }
    dxf.push_str("0\nENDSEC\n0\nEOF\n");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("curvas");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("curvas.dxf"), dxf).unwrap();

    let mut doc = Documento::nuevo();
    doc.importar_puntos(&dir.join("curvas.dxf")).unwrap();
    assert_eq!(doc.puntos.len(), 40, "4 vértices por curva");
    doc.eje = Some(vec![[400000.0 - 95.0, 8300000.0], [400000.0 + 95.0, 8300000.0]]);
    let t = doc.topografia().unwrap();
    let p = t.perfil.as_ref().unwrap();
    // en el centro la cota es la de la curva más alta y a 50 m del centro, la de la curva 5
    let z = |s: f64| p.cota_en(s);
    assert!((z(95.0) - 3818.0).abs() < 0.5, "centro {}", z(95.0));
    assert!((z(45.0) - 3810.0).abs() < 1.0, "a 50 m del centro {}", z(45.0));
    assert!(z(5.0) < z(45.0) && z(45.0) < z(95.0), "sube hacia el centro");
}
