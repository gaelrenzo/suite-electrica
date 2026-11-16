//! Revisión automática del diseño de la red: cada regla dispara con su caso y la red de ejemplo,
//! coherente, no tiene observaciones.

use std::path::PathBuf;

use suite_motor::red::{cargar, revisar};

#[test]
fn observaciones_del_diseno() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("examples").join("gridcraft");
    let mut red = cargar(&raiz.join("red.json")).unwrap();
    assert!(revisar(&red).is_empty(), "{:?}", revisar(&red));

    let tiene = |obs: &[String], nodo: &str, texto: &str| obs.iter().any(|o| o.starts_with(nodo) && o.contains(texto));
    red.nodos[1].longitud_m += 20.0; // N1: el vano no coincide con el plano
    red.nodos[2].este = None; // N2 sin coordenadas
    red.nodos[3].estructura.armado = "E1".into(); // con armado en uno, falta en los demás
    red.nodos[3].estructura.pastoral = "P1".into(); // lámparas sin pastoral en otros
    red.nodos[3].estructura.acometidas = vec!["C".into()]; // N3 tiene más lotes que acometidas
    red.nodos[3].estructura.retenida = "RI-BT".into(); // fines de línea sin retenida
    let dup = red.nodos[5].clone();
    red.nodos.push(dup);
    let o = revisar(&red);
    assert!(tiene(&o, "N1", "las coordenadas dan"), "{o:?}");
    assert!(tiene(&o, "N2", "sin coordenadas"));
    assert!(tiene(&o, "N1", "sin armado") && !tiene(&o, "N3", "sin armado"));
    assert!(tiene(&o, "N1", "sin pastoral"));
    assert!(tiene(&o, "N3", "acometida"));
    assert!(tiene(&o, "N4", "fin de línea sin retenida"));
    assert!(o.iter().any(|x| x.contains("repetido")));
}
