//! Alertas de diseño con exceso `E = (P* − P) / P` (criterio de REDLIN). Port de `linecraft/alertas.py`.

use crate::catalogos::{Catalogos, FilaVerificacion};
use crate::linea::{fmt_g, Proyecto, Resultado};
use crate::mecanica::Curva;

/// Límites opcionales (0 = no verificar).
#[derive(Clone, Debug, Default)]
pub struct ConfigAlertas {
    pub fs_poste: f64,
    pub fs_retenida: f64,
    pub tension_kv: f64,
    /// Separación mínima `D = k_U·U + k_f·√f`: coeficiente de U (m/kV).
    pub sep_k_tension: f64,
    pub sep_k_flecha: f64,
    pub angulo_salida_min: f64,
    pub angulo_salida_max: f64,
    /// Carga máxima por espiga (N).
    pub tiro_max_espiga: f64,
    /// EDS máximo por vibraciones eólicas (% de rotura).
    pub eds_max_pct: f64,
}

/// Una alerta (`exceso` en %, ∞ si no hay magnitud permisible).
#[derive(Clone, Debug, PartialEq)]
pub struct Alerta {
    pub elemento: String,
    pub codigo: String,
    pub exceso: f64,
    pub detalle: String,
    /// La de mayor exceso de su elemento.
    pub critica: bool,
}

fn exceso(evaluado: f64, permitido: f64) -> f64 {
    100.0 * (evaluado - permitido) / permitido
}

fn r1(x: f64) -> f64 {
    if x.is_finite() { (x * 10.0).round() / 10.0 } else { x }
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Ángulo (°) del conductor al salir del soporte hacia el vano (+ hacia arriba).
pub fn pendiente_salida(c: &Curva, extremo_a: bool) -> f64 {
    let dx = 0.01f64.min(c.a / 1000.0);
    if extremo_a {
        ((c.y(dx) - c.y(0.0)) / dx).atan().to_degrees()
    } else {
        ((c.y(c.a - dx) - c.y(c.a)) / dx).atan().to_degrees()
    }
}

/// Alertas ordenadas de mayor a menor exceso.
pub fn alertas(
    r: &Resultado, p: &Proyecto, cat: Option<&Catalogos>, ver: &[FilaVerificacion], cfg: &ConfigAlertas,
) -> Vec<Alerta> {
    let est = &r.estructuras;
    let mut filas: Vec<Alerta> = Vec::new();
    let mut add = |elem: String, cod: &str, e: f64, det: String| {
        if e > 1e-9 {
            filas.push(Alerta { elemento: elem, codigo: cod.into(), exceso: r1(e), detalle: det, critica: false });
        }
    };

    // vanos y tramos
    let hf = &p.hip_flecha_maxima;
    for v in &r.vanos {
        let nombre = format!("Vano {}-{}", est[v.a].codigo(), est[v.a + 1].codigo());
        let e = if v.despeje > 0.0 { exceso(p.distancia_minima_terreno, v.despeje) } else { f64::INFINITY };
        add(
            nombre.clone(),
            "DMS",
            e,
            format!("despeje {:.2} m < {:.2} m en {:.1} m", v.despeje, p.distancia_minima_terreno, v.s_despeje),
        );
        if cfg.tension_kv != 0.0 && (cfg.sep_k_tension != 0.0 || cfg.sep_k_flecha != 0.0) {
            if let Some(cat) = cat {
                let seps: Vec<f64> = [v.a, v.a + 1]
                    .iter()
                    .filter_map(|&i| cat.armado(&est[i].armado))
                    .filter_map(|a| a.separacion_fases_m)
                    .collect();
                if !seps.is_empty() {
                    let actual = seps.iter().copied().fold(f64::INFINITY, f64::min);
                    let f = v.flechas.iter().find(|x| &x.0 == hf).map(|x| x.1).unwrap_or(0.0);
                    let req = cfg.sep_k_tension * cfg.tension_kv + cfg.sep_k_flecha * f.max(0.0).sqrt();
                    add(nombre, "H", exceso(req, actual), format!("separación {actual:.2} m < {req:.2} m requerida"));
                }
            }
        }
    }
    for t in &r.tramos {
        let nombre = format!("Tramo {}-{}", est[t.inicio].codigo(), est[t.fin].codigo());
        for s in &t.estados {
            add(
                nombre.clone(),
                "C",
                exceso(s.pct_rotura, p.tiro_maximo_pct),
                format!("{}: tiro {:.1} % > {} %", s.hipotesis, s.pct_rotura, fmt_g(p.tiro_maximo_pct)),
            );
        }
        if cfg.eds_max_pct != 0.0 {
            if let Some(s) = t.estado(&p.hip_inicial) {
                let eds = 100.0 * s.h / p.conductor.carga_rotura;
                add(
                    nombre,
                    "V",
                    exceso(eds, cfg.eds_max_pct),
                    format!("EDS {eds:.1} % > {} % (vibraciones eólicas)", fmt_g(cfg.eds_max_pct)),
                );
            }
        }
    }

    // postes y retenidas (FS de la verificación, redondeado como en la planilla)
    for f in ver {
        let Some(e) = est.iter().find(|x| x.codigo() == f.estructura) else { continue };
        if cfg.fs_poste != 0.0 {
            if let Some(fs) = f.fs_poste.filter(|x| x.is_finite()).map(r2).filter(|&x| x > 0.0) {
                let mut peor: Option<&crate::linea::Cargas> = None;
                for c in r.cargas.iter().filter(|c| c.estructura == e.numero) {
                    if peor.map_or(true, |p| c.horizontal >= p.horizontal) {
                        peor = Some(c);
                    }
                }
                if let Some(c) = peor {
                    let cod = if c.transversal >= c.longitudinal { "T" } else { "L" };
                    add(
                        f.estructura.clone(),
                        cod,
                        exceso(cfg.fs_poste, fs),
                        format!("FS poste {fs:.2} < {} ({})", fmt_g(cfg.fs_poste), c.hipotesis),
                    );
                }
            }
        }
        if cfg.fs_retenida != 0.0 {
            if let Some(fsr) = f.fs_retenida.filter(|x| x.is_finite()).map(r2).filter(|&x| x > 0.0) {
                add(
                    f.estructura.clone(),
                    "R",
                    exceso(cfg.fs_retenida, fsr),
                    format!("FS retenida {fsr:.2} < {}", fmt_g(cfg.fs_retenida)),
                );
            }
        }
    }

    // verticales y aisladores
    let phi = p.angulo_retenida.to_radians();
    for (i, e) in est.iter().enumerate() {
        let ce: Vec<&crate::linea::Cargas> = r.cargas.iter().filter(|c| c.estructura == e.numero).collect();
        if let Some(cmax) = cat.and_then(|c| c.poste(&e.poste)).and_then(|p| p.carga_vertical_max_dan) {
            let comp = ce.iter().map(|c| c.vertical + c.tiro_retenida * phi.sin()).fold(f64::NEG_INFINITY, f64::max);
            add(
                e.codigo(),
                "S↓",
                exceso(comp, cmax * 10.0),
                format!("compresión {comp:.0} N > {:.0} N", cmax * 10.0),
            );
        }
        let Some(arm) = cat.and_then(|c| c.armado(&e.armado)) else { continue };
        if !arm.aislador.eq_ignore_ascii_case("RIGIDO") {
            continue;
        }
        if let Some(c) = ce.iter().copied().filter(|c| c.vano_peso < 0.0).fold(None::<&crate::linea::Cargas>, |m, c| {
            if m.map_or(true, |m| c.vano_peso < m.vano_peso) { Some(c) } else { m }
        }) {
            add(
                e.codigo(),
                "S↑",
                f64::INFINITY,
                format!("vano peso {:.1} m en {} (arrancamiento)", c.vano_peso, c.hipotesis),
            );
        }
        let n_ais = arm.aisladores_por_fase.unwrap_or(1.0);
        if cfg.tiro_max_espiga != 0.0 && !ce.is_empty() {
            let f_fase = ce.iter().map(|c| c.horizontal / p.n_conductores as f64).fold(f64::NEG_INFINITY, f64::max);
            let lim = cfg.tiro_max_espiga * n_ais;
            add(e.codigo(), "I→", exceso(f_fase, lim), format!("fuerza por fase {f_fase:.0} N > {lim:.0} N"));
        }
        if cfg.angulo_salida_min != 0.0 || cfg.angulo_salida_max != 0.0 {
            let mut angs = Vec::new();
            for (k, extremo_a) in [(i as isize - 1, false), (i as isize, true)] {
                if k >= 0 && (k as usize) < r.vanos.len() {
                    let v = &r.vanos[k as usize];
                    for s in &r.tramos[v.tramo].estados {
                        let c = Curva::nueva(p.modelo, v.longitud, v.desnivel, s.h, s.w_vertical);
                        angs.push(pendiente_salida(&c, extremo_a));
                    }
                }
            }
            if !angs.is_empty() {
                let hi = angs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let lo = angs.iter().copied().fold(f64::INFINITY, f64::min);
                if cfg.angulo_salida_max != 0.0 {
                    add(
                        e.codigo(),
                        "I↑",
                        100.0 * (hi - cfg.angulo_salida_max) / cfg.angulo_salida_max.abs(),
                        format!("ángulo de salida {hi:.1}° > {}° (desamarre)", fmt_g(cfg.angulo_salida_max)),
                    );
                }
                if cfg.angulo_salida_min != 0.0 {
                    add(
                        e.codigo(),
                        "I↓",
                        100.0 * (cfg.angulo_salida_min - lo) / cfg.angulo_salida_min.abs(),
                        format!("ángulo de salida {lo:.1}° < {}°", fmt_g(cfg.angulo_salida_min)),
                    );
                }
            }
        }
    }

    // orden estable de mayor a menor exceso; crítica = primera de cada elemento
    filas.sort_by(|a, b| b.exceso.partial_cmp(&a.exceso).unwrap());
    let mut vistos = std::collections::HashSet::new();
    for f in &mut filas {
        f.critica = vistos.insert(f.elemento.clone());
    }
    filas
}
