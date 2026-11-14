//! PresuCraft: presupuesto por unidades constructivas. Port de `presucraft/presupuesto.py`.

use std::collections::{BTreeMap, HashMap};

use crate::catalogos::leer_csv;

#[derive(Clone, Debug, Default)]
pub struct Suministro {
    pub codigo: String,
    pub descripcion: String,
    pub unidad: String,
    pub precio: Option<f64>,
    pub peso_kg: Option<f64>,
    pub fuente: String,
}

#[derive(Clone, Debug, Default)]
pub struct ComponenteUc {
    pub uc: String,
    pub suministro: String,
    pub cantidad: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Montaje {
    pub uc: String,
    pub descripcion: String,
    pub unidad: String,
    pub precio: Option<f64>,
    pub fuente: String,
}

#[derive(Clone, Debug, Default)]
pub struct ItemMetrado {
    pub rubro: String,
    pub uc: String,
    pub cantidad: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Presupuesto {
    pub nombre: String,
    pub suministros: Vec<Suministro>,
    pub unidades: Vec<ComponenteUc>,
    pub montaje: Vec<Montaje>,
    pub metrado: Vec<ItemMetrado>,
    pub flete_por_kg: f64,
    pub gastos_generales_pct: f64,
    pub utilidad_pct: f64,
    pub igv_pct: f64,
    pub moneda: String,
    pub fuentes: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilaResumen {
    pub rubro: String,
    pub suministro: f64,
    pub montaje: f64,
    pub transporte: f64,
    pub costo_directo: f64,
    pub gastos_generales: f64,
    pub utilidad: f64,
    pub subtotal: f64,
    pub igv: f64,
    pub total: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilaSuministro {
    pub rubro: String,
    pub codigo: String,
    pub descripcion: String,
    pub unidad: String,
    pub cantidad: f64,
    pub precio: Option<f64>,
    pub parcial: f64,
    pub peso_kg: f64,
    pub transporte: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilaMontaje {
    pub rubro: String,
    pub uc: String,
    pub partida: String,
    pub unidad: String,
    pub cantidad: f64,
    pub precio: Option<f64>,
    pub parcial: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilaUc {
    pub rubro: String,
    pub uc: String,
    pub cantidad: f64,
    pub suministro_unit: f64,
    pub montaje_unit: f64,
    pub peso_unit: f64,
    pub transporte_unit: f64,
    pub total: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ResultadoPresupuesto {
    pub resumen: Vec<FilaResumen>,
    pub suministro: Vec<FilaSuministro>,
    pub montaje: Vec<FilaMontaje>,
    pub planilla_uc: Vec<FilaUc>,
    pub avisos: Vec<String>,
}

fn rd(x: f64, d: i32) -> f64 {
    crate::numerico::redondear(x, d as usize)
}

fn pendiente(f: &str) -> bool {
    let u = f.to_uppercase();
    f.is_empty() || u.contains("PENDIENTE") || u.contains("EJEMPLO")
}

/// Calcula el presupuesto por rubro y total.
pub fn calcular(p: &Presupuesto) -> Result<ResultadoPresupuesto, String> {
    let mut avisos = Vec::new();
    let mut sm: HashMap<&str, &Suministro> = HashMap::new();
    let mut rep = std::collections::BTreeSet::new();
    for s in &p.suministros {
        if sm.insert(&s.codigo, s).is_some() {
            rep.insert(s.codigo.clone());
        }
    }
    if !rep.is_empty() {
        return Err(format!("Suministros repetidos: {rep:?}"));
    }
    let me: Vec<&ItemMetrado> = p.metrado.iter().filter(|m| m.cantidad.unwrap_or(0.0) != 0.0).collect();
    let ucs_def: std::collections::BTreeSet<&str> =
        p.unidades.iter().map(|u| u.uc.as_str()).chain(p.montaje.iter().map(|m| m.uc.as_str())).collect();
    let sin_def: std::collections::BTreeSet<&str> =
        me.iter().map(|m| m.uc.as_str()).filter(|u| !ucs_def.contains(u)).collect();
    if !sin_def.is_empty() {
        avisos.push(format!(
            "UC del metrado sin composición ni montaje (costo 0): {}",
            sin_def.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let falta: std::collections::BTreeSet<&str> =
        p.unidades.iter().map(|u| u.suministro.as_str()).filter(|s| !sm.contains_key(s)).collect();
    if !falta.is_empty() {
        return Err(format!("Suministros usados en unidades.csv y no definidos: {falta:?}"));
    }
    let usados: std::collections::BTreeSet<&str> = p.unidades.iter().map(|u| u.suministro.as_str()).collect();
    let sin_precio: Vec<&str> = usados.iter().copied().filter(|s| sm[s].precio.is_none()).collect();
    if !sin_precio.is_empty() {
        avisos.push(format!("Suministros sin precio (costo 0): {}", sin_precio.join(", ")));
    }
    for (nombre, n) in [
        ("suministros", p.suministros.iter().filter(|s| pendiente(&s.fuente)).count()),
        ("montaje", p.montaje.iter().filter(|m| pendiente(&m.fuente)).count()),
    ] {
        if n > 0 {
            avisos.push(format!("PENDIENTE_REVISION: precios de {nombre} sin fuente verificada ({n} filas)"));
        }
    }
    for (k, v) in [
        ("flete_por_kg", p.flete_por_kg),
        ("gastos_generales_pct", p.gastos_generales_pct),
        ("utilidad_pct", p.utilidad_pct),
        ("igv_pct", p.igv_pct),
    ] {
        let f = p.fuentes.get(k).cloned().unwrap_or_default();
        if v != 0.0 && pendiente(&f) {
            avisos.push(format!("PENDIENTE_REVISION: '{k}' -> {}", if f.is_empty() { "sin fuente" } else { &f }));
        }
    }

    // materiales por (rubro, suministro), ordenados como groupby de pandas
    let mut mat: BTreeMap<(String, String), f64> = BTreeMap::new();
    for m in &me {
        for u in p.unidades.iter().filter(|u| u.uc == m.uc) {
            *mat.entry((m.rubro.clone(), u.suministro.clone())).or_default() +=
                m.cantidad.unwrap_or(0.0) * u.cantidad.unwrap_or(f64::NAN);
        }
    }
    let suministro: Vec<FilaSuministro> = mat
        .iter()
        .map(|((rubro, cod), cant)| {
            let s = sm[cod.as_str()];
            let peso = cant * s.peso_kg.unwrap_or(0.0);
            FilaSuministro {
                rubro: rubro.clone(),
                codigo: cod.clone(),
                descripcion: s.descripcion.clone(),
                unidad: s.unidad.clone(),
                cantidad: *cant,
                precio: s.precio,
                parcial: cant * s.precio.unwrap_or(0.0),
                peso_kg: peso,
                transporte: peso * p.flete_por_kg,
            }
        })
        .collect();
    let mut montaje = Vec::new();
    for m in &me {
        for mo in p.montaje.iter().filter(|x| x.uc == m.uc) {
            let q = m.cantidad.unwrap_or(0.0);
            montaje.push(FilaMontaje {
                rubro: m.rubro.clone(),
                uc: m.uc.clone(),
                partida: mo.descripcion.clone(),
                unidad: mo.unidad.clone(),
                cantidad: q,
                precio: mo.precio,
                parcial: q * mo.precio.unwrap_or(0.0),
            });
        }
    }
    let planilla_uc = me
        .iter()
        .map(|m| {
            let (mut s, mut w) = (0.0, 0.0);
            for u in p.unidades.iter().filter(|u| u.uc == m.uc) {
                let q = u.cantidad.unwrap_or(0.0);
                let x = sm[u.suministro.as_str()];
                s += q * x.precio.unwrap_or(0.0);
                w += q * x.peso_kg.unwrap_or(0.0);
            }
            let mo: f64 = p.montaje.iter().filter(|x| x.uc == m.uc).map(|x| x.precio.unwrap_or(0.0)).sum();
            let q = m.cantidad.unwrap_or(0.0);
            FilaUc {
                rubro: m.rubro.clone(),
                uc: m.uc.clone(),
                cantidad: q,
                suministro_unit: rd(s, 2),
                montaje_unit: rd(mo, 2),
                peso_unit: rd(w, 3),
                transporte_unit: rd(w * p.flete_por_kg, 2),
                total: rd(q * (s + mo + w * p.flete_por_kg), 2),
            }
        })
        .collect();

    let mut rubros: Vec<String> = Vec::new();
    for m in &me {
        if !rubros.contains(&m.rubro) {
            rubros.push(m.rubro.clone());
        }
    }
    let mut resumen = Vec::new();
    for r in rubros.iter().map(|s| s.as_str()).chain(["TOTAL"]) {
        let sel = |x: &str| r == "TOTAL" || x == r;
        let s: f64 = suministro.iter().filter(|x| sel(&x.rubro)).map(|x| x.parcial).sum();
        let t: f64 = suministro.iter().filter(|x| sel(&x.rubro)).map(|x| x.transporte).sum();
        let m: f64 = montaje.iter().filter(|x| sel(&x.rubro)).map(|x| x.parcial).sum();
        let cd = s + m + t;
        let (gg, ut) = (cd * p.gastos_generales_pct / 100.0, cd * p.utilidad_pct / 100.0);
        let sub = cd + gg + ut;
        let igv = sub * p.igv_pct / 100.0;
        resumen.push(FilaResumen {
            rubro: r.into(),
            suministro: rd(s, 2),
            montaje: rd(m, 2),
            transporte: rd(t, 2),
            costo_directo: rd(cd, 2),
            gastos_generales: rd(gg, 2),
            utilidad: rd(ut, 2),
            subtotal: rd(sub, 2),
            igv: rd(igv, 2),
            total: rd(sub + igv, 2),
        });
    }
    let suministro = suministro
        .into_iter()
        .map(|mut x| {
            x.cantidad = rd(x.cantidad, 3);
            x.parcial = rd(x.parcial, 3);
            x.peso_kg = rd(x.peso_kg, 3);
            x.transporte = rd(x.transporte, 3);
            x
        })
        .collect();
    let montaje = montaje
        .into_iter()
        .map(|mut x| {
            x.parcial = rd(x.parcial, 2);
            x
        })
        .collect();
    Ok(ResultadoPresupuesto { resumen, suministro, montaje, planilla_uc, avisos })
}

/// Tablas desde el texto de los cuatro CSV.
pub fn leer_tablas(
    suministros: &str, unidades: &str, montaje: &str, metrado: &str,
) -> Result<(Vec<Suministro>, Vec<ComponenteUc>, Vec<Montaje>, Vec<ItemMetrado>), String> {
    let num = |f: &BTreeMap<String, String>, k: &str| {
        f.get(k).and_then(|s| s.replace(',', ".").parse::<f64>().ok()).filter(|v| v.is_finite())
    };
    let txt = |f: &BTreeMap<String, String>, k: &str| f.get(k).cloned().unwrap_or_default();
    Ok((
        leer_csv(suministros)?
            .iter()
            .map(|f| Suministro {
                codigo: txt(f, "codigo"),
                descripcion: txt(f, "descripcion"),
                unidad: txt(f, "unidad"),
                precio: num(f, "precio"),
                peso_kg: num(f, "peso_kg"),
                fuente: txt(f, "fuente"),
            })
            .collect(),
        leer_csv(unidades)?
            .iter()
            .map(|f| ComponenteUc { uc: txt(f, "uc"), suministro: txt(f, "suministro"), cantidad: num(f, "cantidad") })
            .collect(),
        leer_csv(montaje)?
            .iter()
            .map(|f| Montaje {
                uc: txt(f, "uc"),
                descripcion: txt(f, "descripcion"),
                unidad: txt(f, "unidad"),
                precio: num(f, "precio"),
                fuente: txt(f, "fuente"),
            })
            .collect(),
        leer_csv(metrado)?
            .iter()
            .map(|f| ItemMetrado { rubro: txt(f, "rubro"), uc: txt(f, "uc"), cantidad: num(f, "cantidad") })
            .collect(),
    ))
}

/// Lee `presupuesto.json` (parámetros + cuatro CSV con rutas relativas).
pub fn cargar(ruta: &std::path::Path) -> Result<Presupuesto, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let cfg: serde_json::Value = serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let base = ruta.parent().unwrap_or(std::path::Path::new("."));
    let leer = |k: &str| -> Result<String, String> {
        let r = cfg.get(k).and_then(|v| v.as_str()).ok_or(format!("presupuesto: falta '{k}'"))?;
        std::fs::read(base.join(r)).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("{r}: {e}"))
    };
    let (suministros, unidades, montaje, metrado) =
        leer_tablas(&leer("suministros")?, &leer("unidades")?, &leer("montaje")?, &leer("metrado")?)?;
    let f = |k: &str| cfg.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    Ok(Presupuesto {
        nombre: cfg.get("nombre").and_then(|v| v.as_str()).map(String::from)
            .unwrap_or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
        suministros,
        unidades,
        montaje,
        metrado,
        flete_por_kg: f("flete_por_kg"),
        gastos_generales_pct: f("gastos_generales_pct"),
        utilidad_pct: f("utilidad_pct"),
        igv_pct: f("igv_pct"),
        moneda: cfg.get("moneda").and_then(|v| v.as_str()).unwrap_or("S/").into(),
        fuentes: crate::red::fuentes_json(&cfg),
    })
}
