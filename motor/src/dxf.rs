//! Planos DXF: plano y perfil de TopoCraft, perfil de LineCraft y plano de red de GridCraft.
//!
//! Port de `topocraft/salidas.py`, `linecraft/salidas.py` y `gridcraft/cli.py` (mismas capas,
//! colores, textos y geometría). Se escribe **DXF R12 ASCII** (AC1009, sin handles), que abren
//! AutoCAD, BricsCAD, LibreCAD, QGIS y ezdxf. Diferencia intencional con Python (ezdxf R2010):
//! las polilíneas se escriben como POLYLINE 2D (con elevación) en lugar de LWPOLYLINE, que no
//! existe en R12. Ver `docs/DIFERENCIAS_RUST.md`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::entrada::Punto;
use crate::geo::Perfil;
use crate::linea::{Proyecto, Resultado};
use crate::numerico::formato_g;
use crate::red::{Red, ResultadoRed};
use crate::salidas::{cota_referencia, curvas_vanos, formato_progresiva, principales, tramos_validos};
use crate::tin::{es_mayor, Curva, Tin};

/// Documento DXF R12 en construcción.
#[derive(Default)]
pub struct Dxf {
    capas: Vec<(String, i32)>,
    ent: String,
    /// Extensión del dibujo [xmin, ymin, xmax, ymax] (para encuadrar la lámina).
    ext: Option<[f64; 4]>,
}

/// Texto DXF: caracteres no ASCII como `\U+XXXX` (convención de AutoCAD).
fn texto_dxf(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii() { c.to_string() } else { format!("\\U+{:04X}", c as u32) }).collect()
}

fn num(x: f64) -> String {
    let s = format!("{x}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

impl Dxf {
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Crea la capa si no existe (color ACI).
    pub fn capa(&mut self, nombre: &str, color: i32) {
        if !self.capas.iter().any(|c| c.0 == nombre) {
            self.capas.push((nombre.into(), color));
        }
    }

    fn par(&mut self, codigo: i32, valor: &str) {
        let _ = write!(self.ent, "{codigo:>3}\n{valor}\n");
    }

    fn xyz(&mut self, base: i32, p: [f64; 3]) {
        self.par(base, &num(p[0]));
        self.par(base + 10, &num(p[1]));
        self.par(base + 20, &num(p[2]));
    }

    fn ampliar(&mut self, x: f64, y: f64) {
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        let e = self.ext.get_or_insert([x, y, x, y]);
        *e = [e[0].min(x), e[1].min(y), e[2].max(x), e[3].max(y)];
    }

    fn inicio(&mut self, tipo: &str, capa: &str) {
        self.par(0, tipo);
        self.par(8, capa);
    }

    pub fn punto(&mut self, capa: &str, p: [f64; 3]) {
        self.inicio("POINT", capa);
        self.xyz(10, p);
        self.ampliar(p[0], p[1]);
    }

    pub fn linea(&mut self, capa: &str, a: [f64; 2], b: [f64; 2]) {
        self.inicio("LINE", capa);
        self.xyz(10, [a[0], a[1], 0.0]);
        self.xyz(11, [b[0], b[1], 0.0]);
        self.ampliar(a[0], a[1]);
        self.ampliar(b[0], b[1]);
    }

    pub fn circulo(&mut self, capa: &str, c: [f64; 2], radio: f64) {
        self.inicio("CIRCLE", capa);
        self.xyz(10, [c[0], c[1], 0.0]);
        self.par(40, &num(radio));
        self.ampliar(c[0] - radio, c[1] - radio);
        self.ampliar(c[0] + radio, c[1] + radio);
    }

    /// Texto alineado a la izquierda en `p` (`set_placement` por defecto de ezdxf).
    pub fn texto(&mut self, capa: &str, p: [f64; 2], altura: f64, rotacion: f64, texto: &str) {
        self.inicio("TEXT", capa);
        self.xyz(10, [p[0], p[1], 0.0]);
        self.ampliar(p[0], p[1]);
        self.par(40, &num(altura));
        self.par(1, &texto_dxf(texto));
        if rotacion != 0.0 {
            self.par(50, &num(rotacion));
        }
    }

    pub fn cara3d(&mut self, capa: &str, v: [[f64; 3]; 4]) {
        self.inicio("3DFACE", capa);
        for (k, p) in v.iter().enumerate() {
            self.xyz(10 + k as i32, *p);
            self.ampliar(p[0], p[1]);
        }
    }

    /// Polilínea 2D con elevación (`LWPOLYLINE` de ezdxf → `POLYLINE` en R12).
    pub fn polilinea(&mut self, capa: &str, xy: &[[f64; 2]], cerrada: bool, elevacion: f64) {
        self.inicio("POLYLINE", capa);
        self.par(66, "1");
        self.xyz(10, [0.0, 0.0, elevacion]);
        if cerrada {
            self.par(70, "1");
        }
        for p in xy {
            self.inicio("VERTEX", capa);
            self.xyz(10, [p[0], p[1], elevacion]);
            self.ampliar(p[0], p[1]);
        }
        self.inicio("SEQEND", capa);
    }

    /// Texto completo del archivo.
    pub fn contenido(&self) -> String {
        let mut s = String::new();
        let mut p = |c: i32, v: &str| {
            let _ = write!(s, "{c:>3}\n{v}\n");
        };
        p(0, "SECTION");
        p(2, "HEADER");
        p(9, "$ACADVER");
        p(1, "AC1009");
        p(9, "$DWGCODEPAGE");
        p(3, "ANSI_1252");
        p(9, "$INSBASE");
        p(10, "0.0");
        p(20, "0.0");
        p(30, "0.0");
        p(0, "ENDSEC");
        p(0, "SECTION");
        p(2, "TABLES");
        p(0, "TABLE");
        p(2, "LTYPE");
        p(70, "1");
        p(0, "LTYPE");
        p(2, "CONTINUOUS");
        p(70, "0");
        p(3, "Solid line");
        p(72, "65");
        p(73, "0");
        p(40, "0.0");
        p(0, "ENDTAB");
        p(0, "TABLE");
        p(2, "LAYER");
        p(70, &(self.capas.len() + 1).to_string());
        for (nombre, color) in std::iter::once(&("0".to_string(), 7)).chain(self.capas.iter()) {
            p(0, "LAYER");
            p(2, nombre);
            p(70, "0");
            p(62, &color.to_string());
            p(6, "CONTINUOUS");
        }
        p(0, "ENDTAB");
        p(0, "TABLE");
        p(2, "STYLE");
        p(70, "1");
        p(0, "STYLE");
        p(2, "STANDARD");
        p(70, "0");
        p(40, "0.0");
        p(41, "1.0");
        p(50, "0.0");
        p(71, "0");
        p(42, "2.5");
        p(3, "txt");
        p(4, "");
        p(0, "ENDTAB");
        p(0, "ENDSEC");
        p(0, "SECTION");
        p(2, "ENTITIES");
        s.push_str(&self.ent);
        let mut p = |c: i32, v: &str| {
            let _ = write!(s, "{c:>3}\n{v}\n");
        };
        p(0, "ENDSEC");
        p(0, "EOF");
        s
    }

    pub fn guardar(&self, ruta: &Path) -> Result<PathBuf, String> {
        if let Some(d) = ruta.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(ruta, self.contenido()).map_err(|e| format!("{}: {e}", ruta.display()))?;
        Ok(ruta.to_path_buf())
    }
}

/// Capas de TopoCraft y su color ACI.
pub const CAPAS_TOPO: [(&str, i32); 6] =
    [("PUNTOS", 7), ("TIN", 8), ("CURVAS_MAYORES", 30), ("CURVAS_MENORES", 33), ("EJE", 1), ("PERFIL", 3)];

/// Plano topográfico (UTM, m): PUNTOS, TIN (3DFACE), CURVAS_MAYORES/MENORES (con elevación), EJE.
#[allow(clippy::too_many_arguments)]
pub fn plano_topo(
    ruta: &Path, puntos: &[Punto], tin: Option<&Tin>, curvas: &[Curva], equidistancia: f64, mayor_cada: i64,
    eje: Option<&[[f64; 2]]>, altura_texto: f64,
) -> Result<PathBuf, String> {
    construir_plano_topo(puntos, tin, curvas, equidistancia, mayor_cada, eje, altura_texto).guardar(ruta)
}

#[allow(clippy::too_many_arguments)]
pub fn construir_plano_topo(
    puntos: &[Punto], tin: Option<&Tin>, curvas: &[Curva], equidistancia: f64, mayor_cada: i64,
    eje: Option<&[[f64; 2]]>, altura_texto: f64,
) -> Dxf {
    let mut d = Dxf::nuevo();
    for (c, col) in CAPAS_TOPO.iter().filter(|c| c.0 != "PERFIL") {
        d.capa(c, *col);
    }
    let h = altura_texto;
    for p in puntos {
        d.punto("PUNTOS", [p.este, p.norte, p.cota]);
        d.texto("PUNTOS", [p.este + 0.5 * h, p.norte + 0.2 * h], h, 0.0, &format!("{:.2}", p.cota));
    }
    if let Some(tin) = tin {
        for (k, tr) in tin.triangulos.iter().enumerate().filter(|(k, _)| tin.activos[*k]) {
            let _ = k;
            let v = tr.map(|i| tin.xyz[i]);
            d.cara3d("TIN", [v[0], v[1], v[2], v[2]]);
        }
    }
    for c in curvas {
        let mayor = es_mayor(c.cota, equidistancia, mayor_cada);
        let capa = if mayor { "CURVAS_MAYORES" } else { "CURVAS_MENORES" };
        d.polilinea(capa, &c.xy, c.cerrada, c.cota);
        if mayor && c.xy.len() >= 2 {
            let i = c.xy.len() / 2;
            let (a, b) = (c.xy[i - 1], c.xy[i]);
            let mut ang = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
            ang = if ang > 90.0 { ang - 180.0 } else if ang < -90.0 { ang + 180.0 } else { ang };
            d.texto(capa, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], 1.5 * h, ang, &formato_g(c.cota));
        }
    }
    if let Some(eje) = eje {
        d.polilinea("EJE", eje, false, 0.0);
    }
    d
}

/// Altura de texto del perfil según su longitud.
fn altura_perfil(l: f64) -> f64 {
    (l / 250.0).max(0.5).min(5.0)
}

/// Perfil de TopoCraft: x = progresiva, y = (cota − cota_ref) × exageración (capa PERFIL).
pub fn perfil_topo(ruta: &Path, perfil: &Perfil, exageracion: f64) -> Result<PathBuf, String> {
    construir_perfil_topo(perfil, exageracion).guardar(ruta)
}

pub fn construir_perfil_topo(perfil: &Perfil, exageracion: f64) -> Dxf {
    let mut d = Dxf::nuevo();
    d.capa("PERFIL", 3);
    let refz = cota_referencia(perfil);
    let l = perfil.longitud();
    let h = altura_perfil(l);
    for tr in tramos_validos(&perfil.progresiva, &perfil.cota) {
        let xy: Vec<[f64; 2]> = tr.iter().map(|p| [p[0], (p[1] - refz) * exageracion]).collect();
        d.polilinea("PERFIL", &xy, false, 0.0);
    }
    d.linea("PERFIL", [0.0, 0.0], [l, 0.0]);
    d.texto("PERFIL", [0.0, -9.0 * h], h, 0.0, &format!("COTA REF. {refz:.2} m   EXAG. V {}x", formato_g(exageracion)));
    for (s, z, _) in principales(perfil) {
        let ytop = if z.is_nan() { 0.0 } else { (z - refz) * exageracion };
        d.linea("PERFIL", [s, 0.0], [s, ytop]);
        d.texto("PERFIL", [s + 0.4 * h, -7.0 * h], h, 90.0, &formato_progresiva(s, 2));
        if !z.is_nan() {
            d.texto("PERFIL", [s + 0.4 * h, -2.5 * h], h, 90.0, &format!("{z:.2}"));
        }
    }
    d
}

/// Perfil de LineCraft: terreno, postes, conductor (flecha máxima) y despeje mínimo.
pub fn perfil_linea(ruta: &Path, r: &Resultado, p: &Proyecto, exageracion: f64) -> Result<PathBuf, String> {
    construir_perfil_linea(r, p, exageracion).guardar(ruta)
}

pub fn construir_perfil_linea(r: &Resultado, p: &Proyecto, exageracion: f64) -> Dxf {
    let mut d = Dxf::nuevo();
    for (c, col) in [("PERFIL", 3), ("POSTES", 4), ("CONDUCTOR", 1), ("DESPEJE", 6), ("TEXTO_LC", 7)] {
        d.capa(c, col);
    }
    let refz = cota_referencia(&p.perfil);
    let y = |z: f64| (z - refz) * exageracion;
    let h = altura_perfil(p.perfil.longitud());
    for tr in tramos_validos(&p.perfil.progresiva, &p.perfil.cota) {
        d.polilinea("PERFIL", &tr.iter().map(|q| [q[0], y(q[1])]).collect::<Vec<_>>(), false, 0.0);
    }
    for e in &r.estructuras {
        d.linea("POSTES", [e.progresiva, y(e.cota_terreno)], [e.progresiva, y(e.cota_amarre())]);
        d.texto("TEXTO_LC", [e.progresiva - 0.5 * h, y(e.cota_amarre()) + h], h, 90.0,
            &format!("{} {}", e.codigo(), e.tipo.como_str()));
        d.texto("TEXTO_LC", [e.progresiva + 0.4 * h, -8.0 * h], h, 90.0, &formato_progresiva(e.progresiva, 2));
    }
    for (capa, desp) in [("CONDUCTOR", 0.0), ("DESPEJE", p.distancia_minima_terreno)] {
        for (s, z) in curvas_vanos(r, p, desp) {
            d.polilinea(capa, &s.iter().zip(&z).map(|(a, b)| [*a, y(*b)]).collect::<Vec<_>>(), false, 0.0);
        }
    }
    d.texto("TEXTO_LC", [0.0, -11.0 * h], h, 0.0, &format!(
        "COTA REF. {refz:.2} m   EXAG. V {}x   CONDUCTOR: {}   DESPEJE MIN. {:.2} m",
        formato_g(exageracion), p.hip_flecha_maxima, p.distancia_minima_terreno));
    d
}

/// Plano de la red (RED, NODOS, TEXTO_RED, ALERTAS_RED); `None` si algún nodo no tiene coordenadas.
pub fn plano_red(ruta: &Path, red: &Red, r: &ResultadoRed, alto_texto: f64) -> Result<Option<PathBuf>, String> {
    match construir_plano_red(red, r, alto_texto)? {
        Some(d) => Ok(Some(d.guardar(ruta)?)),
        None => Ok(None),
    }
}

pub fn construir_plano_red(red: &Red, r: &ResultadoRed, alto_texto: f64) -> Result<Option<Dxf>, String> {
    let mut pos = Vec::new();
    for n in &red.nodos {
        match (n.este, n.norte) {
            (Some(x), Some(y)) => pos.push((n.nodo.clone(), [x, y])),
            _ => return Ok(None),
        }
    }
    let xy = |nodo: &str| pos.iter().find(|p| p.0 == nodo).map(|p| p.1).ok_or(format!("nodo {nodo} sin coordenadas"));
    let mut d = Dxf::nuevo();
    for (c, col) in [("RED", 1), ("NODOS", 3), ("TEXTO_RED", 7), ("ALERTAS_RED", 1)] {
        d.capa(c, col);
    }
    for t in &r.tramos {
        let (a, b) = (xy(&t.padre)?, xy(&t.nodo)?);
        d.linea("RED", a, b);
        d.texto("TEXTO_RED", [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], alto_texto * 0.7, 0.0,
            &format!("{} {:.0}m", t.formacion, t.longitud_m));
    }
    for (nodo, [x, y]) in &pos {
        let malo = r.alertas.iter().any(|a| &a.elemento == nodo);
        d.circulo(if malo { "ALERTAS_RED" } else { "NODOS" }, [*x, *y], alto_texto * 0.6);
        let txt = match r.tramos.iter().find(|t| &t.nodo == nodo) {
            Some(t) => format!("{nodo} ({:.2}%)", t.dv_pct),
            None => nodo.clone(),
        };
        d.texto("TEXTO_RED", [x + alto_texto, y + alto_texto], alto_texto, 0.0, &txt);
    }
    Ok(Some(d))
}

// ------------------------------------------------------------------ láminas de entrega

/// Datos del cajetín de una lámina.
#[derive(Debug, Clone, Default)]
pub struct Cajetin {
    pub proyecto: String,
    pub titulo: String,
    pub autor: String,
    /// Código de lámina, p. ej. `L-01`.
    pub lamina: String,
    /// Nota libre (p. ej. la exageración vertical de un perfil).
    pub nota: String,
}

/// Escalas normalizadas (1:n) que se prueban, de la mayor a la menor.
const ESCALAS: [f64; 17] =
    [50.0, 100.0, 200.0, 250.0, 500.0, 750.0, 1000.0, 1250.0, 1500.0, 2000.0, 2500.0, 5000.0, 7500.0, 10000.0, 20000.0, 25000.0, 50000.0];
/// Formatos ISO apaisados (mm). A3 hasta 1:5000; si no entra, A1.
const FORMATOS: [(&str, f64, f64, f64); 2] = [("A3", 420.0, 297.0, 5000.0), ("A1", 841.0, 594.0, f64::INFINITY)];

/// Formato y escala 1:n en que entra un dibujo de `ancho` × `alto` m (márgenes y cajetín aparte).
pub fn elegir_lamina(ancho: f64, alto: f64) -> (&'static str, f64, f64, f64) {
    for (f, w, h, max) in FORMATOS {
        let (aw, ah) = (w - 35.0, h - 65.0); // margen 25 (encuadernación) + 10; 10 + cajetín 45 + 10
        if let Some(e) = ESCALAS.iter().find(|e| **e <= max && ancho * 1000.0 / **e <= aw && alto * 1000.0 / **e <= ah) {
            return (f, w, h, *e);
        }
    }
    let (f, w, h, _) = FORMATOS[1];
    (f, w, h, ESCALAS[ESCALAS.len() - 1])
}

/// Fecha de hoy `dd/mm/aaaa` (UTC), sin dependencias.
pub fn fecha_hoy() -> String {
    let dias = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    let (a, m, d) = fecha_civil(dias);
    format!("{d:02}/{m:02}/{a}")
}

/// Días desde 1970-01-01 → (año, mes, día) (algoritmo de H. Hinnant).
fn fecha_civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

impl Dxf {
    /// Encierra el dibujo en una lámina (marco, margen de encuadernación y cajetín) en la capa
    /// LAMINA, a la escala normalizada que lo encuadra. Devuelve (formato, 1:n).
    pub fn lamina(&mut self, c: &Cajetin) -> Option<(&'static str, f64)> {
        let [x0, y0, x1, y1] = self.ext?;
        let (f, w, h, e) = elegir_lamina(x1 - x0, y1 - y0);
        let k = e / 1000.0; // m de modelo por mm de papel
        // centro del área útil del papel → centro del dibujo
        let (pcx, pcy) = ((25.0 + w - 10.0) / 2.0, (55.0 + h - 10.0) / 2.0);
        let o = [(x0 + x1) / 2.0 - pcx * k, (y0 + y1) / 2.0 - pcy * k];
        let q = |x: f64, y: f64| [o[0] + x * k, o[1] + y * k];
        self.capa("LAMINA", 7);
        let rect = |d: &mut Dxf, a: (f64, f64), b: (f64, f64)| {
            d.polilinea("LAMINA", &[q(a.0, a.1), q(b.0, a.1), q(b.0, b.1), q(a.0, b.1)], true, 0.0);
        };
        rect(self, (0.0, 0.0), (w, h));
        rect(self, (25.0, 10.0), (w - 10.0, h - 10.0));
        // cajetín 180 × 45 mm en la esquina inferior derecha
        let (cx0, cx1) = (w - 190.0, w - 10.0);
        rect(self, (cx0, 10.0), (cx1, 55.0));
        for y in [19.0, 28.0, 37.0, 46.0] {
            self.linea("LAMINA", q(cx0, y), q(cx1, y));
        }
        self.linea("LAMINA", q(cx1 - 50.0, 10.0), q(cx1 - 50.0, 28.0));
        let t = |d: &mut Dxf, x: f64, y: f64, alto: f64, s: &str| d.texto("LAMINA", q(x, y), alto * k, 0.0, s);
        t(self, cx0 + 2.0, 48.5, 3.5, &format!("PROYECTO: {}", c.proyecto));
        t(self, cx0 + 2.0, 39.5, 3.5, &format!("PLANO: {}", c.titulo));
        t(self, cx0 + 2.0, 30.5, 2.5, &format!("AUTOR: {}", c.autor));
        t(self, cx0 + 2.0, 21.5, 2.5, &format!("ESCALA 1:{}   FORMATO {f}   {}", formato_g(e), c.nota));
        t(self, cx0 + 2.0, 12.5, 2.5, &format!("FECHA: {}", fecha_hoy()));
        t(self, cx1 - 48.0, 21.5, 2.5, "LÁMINA");
        t(self, cx1 - 48.0, 12.0, 5.0, &c.lamina);
        t(self, 27.0, 12.0, 2.0, "Suite Eléctrica - software libre (MIT). Verificar valores normativos antes de usar.");
        Some((f, e))
    }
}

#[cfg(test)]
mod prueba_lamina {
    use super::*;

    #[test]
    fn escala_formato_y_fecha() {
        assert_eq!(fecha_civil(0), (1970, 1, 1));
        assert_eq!(fecha_civil(20_000), (2024, 10, 4));
        assert_eq!(fecha_civil(11_016), (2000, 2, 29));
        let mut d = Dxf::nuevo();
        d.linea("X", [1000.0, 5000.0], [1800.0, 5300.0]); // 800 × 300 m
        assert_eq!(d.lamina(&Cajetin::default()), Some(("A3", 2500.0)));
        let e = d.ext.unwrap();
        // el marco (420 × 297 mm a 1:2500 = 1050 × 742,5 m) contiene el dibujo
        assert!((e[2] - e[0] - 1050.0).abs() < 1e-6 && (e[3] - e[1] - 742.5).abs() < 1e-6);
        assert!(e[0] <= 1000.0 && e[2] >= 1800.0 && e[1] <= 5000.0 && e[3] >= 5300.0);
        assert_eq!(elegir_lamina(20_000.0, 5_000.0).0, "A1"); // en A3 pasaría de 1:5000
        assert!(Dxf::nuevo().lamina(&Cajetin::default()).is_none());
    }
}
