//! Proyectos: `proyecto.json` de LineCraft y documento `.sel` de la suite.
//!
//! Port de `linecraft/proyecto.py` (carga) y `documento.py` (formato `.sel`). El `.sel` se lee y
//! escribe como JSON genérico conservando claves desconocidas y su orden, para que un archivo de
//! Python abierto y guardado en Rust (y viceversa) no pierda datos.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::catalogos::leer_csv;
use crate::conductor::{Conductor, Hipotesis, G};
use crate::entrada::{leer, leer_perfil_tabular, leer_polilinea, leer_csv_texto, limpiar, Punto};
use crate::geo::{perfil_longitudinal, Perfil};
use crate::linea::{EstructuraManual, Proyecto, Tipo};
use crate::mecanica::Modelo;
use crate::tin::Tin;

fn curva(t: &str) -> Vec<f64> {
    t.split(';').filter(|v| !v.trim().is_empty()).filter_map(|v| v.trim().replace(',', ".").parse().ok()).collect()
}

/// Catálogo CSV de conductores (unidades de catálogo → SI).
pub fn leer_catalogo_conductores(texto: &str) -> Result<BTreeMap<String, Conductor>, String> {
    let mut out = BTreeMap::new();
    for f in leer_csv(texto)? {
        let n = |k: &str| f.get(k).and_then(|s| s.parse::<f64>().ok()).ok_or(format!("catálogo: falta {k}"));
        let nombre = f.get("nombre").cloned().unwrap_or_default().trim().to_string();
        out.insert(
            nombre.clone(),
            Conductor {
                nombre,
                seccion: n("seccion_mm2")? * 1e-6,
                diametro: n("diametro_mm")? * 1e-3,
                peso: n("masa_kg_m")? * G,
                carga_rotura: n("carga_rotura_kN")? * 1e3,
                modulo_elasticidad: n("modulo_elasticidad_MPa")? * 1e6,
                coef_dilatacion: n("coef_dilatacion_1_C")?,
                curva_inicial: curva(f.get("curva_inicial").map(|s| s.as_str()).unwrap_or("")),
                curva_creep: curva(f.get("curva_creep").map(|s| s.as_str()).unwrap_or("")),
            },
        );
    }
    Ok(out)
}

fn leer_texto(ruta: &Path) -> Result<String, String> {
    std::fs::read(ruta).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("{}: {e}", ruta.display()))
}

/// Perfil guardado por TopoCraft en Excel (hoja `Perfil`); el eje son los puntos de tipo `V`.
pub fn leer_perfil_excel(ruta: &Path) -> Result<(Perfil, Vec<[f64; 2]>), String> {
    use calamine::{Data, Reader};
    let mut wb = calamine::open_workbook_auto(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let r = wb.worksheet_range("Perfil").map_err(|e| e.to_string())?;
    let filas: Vec<Vec<Data>> = r.rows().map(|f| f.to_vec()).collect();
    let enc: Vec<String> = filas.first().ok_or("hoja Perfil vacía")?.iter().map(|c| c.to_string()).collect();
    let col = |k: &str| enc.iter().position(|c| c == k).ok_or(format!("falta columna {k}"));
    let (cs, ce, cn, cz, ct) = (col("Distancia (m)")?, col("Este (m)")?, col("Norte (m)")?, col("Cota terreno (m)")?, col("Tipo")?);
    let num = |d: &Data| match d {
        Data::Float(x) => *x,
        Data::Int(x) => *x as f64,
        _ => f64::NAN,
    };
    let mut p = Perfil::default();
    for f in &filas[1..] {
        p.progresiva.push(num(&f[cs]));
        p.este.push(num(&f[ce]));
        p.norte.push(num(&f[cn]));
        p.cota.push(num(&f[cz]));
        p.tipo.push(f[ct].to_string().chars().next().unwrap_or('E'));
    }
    let eje = (0..p.tipo.len()).filter(|&i| p.tipo[i] == 'V').map(|i| [p.este[i], p.norte[i]]).collect();
    Ok((p, eje))
}

fn perfil_desde(cfg: &Value, base: &Path) -> Result<(Perfil, Vec<[f64; 2]>), String> {
    let s = |k: &str| cfg.get(k).and_then(|v| v.as_str()).map(|v| base.join(v));
    if let Some(t) = s("tabla") {
        return leer_perfil_tabular(&t);
    }
    if let Some(x) = s("excel") {
        return leer_perfil_excel(&x);
    }
    let pts = leer(&s("puntos").ok_or("perfil: falta 'puntos'")?)?;
    let (limpio, _) = limpiar(&pts, 0.001, 0.001, false)?;
    let xyz: Vec<[f64; 3]> = limpio.iter().map(|p| [p.este, p.norte, p.cota]).collect();
    let tin = Tin::nuevo(&xyz, cfg.get("max_lado").and_then(|v| v.as_f64()))?;
    let ruta_eje = s("eje").ok_or("perfil: falta 'eje'")?;
    let eje = if ruta_eje.extension().is_some_and(|e| e.eq_ignore_ascii_case("dxf")) {
        leer_polilinea(&ruta_eje, None)?
    } else {
        leer_csv_texto(&leer_texto(&ruta_eje)?, &ruta_eje.display().to_string(), &["este", "norte"])?
            .iter()
            .map(|p: &Punto| [p.este, p.norte])
            .collect()
    };
    let paso = cfg.get("paso").and_then(|v| v.as_f64()).unwrap_or(5.0);
    Ok((perfil_longitudinal(&tin, &eje, paso)?, eje))
}

fn estructuras_csv(texto: &str) -> Result<Vec<EstructuraManual>, String> {
    leer_csv(texto)?
        .iter()
        .map(|f| {
            let t = |k: &str| f.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
            Ok(EstructuraManual {
                progresiva: t("progresiva_m").parse().map_err(|_| "estructuras: progresiva_m inválida".to_string())?,
                tipo: if t("tipo").is_empty() { None } else { Some(Tipo::desde_str(&t("tipo"))?) },
                altura_amarre: t("altura_amarre_m").parse().ok(),
                armado: t("armado"),
                poste: t("poste"),
            })
        })
        .collect()
}

/// Lee un `proyecto.json` de LineCraft (rutas relativas al archivo).
pub fn cargar_proyecto_json(ruta: &Path) -> Result<Proyecto, String> {
    let cfg: Value = serde_json::from_str(&leer_texto(ruta)?).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let base = ruta.parent().map(PathBuf::from).unwrap_or_default();
    let g = |k: &str| cfg.get(k).ok_or(format!("proyecto: falta '{k}'"));
    let f = |k: &str| g(k).and_then(|v| v.as_f64().ok_or(format!("proyecto: '{k}' no numérico")));
    let s = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from).ok_or(format!("proyecto: falta '{k}'"));
    let cond_cfg = g("conductor")?;
    let cat = leer_catalogo_conductores(&leer_texto(&base.join(s(cond_cfg, "catalogo")?))?)?;
    let nombre_c = s(cond_cfg, "nombre")?;
    let conductor = cat
        .get(&nombre_c)
        .cloned()
        .ok_or(format!("Conductor '{nombre_c}' no está en el catálogo ({:?})", cat.keys().collect::<Vec<_>>()))?;
    let num = |h: &Value, k: &str, d: f64| h.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let hipotesis = g("hipotesis")?
        .as_array()
        .ok_or("hipotesis debe ser una lista")?
        .iter()
        .map(|h| {
            Ok(Hipotesis {
                nombre: s(h, "nombre")?,
                temperatura: num(h, "temperatura_C", f64::NAN),
                presion_viento: num(h, "presion_viento_Pa", 0.0),
                espesor_hielo: num(h, "espesor_hielo_mm", 0.0) / 1000.0,
                densidad_hielo: num(h, "densidad_hielo_kg_m3", 0.0),
                altitud_min: num(h, "altitud_min_m", f64::NEG_INFINITY),
                altitud_max: num(h, "altitud_max_m", f64::INFINITY),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let (perfil, eje) = perfil_desde(g("perfil")?, &base)?;
    let manual = match cfg.get("estructuras").and_then(|v| v.as_str()) {
        Some(r) if !r.is_empty() => Some(estructuras_csv(&leer_texto(&base.join(r))?)?),
        _ => None,
    };
    let ini = g("condicion_inicial")?;
    let vano_maximo = f("vano_maximo_m")?;
    let obj = |k: &str| cfg.get(k).and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let p = Proyecto {
        nombre: cfg.get("nombre").and_then(|v| v.as_str()).map(String::from)
            .unwrap_or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
        conductor,
        n_conductores: cfg.get("n_conductores").and_then(|v| v.as_u64()).unwrap_or(3) as usize,
        modelo: if cfg.get("modelo").and_then(|v| v.as_str()) == Some("parabola") { Modelo::Parabola } else { Modelo::Catenaria },
        hipotesis,
        hip_inicial: s(ini, "hipotesis")?,
        tiro_inicial_pct: ini.get("tiro_pct_rotura").and_then(|v| v.as_f64()).ok_or("falta tiro_pct_rotura")?,
        hip_flecha_maxima: s(&cfg, "hipotesis_flecha_maxima")?,
        tiro_maximo_pct: f("tiro_maximo_pct_rotura")?,
        distancia_minima_terreno: f("distancia_minima_terreno_m")?,
        altura_amarre: f("altura_amarre_m")?,
        vano_maximo,
        vano_minimo: cfg.get("vano_minimo_m").and_then(|v| v.as_f64()).unwrap_or(0.2 * vano_maximo),
        angulo_retenida: cfg.get("angulo_retenida_deg").and_then(|v| v.as_f64()).unwrap_or(45.0),
        perfil,
        eje,
        estructuras_manual: manual,
        fuentes: obj("fuentes").into_iter().map(|(k, v)| (k, v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect(),
        alertas: obj("alertas").into_iter().filter_map(|(k, v)| v.as_f64().map(|x| (k, x))).collect::<HashMap<_, _>>(),
        condicion_final: cfg.get("condicion_final").and_then(|v| v.as_str()).unwrap_or("creep").into(),
        vanos_independientes: cfg.get("vanos_independientes").and_then(|v| v.as_bool()).unwrap_or(false),
    };
    p.hip(&p.hip_inicial, None)?;
    p.hip(&p.hip_flecha_maxima, None)?;
    Ok(p)
}

// ---------------------------------------------------------------- documento .sel

pub const FORMATO: &str = "suite-electrica";
pub const VERSION: i64 = 1;
pub const EXTENSION: &str = "sel";

/// Documento `.sel` (JSON genérico; se conserva todo lo que no se modifica).
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentoSel {
    pub datos: Map<String, Value>,
}

impl DocumentoSel {
    pub fn abrir(ruta: &Path) -> Result<Self, String> {
        let v: Value = serde_json::from_str(&leer_texto(ruta)?).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let datos = v.as_object().cloned().ok_or(format!("{} no es un proyecto de Suite Eléctrica", ruta.display()))?;
        if datos.get("formato").and_then(|x| x.as_str()) != Some(FORMATO) {
            return Err(format!("{} no es un proyecto de Suite Eléctrica", ruta.display()));
        }
        if datos.get("version").and_then(|x| x.as_i64()).unwrap_or(0) > VERSION {
            return Err("El archivo es de una versión más nueva de la suite".into());
        }
        Ok(DocumentoSel { datos })
    }

    /// Guarda con sangría de 1 espacio y UTF-8 (como Python); agrega la extensión `.sel`.
    pub fn guardar(&self, ruta: &Path) -> Result<PathBuf, String> {
        let ruta = if ruta.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION)) {
            ruta.to_path_buf()
        } else {
            ruta.with_extension(EXTENSION)
        };
        let mut buf = Vec::new();
        let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
        serde::Serialize::serialize(&Value::Object(self.datos.clone()), &mut ser).map_err(|e| e.to_string())?;
        std::fs::write(&ruta, buf).map_err(|e| format!("{}: {e}", ruta.display()))?;
        Ok(ruta)
    }

    /// Puntos topográficos del documento.
    pub fn puntos(&self) -> Vec<Punto> {
        let p = &self.datos["puntos"];
        let cols: Vec<&str> = p["columnas"].as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect()).unwrap_or_default();
        let idx = |k: &str| cols.iter().position(|c| *c == k);
        p["filas"]
            .as_array()
            .map(|filas| {
                filas
                    .iter()
                    .map(|f| {
                        let v = |k: &str| idx(k).and_then(|i| f.get(i)).cloned().unwrap_or(Value::Null);
                        let n = |k: &str| v(k).as_f64().unwrap_or(f64::NAN);
                        let t = |k: &str| match v(k) {
                            Value::String(s) => s,
                            Value::Null => String::new(),
                            otro => otro.to_string(),
                        };
                        Punto { punto: t("punto"), este: n("este"), norte: n("norte"), cota: n("cota"), descripcion: t("descripcion") }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Eje en planta (si hay).
    pub fn eje(&self) -> Option<Vec<[f64; 2]>> {
        self.datos.get("eje")?.as_array().map(|a| {
            a.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect()
        })
    }
}
