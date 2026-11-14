//! GridCraft: red radial (MT/BT) por sumatoria de potencias. Port de `gridcraft/red.py`.

use std::collections::HashMap;

use crate::catalogos::leer_csv;
use crate::linea::fmt_g;

/// Nodo de la red: el tramo `padre → nodo` lleva su longitud y conductor.
#[derive(Clone, Debug, Default)]
pub struct Nodo {
    pub nodo: String,
    pub padre: String,
    pub longitud_m: f64,
    pub conductor: String,
    pub lotes: f64,
    pub kw_ce: f64,
    pub lamparas: f64,
    pub este: Option<f64>,
    pub norte: Option<f64>,
    /// Estructura del nodo para el metrado (opcional).
    pub estructura: EstructuraBt,
}

/// Datos de la estructura de un nodo para el metrado (columnas opcionales del CSV de nodos).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EstructuraBt {
    /// Armado: `E1/S`, `2E3/S` (2 unidades), `E3+E3/S` (uno de cada).
    pub armado: String,
    pub soporte: String,
    pub n_soportes: f64,
    pub pat: String,
    pub caja: String,
    pub pastoral: String,
    pub retenida: String,
    pub n_retenidas: f64,
    /// (tipo, cantidad, servicio particular) — `false` = alumbrado público.
    pub conectores: Vec<(String, f64, bool)>,
    /// Acometidas: `C`/`L`/`X` (corta, larga, extra larga) + `3` si es trifásica + `M` con murete.
    pub acometidas: Vec<String>,
}

/// Renglón del metrado de la red.
#[derive(Clone, Debug, PartialEq)]
pub struct FilaMetradoRed {
    pub item: String,
    pub codigo: String,
    pub unidad: String,
    pub cantidad: f64,
}

/// Armado con multiplicador y suma: `2E3/S` -> [(2, E3/S)], `E3+E3/S` -> [(1, E3), (1, E3/S)].
pub fn partes_armado(a: &str) -> Vec<(f64, String)> {
    a.split('+')
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "-")
        .map(|t| {
            let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            if d.is_empty() || d.len() == t.len() { (1.0, t.to_string()) } else { (d.parse().unwrap_or(1.0), t[d.len()..].to_string()) }
        })
        .collect()
}

/// Metrado de la red (como el «Metrado de Redes Secundarias» de REDCAD): armados, soportes,
/// puestas a tierra, retenidas, cajas, pastorales, puntos de iluminación, conectores,
/// acometidas y conductor (km por formación, suma de vanos sin recargo).
pub fn metrado(red: &Red) -> Vec<FilaMetradoRed> {
    use std::collections::BTreeMap;
    let mut grupos: Vec<(&str, &str, BTreeMap<String, f64>)> = ["Armado", "Soporte", "Puesta a tierra", "Retenida", "Caja de derivación",
        "Pastoral", "Punto de iluminación", "Conector", "Acometida", "Conductor"]
        .iter()
        .map(|g| (*g, if *g == "Conductor" { "km" } else { "und" }, BTreeMap::new()))
        .collect();
    let mut sumar = |g: usize, k: &str, v: f64| {
        let k = k.trim();
        if !k.is_empty() && k != "-" && v != 0.0 {
            *grupos[g].2.entry(k.to_string()).or_default() += v;
        }
    };
    for n in &red.nodos {
        let e = &n.estructura;
        for (m, c) in partes_armado(&e.armado) {
            sumar(0, &c, m);
        }
        sumar(1, &e.soporte, if e.n_soportes > 0.0 { e.n_soportes } else { 1.0 });
        sumar(2, &e.pat, 1.0);
        sumar(3, &e.retenida, if e.n_retenidas > 0.0 { e.n_retenidas } else { 1.0 });
        sumar(4, &e.caja, 1.0);
        sumar(5, &e.pastoral, 1.0);
        sumar(6, "Puntos de iluminación", n.lamparas);
        for (t, q, sp) in &e.conectores {
            sumar(7, &format!("{t}-{}", if *sp { "SP" } else { "AP" }), *q);
        }
        for a in &e.acometidas {
            let a = a.to_uppercase();
            let largo = match a.chars().next() {
                Some('L') => "Largas",
                Some('X') => "Extra Largas",
                _ => "Cortas",
            };
            sumar(8, &format!("Acometidas {largo}"), 1.0);
            sumar(8, if a.contains('3') { "Acometidas 3φ" } else { "Acometidas 1φ" }, 1.0);
            if a.contains('M') {
                sumar(8, &format!("Muretes en acometidas {}", largo.to_lowercase()), 1.0);
            }
        }
        if !n.padre.is_empty() && n.longitud_m > 0.0 {
            sumar(9, &n.conductor, n.longitud_m / 1000.0);
        }
    }
    grupos
        .into_iter()
        .flat_map(|(g, u, m)| m.into_iter().map(move |(k, v)| FilaMetradoRed { item: g.into(), codigo: k, unidad: u.into(), cantidad: v }))
        .collect()
}

/// Materiales por armado de la Norma DGE RD 023-2003 (plantilla `materiales_bt.csv`).
pub const MATERIALES_BT: &str = include_str!("../../suite_electrica/plantillas/materiales_bt.csv");

/// Materiales del metrado (cantidad por unidad × unidades): armados (`E'n` usa los de `En`),
/// retenidas, puesta a tierra, pastorales y acometidas cuyo código esté en la tabla.
/// Devuelve los materiales y los armados que no están en la tabla.
pub fn materiales(metrado: &[FilaMetradoRed], tabla_csv: &str) -> Result<(Vec<FilaMetradoRed>, Vec<String>), String> {
    use std::collections::BTreeMap;
    let tabla = leer_csv(tabla_csv)?;
    let mut por: BTreeMap<(String, String), f64> = BTreeMap::new();
    let mut faltan = Vec::new();
    for m in metrado.iter().filter(|m| m.item != "Conductor") {
        let base = if m.item == "Armado" { m.codigo.replace('\'', "") } else { m.codigo.clone() };
        let filas: Vec<_> = tabla.iter().filter(|f| f.get("armado").map(|a| a.trim()) == Some(base.as_str())).collect();
        if filas.is_empty() && m.item == "Armado" {
            faltan.push(m.codigo.clone());
        }
        for f in filas {
            let q: f64 = f.get("cantidad").and_then(|x| x.trim().replace(',', ".").parse().ok()).unwrap_or(0.0);
            let k = (f.get("material").cloned().unwrap_or_default(), f.get("unidad").cloned().unwrap_or_default());
            *por.entry(k).or_default() += q * m.cantidad;
        }
    }
    let filas = por
        .into_iter()
        .map(|((mat, u), q)| FilaMetradoRed { item: "Material".into(), codigo: mat, unidad: u, cantidad: crate::numerico::redondear(q, 3) })
        .collect();
    Ok((filas, faltan))
}

/// Metrado de GridCraft -> `rubro, uc, cantidad` para PresuCraft.
pub fn metrado_presupuesto(filas: &[FilaMetradoRed], rubro: &str) -> String {
    let mut out = String::from("rubro,uc,cantidad
");
    for f in filas {
        let uc = if f.codigo.contains(',') || f.codigo.contains('"') { format!("\"{}\"", f.codigo.replace('"', "\"\"")) } else { f.codigo.clone() };
        out.push_str(&format!("{rubro},{uc},{}
", f.cantidad));
    }
    out
}

/// Conductor de la red (Ω/km, A).
#[derive(Clone, Debug, Default)]
pub struct ConductorRed {
    pub formacion: String,
    pub k_ohm_km: Option<f64>,
    pub r_ohm_km: Option<f64>,
    pub x_ohm_km: Option<f64>,
    pub k_ap_ohm_km: Option<f64>,
    pub r_ap_ohm_km: Option<f64>,
    pub corriente_max_a: Option<f64>,
    pub fuente: String,
    /// Fases del tramo (1, 2 o 3). Si se indica, cada tramo se calcula con sus fases (como REDCAD).
    pub fases: Option<f64>,
}

/// Datos de una red radial.
#[derive(Clone, Debug)]
pub struct Red {
    pub nombre: String,
    /// "3F" o "1F".
    pub sistema: String,
    pub tension_v: f64,
    pub nodos: Vec<Nodo>,
    pub conductores: Vec<ConductorRed>,
    pub calificacion_kw_lote: f64,
    pub fs_sp: f64,
    pub fp_sp: f64,
    pub fs_ce: f64,
    pub kw_lampara: f64,
    pub fs_ap: f64,
    pub fp_ap: f64,
    pub tension_ap_v: Option<f64>,
    /// Tensión fase-neutro para tramos de 1 fase (por defecto V/√3; 220 V en 440/220 V).
    pub tension_fn_v: Option<f64>,
    pub caida_max_pct: f64,
    pub perdidas_max_pct: f64,
    pub factor_carga: f64,
    pub transformador_kva: f64,
    pub factor_sobrecarga: f64,
    /// `[(abonados, fs)]` escalonada.
    pub tabla_fs: Vec<(f64, f64)>,
    pub fuentes: HashMap<String, String>,
}

pub const PARAMETROS_NORMATIVOS: [&str; 4] = ["calificacion_kw_lote", "fs_sp", "caida_max_pct", "conductores"];

impl Red {
    pub fn fs_efectivo(&self) -> f64 {
        let n = self.nodos.iter().map(|x| x.lotes).sum::<f64>().trunc();
        let mut tabla = self.tabla_fs.clone();
        tabla.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut fs = self.fs_sp;
        for (umbral, v) in tabla {
            if n >= umbral {
                fs = v;
            }
        }
        fs
    }

    pub fn avisos_fuentes(&self) -> Vec<String> {
        PARAMETROS_NORMATIVOS
            .iter()
            .filter_map(|k| {
                let f = self.fuentes.get(*k).map(|s| s.trim().to_string()).unwrap_or_default();
                let u = f.to_uppercase();
                if f.is_empty() || u.contains("PENDIENTE") || u.contains("EJEMPLO") {
                    Some(format!("PENDIENTE_REVISION: '{k}' -> {}", if f.is_empty() { "sin fuente" } else { &f }))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Fila por tramo (valores redondeados como en la planilla de Python).
#[derive(Clone, Debug, Default)]
pub struct FilaRed {
    pub nodo: String,
    pub padre: String,
    pub longitud_m: f64,
    pub formacion: String,
    pub lotes: f64,
    pub p_sp_kw: f64,
    pub p_acum_kw: f64,
    pub k_ohm_km: f64,
    pub i_a: f64,
    pub dv_tramo_v: f64,
    pub dv_acum_v: f64,
    pub dv_pct: f64,
    pub perdidas_w: f64,
    pub i_max_a: Option<f64>,
    /// (P acum. AP kW, I AP A, %ΔV AP, pérdidas AP W) si el conductor tiene hilo de AP.
    pub ap: Option<(f64, f64, f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlertaRed {
    pub elemento: String,
    pub codigo: String,
    pub exceso: f64,
    pub detalle: String,
}

#[derive(Clone, Debug)]
pub struct ResultadoRed {
    pub tramos: Vec<FilaRed>,
    /// Parámetro → valor (en el orden de la planilla).
    pub resumen: Vec<(String, f64)>,
    pub avisos: Vec<String>,
    pub alertas: Vec<AlertaRed>,
}

impl ResultadoRed {
    pub fn cumple(&self) -> bool {
        self.alertas.is_empty()
    }

    pub fn resumen(&self, k: &str) -> Option<f64> {
        self.resumen.iter().find(|x| x.0 == k).map(|x| x.1)
    }
}

fn rd(x: f64, d: i32) -> f64 {
    crate::numerico::redondear(x, d as usize)
}

/// (k de corriente, conductores con pérdidas, c de K desde r y x).
fn sistema(s: &str) -> Option<(f64, f64, f64)> {
    match s {
        "3F" => Some((3f64.sqrt(), 3.0, 3f64.sqrt())),
        "1F" => Some((1.0, 2.0, 2.0)),
        _ => None,
    }
}

fn factor_k(c: &ConductorRed, sist: &str, fp: f64, ap: bool) -> Option<f64> {
    let (k, r, x) = if ap { (c.k_ap_ohm_km, c.r_ap_ohm_km, Some(0.0)) } else { (c.k_ohm_km, c.r_ohm_km, c.x_ohm_km) };
    if let Some(k) = k {
        return Some(k);
    }
    let r = r?;
    let cc = if ap { 2.0 } else { sistema(sist).unwrap().2 };
    Some(cc * (r * fp + x.unwrap_or(0.0) * (1.0 - fp * fp).sqrt()))
}

/// Índices de la raíz a las hojas (DFS como en Python); valida árbol con una sola raíz.
fn orden_topologico(n: &[Nodo]) -> Result<Vec<usize>, String> {
    let raices: Vec<usize> = (0..n.len()).filter(|&i| n[i].padre.is_empty()).collect();
    if raices.len() != 1 {
        return Err(format!("La red debe tener una sola raíz (padre vacío); hay {}", raices.len()));
    }
    let mut vistos = std::collections::BTreeSet::new();
    let mut rep = std::collections::BTreeSet::new();
    for x in n {
        if !vistos.insert(x.nodo.clone()) {
            rep.insert(x.nodo.clone());
        }
    }
    if !rep.is_empty() {
        return Err(format!("Nodos repetidos: {rep:?}"));
    }
    let mut hijos: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, x) in n.iter().enumerate() {
        if !x.padre.is_empty() {
            hijos.entry(&x.padre).or_default().push(i);
        }
    }
    let faltan: std::collections::BTreeSet<&str> = hijos.keys().copied().filter(|p| !vistos.contains(*p)).collect();
    if !faltan.is_empty() {
        return Err(format!("Padres inexistentes: {faltan:?}"));
    }
    let (mut orden, mut pila) = (Vec::new(), vec![raices[0]]);
    while let Some(i) = pila.pop() {
        orden.push(i);
        if let Some(h) = hijos.get(n[i].nodo.as_str()) {
            pila.extend(h.iter().rev());
        }
    }
    if orden.len() != n.len() {
        return Err("La red tiene ciclos o nodos desconectados de la raíz".into());
    }
    Ok(orden)
}

/// Flujo radial por sumatoria de potencias.
pub fn calcular(red: &Red) -> Result<ResultadoRed, String> {
    let (k_i, n_f, _) = sistema(&red.sistema).ok_or_else(|| format!("Sistema {:?}; use [\"1F\", \"3F\"]", red.sistema))?;
    let n = &red.nodos;
    let orden = orden_topologico(n)?;
    let mut avisos = red.avisos_fuentes();
    let cat: HashMap<&str, &ConductorRed> = red.conductores.iter().map(|c| (c.formacion.as_str(), c)).collect();
    let mut sin_cat: Vec<&str> =
        n.iter().filter(|x| !x.padre.is_empty() && !cat.contains_key(x.conductor.as_str())).map(|x| x.conductor.as_str()).collect();
    sin_cat.sort();
    sin_cat.dedup();
    if !sin_cat.is_empty() {
        return Err(format!("Conductores no definidos en el catálogo: {sin_cat:?}"));
    }
    let pend: Vec<&str> = red
        .conductores
        .iter()
        .filter(|c| c.fuente.is_empty() || c.fuente.to_uppercase().contains("PENDIENTE"))
        .map(|c| c.formacion.as_str())
        .collect();
    if !pend.is_empty() {
        avisos.push(format!("PENDIENTE_REVISION: catálogo de conductores sin fuente ({})", pend.join(", ")));
    }

    let fs = red.fs_efectivo();
    let p_sp: Vec<f64> = n.iter().map(|x| x.lotes * red.calificacion_kw_lote * fs + x.kw_ce * red.fs_ce).collect();
    let p_ap: Vec<f64> = n.iter().map(|x| x.lamparas * red.kw_lampara * red.fs_ap).collect();
    let v_ap = red.tension_ap_v.filter(|&v| v != 0.0).unwrap_or(if red.sistema == "3F" {
        red.tension_v / 3f64.sqrt()
    } else {
        red.tension_v
    });
    // fases por tramo (REDCAD): todos los conductores usados deben declararlas
    let por_tramo = red.conductores.iter().any(|c| c.fases.is_some());
    if por_tramo {
        let mut sin: Vec<&str> = n
            .iter()
            .filter(|x| !x.padre.is_empty() && cat[x.conductor.as_str()].fases.is_none())
            .map(|x| x.conductor.as_str())
            .collect();
        sin.sort();
        sin.dedup();
        if !sin.is_empty() {
            return Err(format!("Conductores sin 'fases' (si una formación las declara, todas deben): {sin:?}"));
        }
    }
    let v_fn = red.tension_fn_v.filter(|&v| v > 0.0).unwrap_or(red.tension_v / 3f64.sqrt());
    let v_ap = if por_tramo { red.tension_ap_v.filter(|&v| v != 0.0).unwrap_or(v_fn) } else { v_ap };
    let ap_sep = |f: &str| cat.get(f).is_some_and(|c| c.k_ap_ohm_km.is_some() || c.r_ap_ohm_km.is_some());

    let pos: HashMap<&str, usize> = n.iter().enumerate().map(|(i, x)| (x.nodo.as_str(), i)).collect();
    let (mut acum_sp, mut acum_ap) = (p_sp.clone(), p_ap.clone());
    for &i in orden.iter().rev() {
        if !n[i].padre.is_empty() {
            let j = pos[n[i].padre.as_str()];
            acum_sp[j] += acum_sp[i];
            acum_ap[j] += acum_ap[i];
        }
    }

    let (mut dv_sp, mut dv_ap): (HashMap<&str, f64>, HashMap<&str, f64>) = (HashMap::new(), HashMap::new());
    let mut tramos = Vec::new();
    for &i in &orden {
        let (nodo, padre) = (n[i].nodo.as_str(), n[i].padre.as_str());
        if padre.is_empty() {
            dv_sp.insert(nodo, 0.0);
            dv_ap.insert(nodo, 0.0);
            continue;
        }
        let c = cat[n[i].conductor.as_str()];
        let l = n[i].longitud_m / 1000.0;
        let sep = ap_sep(&n[i].conductor);
        let p = acum_sp[i] + if sep { 0.0 } else { acum_ap[i] };
        let k = factor_k(c, &red.sistema, red.fp_sp, false)
            .ok_or_else(|| format!("Conductor {}: falta k_ohm_km o r_ohm_km", c.formacion))?;
        // ponytail: en modo por tramo la caída se acumula en V fase-neutro (%ΔV sobre V fase-neutro)
        // corriente con la tensión real en el extremo emisor del tramo (como REDCAD)
        let caida = if por_tramo { (v_fn - dv_sp[padre]) / v_fn } else { 1.0 };
        let (corr, dv, nf) = match c.fases.filter(|_| por_tramo).map(|f| f.round() as i64) {
            Some(1) => {
                let i = p * 1e3 / (v_fn * caida * red.fp_sp);
                (i, k * i * l, 1.0)
            }
            Some(f) => {
                let ki = if f == 3 { 3f64.sqrt() } else { 1.0 };
                let i = p * 1e3 / (ki * red.tension_v * caida * red.fp_sp);
                (i, k * i * l * v_fn / red.tension_v, f as f64)
            }
            None => {
                let i = p * 1e3 / (k_i * red.tension_v * red.fp_sp);
                (i, k * i * l, n_f)
            }
        };
        let perd = c.r_ohm_km.map_or(f64::NAN, |r| nf * corr * corr * r * l);
        let acum = dv_sp[padre] + dv;
        dv_sp.insert(nodo, acum);
        let ap = if sep {
            let v_ap_emisor = if por_tramo { v_ap - dv_ap[padre] } else { v_ap };
            let i_ap = acum_ap[i] * 1e3 / (v_ap_emisor * red.fp_ap);
            let k_ap = factor_k(c, &red.sistema, red.fp_ap, true).unwrap_or(f64::NAN);
            let a = dv_ap[padre] + k_ap * i_ap * l;
            dv_ap.insert(nodo, a);
            let n_ap = if por_tramo { 1.0 } else { 2.0 };
            let perd_ap = c.r_ap_ohm_km.map_or(f64::NAN, |r| rd(n_ap * i_ap * i_ap * r * l, 2));
            Some((rd(acum_ap[i], 3), rd(i_ap, 2), rd(100.0 * a / v_ap, 3), perd_ap))
        } else {
            let a = dv_ap[padre];
            dv_ap.insert(nodo, a);
            None
        };
        tramos.push(FilaRed {
            nodo: nodo.into(),
            padre: padre.into(),
            longitud_m: rd(l * 1e3, 2),
            formacion: c.formacion.clone(),
            lotes: n[i].lotes,
            p_sp_kw: rd(p_sp[i], 3),
            p_acum_kw: rd(p, 3),
            k_ohm_km: rd(k, 4),
            i_a: rd(corr, 2),
            dv_tramo_v: rd(dv, 3),
            dv_acum_v: rd(acum, 3),
            dv_pct: rd(100.0 * acum / if por_tramo { v_fn } else { red.tension_v }, 3),
            perdidas_w: rd(perd, 2),
            i_max_a: c.corriente_max_a,
            ap,
        });
    }

    let raiz = orden[0];
    let md = acum_sp[raiz] + acum_ap[raiz];
    let nansum = |it: &mut dyn Iterator<Item = f64>| it.filter(|x| !x.is_nan()).sum::<f64>();
    let perd_kw = (nansum(&mut tramos.iter().map(|t| t.perdidas_w))
        + nansum(&mut tramos.iter().filter_map(|t| t.ap.map(|a| a.3))))
        / 1e3;
    let hay_ap = tramos.iter().any(|t| t.ap.is_some());
    let mut resumen: Vec<(String, f64)> = vec![
        ("Abonados".into(), n.iter().map(|x| x.lotes).sum::<f64>().trunc()),
        ("Factor de simultaneidad SP".into(), fs),
        ("M.D. SP (kW)".into(), rd(acum_sp[raiz], 3)),
        ("M.D. AP (kW)".into(), rd(acum_ap[raiz], 3)),
        ("M.D. total (kW)".into(), rd(md, 3)),
        ("Potencia aparente (kVA)".into(), rd(acum_sp[raiz] / red.fp_sp + acum_ap[raiz] / red.fp_ap, 3)),
        ("Pérdidas (kW)".into(), rd(perd_kw, 4)),
        ("Pérdidas (% M.D.)".into(), if md != 0.0 { rd(100.0 * perd_kw / md, 3) } else { 0.0 }),
        ("%ΔV máx.".into(), tramos.iter().map(|t| t.dv_pct).fold(0.0f64, f64::max)),
    ];
    if hay_ap {
        let m = tramos.iter().filter_map(|t| t.ap.map(|a| a.2)).fold(f64::NEG_INFINITY, f64::max);
        resumen.push(("%ΔV AP máx.".into(), m));
    }
    if red.factor_carga != 0.0 {
        let fper = 0.3 * red.factor_carga + 0.7 * red.factor_carga * red.factor_carga;
        resumen.push(("Factor de pérdidas".into(), rd(fper, 4)));
        resumen.push(("Energía perdida (kWh/año)".into(), rd(perd_kw * fper * 8760.0, 1)));
        resumen.push(("Energía vendida (kWh/año)".into(), rd(md * red.factor_carga * 8760.0, 1)));
    }

    let mut al: Vec<AlertaRed> = Vec::new();
    let mut add = |elem: String, cod: &str, ev: f64, perm: f64, det: String| {
        if perm > 0.0 && ev > perm + 1e-9 {
            al.push(AlertaRed { elemento: elem, codigo: cod.into(), exceso: rd(100.0 * (ev - perm) / perm, 1), detalle: det });
        }
    };
    for t in &tramos {
        if red.caida_max_pct != 0.0 {
            add(t.nodo.clone(), "DV", t.dv_pct, red.caida_max_pct, format!("%ΔV {:.2} % > {} %", t.dv_pct, fmt_g(red.caida_max_pct)));
        }
        if let Some(im) = t.i_max_a {
            add(format!("{}-{}", t.padre, t.nodo), "I", t.i_a, im, format!("I {:.1} A > {} A", t.i_a, fmt_g(im)));
        }
    }
    if red.caida_max_pct != 0.0 {
        for t in &tramos {
            if let Some(a) = t.ap.filter(|a| !a.2.is_nan()) {
                add(t.nodo.clone(), "DV-AP", a.2, red.caida_max_pct, format!("%ΔV AP {:.2} % > {} %", a.2, fmt_g(red.caida_max_pct)));
            }
        }
    }
    let r = |k: &str| resumen.iter().find(|x| x.0 == k).unwrap().1;
    if red.perdidas_max_pct != 0.0 {
        let v = r("Pérdidas (% M.D.)");
        add(red.nombre.clone(), "PERD", v, red.perdidas_max_pct, format!("pérdidas {v:.2} % > {} %", fmt_g(red.perdidas_max_pct)));
    }
    if red.transformador_kva != 0.0 {
        let cap = red.transformador_kva * red.factor_sobrecarga;
        let v = r("Potencia aparente (kVA)");
        add(n[raiz].nodo.clone(), "TR", v, cap, format!("{v:.1} kVA > {cap:.1} kVA"));
    }
    al.sort_by(|a, b| b.exceso.partial_cmp(&a.exceso).unwrap());
    Ok(ResultadoRed { tramos, resumen, avisos, alertas: al })
}

/// Nodos y conductores desde el texto de sus CSV.
pub fn leer_tablas(nodos_csv: &str, conductores_csv: &str) -> Result<(Vec<Nodo>, Vec<ConductorRed>), String> {
    let num = |f: &std::collections::BTreeMap<String, String>, k: &str| {
        f.get(k).and_then(|s| s.replace(',', ".").parse::<f64>().ok()).filter(|v| v.is_finite())
    };
    let txt = |f: &std::collections::BTreeMap<String, String>, k: &str| f.get(k).cloned().unwrap_or_default();
    let nodos = leer_csv(nodos_csv)?
        .iter()
        .map(|f| Nodo {
            nodo: txt(f, "nodo"),
            padre: txt(f, "padre"),
            longitud_m: num(f, "longitud_m").unwrap_or(0.0),
            conductor: txt(f, "conductor"),
            lotes: num(f, "lotes").unwrap_or(0.0),
            kw_ce: num(f, "kw_ce").unwrap_or(0.0),
            lamparas: num(f, "lamparas").unwrap_or(0.0),
            este: num(f, "este"),
            norte: num(f, "norte"),
            estructura: EstructuraBt {
                armado: txt(f, "armado"),
                soporte: txt(f, "soporte"),
                n_soportes: num(f, "n_soportes").unwrap_or(0.0),
                pat: txt(f, "pat"),
                caja: txt(f, "caja"),
                pastoral: txt(f, "pastoral"),
                retenida: txt(f, "retenida"),
                n_retenidas: num(f, "n_retenidas").unwrap_or(0.0),
                // "CFF35-16:3:SP; CNN25-25:1:AP"
                conectores: txt(f, "conectores")
                    .split(';')
                    .filter_map(|c| {
                        let p: Vec<&str> = c.split(':').map(str::trim).collect();
                        (!p[0].is_empty()).then(|| {
                            (p[0].to_string(), p.get(1).and_then(|q| q.parse().ok()).unwrap_or(1.0), p.get(2).map_or(true, |s| !s.eq_ignore_ascii_case("AP")))
                        })
                    })
                    .collect(),
                acometidas: txt(f, "acometidas").split(';').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
            },
        })
        .collect();
    let cond = leer_csv(conductores_csv)?
        .iter()
        .map(|f| ConductorRed {
            formacion: txt(f, "formacion"),
            k_ohm_km: num(f, "k_ohm_km"),
            r_ohm_km: num(f, "r_ohm_km"),
            x_ohm_km: num(f, "x_ohm_km"),
            k_ap_ohm_km: num(f, "k_ap_ohm_km"),
            r_ap_ohm_km: num(f, "r_ap_ohm_km"),
            corriente_max_a: num(f, "corriente_max_A"),
            fuente: txt(f, "fuente"),
            fases: num(f, "fases"),
        })
        .collect();
    Ok((nodos, cond))
}

fn leer_relativo(base: &std::path::Path, cfg: &serde_json::Value, k: &str) -> Result<String, String> {
    let r = cfg.get(k).and_then(|v| v.as_str()).ok_or(format!("proyecto: falta '{k}'"))?;
    let ruta = base.join(r);
    std::fs::read(&ruta).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("{}: {e}", ruta.display()))
}

/// Texto → mapa de fuentes (`"fuentes": {...}` del JSON).
pub(crate) fn fuentes_json(cfg: &serde_json::Value) -> HashMap<String, String> {
    cfg.get("fuentes")
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect())
        .unwrap_or_default()
}

/// Lee una red `.json` (parámetros + `nodos` y `conductores` CSV con rutas relativas).
pub fn cargar(ruta: &std::path::Path) -> Result<Red, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let cfg: serde_json::Value = serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let base = ruta.parent().unwrap_or(std::path::Path::new("."));
    let (nodos, conductores) = leer_tablas(&leer_relativo(base, &cfg, "nodos")?, &leer_relativo(base, &cfg, "conductores")?)?;
    let f = |k: &str, d: Option<f64>| cfg.get(k).and_then(|v| v.as_f64()).or(d).ok_or(format!("proyecto: falta '{k}'"));
    Ok(Red {
        nombre: cfg.get("nombre").and_then(|v| v.as_str()).map(String::from)
            .unwrap_or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
        sistema: cfg.get("sistema").and_then(|v| v.as_str()).ok_or("proyecto: falta 'sistema'")?.into(),
        tension_v: f("tension_V", None)?,
        nodos,
        conductores,
        calificacion_kw_lote: f("calificacion_kw_lote", None)?,
        fs_sp: f("fs_sp", None)?,
        fp_sp: f("fp_sp", Some(0.9))?,
        fs_ce: f("fs_ce", Some(1.0))?,
        kw_lampara: f("kw_lampara", Some(0.0))?,
        fs_ap: f("fs_ap", Some(1.0))?,
        fp_ap: f("fp_ap", Some(1.0))?,
        tension_ap_v: cfg.get("tension_ap_V").and_then(|v| v.as_f64()),
        tension_fn_v: cfg.get("tension_fn_V").and_then(|v| v.as_f64()),
        caida_max_pct: f("caida_max_pct", Some(0.0))?,
        perdidas_max_pct: f("perdidas_max_pct", Some(0.0))?,
        factor_carga: f("factor_carga", Some(0.0))?,
        transformador_kva: f("transformador_kva", Some(0.0))?,
        factor_sobrecarga: f("factor_sobrecarga", Some(1.0))?,
        tabla_fs: cfg.get("tabla_fs").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?))).collect()
        }).unwrap_or_default(),
        fuentes: fuentes_json(&cfg),
    })
}

/// Guarda la red: actualiza el JSON (parámetros; conserva las claves que no conoce) y reescribe
/// el CSV de nodos (con las columnas de estructura) y el de conductores. Los comentarios `#` de
/// los CSV no se conservan.
pub fn guardar(red: &Red, ruta: &std::path::Path) -> Result<(), String> {
    use serde_json::{json, Map, Value};
    let mut cfg: Map<String, Value> = std::fs::read_to_string(ruta)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let nombre = |k: &str, d: &str| cfg.get(k).and_then(|v| v.as_str()).unwrap_or(d).to_string();
    let (f_nodos, f_cond) = (nombre("nodos", "nodos.csv"), nombre("conductores", "conductores_bt.csv"));
    let opt = |v: Option<f64>| v.map_or(Value::Null, |x| json!(x));
    for (k, v) in [
        ("nombre", json!(red.nombre)),
        ("nodos", json!(f_nodos)),
        ("conductores", json!(f_cond)),
        ("sistema", json!(red.sistema)),
        ("tension_V", json!(red.tension_v)),
        ("calificacion_kw_lote", json!(red.calificacion_kw_lote)),
        ("fs_sp", json!(red.fs_sp)),
        ("tabla_fs", json!(red.tabla_fs.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>())),
        ("fp_sp", json!(red.fp_sp)),
        ("fs_ce", json!(red.fs_ce)),
        ("kw_lampara", json!(red.kw_lampara)),
        ("fs_ap", json!(red.fs_ap)),
        ("fp_ap", json!(red.fp_ap)),
        ("tension_ap_V", opt(red.tension_ap_v)),
        ("tension_fn_V", opt(red.tension_fn_v)),
        ("caida_max_pct", json!(red.caida_max_pct)),
        ("perdidas_max_pct", json!(red.perdidas_max_pct)),
        ("factor_carga", json!(red.factor_carga)),
        ("transformador_kva", json!(red.transformador_kva)),
        ("factor_sobrecarga", json!(red.factor_sobrecarga)),
        ("fuentes", Value::Object(red.fuentes.iter().map(|(k, v)| (k.clone(), json!(v))).collect())),
    ] {
        if v.is_null() {
            cfg.remove(k);
        } else {
            cfg.insert(k.into(), v);
        }
    }
    let base = ruta.parent().unwrap_or(std::path::Path::new("."));
    let num = |x: f64| if x.is_nan() { String::new() } else { x.to_string() };
    let on = |x: Option<f64>| x.map(num).unwrap_or_default();
    let mut w = csv::Writer::from_writer(Vec::new());
    let err = |e: csv::Error| e.to_string();
    w.write_record([
        "nodo", "padre", "longitud_m", "conductor", "lotes", "kw_ce", "lamparas", "este", "norte", "armado", "soporte", "n_soportes",
        "pat", "caja", "pastoral", "retenida", "n_retenidas", "conectores", "acometidas",
    ])
    .map_err(err)?;
    for n in &red.nodos {
        let e = &n.estructura;
        let conectores = e
            .conectores
            .iter()
            .map(|(t, q, sp)| format!("{t}:{}:{}", num(*q), if *sp { "SP" } else { "AP" }))
            .collect::<Vec<_>>()
            .join(";");
        let nz = |x: f64| if x == 0.0 { String::new() } else { num(x) };
        w.write_record([
            n.nodo.clone(), n.padre.clone(), if n.padre.is_empty() { String::new() } else { num(n.longitud_m) }, n.conductor.clone(),
            num(n.lotes), num(n.kw_ce), num(n.lamparas), on(n.este), on(n.norte), e.armado.clone(), e.soporte.clone(),
            nz(e.n_soportes), e.pat.clone(), e.caja.clone(), e.pastoral.clone(), e.retenida.clone(), nz(e.n_retenidas), conectores,
            e.acometidas.join(";"),
        ])
        .map_err(err)?;
    }
    let nodos = w.into_inner().map_err(|e| e.to_string())?;
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(["formacion", "k_ohm_km", "r_ohm_km", "x_ohm_km", "k_ap_ohm_km", "r_ap_ohm_km", "corriente_max_A", "fases", "fuente"])
        .map_err(err)?;
    for c in &red.conductores {
        w.write_record([
            c.formacion.clone(), on(c.k_ohm_km), on(c.r_ohm_km), on(c.x_ohm_km), on(c.k_ap_ohm_km), on(c.r_ap_ohm_km),
            on(c.corriente_max_a), on(c.fases), c.fuente.clone(),
        ])
        .map_err(err)?;
    }
    let cond = w.into_inner().map_err(|e| e.to_string())?;
    let escribir = |n: &str, b: &[u8]| std::fs::write(base.join(n), b).map_err(|e| format!("{n}: {e}"));
    escribir(&f_nodos, &nodos)?;
    escribir(&f_cond, &cond)?;
    let texto = serde_json::to_string_pretty(&Value::Object(cfg)).map_err(|e| e.to_string())?;
    std::fs::write(ruta, texto + "\n").map_err(|e| format!("{}: {e}", ruta.display()))
}

/// Longitud del tramo padre -> nodo según las coordenadas (m), si ambos las tienen.
pub fn longitud_plano(red: &Red, i: usize) -> Option<f64> {
    let n = &red.nodos[i];
    let p = red.nodos.iter().find(|x| x.nodo == n.padre)?;
    Some((n.este? - p.este?).hypot(n.norte? - p.norte?))
}
