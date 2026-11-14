//! PresuCraft: análisis de precios unitarios (APU) y fórmula polinómica (DS 011-79-VC).
//!
//! Criterios (validados contra PRESIO 2023 con `comparar-presio`):
//!
//! * **APU.** Mano de obra y equipo por cuadrilla: cantidad = cuadrilla · jornada / rendimiento,
//!   redondeada a 2 decimales; parcial = cantidad · precio, redondeado a 2 decimales (redondeo de
//!   Excel: la mitad hacia arriba). Líneas en `%mo`: % del subtotal de mano de obra. Líneas en `%`:
//!   % de la suma (sin redondear) de las líneas anteriores de su sección o, si es la primera, del
//!   subtotal de la sección anterior. Secciones: mano de obra, materiales, equipos, maquinaria y
//!   gastos varios; total = suma de subtotales.
//! * **Fórmula polinómica.** Parciales por Índice Unificado (INEI); incidencia = parcial / total
//!   redondeada a 3 decimales; agrupación preliminar (índice → índice similar) y conformación de
//!   monomios (grupo → símbolo); coeficiente de cada monomio = suma de incidencias; el residuo
//!   del redondeo (1 − Σ) va al índice de mayor parcial. El DS exige
//!   como máximo 8 monomios, cada uno de al menos 5 %, y coeficientes que sumen 1.

use std::collections::BTreeMap;

use crate::numerico::redondear;

/// Redondeo de Excel (`REDONDEAR`): la mitad se aleja de cero; tolera la representación binaria.
pub fn redondear_excel(x: f64, d: i32) -> f64 {
    let k = 10f64.powi(d);
    let y = x * k;
    (y + y.signum() * 1e-9 * y.abs().max(1.0)).round() / k
}

/// Tipo de insumo del APU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TipoInsumo {
    ManoObra,
    Material,
    Equipo,
    Maquinaria,
    GastosVarios,
}

impl TipoInsumo {
    pub fn desde_str(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "MO" | "MANO DE OBRA" => Some(Self::ManoObra),
            "MAT" | "MATERIAL" | "MATERIALES" => Some(Self::Material),
            "EQ" | "EQUIPO" | "EQUIPOS" | "EQUIPOS Y HERRAMIENTAS" => Some(Self::Equipo),
            "MAQ" | "MAQUINARIA" | "MAQUINARIAS" => Some(Self::Maquinaria),
            "GV" | "GASTOS VARIOS" => Some(Self::GastosVarios),
            _ => None,
        }
    }
}

/// Línea de insumo: por cuadrilla (h-h, h-m), por cantidad fija o en `%mo`.
#[derive(Clone, Debug)]
pub struct Insumo {
    pub tipo: TipoInsumo,
    pub descripcion: String,
    pub unidad: String,
    pub cuadrilla: Option<f64>,
    /// Cantidad fija o porcentaje (si `unidad` es `%mo`).
    pub cantidad: Option<f64>,
    pub precio: f64,
    /// Índice Unificado (INEI) para la fórmula polinómica.
    pub iu: String,
    /// Para líneas en `%`: sección base (por defecto, lo anterior de su sección o la sección previa).
    pub base_pct: Option<TipoInsumo>,
}

#[derive(Clone, Debug)]
pub struct PartidaApu {
    pub codigo: String,
    pub descripcion: String,
    pub unidad: String,
    /// Unidades por día.
    pub rendimiento: f64,
    pub insumos: Vec<Insumo>,
}

/// Línea calculada.
#[derive(Clone, Debug)]
pub struct LineaApu {
    pub tipo: TipoInsumo,
    pub descripcion: String,
    pub unidad: String,
    pub cantidad: f64,
    pub precio: f64,
    pub parcial: f64,
    pub iu: String,
}

#[derive(Clone, Debug)]
pub struct ApuCalculado {
    pub codigo: String,
    pub lineas: Vec<LineaApu>,
    /// Subtotales: mano de obra, materiales, equipos, maquinaria, gastos varios.
    pub subtotales: [f64; 5],
    pub total: f64,
}

fn r2(x: f64) -> f64 {
    redondear_excel(x, 2)
}

/// Calcula un APU con la jornada en horas (8 en PRESIO).
pub fn calcular_apu(p: &PartidaApu, jornada_h: f64) -> Result<ApuCalculado, String> {
    if !(p.rendimiento > 0.0) && p.insumos.iter().any(|i| i.cuadrilla.is_some()) {
        return Err(format!("APU {}: rendimiento inválido", p.codigo));
    }
    let es_mo = |i: &Insumo| i.unidad.trim().eq_ignore_ascii_case("%mo");
    let es_pct = |i: &Insumo| i.unidad.trim() == "%";
    // mano de obra primero (base de las líneas %mo)
    let mut mo = 0.0;
    for i in p.insumos.iter().filter(|i| i.tipo == TipoInsumo::ManoObra && !es_mo(i) && !es_pct(i)) {
        mo += r2(cantidad_de(p, i, jornada_h)? * i.precio);
    }
    let mo = r2(mo);
    let mut lineas = Vec::new();
    // por sección, en orden; `%` toma la suma sin redondear de lo anterior de su sección
    let mut sin_redondeo = [0.0f64; 5];
    let (mut actual, mut anterior): (Option<TipoInsumo>, Option<TipoInsumo>) = (None, None);
    for i in &p.insumos {
        if actual != Some(i.tipo) {
            anterior = actual;
            actual = Some(i.tipo);
        }
        let t = i.tipo as usize;
        let (cant, precio) = if es_mo(i) {
            (i.cantidad.unwrap_or(0.0), mo)
        } else if es_pct(i) {
            let base = match i.base_pct {
                Some(TipoInsumo::ManoObra) => mo,
                Some(q) => sin_redondeo[q as usize],
                None if sin_redondeo[t] != 0.0 => sin_redondeo[t],
                None => anterior.map_or(0.0, |q| sin_redondeo[q as usize]),
            };
            (i.cantidad.unwrap_or(0.0), base)
        } else {
            (cantidad_de(p, i, jornada_h)?, i.precio)
        };
        let exacto = if es_mo(i) || es_pct(i) { cant / 100.0 * precio } else { cant * precio };
        sin_redondeo[t] += exacto;
        lineas.push(LineaApu { tipo: i.tipo, descripcion: i.descripcion.clone(), unidad: i.unidad.clone(), cantidad: cant, precio,
            parcial: r2(exacto), iu: i.iu.clone() });
    }
    let mut sub = [0.0; 5];
    for l in &lineas {
        sub[l.tipo as usize] += l.parcial;
    }
    let sub = sub.map(r2);
    Ok(ApuCalculado { codigo: p.codigo.clone(), total: r2(sub.iter().sum()), subtotales: sub, lineas })
}

/// Cantidad de una línea: por cuadrilla (cuadrilla · jornada / rendimiento) o fija.
fn cantidad_de(p: &PartidaApu, i: &Insumo, jornada_h: f64) -> Result<f64, String> {
    match (i.cuadrilla, i.cantidad) {
        (Some(c), _) => Ok(r2(c * jornada_h / p.rendimiento)),
        (None, Some(q)) => Ok(q),
        (None, None) => Err(format!("APU {}: «{}» sin cuadrilla ni cantidad", p.codigo, i.descripcion)),
    }
}

// ---------------------------------------------------------------- fórmula polinómica

/// Elemento de la fórmula: índice unificado, su parcial (S/) y descripción.
#[derive(Clone, Debug)]
pub struct ElementoFp {
    pub iu: String,
    pub parcial: f64,
    pub descripcion: String,
}

/// Monomio de la fórmula.
#[derive(Clone, Debug, PartialEq)]
pub struct Monomio {
    pub simbolo: String,
    pub coeficiente: f64,
    /// (índice de la agrupación preliminar, incidencia del grupo, fracción dentro del monomio).
    pub grupos: Vec<(String, f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct FormulaPolinomica {
    pub total: f64,
    /// Incidencia de cada índice (redondeada a 3 decimales), en orden de mayor a menor.
    pub incidencias: Vec<(String, f64)>,
    /// Incidencia de cada grupo preliminar.
    pub preliminar: BTreeMap<String, f64>,
    pub monomios: Vec<Monomio>,
    pub avisos: Vec<String>,
}

/// Fórmula polinómica: `similar` agrupa índices (preliminar) y `monomio` asigna cada grupo a un
/// símbolo. Índices sin entrada forman su propio grupo y su propio monomio.
pub fn formula_polinomica(
    elementos: &[ElementoFp], similar: &BTreeMap<String, String>, monomio: &BTreeMap<String, String>,
) -> FormulaPolinomica {
    let mut por_iu: BTreeMap<String, f64> = BTreeMap::new();
    for e in elementos {
        *por_iu.entry(e.iu.clone()).or_default() += e.parcial;
    }
    let total: f64 = por_iu.values().sum();
    let mut incid: Vec<(String, f64)> = por_iu.iter().map(|(k, v)| (k.clone(), redondear(v / total, 3))).collect();
    // residuo del redondeo al índice de mayor parcial (como PRESIO)
    let residuo = redondear(1.0 - incid.iter().map(|x| x.1).sum::<f64>(), 3);
    if residuo != 0.0 {
        let mayor = por_iu.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|x| x.0.clone()).unwrap_or_default();
        if let Some(x) = incid.iter_mut().find(|x| x.0 == mayor) {
            x.1 = redondear(x.1 + residuo, 3);
        }
    }
    incid.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    let mut preliminar: BTreeMap<String, f64> = BTreeMap::new();
    for (iu, x) in &incid {
        let g = similar.get(iu).cloned().unwrap_or_else(|| iu.clone());
        *preliminar.entry(g).or_default() += x;
    }
    let mut mon: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
    for (g, x) in &preliminar {
        let s = monomio.get(g).cloned().unwrap_or_else(|| g.clone());
        mon.entry(s).or_default().push((g.clone(), redondear(*x, 3)));
    }
    let mut monomios: Vec<Monomio> = mon
        .into_iter()
        .map(|(s, gs)| {
            let c = redondear(gs.iter().map(|g| g.1).sum(), 3);
            let grupos = gs.into_iter().map(|(g, x)| (g, x, if c > 0.0 { redondear(x / c, 4) } else { 0.0 })).collect();
            Monomio { simbolo: s, coeficiente: c, grupos }
        })
        .collect();
    monomios.sort_by(|a, b| b.coeficiente.partial_cmp(&a.coeficiente).unwrap());
    let mut avisos = Vec::new();
    let suma = redondear(monomios.iter().map(|m| m.coeficiente).sum(), 3);
    if (suma - 1.0).abs() > 1e-9 {
        avisos.push(format!("Los coeficientes suman {suma:.3}, no 1,000: ajuste la agrupación"));
    }
    if monomios.len() > 8 {
        avisos.push(format!("{} monomios: el DS 011-79-VC admite como máximo 8", monomios.len()));
    }
    for m in monomios.iter().filter(|m| m.coeficiente < 0.05) {
        avisos.push(format!("Monomio {} con coeficiente {:.3} < 0,050 (DS 011-79-VC): agrúpelo", m.simbolo, m.coeficiente));
    }
    FormulaPolinomica { total, incidencias: incid, preliminar, monomios, avisos }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn apu_con_cuadrilla_y_porcentaje() {
        // partida sintética: rendimiento 3,5 und/día
        let i = |t, d: &str, u: &str, c: Option<f64>, q: Option<f64>, p: f64| Insumo {
            tipo: t, descripcion: d.into(), unidad: u.into(), cuadrilla: c, cantidad: q, precio: p, iu: "47".into(), base_pct: None,
        };
        let p = PartidaApu {
            codigo: "X".into(),
            descripcion: "x".into(),
            unidad: "und".into(),
            rendimiento: 3.5,
            insumos: vec![
                i(TipoInsumo::ManoObra, "Ingeniero", "h-h", Some(0.25), None, 53.22),
                i(TipoInsumo::ManoObra, "Peón", "h-h", Some(1.0), None, 18.68),
                i(TipoInsumo::Material, "Material varios", "%mo", None, Some(5.0), 0.0),
                i(TipoInsumo::Material, "Cemento", "bls", None, Some(0.5), 29.0),
                i(TipoInsumo::Equipo, "Herramientas", "%mo", None, Some(5.0), 0.0),
            ],
        };
        let a = calcular_apu(&p, 8.0).unwrap();
        // 0,25·8/3,5 = 0,57 -> 30,34; 1·8/3,5 = 2,29 -> 42,78; MO 73,12
        assert_eq!(a.lineas[0].cantidad, 0.57);
        assert_eq!(a.lineas[0].parcial, 30.34);
        assert_eq!(a.subtotales[0], 73.12);
        assert_eq!(a.lineas[2].parcial, 3.66); // 5 % de 73,12
        assert_eq!(a.subtotales[1], 3.66 + 14.5);
        assert_eq!(a.total, 73.12 + 18.16 + 3.66);
        assert_eq!(redondear_excel(0.125, 2), 0.13);
        assert_eq!(redondear_excel(2.675, 2), 2.68);
    }

    #[test]
    fn formula_agrupa_y_suma_uno() {
        let e = |iu: &str, p: f64| ElementoFp { iu: iu.into(), parcial: p, descripcion: String::new() };
        let el = [e("47", 470.0), e("49", 120.0), e("48", 30.0), e("30", 300.0), e("43", 50.0), e("39", 30.0)];
        let sim: BTreeMap<String, String> = [("48", "49")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        let mon: BTreeMap<String, String> = [("47", "J"), ("49", "E"), ("30", "D"), ("43", "D"), ("39", "GU")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let f = formula_polinomica(&el, &sim, &mon);
        let c = |s: &str| f.monomios.iter().find(|m| m.simbolo == s).unwrap().coeficiente;
        assert_eq!(f.preliminar["49"], 0.15);
        assert_eq!((c("J"), c("E"), c("D"), c("GU")), (0.47, 0.15, 0.35, 0.03));
        assert!(f.avisos.iter().any(|a| a.contains("GU")), "GU < 5 %: {:?}", f.avisos);
    }
}
