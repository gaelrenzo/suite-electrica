//! Catálogos de postes, armados, retenidas y materiales; verificación de estructuras y metrado.
//!
//! Port de `linecraft/catalogos.py`. Columnas numéricas vacías = `None`.

use std::collections::BTreeMap;

use crate::conductor::G;
use crate::linea::{Cargas, Estructura, Proyecto, Resultado, Tipo};
use crate::mecanica::{Curva, Modelo};

#[derive(Clone, Debug, Default)]
pub struct Poste {
    pub codigo: String,
    pub material: String,
    pub longitud_m: Option<f64>,
    pub carga_nominal_dan: Option<f64>,
    pub empotramiento_m: Option<f64>,
    pub carga_vertical_max_dan: Option<f64>,
    pub diam_punta_m: Option<f64>,
    pub diam_tierra_m: Option<f64>,
    pub mor_mpa: Option<f64>,
    pub dist_aplicacion_m: Option<f64>,
    pub fuente: String,
}

#[derive(Clone, Debug, Default)]
pub struct Armado {
    pub codigo: String,
    pub descripcion: String,
    pub funcion: String,
    pub angulo_max_deg: Option<f64>,
    pub distancia_punta_amarre_m: Option<f64>,
    pub separacion_fases_m: Option<f64>,
    pub aislador: String,
    pub aisladores_por_fase: Option<f64>,
    pub alturas_fases_m: String,
    pub retenida_toma: String,
    pub fuente: String,
}

#[derive(Clone, Debug, Default)]
pub struct Retenida {
    pub codigo: String,
    pub descripcion: String,
    pub carga_rotura_kn: Option<f64>,
    pub fuente: String,
}

#[derive(Clone, Debug, Default)]
pub struct Material {
    pub armado: String,
    pub material: String,
    pub unidad: String,
    pub cantidad: Option<f64>,
    pub fuente: String,
}

/// Lee un CSV (líneas `#` = comentarios) como filas de `encabezado -> valor`.
pub fn leer_csv(texto: &str) -> Result<Vec<BTreeMap<String, String>>, String> {
    let limpio: String = texto.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    let mut rd = csv::ReaderBuilder::new().trim(csv::Trim::All).flexible(true).from_reader(limpio.as_bytes());
    let enc: Vec<String> = rd.headers().map_err(|e| e.to_string())?.iter().map(|s| s.to_string()).collect();
    let mut filas = Vec::new();
    for r in rd.records() {
        let r = r.map_err(|e| e.to_string())?;
        filas.push(enc.iter().cloned().zip(r.iter().map(|s| s.to_string())).collect());
    }
    Ok(filas)
}

fn txt(f: &BTreeMap<String, String>, k: &str) -> String {
    f.get(k).cloned().unwrap_or_default()
}

fn numero(f: &BTreeMap<String, String>, k: &str) -> Option<f64> {
    f.get(k).and_then(|s| s.replace(',', ".").parse::<f64>().ok()).filter(|v| v.is_finite())
}

/// Tablas de catálogo.
#[derive(Clone, Debug, Default)]
pub struct Catalogos {
    pub postes: Vec<Poste>,
    pub armados: Vec<Armado>,
    pub retenidas: Vec<Retenida>,
    pub materiales: Vec<Material>,
}

impl Catalogos {
    /// Desde el texto de los cuatro CSV (postes, armados, retenidas, materiales).
    pub fn desde_csv(postes: &str, armados: &str, retenidas: &str, materiales: &str) -> Result<Self, String> {
        Ok(Catalogos {
            postes: leer_csv(postes)?
                .iter()
                .map(|f| Poste {
                    codigo: txt(f, "codigo"),
                    material: txt(f, "material"),
                    longitud_m: numero(f, "longitud_m"),
                    carga_nominal_dan: numero(f, "carga_nominal_daN"),
                    empotramiento_m: numero(f, "empotramiento_m"),
                    carga_vertical_max_dan: numero(f, "carga_vertical_max_daN"),
                    diam_punta_m: numero(f, "diam_punta_m"),
                    diam_tierra_m: numero(f, "diam_tierra_m"),
                    mor_mpa: numero(f, "mor_MPa"),
                    dist_aplicacion_m: numero(f, "dist_aplicacion_m"),
                    fuente: txt(f, "fuente"),
                })
                .collect(),
            armados: leer_csv(armados)?
                .iter()
                .map(|f| Armado {
                    codigo: txt(f, "codigo"),
                    descripcion: txt(f, "descripcion"),
                    funcion: txt(f, "funcion"),
                    angulo_max_deg: numero(f, "angulo_max_deg"),
                    distancia_punta_amarre_m: numero(f, "distancia_punta_amarre_m"),
                    separacion_fases_m: numero(f, "separacion_fases_m"),
                    aislador: txt(f, "aislador"),
                    aisladores_por_fase: numero(f, "aisladores_por_fase"),
                    alturas_fases_m: txt(f, "alturas_fases_m"),
                    retenida_toma: txt(f, "retenida_toma"),
                    fuente: txt(f, "fuente"),
                })
                .collect(),
            retenidas: leer_csv(retenidas)?
                .iter()
                .map(|f| Retenida {
                    codigo: txt(f, "codigo"),
                    descripcion: txt(f, "descripcion"),
                    carga_rotura_kn: numero(f, "carga_rotura_kN"),
                    fuente: txt(f, "fuente"),
                })
                .collect(),
            materiales: leer_csv(materiales)?
                .iter()
                .map(|f| Material {
                    armado: txt(f, "armado"),
                    material: txt(f, "material"),
                    unidad: txt(f, "unidad"),
                    cantidad: numero(f, "cantidad"),
                    fuente: txt(f, "fuente"),
                })
                .collect(),
        })
    }

    /// Catálogos por defecto (plantillas del repositorio, incluidas en el binario).
    pub fn plantilla() -> Self {
        Self::desde_csv(
            include_str!("../../suite_electrica/plantillas/postes.csv"),
            include_str!("../../suite_electrica/plantillas/armados.csv"),
            include_str!("../../suite_electrica/plantillas/retenidas.csv"),
            include_str!("../../suite_electrica/plantillas/materiales.csv"),
        )
        .expect("plantillas válidas")
    }

    /// Filas sin fuente o marcadas PENDIENTE.
    pub fn avisos(&self) -> Vec<String> {
        let mut av = Vec::new();
        let tablas: [(&str, Vec<(&str, &str)>); 3] = [
            ("postes", self.postes.iter().map(|p| (p.codigo.as_str(), p.fuente.as_str())).collect()),
            ("armados", self.armados.iter().map(|p| (p.codigo.as_str(), p.fuente.as_str())).collect()),
            ("retenidas", self.retenidas.iter().map(|p| (p.codigo.as_str(), p.fuente.as_str())).collect()),
        ];
        for (nombre, filas) in tablas {
            let pend: Vec<&str> = filas
                .iter()
                .filter(|(_, f)| f.is_empty() || f.to_uppercase().contains("PENDIENTE"))
                .map(|(c, _)| *c)
                .collect();
            if !pend.is_empty() {
                av.push(format!("PENDIENTE_REVISION: catálogo de {nombre} sin fuente verificada ({})", pend.join(", ")));
            }
        }
        av
    }

    pub fn poste(&self, codigo: &str) -> Option<&Poste> {
        self.postes.iter().find(|p| p.codigo == codigo)
    }

    pub fn armado(&self, codigo: &str) -> Option<&Armado> {
        self.armados.iter().find(|a| a.codigo == codigo)
    }

    /// `longitud − empotramiento − distancia punta–amarre` si todo está definido (m).
    pub fn altura_amarre(&self, poste: &str, armado: &str) -> Option<f64> {
        let (p, a) = (self.poste(poste)?, self.armado(armado)?);
        Some(p.longitud_m? - p.empotramiento_m? - a.distancia_punta_amarre_m?)
    }

    /// Primer armado cuya función incluye el tipo y admite la deflexión.
    pub fn elegir_armado(&self, tipo: Tipo, angulo: f64) -> String {
        for a in &self.armados {
            let incluye = a.funcion.split('|').any(|f| f.trim().eq_ignore_ascii_case(tipo.como_str()));
            if incluye && a.angulo_max_deg.map_or(true, |m| angulo.abs() <= m) {
                return a.codigo.clone();
            }
        }
        String::new()
    }
}

/// Estructuras de fin, anclaje o con deflexión llevan retenida.
pub fn lleva_retenida(e: &Estructura) -> bool {
    matches!(e.tipo, Tipo::Fin | Tipo::Anclaje) || e.angulo.abs() > 0.01
}

/// Por estructura: (F horizontal máx., momento base máx., tiro de retenida máx.).
pub fn fuerzas_maximas(r: &Resultado) -> BTreeMap<usize, (f64, f64, f64)> {
    let mut out: BTreeMap<usize, (f64, f64, f64)> = BTreeMap::new();
    for c in &r.cargas {
        let e = out.entry(c.estructura).or_insert((0.0, 0.0, 0.0));
        *e = (e.0.max(c.horizontal), e.1.max(c.momento_base), e.2.max(c.tiro_retenida));
    }
    out
}

/// Altura de cada fase sobre el terreno (m) a partir de `alturas_fases_m` del armado.
pub fn alturas_fases(e: &Estructura, poste: Option<&Poste>, armado: Option<&Armado>, n: usize) -> Vec<f64> {
    let texto = armado.map(|a| a.alturas_fases_m.trim()).unwrap_or("");
    let (Some(p), false) = (poste, texto.is_empty()) else { return vec![e.altura_amarre; n] };
    let (Some(lon), Some(emp)) = (p.longitud_m, p.empotramiento_m) else { return vec![e.altura_amarre; n] };
    let libre = lon - emp;
    let bajo: Vec<f64> =
        texto.split(';').filter(|v| !v.trim().is_empty()).filter_map(|v| v.trim().replace(',', ".").parse().ok()).collect();
    if bajo.is_empty() {
        return vec![e.altura_amarre; n];
    }
    (0..n).map(|i| libre - bajo[i.min(bajo.len() - 1)]).collect()
}

/// Qué carga toma la retenida: `None` sin retenida; `todo`, `angulo` o `longitudinal`.
pub fn retenida_toma(e: &Estructura, armado: Option<&Armado>) -> Option<String> {
    if !lleva_retenida(e) {
        return None;
    }
    let v = armado.map(|a| a.retenida_toma.trim().to_lowercase()).unwrap_or_default();
    if !v.is_empty() {
        return Some(v);
    }
    Some(match e.tipo {
        Tipo::Anclaje => "todo",
        Tipo::Fin => "longitudinal",
        _ => "angulo",
    }
    .into())
}

/// Momentos en la línea de tierra (N·m), criterio de REDLIN.
#[derive(Clone, Copy, Debug, Default)]
pub struct Momentos {
    pub mvc: f64,
    pub mtb: f64,
    pub mtd: f64,
    pub mve: f64,
    pub mrn: f64,
}

/// MVC, MTB, MTD, MVE y MRN de una hipótesis (ver `momentos_poste` en Python).
pub fn momentos_poste(c: &Cargas, alturas: &[f64], poste: &Poste, retenida: Option<&str>) -> Momentos {
    let n = alturas.len() as f64;
    let sh: f64 = alturas.iter().sum();
    let mvc = c.viento_conductor / n * sh;
    let mtb = (c.transversal - c.viento_conductor) / n * sh;
    let mtd = c.longitudinal / n * sh;
    let libre = poste.longitud_m.unwrap_or(0.0) - poste.empotramiento_m.unwrap_or(0.0);
    let mve = c.presion_viento * libre * libre
        * (2.0 * poste.diam_punta_m.unwrap_or(0.0) + poste.diam_tierra_m.unwrap_or(0.0)) / 6.0;
    let mrn = match retenida {
        Some("todo") => mve,
        Some("angulo") => mvc.hypot(mtd) + mve,
        Some("longitudinal") => mvc + mtb + mve,
        _ => (mvc + mtb).hypot(mtd) + mve,
    };
    Momentos { mvc, mtb, mtd, mve, mrn }
}

/// (FS, detalle): madera `MOR / (MRN/S)`; troncocónico `carga nominal / (MRN/brazo)`.
pub fn factor_seguridad_poste(p: &Poste, mrn: f64) -> (f64, String) {
    if mrn <= 0.0 {
        return (f64::INFINITY, String::new());
    }
    if let Some(mor) = p.mor_mpa {
        let d = p.diam_tierra_m.unwrap_or(0.0);
        let rh = mrn / (std::f64::consts::PI * d.powi(3) / 32.0) / 1e6;
        return (mor / rh, format!("esfuerzo en línea de tierra {rh:.2} MPa"));
    }
    let brazo = p.longitud_m.unwrap_or(0.0) - p.empotramiento_m.unwrap_or(0.0) - p.dist_aplicacion_m.unwrap_or(0.0);
    let feq = mrn / brazo;
    (p.carga_nominal_dan.unwrap_or(0.0) * 10.0 / feq, format!("Feq {feq:.0} N"))
}

fn geometria(p: Option<&Poste>) -> bool {
    p.is_some_and(|p| {
        p.empotramiento_m.is_some()
            && p.diam_punta_m.is_some()
            && p.diam_tierra_m.is_some()
            && (p.mor_mpa.is_some() || p.carga_nominal_dan.is_some())
    })
}

/// Fila de la verificación de estructuras.
#[derive(Clone, Debug, Default)]
pub struct FilaVerificacion {
    pub estructura: String,
    pub tipo: String,
    pub armado: String,
    pub poste: String,
    pub f_horizontal: f64,
    pub f_punta: Option<f64>,
    pub mrn: Option<f64>,
    /// `Some(inf)` si no hay carga.
    pub fs_poste: Option<f64>,
    pub retenidas: usize,
    pub tiro_por_retenida: Option<f64>,
    pub fs_retenida: Option<f64>,
    pub cumple: bool,
    pub observacion: String,
}

fn g(x: f64) -> String {
    crate::linea::fmt_g(x)
}

/// Verifica postes y retenidas en la hipótesis más desfavorable.
pub fn verificar(
    r: &Resultado, p: &Proyecto, cat: &Catalogos, fs_poste: f64, fs_retenida: f64, retenida: Option<&str>,
    n_retenidas: &BTreeMap<usize, usize>,
) -> Vec<FilaVerificacion> {
    let fm = fuerzas_maximas(r);
    let ret = match retenida {
        Some(c) => cat.retenidas.iter().find(|x| x.codigo == c),
        None => cat.retenidas.first(),
    };
    let mut filas = Vec::new();
    for e in &r.estructuras {
        let (fh, mb, tr) = fm.get(&e.numero).copied().unwrap_or((0.0, 0.0, 0.0));
        let mut f = FilaVerificacion {
            estructura: e.codigo(),
            tipo: e.tipo.como_str().into(),
            armado: if e.armado.is_empty() { "—".into() } else { e.armado.clone() },
            poste: if e.poste.is_empty() { "—".into() } else { e.poste.clone() },
            f_horizontal: redondear(fh, 1),
            cumple: true,
            ..Default::default()
        };
        let mut obs: Vec<String> = Vec::new();
        let pg = if e.poste.is_empty() { None } else { cat.poste(&e.poste) };
        if geometria(pg) {
            let pg = pg.unwrap();
            let arm = if e.armado.is_empty() { None } else { cat.armado(&e.armado) };
            let alt = alturas_fases(e, Some(pg), arm, p.n_conductores);
            let toma = retenida_toma(e, arm);
            let mut peor: Option<(f64, String, Momentos)> = None;
            for c in r.cargas.iter().filter(|c| c.estructura == e.numero) {
                let m = momentos_poste(c, &alt, pg, toma.as_deref());
                let (fs, det) = factor_seguridad_poste(pg, m.mrn);
                if peor.as_ref().map_or(true, |x| fs < x.0) {
                    peor = Some((fs, det, m));
                }
            }
            if let Some((fs, det, m)) = peor {
                f.mrn = Some(redondear(m.mrn, 1));
                f.fs_poste = Some(fs);
                if fs < fs_poste {
                    f.cumple = false;
                    obs.push(format!("FS poste {fs:.2} < {} ({det})", g(fs_poste)));
                }
            }
        }
        if lleva_retenida(e) {
            let n = n_retenidas.get(&e.numero).copied().filter(|&n| n > 0).unwrap_or(1);
            f.retenidas = n;
            f.tiro_por_retenida = Some(tr / n as f64);
            match ret.and_then(|x| x.carga_rotura_kn) {
                Some(cr) => {
                    let fs = if tr > 0.0 { cr * 1e3 / (tr / n as f64) } else { f64::INFINITY };
                    f.fs_retenida = Some(fs);
                    if fs < fs_retenida {
                        f.cumple = false;
                        obs.push(format!("FS retenida {fs:.2} < {}", g(fs_retenida)));
                    }
                }
                None => obs.push("Sin retenida en catálogo".into()),
            }
        } else if geometria(pg) {
            // ya verificado con momentos
        } else if let Some(pp) = pg.filter(|pp| pp.carga_nominal_dan.is_some()) {
            let f_punta = match pp.empotramiento_m {
                Some(emp) => mb / (pp.longitud_m.unwrap_or(0.0) - emp),
                None => fh,
            };
            let fs = if f_punta > 0.0 { pp.carga_nominal_dan.unwrap() * 10.0 / f_punta } else { f64::INFINITY };
            f.f_punta = Some(f_punta);
            f.fs_poste = Some(fs);
            if fs < fs_poste {
                f.cumple = false;
                obs.push(format!("FS poste {fs:.2} < {}", g(fs_poste)));
            }
        } else if !e.poste.is_empty() {
            f.cumple = false;
            obs.push("Poste no está en catálogo".into());
        }
        if e.poste.is_empty() {
            f.cumple = false;
            obs.push("Sin poste asignado (ningún poste del catálogo cumple o falta asignar)".into());
        }
        if let Some(lon) = pg.and_then(|pp| pp.longitud_m) {
            if e.altura_amarre >= lon {
                f.cumple = false;
                obs.push("Altura de amarre ≥ longitud del poste".into());
            }
        }
        f.observacion = obs.join("; ");
        filas.push(f);
    }
    filas
}

/// Poste de menor carga nominal (y luego menor longitud) que cumple FS y altura.
pub fn elegir_poste(cat: &Catalogos, e: &Estructura, fh: f64, mb: f64, fs_poste: f64) -> String {
    let mut cand: Vec<&Poste> = cat.postes.iter().filter(|p| p.carga_nominal_dan.is_some()).collect();
    cand.sort_by(|a, b| {
        (a.carga_nominal_dan, a.longitud_m.unwrap_or(f64::INFINITY))
            .partial_cmp(&(b.carga_nominal_dan, b.longitud_m.unwrap_or(f64::INFINITY)))
            .unwrap()
    });
    for p in cand {
        if p.longitud_m.is_some_and(|l| e.altura_amarre >= l) {
            continue;
        }
        if lleva_retenida(e) {
            return p.codigo.clone();
        }
        let f_punta = match (p.longitud_m, p.empotramiento_m) {
            (Some(l), Some(emp)) => mb / (l - emp),
            _ => fh,
        };
        if f_punta <= 0.0 || p.carga_nominal_dan.unwrap() * 10.0 / f_punta >= fs_poste {
            return p.codigo.clone();
        }
    }
    String::new()
}

/// Momento resistente en la línea de tierra (N·m) para ordenar postes de menor a mayor.
fn capacidad(p: &Poste) -> Option<f64> {
    if let (Some(mor), Some(d)) = (p.mor_mpa, p.diam_tierra_m) {
        return Some(mor * 1e6 * std::f64::consts::PI * d.powi(3) / 32.0);
    }
    let brazo = p.longitud_m? - p.empotramiento_m? - p.dist_aplicacion_m.unwrap_or(0.0);
    Some(p.carga_nominal_dan? * 10.0 * brazo)
}

/// Peor FS por momento de vuelco (MRN, criterio de REDLIN) de un poste en todas las hipótesis.
pub fn fs_momentos(e: &Estructura, cargas: &[&Cargas], poste: &Poste, armado: Option<&Armado>, n: usize) -> f64 {
    let alt = alturas_fases(e, Some(poste), armado, n);
    let toma = retenida_toma(e, armado);
    cargas
        .iter()
        .map(|c| factor_seguridad_poste(poste, momentos_poste(c, &alt, poste, toma.as_deref()).mrn).0)
        .fold(f64::INFINITY, f64::min)
}

/// Poste más chico que cumple, como REDLIN: con geometría (diámetros, empotramiento y MOR o
/// carga nominal) se exige `FS(MRN) ≥ fs_poste` en todas las hipótesis, también con retenida
/// (según lo que ésta tome); sin geometría, el criterio simplificado de [`elegir_poste`].
/// Candidatos de menor a mayor momento resistente en la línea de tierra y luego por longitud.
pub fn elegir_poste_mrn(
    cat: &Catalogos, e: &Estructura, cargas: &[&Cargas], armado: Option<&Armado>, n: usize, fh: f64, mb: f64, fs_poste: f64,
) -> String {
    let mut cand: Vec<&Poste> = cat.postes.iter().filter(|p| p.carga_nominal_dan.is_some() || geometria(Some(p))).collect();
    let clave = |p: &Poste| {
        (capacidad(p).unwrap_or(f64::INFINITY), p.carga_nominal_dan.unwrap_or(f64::INFINITY), p.longitud_m.unwrap_or(f64::INFINITY))
    };
    cand.sort_by(|a, b| clave(a).partial_cmp(&clave(b)).unwrap_or(std::cmp::Ordering::Equal));
    for p in cand {
        if p.longitud_m.is_some_and(|l| e.altura_amarre >= l) {
            continue;
        }
        if geometria(Some(p)) {
            if fs_momentos(e, cargas, p, armado, n) >= fs_poste {
                return p.codigo.clone();
            }
            continue;
        }
        if lleva_retenida(e) {
            return p.codigo.clone();
        }
        let f_punta = match (p.longitud_m, p.empotramiento_m) {
            (Some(l), Some(emp)) => mb / (l - emp),
            _ => fh,
        };
        if f_punta <= 0.0 || p.carga_nominal_dan.unwrap_or(0.0) * 10.0 / f_punta >= fs_poste {
            return p.codigo.clone();
        }
    }
    String::new()
}

/// Fila de metrado.
#[derive(Clone, Debug, PartialEq)]
pub struct FilaMetrado {
    pub item: String,
    pub codigo: String,
    pub descripcion: String,
    pub unidad: String,
    pub cantidad: f64,
}

fn redondear(x: f64, d: i32) -> f64 {
    crate::numerico::redondear(x, d as usize)
}

/// Metrado: postes, armados, retenidas, materiales y conductor.
pub fn metrado(
    r: &Resultado, p: &Proyecto, cat: &Catalogos, desperdicio_pct: f64, retenida: Option<&str>,
    n_retenidas: &BTreeMap<usize, usize>,
) -> Vec<FilaMetrado> {
    let mut filas = Vec::new();
    let mut agregar = |item: &str, desc: String, und: &str, cant: f64, codigo: String| {
        filas.push(FilaMetrado { item: item.into(), codigo, descripcion: desc, unidad: und.into(), cantidad: cant });
    };
    let mut postes: BTreeMap<&str, usize> = BTreeMap::new();
    for e in r.estructuras.iter().filter(|e| !e.poste.is_empty()) {
        *postes.entry(&e.poste).or_default() += 1;
    }
    for (cod, n) in &postes {
        let d = match cat.poste(cod) {
            Some(pp) => format!("{cod} — {}", pp.material),
            None => cod.to_string(),
        };
        agregar("Poste", d, "und", *n as f64, cod.to_string());
    }
    let mut arms: BTreeMap<&str, usize> = BTreeMap::new();
    for e in r.estructuras.iter().filter(|e| !e.armado.is_empty()) {
        *arms.entry(&e.armado).or_default() += 1;
    }
    for (cod, n) in &arms {
        let d = match cat.armado(cod) {
            Some(a) => format!("{cod} — {}", a.descripcion),
            None => cod.to_string(),
        };
        agregar("Armado", d, "und", *n as f64, cod.to_string());
    }
    let n_ret: usize = r
        .estructuras
        .iter()
        .filter(|e| lleva_retenida(e))
        .map(|e| n_retenidas.get(&e.numero).copied().filter(|&n| n > 0).unwrap_or(1))
        .sum();
    if n_ret > 0 {
        let cod = retenida
            .map(|s| s.to_string())
            .or_else(|| cat.retenidas.first().map(|x| x.codigo.clone()))
            .unwrap_or_else(|| "Retenida".into());
        agregar("Retenida", cod.clone(), "und", n_ret as f64, cod);
    }
    let mut mats: BTreeMap<(String, String), f64> = BTreeMap::new();
    for (cod, n) in &arms {
        for m in cat.materiales.iter().filter(|m| m.armado == *cod) {
            if let Some(q) = m.cantidad {
                *mats.entry((m.material.clone(), m.unidad.clone())).or_default() += q * *n as f64;
            }
        }
    }
    for ((mat, und), cant) in mats {
        agregar("Material", mat.clone(), &und, redondear(cant, 2), mat);
    }
    let mut largo = 0.0;
    for v in &r.vanos {
        let e = r.tramos[v.tramo].estado(&p.hip_inicial).expect("estado inicial");
        largo += Curva::nueva(Modelo::Catenaria, v.longitud, v.desnivel, e.h, e.w_resultante).longitud();
    }
    let total = largo * p.n_conductores as f64 * (1.0 + desperdicio_pct / 100.0);
    let nom = &p.conductor.nombre;
    agregar(
        "Conductor",
        format!("{nom} ({} × {largo:.1} m, desperdicio {} %)", p.n_conductores, g(desperdicio_pct)),
        "m",
        redondear(total, 1),
        nom.clone(),
    );
    agregar("Conductor", format!("{nom} — masa"), "kg", redondear(total * p.conductor.peso / G, 1), String::new());
    filas
}
