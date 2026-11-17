//! Autodistribución de la red BT desde los lotes: cada lote queda al alcance de una acometida de
//! algún poste, ningún vano supera el máximo, la red es un árbol desde la SED y se calcula.

use suite_motor::red::{autodistribuir, ParamAutodistribucion};

#[test]
fn red_desde_lotes() {
    // manzana de 4 × 12 lotes de 10 × 20 m y un grupo aislado a 150 m
    let mut lotes: Vec<[f64; 2]> = Vec::new();
    for i in 0..12 {
        for j in 0..4 {
            lotes.push([i as f64 * 10.0, j as f64 * 20.0]);
        }
    }
    lotes.extend([[260.0, 30.0], [270.0, 30.0], [265.0, 45.0]]);
    let p = ParamAutodistribucion { conductor: "3x35+16/25".into(), ..Default::default() };
    let nodos = autodistribuir(&lotes, [-20.0, 30.0], &p).unwrap();
    let pos = |n: &str| nodos.iter().find(|x| x.nodo == n).map(|x| [x.este.unwrap(), x.norte.unwrap()]).unwrap();
    let d = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    // árbol: una sola raíz y cada padre existe y aparece antes
    assert_eq!(nodos.iter().filter(|n| n.padre.is_empty()).count(), 1);
    for (k, n) in nodos.iter().enumerate().skip(1) {
        let i = nodos.iter().position(|x| x.nodo == n.padre).unwrap();
        assert!(i < k, "{} antes que su padre", n.nodo);
        assert!(n.longitud_m <= p.vano_max_m + 1e-6, "vano {} de {} m", n.nodo, n.longitud_m);
        assert!((n.longitud_m - d(pos(&n.padre), pos(&n.nodo))).abs() < 0.01);
    }
    // todos los lotes asignados una vez y al alcance de su poste
    assert_eq!(nodos.iter().map(|n| n.lotes).sum::<f64>(), lotes.len() as f64);
    assert_eq!(nodos.iter().map(|n| n.estructura.acometidas.len()).sum::<usize>(), lotes.len());
    for l in &lotes {
        let cerca = nodos.iter().filter(|n| n.lotes > 0.0).map(|n| d(pos(&n.nodo), *l)).fold(f64::INFINITY, f64::min);
        assert!(cerca <= p.acometida_larga_m + 1e-6, "lote {l:?} a {cerca:.1} m");
    }
    // menos postes con carga que lotes (agrupa) y el grupo aislado se alcanza con postes intermedios
    let con_carga = nodos.iter().filter(|n| n.lotes > 0.0).count();
    assert!(con_carga < lotes.len() / 3, "{con_carga} postes con carga");
    assert!(nodos.iter().any(|n| n.lotes == 0.0 && !n.padre.is_empty()), "postes intermedios");
    assert!(autodistribuir(&[], [0.0, 0.0], &p).is_err());
}
