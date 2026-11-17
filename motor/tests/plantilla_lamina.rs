//! Plantilla de lámina del usuario: su marco y cajetín (DXF en mm de papel) se colocan a la escala
//! del plano y los campos del cajetín se reemplazan; sin plantilla, la lámina propia.

use std::path::PathBuf;

use suite_motor::dxf::{aplicar_lamina, Cajetin, Dxf, PLANTILLA_LAMINA};

#[test]
fn plano_con_plantilla_del_usuario() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("plantilla");
    std::fs::create_dir_all(&dir).unwrap();
    let caj = Cajetin { proyecto: "LP Capachica".into(), titulo: "PERFIL".into(), autor: "R. M.".into(), lamina: "PP-01".into(), nota: String::new() };
    let dibujo = || {
        let mut d = Dxf::nuevo();
        d.linea("X", [1000.0, 5000.0], [1800.0, 5300.0]); // 800 × 300 m
        d
    };
    // sin plantilla: la lámina propia (A3 a 1:2500)
    let _ = std::fs::remove_file(dir.join(PLANTILLA_LAMINA));
    let mut d = dibujo();
    aplicar_lamina(&mut d, &caj, Some(&dir)).unwrap();
    assert!(d.contenido().contains("ESCALA 1:2500"));

    // plantilla A3 (420 × 297 mm) con su propio cajetín y campos
    let plantilla = "0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\nMARCO\n90\n4\n70\n1\n10\n0\n20\n0\n10\n420\n20\n0\n10\n420\n20\n297\n10\n0\n20\n297\n\
0\nLINE\n8\nMARCO\n10\n240\n20\n0\n11\n240\n21\n40\n\
0\nTEXT\n8\nMARCO\n10\n245\n20\n30\n40\n3.5\n1\nUNA - FIME {PROYECTO}\n\
0\nTEXT\n8\nMARCO\n10\n245\n20\n20\n40\n2.5\n1\nPlano {PLANO} lámina {LAMINA} esc. {ESCALA}\n\
0\nENDSEC\n0\nEOF\n";
    std::fs::write(dir.join(PLANTILLA_LAMINA), plantilla).unwrap();
    let mut d = dibujo();
    aplicar_lamina(&mut d, &caj, Some(&dir)).unwrap();
    let t = d.contenido();
    assert!(t.contains("UNA - FIME LP Capachica"), "campos reemplazados");
    assert!(t.contains("Plano PERFIL l\\U+00E1mina PP-01 esc. 1:2500"), "{}", t.lines().filter(|l| l.contains("Plano")).collect::<Vec<_>>().join("|"));
    assert!(!t.contains("ESCALA 1:2500"), "no se dibuja el cajetín propio");
    // el marco de 420 mm mide 1050 m a 1:2500 y contiene el dibujo
    let mut d2 = dibujo();
    let pl = suite_motor::entrada::leer_plantilla_lamina(&dir.join(PLANTILLA_LAMINA)).unwrap();
    assert_eq!(d2.lamina_con_plantilla(&caj, &pl), Some(2500.0));
    // plantilla demasiado chica: error claro
    std::fs::write(dir.join(PLANTILLA_LAMINA), "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nM\n10\n0\n20\n0\n11\n20\n21\n20\n0\nENDSEC\n0\nEOF\n").unwrap();
    assert!(aplicar_lamina(&mut dibujo(), &caj, Some(&dir)).unwrap_err().contains("chica"));
}
