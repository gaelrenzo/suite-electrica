//! Transformación de la nube de puntos: rotación alrededor del centroide y traslación, también el
//! eje si se pide.

use suite_motor::documento::Documento;
use suite_motor::entrada::Punto;

fn p(n: &str, e: f64, no: f64, z: f64) -> Punto {
    Punto { punto: n.into(), este: e, norte: no, cota: z, descripcion: String::new() }
}

#[test]
fn rotar_y_trasladar() {
    let mut d = Documento::nuevo();
    assert!(d.transformar_puntos(1.0, 0.0, 0.0, 0.0, true).is_err());
    // cuadrado centrado en (100, 200)
    d.puntos = vec![p("A", 90.0, 190.0, 10.0), p("B", 110.0, 190.0, 11.0), p("C", 110.0, 210.0, 12.0), p("D", 90.0, 210.0, 13.0)];
    d.eje = Some(vec![[90.0, 200.0], [110.0, 200.0]]);
    d.transformar_puntos(5.0, -3.0, 0.5, 90.0, true).unwrap();
    let c = |k: usize| [d.puntos[k].este, d.puntos[k].norte, d.puntos[k].cota];
    let cerca = |a: [f64; 3], b: [f64; 3]| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-9);
    // A (−10, −10) rota 90° → (10, −10), luego +5, −3
    assert!(cerca(c(0), [115.0, 187.0, 10.5]), "{:?}", c(0));
    assert!(cerca(c(2), [95.0, 207.0, 12.5]), "{:?}", c(2));
    let e = d.eje.clone().unwrap();
    assert!((e[0][0] - 105.0).abs() < 1e-9 && (e[0][1] - 187.0).abs() < 1e-9, "{e:?}");
    assert!(d.modificado);
    // sin el eje
    d.transformar_puntos(1.0, 0.0, 0.0, 0.0, false).unwrap();
    assert_eq!(d.eje.unwrap(), e);
}

#[test]
fn invertir_eje_conserva_ubicaciones() {
    use suite_motor::documento::FilaEstructura;
    use suite_motor::geo::punto_en_progresiva;
    let mut d = Documento::nuevo();
    assert!(d.invertir_eje().is_err());
    d.eje = Some(vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0]]); // L = 150
    d.estructuras = vec![FilaEstructura::en(0.0), FilaEstructura::en(40.0), FilaEstructura::en(150.0)];
    d.cruces = vec![vec!["120".into(), "camino".into(), "4".into(), "6".into(), "CNE".into()]];
    let antes: Vec<[f64; 2]> = d.estructuras.iter().map(|e| punto_en_progresiva(d.eje.as_ref().unwrap(), e.progresiva_m)).collect();
    d.invertir_eje().unwrap();
    assert_eq!(d.eje.as_ref().unwrap()[0], [100.0, 50.0]);
    assert_eq!(d.estructuras.iter().map(|e| e.progresiva_m).collect::<Vec<_>>(), vec![0.0, 110.0, 150.0]);
    assert_eq!(d.cruces[0][0], "30");
    // cada estructura sigue en el mismo punto del terreno
    let despues: Vec<[f64; 2]> = d.estructuras.iter().map(|e| punto_en_progresiva(d.eje.as_ref().unwrap(), e.progresiva_m)).collect();
    for a in &antes {
        assert!(despues.iter().any(|b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9));
    }
}
