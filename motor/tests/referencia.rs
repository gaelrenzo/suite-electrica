//! Compara el port Rust con los resultados de la versión Python (`tests/referencia.txt`,
//! generado por `motor/generar_referencia.py`).

use suite_motor::conductor::{carga_unitaria, Conductor, Hipotesis, G};
use suite_motor::mecanica::{
    cambio_estado, deformacion, deformacion_creep, vano_regulador, Curva, Modelo,
};

fn conductor(nombre: &str) -> Conductor {
    let mut c = Conductor {
        nombre: "AAAC 35".into(),
        seccion: 34.36e-6,
        diametro: 7.5e-3,
        peso: 0.0945 * G,
        carga_rotura: 10.199e3,
        modulo_elasticidad: 58860e6,
        coef_dilatacion: 23e-6,
        ..Default::default()
    };
    if nombre == "creep" {
        c.curva_inicial = vec![0.05, 50.0, 30.0, -80.0, 0.0];
        c.curva_creep = vec![0.0, 40.0];
    }
    c
}

fn num(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("número inválido: {s}"))
}

/// Error relativo con piso absoluto (valores cercanos a 0).
fn cerca(obtenido: f64, esperado: f64, rel: f64, abs: f64) -> bool {
    (obtenido - esperado).abs() <= rel * esperado.abs() + abs
}

#[test]
fn reproduce_python() {
    let texto = include_str!("referencia.txt");
    let mut n = 0;
    let mut fallos = Vec::new();
    for linea in texto.lines().filter(|l| !l.trim().is_empty()) {
        let (izq, der) = linea.split_once(" = ").unwrap();
        let p: Vec<&str> = izq.split_whitespace().collect();
        let esperado: Vec<f64> = der.split_whitespace().map(num).collect();
        let obtenido: Vec<f64> = match p[0] {
            "cambio_estado" => {
                let c = conductor(p[1]);
                let a: Vec<f64> = p[2..].iter().map(|s| num(s)).collect();
                vec![cambio_estado(a[0], a[1], a[2], a[3], a[4], a[5], &c, a[6]).unwrap()]
            }
            "deformacion_creep" => {
                let c = conductor(p[1]);
                let a: Vec<f64> = p[2..].iter().map(|s| num(s)).collect();
                vec![deformacion_creep(&c, a[0], a[1], a[2], a[3], a[4]).unwrap()]
            }
            "deformacion_ini" => vec![deformacion(&conductor(p[1]).curva_inicial, num(p[2])).unwrap()],
            "deformacion_creep_curva" => vec![deformacion(&conductor(p[1]).curva_creep, num(p[2])).unwrap()],
            "curva_catenaria" | "curva_parabola" => {
                let a: Vec<f64> = p[1..].iter().map(|s| num(s)).collect();
                let m = if p[0] == "curva_catenaria" { Modelo::Catenaria } else { Modelo::Parabola };
                let cv = Curva::nueva(m, a[0], a[1], a[2], a[3]);
                let mut v = vec![cv.x_vertice(), cv.y(a[0] * 0.3), cv.flecha(), cv.flecha_medio(), cv.tiro_maximo()];
                if m == Modelo::Catenaria {
                    v.push(cv.longitud());
                }
                v
            }
            "vano_regulador" => {
                let vs: Vec<f64> = p[1..].iter().map(|s| num(s)).collect();
                vec![vano_regulador(&vs).unwrap()]
            }
            "carga_unitaria" => {
                let a: Vec<f64> = p[1..].iter().map(|s| num(s)).collect();
                let mut h = Hipotesis::nueva("h", 0.0, a[0]);
                h.espesor_hielo = a[1];
                h.densidad_hielo = a[2];
                let cu = carga_unitaria(&conductor("base"), &h).unwrap();
                vec![cu.vertical, cu.viento, cu.resultante()]
            }
            otro => panic!("caso desconocido: {otro}"),
        };
        n += 1;
        // Tolerancias: 1e-9 relativo en fórmulas cerradas; 1e-6 donde Python itera con brentq.
        let rel = if p[0].starts_with("deformacion_creep") || (p[0] == "cambio_estado" && num(p[8]) != 0.0) {
            1e-6
        } else {
            1e-9
        };
        let ok = esperado.len() == obtenido.len()
            && esperado.iter().zip(&obtenido).all(|(e, o)| cerca(*o, *e, rel, 1e-9));
        if !ok {
            fallos.push(format!("{linea}\n  rust = {obtenido:?}"));
        }
    }
    assert!(n > 600, "pocos casos: {n}");
    assert!(fallos.is_empty(), "{} de {n} casos no coinciden:\n{}", fallos.len(), fallos[..fallos.len().min(8)].join("\n"));
}
