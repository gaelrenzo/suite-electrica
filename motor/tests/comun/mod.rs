//! Lector común de los casos de referencia generados en Python.
#![allow(dead_code)]

use std::collections::HashMap;

use suite_motor::conductor::{Conductor, Hipotesis};
use suite_motor::geo::Perfil;
use suite_motor::linea::{EstructuraManual, Proyecto, Tipo};
use suite_motor::mecanica::Modelo;

pub fn n(s: &str) -> f64 {
    match s {
        "nan" => f64::NAN,
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        _ => s.parse().unwrap_or_else(|_| panic!("número inválido {s}")),
    }
}

pub struct Caso {
    pub p: Proyecto,
    pub res: Vec<Vec<String>>,
}

pub fn leer(texto: &str) -> Caso {
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

