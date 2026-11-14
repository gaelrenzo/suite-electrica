//! `proyecto.json` (carga completa → mismo cálculo que Python) y `.sel` (ida y vuelta sin pérdidas).

mod comun;

use std::path::PathBuf;

use suite_motor::linea::calcular;
use suite_motor::proyectos::{cargar_proyecto_json, DocumentoSel};

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Carga el JSON en Rust y compara con el volcado de Python (perfil de Python + cálculo).
///
/// `colina_gaussiana.csv` es una malla regular: la triangulación no es única y qhull y spade
/// eligen diagonales distintas (ver docs/PLAN_RUST.md). El perfil se compara entonces por cota
/// interpolada cada 1 m y el resultado de ingeniería (estructuras, tipos, despejes) con
/// tolerancias de diseño; con puntos irregulares la coincidencia es exacta (tests/tin.rs).
fn comparar_json(json: &str, volcado: &str) {
    let p = cargar_proyecto_json(&raiz().join("examples").join("linecraft").join(json)).unwrap();
    let caso = comun::leer(volcado);
    let (r, rp) = (calcular(&p).unwrap(), calcular(&caso.p).unwrap());
    let l = p.perfil.longitud();
    assert!((l - caso.p.perfil.longitud()).abs() < 1e-6, "{json}: longitud del eje");
    let mut dz_max: f64 = 0.0;
    let mut s = 0.0;
    while s <= l {
        dz_max = dz_max.max((p.perfil.cota_en(s) - caso.p.perfil.cota_en(s)).abs());
        s += 1.0;
    }
    println!("{json}: perfil Rust {} pts, Python {} pts; |Δcota| máx. {dz_max:.4} m",
             p.perfil.progresiva.len(), caso.p.perfil.progresiva.len());
    assert!(dz_max < 0.10, "{json}: diferencia de cota del terreno {dz_max} m"); // malla regular: diagonales distintas
    assert_eq!(r.estructuras.len(), rp.estructuras.len(), "{json}: estructuras");
    let mut ds_max: f64 = 0.0;
    for (a, b) in r.estructuras.iter().zip(&rp.estructuras) {
        assert_eq!(a.tipo, b.tipo, "{json}: tipo de {}", a.codigo());
        ds_max = ds_max.max((a.progresiva - b.progresiva).abs());
    }
    let dd_max = r.vanos.iter().zip(&rp.vanos).map(|(a, b)| (a.despeje - b.despeje).abs()).fold(0.0, f64::max);
    println!("{json}: |Δprogresiva| máx. {ds_max:.3} m, |Δdespeje| máx. {dd_max:.4} m");
    assert!(ds_max <= 1.0 + 1e-9, "{json}: estructuras desplazadas {ds_max} m");
    assert!(dd_max < 0.05, "{json}: despejes {dd_max} m");
    // el volcado de prueba codifica los espacios como "_": se normalizan ambos lados
    let norm = |m: &std::collections::HashMap<String, String>| {
        m.iter().map(|(k, v)| (k.clone(), v.replace('_', " "))).collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(norm(&p.fuentes), norm(&caso.p.fuentes), "{json}: fuentes");
    assert_eq!(p.hipotesis.len(), caso.p.hipotesis.len());
}

#[test]
fn proyecto_json_auto() {
    comparar_json("proyecto.json", include_str!("linea_auto.txt"));
}

#[test]
fn proyecto_json_manual() {
    comparar_json("proyecto_manual.json", include_str!("linea_manual.txt"));
}

#[test]
fn sel_ida_y_vuelta() {
    let origen = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("proyecto_python.sel");
    let doc = DocumentoSel::abrir(&origen).unwrap();
    assert!(!doc.puntos().is_empty() && doc.eje().is_some_and(|e| e.len() >= 2));
    assert!(doc.puntos()[3].cota.is_nan(), "la cota vacía se conserva como vacía");
    let salida = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("proyecto_desde_rust.sel");
    let guardado = doc.guardar(&salida).unwrap();
    let otra = DocumentoSel::abrir(&guardado).unwrap();
    assert_eq!(doc, otra, "el .sel guardado por Rust debe tener exactamente los mismos datos");
    let ajeno = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ajeno.sel");
    std::fs::write(&ajeno, r#"{"formato": "otro"}"#).unwrap();
    assert!(DocumentoSel::abrir(&ajeno).unwrap_err().contains("no es un proyecto"));
    std::fs::write(&ajeno, r#"{"formato": "suite-electrica", "version": 99}"#).unwrap();
    assert!(DocumentoSel::abrir(&ajeno).unwrap_err().contains("más nueva"));
}
