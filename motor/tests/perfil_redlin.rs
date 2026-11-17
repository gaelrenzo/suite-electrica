//! Flujo de un usuario de REDLIN en la interfaz: perfil RLT ACUMULADA/ANGULO o `.tpg` importado
//! directo (sin nube de puntos), estructuras en progresivas exactas y aviso de creep sin curva.

use std::path::PathBuf;

use suite_motor::documento::{leer_progresiva, Documento};

fn archivo(n: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join(n)
}

#[test]
fn perfil_directo_reproduce_las_cotas() {
    for n in ["rlt_acumulada.xlsx", "perfil.tpg"] {
        let (p, _) = suite_motor::entrada::leer_perfil_ruta(&archivo(n), None).unwrap();
        let mut doc = Documento::nuevo();
        let k = doc.importar_perfil(&archivo(n), None).unwrap();
        assert_eq!(k, p.progresiva.len());
        let t = doc.topografia().unwrap();
        let q = t.perfil.as_ref().unwrap();
        assert!((q.longitud() - p.longitud()).abs() < 1e-6, "{n}: longitud {} vs {}", q.longitud(), p.longitud());
        for (s, z) in p.progresiva.iter().zip(&p.cota) {
            assert!((q.cota_en(*s) - z).abs() < 0.01, "{n}: cota en {s} = {} (tabla {z})", q.cota_en(*s));
        }
    }
    // un RLT de puntos no es un perfil y al revés: mensajes que dicen qué hacer
    let e = suite_motor::entrada::leer_xls_utm(&archivo("rlt_acumulada.xlsx")).unwrap_err();
    assert!(e.contains("Importar perfil"), "{e}");
    assert_eq!(suite_motor::entrada::rutas_perfil_tabular(&archivo("perfil.tpg")).unwrap().len(), 1);
}

#[test]
fn estructuras_en_progresivas_exactas() {
    assert_eq!(leer_progresiva("1+322.50"), Some(1322.5));
    assert_eq!(leer_progresiva("457,5"), Some(457.5));
    assert_eq!(leer_progresiva("x"), None);
    let mut doc = Documento::nuevo();
    doc.importar_perfil(&archivo("perfil.tpg"), None).unwrap();
    let l = doc.eje.as_deref().map(|e| *suite_motor::geo::progresivas_eje(e).last().unwrap()).unwrap();
    let n = doc.fijar_estructuras("# comentario\n0+120;ANCLAJE;PR3-0\n150.5\tsuspension\n").unwrap();
    // inicio, fin y vértices se agregan solos
    let s: Vec<f64> = doc.estructuras.iter().map(|e| e.progresiva_m).collect();
    assert_eq!(n, s.len());
    assert!(s.contains(&0.0) && s.contains(&120.0) && s.contains(&150.5) && (s.last().unwrap() - l).abs() < 1e-3, "{s:?}");
    let e120 = doc.estructuras.iter().find(|e| e.progresiva_m == 120.0).unwrap();
    assert_eq!((e120.tipo.as_str(), e120.armado.as_str()), ("ANCLAJE", "PR3-0"));
    assert!(doc.fijar_estructuras("9999").unwrap_err().contains("fuera del eje"));
    assert!(doc.fijar_estructuras("120;XYZ").unwrap_err().contains("tipo inválido"));
    assert_eq!(doc.estructuras.len(), n, "una lista con error no cambia nada");
    doc.deshacer();
    assert!(doc.estructuras.is_empty(), "Ctrl+Z vuelve a antes de aplicar la lista");
}

#[test]
fn creep_sin_curva_avisa() {
    let sel = archivo("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    doc.linea.condicion_final = "creep".into();
    doc.conductor.curva_creep.clear();
    let c = doc.calcular().unwrap();
    assert!(c.resultado.avisos.first().is_some_and(|a| a.contains("sin curva de creep")), "{:?}", c.resultado.avisos.first());
    doc.conductor.curva_creep = vec![0.0, 38.0];
    let c = doc.calcular().unwrap();
    assert!(!c.resultado.avisos.iter().any(|a| a.contains("sin curva de creep")));
}
