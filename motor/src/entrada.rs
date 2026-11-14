//! Lectura de topografía y limpieza de puntos.
//!
//! Port de `topocraft/{datos, importar}.py` y `core/dxf/lectura.py`: CSV (encabezado opcional,
//! `,` `;` o tabulador, coma decimal, PENZD/PNEZD), DXF ASCII (POINT, POLYLINE, LWPOLYLINE),
//! KML, GPX, TXT de MapSource, Excel RLT (UTM y ACUMULADA/ANGULO) y TPG.

use std::collections::HashMap;
use std::path::Path;

use crate::geo::{interp, Perfil, ESTACION, VERTICE};
use crate::utm::{geograficas_a_utm, zona_desde_longitud};

/// Punto topográfico (UTM, m). `cota` NaN = sin dato.
#[derive(Clone, Debug, PartialEq)]
pub struct Punto {
    pub punto: String,
    pub este: f64,
    pub norte: f64,
    pub cota: f64,
    pub descripcion: String,
}

/// Ruta (nombre, vértices este/norte/cota; cota NaN si no hay).
pub type Ruta = (String, Vec<[f64; 3]>);

fn nombre_archivo(ruta: &Path) -> String {
    ruta.display().to_string()
}

/// Minúsculas sin acentos (como `unicodedata.normalize("NFKD").encode("ascii", "ignore")`).
fn normalizar(t: &str) -> String {
    t.trim()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            'ç' | 'Ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_ascii())
        .collect::<String>()
        .to_lowercase()
}

fn alias(v: &str) -> Option<&'static str> {
    Some(match v {
        "punto" | "pto" | "id" | "nombre" | "p" => "punto",
        "este" | "x" | "e" | "easting" => "este",
        "norte" | "y" | "n" | "northing" => "norte",
        "cota" | "z" | "elevacion" | "altura" => "cota",
        "descripcion" | "desc" | "d" | "codigo" => "descripcion",
        _ => return None,
    })
}

/// Número como `pd.to_numeric(..., errors="coerce")` tras cambiar coma por punto.
fn a_float(s: &str) -> f64 {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return f64::NAN;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite()).unwrap_or(f64::NAN)
}

fn mediana(v: &mut Vec<f64>) -> f64 {
    v.retain(|x| !x.is_nan());
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// Lee un CSV de puntos desde su texto (`requeridas`: columnas obligatorias).
pub fn leer_csv_texto(texto: &str, ruta: &str, requeridas: &[&str]) -> Result<Vec<Punto>, String> {
    let texto = texto.trim_start_matches('\u{feff}');
    let primera = texto.lines().next().unwrap_or("");
    let sep = if primera.contains(';') { b';' } else if primera.contains('\t') { b'\t' } else { b',' };
    let mut rd = csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(sep).from_reader(texto.as_bytes());
    let mut filas: Vec<Vec<String>> = Vec::new();
    for r in rd.records() {
        let r = r.map_err(|e| format!("{ruta}: {e}"))?;
        filas.push(r.iter().map(|s| s.trim_start().to_string()).collect());
    }
    if filas.is_empty() {
        return Err(format!("{ruta}: archivo vacío"));
    }
    let ncol = filas.iter().map(|f| f.len()).max().unwrap();
    let norm: Vec<String> = filas[0].iter().map(|v| normalizar(v)).collect();
    let n_alias = norm.iter().filter(|v| alias(v).is_some()).count();
    let columnas: Vec<String>;
    if n_alias >= requeridas.len().min(3) {
        columnas = norm.iter().map(|v| alias(v).map(String::from).unwrap_or_else(|| v.clone())).collect();
        filas.remove(0);
    } else if ncol == 2 {
        columnas = vec!["este".into(), "norte".into()];
    } else if ncol == 3 && !requeridas.contains(&"cota") {
        columnas = vec!["punto".into(), "este".into(), "norte".into()];
    } else {
        let base = ["punto", "este", "norte", "cota", "descripcion"];
        columnas = (0..ncol).map(|k| if k < 5 { base[k].to_string() } else { format!("extra{}", k - 5) }).collect();
    }
    let mut faltan: Vec<&str> = requeridas.iter().copied().filter(|r| !columnas.iter().any(|c| c == r)).collect();
    faltan.sort();
    if !faltan.is_empty() {
        return Err(format!("Faltan columnas en {ruta}: {faltan:?}"));
    }
    let idx = |c: &str| columnas.iter().position(|x| x == c);
    let val = |f: &Vec<String>, c: Option<usize>| c.and_then(|k| f.get(k)).cloned().unwrap_or_default();
    let mut pts: Vec<Punto> = filas
        .iter()
        .enumerate()
        .map(|(i, f)| Punto {
            punto: match idx("punto") {
                Some(k) => f.get(k).map(|s| s.trim().to_string()).unwrap_or_default(),
                None => (i + 1).to_string(),
            },
            este: a_float(&val(f, idx("este"))),
            norte: a_float(&val(f, idx("norte"))),
            cota: if idx("cota").is_some() { a_float(&val(f, idx("cota"))) } else { f64::NAN },
            descripcion: val(f, idx("descripcion")).trim().to_string(),
        })
        .collect();
    // autodetección PNEZD (estación total): el Este UTM no supera 1e6 m; el Norte sur sí
    let me = mediana(&mut pts.iter().map(|p| p.este).collect());
    let mn = mediana(&mut pts.iter().map(|p| p.norte).collect());
    if me > 1e6 && 1e6 > mn {
        for p in &mut pts {
            std::mem::swap(&mut p.este, &mut p.norte);
        }
    }
    Ok(pts)
}

// ---------------------------------------------------------------- limpieza

/// Resultado de la limpieza (listas con nombres de punto).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReporteLimpieza {
    pub n_entrada: usize,
    pub n_salida: usize,
    pub invalidos: Vec<String>,
    pub duplicados: Vec<String>,
    pub conflictos: Vec<String>,
    pub nombres_repetidos: Vec<String>,
    pub atipicos: Vec<String>,
    pub fuera_zona: Vec<String>,
    pub cotas_fuera_rango: Vec<String>,
}

pub const RANGO_ESTE: (f64, f64) = (100_000.0, 900_000.0);
pub const RANGO_NORTE: (f64, f64) = (0.0, 10_000_000.0);
pub const RANGO_COTA: (f64, f64) = (-500.0, 7_000.0);

/// Índices de cotas atípicas (ajuste de plano a los `k` vecinos; ver `detectar_atipicos` en Python).
pub fn detectar_atipicos(xyz: &[[f64; 3]], k: usize, factor: f64, tol_abs: f64, max_iter: usize) -> Vec<usize> {
    let mut activos: Vec<usize> = (0..xyz.len()).collect();
    let mut hallados = Vec::new();
    for _ in 0..max_iter {
        let kk = k.min(activos.len().saturating_sub(1));
        if kk < 4 {
            break;
        }
        let p: Vec<[f64; 3]> = activos.iter().map(|&i| xyz[i]).collect();
        let m = p.len();
        // vecinos más cercanos (sin el propio punto), por fuerza bruta
        let vec: Vec<Vec<usize>> = (0..m)
            .map(|i| {
                let mut d: Vec<(f64, usize)> =
                    (0..m).filter(|&j| j != i).map(|j| ((p[j][0] - p[i][0]).hypot(p[j][1] - p[i][1]), j)).collect();
                d.sort_by(|a, b| a.partial_cmp(b).unwrap());
                d.into_iter().take(kk).map(|x| x.1).collect()
            })
            .collect();
        let mut r = vec![0.0; m];
        let mut s = vec![0.0; m];
        for i in 0..m {
            // mínimos cuadrados z = a dx + b dy + c (ecuaciones normales 3×3)
            let mut ata = [[0.0f64; 3]; 3];
            let mut atz = [0.0f64; 3];
            for &j in &vec[i] {
                let fila = [p[j][0] - p[i][0], p[j][1] - p[i][1], 1.0];
                for a in 0..3 {
                    atz[a] += fila[a] * p[j][2];
                    for b in 0..3 {
                        ata[a][b] += fila[a] * fila[b];
                    }
                }
            }
            let coef = resolver3(ata, atz);
            let mut sse = 0.0;
            for &j in &vec[i] {
                let pred = coef[0] * (p[j][0] - p[i][0]) + coef[1] * (p[j][1] - p[i][1]) + coef[2];
                sse += (p[j][2] - pred).powi(2);
            }
            s[i] = (sse / (kk as f64 - 3.0)).sqrt();
            r[i] = p[i][2] - coef[2];
        }
        let nuevos: Vec<usize> = (0..m)
            .filter(|&i| {
                let cand = r[i].abs() > tol_abs.max(factor * s[i]);
                let maxv = vec[i].iter().map(|&j| r[j].abs()).fold(f64::NEG_INFINITY, f64::max);
                cand && r[i].abs() >= maxv
            })
            .collect();
        if nuevos.is_empty() {
            break;
        }
        hallados.extend(nuevos.iter().map(|&i| activos[i]));
        let fuera: std::collections::HashSet<usize> = nuevos.into_iter().collect();
        activos = activos.into_iter().enumerate().filter(|(i, _)| !fuera.contains(i)).map(|x| x.1).collect();
    }
    hallados.sort();
    hallados
}

fn resolver3(a: [[f64; 3]; 3], b: [f64; 3]) -> [f64; 3] {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    let mut x = [0.0; 3];
    for c in 0..3 {
        let mut m = a;
        for f in 0..3 {
            m[f][c] = b[f];
        }
        x[c] = det(m) / d;
    }
    x
}

/// Error si las coordenadas parecen geográficas (grados) en lugar de UTM (m).
pub fn validar_unidades(pts: &[Punto]) -> Result<(), String> {
    let geo = pts.iter().filter(|p| p.este.abs() <= 180.0 && p.norte.abs() <= 90.0).count();
    if !pts.is_empty() && geo as f64 / pts.len() as f64 > 0.5 {
        return Err("Las coordenadas parecen geográficas (grados): importe el archivo como KML/GPX \
                    o conviértalas a UTM WGS84 (m)"
            .into());
    }
    Ok(())
}

/// Valida y limpia los puntos (tolerancias en m; atípicos con k = 8, factor 5, 0,5 m).
pub fn limpiar(pts: &[Punto], tol_xy: f64, tol_z: f64, quitar_atipicos: bool) -> Result<(Vec<Punto>, ReporteLimpieza), String> {
    let mut rep = ReporteLimpieza { n_entrada: pts.len(), ..Default::default() };
    let (malos, mut d): (Vec<&Punto>, Vec<&Punto>) =
        pts.iter().partition(|p| p.este.is_nan() || p.norte.is_nan() || p.cota.is_nan());
    rep.invalidos = malos.iter().map(|p| p.punto.clone()).collect();
    let dv: Vec<Punto> = d.iter().map(|p| (*p).clone()).collect();
    validar_unidades(&dv)?;
    let dentro = |p: &&Punto| {
        p.este >= RANGO_ESTE.0 && p.este <= RANGO_ESTE.1 && p.norte >= RANGO_NORTE.0 && p.norte <= RANGO_NORTE.1
    };
    rep.fuera_zona = d.iter().filter(|p| !dentro(p)).map(|p| p.punto.clone()).collect();
    d.retain(dentro);
    rep.cotas_fuera_rango =
        d.iter().filter(|p| !(p.cota >= RANGO_COTA.0 && p.cota <= RANGO_COTA.1)).map(|p| p.punto.clone()).collect();

    let mut vistos = std::collections::HashSet::new();
    let mut rep_nom = std::collections::BTreeSet::new();
    for p in &d {
        if !vistos.insert(p.punto.clone()) {
            rep_nom.insert(p.punto.clone());
        }
    }
    rep.nombres_repetidos = rep_nom.into_iter().collect();

    // np.round: mitad al par
    let clave = |p: &Punto| ((p.este / tol_xy).round_ties_even() as i64, (p.norte / tol_xy).round_ties_even() as i64);
    let mut primero: HashMap<(i64, i64), usize> = HashMap::new();
    let mut quedan: Vec<Punto> = Vec::new();
    for p in &d {
        let c = clave(p);
        match primero.get(&c) {
            Some(&k) => {
                if (p.cota - quedan[k].cota).abs() <= tol_z {
                    rep.duplicados.push(p.punto.clone());
                } else {
                    rep.conflictos.push(p.punto.clone());
                }
            }
            None => {
                primero.insert(c, quedan.len());
                quedan.push((*p).clone());
            }
        }
    }
    let xyz: Vec<[f64; 3]> = quedan.iter().map(|p| [p.este, p.norte, p.cota]).collect();
    let idx = detectar_atipicos(&xyz, 8, 5.0, 0.5, 20);
    rep.atipicos = idx.iter().map(|&i| quedan[i].punto.clone()).collect();
    if quitar_atipicos && !idx.is_empty() {
        let fuera: std::collections::HashSet<usize> = idx.into_iter().collect();
        quedan = quedan.into_iter().enumerate().filter(|(i, _)| !fuera.contains(i)).map(|x| x.1).collect();
    }
    rep.n_salida = quedan.len();
    Ok((quedan, rep))
}

// ---------------------------------------------------------------- DXF ASCII

struct EntidadDxf {
    tipo: String,
    pares: Vec<(i32, String)>,
}

fn entidades_dxf(ruta: &Path) -> Result<Vec<EntidadDxf>, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("{}: {e}", nombre_archivo(ruta)))?;
    let texto = String::from_utf8_lossy(&bytes);
    let lineas: Vec<&str> = texto.lines().collect();
    let mut ents = Vec::new();
    let mut en_entidades = false;
    let mut i = 0;
    while i + 1 < lineas.len() {
        let codigo: i32 = lineas[i].trim().parse().map_err(|_| format!("{}: DXF ASCII inválido", nombre_archivo(ruta)))?;
        let valor = lineas[i + 1].trim().to_string();
        i += 2;
        if codigo == 2 && valor == "ENTITIES" {
            en_entidades = true;
            continue;
        }
        if !en_entidades {
            continue;
        }
        if codigo == 0 {
            if valor == "ENDSEC" {
                break;
            }
            ents.push(EntidadDxf { tipo: valor, pares: Vec::new() });
        } else if let Some(e) = ents.last_mut() {
            e.pares.push((codigo, valor));
        }
    }
    Ok(ents)
}

fn num(e: &EntidadDxf, codigo: i32) -> Option<f64> {
    e.pares.iter().find(|p| p.0 == codigo).and_then(|p| p.1.parse().ok())
}

fn capa(e: &EntidadDxf) -> &str {
    e.pares.iter().find(|p| p.0 == 8).map(|p| p.1.as_str()).unwrap_or("0")
}

/// Polilíneas del DXF en orden: (tipo, capa, vértices xyz, cerrada).
fn polilineas(ents: &[EntidadDxf]) -> Vec<(&'static str, String, Vec<[f64; 3]>, bool)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < ents.len() {
        let e = &ents[k];
        match e.tipo.as_str() {
            "LWPOLYLINE" => {
                let z = num(e, 38).unwrap_or(0.0);
                let (mut xs, mut ys) = (Vec::new(), Vec::new());
                for (c, v) in &e.pares {
                    match c {
                        10 => xs.push(v.parse().unwrap_or(f64::NAN)),
                        20 => ys.push(v.parse().unwrap_or(f64::NAN)),
                        _ => {}
                    }
                }
                let cerrada = num(e, 70).map_or(false, |f| (f as i64) & 1 == 1);
                out.push(("LWPOLYLINE", capa(e).into(), xs.iter().zip(&ys).map(|(&x, &y)| [x, y, z]).collect(), cerrada));
            }
            "POLYLINE" => {
                let cerrada = num(e, 70).map_or(false, |f| (f as i64) & 1 == 1);
                let mut v = Vec::new();
                let mut j = k + 1;
                while j < ents.len() && ents[j].tipo == "VERTEX" {
                    let x = &ents[j];
                    v.push([num(x, 10).unwrap_or(0.0), num(x, 20).unwrap_or(0.0), num(x, 30).unwrap_or(0.0)]);
                    j += 1;
                }
                out.push(("POLYLINE", capa(e).into(), v, cerrada));
                k = j;
                continue;
            }
            _ => {}
        }
        k += 1;
    }
    out
}

/// Puntos 3D de POINT y vértices de POLYLINE/LWPOLYLINE (en ese orden, como ezdxf).
pub fn leer_puntos_dxf(ruta: &Path, capa_filtro: Option<&str>) -> Result<Vec<[f64; 3]>, String> {
    let ents = entidades_dxf(ruta)?;
    let ok = |c: &str| capa_filtro.map_or(true, |f| f == c);
    let mut pts: Vec<[f64; 3]> = ents
        .iter()
        .filter(|e| e.tipo == "POINT" && ok(capa(e)))
        .map(|e| [num(e, 10).unwrap_or(0.0), num(e, 20).unwrap_or(0.0), num(e, 30).unwrap_or(0.0)])
        .collect();
    let pl = polilineas(&ents);
    for t in ["POLYLINE", "LWPOLYLINE"] {
        for (tipo, c, v, _) in &pl {
            if *tipo == t && ok(c) {
                pts.extend(v.iter().copied());
            }
        }
    }
    if pts.is_empty() {
        return Err(format!("No se encontraron puntos en {}", nombre_archivo(ruta)));
    }
    Ok(pts)
}

/// Polilínea de eje (planta): primera en la capa `EJE`, si no, la primera del archivo.
pub fn leer_polilinea(ruta: &Path, capa_filtro: Option<&str>) -> Result<Vec<[f64; 2]>, String> {
    let pl = polilineas(&entidades_dxf(ruta)?);
    let capas: Vec<Option<&str>> = match capa_filtro {
        Some(c) => vec![Some(c)],
        None => vec![Some("EJE"), None],
    };
    for c in capas {
        for (_, cp, v, cerrada) in &pl {
            if c.map_or(true, |x| x == cp) {
                let mut xy: Vec<[f64; 2]> = v.iter().map(|p| [p[0], p[1]]).collect();
                if *cerrada && !xy.is_empty() {
                    xy.push(xy[0]);
                }
                if xy.len() >= 2 {
                    return Ok(xy);
                }
            }
        }
    }
    Err(format!("No se encontró polilínea de eje en {}", nombre_archivo(ruta)))
}

// ---------------------------------------------------------------- XML (KML, GPX)

#[derive(Debug, Default)]
struct Nodo {
    nombre: String,
    atributos: HashMap<String, String>,
    texto: String,
    hijos: Vec<Nodo>,
}

impl Nodo {
    fn descendientes<'a>(&'a self, nombre: &str, out: &mut Vec<&'a Nodo>) {
        for h in &self.hijos {
            if h.nombre == nombre {
                out.push(h);
            }
            h.descendientes(nombre, out);
        }
    }

    fn todos(&self, nombre: &str) -> Vec<&Nodo> {
        let mut v = Vec::new();
        self.descendientes(nombre, &mut v);
        v
    }

    fn hijo_texto(&self, nombre: &str) -> String {
        self.hijos.iter().find(|h| h.nombre == nombre).map(|h| h.texto.trim().to_string()).unwrap_or_default()
    }
}

fn leer_xml(texto: &str) -> Result<Nodo, String> {
    use quick_xml::events::Event;
    let mut rd = quick_xml::Reader::from_str(texto);
    let mut pila: Vec<Nodo> = vec![Nodo::default()];
    let local = |s: &str| s.rsplit(':').next().unwrap_or("").to_string();
    let valor = |a: &quick_xml::events::attributes::Attribute| {
        a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|v| v.to_string()).unwrap_or_default()
    };
    loop {
        match rd.read_event().map_err(|e| format!("XML inválido: {e}"))? {
            Event::Start(e) => {
                let mut n = Nodo { nombre: local(e.name().as_ref()), ..Default::default() };
                for a in e.attributes().flatten() {
                    n.atributos.insert(local(a.key.as_ref()), valor(&a));
                }
                pila.push(n);
            }
            Event::Empty(e) => {
                let mut n = Nodo { nombre: local(e.name().as_ref()), ..Default::default() };
                for a in e.attributes().flatten() {
                    n.atributos.insert(local(a.key.as_ref()), valor(&a));
                }
                pila.last_mut().unwrap().hijos.push(n);
            }
            Event::Text(t) => pila.last_mut().unwrap().texto.push_str(&t.xml10_content()),
            Event::CData(t) => pila.last_mut().unwrap().texto.push_str(&t),
            Event::End(_) => {
                let n = pila.pop().unwrap();
                pila.last_mut().ok_or("XML mal cerrado")?.hijos.push(n);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut raiz = pila.pop().ok_or("XML vacío")?;
    Ok(if raiz.hijos.len() == 1 { raiz.hijos.remove(0) } else { raiz })
}

/// Puntos y rutas de un KML. Altitud 0 (sin relieve) = cota desconocida.
pub fn leer_kml(ruta: &Path, zona: Option<&str>) -> Result<(Vec<Punto>, Vec<Ruta>), String> {
    let raiz = leer_xml(&std::fs::read_to_string(ruta).map_err(|e| e.to_string())?)?;
    let mut pts: Vec<(String, [f64; 3])> = Vec::new();
    let mut geo: Vec<(String, Vec<[f64; 3]>)> = Vec::new();
    for pm in raiz.todos("Placemark") {
        let nombre = pm.hijo_texto("name");
        // orden del documento (como pm.iter() en Python)
        let mut orden = Vec::new();
        fn recorrer<'a>(n: &'a Nodo, out: &mut Vec<&'a Nodo>) {
            for h in &n.hijos {
                if h.nombre == "Point" || h.nombre == "LineString" {
                    out.push(h);
                }
                recorrer(h, out);
            }
        }
        recorrer(pm, &mut orden);
        for g in orden {
            let texto: Vec<String> = g.todos("coordinates").iter().map(|c| c.texto.clone()).collect();
            let coords: Vec<[f64; 3]> = texto
                .join(" ")
                .split_whitespace()
                .map(|c| {
                    let v: Vec<f64> = c.split(',').map(|x| x.parse().unwrap_or(f64::NAN)).collect();
                    [v[0], v[1], *v.get(2).unwrap_or(&0.0)]
                })
                .collect();
            if coords.is_empty() {
                continue;
            }
            if g.nombre == "Point" {
                pts.push((nombre.clone(), coords[0]));
            } else {
                geo.push((nombre.clone(), coords));
            }
        }
    }
    let todos: Vec<f64> = pts.iter().map(|p| p.1[0]).chain(geo.iter().flat_map(|g| g.1.iter().map(|c| c[0]))).collect();
    if todos.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let z_auto = zona_desde_longitud(todos.iter().sum::<f64>() / todos.len() as f64);
    let zona = zona.map(String::from).unwrap_or(z_auto);
    let cota = |v: f64| if v == 0.0 { f64::NAN } else { v };
    let mut out = Vec::new();
    for (n, c) in &pts {
        let (e, nn) = geograficas_a_utm(c[0], c[1], &zona)?;
        out.push(Punto { punto: n.clone(), este: e, norte: nn, cota: cota(c[2]), descripcion: "KML".into() });
    }
    let mut rutas = Vec::new();
    for (n, cs) in geo {
        let mut v = Vec::new();
        for c in cs {
            let (e, nn) = geograficas_a_utm(c[0], c[1], &zona)?;
            v.push([e, nn, cota(c[2])]);
        }
        rutas.push((n, v));
    }
    Ok((out, rutas))
}

/// Waypoints (puntos) y rutas/tracks de un GPX.
pub fn leer_gpx(ruta: &Path, zona: Option<&str>) -> Result<(Vec<Punto>, Vec<Ruta>), String> {
    let raiz = leer_xml(&std::fs::read_to_string(ruta).map_err(|e| e.to_string())?)?;
    let leer = |nodos: Vec<&Nodo>| -> Vec<(String, f64, f64, f64, String)> {
        nodos
            .iter()
            .map(|w| {
                let ele = w.hijo_texto("ele");
                let cmt = w.hijo_texto("cmt");
                (
                    w.hijo_texto("name"),
                    w.atributos.get("lon").and_then(|v| v.parse().ok()).unwrap_or(f64::NAN),
                    w.atributos.get("lat").and_then(|v| v.parse().ok()).unwrap_or(f64::NAN),
                    if ele.is_empty() { f64::NAN } else { ele.parse().unwrap_or(f64::NAN) },
                    if cmt.is_empty() { w.hijo_texto("desc") } else { cmt },
                )
            })
            .collect()
    };
    let wpt = leer(raiz.hijos.iter().filter(|n| n.nombre == "wpt").collect());
    let mut grupos: Vec<(String, Vec<(String, f64, f64, f64, String)>)> = raiz
        .hijos
        .iter()
        .filter(|n| n.nombre == "rte")
        .map(|r| (r.hijo_texto("name"), leer(r.todos("rtept"))))
        .collect();
    grupos.extend(raiz.hijos.iter().filter(|n| n.nombre == "trk").map(|t| (t.hijo_texto("name"), leer(t.todos("trkpt")))));
    let lons: Vec<f64> = wpt.iter().map(|w| w.1).chain(grupos.iter().flat_map(|g| g.1.iter().map(|p| p.1))).collect();
    if lons.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let zona = zona.map(String::from).unwrap_or(zona_desde_longitud(lons.iter().sum::<f64>() / lons.len() as f64));
    let mut pts = Vec::new();
    for w in &wpt {
        let (e, n) = geograficas_a_utm(w.1, w.2, &zona)?;
        pts.push(Punto { punto: w.0.clone(), este: e, norte: n, cota: w.3, descripcion: w.4.clone() });
    }
    let mut rutas = Vec::new();
    for (nombre, g) in grupos {
        if g.is_empty() {
            continue;
        }
        let mut v = Vec::new();
        for p in g {
            let (e, n) = geograficas_a_utm(p.1, p.2, &zona)?;
            v.push([e, n, p.3]);
        }
        rutas.push((nombre, v));
    }
    Ok((pts, rutas))
}

// ---------------------------------------------------------------- MapSource

fn leer_latin1(ruta: &Path) -> Result<String, String> {
    let b = std::fs::read(ruta).map_err(|e| format!("{}: {e}", nombre_archivo(ruta)))?;
    Ok(b.iter().map(|&c| c as char).collect())
}

pub fn es_mapsource(ruta: &Path) -> bool {
    leer_latin1(ruta).map(|t| t.starts_with("Grid")).unwrap_or(false)
}

/// Waypoints de un TXT de Garmin MapSource con `Grid UTM`.
pub fn leer_mapsource(ruta: &Path) -> Result<Vec<Punto>, String> {
    let mut out = Vec::new();
    for linea in leer_latin1(ruta)?.lines() {
        let c: Vec<&str> = linea.split('\t').collect();
        if c[0] != "Waypoint" || c.len() < 6 {
            continue;
        }
        // ^\s*(\d{1,2})\s*([C-X])\s+([\d.]+)\s+([\d.]+)\s*$  (sin distinguir mayúsculas)
        let parse = || -> Option<(f64, f64)> {
            let ch: Vec<char> = c[4].chars().collect();
            let mut i = 0;
            let salta = |i: &mut usize| {
                let ini = *i;
                while *i < ch.len() && ch[*i].is_whitespace() {
                    *i += 1;
                }
                *i - ini
            };
            salta(&mut i);
            let ini = i;
            while i < ch.len() && ch[i].is_ascii_digit() && i - ini < 2 {
                i += 1;
            }
            if i == ini {
                return None;
            }
            salta(&mut i);
            if i >= ch.len() || !('C'..='X').contains(&ch[i].to_ascii_uppercase()) {
                return None;
            }
            i += 1;
            let mut nums = Vec::new();
            for _ in 0..2 {
                if salta(&mut i) == 0 {
                    return None;
                }
                let ini = i;
                while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') {
                    i += 1;
                }
                if i == ini {
                    return None;
                }
                nums.push(ch[ini..i].iter().collect::<String>().parse::<f64>().ok()?);
            }
            salta(&mut i);
            (i == ch.len()).then_some((nums[0], nums[1]))
        };
        let Some((e, n)) = parse() else {
            return Err(format!("{}: posición no UTM ('{}'); exporte con Grid UTM", nombre_archivo(ruta), c[4]));
        };
        let alt: String = c[5].trim_start().chars().take_while(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == '-').collect();
        out.push(Punto {
            punto: c[1].into(),
            este: e,
            norte: n,
            cota: alt.parse().unwrap_or(f64::NAN),
            descripcion: c[2].into(),
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------- Excel RLT

fn celda_texto(c: &calamine::Data) -> Option<String> {
    use calamine::Data;
    match c {
        Data::Empty => None,
        Data::String(s) => Some(s.clone()),
        Data::Float(f) => Some(if f.fract() == 0.0 { format!("{f:.1}") } else { f.to_string() }),
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(if *b { "True".into() } else { "False".into() }),
        other => Some(other.to_string()),
    }
}

fn celda_num(c: &calamine::Data) -> f64 {
    use calamine::Data;
    match c {
        Data::Float(f) => *f,
        Data::Int(i) => *i as f64,
        Data::String(s) => a_float(s),
        _ => f64::NAN,
    }
}

/// (formato, encabezados, filas) de un Excel RLT (primera hoja).
fn rlt(ruta: &Path) -> Result<(String, Vec<String>, Vec<Vec<calamine::Data>>), String> {
    use calamine::Reader;
    let mut wb = calamine::open_workbook_auto(ruta).map_err(|e| format!("{}: {e}", nombre_archivo(ruta)))?;
    let rango = wb.worksheet_range_at(0).ok_or("Excel sin hojas")?.map_err(|e| e.to_string())?;
    let filas: Vec<Vec<calamine::Data>> = rango.rows().map(|r| r.to_vec()).collect();
    if filas.len() < 2 {
        return Err(format!("{}: Excel RLT sin datos", nombre_archivo(ruta)));
    }
    let formato = filas[0].get(3).and_then(celda_texto).unwrap_or_else(|| "nan".into()).trim().to_uppercase();
    let enc: Vec<String> = filas[1].iter().map(|c| celda_texto(c).unwrap_or_else(|| "nan".into())).collect();
    Ok((formato, enc, filas[2..].to_vec()))
}

/// Primera columna cuyo nombre (minúsculas, solo ASCII) contiene alguna clave.
fn col(enc: &[String], claves: &[&str]) -> Option<usize> {
    enc.iter().position(|c| {
        let n: String = c.to_lowercase().chars().filter(|c| c.is_ascii()).collect();
        claves.iter().any(|k| n.contains(k))
    })
}

/// Excel RLT formato UTM: puntos (todas las filas) y rutas (vértices de cada ruta).
pub fn leer_xls_utm(ruta: &Path) -> Result<(Vec<Punto>, Vec<Ruta>), String> {
    let (formato, enc, filas) = rlt(ruta)?;
    if formato != "UTM" {
        return Err(format!("{}: formato RLT '{formato}'; use leer_perfil_tabular", nombre_archivo(ruta)));
    }
    let get = |f: &Vec<calamine::Data>, k: Option<usize>| k.and_then(|k| f.get(k)).cloned().unwrap_or(calamine::Data::Empty);
    let (cn, cv, cx, cy, cz, cc) =
        (col(&enc, &["nombre"]), col(&enc, &["rtice"]), col(&enc, &["x"]), col(&enc, &["y"]), col(&enc, &["altitud"]), col(&enc, &["comentario"]));
    let mut nombre_actual: Option<String> = None;
    let mut nombres = Vec::new();
    let mut pts = Vec::new();
    let mut verts = Vec::new();
    for (i, f) in filas.iter().enumerate() {
        if let Some(n) = celda_texto(&get(f, cn)) {
            nombre_actual = Some(n);
        }
        let nombre = nombre_actual.clone().unwrap_or_else(|| "Ruta".into());
        let vert = celda_texto(&get(f, cv)).unwrap_or_default().trim().to_string();
        let com = celda_texto(&get(f, cc)).unwrap_or_default();
        pts.push(Punto {
            punto: if vert.is_empty() { (i + 1).to_string() } else { vert.clone() },
            este: celda_num(&get(f, cx)),
            norte: celda_num(&get(f, cy)),
            cota: celda_num(&get(f, cz)),
            descripcion: if com == "nan" { String::new() } else { com },
        });
        nombres.push(nombre);
        verts.push(!vert.is_empty());
    }
    let mut orden: Vec<String> = Vec::new();
    for n in &nombres {
        if !orden.contains(n) {
            orden.push(n.clone());
        }
    }
    let mut rutas = Vec::new();
    for nom in orden {
        let v: Vec<[f64; 3]> = (0..pts.len())
            .filter(|&i| nombres[i] == nom && verts[i])
            .map(|i| [pts[i].este, pts[i].norte, pts[i].cota])
            .collect();
        if v.len() >= 2 {
            rutas.push((nom, v));
        }
    }
    Ok((pts, rutas))
}

/// Ángulo en grados desde número o texto `-21°17'44"` (NaN si vacío o "-").
pub fn angulo_grados(v: &str) -> Result<f64, String> {
    let s = v.trim().replace(',', ".");
    if s.is_empty() || s == "-" {
        return Ok(f64::NAN);
    }
    if let Ok(x) = s.parse::<f64>() {
        return Ok(x);
    }
    // (-?)\s*(\d+(\.\d+)?)\D*((\d+(\.\d+)?)\D*)?((\d+(\.\d+)?)\D*)?$
    let ch: Vec<char> = s.chars().collect();
    let mut i = 0;
    let neg = ch.first() == Some(&'-');
    if neg {
        i += 1;
    }
    while i < ch.len() && ch[i].is_whitespace() {
        i += 1;
    }
    let mut numeros = Vec::new();
    while numeros.len() < 3 && i < ch.len() && ch[i].is_ascii_digit() {
        let ini = i;
        while i < ch.len() && ch[i].is_ascii_digit() {
            i += 1;
        }
        if i + 1 < ch.len() && ch[i] == '.' && ch[i + 1].is_ascii_digit() {
            i += 1;
            while i < ch.len() && ch[i].is_ascii_digit() {
                i += 1;
            }
        }
        numeros.push(ch[ini..i].iter().collect::<String>().parse::<f64>().unwrap());
        while i < ch.len() && !ch[i].is_ascii_digit() {
            i += 1;
        }
    }
    if numeros.is_empty() || i < ch.len() {
        return Err(format!("Ángulo no reconocido: {v:?}"));
    }
    let g = numeros[0] + numeros.get(1).unwrap_or(&0.0) / 60.0 + numeros.get(2).unwrap_or(&0.0) / 3600.0;
    Ok(if neg { -g } else { g })
}

/// Perfil y eje en planta reconstruido girando el rumbo el ángulo de cada vértice.
fn perfil_de_tabla(prog: &[f64], cota: &[f64], ang: &[f64], es_vertice: &[bool]) -> (Perfil, Vec<[f64; 2]>) {
    let n = prog.len();
    let mut ev = es_vertice.to_vec();
    ev[0] = true;
    ev[n - 1] = true;
    let sv: Vec<f64> = (0..n).filter(|&i| ev[i]).map(|i| prog[i]).collect();
    let av: Vec<f64> = (0..n).filter(|&i| ev[i]).map(|i| if ang[i].is_nan() { 0.0 } else { ang[i] }).collect();
    let mut rumbo = 0.0f64;
    let mut eje = vec![[0.0, 0.0]];
    for k in 1..sv.len() {
        if k > 1 {
            rumbo -= av[k - 1].to_radians(); // deflexión + = izquierda
        }
        let d = sv[k] - sv[k - 1];
        let u = eje[k - 1];
        eje.push([u[0] + d * rumbo.sin(), u[1] + d * rumbo.cos()]);
    }
    let xs: Vec<f64> = eje.iter().map(|p| p[0]).collect();
    let ys: Vec<f64> = eje.iter().map(|p| p[1]).collect();
    let p = Perfil {
        progresiva: prog.to_vec(),
        este: prog.iter().map(|&s| interp(s, &sv, &xs)).collect(),
        norte: prog.iter().map(|&s| interp(s, &sv, &ys)).collect(),
        cota: cota.to_vec(),
        tipo: ev.iter().map(|&v| if v { VERTICE } else { ESTACION }).collect(),
    };
    (p, eje)
}

/// Perfil desde Excel RLT ACUMULADA/ANGULO o `.tpg`.
pub fn leer_perfil_tabular(ruta: &Path) -> Result<(Perfil, Vec<[f64; 2]>), String> {
    let ext = ruta.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext == "tpg" {
        let texto = leer_latin1(ruta)?;
        let mut lineas: Vec<Vec<&str>> = texto.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split('\t').collect()).collect();
        let es_num = |s: &str| {
            let t = s.trim();
            let t = t.strip_prefix('-').unwrap_or(t);
            !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.')
        };
        if !lineas.is_empty() && !es_num(lineas[0][0]) {
            lineas.remove(0);
        }
        let prog: Vec<f64> = lineas.iter().map(|l| a_float(l[0])).collect();
        let cota: Vec<f64> = lineas.iter().map(|l| a_float(l[1])).collect();
        let ang: Vec<f64> =
            lineas.iter().map(|l| if l.len() > 3 { angulo_grados(l[3]) } else { Ok(f64::NAN) }).collect::<Result<_, _>>()?;
        let vert: Vec<bool> = lineas.iter().map(|l| l.len() > 4 && l[4].trim().to_lowercase().starts_with('v')).collect();
        return Ok(perfil_de_tabla(&prog, &cota, &ang, &vert));
    }
    let (formato, enc, mut filas) = rlt(ruta)?;
    if !formato.contains("ACUMULADA") {
        return Err(format!("{}: formato RLT '{formato}' no es ACUMULADA/ANGULO", nombre_archivo(ruta)));
    }
    let cn = col(&enc, &["nombre"]);
    let con_nombre: Vec<usize> =
        (0..filas.len()).filter(|&i| cn.and_then(|k| filas[i].get(k)).and_then(celda_texto).is_some()).collect();
    if con_nombre.len() > 1 {
        filas.truncate(con_nombre[1]); // solo la primera ruta del archivo
    }
    let get = |f: &Vec<calamine::Data>, k: Option<usize>| k.and_then(|k| f.get(k)).cloned().unwrap_or(calamine::Data::Empty);
    let (ca, cz, cg, cv) = (col(&enc, &["acumulada"]), col(&enc, &["altitud"]), col(&enc, &["ngulo"]), col(&enc, &["rtice"]));
    let prog: Vec<f64> = filas.iter().map(|f| celda_num(&get(f, ca))).collect();
    let cota: Vec<f64> = filas.iter().map(|f| celda_num(&get(f, cz))).collect();
    let ang: Vec<f64> = filas
        .iter()
        .map(|f| match get(f, cg) {
            calamine::Data::Empty => Ok(f64::NAN),
            calamine::Data::Float(x) => Ok(x),
            calamine::Data::Int(x) => Ok(x as f64),
            d => angulo_grados(&celda_texto(&d).unwrap_or_default()),
        })
        .collect::<Result<_, _>>()?;
    let vert: Vec<bool> = filas.iter().map(|f| !celda_texto(&get(f, cv)).unwrap_or_default().trim().is_empty()).collect();
    Ok(perfil_de_tabla(&prog, &cota, &ang, &vert))
}

/// Puntos y rutas de KML, GPX, TXT MapSource o Excel RLT UTM.
pub fn importar(ruta: &Path, zona: Option<&str>) -> Result<(Vec<Punto>, Vec<Ruta>), String> {
    let ext = ruta.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "kml" => leer_kml(ruta, zona),
        "gpx" => leer_gpx(ruta, zona),
        "txt" if es_mapsource(ruta) => Ok((leer_mapsource(ruta)?, Vec::new())),
        "xls" | "xlsx" => leer_xls_utm(ruta),
        _ => Err(format!("Formato no soportado para importar: .{ext}")),
    }
}

/// Lee puntos según la extensión (como `datos.leer`).
pub fn leer(ruta: &Path) -> Result<Vec<Punto>, String> {
    let ext = ruta.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "dxf" => Ok(leer_puntos_dxf(ruta, None)?
            .into_iter()
            .enumerate()
            .map(|(i, p)| Punto { punto: (i + 1).to_string(), este: p[0], norte: p[1], cota: p[2], descripcion: "DXF".into() })
            .collect()),
        "kml" | "gpx" | "xls" | "xlsx" => Ok(importar(ruta, None)?.0),
        "txt" if es_mapsource(ruta) => Ok(importar(ruta, None)?.0),
        "csv" | "txt" => {
            let b = std::fs::read(ruta).map_err(|e| format!("{}: {e}", nombre_archivo(ruta)))?;
            leer_csv_texto(&String::from_utf8_lossy(&b), &nombre_archivo(ruta), &["este", "norte", "cota"])
        }
        otro => Err(format!("Formato no soportado: .{otro}")),
    }
}

