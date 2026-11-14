//! Comparación de LineCraft con los reportes exportados por REDLIN (port de
//! `examples/validacion/comparar_redlin.py`). No se distribuyen datos de REDLIN: los `.xls` se
//! exportan con REDLIN y se pasan como argumentos.
//!
//! ```text
//! comparar-redlin PlanillaEstTotal.xls                                            # catenaria
//! comparar-redlin TablaFlechado.XLS Suministros.XLS --flechado                    # tiro vs temperatura
//! comparar-redlin Planilla.xls TablaFlechado.XLS Suministros.XLS Configuracion.XLS --final
//! comparar-redlin Planilla.xls CalculoMecanicoEst.xls Suministros.XLS Configuracion.XLS --cargas
//! comparar-redlin Planilla.xls CalculoMecanicoEst.xls Suministros.XLS --postes
//! ```

use std::collections::BTreeMap;
use std::f64::consts::PI;

use calamine::{Data, Reader};
use suite_motor::catalogos::{factor_seguridad_poste, momentos_poste, Poste};
use suite_motor::conductor::{carga_unitaria, Conductor, Hipotesis, G};
use suite_motor::linea::Cargas;
use suite_motor::mecanica::{cambio_estado, deformacion_creep, Curva, Modelo};

type Hoja = Vec<Vec<Data>>;

/// Hoja completa desde la celda A1 (como `pd.read_excel(header=None)`).
fn hoja(ruta: &str, nombre: Option<&str>) -> Result<Hoja, String> {
    let mut wb = calamine::open_workbook_auto(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    let r = match nombre {
        Some(n) => wb.worksheet_range(n).map_err(|e| format!("{ruta} [{n}]: {e}"))?,
        None => wb.worksheet_range_at(0).ok_or(format!("{ruta}: sin hojas"))?.map_err(|e| e.to_string())?,
    };
    let (f0, c0) = r.start().map(|(a, b)| (a as usize, b as usize)).unwrap_or((0, 0));
    let mut out: Hoja = vec![Vec::new(); f0];
    for fila in r.rows() {
        let mut v = vec![Data::Empty; c0];
        v.extend(fila.iter().cloned());
        out.push(v);
    }
    Ok(out)
}

fn celda(h: &Hoja, i: usize, j: usize) -> &Data {
    h.get(i).and_then(|f| f.get(j)).unwrap_or(&Data::Empty)
}

/// `pd.to_numeric(errors="coerce")`.
fn num(d: &Data) -> f64 {
    match d {
        Data::Float(x) => *x,
        Data::Int(i) => *i as f64,
        Data::Bool(b) => *b as i32 as f64,
        Data::String(s) => s.trim().parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

/// `float(x)` de Python: error si el texto no es número.
fn flotante(d: &Data) -> Option<f64> {
    match d {
        Data::String(s) => s.trim().parse().ok(),
        Data::Empty => Some(f64::NAN),
        otro => Some(num(otro)),
    }
}

/// `str(x)` de Python.
fn texto(d: &Data) -> String {
    match d {
        Data::String(s) => s.clone(),
        Data::Empty => "nan".into(),
        Data::Float(x) => suite_motor::salidas::texto_python(*x),
        Data::Int(i) => i.to_string(),
        otro => otro.to_string(),
    }
}

fn ancho(h: &Hoja) -> usize {
    h.iter().map(|f| f.len()).max().unwrap_or(0)
}

/// Cuantil con interpolación lineal (pandas); NaN si no hay datos.
fn cuantil(v: &[f64], q: f64) -> f64 {
    let mut s: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
    if s.is_empty() {
        return f64::NAN;
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = q * (s.len() - 1) as f64;
    let (i, f) = (p.floor() as usize, p - p.floor());
    if i + 1 < s.len() { s[i] + (s[i + 1] - s[i]) * f } else { s[i] }
}

fn mediana(v: &[f64]) -> f64 {
    cuantil(v, 0.5)
}

fn catenaria(a: f64, h: f64, c: f64) -> Curva {
    Curva::nueva(Modelo::Catenaria, a, h, c, 1.0)
}

// ---------------------------------------------------------------- planilla (catenaria)

fn planilla(ruta: &str) -> Result<(), String> {
    let hj = hoja(ruta, None)?;
    // grupos por derivación en orden de aparición
    let mut orden: Vec<String> = Vec::new();
    let mut grupos: BTreeMap<String, Vec<[f64; 8]>> = BTreeMap::new();
    for i in 2..hj.len() {
        let der = celda(&hj, i, 1);
        if matches!(der, Data::Empty) {
            continue;
        }
        let der = texto(der);
        let v = [2, 10, 14, 18, 19, 50, 51, 52].map(|j| num(celda(&hj, i, j)));
        if !grupos.contains_key(&der) {
            orden.push(der.clone());
        }
        grupos.entry(der).or_default().push(v);
    }
    let (mut vv, mut f, mut l, mut vp) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut malos = Vec::new();
    for der in &orden {
        let mut g = grupos[der].clone();
        g.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
        let ultimo = g.len() - 1;
        // [n, desn, vad, vp, vv, C, L, f]
        let h: Vec<f64> = g
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let ok = i < ultimo && r[2] > 0.0 && r[5] > 0.0;
                if !ok {
                    return f64::NAN;
                }
                let s = 2.0 * r[5] * (r[2] / (2.0 * r[5])).sinh();
                (r[6] * r[6] - s * s).max(0.0).sqrt().copysign(r[1])
            })
            .collect();
        for (i, r) in g.iter().enumerate() {
            let vv_lc = ((if i > 0 { g[i - 1][2] } else { 0.0 }) + (if i < ultimo { r[2] } else { 0.0 })) / 2.0;
            vv.push((r[4], vv_lc));
            if (vv_lc - r[4]).abs() > 0.01 {
                malos.push(format!("{der:>22} {:>2} {:>8.3} {:>8.3}", r[0], r[4], vv_lc));
            }
            if !h[i].is_nan() {
                let c = catenaria(r[2], h[i], r[5]);
                f.push((r[7], h[i] / 2.0 - c.y(r[2] / 2.0)));
                l.push((r[6], c.longitud()));
                if i > 0 && !h[i - 1].is_nan() {
                    let p = &g[i - 1];
                    let ca = catenaria(p[2], h[i - 1], p[5]);
                    vp.push((r[3], p[2] - ca.x_vertice() + c.x_vertice()));
                }
            }
        }
    }
    for (nombre, pares) in [("vv", &vv), ("f", &f), ("L", &l), ("vp", &vp)] {
        let e: Vec<f64> = pares.iter().filter(|(a, b)| !a.is_nan() && !b.is_nan()).map(|(a, b)| (b - a).abs()).collect();
        println!("{nombre:3} n={:4}  error máx. {:8.4} m  mediana {:.4} m", e.len(), e.iter().copied().fold(f64::NAN, f64::max), mediana(&e));
    }
    if !malos.is_empty() {
        println!("Vano viento distinto (inicio de derivación: REDLIN excluye el vano de enlace):");
        println!("{:>22} {:>2} {:>8} {:>8}", "der", "n", "vv_R", "vv_LC");
        for m in malos {
            println!("{m}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- conductores e hipótesis

/// Conductores de la hoja Conductor de Suministros.XLS (kg, mm, kg/mm², 1/°C) en SI.
fn conductores(ruta: &str) -> Result<BTreeMap<String, Conductor>, String> {
    let c = hoja(ruta, Some("Conductor"))?;
    let mut out = BTreeMap::new();
    for i in 4..c.len() {
        let x = |j: usize| flotante(celda(&c, i, j));
        let curva = |a: usize| (a..a + 5).map(x).collect::<Option<Vec<f64>>>();
        let (Some(s), Some(d), Some(m), Some(r), Some(e), Some(al), Some(ci), Some(cc)) =
            (x(8), x(7), x(5), x(6), x(12), x(11), curva(15), curva(20))
        else {
            continue;
        };
        let nombre = texto(celda(&c, i, 1));
        out.insert(nombre.clone(), Conductor {
            nombre,
            fuente: String::new(),
            seccion: s * 1e-6,
            diametro: d * 1e-3,
            peso: m * G,
            carga_rotura: r * G,
            modulo_elasticidad: e * G * 1e6,
            coef_dilatacion: al,
            curva_inicial: ci,
            curva_creep: cc,
        });
    }
    Ok(out)
}

/// [(z_min, z_max, [(nombre, (temperatura, presión de viento Pa))])] de la hoja Hipotesis.
fn hipotesis_por_altitud(ruta: &str) -> Result<Vec<(f64, f64, Vec<(String, (f64, f64))>)>, String> {
    let h = hoja(ruta, Some("Hipotesis"))?;
    let mut rangos = Vec::new();
    let mut s = 2;
    while s + 11 < ancho(&h) {
        let (z0, z1) = (num(celda(&h, 1, s + 2)), num(celda(&h, 1, s + 3)));
        if !z0.is_nan() {
            let mut hips: Vec<(String, (f64, f64))> = Vec::new();
            for i in 4..h.len() {
                if matches!(celda(&h, i, 0), Data::Empty) {
                    continue;
                }
                let n = texto(celda(&h, i, 0));
                let v = (num(celda(&h, i, s)), num(celda(&h, i, s + 11)) * (num(celda(&h, i, s + 8)) / 3.6).powi(2));
                match hips.iter_mut().find(|x| x.0 == n) {
                    Some(x) => x.1 = v,
                    None => hips.push((n, v)),
                }
            }
            rangos.push((z0, z1, hips));
        }
        s += 12;
    }
    Ok(rangos)
}

fn rango<'a>(rangos: &'a [(f64, f64, Vec<(String, (f64, f64))>)], z: f64) -> Option<&'a Vec<(String, (f64, f64))>> {
    let ult = rangos.last()?.0;
    rangos.iter().find(|(z0, z1, _)| (*z0 <= z && z < *z1) || z >= ult).map(|r| &r.2)
}

fn empieza5(k: &str, p: &str) -> bool {
    k.chars().take(5).collect::<String>() == p.chars().take(5).collect::<String>()
}

// ---------------------------------------------------------------- flechado

fn flechado(ruta_tabla: &str, ruta_sum: &str) -> Result<(), String> {
    let cond = conductores(ruta_sum)?;
    let df = hoja(ruta_tabla, None)?;
    let temps: Vec<f64> = (11..ancho(&df)).map(|j| num(celda(&df, 1, j))).filter(|t| !t.is_nan()).collect();
    let cab = texto(celda(&df, 0, 0));
    let t_ref: f64 = cab
        .split("Temperatura:")
        .nth(1)
        .and_then(|s| s.split('°').next())
        .and_then(|s| s.split('░').next())
        .and_then(|s| s.trim().parse().ok())
        .ok_or("TablaFlechado: no se encontró la temperatura de referencia")?;
    let mut filas = Vec::new();
    for i in 2..df.len().saturating_sub(2) {
        let Some(c) = cond.get(&texto(celda(&df, i, 8))) else { continue };
        if !texto(celda(&df, i, 10)).starts_with("Flecha") {
            continue;
        }
        let (ar, a, h) = (num(celda(&df, i, 4)), num(celda(&df, i, 5)), num(celda(&df, i, 6)));
        let hh: Vec<f64> = (0..temps.len()).map(|k| num(celda(&df, i + 2, 11 + k)) * G).collect();
        let h_ref = hh[temps.iter().position(|t| *t == t_ref).ok_or("temperatura de referencia fuera de la tabla")?];
        for (k, t) in temps.iter().enumerate() {
            let hl = cambio_estado(h_ref / c.seccion, c.peso, t_ref, c.peso, *t, ar, c, if a == ar { h } else { 0.0 })? * c.seccion;
            filas.push((ar, hh[k] / G, hl / G));
        }
    }
    let e: Vec<f64> = filas.iter().map(|(_, r, l)| (100.0 * (l / r - 1.0)).abs()).collect();
    println!("Tiro H vs temperatura: {} puntos  |error| mediana {:.3} %  p95 {:.3} %  máx {:.2} %", filas.len(), mediana(&e),
        cuantil(&e, 0.95), e.iter().copied().fold(f64::NAN, f64::max));
    for lim in [20.0, 50.0] {
        let s: Vec<f64> = filas.iter().zip(&e).filter(|((ar, _, _), _)| *ar >= lim).map(|(_, x)| *x).collect();
        println!("  vanos >= {lim} m: p95 {:.3} %  máx {:.3} %", cuantil(&s, 0.95), s.iter().copied().fold(f64::NAN, f64::max));
    }
    Ok(())
}

// ---------------------------------------------------------------- estado final con creep

fn final_(planilla: &str, tabla: &str, sum: &str, config: &str) -> Result<(), String> {
    let (cond, rangos) = (conductores(sum)?, hipotesis_por_altitud(config)?);
    let tf = hoja(tabla, None)?;
    let mut tramos: BTreeMap<(String, i64), usize> = BTreeMap::new();
    for i in 2..tf.len() {
        if texto(celda(&tf, i, 10)).starts_with("Flecha") {
            tramos.insert((texto(celda(&tf, i, 1)), num(celda(&tf, i, 2)) as i64), i);
        }
    }
    let pl = hoja(planilla, None)?;
    let mut filas: Vec<(&str, f64, f64)> = Vec::new(); // (grupo, err tramo, err por vano)
    for i in 2..pl.len() {
        let n = num(celda(&pl, i, 2));
        if n.is_nan() {
            continue;
        }
        let Some(&k) = tramos.get(&(texto(celda(&pl, i, 1)), n as i64 + 1)) else { continue };
        let cr = num(celda(&pl, i, 50));
        let Some(c) = cond.get(&texto(celda(&tf, k, 8))) else { continue };
        if !(cr > 0.0) {
            continue;
        }
        let t = |j: usize| num(celda(&tf, k, j));
        let (ar, eds, h, a, z) = (t(4), t(9), t(6), t(5), t(7));
        let hips = rango(&rangos, z).ok_or("sin rangos de altitud")?;
        let t0 = hips.iter().find(|x| x.0 == "Templado").ok_or("falta la hipótesis Templado")?.1 .0;
        let (t2, p2) = hips.iter().find(|x| empieza5(&x.0, "Máximo Esfuerzo")).ok_or("falta Máximo Esfuerzo")?.1;
        let w0 = c.peso;
        let w2 = carga_unitaria(c, &Hipotesis::nueva("f", t2, p2))?.resultante();
        let hh = if (a - ar).abs() < 0.01 { h } else { 0.0 };
        let s1 = eds / 100.0 * c.carga_rotura / c.seccion;
        let dt = deformacion_creep(c, s1, w0, t0, ar, hh)? / c.coef_dilatacion;
        let c_lc = cambio_estado(s1, w0, t0, w2, t2 + dt, ar, c, hh)? * c.seccion / w2;
        let c_pv = cambio_estado(s1, w0, t0, w2, t2 + dt, a, c, h)? * c.seccion / w2;
        let grupo = if hh != 0.0 || (a - ar).abs() < 0.01 { "1" } else { "varios" };
        filas.push((grupo, 100.0 * (c_lc / cr - 1.0), 100.0 * (c_pv / cr - 1.0)));
    }
    let todos: Vec<f64> = filas.iter().map(|f| f.1.abs()).collect();
    println!("Estado final con creep (C en máximo esfuerzo): {} vanos  |error| mediana {:.2} %", filas.len(), mediana(&todos));
    for (k, titulo) in [(1, "estado final por tramo"), (2, "estado final por vano")] {
        println!("  {titulo}:");
        for g in ["1", "varios"] {
            let e: Vec<f64> = filas.iter().filter(|f| f.0 == g).map(|f| if k == 1 { f.1 } else { f.2 }.abs()).collect();
            if !e.is_empty() {
                println!("{g:<9} n={} mediana {:.2} %  p90 {:.2} %", e.len(), mediana(&e), cuantil(&e, 0.9));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- cargas y postes

/// `num(x) or 0.0` de Python: 0 → 0.0; NaN se conserva (es «verdadero»).
fn o_cero(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

fn cargas(planilla: &str, cme: &str, sum: &str, config: &str) -> Result<(), String> {
    let (cond, rangos) = (conductores(sum)?, hipotesis_por_altitud(config)?);
    let pl: Hoja = hoja(planilla, None)?.into_iter().skip(2).collect();
    let cm: Hoja = hoja(cme, None)?.into_iter().skip(4).collect();
    let mut f: Vec<[(f64, f64); 3]> = Vec::new();
    for i in 0..pl.len().min(cm.len()) {
        let th = o_cero(num(celda(&pl, i, 13))).abs().to_radians() / 2.0;
        let (at, ad) = (num(celda(&cm, i, 25)), num(celda(&cm, i, 26)));
        let mut fv_lc = f64::NAN;
        let (tipo, a, z) = (texto(celda(&pl, i, 57)), num(celda(&pl, i, 14)), num(celda(&pl, i, 9)));
        if let (Some(c), true) = (cond.get(&tipo), a > 0.0) {
            let hips = rango(&rangos, z).ok_or("sin rangos de altitud")?;
            let (t, p) = hips.iter().find(|x| empieza5(&x.0, "Máxim") && x.0.contains("Esf")).ok_or("falta Máximo Esfuerzo")?.1;
            fv_lc = carga_unitaria(c, &Hipotesis::nueva("x", t, p))?.viento / G * a / 2.0 * th.cos();
        }
        f.push([
            (num(celda(&cm, i, 41)), (at + ad) * th.sin()),
            (num(celda(&cm, i, 49)), (ad - at).abs() * th.cos()),
            (num(celda(&cm, i, 10)), fv_lc),
        ]);
    }
    for (k, n) in ["FT", "FL", "Fv"].iter().enumerate() {
        let e: Vec<f64> = f
            .iter()
            .map(|x| x[k])
            .filter(|(r, l)| !r.is_nan() && !l.is_nan() && r.abs() > 0.5)
            .map(|(r, l)| (100.0 * (l / r - 1.0)).abs())
            .collect();
        println!("{n:3} n={:3}  |error| mediana {:.3} %  p95 {:.3} %", e.len(), mediana(&e), cuantil(&e, 0.95));
    }
    Ok(())
}

fn postes(planilla: &str, cme: &str, sum: &str) -> Result<(), String> {
    let sop = hoja(sum, Some("Soporte"))?;
    let mut postes: BTreeMap<String, Poste> = BTreeMap::new();
    for i in 4..sop.len() {
        if texto(celda(&sop, i, 5)) == "POSTE-MADERA" {
            let l = num(celda(&sop, i, 4));
            postes.insert(texto(celda(&sop, i, 1)), Poste {
                longitud_m: Some(l),
                empotramiento_m: Some(0.1 * l + 0.6),
                diam_punta_m: Some(num(celda(&sop, i, 16)) / PI),
                diam_tierra_m: Some(num(celda(&sop, i, 17)) / PI),
                mor_mpa: Some(num(celda(&sop, i, 18))),
                ..Default::default()
            });
        }
    }
    let pl: Hoja = hoja(planilla, None)?.into_iter().skip(2).collect();
    let cm: Hoja = hoja(cme, None)?.into_iter().skip(4).collect();
    // (retenida, armado, [MVE, MRN, FS] (R, LC))
    let mut filas: Vec<(bool, String, [(f64, f64); 3])> = Vec::new();
    for i in 0..pl.len().min(cm.len()) {
        let Some(p) = postes.get(&texto(celda(&pl, i, 22))) else { continue };
        let ret = o_cero(num(celda(&pl, i, 28))) + o_cero(num(celda(&pl, i, 29))) > 0.0;
        if num(celda(&pl, i, 21)) != 1.0 {
            continue;
        }
        let x = |j: usize| num(celda(&cm, i, j));
        let fv = (x(9) + x(10)) * G;
        let (ft, fl) = (x(41) * G, x(49) * G);
        let (mvc, mtd, mve_r) = (x(57), x(67), x(58));
        let sh = if fl > 1.0 { mtd / fl } else if fv > 1.0 { mvc / fv } else { f64::NAN };
        let libre = p.longitud_m.unwrap() - p.empotramiento_m.unwrap();
        let pres = mve_r * 6.0 / (libre.powi(2) * (2.0 * p.diam_punta_m.unwrap() + p.diam_tierra_m.unwrap()));
        let pres = if (pres - 231.8).abs() < (pres - 266.1).abs() { 0.613 * (70.0f64 / 3.6).powi(2) } else { 0.613 * (75.0f64 / 3.6).powi(2) };
        if sh.is_nan() {
            continue;
        }
        let c = Cargas {
            estructura: 0,
            hipotesis: "ME".into(),
            vano_viento: 0.0,
            vano_peso: 0.0,
            transversal: 3.0 * (ft + fv),
            longitudinal: 3.0 * fl,
            vertical: 0.0,
            horizontal: 0.0,
            momento_base: 0.0,
            tiro_retenida: 0.0,
            viento_conductor: 3.0 * fv,
            presion_viento: pres,
        };
        let arm = texto(celda(&cm, i, 5));
        let toma = if !ret {
            None
        } else if arm.starts_with("PR") {
            Some("todo")
        } else if arm.starts_with("TS") || arm.starts_with("PTV") {
            Some("longitudinal")
        } else {
            Some("angulo")
        };
        let m = momentos_poste(&c, &[sh / 3.0; 3], p, toma);
        let (fs, _) = factor_seguridad_poste(p, m.mrn);
        filas.push((ret, arm, [(mve_r, m.mve), (x(59), m.mrn), (x(64), fs)]));
    }
    println!("MRN por grupo de armado (PR retención, PA/PS ángulo, TS terminal):");
    let mut grupos: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (ret, arm, v) in &filas {
        let g = if *ret { arm.chars().take(2).collect() } else { "sin".to_string() };
        grupos.entry(g).or_default().push((100.0 * (v[1].1 / v[1].0 - 1.0)).abs());
    }
    println!("grupo");
    for (g, e) in &grupos {
        println!("{g:<6} n={} mediana {:.3} %  p90 {:.2} %", e.len(), mediana(e), cuantil(e, 0.9));
    }
    for (k, n) in ["MVE", "MRN", "FS"].iter().enumerate() {
        for ret in [false, true] {
            let e: Vec<f64> = filas
                .iter()
                .filter(|f| f.0 == ret)
                .map(|f| f.2[k])
                .filter(|(r, l)| !r.is_nan() && !l.is_nan() && *r > 0.0)
                .map(|(r, l)| (100.0 * (l / r - 1.0)).abs())
                .collect();
            println!("{n:3} {} retenida n={:3}  |error| mediana {:.3} %  p90 {:.3} %", if ret { "con" } else { "sin" }, e.len(),
                mediana(&e), cuantil(&e, 0.9));
        }
    }
    Ok(())
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let s: Vec<&str> = a.iter().map(String::as_str).collect();
    let r = match s.as_slice() {
        [p, c, su, "--postes"] => postes(p, c, su),
        [p, c, su, cf, "--cargas"] => cargas(p, c, su, cf),
        [p, t, su, cf, "--final"] => final_(p, t, su, cf),
        [t, su, "--flechado"] => flechado(t, su),
        [p] => planilla(p),
        _ => Err("uso: ver la documentación al inicio de comparar-redlin.rs".into()),
    };
    if let Err(e) = r {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
