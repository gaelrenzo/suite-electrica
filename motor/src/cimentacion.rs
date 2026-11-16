//! Cimentaciones: excavación, solado, concreto, relleno y eliminación por poste, a partir de la
//! tabla `cimentaciones.csv` (dimensiones del hoyo por tipo de poste). La suite no trae valores
//! normativos: la plantilla es EJEMPLO y se avisa mientras la fuente lo diga.
//!
//! Por poste, con A = lado² (cuadrada) o π·d²/4 (circular), h la profundidad y s el solado:
//! - excavación = A·h; solado = A·s;
//! - poste empotrado = π·d_base²/4·(h − s);
//! - concreto (o relleno compactado, según `relleno`) = A·(h − s) − poste empotrado;
//! - eliminación de excedente = excavación − relleno compactado (lo que no vuelve al hoyo).

use crate::catalogos::leer_csv;

/// Plantilla de la tabla (valores de EJEMPLO).
pub const PLANTILLA: &str = include_str!("../../data/plantillas/cimentaciones.csv");

#[derive(Clone, Debug, PartialEq)]
pub struct Cimentacion {
    pub soporte: String,
    pub circular: bool,
    pub dimension_m: f64,
    pub profundidad_m: f64,
    pub solado_m: f64,
    pub diam_base_m: Option<f64>,
    pub concreto: bool,
    pub fuente: String,
}

/// Renglón del metrado de cimentaciones.
#[derive(Clone, Debug, PartialEq)]
pub struct FilaCimentacion {
    pub codigo: String,
    pub descripcion: String,
    pub unidad: String,
    pub cantidad: f64,
}

/// Partidas (código, descripción), en el orden de la planilla.
pub const PARTIDAS: [(&str, &str); 5] = [
    ("CIM-EXC", "Excavación de hoyo para poste"),
    ("CIM-SOL", "Solado"),
    ("CIM-CON", "Concreto de cimentación"),
    ("CIM-REL", "Relleno compactado con material propio"),
    ("CIM-ELI", "Eliminación de material excedente"),
];

pub fn leer(csv: &str) -> Result<Vec<Cimentacion>, String> {
    let num = |f: &std::collections::BTreeMap<String, String>, k: &str| -> Option<f64> {
        f.get(k).and_then(|x| x.trim().replace(',', ".").parse().ok())
    };
    leer_csv(csv)?
        .iter()
        .filter(|f| f.get("soporte").is_some_and(|s| !s.trim().is_empty()))
        .map(|f| {
            let s = f["soporte"].trim().to_string();
            let forma = f.get("forma").map(|x| x.trim().to_lowercase()).unwrap_or_default();
            let relleno = f.get("relleno").map(|x| x.trim().to_lowercase()).unwrap_or_default();
            let req = |k: &str| num(f, k).filter(|x| *x >= 0.0).ok_or(format!("cimentaciones: {s}: falta '{k}'"));
            let c = Cimentacion {
                circular: match forma.as_str() {
                    "circular" => true,
                    "cuadrada" | "" => false,
                    o => return Err(format!("cimentaciones: {s}: forma '{o}' (cuadrada o circular)")),
                },
                concreto: match relleno.as_str() {
                    "concreto" => true,
                    "compactado" | "" => false,
                    o => return Err(format!("cimentaciones: {s}: relleno '{o}' (concreto o compactado)")),
                },
                dimension_m: req("dimension_m")?,
                profundidad_m: req("profundidad_m")?,
                solado_m: num(f, "solado_m").unwrap_or(0.0),
                diam_base_m: num(f, "diam_base_m").filter(|x| *x > 0.0),
                fuente: f.get("fuente").map(|x| x.trim().to_string()).unwrap_or_default(),
                soporte: s,
            };
            if c.solado_m > c.profundidad_m {
                return Err(format!("cimentaciones: {}: el solado es mayor que la profundidad", c.soporte));
            }
            Ok(c)
        })
        .collect()
}

/// Volúmenes de un poste (m³): [excavación, solado, concreto, relleno, eliminación].
pub fn volumenes(c: &Cimentacion) -> [f64; 5] {
    let area = if c.circular { std::f64::consts::PI * c.dimension_m.powi(2) / 4.0 } else { c.dimension_m.powi(2) };
    let exc = area * c.profundidad_m;
    let sol = area * c.solado_m;
    let util = c.profundidad_m - c.solado_m;
    let poste = c.diam_base_m.map_or(0.0, |d| std::f64::consts::PI * d * d / 4.0 * util).min(area * util);
    let lleno = area * util - poste;
    let (con, rel) = if c.concreto { (lleno, 0.0) } else { (0.0, lleno) };
    [exc, sol, con, rel, exc - rel]
}

/// Metrado de cimentaciones para `postes` = [(código del soporte, cantidad)]. Devuelve las filas
/// (solo las partidas con cantidad) y los avisos: soportes sin fila, valores de EJEMPLO o
/// PENDIENTE y filas sin diámetro de base.
pub fn metrado(postes: &[(String, f64)], tabla: &[Cimentacion]) -> (Vec<FilaCimentacion>, Vec<String>) {
    let mut tot = [0.0; 5];
    let mut avisos = Vec::new();
    let mut usados: Vec<&Cimentacion> = Vec::new();
    for (cod, q) in postes.iter().filter(|p| p.1 > 0.0) {
        match tabla.iter().find(|c| c.soporte.eq_ignore_ascii_case(cod.trim())) {
            Some(c) => {
                for (t, v) in tot.iter_mut().zip(volumenes(c)) {
                    *t += q * v;
                }
                if !usados.contains(&c) {
                    usados.push(c);
                }
            }
            None => avisos.push(format!("Soporte {cod} sin cimentación en la tabla ({q} und)")),
        }
    }
    let provisional: Vec<&str> = usados
        .iter()
        .filter(|c| {
            let f = c.fuente.to_uppercase();
            f.is_empty() || f.contains("EJEMPLO") || f.contains("PENDIENTE")
        })
        .map(|c| c.soporte.as_str())
        .collect();
    if !provisional.is_empty() {
        avisos.push(format!("Cimentación con valores de EJEMPLO/PENDIENTE o sin fuente: {}", provisional.join(", ")));
    }
    let sin_base: Vec<&str> = usados.iter().filter(|c| c.diam_base_m.is_none()).map(|c| c.soporte.as_str()).collect();
    if !sin_base.is_empty() {
        avisos.push(format!("Sin diámetro de base (no se descuenta el poste empotrado): {}", sin_base.join(", ")));
    }
    let filas = PARTIDAS
        .iter()
        .zip(tot)
        .filter(|(_, v)| *v > 1e-12)
        .map(|((c, d), v)| FilaCimentacion { codigo: c.to_string(), descripcion: d.to_string(), unidad: "m3".into(), cantidad: crate::numerico::redondear(v, 3) })
        .collect();
    (filas, avisos)
}

/// Nombre del archivo de la tabla, junto al proyecto (`red.json` o `.sel`).
pub const ARCHIVO: &str = "cimentaciones.csv";

/// Lee `cimentaciones.csv` de la carpeta del proyecto `ruta`; `None` si no existe.
pub fn cargar_junto(ruta: &std::path::Path) -> Result<Option<Vec<Cimentacion>>, String> {
    let r = ruta.parent().unwrap_or(std::path::Path::new(".")).join(ARCHIVO);
    if !r.exists() {
        return Ok(None);
    }
    let t = std::fs::read(&r).map_err(|e| format!("{}: {e}", r.display()))?;
    leer(&String::from_utf8_lossy(&t)).map(Some).map_err(|e| format!("{}: {e}", r.display()))
}

/// Filas del metrado → `rubro, uc, cantidad` para PresuCraft (sin encabezado).
pub fn lineas_presupuesto(filas: &[FilaCimentacion], rubro: &str) -> String {
    filas.iter().map(|f| format!("{rubro},{},{}\n", f.codigo, f.cantidad)).collect()
}
