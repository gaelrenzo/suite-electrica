//! Cálculo completo de una línea: Rust vs Python (`tests/linea_*.txt`).

use std::collections::HashMap;

use suite_motor::conductor::{Conductor, Hipotesis};
use suite_motor::geo::Perfil;
use suite_motor::linea::{calcular, EstructuraManual, Proyecto, Tipo};
use suite_motor::mecanica::Modelo;

fn n(s: &str) -> f64 {
    match s {
        "nan" => f64::NAN,
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        _ => s.parse().unwrap_or_else(|_| panic!("número inválido {s}")),
    }
}

struct Caso {
    p: Proyecto,
    res: Vec<Vec<String>>,
}

fn leer(texto: &str) -> Caso {
    let mut c = Conductor { nombre: "c".into(), ..Default::default() };
    let mut hips = Vec::new();
    let mut par: HashMap<String, String> = HashMap::new();
    let mut fuentes = HashMap::new();
    let mut perfil = Perfil::default();
    let mut eje = Vec::new();
    let mut manual: Option<Vec<EstructuraManual>> = None;
    let mut res = Vec::new();
    let mut en_res = false;
    for l in texto.lines() {
        let t: Vec<&str> = l.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        if l.starts_with("== resultados") {
            en_res = true;
            continue;
        }
        if en_res {
            res.push(t.iter().map(|s| s.to_string()).collect());
            continue;
        }
        match t[0] {
            "conductor" => {
                c.seccion = n(t[1]);
                c.diametro = n(t[2]);
                c.peso = n(t[3]);
                c.carga_rotura = n(t[4]);
                c.modulo_elasticidad = n(t[5]);
                c.coef_dilatacion = n(t[6]);
            }
            "curva_inicial" => c.curva_inicial = t[1..].iter().map(|s| n(s)).collect(),
            "curva_creep" => c.curva_creep = t[1..].iter().map(|s| n(s)).collect(),
            "hip" => hips.push(Hipotesis {
                nombre: t[1].into(),
                temperatura: n(t[2]),
                presion_viento: n(t[3]),
                espesor_hielo: n(t[4]),
                densidad_hielo: n(t[5]),
                altitud_min: n(t[6]),
                altitud_max: n(t[7]),
            }),
            "param" => {
                par.insert(t[1].into(), t[2].into());
            }
            "fuente" => {
                fuentes.insert(t[1].to_string(), t[2..].join(" ").replace('_', " "));
            }
            "perfil" => {
                perfil.progresiva.push(n(t[1]));
                perfil.este.push(n(t[2]));
                perfil.norte.push(n(t[3]));
                perfil.cota.push(n(t[4]));
                perfil.tipo.push(t[5].chars().next().unwrap());
            }
            "eje" => eje.push([n(t[1]), n(t[2])]),
            "manual" => manual.get_or_insert_with(Vec::new).push(EstructuraManual {
                progresiva: n(t[1]),
                tipo: if t[2] == "-" { None } else { Some(Tipo::desde_str(t[2]).unwrap()) },
                altura_amarre: if t[3] == "-" { None } else { Some(n(t[3])) },
                ..Default::default()
            }),
            otro => panic!("clave desconocida {otro}"),
        }
    }
    let g = |k: &str| par[k].clone();
    let p = Proyecto {
        nombre: "caso".into(),
        conductor: c,
        n_conductores: g("n_conductores").parse().unwrap(),
        modelo: if g("modelo") == "parabola" { Modelo::Parabola } else { Modelo::Catenaria },
        hipotesis: hips,
        hip_inicial: g("hip_inicial"),
        tiro_inicial_pct: n(&g("tiro_inicial_pct")),
        hip_flecha_maxima: g("hip_flecha_maxima"),
        tiro_maximo_pct: n(&g("tiro_maximo_pct")),
        distancia_minima_terreno: n(&g("distancia_minima_terreno")),
        altura_amarre: n(&g("altura_amarre")),
        vano_maximo: n(&g("vano_maximo")),
        vano_minimo: n(&g("vano_minimo")),
        angulo_retenida: n(&g("angulo_retenida")),
        perfil,
        eje,
        estructuras_manual: manual,
        fuentes,
        alertas: HashMap::new(),
        condicion_final: g("condicion_final"),
        vanos_independientes: g("vanos_independientes") == "True",
    };
    Caso { p, res }
}

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
