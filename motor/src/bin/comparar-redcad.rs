//! GridCraft frente a la «Planilla de Caída de Tensión» de REDCAD (Reportes > Planillas >
//! Planilla de Caída de Tensión, exportada a `.xls`). No se distribuyen datos de REDCAD.
//!
//! ```text
//! comparar-redcad CaidaTension.xls
//! ```
//!
//! Por subestación arma una red de GridCraft con los datos de entrada del reporte (longitud,
//! lotes, cargas especiales, lámparas, formación con K, R y fases), reconstruye el árbol por las
//! tensiones (V_nodo = V_padre − ΔV_tramo), calcula y compara nodo por nodo: potencia acumulada,
//! corriente, caída del tramo, tensión, %ΔV y pérdidas, en servicio particular y alumbrado.

use std::collections::{BTreeMap, HashMap};

use calamine::{Data, Reader};
use suite_motor::red::{calcular, ConductorRed, Nodo, Red};

fn num(d: &Data) -> f64 {
    match d {
        Data::Float(x) => *x,
        Data::Int(i) => *i as f64,
        Data::String(s) => s.trim().trim_end_matches('%').split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn texto(d: &Data) -> String {
    match d {
        Data::String(s) => s.trim().to_string(),
        Data::Empty => String::new(),
        otro => otro.to_string(),
    }
}

/// Fila de datos del reporte.
#[derive(Clone, Debug)]
struct Fila {
    id: String,
    padre: String,
    v: [f64; 25],
}

struct Sed {
    nombre: String,
    vff: f64,
    vfn: f64,
    filas: Vec<Fila>,
}

fn leer(ruta: &str) -> Result<Vec<Sed>, String> {
    let mut wb = calamine::open_workbook_auto(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    let r = wb.worksheet_range_at(0).ok_or("sin hojas")?.map_err(|e| e.to_string())?;
    let filas: Vec<Vec<Data>> = r.rows().map(|f| f.to_vec()).collect();
    let c = |i: usize, j: usize| filas.get(i).and_then(|f| f.get(j)).cloned().unwrap_or(Data::Empty);
    let mut seds: Vec<Sed> = Vec::new();
    let mut circuito = String::new();
    for i in 0..filas.len() {
        let t0 = texto(&c(i, 0));
        if t0 == "Localidad" {
            seds.push(Sed { nombre: texto(&c(i, 12)), vff: f64::NAN, vfn: f64::NAN, filas: Vec::new() });
            circuito.clear();
            continue;
        }
        let Some(s) = seds.last_mut() else { continue };
        if t0 == "Sistema" {
            // "0.38/0.22 kV ..." -> V fase-fase / fase-neutro
            let tx = texto(&c(i, 1));
            let v: Vec<f64> = tx.split_whitespace().next().unwrap_or("").split('/').filter_map(|x| x.parse().ok()).collect();
            if v.len() == 2 {
                s.vff = v[0] * 1e3;
                s.vfn = v[1] * 1e3;
            }
            continue;
        }
        if let Some(cto) = t0.strip_prefix("Circuito ") {
            circuito = cto.to_string();
            continue;
        }
        let es_dato = matches!(c(i, 1), Data::Float(_) | Data::Int(_)) && !t0.is_empty();
        if !es_dato {
            continue;
        }
        let mut v = [f64::NAN; 25];
        for (j, x) in v.iter_mut().enumerate() {
            *x = num(&c(i, j));
        }
        let id = match t0.as_str() {
            "Bornes BT" | "Tablero" => t0.clone(),
            _ => format!("{circuito}:{t0}"),
        };
        let mut f = Fila { id, padre: String::new(), v };
        f.v[7] = f64::NAN;
        s.filas.push(f);
        // la formación es texto (columna 7): se guarda aparte
        let form = texto(&c(i, 7));
        s.filas.last_mut().unwrap().padre = form; // temporal: formación (se reemplaza al armar el árbol)
    }
    Ok(seds)
}

/// Fases de la formación ("3x16+1x16/25" -> 3).
fn fases(form: &str) -> f64 {
    form.split('x').next().and_then(|n| n.trim().parse().ok()).unwrap_or(3.0)
}

struct Comparacion {
    nombre: &'static str,
    errores: Vec<f64>,
}

fn main() {
    let Some(ruta) = std::env::args().nth(1) else {
        eprintln!("uso: comparar-redcad CaidaTension.xls");
        std::process::exit(1);
    };
    let seds = leer(&ruta).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1)
    });
    let mut comp: Vec<Comparacion> = [
        "P acumulada SP (kW)", "Corriente SP (A)", "ΔV tramo f-n (V)", "Tensión f-n (V)", "%ΔV", "Pérdidas SP (W)",
        "P acumulada AP (kW)", "Corriente AP (A)", "%ΔV AP", "Pérdidas AP (W)",
    ]
    .iter()
    .map(|n| Comparacion { nombre: n, errores: Vec::new() })
    .collect();
    let mut sin_padre = 0;
    for s in &seds {
        // catálogo de formaciones y árbol por tensiones
        let mut cat: BTreeMap<String, ConductorRed> = BTreeMap::new();
        let mut nodos: Vec<Nodo> = Vec::new();
        let mut padres: HashMap<String, String> = HashMap::new();
        let filas = &s.filas;
        let (mut lotes, mut psp, mut lamp) = (0.0, 0.0, 0.0);
        for (i, f) in filas.iter().enumerate() {
            let form = f.padre.clone();
            let v = &f.v;
            let padre = if f.id == "Bornes BT" {
                String::new()
            } else if f.id == "Tablero" {
                "Bornes BT".into()
            } else if f.id.ends_with(":Salida") {
                "Tablero".into()
            } else {
                // padre: fila anterior del mismo circuito con V_padre − ΔV = V
                let cto = f.id.split(':').next().unwrap_or("");
                let mejor = filas[..i]
                    .iter()
                    .filter(|p| p.id.starts_with(&format!("{cto}:")))
                    .map(|p| (p.id.clone(), (p.v[13] - v[12] - v[13]).abs()))
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                match mejor {
                    Some((id, d)) if d < 0.006 => id,
                    _ => {
                        sin_padre += 1;
                        continue;
                    }
                }
            };
            if !padre.is_empty() && !cat.contains_key(&form) {
                cat.insert(form.clone(), ConductorRed {
                    formacion: form.clone(),
                    k_ohm_km: Some(v[8]),
                    r_ohm_km: Some(v[9]),
                    x_ohm_km: None,
                    // sin conductor de alumbrado REDCAD informa K = 0: la lámpara sigue en AP, sin caída
                    k_ap_ohm_km: Some(if v[18] > 0.0 { v[18] } else { 0.0 }),
                    r_ap_ohm_km: Some(if v[18] > 0.0 { v[19] } else { 0.0 }),
                    corriente_max_a: None,
                    fuente: "REDCAD".into(),
                    fases: Some(fases(&form)),
                });
            }
            // «Bornes BT» y «Tablero» muestran totales, no cargas propias
            let propia = f.id != "Tablero" && f.id != "Bornes BT";
            let carga = |x: f64| if propia { x.max(0.0) } else { 0.0 };
            lotes += carga(v[2]);
            psp += carga(v[4]);
            lamp += carga(v[16]);
            padres.insert(f.id.clone(), padre.clone());
            nodos.push(Nodo {
                nodo: f.id.clone(),
                padre,
                longitud_m: v[1].max(0.0),
                conductor: if f.id == "Bornes BT" { String::new() } else { form },
                lotes: carga(v[2]),
                kw_ce: carga(v[5]),
                lamparas: carga(v[16]),
                este: None,
                norte: None,
            });
        }
        let tablero = filas.iter().find(|f| f.id == "Tablero").unwrap();
        let vb = filas[0].v[13]; // tensión fase-neutro en bornes (380/√3 en 380/220 V)
        let red = Red {
            nombre: s.nombre.clone(),
            sistema: "3F".into(),
            tension_v: s.vff,
            nodos,
            conductores: cat.into_values().collect(),
            calificacion_kw_lote: if lotes > 0.0 { psp / lotes } else { 0.0 },
            fs_sp: 1.0,
            fp_sp: 1.0,
            fs_ce: 1.0,
            kw_lampara: if lamp > 0.0 { tablero.v[17] / lamp } else { 0.0 },
            fs_ap: 1.0,
            fp_ap: 0.9,
            tension_ap_v: Some(vb),
            tension_fn_v: Some(vb),
            caida_max_pct: 0.0,
            perdidas_max_pct: 0.0,
            factor_carga: 0.0,
            transformador_kva: 0.0,
            factor_sobrecarga: 1.0,
            tabla_fs: Vec::new(),
            fuentes: HashMap::new(),
        };
        let r = match calcular(&red) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}: {e}", s.nombre);
                continue;
            }
        };
        println!("{}: {} nodos, {} V f-f / {} V f-n, bornes {vb:.3} V, {:.2} kW/lote, {:.3} kW/lámpara", s.nombre, r.tramos.len(),
            s.vff, s.vfn, red.calificacion_kw_lote, red.kw_lampara);
        for t in &r.tramos {
            let f = filas.iter().find(|f| f.id == t.nodo).unwrap();
            let v = &f.v;
            let ap = t.ap.unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
            let pares = [
                (t.p_acum_kw, v[6]), (t.i_a, v[10]), (t.dv_tramo_v, v[12]), (vb - t.dv_acum_v, v[13]), (t.dv_pct, v[14]),
                (t.perdidas_w, v[15]), (ap.0, v[17]), (ap.1, v[20]), (ap.2, v[23]), (ap.3, v[24]),
            ];
            if std::env::var("DETALLE").is_ok() && ((pares[0].0 - pares[0].1).abs() > 0.02 || (pares[1].0 - pares[1].1).abs() > 0.15) {
                println!("  {} {}: P {:.3}/{} I {:.2}/{} dv {:.3}/{} form {}", s.nombre, t.nodo, pares[0].0, pares[0].1, pares[1].0, pares[1].1,
                    pares[2].0, pares[2].1, t.formacion);
            }
            for (k, (g, rc)) in pares.iter().enumerate() {
                if !g.is_nan() && !rc.is_nan() {
                    comp[k].errores.push((g - rc).abs());
                }
            }
        }
    }
    if sin_padre > 0 {
        println!("AVISO: {sin_padre} nodo(s) sin padre identificable por tensión");
    }
    println!("\nGridCraft − REDCAD (valor absoluto)    n     máx.      mediana");
    for c in &comp {
        let mut e = c.errores.clone();
        e.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = if e.is_empty() { f64::NAN } else { e[e.len() / 2] };
        println!("{:<34} {:>4} {:>9.4} {:>10.4}", c.nombre, e.len(), e.last().copied().unwrap_or(f64::NAN), med);
    }
}
