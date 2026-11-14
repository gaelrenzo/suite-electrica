//! PresuCraft frente a PRESIO 2023 (no se distribuyen datos de PRESIO).
//!
//! ```text
//! comparar-presio APU.xlsx --apu              # análisis de precios unitarios
//! comparar-presio Presupuesto.xlsx --fp       # fórmula polinómica (hoja FP)
//! ```
//!
//! `--apu` recalcula cada partida desde sus insumos (cuadrilla, rendimiento, cantidades, precios,
//! %mo) y compara cantidades, parciales, subtotales y total. `--fp` toma de la hoja FP los
//! parciales por Índice Unificado y la agrupación elegida (índice similar y monomio), recalcula
//! con PresuCraft y compara incidencias, grupos preliminares y coeficientes.

use std::collections::BTreeMap;

use calamine::{Data, Reader};
use suite_motor::apu::{calcular_apu, formula_polinomica, ElementoFp, Insumo, PartidaApu, TipoInsumo};

fn num(d: &Data) -> f64 {
    match d {
        Data::Float(x) => *x,
        Data::Int(i) => *i as f64,
        Data::String(s) => s.trim().parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn texto(d: &Data) -> String {
    match d {
        Data::String(s) => s.trim().to_string(),
        Data::Empty => String::new(),
        Data::Float(x) if x.fract() == 0.0 => format!("{}", *x as i64),
        otro => otro.to_string(),
    }
}

fn hoja(ruta: &str, nombre: Option<&str>) -> Result<Vec<Vec<Data>>, String> {
    let mut wb = calamine::open_workbook_auto(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    let r = match nombre {
        Some(n) => wb.worksheet_range(n).map_err(|e| e.to_string())?,
        None => wb.worksheet_range_at(0).ok_or("sin hojas")?.map_err(|e| e.to_string())?,
    };
    let (f0, c0) = r.start().map(|(a, b)| (a as usize, b as usize)).unwrap_or((0, 0));
    let mut out = vec![Vec::new(); f0];
    for f in r.rows() {
        let mut v = vec![Data::Empty; c0];
        v.extend(f.iter().cloned());
        out.push(v);
    }
    Ok(out)
}

fn estadistica(nombre: &str, e: &mut Vec<f64>) {
    e.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let iguales = e.iter().filter(|x| **x < 0.005).count();
    println!("{nombre:<28} n={:5}  iguales (<0,005) {iguales:5}  máx. {:.4}", e.len(), e.last().copied().unwrap_or(0.0));
}

struct Leida {
    partida: PartidaApu,
    /// Por línea: (cantidad, parcial) de PRESIO.
    lineas: Vec<(f64, f64)>,
    subtotales: Vec<(TipoInsumo, f64)>,
    total: f64,
}

fn leer_apu(ruta: &str) -> Result<Vec<Leida>, String> {
    let h = hoja(ruta, None)?;
    let c = |i: usize, j: usize| h.get(i).and_then(|f| f.get(j)).cloned().unwrap_or(Data::Empty);
    let mut out: Vec<Leida> = Vec::new();
    let mut tipo = TipoInsumo::ManoObra;
    for i in 0..h.len() {
        let (c0, c1, c2) = (c(i, 0), texto(&c(i, 1)), texto(&c(i, 2)));
        if matches!(c0, Data::Float(x) if x >= 1000.0) {
            out.push(Leida {
                partida: PartidaApu { codigo: texto(&c0), descripcion: c1, unidad: String::new(), rendimiento: f64::NAN, insumos: Vec::new() },
                lineas: Vec::new(),
                subtotales: Vec::new(),
                total: f64::NAN,
            });
            continue;
        }
        let Some(a) = out.last_mut() else { continue };
        match c1.as_str() {
            "UNIDAD :" => a.partida.unidad = c2,
            "RENDIMIENTO :" => a.partida.rendimiento = num(&c(i, 2)),
            _ if TipoInsumo::desde_str(&c1).is_some() => tipo = TipoInsumo::desde_str(&c1).unwrap(),
            _ if c2 == "TOTAL" => a.total = num(&c(i, 6)),
            "" if texto(&c(i, 5)) == "Sub-total" => a.subtotales.push((tipo, num(&c(i, 6)))),
            "DESCRIPCIÓN" | "" | "PARTICIPACIÓN DE LA M. O. EN LOS COSTOS" => {}
            _ => {
                let (und, cua, cant, pu, parc) = (c2, num(&c(i, 3)), num(&c(i, 4)), num(&c(i, 5)), num(&c(i, 6)));
                if parc.is_nan() {
                    continue;
                }
                a.partida.insumos.push(Insumo {
                    tipo,
                    descripcion: c1,
                    unidad: und,
                    cuadrilla: (!cua.is_nan()).then_some(cua),
                    cantidad: Some(cant),
                    precio: if pu.is_nan() { 0.0 } else { pu },
                    iu: texto(&c(i, 7)),
                    base_pct: None,
                });
                a.lineas.push((cant, parc));
            }
        }
    }
    Ok(out)
}

fn comparar_apu(ruta: &str) -> Result<(), String> {
    let apus = leer_apu(ruta)?;
    let (mut e_cant, mut e_parc, mut e_sub, mut e_tot) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut peores: Vec<(f64, String)> = Vec::new();
    for a in &apus {
        // líneas «%» cuyo precio unitario (la base) es el subtotal de mano de obra
        let mut partida = a.partida.clone();
        let mo = a.subtotales.iter().find(|s| s.0 == TipoInsumo::ManoObra).map_or(f64::NAN, |s| s.1);
        for ins in partida.insumos.iter_mut().filter(|i| i.unidad.trim() == "%") {
            if (ins.precio - mo).abs() < 0.005 {
                ins.base_pct = Some(TipoInsumo::ManoObra);
            }
        }
        let calc = match calcular_apu(&partida, 8.0) {
            Ok(c) => c,
            Err(e) => {
                println!("{}: {e}", a.partida.codigo);
                continue;
            }
        };
        for (l, (q, p)) in calc.lineas.iter().zip(&a.lineas) {
            if !l.unidad.eq_ignore_ascii_case("%mo") {
                e_cant.push((l.cantidad - q).abs());
            }
            e_parc.push((l.parcial - p).abs());
        }
        // varias subsecciones de un mismo tipo: se suman
        let mut por_tipo: BTreeMap<TipoInsumo, f64> = BTreeMap::new();
        for (t, s) in &a.subtotales {
            *por_tipo.entry(*t).or_default() += s;
        }
        for (t, s) in &por_tipo {
            let d = (calc.subtotales[*t as usize] - s).abs();
            e_sub.push(d);
            if d >= 0.005 {
                peores.push((d, format!("{} subtotal {:?}: PresuCraft {:.2} PRESIO {:.2}", a.partida.codigo, t, calc.subtotales[*t as usize], s)));
            }
        }
        let d = (calc.total - a.total).abs();
        e_tot.push(d);
        if d >= 0.005 {
            peores.push((d, format!("{} {}: PresuCraft {:.2} PRESIO {:.2}", a.partida.codigo, a.partida.descripcion, calc.total, a.total)));
        }
    }
    println!("{} partidas de APU", apus.len());
    estadistica("Cantidad (h-h, h-m)", &mut e_cant);
    estadistica("Parcial (S/)", &mut e_parc);
    estadistica("Subtotal (S/)", &mut e_sub);
    estadistica("Total de la partida (S/)", &mut e_tot);
    peores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (_, p) in peores.iter().take(10) {
        println!("  {p}");
    }
    Ok(())
}

fn comparar_fp(ruta: &str) -> Result<(), String> {
    let h = hoja(ruta, Some("FP"))?;
    let c = |i: usize, j: usize| h.get(i).and_then(|f| f.get(j)).cloned().unwrap_or(Data::Empty);
    let mut i = 0;
    let mut secciones = 0;
    while i < h.len() {
        // encabezado de la tabla de elementos
        let enc: Vec<String> = h[i].iter().map(texto).collect();
        let Some(ciu) = enc.iter().position(|t| t == "Índice Unificado") else {
            i += 1;
            continue;
        };
        if !enc.iter().any(|t| t == "I.U. similar") {
            i += 1;
            continue;
        }
        let col = |n: &str| enc.iter().position(|t| t == n).unwrap();
        let (cpar, cinc, csim, cpre, csimb, cmon, cimon) = (col("Parcial (S/)"), col("% Incidenc."), col("I.U. similar"),
            col("Incidenc. Agrup. Prel."), col("Símbolo"), col("Agrupam. de monom."), col("Incidenc. Monom."));
        // sección: busca hacia arriba "Sección :"
        let seccion = (0..i).rev().find_map(|k| (0..h[k].len()).find(|&j| texto(&c(k, j)) == "Sección :").map(|j| {
            (j + 1..h[k].len()).map(|jj| texto(&c(k, jj))).find(|t| !t.is_empty()).unwrap_or_default()
        }));
        let mut el = Vec::new();
        let (mut sim, mut mon) = (BTreeMap::new(), BTreeMap::new());
        let mut presio_inc: Vec<(String, f64)> = Vec::new();
        let mut presio_pre: BTreeMap<String, f64> = BTreeMap::new();
        let mut presio_mon: BTreeMap<String, f64> = BTreeMap::new();
        let mut simbolo_de: BTreeMap<String, String> = BTreeMap::new();
        let mut k = i + 1;
        while k < h.len() {
            let iu = texto(&c(k, ciu));
            if iu.is_empty() {
                break;
            }
            let s = texto(&c(k, csim));
            el.push(ElementoFp { iu: iu.clone(), parcial: num(&c(k, cpar)), descripcion: String::new() });
            presio_inc.push((iu.clone(), num(&c(k, cinc))));
            if !s.is_empty() {
                sim.insert(iu.clone(), s.clone());
            }
            let pre = num(&c(k, cpre));
            if !pre.is_nan() {
                presio_pre.insert(iu.clone(), pre);
            }
            simbolo_de.insert(iu.clone(), texto(&c(k, csimb)));
            let m = texto(&c(k, cmon));
            if !m.is_empty() {
                mon.insert(iu.clone(), m.clone());
                let im = num(&c(k, cimon));
                if !im.is_nan() {
                    presio_mon.insert(m, im);
                }
            }
            k += 1;
        }
        i = k;
        if el.is_empty() {
            continue;
        }
        secciones += 1;
        let f = formula_polinomica(&el, &sim, &mon);
        println!("\n== Sección: {} — {} índices, total S/ {:.2}", seccion.unwrap_or_default(), el.len(), f.total);
        let (mut d_inc, mut d_pre) = (0usize, 0usize);
        for (iu, x) in &presio_inc {
            let g = f.incidencias.iter().find(|y| &y.0 == iu).map_or(f64::NAN, |y| y.1);
            if (g - x).abs() > 1e-9 {
                d_inc += 1;
                println!("  incidencia {iu}: PresuCraft {g:.3} PRESIO {x:.3}");
            }
        }
        for (iu, x) in &presio_pre {
            let g = f.preliminar.get(iu).copied().unwrap_or(f64::NAN);
            if (g - x).abs() > 5e-4 {
                d_pre += 1;
                println!("  agrupación preliminar {iu}: PresuCraft {g:.3} PRESIO {x:.3}");
            }
        }
        println!("  incidencias por índice: {} de {} iguales; grupos preliminares: {} de {} iguales", presio_inc.len() - d_inc,
            presio_inc.len(), presio_pre.len() - d_pre, presio_pre.len());
        println!("  {:<8} {:>10} {:>10}", "Monomio", "PresuCraft", "PRESIO");
        for m in &f.monomios {
            let p = presio_mon.get(&m.simbolo).copied();
            let ok = p.is_some_and(|p| (p - m.coeficiente).abs() < 5e-4);
            println!("  {:<8} {:>10.3} {:>10} {}", m.simbolo, m.coeficiente, p.map_or("—".into(), |p| format!("{p:.3}")), if ok { "" } else { "≠" });
        }
        println!("  suma {:.3}", f.monomios.iter().map(|m| m.coeficiente).sum::<f64>());
        for a in &f.avisos {
            println!("  AVISO: {a}");
        }
        let _ = &simbolo_de;
    }
    if secciones == 0 {
        return Err("no se encontró la tabla de la fórmula polinómica en la hoja FP".into());
    }
    Ok(())
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let r = match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [p, "--apu"] => comparar_apu(p),
        [p, "--fp"] => comparar_fp(p),
        _ => Err("uso: comparar-presio APU.xlsx --apu | comparar-presio Presupuesto.xlsx --fp".into()),
    };
    if let Err(e) = r {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
