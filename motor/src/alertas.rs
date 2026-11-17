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
    /// Parámetro de catenaria H/w máximo (m) en la condición de EDS, criterio de CIGRE TB 273 según
    /// la categoría del terreno (0 = no se verifica; el valor lo pone el usuario con su fuente).
    pub hw_max_m: f64,
}

impl ConfigAlertas {
    /// Desde el mapa `"alertas"` del proyecto (claves de Python; las que faltan valen 0).
    pub fn desde_mapa(m: &std::collections::HashMap<String, f64>) -> Self {
        let g = |k: &str| m.get(k).copied().unwrap_or(0.0);
        ConfigAlertas {
            fs_poste: g("fs_poste"),
            fs_retenida: g("fs_retenida"),
            tension_kv: g("tension_kV"),
            sep_k_tension: g("sep_k_tension"),
            sep_k_flecha: g("sep_k_flecha"),
            angulo_salida_min: g("angulo_salida_min"),
            angulo_salida_max: g("angulo_salida_max"),
            tiro_max_espiga: g("tiro_max_espiga"),
            eds_max_pct: g("eds_max_pct"),
            hw_max_m: g("hw_max_m"),
        }
    }
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
    crate::numerico::redondear(x, 1)
}

fn r2(x: f64) -> f64 {
    crate::numerico::redondear(x, 2)
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
    for (k, t) in r.tramos.iter().enumerate() {
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
        if cfg.hw_max_m != 0.0 {
            if let Some(s) = t.estado(&p.hip_inicial) {
                let c = &p.conductor;
                let hw = s.h / c.peso;
                // L·D/m del vano más largo del tramo (m³/kg): orienta la necesidad de amortiguadores
                let l = r.vanos.iter().filter(|v| v.tramo == k).map(|v| v.longitud).fold(0.0, f64::max);
                let ldm = l * c.diametro / (c.peso / crate::conductor::G);
                add(
                    format!("Tramo {}-{}", est[t.inicio].codigo(), est[t.fin].codigo()),
                    "V",
                    exceso(hw, cfg.hw_max_m),
                    format!("H/w {hw:.0} m > {} m (vibraciones eólicas, CIGRE TB 273); L·D/m = {ldm:.2} m³/kg en el vano mayor ({l:.0} m)", fmt_g(cfg.hw_max_m)),
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
        // arrancamiento: el conductor tira hacia arriba (vano peso negativo). Con aislador rígido (se
        // desamarra) o en suspensión (la cadena se voltea); las cadenas de anclaje trabajan
        // horizontales. Con vano peso máximo del armado, el exceso es el vano negativo como % de ese
        // máximo (como REDLIN), si no, sin magnitud permisible (∞)
        let expuesto = arm.aislador.eq_ignore_ascii_case("RIGIDO") || e.tipo == crate::linea::Tipo::Suspension;
        if let Some(c) = ce.iter().copied().filter(|c| expuesto && c.vano_peso < 0.0).fold(None::<&crate::linea::Cargas>, |m, c| {
            if m.map_or(true, |m| c.vano_peso < m.vano_peso) { Some(c) } else { m }
        }) {
            let ex = arm.vano_peso_max_m.filter(|v| *v > 0.0).map_or(f64::INFINITY, |vmax| (100.0 * -c.vano_peso / vmax).max(1e-6));
            add(e.codigo(), "S↑", ex, format!("vano peso {:.1} m en {} (arrancamiento)", c.vano_peso, c.hipotesis));
        }
        // vano peso máximo del armado (prestación)
        if let Some(vmax) = arm.vano_peso_max_m.filter(|v| *v > 0.0) {
            if let Some(c) = ce.iter().copied().max_by(|a, b| a.vano_peso.partial_cmp(&b.vano_peso).unwrap()) {
                add(e.codigo(), "P↓", exceso(c.vano_peso, vmax), format!("vano peso {:.1} m > {} m del armado ({})", c.vano_peso, fmt_g(vmax), c.hipotesis));
            }
        }
        // cruceta: fuerza por fase (vertical y longitudinal) en el brazo contra la carga máxima
        if let Some(lim) = arm.carga_max_cruceta_n.filter(|v| *v > 0.0) {
            let n = p.n_conductores as f64;
            if let Some((f, c)) = ce
                .iter()
                .map(|c| (c.vertical.max(0.0).hypot(c.longitudinal) / n, c))
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            {
                add(e.codigo(), "Cr", exceso(f, lim), format!("cruceta: {f:.0} N por fase > {lim:.0} N ({})", c.hipotesis));
            }
        }
        if !arm.aislador.eq_ignore_ascii_case("RIGIDO") {
            continue;
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
