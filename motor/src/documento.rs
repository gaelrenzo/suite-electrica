//! Documento de proyecto único (`.sel`): topografía, línea, catálogos y estructuras.
//!
//! Port de `suite_electrica/documento.py` (modelo de la aplicación). El archivo es el mismo
//! JSON (UTF-8, sangría de 1 espacio): un `.sel` de Python se abre en Rust y viceversa.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::alertas::{alertas, Alerta, ConfigAlertas};
use crate::catalogos::{self, Catalogos, FilaMetrado, FilaVerificacion};
use crate::conductor::{Conductor, Hipotesis, G};
use crate::entrada::{self, limpiar, Punto, ReporteLimpieza};
use crate::flechado::{tabla_flechado, FilaFlechado};
use crate::geo::{angulos_eje, perfil_longitudinal, progresivas_eje, Perfil};
use crate::linea::{calcular, EstructuraManual, Proyecto, Resultado, Tipo};
use crate::mecanica::Modelo;
use crate::proyectos::leer_catalogo_conductores;
use crate::tin::{niveles, Curva, Tin};

pub const FORMATO: &str = "suite-electrica";
pub const VERSION: i64 = 1;
pub const EXTENSION: &str = "sel";
pub const FUENTES_EXTRA: [&str; 2] = ["factor_seguridad_poste", "factor_seguridad_retenida"];
pub const FUENTES_ALERTAS: [&str; 6] =
    ["sep_k_tension", "sep_k_flecha", "angulo_salida_min", "angulo_salida_max", "tiro_max_espiga", "eds_max_pct"];
/// Separación mínima entre estructuras al editar (m).
pub const SEPARACION_MIN: f64 = 1.0;
/// Tolerancia para reconocer un vértice del eje (m).
pub const TOL_VERTICE: f64 = 0.01;
pub const COLS_CATALOGO: [(&str, &[&str]); 4] = [
    ("postes", &["codigo", "material", "longitud_m", "carga_nominal_daN", "empotramiento_m", "carga_vertical_max_daN",
        "diam_punta_m", "diam_tierra_m", "mor_MPa", "dist_aplicacion_m", "fuente"]),
    ("armados", &["codigo", "descripcion", "funcion", "angulo_max_deg", "distancia_punta_amarre_m", "separacion_fases_m",
        "aislador", "aisladores_por_fase", "alturas_fases_m", "retenida_toma", "fuente"]),
    ("retenidas", &["codigo", "descripcion", "carga_rotura_kN", "fuente"]),
    ("materiales", &["armado", "material", "unidad", "cantidad", "fuente"]),
];

/// Parámetros topográficos (m).
#[derive(Clone, Debug, PartialEq)]
pub struct ParamTopo {
    pub equidistancia: f64,
    pub mayor_cada: i64,
    pub paso_perfil: f64,
    pub max_lado: Option<f64>,
    pub quitar_atipicos: bool,
}

impl Default for ParamTopo {
    fn default() -> Self {
        ParamTopo { equidistancia: 1.0, mayor_cada: 5, paso_perfil: 20.0, max_lado: None, quitar_atipicos: false }
    }
}

/// Parámetros de diseño de la línea (nombres de campo del `.sel`).
#[derive(Clone, Debug, PartialEq)]
pub struct ParamLinea {
    pub n_conductores: i64,
    pub modelo: String,
    pub hip_inicial: String,
    pub tiro_inicial_pct: f64,
    pub hip_flecha_maxima: String,
    pub tiro_maximo_pct: f64,
    pub distancia_minima_terreno: f64,
    pub altura_amarre: f64,
    pub vano_maximo: f64,
    pub vano_minimo: f64,
    pub angulo_retenida: f64,
    pub condicion_final: String,
    /// `tramo` (vano regulador) o `por vano` (vanos independientes, como REDLIN).
    pub estado_final_vanos: String,
    pub fs_poste: f64,
    pub fs_retenida: f64,
    pub retenida: String,
    pub desperdicio_pct: f64,
    pub tension_kv: f64,
    pub sep_k_tension: f64,
    pub sep_k_flecha: f64,
    pub angulo_salida_min: f64,
    pub angulo_salida_max: f64,
    pub tiro_max_espiga: f64,
    pub eds_max_pct: f64,
}

impl Default for ParamLinea {
    fn default() -> Self {
        ParamLinea {
            n_conductores: 3,
            modelo: "catenaria".into(),
            hip_inicial: "V_EDS".into(),
            tiro_inicial_pct: 18.0,
            hip_flecha_maxima: "IV_TEMP_MAX".into(),
            tiro_maximo_pct: 40.0,
            distancia_minima_terreno: 6.5,
            altura_amarre: 10.6,
            vano_maximo: 120.0,
            vano_minimo: 30.0,
            angulo_retenida: 45.0,
            condicion_final: "creep".into(),
            estado_final_vanos: "tramo".into(),
            fs_poste: 2.0,
            fs_retenida: 2.0,
            retenida: String::new(),
            desperdicio_pct: 0.0,
            tension_kv: 0.0,
            sep_k_tension: 0.0,
            sep_k_flecha: 0.0,
            angulo_salida_min: 0.0,
            angulo_salida_max: 0.0,
            tiro_max_espiga: 0.0,
            eds_max_pct: 0.0,
        }
    }
}

impl ParamLinea {
    fn a_json(&self) -> Value {
        json!({
            "n_conductores": self.n_conductores, "modelo": self.modelo, "hip_inicial": self.hip_inicial,
            "tiro_inicial_pct": self.tiro_inicial_pct, "hip_flecha_maxima": self.hip_flecha_maxima,
            "tiro_maximo_pct": self.tiro_maximo_pct, "distancia_minima_terreno": self.distancia_minima_terreno,
            "altura_amarre": self.altura_amarre, "vano_maximo": self.vano_maximo, "vano_minimo": self.vano_minimo,
            "angulo_retenida": self.angulo_retenida, "condicion_final": self.condicion_final,
            "estado_final_vanos": self.estado_final_vanos, "fs_poste": self.fs_poste, "fs_retenida": self.fs_retenida,
            "retenida": self.retenida, "desperdicio_pct": self.desperdicio_pct, "tension_kV": self.tension_kv,
            "sep_k_tension": self.sep_k_tension, "sep_k_flecha": self.sep_k_flecha,
            "angulo_salida_min": self.angulo_salida_min, "angulo_salida_max": self.angulo_salida_max,
            "tiro_max_espiga": self.tiro_max_espiga, "eds_max_pct": self.eds_max_pct,
        })
    }

    fn desde_json(v: &Value) -> Self {
        let d = ParamLinea::default();
        let f = |k: &str, x: f64| v.get(k).and_then(|y| y.as_f64()).unwrap_or(x);
        let s = |k: &str, x: &str| v.get(k).and_then(|y| y.as_str()).unwrap_or(x).to_string();
        ParamLinea {
            n_conductores: v.get("n_conductores").and_then(|y| y.as_i64()).unwrap_or(d.n_conductores),
            modelo: s("modelo", &d.modelo),
            hip_inicial: s("hip_inicial", &d.hip_inicial),
            tiro_inicial_pct: f("tiro_inicial_pct", d.tiro_inicial_pct),
            hip_flecha_maxima: s("hip_flecha_maxima", &d.hip_flecha_maxima),
            tiro_maximo_pct: f("tiro_maximo_pct", d.tiro_maximo_pct),
            distancia_minima_terreno: f("distancia_minima_terreno", d.distancia_minima_terreno),
            altura_amarre: f("altura_amarre", d.altura_amarre),
            vano_maximo: f("vano_maximo", d.vano_maximo),
            vano_minimo: f("vano_minimo", d.vano_minimo),
            angulo_retenida: f("angulo_retenida", d.angulo_retenida),
            condicion_final: s("condicion_final", &d.condicion_final),
            estado_final_vanos: s("estado_final_vanos", &d.estado_final_vanos),
            fs_poste: f("fs_poste", d.fs_poste),
            fs_retenida: f("fs_retenida", d.fs_retenida),
            retenida: s("retenida", &d.retenida),
            desperdicio_pct: f("desperdicio_pct", d.desperdicio_pct),
            tension_kv: f("tension_kV", d.tension_kv),
            sep_k_tension: f("sep_k_tension", 0.0),
            sep_k_flecha: f("sep_k_flecha", 0.0),
            angulo_salida_min: f("angulo_salida_min", 0.0),
            angulo_salida_max: f("angulo_salida_max", 0.0),
            tiro_max_espiga: f("tiro_max_espiga", 0.0),
            eds_max_pct: f("eds_max_pct", 0.0),
        }
    }

    pub fn config_alertas(&self) -> ConfigAlertas {
        ConfigAlertas {
            fs_poste: self.fs_poste,
            fs_retenida: self.fs_retenida,
            tension_kv: self.tension_kv,
            sep_k_tension: self.sep_k_tension,
            sep_k_flecha: self.sep_k_flecha,
            angulo_salida_min: self.angulo_salida_min,
            angulo_salida_max: self.angulo_salida_max,
            tiro_max_espiga: self.tiro_max_espiga,
            eds_max_pct: self.eds_max_pct,
        }
    }

    /// Valor de una alerta por nombre (para avisos de fuente).
    fn alerta(&self, k: &str) -> f64 {
        match k {
            "sep_k_tension" => self.sep_k_tension,
            "sep_k_flecha" => self.sep_k_flecha,
            "angulo_salida_min" => self.angulo_salida_min,
            "angulo_salida_max" => self.angulo_salida_max,
            "tiro_max_espiga" => self.tiro_max_espiga,
            _ => self.eds_max_pct,
        }
    }
}

/// Hipótesis editable (`altitud_*` `None` = sin límite).
#[derive(Clone, Debug, PartialEq)]
pub struct FilaHipotesis {
    pub nombre: String,
    pub temperatura_c: f64,
    pub presion_viento_pa: f64,
    pub espesor_hielo_mm: f64,
    pub densidad_hielo_kg_m3: f64,
    pub altitud_min_m: Option<f64>,
    pub altitud_max_m: Option<f64>,
}

/// Estructura editable (vacíos = automático).
#[derive(Clone, Debug, PartialEq)]
pub struct FilaEstructura {
    pub progresiva_m: f64,
    /// `""` = según geometría.
    pub tipo: String,
    pub altura_amarre_m: Option<f64>,
    pub armado: String,
    pub poste: String,
    pub retenidas: Option<f64>,
}

impl FilaEstructura {
    pub fn en(s: f64) -> Self {
        FilaEstructura { progresiva_m: s, tipo: String::new(), altura_amarre_m: None, armado: String::new(), poste: String::new(), retenidas: None }
    }
}

/// Tabla de catálogo como texto (columnas fijas de Python).
#[derive(Clone, Debug, PartialEq)]
pub struct TablaCatalogo {
    pub columnas: Vec<String>,
    pub filas: Vec<Vec<String>>,
}

impl TablaCatalogo {
    fn desde_csv(nombre: &str, texto: &str) -> Result<Self, String> {
        let cols = columnas_catalogo(nombre);
        let filas = catalogos::leer_csv(texto)?
            .iter()
            .map(|f| cols.iter().map(|c| f.get(*c).cloned().unwrap_or_default()).collect())
            .collect();
        Ok(TablaCatalogo { columnas: cols.iter().map(|c| c.to_string()).collect(), filas })
    }

    fn a_csv(&self) -> String {
        let mut w = csv::Writer::from_writer(Vec::new());
        let _ = w.write_record(&self.columnas);
        for f in &self.filas {
            let _ = w.write_record(f);
        }
        String::from_utf8(w.into_inner().unwrap_or_default()).unwrap_or_default()
    }
}

fn columnas_catalogo(nombre: &str) -> &'static [&'static str] {
    COLS_CATALOGO.iter().find(|c| c.0 == nombre).map(|c| c.1).unwrap_or(&[])
}

/// Topografía calculada (en caché hasta el próximo cambio).
pub struct Topografia {
    pub puntos: Vec<Punto>,
    pub reporte: ReporteLimpieza,
    pub tin: Tin,
    pub curvas: Vec<Curva>,
    pub perfil: Option<Perfil>,
}

/// Cálculo completo de la línea (lo que muestran la interfaz y las salidas).
#[derive(Clone, Debug)]
pub struct Calculo {
    pub proyecto: Proyecto,
    pub resultado: Resultado,
    pub verificacion: Vec<FilaVerificacion>,
    pub metrado: Vec<FilaMetrado>,
    pub alertas: Vec<Alerta>,
    pub flechado: Vec<FilaFlechado>,
}

impl Calculo {
    pub fn extras(&self) -> crate::salidas::Extras<'_> {
        crate::salidas::Extras {
            verificacion: Some(&self.verificacion),
            metrado: Some(&self.metrado),
            flechado: &self.flechado,
            alertas: Some(&self.alertas),
        }
    }

    pub fn cumple(&self) -> bool {
        crate::salidas::cumple(&self.resultado, Some(&self.verificacion), Some(&self.alertas))
    }
}

/// Proyecto completo editable (modelo de la aplicación).
#[derive(Clone)]
pub struct Documento {
    pub nombre: String,
    pub autor: String,
    pub zona_utm: String,
    pub puntos: Vec<Punto>,
    pub eje: Option<Vec<[f64; 2]>>,
    pub topo: ParamTopo,
    pub linea: ParamLinea,
    pub conductor: Conductor,
    pub hipotesis: Vec<FilaHipotesis>,
    pub fuentes: Vec<(String, String)>,
    pub estructuras: Vec<FilaEstructura>,
    pub catalogos: BTreeMap<String, TablaCatalogo>,
    pub ruta: Option<PathBuf>,
    pub modificado: bool,
    cache_topo: Option<std::sync::Arc<Topografia>>,
    pila: Vec<Vec<FilaEstructura>>,
}

const PLANTILLA_PROYECTO: &str = include_str!("../../suite_electrica/plantillas/proyecto.json");
const PLANTILLA_CONDUCTORES: &str = include_str!("../../suite_electrica/plantillas/conductores.csv");
const PLANTILLA_CAT: [(&str, &str); 4] = [
    ("postes", include_str!("../../suite_electrica/plantillas/postes.csv")),
    ("armados", include_str!("../../suite_electrica/plantillas/armados.csv")),
    ("retenidas", include_str!("../../suite_electrica/plantillas/retenidas.csv")),
    ("materiales", include_str!("../../suite_electrica/plantillas/materiales.csv")),
];

fn redondear(x: f64, d: usize) -> f64 {
    crate::numerico::redondear(x, d)
}

impl Default for Documento {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Documento {
    /// Proyecto nuevo con las plantillas (hipótesis, fuentes PENDIENTE, conductor y catálogos).
    pub fn nuevo() -> Self {
        let pl: Value = serde_json::from_str(PLANTILLA_PROYECTO).expect("plantilla válida");
        let hipotesis = pl["hipotesis"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|h| FilaHipotesis {
                        nombre: h["nombre"].as_str().unwrap_or("").into(),
                        temperatura_c: h["temperatura_C"].as_f64().unwrap_or(0.0),
                        presion_viento_pa: h.get("presion_viento_Pa").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        espesor_hielo_mm: h.get("espesor_hielo_mm").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        densidad_hielo_kg_m3: h.get("densidad_hielo_kg_m3").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        altitud_min_m: None,
                        altitud_max_m: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut fuentes: Vec<(String, String)> = pl["fuentes"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
            .unwrap_or_default();
        for k in FUENTES_EXTRA {
            if !fuentes.iter().any(|f| f.0 == k) {
                fuentes.push((k.into(), "PENDIENTE: citar CNE / norma DGE".into()));
            }
        }
        let conductor = leer_catalogo_conductores(PLANTILLA_CONDUCTORES)
            .ok()
            .and_then(|c| {
                // primer conductor del archivo (orden del CSV, como Python)
                let primero = catalogos::leer_csv(PLANTILLA_CONDUCTORES).ok()?.first()?.get("nombre")?.trim().to_string();
                c.get(&primero).cloned()
            })
            .unwrap_or_default();
        let catalogos = PLANTILLA_CAT
            .iter()
            .map(|(n, t)| (n.to_string(), TablaCatalogo::desde_csv(n, t).expect("plantilla de catálogo")))
            .collect();
        Documento {
            nombre: "Proyecto nuevo".into(),
            autor: String::new(),
            zona_utm: "19S".into(),
            puntos: Vec::new(),
            eje: None,
            topo: ParamTopo::default(),
            linea: ParamLinea::default(),
            conductor,
            hipotesis,
            fuentes,
            estructuras: Vec::new(),
            catalogos,
            ruta: None,
            modificado: false,
            cache_topo: None,
            pila: Vec::new(),
        }
    }

    pub fn fuente(&self, k: &str) -> String {
        self.fuentes.iter().find(|f| f.0 == k).map(|f| f.1.clone()).unwrap_or_default()
    }

    pub fn poner_fuente(&mut self, k: &str, v: &str) {
        match self.fuentes.iter_mut().find(|f| f.0 == k) {
            Some(f) => f.1 = v.into(),
            None => self.fuentes.push((k.into(), v.into())),
        }
        self.modificado = true;
    }

    /// Catálogos de LineCraft armados desde las tablas del documento.
    pub fn catalogos_obj(&self) -> Result<Catalogos, String> {
        let t = |n: &str| self.catalogos.get(n).map(|t| t.a_csv()).unwrap_or_else(|| columnas_catalogo(n).join(",") + "\n");
        Catalogos::desde_csv(&t("postes"), &t("armados"), &t("retenidas"), &t("materiales"))
    }

    // ------------------------------------------------------------------ topografía

    /// Invalida la topografía calculada (tras cambiar puntos, eje o parámetros).
    pub fn cambio_topografia(&mut self) {
        self.cache_topo = None;
        self.modificado = true;
    }

    pub fn importar_puntos(&mut self, ruta: &Path) -> Result<(), String> {
        self.puntos = entrada::leer(ruta)?;
        self.cambio_topografia();
        Ok(())
    }

    /// Eje desde DXF/CSV, o la ruta `nombre` (por defecto la primera) de KML/GPX/XLS.
    pub fn importar_eje(&mut self, ruta: &Path, nombre: Option<&str>) -> Result<(), String> {
        let ext = ruta.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let eje: Vec<[f64; 2]> = match ext.as_str() {
            "kml" | "gpx" | "xls" | "xlsx" => {
                let rutas = entrada::importar(ruta, None)?.1;
                let elegida = rutas.into_iter().find(|(n, _)| nombre.map_or(true, |x| x == n)).ok_or_else(|| {
                    format!("{}: sin rutas{}", ruta.display(), nombre.map(|n| format!(" llamadas {n:?}")).unwrap_or_default())
                })?;
                elegida.1.iter().map(|p| [p[0], p[1]]).collect()
            }
            "dxf" => entrada::leer_polilinea(ruta, None)?,
            _ => {
                let b = std::fs::read(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
                entrada::leer_csv_texto(&String::from_utf8_lossy(&b), &ruta.display().to_string(), &["este", "norte"])?
                    .iter()
                    .map(|p| [p.este, p.norte])
                    .collect()
            }
        };
        if eje.len() < 2 {
            return Err(format!("{}: el eje necesita al menos 2 vértices", ruta.display()));
        }
        self.eje = Some(eje);
        self.estructuras.clear();
        self.cambio_topografia();
        Ok(())
    }

    /// Limpieza, TIN, curvas y perfil (en caché).
    pub fn topografia(&mut self) -> Result<&Topografia, String> {
        if self.cache_topo.is_none() {
            if self.puntos.is_empty() {
                return Err("El proyecto no tiene puntos topográficos (Importar puntos)".into());
            }
            let t = &self.topo;
            let (pts, rep) = limpiar(&self.puntos, 0.001, 0.001, t.quitar_atipicos)?;
            let xyz: Vec<[f64; 3]> = pts.iter().map(|p| [p.este, p.norte, p.cota]).collect();
            let tin = Tin::nuevo(&xyz, t.max_lado)?;
            let (zmin, zmax) = xyz.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, p| (a.0.min(p[2]), a.1.max(p[2])));
            let curvas = tin.curvas_nivel(&niveles(zmin, zmax, t.equidistancia)?);
            let perfil = match &self.eje {
                Some(e) => Some(perfil_longitudinal(&tin, e, t.paso_perfil)?),
                None => None,
            };
            self.cache_topo = Some(std::sync::Arc::new(Topografia { puntos: pts, reporte: rep, tin, curvas, perfil }));
        }
        Ok(self.cache_topo.as_deref().unwrap())
    }

    // ------------------------------------------------------------------ línea

    pub fn hipotesis_obj(&self) -> Vec<Hipotesis> {
        let lim = |v: Option<f64>, defecto: f64| match v {
            None => defecto,
            Some(x) if x.is_nan() || (x == 0.0 && defecto > 0.0) => defecto,
            Some(x) => x,
        };
        self.hipotesis
            .iter()
            .filter(|h| !h.nombre.trim().is_empty())
            .map(|h| Hipotesis {
                nombre: h.nombre.clone(),
                temperatura: h.temperatura_c,
                presion_viento: h.presion_viento_pa,
                espesor_hielo: h.espesor_hielo_mm / 1000.0,
                densidad_hielo: h.densidad_hielo_kg_m3,
                altitud_min: lim(h.altitud_min_m, f64::NEG_INFINITY),
                altitud_max: lim(h.altitud_max_m, f64::INFINITY),
            })
            .collect()
    }

    /// Proyecto de LineCraft equivalente al documento.
    pub fn proyecto(&mut self, con_estructuras: bool) -> Result<Proyecto, String> {
        let eje = self.eje.clone().ok_or("El proyecto no tiene eje de línea (Importar eje)")?;
        let perfil = self.topografia()?.perfil.clone().ok_or("Sin perfil")?;
        let cat = self.catalogos_obj()?;
        let l = &self.linea;
        let manual = if con_estructuras && !self.estructuras.is_empty() {
            let mut v = Vec::new();
            for e in &self.estructuras {
                v.push(EstructuraManual {
                    progresiva: e.progresiva_m,
                    tipo: if e.tipo.is_empty() { None } else { Some(Tipo::desde_str(&e.tipo)?) },
                    altura_amarre: Some(e.altura_amarre_m.unwrap_or_else(|| cat.altura_amarre(&e.poste, &e.armado).unwrap_or(l.altura_amarre))),
                    armado: e.armado.clone(),
                    poste: e.poste.clone(),
                });
            }
            Some(v)
        } else {
            None
        };
        let p = Proyecto {
            nombre: self.nombre.clone(),
            conductor: self.conductor.clone(),
            n_conductores: l.n_conductores.max(1) as usize,
            modelo: if l.modelo == "parabola" { Modelo::Parabola } else { Modelo::Catenaria },
            hipotesis: self.hipotesis_obj(),
            hip_inicial: l.hip_inicial.clone(),
            tiro_inicial_pct: l.tiro_inicial_pct,
            hip_flecha_maxima: l.hip_flecha_maxima.clone(),
            tiro_maximo_pct: l.tiro_maximo_pct,
            distancia_minima_terreno: l.distancia_minima_terreno,
            altura_amarre: l.altura_amarre,
            vano_maximo: l.vano_maximo,
            vano_minimo: l.vano_minimo,
            angulo_retenida: l.angulo_retenida,
            perfil,
            eje,
            estructuras_manual: manual,
            fuentes: self.fuentes.iter().cloned().collect(),
            alertas: Default::default(),
            condicion_final: l.condicion_final.clone(),
            vanos_independientes: l.estado_final_vanos == "por vano",
        };
        p.hip(&p.hip_inicial, None)?;
        p.hip(&p.hip_flecha_maxima, None)?;
        Ok(p)
    }

    fn n_retenidas(&self, r: &Resultado) -> BTreeMap<usize, usize> {
        let mut out = BTreeMap::new();
        if self.estructuras.len() == r.estructuras.len() {
            for (e, f) in r.estructuras.iter().zip(&self.estructuras) {
                if let Some(n) = f.retenidas.filter(|n| !n.is_nan()) {
                    out.insert(e.numero, n as usize);
                }
            }
        }
        out
    }

    /// Verificación de catálogos, metrado, flechado, alertas y avisos.
    fn completar(&self, p: Proyecto, mut r: Resultado) -> Result<Calculo, String> {
        let l = &self.linea;
        let cat = self.catalogos_obj()?;
        let nret = self.n_retenidas(&r);
        let ret = if l.retenida.is_empty() { None } else { Some(l.retenida.as_str()) };
        let ver = catalogos::verificar(&r, &p, &cat, l.fs_poste, l.fs_retenida, ret, &nret);
        let met = catalogos::metrado(&r, &p, &cat, l.desperdicio_pct, ret, &nret);
        r.avisos.extend(cat.avisos());
        let pendiente = |f: &str| f.is_empty() || f.to_uppercase().contains("PENDIENTE") || f.to_uppercase().contains("EJEMPLO");
        for k in FUENTES_EXTRA {
            let f = self.fuente(k).trim().to_string();
            if pendiente(&f) {
                r.avisos.push(format!("PENDIENTE_REVISION: '{k}' -> {}", if f.is_empty() { "sin fuente".into() } else { f }));
            }
        }
        for v in ver.iter().filter(|v| !v.cumple) {
            r.avisos.push(format!("{}: {}", v.estructura, v.observacion));
        }
        let al = alertas(&r, &p, Some(&cat), &ver, &l.config_alertas());
        for k in FUENTES_ALERTAS {
            let f = self.fuente(k).trim().to_string();
            if l.alerta(k) != 0.0 && pendiente(&f) {
                r.avisos.push(format!("PENDIENTE_REVISION: '{k}' -> {}", if f.is_empty() { "sin fuente".into() } else { f }));
            }
        }
        let fl = tabla_flechado(&r, &p, 20.0, 5.0)?;
        Ok(Calculo { proyecto: p, resultado: r, verificacion: ver, metrado: met, alertas: al, flechado: fl })
    }

    /// Calcula la línea; si no hay estructuras, primero las ubica automáticamente.
    pub fn calcular(&mut self) -> Result<Calculo, String> {
        if self.estructuras.is_empty() {
            return self.ubicar_automatico();
        }
        let p = self.proyecto(true)?;
        let r = calcular(&p)?;
        self.completar(p, r)
    }

    fn guardar_estructuras(&mut self, r: &Resultado) {
        let nuevas: Vec<FilaEstructura> = r
            .estructuras
            .iter()
            .map(|e| FilaEstructura {
                progresiva_m: redondear(e.progresiva, 3),
                tipo: e.tipo.como_str().into(),
                altura_amarre_m: None,
                armado: e.armado.clone(),
                poste: e.poste.clone(),
                retenidas: if catalogos::lleva_retenida(e) { Some(1.0) } else { None },
            })
            .collect();
        if nuevas != self.estructuras {
            self.pila.push(std::mem::replace(&mut self.estructuras, nuevas));
        }
        self.ordenar();
        self.modificado = true;
    }

    /// Ubicación automática de estructuras + asignación de armados y postes.
    pub fn ubicar_automatico(&mut self) -> Result<Calculo, String> {
        let p = self.proyecto(false)?;
        let r = calcular(&p)?;
        self.guardar_estructuras(&r);
        self.asignar_catalogos(true)
    }

    /// Solo inicio, fin y vértices del eje (para ubicar el resto a mano).
    pub fn estructuras_minimas(&mut self) -> Result<(), String> {
        let eje = self.eje.as_ref().ok_or("El proyecto no tiene eje de línea")?;
        let nuevas = progresivas_eje(eje).into_iter().map(|s| FilaEstructura::en(redondear(s, 3))).collect();
        self.apilar();
        self.estructuras = nuevas;
        self.modificado = true;
        Ok(())
    }

    /// Asigna armado (por tipo y deflexión) y poste (menor que cumple) a cada estructura.
    pub fn asignar_catalogos(&mut self, solo_vacios: bool) -> Result<Calculo, String> {
        let p = self.proyecto(true)?;
        let r = calcular(&p)?;
        let cat = self.catalogos_obj()?;
        let fm = catalogos::fuerzas_maximas(&r);
        let mut df = self.estructuras.clone();
        for (i, e) in r.estructuras.iter().enumerate() {
            if i >= df.len() {
                break;
            }
            if !(solo_vacios && !df[i].armado.is_empty()) {
                df[i].armado = cat.elegir_armado(e.tipo, e.angulo);
            }
            if !(solo_vacios && !df[i].poste.is_empty()) {
                let (fh, mb, _) = fm.get(&e.numero).copied().unwrap_or((0.0, 0.0, 0.0));
                let cargas: Vec<_> = r.cargas.iter().filter(|c| c.estructura == e.numero).collect();
                let arm = if df[i].armado.is_empty() { None } else { cat.armado(&df[i].armado) };
                df[i].poste = catalogos::elegir_poste_mrn(&cat, e, &cargas, arm, p.n_conductores, fh, mb, self.linea.fs_poste);
            }
        }
        if df != self.estructuras {
            self.apilar();
            self.estructuras = df;
            self.modificado = true;
        }
        let p = self.proyecto(true)?;
        let r = calcular(&p)?;
        self.completar(p, r)
    }

    // ------------------------------------------------------------------ edición de estructuras

    fn apilar(&mut self) {
        self.pila.push(self.estructuras.clone());
        if self.pila.len() > 50 {
            self.pila.remove(0);
        }
    }

    fn ordenar(&mut self) {
        self.estructuras.retain(|e| !e.progresiva_m.is_nan());
        self.estructuras.sort_by(|a, b| a.progresiva_m.partial_cmp(&b.progresiva_m).unwrap());
    }

    pub fn puede_deshacer(&self) -> bool {
        !self.pila.is_empty()
    }

    pub fn deshacer(&mut self) -> bool {
        match self.pila.pop() {
            Some(e) => {
                self.estructuras = e;
                self.modificado = true;
                true
            }
            None => false,
        }
    }

    fn acumuladas(&self) -> Vec<f64> {
        self.eje.as_ref().map(|e| progresivas_eje(e)).unwrap_or_default()
    }

    /// Inicio, fin y vértices del eje no se mueven ni se eliminan.
    pub fn bloqueada(&self, i: usize) -> bool {
        let s = self.estructuras[i].progresiva_m;
        self.acumuladas().iter().any(|a| (a - s).abs() <= TOL_VERTICE)
    }

    fn limites(&self, i: usize) -> (f64, f64) {
        let s: Vec<f64> = self.estructuras.iter().map(|e| e.progresiva_m).collect();
        let lo = if i > 0 { s[i - 1] + SEPARACION_MIN } else { 0.0 };
        let hi = if i + 1 < s.len() { s[i + 1] - SEPARACION_MIN } else { *self.acumuladas().last().unwrap_or(&0.0) };
        (lo, hi)
    }

    /// Inserta una estructura en la progresiva `s`; devuelve su índice.
    pub fn agregar_estructura(&mut self, s: f64) -> Result<usize, String> {
        let l = *self.acumuladas().last().ok_or("Sin eje")?;
        if !(s > 0.0 && s < l) || self.estructuras.iter().any(|e| (e.progresiva_m - s).abs() < SEPARACION_MIN) {
            return Err(format!("No se puede ubicar una estructura en {s:.2} m"));
        }
        self.apilar();
        let s = redondear(s, 2);
        self.estructuras.push(FilaEstructura::en(s));
        self.ordenar();
        self.modificado = true;
        Ok(self.estructuras.iter().position(|e| (e.progresiva_m - s).abs() < 1e-9).unwrap())
    }

    /// Mueve la estructura `i` (limitada entre vecinas); devuelve la progresiva final.
    pub fn mover_estructura(&mut self, i: usize, s: f64) -> Result<f64, String> {
        if self.bloqueada(i) {
            return Err("Las estructuras de inicio, fin y vértices no se mueven".into());
        }
        let (lo, hi) = self.limites(i);
        let s = redondear(s.max(lo).min(hi), 2);
        if s != self.estructuras[i].progresiva_m {
            self.apilar();
            self.estructuras[i].progresiva_m = s;
            self.modificado = true;
        }
        Ok(s)
    }

    pub fn eliminar_estructura(&mut self, i: usize) -> Result<(), String> {
        if self.bloqueada(i) {
            return Err("Las estructuras de inicio, fin y vértices no se eliminan".into());
        }
        self.apilar();
        self.estructuras.remove(i);
        self.modificado = true;
        Ok(())
    }

    /// Cambia `tipo`, `armado`, `poste`, `altura_amarre_m` o `retenidas` (texto vacío = automático).
    pub fn cambiar_estructura(&mut self, i: usize, campo: &str, valor: &str) -> Result<(), String> {
        let v = valor.trim();
        let num = |v: &str| -> Result<Option<f64>, String> {
            if v.is_empty() { Ok(None) } else { v.replace(',', ".").parse::<f64>().map(Some).map_err(|_| format!("Número inválido: {v}")) }
        };
        let mut e = self.estructuras[i].clone();
        match campo {
            "tipo" => {
                let t = v.to_uppercase();
                if !t.is_empty() {
                    Tipo::desde_str(&t).map_err(|_| format!("Tipo inválido: {v}"))?;
                }
                e.tipo = t;
            }
            "armado" => e.armado = v.into(),
            "poste" => e.poste = v.into(),
            "altura_amarre_m" => e.altura_amarre_m = num(v)?,
            "retenidas" => e.retenidas = num(v)?,
            otro => return Err(format!("Campo no editable: {otro}")),
        }
        if e != self.estructuras[i] {
            self.apilar();
            self.estructuras[i] = e;
            self.modificado = true;
        }
        Ok(())
    }

    /// Deflexión del eje si `s` coincide con un vértice (grados).
    pub fn angulo_en(&self, s: f64) -> f64 {
        let Some(eje) = &self.eje else { return 0.0 };
        let acum = progresivas_eje(eje);
        acum.iter().position(|a| (a - s).abs() <= TOL_VERTICE).map(|j| angulos_eje(eje)[j]).unwrap_or(0.0)
    }

    // ------------------------------------------------------------------ archivo

    pub fn a_json(&self) -> Value {
        let c = &self.conductor;
        let lista = |v: &[f64]| v.iter().map(|x| crate::numerico::formato_g(*x)).collect::<Vec<_>>().join(";");
        let opt = |v: Option<f64>| v.filter(|x| !x.is_nan()).map_or(Value::Null, |x| json!(x));
        let num = |x: f64| if x.is_nan() { Value::Null } else { json!(x) };
        let mut d = Map::new();
        d.insert("formato".into(), json!(FORMATO));
        d.insert("version".into(), json!(VERSION));
        d.insert("nombre".into(), json!(self.nombre));
        d.insert("autor".into(), json!(self.autor));
        d.insert("zona_utm".into(), json!(self.zona_utm));
        d.insert("topo".into(), json!({
            "equidistancia": self.topo.equidistancia, "mayor_cada": self.topo.mayor_cada,
            "paso_perfil": self.topo.paso_perfil, "max_lado": self.topo.max_lado,
            "quitar_atipicos": self.topo.quitar_atipicos,
        }));
        d.insert("linea".into(), self.linea.a_json());
        d.insert("conductor".into(), json!({
            "nombre": c.nombre, "seccion_mm2": c.seccion * 1e6, "diametro_mm": c.diametro * 1e3, "masa_kg_m": c.peso / G,
            "carga_rotura_kN": c.carga_rotura / 1e3, "modulo_elasticidad_MPa": c.modulo_elasticidad / 1e6,
            "coef_dilatacion_1_C": c.coef_dilatacion, "fuente": c.fuente, "curva_inicial": lista(&c.curva_inicial),
            "curva_creep": lista(&c.curva_creep),
        }));
        d.insert("hipotesis".into(), Value::Array(self.hipotesis.iter().map(|h| json!({
            "nombre": h.nombre, "temperatura_C": h.temperatura_c, "presion_viento_Pa": h.presion_viento_pa,
            "espesor_hielo_mm": h.espesor_hielo_mm, "densidad_hielo_kg_m3": h.densidad_hielo_kg_m3,
            "altitud_min_m": opt(h.altitud_min_m), "altitud_max_m": opt(h.altitud_max_m),
        })).collect()));
        d.insert("fuentes".into(), Value::Object(self.fuentes.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
        d.insert("puntos".into(), json!({
            "columnas": ["punto", "este", "norte", "cota", "descripcion"],
            "filas": self.puntos.iter().map(|p| json!([p.punto, num(p.este), num(p.norte), num(p.cota), p.descripcion])).collect::<Vec<_>>(),
        }));
        d.insert("eje".into(), self.eje.as_ref().map_or(Value::Null, |e| json!(e)));
        d.insert("estructuras".into(), Value::Array(self.estructuras.iter().map(|e| json!({
            "progresiva_m": e.progresiva_m, "tipo": e.tipo, "altura_amarre_m": opt(e.altura_amarre_m),
            "armado": e.armado, "poste": e.poste, "retenidas": opt(e.retenidas),
        })).collect()));
        let cats: Map<String, Value> = COLS_CATALOGO
            .iter()
            .map(|(n, _)| {
                let t = self.catalogos.get(*n);
                let filas = t.map(|t| {
                    t.filas.iter().map(|f| {
                        Value::Object(t.columnas.iter().zip(f).map(|(c, v)| {
                            let val = if v.trim().is_empty() {
                                Value::Null
                            } else if c == "codigo" || c == "armado" || c == "fuente" || c == "descripcion" || c == "material" || c == "unidad" {
                                json!(v)
                            } else {
                                v.trim().parse::<f64>().map(|x| json!(x)).unwrap_or_else(|_| json!(v))
                            };
                            (c.clone(), val)
                        }).collect())
                    }).collect::<Vec<_>>()
                }).unwrap_or_default();
                (n.to_string(), Value::Array(filas))
            })
            .collect();
        d.insert("catalogos".into(), Value::Object(cats));
        Value::Object(d)
    }

    pub fn desde_json(d: &Value, ruta: &Path) -> Result<Self, String> {
        if d.get("formato").and_then(|x| x.as_str()) != Some(FORMATO) {
            return Err(format!("{} no es un proyecto de Suite Eléctrica", ruta.display()));
        }
        if d.get("version").and_then(|x| x.as_i64()).unwrap_or(0) > VERSION {
            return Err("El archivo es de una versión más nueva de la suite".into());
        }
        let mut doc = Documento::nuevo();
        let s = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let f = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_f64());
        doc.nombre = s(d, "nombre");
        doc.autor = s(d, "autor");
        doc.zona_utm = d.get("zona_utm").and_then(|x| x.as_str()).unwrap_or("19S").into();
        let t = &d["topo"];
        doc.topo = ParamTopo {
            equidistancia: f(t, "equidistancia").unwrap_or(1.0),
            mayor_cada: t.get("mayor_cada").and_then(|x| x.as_i64()).unwrap_or(5),
            paso_perfil: f(t, "paso_perfil").unwrap_or(20.0),
            max_lado: f(t, "max_lado"),
            quitar_atipicos: t.get("quitar_atipicos").and_then(|x| x.as_bool()).unwrap_or(false),
        };
        doc.linea = ParamLinea::desde_json(&d["linea"]);
        let c = &d["conductor"];
        let req = |k: &str| f(c, k).ok_or(format!("conductor: falta {k}"));
        let curva = |k: &str| {
            let t = match c.get(k) {
                Some(Value::String(t)) => t.clone(),
                Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_f64()).map(|x| x.to_string()).collect::<Vec<_>>().join(";"),
                _ => String::new(),
            };
            t.split(';').filter_map(|v| v.trim().replace(',', ".").parse::<f64>().ok()).collect::<Vec<f64>>()
        };
        doc.conductor = Conductor {
            nombre: s(c, "nombre"),
            fuente: s(c, "fuente"),
            seccion: req("seccion_mm2")? * 1e-6,
            diametro: req("diametro_mm")? * 1e-3,
            peso: req("masa_kg_m")? * G,
            carga_rotura: req("carga_rotura_kN")? * 1e3,
            modulo_elasticidad: req("modulo_elasticidad_MPa")? * 1e6,
            coef_dilatacion: req("coef_dilatacion_1_C")?,
            curva_inicial: curva("curva_inicial"),
            curva_creep: curva("curva_creep"),
        };
        doc.hipotesis = d["hipotesis"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|h| FilaHipotesis {
                        nombre: s(h, "nombre"),
                        temperatura_c: f(h, "temperatura_C").unwrap_or(f64::NAN),
                        presion_viento_pa: f(h, "presion_viento_Pa").unwrap_or(0.0),
                        espesor_hielo_mm: f(h, "espesor_hielo_mm").unwrap_or(0.0),
                        densidad_hielo_kg_m3: f(h, "densidad_hielo_kg_m3").unwrap_or(0.0),
                        altitud_min_m: f(h, "altitud_min_m"),
                        altitud_max_m: f(h, "altitud_max_m"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        doc.fuentes = d
            .get("fuentes")
            .and_then(|x| x.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect())
            .unwrap_or_default();
        let p = &d["puntos"];
        let cols: Vec<String> = p["columnas"].as_array().map(|a| a.iter().filter_map(|c| c.as_str().map(String::from)).collect()).unwrap_or_default();
        let idx = |k: &str| cols.iter().position(|c| c == k);
        doc.puntos = p["filas"]
            .as_array()
            .map(|filas| {
                filas
                    .iter()
                    .map(|fila| {
                        let v = |k: &str| idx(k).and_then(|i| fila.get(i)).cloned().unwrap_or(Value::Null);
                        let n = |k: &str| match v(k) {
                            Value::Number(x) => x.as_f64().unwrap_or(f64::NAN),
                            Value::String(t) => t.replace(',', ".").parse().unwrap_or(f64::NAN),
                            _ => f64::NAN,
                        };
                        let t = |k: &str| match v(k) {
                            Value::String(t) => t,
                            Value::Null => String::new(),
                            otro => otro.to_string(),
                        };
                        Punto { punto: t("punto"), este: n("este"), norte: n("norte"), cota: n("cota"), descripcion: t("descripcion") }
                    })
                    .collect()
            })
            .unwrap_or_default();
        doc.eje = d.get("eje").and_then(|e| e.as_array()).map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect());
        doc.estructuras = d
            .get("estructuras")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        Some(FilaEstructura {
                            progresiva_m: f(e, "progresiva_m")?,
                            tipo: s(e, "tipo").trim().to_uppercase(),
                            altura_amarre_m: f(e, "altura_amarre_m"),
                            armado: s(e, "armado").trim().into(),
                            poste: s(e, "poste").trim().into(),
                            retenidas: f(e, "retenidas"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        doc.ordenar();
        if let Some(cats) = d.get("catalogos").and_then(|x| x.as_object()) {
            for (n, cols) in COLS_CATALOGO {
                if let Some(filas) = cats.get(n).and_then(|x| x.as_array()) {
                    let tabla = TablaCatalogo {
                        columnas: cols.iter().map(|c| c.to_string()).collect(),
                        filas: filas
                            .iter()
                            .map(|r| {
                                cols.iter()
                                    .map(|c| match r.get(*c) {
                                        Some(Value::String(t)) => t.clone(),
                                        Some(Value::Number(x)) => x.as_f64().map(|v| v.to_string()).unwrap_or_default(),
                                        Some(Value::Bool(b)) => b.to_string(),
                                        _ => String::new(),
                                    })
                                    .collect()
                            })
                            .collect(),
                    };
                    doc.catalogos.insert(n.to_string(), tabla);
                }
            }
        }
        doc.ruta = Some(ruta.to_path_buf());
        doc.modificado = false;
        Ok(doc)
    }

    pub fn abrir(ruta: &Path) -> Result<Self, String> {
        let texto = std::fs::read(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let v: Value = serde_json::from_slice(&texto).map_err(|e| format!("{}: {e}", ruta.display()))?;
        Self::desde_json(&v, ruta)
    }

    /// Guarda (UTF-8, sangría de 1 espacio como Python); agrega la extensión `.sel`.
    pub fn guardar(&mut self, ruta: Option<&Path>) -> Result<PathBuf, String> {
        let ruta = ruta.map(Path::to_path_buf).or_else(|| self.ruta.clone()).ok_or("Indique dónde guardar")?;
        let ruta = if ruta.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION)) { ruta } else { ruta.with_extension(EXTENSION) };
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::PrettyFormatter::with_indent(b" "));
        serde::Serialize::serialize(&self.a_json(), &mut ser).map_err(|e| e.to_string())?;
        std::fs::write(&ruta, buf).map_err(|e| format!("{}: {e}", ruta.display()))?;
        self.ruta = Some(ruta.clone());
        self.modificado = false;
        Ok(ruta)
    }

    /// Importa un `proyecto.json` de LineCraft (con `puntos` y `eje`, no Excel).
    pub fn desde_linecraft_json(ruta: &Path) -> Result<Self, String> {
        let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let cfg: Value = serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let base = ruta.parent().unwrap_or(Path::new("."));
        let mut doc = Documento::nuevo();
        doc.nombre = cfg.get("nombre").and_then(|v| v.as_str()).map(String::from)
            .unwrap_or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        let pf = &cfg["perfil"];
        let (Some(pp), Some(pe)) = (pf.get("puntos").and_then(|v| v.as_str()), pf.get("eje").and_then(|v| v.as_str())) else {
            return Err("Solo se importan proyectos con 'puntos' y 'eje'".into());
        };
        doc.importar_puntos(&base.join(pp))?;
        doc.importar_eje(&base.join(pe), None)?;
        doc.topo.paso_perfil = pf.get("paso").and_then(|v| v.as_f64()).unwrap_or(doc.topo.paso_perfil);
        doc.topo.max_lado = pf.get("max_lado").and_then(|v| v.as_f64());
        let p = crate::proyectos::cargar_proyecto_json(ruta)?;
        doc.conductor = p.conductor.clone();
        doc.hipotesis = p
            .hipotesis
            .iter()
            .map(|h| FilaHipotesis {
                nombre: h.nombre.clone(),
                temperatura_c: h.temperatura,
                presion_viento_pa: h.presion_viento,
                espesor_hielo_mm: h.espesor_hielo * 1000.0,
                densidad_hielo_kg_m3: h.densidad_hielo,
                altitud_min_m: h.altitud_min.is_finite().then_some(h.altitud_min),
                altitud_max_m: h.altitud_max.is_finite().then_some(h.altitud_max),
            })
            .collect();
        let l = &mut doc.linea;
        l.n_conductores = p.n_conductores as i64;
        l.modelo = if matches!(p.modelo, Modelo::Parabola) { "parabola".into() } else { "catenaria".into() };
        l.hip_inicial = p.hip_inicial.clone();
        l.tiro_inicial_pct = p.tiro_inicial_pct;
        l.hip_flecha_maxima = p.hip_flecha_maxima.clone();
        l.tiro_maximo_pct = p.tiro_maximo_pct;
        l.distancia_minima_terreno = p.distancia_minima_terreno;
        l.altura_amarre = p.altura_amarre;
        l.vano_maximo = p.vano_maximo;
        l.vano_minimo = p.vano_minimo;
        l.angulo_retenida = p.angulo_retenida;
        if let Some(m) = cfg.get("fuentes").and_then(|v| v.as_object()) {
            for (k, v) in m {
                doc.poner_fuente(k, &v.as_str().map(String::from).unwrap_or_else(|| v.to_string()));
            }
        }
        if let Some(man) = &p.estructuras_manual {
            doc.estructuras = man
                .iter()
                .map(|e| FilaEstructura {
                    progresiva_m: e.progresiva,
                    tipo: e.tipo.map(|t| t.como_str().to_string()).unwrap_or_default(),
                    altura_amarre_m: e.altura_amarre,
                    armado: e.armado.clone(),
                    poste: e.poste.clone(),
                    retenidas: None,
                })
                .collect();
        }
        doc.ruta = None;
        doc.modificado = true;
        Ok(doc)
    }
}

/// Metrado de LineCraft → `rubro, uc, cantidad` para PresuCraft (conductor de m a km).
pub fn metrado_presupuesto(metrado: &[FilaMetrado], rubro: &str) -> String {
    let mut out = String::from("rubro,uc,cantidad\n");
    for m in metrado {
        let conductor = m.item == "Conductor";
        if m.codigo.is_empty() || (conductor && m.unidad != "m") {
            continue;
        }
        let cant = if conductor { m.cantidad / 1000.0 } else { m.cantidad };
        let uc = if m.codigo.contains(',') || m.codigo.contains('"') { format!("\"{}\"", m.codigo.replace('"', "\"\"")) } else { m.codigo.clone() };
        out.push_str(&format!("{rubro},{uc},{cant}\n"));
    }
    out
}
