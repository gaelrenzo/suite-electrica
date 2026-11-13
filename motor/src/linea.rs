//! LineCraft: proyecto, ubicación de estructuras, tramos, vanos y cargas.
//!
//! Port de `linecraft/{proyecto, estructuras, calculo}.py`.

use std::collections::HashMap;

use crate::conductor::{carga_unitaria, CargaUnitaria, Conductor, Hipotesis};
use crate::geo::{angulos_eje, interp, progresivas_eje, punto_en_progresiva, Perfil};
use crate::mecanica::{cambio_estado, deformacion_creep, vano_regulador, Curva, Modelo};

const TOL: f64 = 1e-6;
/// Tolerancia (m) para asociar una estructura a un vértice del eje.
pub const TOL_VERTICE: f64 = 0.01;
/// Claves de `fuentes` que deben citar norma o ficha técnica.
pub const PARAMETROS_NORMATIVOS: [&str; 5] =
    ["hipotesis", "tiro_inicial_pct_rotura", "tiro_maximo_pct_rotura", "distancia_minima_terreno_m", "conductor"];

/// Tipo de estructura.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tipo {
    Fin,
    Anclaje,
    Angulo,
    Suspension,
}

impl Tipo {
    pub fn como_str(&self) -> &'static str {
        match self {
            Tipo::Fin => "FIN",
            Tipo::Anclaje => "ANCLAJE",
            Tipo::Angulo => "ANGULO",
            Tipo::Suspension => "SUSPENSION",
        }
    }

    pub fn desde_str(s: &str) -> Result<Tipo, String> {
        match s.trim().to_uppercase().as_str() {
            "FIN" => Ok(Tipo::Fin),
            "ANCLAJE" => Ok(Tipo::Anclaje),
            "ANGULO" => Ok(Tipo::Angulo),
            "SUSPENSION" => Ok(Tipo::Suspension),
            otro => Err(format!("Tipo de estructura desconocido: {otro}")),
        }
    }
}

/// Estructura indicada a mano.
#[derive(Clone, Debug, Default)]
pub struct EstructuraManual {
    pub progresiva: f64,
    pub tipo: Option<Tipo>,
    pub altura_amarre: Option<f64>,
    pub armado: String,
    pub poste: String,
}

/// Datos de entrada de una línea (SI).
#[derive(Clone, Debug)]
pub struct Proyecto {
    pub nombre: String,
    pub conductor: Conductor,
    pub n_conductores: usize,
    pub modelo: Modelo,
    pub hipotesis: Vec<Hipotesis>,
    pub hip_inicial: String,
    /// Tiro horizontal EDS (% de rotura).
    pub tiro_inicial_pct: f64,
    pub hip_flecha_maxima: String,
    pub tiro_maximo_pct: f64,
    pub distancia_minima_terreno: f64,
    pub altura_amarre: f64,
    pub vano_maximo: f64,
    pub vano_minimo: f64,
    /// Ángulo de la retenida con la horizontal (°).
    pub angulo_retenida: f64,
    pub perfil: Perfil,
    pub eje: Vec<[f64; 2]>,
    pub estructuras_manual: Option<Vec<EstructuraManual>>,
    pub fuentes: HashMap<String, String>,
    pub alertas: HashMap<String, f64>,
    /// "creep" o "inicial".
    pub condicion_final: String,
    pub vanos_independientes: bool,
}

impl Proyecto {
    /// Hipótesis por nombre; con `z` (msnm), la de ese rango de altitud.
    pub fn hip(&self, nombre: &str, z: Option<f64>) -> Result<&Hipotesis, String> {
        let cand: Vec<&Hipotesis> = self.hipotesis.iter().filter(|h| h.nombre == nombre).collect();
        if cand.is_empty() {
            return Err(format!("Hipótesis no definida: {nombre}"));
        }
        match z {
            None => Ok(cand[0]),
            Some(z) => cand
                .into_iter()
                .find(|h| h.altitud_min <= z && z < h.altitud_max)
                .ok_or_else(|| format!("Hipótesis {nombre}: ningún rango de altitud contiene {z:.0} msnm")),
        }
    }

    /// Una hipótesis por nombre (en orden de aparición), la del rango de altitud `z`.
    pub fn hipotesis_en(&self, z: Option<f64>) -> Result<Vec<&Hipotesis>, String> {
        let mut nombres: Vec<&str> = Vec::new();
        for h in &self.hipotesis {
            if !nombres.contains(&h.nombre.as_str()) {
                nombres.push(&h.nombre);
            }
        }
        nombres.into_iter().map(|n| self.hip(n, z)).collect()
    }

    /// Parámetros normativos sin fuente verificada.
    pub fn avisos_fuentes(&self) -> Vec<String> {
        let mut av = Vec::new();
        for k in PARAMETROS_NORMATIVOS {
            let f = self.fuentes.get(k).map(|s| s.trim()).unwrap_or("");
            if f.is_empty() {
                av.push(format!("PENDIENTE_REVISION: '{k}' sin fuente normativa"));
            } else if f.to_uppercase().contains("PENDIENTE") || f.to_uppercase().contains("EJEMPLO") {
                av.push(format!("PENDIENTE_REVISION: '{k}' -> {f}"));
            }
        }
        av
    }
}

/// Estructura ubicada sobre el eje (m).
#[derive(Clone, Debug)]
pub struct Estructura {
    pub numero: usize,
    pub progresiva: f64,
    pub este: f64,
    pub norte: f64,
    pub cota_terreno: f64,
    /// Altura del amarre del conductor más bajo (m).
    pub altura_amarre: f64,
    pub tipo: Tipo,
    /// Deflexión del eje (°, + izquierda).
    pub angulo: f64,
    pub armado: String,
    pub poste: String,
}

impl Estructura {
    pub fn cota_amarre(&self) -> f64 {
        self.cota_terreno + self.altura_amarre
    }

    pub fn codigo(&self) -> String {
        format!("E-{:02}", self.numero)
    }
}

/// Crea estructuras en las progresivas dadas (tipo por defecto: FIN extremos, ANGULO vértices).
pub fn crear_estructuras(
    perfil: &Perfil, eje: &[[f64; 2]], progresivas: &[f64], alturas: &[f64], tipos: Option<&[Option<Tipo>]>,
    armados: Option<&[String]>, postes: Option<&[String]>,
) -> Result<Vec<Estructura>, String> {
    if progresivas.windows(2).any(|w| w[1] - w[0] <= 0.0) {
        return Err("Las progresivas de estructuras deben ser crecientes".into());
    }
    let acum = progresivas_eje(eje);
    let ang_v = angulos_eje(eje);
    for (sv, av) in acum.iter().zip(&ang_v).skip(1).take(acum.len().saturating_sub(2)) {
        let cerca = progresivas.iter().map(|s| (s - sv).abs()).fold(f64::INFINITY, f64::min);
        if av.abs() > 0.01 && cerca > TOL_VERTICE {
            return Err(format!("El vértice del eje en {sv:.2} m (deflexión {av:.2}°) no tiene estructura"));
        }
    }
    let l = *acum.last().unwrap();
    if progresivas[0].abs() > TOL_VERTICE || (progresivas[progresivas.len() - 1] - l).abs() > TOL_VERTICE {
        return Err(format!("Las estructuras deben ir de 0 a {l:.2} m"));
    }
    let n = progresivas.len();
    let mut res = Vec::with_capacity(n);
    for (i, &s) in progresivas.iter().enumerate() {
        let z = perfil.cota_en(s);
        if z.is_nan() {
            return Err("Hay estructuras fuera del modelo de terreno".into());
        }
        let xy = punto_en_progresiva(eje, s);
        let ang = acum.iter().position(|a| (a - s).abs() <= TOL_VERTICE).map(|j| ang_v[j]).unwrap_or(0.0);
        let tipo = match tipos.and_then(|t| t[i]) {
            Some(t) => t,
            None if i == 0 || i == n - 1 => Tipo::Fin,
            None if ang.abs() > 0.01 => Tipo::Angulo,
            None => Tipo::Suspension,
        };
        res.push(Estructura {
            numero: i + 1,
            progresiva: s,
            este: xy[0],
            norte: xy[1],
            cota_terreno: z,
            altura_amarre: alturas[i],
            tipo,
            angulo: ang,
            armado: armados.map(|a| a[i].clone()).unwrap_or_default(),
            poste: postes.map(|p| p[i].clone()).unwrap_or_default(),
        });
    }
    Ok(res)
}

/// Muestrea un vano: progresivas, cota del conductor y del terreno.
pub fn linea_conductor(
    perfil: &Perfil, s0: f64, s1: f64, za: f64, zb: f64, h: f64, w: f64, modelo: Modelo, factor_vertical: f64,
    paso: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let a = s1 - s0;
    let n = ((a / paso).ceil() as usize).max(2);
    let mut s: Vec<f64> = (0..=n).map(|k| s0 + (s1 - s0) * k as f64 / n as f64).collect();
    s.extend(perfil.progresiva.iter().copied().filter(|&p| p > s0 && p < s1));
    s.sort_by(|x, y| x.partial_cmp(y).unwrap());
    s.dedup();
    let c = Curva::nueva(modelo, a, zb - za, h, w);
    let zc = s
        .iter()
        .map(|&si| {
            let x = si - s0;
            let cuerda = (zb - za) * x / a;
            za + cuerda - (cuerda - c.y(x)) * factor_vertical
        })
        .collect();
    let zt = s.iter().map(|&si| perfil.cota_en(si)).collect();
    (s, zc, zt)
}

/// Despeje mínimo conductor–terreno (m) y su progresiva.
pub fn despeje_vano(
    perfil: &Perfil, s0: f64, s1: f64, za: f64, zb: f64, h: f64, w: f64, modelo: Modelo, factor_vertical: f64,
) -> Result<(f64, f64), String> {
    let (s, zc, zt) = linea_conductor(perfil, s0, s1, za, zb, h, w, modelo, factor_vertical, 1.0);
    if zt.iter().any(|z| z.is_nan()) {
        return Err(format!("Vano {s0:.1}-{s1:.1} m sale del modelo de terreno"));
    }
    let mut best = (f64::INFINITY, 0.0);
    for i in 0..s.len() {
        let d = zc[i] - zt[i];
        if d < best.0 {
            best = (d, s[i]);
        }
    }
    Ok(best)
}

/// Ubicación automática voraz (ver `estructuras.ubicar_automatico` en Python).
pub fn ubicar_automatico(
    perfil: &Perfil, eje: &[[f64; 2]], altura_amarre: f64, vano_maximo: f64, vano_minimo: f64,
    despeje_minimo: f64, h: f64, w: f64, modelo: Modelo, paso: f64, factor_vertical: f64,
) -> Result<(Vec<f64>, Vec<String>), String> {
    let forzadas = progresivas_eje(eje);
    let l = *forzadas.last().unwrap();
    let zt = |s: f64| perfil.cota_en(s);
    let (mut res, mut avisos) = (vec![0.0], Vec::new());
    let mut s0 = 0.0;
    while s0 < l - TOL {
        let sig = *forzadas.iter().find(|&&f| f > s0 + TOL).unwrap();
        let lim = (s0 + vano_maximo).min(sig);
        let piso = (s0 + vano_minimo).min(lim);
        let za = zt(s0) + altura_amarre;
        let (mut mejor, mut mejor_d, mut elegido) = (lim, f64::NEG_INFINITY, None);
        let mut k = 0usize;
        loop {
            let s = lim - k as f64 * paso; // np.arange(lim, piso − TOL, −paso)
            if s <= piso - TOL {
                break;
            }
            let (d, _) = despeje_vano(perfil, s0, s, za, zt(s) + altura_amarre, h, w, modelo, factor_vertical)?;
            if d >= despeje_minimo {
                elegido = Some(s);
                break;
            }
            if d > mejor_d {
                mejor = s;
                mejor_d = d;
            }
            k += 1;
        }
        let e = match elegido {
            Some(e) => e,
            None => {
                avisos.push(format!(
                    "Vano desde {s0:.2} m: ningún vano cumple despeje {despeje_minimo:.2} m (mejor {mejor_d:.2} m en {mejor:.2} m)"
                ));
                mejor
            }
        };
        res.push(e);
        s0 = e;
    }
    Ok((res, avisos))
}

/// Estado del conductor en un tramo para una hipótesis.
#[derive(Clone, Debug)]
pub struct Estado {
    pub hipotesis: String,
    pub temperatura: f64,
    pub w_vertical: f64,
    pub w_viento: f64,
    pub w_resultante: f64,
    /// Tiro horizontal (N).
    pub h: f64,
    pub tiro_maximo: f64,
    pub pct_rotura: f64,
    pub cumple: bool,
}

/// Tramo de tensado entre anclajes/fin (índices inclusive).
#[derive(Clone, Debug)]
pub struct Tramo {
    pub inicio: usize,
    pub fin: usize,
    pub vano_regulador: f64,
    pub estados: Vec<Estado>,
}

impl Tramo {
    pub fn estado(&self, nombre: &str) -> Option<&Estado> {
        self.estados.iter().find(|e| e.hipotesis == nombre)
    }
}

/// Vano entre las estructuras `a` y `a + 1`.
#[derive(Clone, Debug)]
pub struct Vano {
    pub a: usize,
    pub tramo: usize,
    pub longitud: f64,
    pub desnivel: f64,
    /// Flecha vertical por hipótesis (m).
    pub flechas: Vec<(String, f64)>,
    pub despeje: f64,
    pub s_despeje: f64,
    pub cumple: bool,
    /// Flecha a medio vano en la hipótesis de flecha máxima (m).
    pub flecha_medio: f64,
}

/// Cargas en una estructura para una hipótesis (todos los conductores, N).
#[derive(Clone, Debug, Default)]
pub struct Cargas {
    pub estructura: usize,
    pub hipotesis: String,
    pub vano_viento: f64,
    pub vano_peso: f64,
    pub transversal: f64,
    pub longitudinal: f64,
    pub vertical: f64,
    pub horizontal: f64,
    pub momento_base: f64,
    pub tiro_retenida: f64,
    pub viento_conductor: f64,
    pub presion_viento: f64,
}

/// Resultado del cálculo de una línea.
#[derive(Clone, Debug)]
pub struct Resultado {
    pub estructuras: Vec<Estructura>,
    pub tramos: Vec<Tramo>,
    pub vanos: Vec<Vano>,
    pub cargas: Vec<Cargas>,
    pub avisos: Vec<String>,
}

/// H (N) y carga unitaria por hipótesis (en el orden del proyecto).
pub type Estados = Vec<(String, f64, CargaUnitaria)>;

/// H y cargas por hipótesis para un tramo (ver `_estados_tramo` en Python).
pub fn estados_tramo(
    p: &Proyecto, a_r: f64, desnivel: f64, z: Option<f64>, propio: Option<(f64, f64)>,
) -> Result<Estados, String> {
    let c = &p.conductor;
    let hips = p.hipotesis_en(z)?;
    let h1 = *hips.iter().find(|h| h.nombre == p.hip_inicial).ok_or("Falta la hipótesis inicial")?;
    let w1 = carga_unitaria(c, h1)?.resultante();
    let sigma1 = p.tiro_inicial_pct / 100.0 * c.carga_rotura / c.seccion;
    let dt = if p.condicion_final == "creep" {
        deformacion_creep(c, sigma1, w1, h1.temperatura, a_r, desnivel)? / c.coef_dilatacion
    } else {
        0.0
    };
    let (a2, h2) = propio.unwrap_or((a_r, desnivel));
    let mut res = Vec::with_capacity(hips.len());
    for h in hips {
        let cu = carga_unitaria(c, h)?;
        let s2 = if h.nombre == p.hip_inicial {
            sigma1
        } else {
            cambio_estado(sigma1, w1, h1.temperatura, cu.resultante(), h.temperatura + dt, a2, c, h2)?
        };
        res.push((h.nombre.clone(), s2 * c.seccion, cu));
    }
    Ok(res)
}

fn buscar<'a>(e: &'a Estados, nombre: &str) -> &'a (String, f64, CargaUnitaria) {
    e.iter().find(|x| x.0 == nombre).expect("hipótesis presente")
}

fn necesita_retenida(e: &Estructura) -> bool {
    matches!(e.tipo, Tipo::Fin | Tipo::Anclaje) || e.angulo.abs() > 0.01
}

/// Ejecuta el cálculo completo de la línea.
pub fn calcular(p: &Proyecto) -> Result<Resultado, String> {
    let mut avisos = p.avisos_fuentes();
    let c = &p.conductor;

    // 1. Estructuras
    let est = if let Some(m) = &p.estructuras_manual {
        let prog: Vec<f64> = m.iter().map(|e| e.progresiva).collect();
        let alt: Vec<f64> = m.iter().map(|e| e.altura_amarre.unwrap_or(p.altura_amarre)).collect();
        let tipos: Vec<Option<Tipo>> = m.iter().map(|e| e.tipo).collect();
        let arm: Vec<String> = m.iter().map(|e| e.armado.trim().to_string()).collect();
        let pos: Vec<String> = m.iter().map(|e| e.poste.trim().to_string()).collect();
        crear_estructuras(&p.perfil, &p.eje, &prog, &alt, Some(&tipos), Some(&arm), Some(&pos))?
    } else {
        let mut a_r = 0.8 * p.vano_maximo;
        let hf = p.hip(&p.hip_flecha_maxima, None)?;
        let cu_f = carga_unitaria(c, hf)?;
        let mut prog = Vec::new();
        let mut av = Vec::new();
        for _ in 0..8 {
            let h = buscar(&estados_tramo(p, a_r, 0.0, None, None)?, &hf.nombre).1;
            let r = ubicar_automatico(
                &p.perfil, &p.eje, p.altura_amarre, p.vano_maximo, p.vano_minimo, p.distancia_minima_terreno, h,
                cu_f.resultante(), p.modelo, 1.0, cu_f.vertical / cu_f.resultante(),
            )?;
            prog = r.0;
            av = r.1;
            let vanos: Vec<f64> = prog.windows(2).map(|w| w[1] - w[0]).collect();
            let nuevo = vano_regulador(&vanos)?;
            if (nuevo - a_r).abs() < 0.5 {
                break;
            }
            a_r = nuevo;
        }
        avisos.extend(av);
        let alt = vec![p.altura_amarre; prog.len()];
        crear_estructuras(&p.perfil, &p.eje, &prog, &alt, None, None, None)?
    };

    // 2. Tramos de tensado y 3. vanos
    let n_est = est.len();
    let cortes: Vec<usize> = (0..n_est)
        .filter(|&i| matches!(est[i].tipo, Tipo::Fin | Tipo::Anclaje) || i == 0 || i == n_est - 1)
        .collect();
    let mut tramos: Vec<Tramo> = Vec::new();
    let mut vanos: Vec<Vano> = Vec::new();
    let mut estados_vano: Vec<Estados> = Vec::new();
    for k in 0..cortes.len() - 1 {
        let (i0, i1) = (cortes[k], cortes[k + 1]);
        let longitudes: Vec<f64> = (i0..i1).map(|i| est[i + 1].progresiva - est[i].progresiva).collect();
        let a_r = vano_regulador(&longitudes)?;
        let h_unico = if i1 - i0 == 1 { est[i1].cota_amarre() - est[i0].cota_amarre() } else { 0.0 };
        let z = est[i0..=i1].iter().map(|e| e.cota_terreno).sum::<f64>() / (i1 - i0 + 1) as f64;
        let hs_v: Vec<Estados> = if p.vanos_independientes && i1 - i0 > 1 {
            (i0..i1)
                .map(|i| {
                    let propio = (est[i + 1].progresiva - est[i].progresiva, est[i + 1].cota_amarre() - est[i].cota_amarre());
                    estados_tramo(p, a_r, 0.0, Some(z), Some(propio))
                })
                .collect::<Result<_, _>>()?
        } else {
            vec![estados_tramo(p, a_r, h_unico, Some(z), None)?; i1 - i0]
        };
        let temps: HashMap<String, f64> =
            p.hipotesis_en(Some(z))?.iter().map(|h| (h.nombre.clone(), h.temperatura)).collect();
        let mut t = Tramo { inicio: i0, fin: i1, vano_regulador: a_r, estados: Vec::new() };
        for (nombre, _, cu) in &hs_v[0] {
            let mut mejor = (f64::NEG_INFINITY, 0.0);
            for (i, hv) in (i0..i1).zip(&hs_v) {
                let (_, hh, cuv) = buscar(hv, nombre);
                let a = est[i + 1].progresiva - est[i].progresiva;
                let dz = est[i + 1].cota_amarre() - est[i].cota_amarre();
                let tm = Curva::nueva(p.modelo, a, dz, *hh, cuv.resultante()).tiro_maximo();
                if tm > mejor.0 || (tm == mejor.0 && *hh > mejor.1) {
                    mejor = (tm, *hh);
                }
            }
            let (tmax, h) = mejor;
            let pct = 100.0 * tmax / c.carga_rotura;
            let ok = pct <= p.tiro_maximo_pct + 1e-9;
            t.estados.push(Estado {
                hipotesis: nombre.clone(),
                temperatura: temps[nombre],
                w_vertical: cu.vertical,
                w_viento: cu.viento,
                w_resultante: cu.resultante(),
                h,
                tiro_maximo: tmax,
                pct_rotura: pct,
                cumple: ok,
            });
            if !ok {
                avisos.push(format!(
                    "Tramo {}-{}, {nombre}: tiro {pct:.1} % > {} % de rotura",
                    est[i0].codigo(), est[i1].codigo(), fmt_g(p.tiro_maximo_pct)
                ));
            }
        }
        tramos.push(t);

        for (i, hs) in (i0..i1).zip(hs_v) {
            let (ea, eb) = (&est[i], &est[i + 1]);
            let a = eb.progresiva - ea.progresiva;
            let h = eb.cota_amarre() - ea.cota_amarre();
            let flechas = hs
                .iter()
                .map(|(n, hh, cu)| {
                    (n.clone(), Curva::nueva(p.modelo, a, h, *hh, cu.resultante()).flecha() * cu.vertical / cu.resultante())
                })
                .collect();
            let (_, hf, cuf) = buscar(&hs, &p.hip_flecha_maxima);
            let fv = cuf.vertical / cuf.resultante();
            let (d, sd) = despeje_vano(
                &p.perfil, ea.progresiva, eb.progresiva, ea.cota_amarre(), eb.cota_amarre(), *hf, cuf.resultante(),
                p.modelo, fv,
            )?;
            let ok = d >= p.distancia_minima_terreno - 1e-9;
            if !ok {
                avisos.push(format!(
                    "Vano {}-{}: despeje {d:.2} m < {:.2} m en {sd:.1} m",
                    ea.codigo(), eb.codigo(), p.distancia_minima_terreno
                ));
            }
            let cf = Curva::nueva(p.modelo, a, h, *hf, cuf.resultante());
            let f_medio = cf.flecha_medio() * fv;
            vanos.push(Vano {
                a: i,
                tramo: tramos.len() - 1,
                longitud: a,
                desnivel: h,
                flechas,
                despeje: d,
                s_despeje: sd,
                cumple: ok,
                flecha_medio: f_medio,
            });
            estados_vano.push(hs);
        }
    }

    // 4. Cargas en estructuras
    let mut cargas = Vec::new();
    let n = p.n_conductores as f64;
    let phi = p.angulo_retenida.to_radians();
    for (i, e) in est.iter().enumerate() {
        let mut arranque = Vec::new();
        let atras = if i > 0 { Some(&vanos[i - 1]) } else { None };
        let adelante = vanos.get(i);
        let mitad = e.angulo.abs().to_radians() / 2.0;
        let ref_v = adelante.or(atras).expect("la estructura tiene al menos un vano");
        for (nombre, _, cu) in &estados_vano[ref_v.a] {
            let (mut av, mut ap, mut hb, mut ha) = (0.0, 0.0, 0.0, 0.0);
            for (v, atras_lado) in [(atras, true), (adelante, false)] {
                let Some(v) = v else { continue };
                let (_, hh, cuv) = buscar(&estados_vano[v.a], nombre);
                let xm = Curva::nueva(p.modelo, v.longitud, v.desnivel, *hh, cuv.vertical).x_vertice();
                av += v.longitud / 2.0;
                if atras_lado {
                    hb = *hh;
                    ap += v.longitud - xm;
                } else {
                    ha = *hh;
                    ap += xm;
                }
            }
            let viento = n * cu.viento * av * mitad.cos();
            let trans = n * (hb + ha) * mitad.sin() + viento;
            let longi = n * (ha - hb).abs() * mitad.cos();
            let vert = n * cu.vertical * ap;
            let fh = trans.hypot(longi);
            let ret = if necesita_retenida(e) { fh / phi.cos() } else { 0.0 };
            cargas.push(Cargas {
                estructura: e.numero,
                hipotesis: nombre.clone(),
                vano_viento: av,
                vano_peso: ap,
                transversal: trans,
                longitudinal: longi,
                vertical: vert,
                horizontal: fh,
                momento_base: fh * e.altura_amarre,
                tiro_retenida: ret,
                viento_conductor: viento,
                presion_viento: cu.presion,
            });
            if ap < 0.0 {
                arranque.push(format!("{nombre} ({ap:.1} m)"));
            }
        }
        if !arranque.is_empty() {
            avisos.push(format!("{}: vano peso negativo (arrancamiento) en {}", e.codigo(), arranque.join(", ")));
        }
    }
    let _ = interp; // usado vía Perfil::cota_en
    Ok(Resultado { estructuras: est, tramos, vanos, cargas, avisos })
}

/// Formato `{:g}` de Python para números "redondos".
pub fn fmt_g(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}
