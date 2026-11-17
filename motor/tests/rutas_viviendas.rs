//! Operaciones de ruta (separar, unir, empalmar, proyectar puntos) y numeración/validación de
//! viviendas del catastro (TOPOMAGIC §7.4–7.5, §7.11–7.15).

use suite_motor::catastro::{numerar, tabla_puntos_en_eje, validar, Predio};
use suite_motor::documento::Documento;
use suite_motor::entrada::Punto;
use suite_motor::geo::{cortar_eje, lado_del_eje, progresivas_eje, unir_ejes};

fn largo(e: &[[f64; 2]]) -> f64 {
    *progresivas_eje(e).last().unwrap()
}

#[test]
fn separar_unir_empalmar() {
    let eje = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 80.0]];
    let (a, b) = cortar_eje(&eje, 130.0);
    assert_eq!(a, vec![[0.0, 0.0], [100.0, 0.0], [100.0, 30.0]]);
    assert_eq!(b, vec![[100.0, 30.0], [100.0, 80.0]]);
    // unir de vuelta (b invertido también) devuelve el eje original, con el vértice del corte
    let unido = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 30.0], [100.0, 80.0]];
    assert_eq!(unir_ejes(&a, &b), unido);
    assert_eq!(largo(&unido), largo(&eje));
    let mut br = b.clone();
    br.reverse();
    assert_eq!(unir_ejes(&a, &br), unido);

    let mut doc = Documento::nuevo();
    doc.eje = Some(eje.clone());
    doc.estructuras = (0..4).map(|i| suite_motor::documento::FilaEstructura::en(i as f64 * 60.0)).collect();
    let resto = doc.separar_eje(130.0).unwrap();
    assert_eq!(resto, b);
    assert!((largo(doc.eje.as_ref().unwrap()) - 130.0).abs() < 1e-9);
    assert_eq!(doc.estructuras.len(), 3, "se quitan las estructuras después del corte");
    assert!(doc.separar_eje(500.0).is_err());
    // empalme de una variante en la progresiva 50: la variante va de (50, 0) a (50, 100)
    doc.unir_eje(&[[50.0, 100.0], [50.0, 0.0]], Some(50.0)).unwrap();
    assert_eq!(doc.eje.as_ref().unwrap(), &vec![[0.0, 0.0], [50.0, 0.0], [50.0, 100.0]]);
    assert_eq!(doc.estructuras.len(), 1);
}

#[test]
fn proyectar_puntos_sobre_ruta() {
    let eje = [[0.0, 0.0], [100.0, 0.0]];
    assert_eq!(lado_del_eje(&eje, [10.0, 5.0]), 1.0);
    assert_eq!(lado_del_eje(&eje, [10.0, -5.0]), -1.0);
    let p = |n: &str, x: f64, y: f64| Punto { punto: n.into(), este: x, norte: y, cota: 3800.0, descripcion: "TN".into() };
    let t = tabla_puntos_en_eje(&eje, &[p("1", 25.0, 4.0), p("2", 70.0, -3.0)]);
    let txt = format!("{:?}", t.filas);
    assert!(txt.contains("Num(25.0)") && txt.contains("Num(4.0)") && txt.contains("Txt(\"I\")"), "{txt}");
    assert!(txt.contains("Num(70.0)") && txt.contains("Num(3.0)") && txt.contains("Txt(\"D\")"), "{txt}");
}

#[test]
fn numerar_y_validar_viviendas() {
    let lote = |x: f64, y: f64, n: &str| Predio { nombre: n.into(), xy: vec![[x, y], [x + 10.0, y], [x + 10.0, y + 20.0], [x, y + 20.0]] };
    // dos filas de 3 lotes, desordenados
    let mut p = vec![lote(20.0, 0.0, ""), lote(0.0, 30.0, ""), lote(0.0, 0.0, ""), lote(10.0, 30.0, ""), lote(20.0, 30.0, ""), lote(10.0, 0.0, "")];
    assert_eq!(numerar(&mut p, "V-", None, false, 15.0), 6);
    // fila de arriba primero, de oeste a este
    let por_pos = |p: &[Predio], x: f64, y: f64| p.iter().find(|q| q.xy[0] == [x, y]).unwrap().nombre.clone();
    assert_eq!((por_pos(&p, 0.0, 30.0), por_pos(&p, 20.0, 30.0), por_pos(&p, 0.0, 0.0), por_pos(&p, 20.0, 0.0)), ("V-001".into(), "V-003".into(), "V-004".into(), "V-006".into()));
    assert!(validar(&p, 1.0).is_empty(), "{:?}", validar(&p, 1.0));
    // con eje: orden por progresiva (eje de este a oeste)
    numerar(&mut p, "L", Some(&[[100.0, 15.0], [-10.0, 15.0]]), false, 15.0);
    assert_eq!(por_pos(&p, 20.0, 0.0).as_str(), "L001", "el más al este primero");
    // solo vacíos: conserva los existentes y no repite
    p[0].nombre.clear();
    let antes = p[1].nombre.clone();
    assert_eq!(numerar(&mut p, "L", None, true, 15.0), 1);
    assert_eq!(p[1].nombre, antes);
    assert!(validar(&p, 1.0).is_empty(), "{:?}", validar(&p, 1.0));
    // errores: repetido, sin nombre, degenerado, superpuesto
    p[2].nombre = p[3].nombre.clone();
    p.push(lote(1.0, 1.0, ""));
    p.push(Predio { nombre: "X".into(), xy: vec![[0.0, 0.0], [1.0, 0.0]] });
    let a = validar(&p, 1.0).join(" | ");
    assert!(a.contains("repetido") && a.contains("sin nombre") && a.contains("degenerado") && a.contains("superponen"), "{a}");
}
