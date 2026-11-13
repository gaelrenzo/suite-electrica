//! Cálculo completo de una línea: Rust vs Python (`tests/linea_*.txt`).

mod comun;

use comun::{leer, n};
use suite_motor::linea::calcular;

fn cerca(o: f64, e: f64, rel: f64) -> bool {
    (o - e).abs() <= rel * e.abs().max(1.0)
}

fn comparar(nombre: &str, texto: &str) {
    let caso = leer(texto);
    let r = calcular(&caso.p).unwrap_or_else(|e| panic!("{nombre}: {e}"));
    let rel = 2e-6;
    let mut fallos: Vec<String> = Vec::new();
    let mut chk = |etq: String, o: f64, e: f64| {
        if !cerca(o, e, rel) {
            fallos.push(format!("{etq}: rust {o} python {e}"));
        }
    };
    let (mut ie, mut it, mut iv, mut ic) = (0, 0, 0, 0);
    let mut iest: (usize, usize) = (0, 0);
    for t in &caso.res {
        match t[0].as_str() {
            "est" => {
                let e = &r.estructuras[ie];
                chk(format!("est{ie}.s"), e.progresiva, n(&t[1]));
                chk(format!("est{ie}.z"), e.cota_terreno, n(&t[4]));
                chk(format!("est{ie}.ang"), e.angulo, n(&t[7]));
                assert_eq!(e.tipo.como_str(), t[6], "{nombre}: tipo est{ie}");
                ie += 1;
            }
            "tramo" => {
                let tr = &r.tramos[it];
                assert_eq!((tr.inicio, tr.fin), (t[1].parse().unwrap(), t[2].parse().unwrap()), "{nombre}: tramo {it}");
                chk(format!("tramo{it}.ar"), tr.vano_regulador, n(&t[3]));
                iest = (it, 0);
                it += 1;
            }
            "estado" => {
                let s = &r.tramos[iest.0].estados[iest.1];
                assert_eq!(s.hipotesis, t[1]);
                chk(format!("tramo{}.{}.T", iest.0, t[1]), s.temperatura, n(&t[2]));
                chk(format!("tramo{}.{}.H", iest.0, t[1]), s.h, n(&t[3]));
                chk(format!("tramo{}.{}.tmax", iest.0, t[1]), s.tiro_maximo, n(&t[4]));
                iest.1 += 1;
            }
            "vano" => {
                let v = &r.vanos[iv];
                chk(format!("vano{iv}.L"), v.longitud, n(&t[3]));
                chk(format!("vano{iv}.despeje"), v.despeje, n(&t[5]));
                chk(format!("vano{iv}.fmedio"), v.flecha_medio, n(&t[7]));
                for (k, kv) in t[8..].iter().enumerate() {
                    let (h, x) = kv.split_once('=').unwrap();
                    assert_eq!(v.flechas[k].0, h);
                    chk(format!("vano{iv}.f.{h}"), v.flechas[k].1, n(x));
                }
                iv += 1;
            }
            "carga" => {
                let c = &r.cargas[ic];
                assert_eq!(c.hipotesis, t[2]);
                let vals = [
                    c.vano_viento, c.vano_peso, c.transversal, c.longitudinal, c.vertical, c.horizontal,
                    c.momento_base, c.tiro_retenida, c.viento_conductor, c.presion_viento,
                ];
                for (k, v) in vals.iter().enumerate() {
                    chk(format!("carga{ic}.{k}"), *v, n(&t[3 + k]));
                }
                ic += 1;
            }
            "avisos" => assert_eq!(r.avisos.len(), t[1].parse::<usize>().unwrap(), "{nombre}: avisos {:#?}", r.avisos),
            otro => panic!("resultado desconocido {otro}"),
        }
    }
    assert_eq!((ie, it, iv, ic), (r.estructuras.len(), r.tramos.len(), r.vanos.len(), r.cargas.len()));
    assert!(fallos.is_empty(), "{nombre}: {} diferencias:\n{}", fallos.len(), fallos[..fallos.len().min(10)].join("\n"));
}

#[test]
fn auto() {
    comparar("auto", include_str!("linea_auto.txt"));
}

#[test]
fn manual() {
    comparar("manual", include_str!("linea_manual.txt"));
}

#[test]
fn creep() {
    comparar("creep", include_str!("linea_creep.txt"));
}

#[test]
fn altitud_porvano() {
    comparar("altitud_porvano", include_str!("linea_altitud_porvano.txt"));
}
