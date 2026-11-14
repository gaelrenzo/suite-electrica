//! Armados DGE monofásicos y bifásicos (RD 024-2003, láminas 001-025): elección por sistema,
//! materiales completos y flujo de la app con una línea monofásica y una bifásica con neutro.

use std::path::PathBuf;

use suite_motor::catalogos::Catalogos;
use suite_motor::documento::Documento;
use suite_motor::linea::Tipo;

#[test]
fn eleccion_por_sistema() {
    let cat = Catalogos::plantilla();
    let casos = [
        // (fases, neutro, tipo, deflexión, esperado)
        (1, false, Tipo::Suspension, 0.0, "PS1-0"),
        (1, false, Tipo::Angulo, 20.0, "PA1-0"),
        (1, false, Tipo::Angulo, 45.0, "PA2-0"),
        (1, false, Tipo::Angulo, 75.0, "PA3-0"),
        (1, false, Tipo::Anclaje, 0.0, "PR3-0"),
        (1, false, Tipo::Fin, 0.0, "PTV-0"),
        (1, true, Tipo::Suspension, 3.0, "PS1-1N"),
        (1, true, Tipo::Fin, 0.0, "PTV-1N"),
        (2, false, Tipo::Suspension, 0.0, "PS1-2"),
        (2, false, Tipo::Angulo, 25.0, "PA1-2"),
        (2, true, Tipo::Angulo, 50.0, "PA2-2N"),
        (2, true, Tipo::Anclaje, 0.0, "PR3-2N"),
        (2, false, Tipo::Fin, 0.0, "PTV-2"),
        (3, false, Tipo::Suspension, 0.0, "PS1-3"),
        (3, false, Tipo::Fin, 0.0, "PTV-3"),
        (3, true, Tipo::Suspension, 0.0, "PS1-3N"),
        (3, true, Tipo::Angulo, 45.0, "PA2-3N"),
        (3, true, Tipo::Anclaje, 0.0, "PR3-3N"),
        (3, true, Tipo::Fin, 0.0, "PTV-3N"),
    ];
    for (f, n, t, a, esp) in casos {
        assert_eq!(cat.elegir_armado_sistema(t, a, f, n), esp, "{f} fase(s) neutro={n} {t:?} {a}°");
    }
    // la elección sin sistema (paridad con Python) sigue siendo trifásica
    assert_eq!(cat.elegir_armado(Tipo::Suspension, 0.0), "PS1-3");
    // todo armado tiene geometría y materiales de su lámina
    for a in &cat.armados {
        assert!(a.distancia_punta_amarre_m.is_some() && !a.alturas_fases_m.is_empty(), "{}", a.codigo);
        let fases = a.alturas_fases_m.split(';').count();
        assert_eq!(fases, a.fases.unwrap_or(3.0) as usize, "{}: alturas por fase", a.codigo);
        assert!(cat.materiales.iter().any(|m| m.armado == a.codigo), "{}: sin materiales", a.codigo);
    }
}

#[test]
fn linea_monofasica_y_bifasica_con_neutro() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    for (fases, neutro, prefijo) in [(1, false, "-0"), (2, true, "-2N"), (3, true, "-3N")] {
        let mut doc = Documento::abrir(&sel).unwrap();
        doc.catalogos = Documento::nuevo().catalogos; // el .sel trae los catálogos de cuando se creó
        doc.linea.fases = fases;
        doc.linea.neutro = neutro;
        doc.linea.n_conductores = fases;
        let c = doc.calcular().unwrap();
        for e in &doc.estructuras {
            assert!(e.armado.ends_with(prefijo), "{fases} fase(s): armado {}", e.armado);
        }
        assert!(!c.resultado.avisos.iter().any(|a| a.starts_with("Sin armado")), "{:?}", c.resultado.avisos);
        assert!(c.metrado.iter().any(|m| m.item == "Material"), "metrado con materiales de las láminas");
        // el sistema se guarda en el .sel
        let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sistema_{fases}.sel"));
        doc.guardar(Some(&tmp)).unwrap();
        let otro = Documento::abrir(&tmp).unwrap();
        assert_eq!((otro.linea.fases, otro.linea.neutro), (fases, neutro));
    }
    // catálogo del .sel (anterior a los monofásicos): aviso claro
    let mut doc = Documento::abrir(&sel).unwrap();
    doc.linea.fases = 1;
    doc.linea.n_conductores = 1;
    let c = doc.calcular().unwrap();
    assert!(c.resultado.avisos.iter().any(|a| a.starts_with("Sin armado del catálogo para el sistema 1 fase(s)")), "{:?}", c.resultado.avisos);
}
