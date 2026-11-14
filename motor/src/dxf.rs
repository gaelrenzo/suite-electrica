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

    fn inicio(&mut self, tipo: &str, capa: &str) {
        self.par(0, tipo);
        self.par(8, capa);
    }

    pub fn punto(&mut self, capa: &str, p: [f64; 3]) {
        self.inicio("POINT", capa);
        self.xyz(10, p);
    }

    pub fn linea(&mut self, capa: &str, a: [f64; 2], b: [f64; 2]) {
        self.inicio("LINE", capa);
        self.xyz(10, [a[0], a[1], 0.0]);
        self.xyz(11, [b[0], b[1], 0.0]);
    }

    pub fn circulo(&mut self, capa: &str, c: [f64; 2], radio: f64) {
        self.inicio("CIRCLE", capa);
        self.xyz(10, [c[0], c[1], 0.0]);
        self.par(40, &num(radio));
    }

    /// Texto alineado a la izquierda en `p` (`set_placement` por defecto de ezdxf).
    pub fn texto(&mut self, capa: &str, p: [f64; 2], altura: f64, rotacion: f64, texto: &str) {
        self.inicio("TEXT", capa);
        self.xyz(10, [p[0], p[1], 0.0]);
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
    d.guardar(ruta)
}

/// Altura de texto del perfil según su longitud.
fn altura_perfil(l: f64) -> f64 {
    (l / 250.0).max(0.5).min(5.0)
}

/// Perfil de TopoCraft: x = progresiva, y = (cota − cota_ref) × exageración (capa PERFIL).
pub fn perfil_topo(ruta: &Path, perfil: &Perfil, exageracion: f64) -> Result<PathBuf, String> {
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
    d.guardar(ruta)
}

/// Perfil de LineCraft: terreno, postes, conductor (flecha máxima) y despeje mínimo.
pub fn perfil_linea(ruta: &Path, r: &Resultado, p: &Proyecto, exageracion: f64) -> Result<PathBuf, String> {
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
    d.guardar(ruta)
}

/// Plano de la red (RED, NODOS, TEXTO_RED, ALERTAS_RED); `None` si algún nodo no tiene coordenadas.
pub fn plano_red(ruta: &Path, red: &Red, r: &ResultadoRed, alto_texto: f64) -> Result<Option<PathBuf>, String> {
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
    Ok(Some(d.guardar(ruta)?))
}
