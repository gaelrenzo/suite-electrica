//! Guardar una red editada en la app y volver a abrirla: mismos nodos, estructuras, conductores,
//! parámetros y mismo cálculo.

use std::path::PathBuf;

use suite_motor::red::{calcular, cargar, guardar, longitud_plano, metrado};

#[test]
fn guardar_y_reabrir_red() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("examples").join("gridcraft");
    let mut red = cargar(&raiz.join("red.json")).unwrap();
    // edición como en la app: estructura, acometidas y un nodo nuevo
    red.nodos[1].estructura.armado = "2E3/S".into();
    red.nodos[1].estructura.acometidas = vec!["C".into(), "L3M".into()];
    red.nodos[1].estructura.conectores = vec![("CFF35-16".into(), 2.0, true)];
    let mut nuevo = red.nodos[1].clone();
    nuevo.nodo = "N_nuevo".into();
    nuevo.padre = red.nodos[1].nodo.clone();
    nuevo.este = red.nodos[1].este.map(|x| x + 30.0);
    nuevo.norte = red.nodos[1].norte.map(|y| y + 40.0);
    red.nodos.push(nuevo);
    let i = red.nodos.len() - 1;
    assert_eq!(longitud_plano(&red, i), Some(50.0));
    red.nodos[i].longitud_m = 50.0;
    red.caida_max_pct = 4.5;

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("red_guardada");
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["red.json", "nodos.csv", "conductores_bt.csv"] {
        std::fs::copy(raiz.join(f), dir.join(f)).unwrap();
    }
    guardar(&red, &dir.join("red.json")).unwrap();
    let otra = cargar(&dir.join("red.json")).unwrap();
    assert_eq!(otra.nodos.len(), red.nodos.len());
    for (a, b) in otra.nodos.iter().zip(&red.nodos) {
        assert_eq!((&a.nodo, &a.padre, &a.conductor, &a.estructura), (&b.nodo, &b.padre, &b.conductor, &b.estructura));
        assert!((a.longitud_m - b.longitud_m).abs() < 1e-9 || b.padre.is_empty());
        assert_eq!((a.lotes, a.kw_ce, a.lamparas, a.este, a.norte), (b.lotes, b.kw_ce, b.lamparas, b.este, b.norte));
    }
    assert_eq!(otra.caida_max_pct, 4.5);
    assert_eq!(otra.fuentes, red.fuentes);
    let (r1, r2) = (calcular(&red).unwrap(), calcular(&otra).unwrap());
    assert_eq!(r1.tramos.iter().map(|t| t.dv_pct).collect::<Vec<_>>(), r2.tramos.iter().map(|t| t.dv_pct).collect::<Vec<_>>());
    assert_eq!(metrado(&red), metrado(&otra));
}
