//! Terminal a media línea (doble remate): un FIN con vanos a ambos lados resiste el tiro completo
//! del lado mayor (no la diferencia, como un anclaje), lleva 2 retenidas y cada una toma su lado.

use std::path::PathBuf;

use suite_motor::documento::Documento;

#[test]
fn fin_a_media_linea() {
    let sel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos").join("documento_irregular.sel");
    let mut doc = Documento::abrir(&sel).unwrap();
    assert!(doc.cambiar_estructura(999, "tipo", "FIN").is_err(), "índice fuera de rango: error, no pánico");
    doc.calcular().unwrap(); // ubica las estructuras
    let i = doc.estructuras.len() / 2;
    let carga = |c: &suite_motor::documento::Calculo| {
        let num = c.resultado.estructuras[i].numero;
        c.resultado.cargas.iter().filter(|x| x.estructura == num).map(|x| (x.longitudinal, x.tiro_retenida)).fold((0.0f64, 0.0f64), |a, b| (a.0.max(b.0), a.1.max(b.1)))
    };
    doc.cambiar_estructura(i, "tipo", "ANCLAJE").unwrap();
    let anclaje = doc.calcular().unwrap();
    doc.cambiar_estructura(i, "tipo", "FIN").unwrap();
    let fin = doc.calcular().unwrap();
    let (la, lf) = (carga(&anclaje).0, carga(&fin).0);
    assert!(lf > 10.0 * la.max(1.0), "terminal: {lf} N, anclaje: {la} N");

    // el longitudinal del terminal es n × el mayor tiro horizontal de sus dos vanos (hipótesis crítica)
    let r = &fin.resultado;
    let num = r.estructuras[i].numero;
    let ang = (r.estructuras[i].angulo.abs().to_radians() / 2.0).cos();
    let n = fin.proyecto.n_conductores as f64;
    let esperado = r.cargas.iter().filter(|c| c.estructura == num).map(|c| c.longitudinal).fold(0.0, f64::max);
    let h_max = r.tramos.iter().flat_map(|t| t.estados.iter().map(|e| e.h)).fold(0.0, f64::max);
    assert!(esperado <= n * h_max * ang + 1e-6 && esperado > 0.0);

    let v = fin.verificacion.iter().find(|f| f.estructura == r.estructuras[i].codigo()).unwrap();
    assert_eq!(v.retenidas, 2, "una retenida por lado");
    // cada retenida toma el tiro completo de su lado (no la mitad del total)
    assert!((v.tiro_por_retenida.unwrap() - carga(&fin).1).abs() < 1e-6);
    // los extremos siguen con 1 retenida
    assert_eq!(fin.verificacion[0].retenidas, 1);
}
