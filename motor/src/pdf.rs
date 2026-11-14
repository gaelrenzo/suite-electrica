//! Memorias PDF: memoria de cálculo de LineCraft y perfil a escala de TopoCraft.
//!
//! Port de `linecraft.salidas.memoria_pdf` (reportlab) y `topocraft.salidas.perfil_pdf`
//! (matplotlib): mismo contenido (títulos, párrafos, tablas, textos del perfil) y mismas
//! dimensiones de página. Diferencias intencionales (ver `docs/DIFERENCIAS_RUST.md`): fuente
//! DejaVu Sans embebida, el perfil de la memoria se dibuja en vectores (no como imagen PNG) y
//! las tablas anchas reducen su letra para caber en la página.

use std::path::{Path, PathBuf};

use printpdf::{
    Color, FontId, Line, LineDashPattern, LinePoint, Op, PaintMode, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rgb, TextItem, TextMatrix, WindingOrder,
};

use crate::geo::Perfil;
use crate::linea::{Proyecto, Resultado};
use crate::numerico::formato_g;
use crate::salidas::{cota_referencia, curvas_vanos, cumple, formato_progresiva, principales, tablas_linea, tramos_validos, Extras, Tabla};

const NORMAL: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
const NEGRITA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");
const MM: f32 = 72.0 / 25.4;

struct Fuente {
    id: FontId,
    pf: ParsedFont,
}

impl Fuente {
    fn ancho(&self, s: &str, tam: f32) -> f32 {
        let upm = self.pf.units_per_em.max(1) as f32;
        let w: u32 = s
            .chars()
            .map(|c| {
                let g = self.pf.codepoint_to_glyph.get(&(c as u32)).copied().unwrap_or(0);
                self.pf.glyph_widths.get(&g).copied().unwrap_or(0) as u32
            })
            .sum();
        w as f32 / upm * tam
    }
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::Rgb(Rgb { r, g, b, icc_profile: None })
}

fn hex(h: u32) -> Color {
    rgb(((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0)
}

fn gris(v: f32) -> Color {
    rgb(v, v, v)
}

fn pt(x: f32, y: f32) -> LinePoint {
    LinePoint { p: Point { x: Pt(x), y: Pt(y) }, bezier: false }
}

/// Lienzo de una página (operaciones en puntos, origen abajo a la izquierda).
struct Hoja {
    ops: Vec<Op>,
}

impl Hoja {
    fn nueva() -> Self {
        Hoja { ops: Vec::new() }
    }

    fn trazo(&mut self, color: Color, grosor: f32, discontinua: bool) {
        self.ops.push(Op::SetOutlineColor { col: color });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(grosor) });
        self.ops.push(Op::SetLineDashPattern {
            dash: if discontinua { LineDashPattern::new(0.0, &[3.0, 2.0]) } else { LineDashPattern::solid() },
        });
    }

    fn polilinea(&mut self, xy: &[(f32, f32)], color: Color, grosor: f32, discontinua: bool) {
        if xy.len() < 2 {
            return;
        }
        self.trazo(color, grosor, discontinua);
        self.ops.push(Op::DrawLine { line: Line { points: xy.iter().map(|&(x, y)| pt(x, y)).collect(), is_closed: false } });
    }

    fn linea(&mut self, a: (f32, f32), b: (f32, f32), color: Color, grosor: f32) {
        self.polilinea(&[a, b], color, grosor, false);
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, borde: Option<(Color, f32)>, relleno: Option<Color>) {
        let anillo = vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)];
        if let Some(c) = relleno {
            self.relleno(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], c);
        }
        if let Some((c, g)) = borde {
            self.trazo(c, g, false);
            self.ops.push(Op::DrawLine { line: Line { points: anillo, is_closed: true } });
        }
    }

    fn relleno(&mut self, xy: &[(f32, f32)], color: Color) {
        self.ops.push(Op::SetFillColor { col: color });
        self.ops.push(Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points: xy.iter().map(|&(x, y)| pt(x, y)).collect() }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        });
    }

    /// Texto con su línea base en (x, y), girado `ang` grados (antihorario).
    fn texto(&mut self, f: &Fuente, tam: f32, x: f32, y: f32, ang: f32, s: &str) {
        let (c, sn) = (ang.to_radians().cos(), ang.to_radians().sin());
        self.ops.push(Op::SetFillColor { col: gris(0.0) });
        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetFont { font: PdfFontHandle::External(f.id.clone()), size: Pt(tam) });
        self.ops.push(Op::SetTextMatrix { matrix: TextMatrix::Raw([c, sn, -sn, c, x, y]) });
        self.ops.push(Op::ShowText { items: vec![TextItem::Text(s.to_string())] });
        self.ops.push(Op::EndTextSection);
    }

    /// Texto alineado: `ha` 0 izquierda, 0.5 centro, 1 derecha; `va` igual desde la base.
    #[allow(clippy::too_many_arguments)]
    fn texto_alineado(&mut self, f: &Fuente, tam: f32, x: f32, y: f32, ang: f32, ha: f32, va: f32, s: &str) {
        let w = f.ancho(s, tam);
        let alto = 0.72 * tam; // altura de mayúsculas aproximada
        let (c, sn) = (ang.to_radians().cos(), ang.to_radians().sin());
        let (dx, dy) = (-ha * w, -va * alto);
        self.texto(f, tam, x + dx * c - dy * sn, y + dx * sn + dy * c, ang, s);
    }
}

/// Documento con flujo vertical (títulos, párrafos, tablas) en páginas iguales.
struct Flujo {
    normal: Fuente,
    negrita: Fuente,
    ancho: f32,
    alto: f32,
    margen: f32,
    hojas: Vec<Hoja>,
    y: f32,
}

impl Flujo {
    fn util(&self) -> f32 {
        self.ancho - 2.0 * self.margen
    }

    fn salto(&mut self) {
        self.hojas.push(Hoja::nueva());
        self.y = self.alto - self.margen;
    }

    fn reservar(&mut self, h: f32) {
        if self.y - h < self.margen {
            self.salto();
        }
    }

    fn titulo(&mut self, s: &str) {
        self.reservar(22.0);
        self.y -= 22.0;
        let x = self.margen + (self.util() - self.negrita.ancho(s, 18.0)) / 2.0;
        let y = self.y + 5.0;
        let Flujo { negrita, hojas, .. } = self;
        hojas.last_mut().unwrap().texto(negrita, 18.0, x, y, 0.0, s);
        self.y -= 6.0;
    }

    fn encabezado(&mut self, s: &str) {
        self.y -= 12.0;
        self.reservar(18.0 + 40.0); // no dejar el título solo al pie
        self.y -= 18.0;
        let (x, y) = (self.margen, self.y + 4.0);
        let Flujo { negrita, hojas, .. } = self;
        hojas.last_mut().unwrap().texto(negrita, 14.0, x, y, 0.0, s);
        self.y -= 6.0;
    }

    /// Párrafo de 8 pt (interlineado 10) con tramos en negrita, ajustado al ancho.
    fn parrafo(&mut self, tramos: &[(&str, bool)]) {
        let (tam, inter) = (8.0, 10.0);
        // palabras con su estilo; un tramo que no empieza con espacio continúa la palabra anterior
        let mut palabras: Vec<(String, bool)> = Vec::new();
        let mut pegar = false;
        for (t, b) in tramos {
            for (k, p) in t.split(' ').enumerate() {
                if k == 0 && pegar && !p.is_empty() {
                    let ultima = palabras.last_mut().unwrap();
                    ultima.0.push_str(p);
                } else {
                    palabras.push((p.to_string(), *b));
                }
            }
            pegar = !t.ends_with(' ');
        }
        palabras.retain(|p| !p.0.is_empty());
        let mut lineas: Vec<Vec<(String, bool)>> = vec![Vec::new()];
        let mut w = 0.0;
        let esp = self.normal.ancho(" ", tam);
        for (p, b) in palabras {
            let pw = if b { self.negrita.ancho(&p, tam) } else { self.normal.ancho(&p, tam) };
            if w > 0.0 && w + esp + pw > self.util() {
                lineas.push(Vec::new());
                w = 0.0;
            }
            w += if w > 0.0 { esp } else { 0.0 } + pw;
            lineas.last_mut().unwrap().push((p, b));
        }
        self.y -= 6.0;
        for l in lineas {
            self.reservar(inter);
            self.y -= inter;
            let mut x = self.margen;
            let y = self.y + 2.5;
            for (p, b) in l {
                let Flujo { normal, negrita, hojas, .. } = self;
                let f = if b { &*negrita } else { &*normal };
                hojas.last_mut().unwrap().texto(f, tam, x, y, 0.0, &p);
                x += f.ancho(&p, tam) + esp;
            }
        }
    }

    /// Tabla con cuadrícula, encabezado gris y encabezado repetido al cambiar de página.
    fn tabla(&mut self, tb: &Tabla, tam0: f32) {
        let celdas: Vec<Vec<String>> = std::iter::once(tb.columnas.clone())
            .chain(tb.filas.iter().map(|f| f.iter().map(|c| c.texto()).collect()))
            .collect();
        let anchos = |tam: f32| -> Vec<f32> {
            (0..tb.columnas.len())
                .map(|j| celdas.iter().map(|f| self.normal.ancho(f.get(j).map_or("", |s| s.as_str()), tam)).fold(0.0, f32::max) + 12.0)
                .collect()
        };
        let mut tam = tam0;
        let mut cw = anchos(tam);
        while cw.iter().sum::<f32>() > self.util() && tam > 3.0 {
            tam -= 0.25;
            cw = anchos(tam);
        }
        let total: f32 = cw.iter().sum();
        let x0 = self.margen + ((self.util() - total) / 2.0).max(0.0);
        let alto_fila = tam * 1.2 + 6.0;
        let mut fila = 0;
        while fila < celdas.len() {
            // encabezado + al menos una fila
            self.reservar(2.0 * alto_fila);
            let mut filas_pag = vec![0usize];
            if fila == 0 {
                fila = 1;
            }
            while fila < celdas.len() && self.y - (filas_pag.len() + 1) as f32 * alto_fila >= self.margen {
                filas_pag.push(fila);
                fila += 1;
            }
            let y_top = self.y;
            let Flujo { normal, hojas, .. } = self;
            let h = hojas.last_mut().unwrap();
            h.rect(x0, y_top - alto_fila, total, alto_fila, None, Some(hex(0xe8e8e8)));
            for (k, &i) in filas_pag.iter().enumerate() {
                let yb = y_top - (k + 1) as f32 * alto_fila;
                let mut x = x0;
                for (j, w) in cw.iter().enumerate() {
                    if let Some(s) = celdas[i].get(j) {
                        h.texto(normal, tam, x + 6.0, yb + (alto_fila - 0.72 * tam) / 2.0, 0.0, s);
                    }
                    x += w;
                }
            }
            let yb = y_top - filas_pag.len() as f32 * alto_fila;
            for k in 0..=filas_pag.len() {
                let y = y_top - k as f32 * alto_fila;
                h.linea((x0, y), (x0 + total, y), gris(0.5), 0.25);
            }
            let mut x = x0;
            for w in std::iter::once(&0.0).chain(cw.iter()) {
                x += w;
                h.linea((x, y_top), (x, yb), gris(0.5), 0.25);
            }
            self.y = yb;
            if fila < celdas.len() {
                self.salto();
            }
        }
    }

    fn espacio(&mut self, h: f32) {
        self.y -= h;
    }
}

fn fuente(doc: &mut PdfDocument, bytes: &[u8]) -> Result<Fuente, String> {
    let pf = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).ok_or("fuente DejaVu inválida")?;
    Ok(Fuente { id: doc.add_font(&pf), pf })
}

fn guardar(doc: &mut PdfDocument, hojas: Vec<Hoja>, ancho: f32, alto: f32, ruta: &Path) -> Result<PathBuf, String> {
    let paginas = hojas.into_iter().map(|h| PdfPage::new(printpdf::Mm(ancho / MM), printpdf::Mm(alto / MM), h.ops)).collect();
    let bytes = doc.with_pages(paginas).save(&PdfSaveOptions::default(), &mut Vec::new());
    if let Some(d) = ruta.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(ruta, bytes).map_err(|e| format!("{}: {e}", ruta.display()))?;
    Ok(ruta.to_path_buf())
}

/// Paso "redondo" (1, 2, 2,5 o 5 × 10^k) para unas `n` divisiones.
fn paso_redondo(rango: f64, n: f64) -> f64 {
    let bruto = (rango / n).max(1e-9);
    let e = 10f64.powf(bruto.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * e).find(|&p| p >= bruto).unwrap_or(10.0 * e)
}

/// Figura del perfil con conductor y despeje (como `figura_perfil`), en el rectángulo dado.
fn figura_perfil(h: &mut Hoja, f: &Fuente, r: &Resultado, p: &Proyecto, x0: f32, y0: f32, w: f32, alto: f32) {
    let (ml, mb, mt, mr) = (34.0, 24.0, 6.0, 6.0);
    let (ax, ay, aw, ah) = (x0 + ml, y0 + mb, w - ml - mr, alto - mb - mt);
    let cond = curvas_vanos(r, p, 0.0);
    let desp = curvas_vanos(r, p, p.distancia_minima_terreno);
    let zt: Vec<f64> = p.perfil.cota.iter().copied().filter(|z| !z.is_nan()).collect();
    let zmin_t = zt.iter().copied().fold(f64::INFINITY, f64::min);
    let mut zmin = zmin_t - 5.0;
    let mut zmax = zt.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    for (_, z) in cond.iter().chain(&desp) {
        for &v in z {
            zmax = zmax.max(v);
            zmin = zmin.min(v);
        }
    }
    for e in &r.estructuras {
        zmax = zmax.max(e.cota_amarre());
    }
    let m = 0.05 * (zmax - zmin);
    let (y_lo, y_hi) = (zmin - m, zmax + m);
    let l = p.perfil.longitud().max(1e-9);
    let px = |s: f64| ax + (s / l) as f32 * aw;
    let py = |z: f64| ay + ((z - y_lo) / (y_hi - y_lo)) as f32 * ah;
    // cuadrícula y escalas
    let (dx, dy) = (paso_redondo(l, 8.0), paso_redondo(y_hi - y_lo, 5.0));
    let mut s = 0.0;
    while s <= l + 1e-9 {
        h.linea((px(s), ay), (px(s), ay + ah), gris(0.85), 0.3);
        h.texto_alineado(f, 6.0, px(s), ay - 9.0, 0.0, 0.5, 0.0, &formato_g(s));
        s += dx;
    }
    let mut z = (y_lo / dy).ceil() * dy;
    while z <= y_hi + 1e-9 {
        h.linea((ax, py(z)), (ax + aw, py(z)), gris(0.85), 0.3);
        h.texto_alineado(f, 6.0, ax - 3.0, py(z), 0.0, 1.0, 0.5, &formato_g(z));
        z += dy;
    }
    for t in tramos_validos(&p.perfil.progresiva, &p.perfil.cota) {
        let mut poly: Vec<(f32, f32)> = t.iter().map(|q| (px(q[0]), py(q[1]))).collect();
        poly.push((px(t[t.len() - 1][0]), py(zmin_t - 5.0).max(ay)));
        poly.push((px(t[0][0]), py(zmin_t - 5.0).max(ay)));
        h.relleno(&poly, hex(0xd9c7a3));
        h.polilinea(&t.iter().map(|q| (px(q[0]), py(q[1]))).collect::<Vec<_>>(), hex(0x5a3e1b), 0.8, false);
    }
    for ((s, z), (_, zd)) in cond.iter().zip(&desp) {
        h.polilinea(&s.iter().zip(z).map(|(a, b)| (px(*a), py(*b))).collect::<Vec<_>>(), hex(0xc0392b), 0.8, false);
        h.polilinea(&s.iter().zip(zd).map(|(a, b)| (px(*a), py(*b))).collect::<Vec<_>>(), hex(0x8e44ad), 0.5, true);
    }
    for e in &r.estructuras {
        h.linea((px(e.progresiva), py(e.cota_terreno)), (px(e.progresiva), py(e.cota_amarre())), gris(0.0), 1.2);
        h.texto_alineado(f, 6.0, px(e.progresiva), py(e.cota_amarre()) + 3.0, 0.0, 0.5, 0.0, &e.codigo());
    }
    h.rect(ax, ay, aw, ah, Some((gris(0.0), 0.8)), None);
    h.texto_alineado(f, 7.0, ax + aw / 2.0, y0 + 2.0, 0.0, 0.5, 0.0, "Progresiva (m)");
    h.texto_alineado(f, 7.0, x0 + 8.0, ay + ah / 2.0, 90.0, 0.5, 0.0, "Cota (m)");
    // leyenda
    let items = [
        (format!("Conductor ({})", p.hip_flecha_maxima), hex(0xc0392b), false),
        (format!("Despeje mínimo {:.2} m", p.distancia_minima_terreno), hex(0x8e44ad), true),
    ];
    let lw = items.iter().map(|i| f.ancho(&i.0, 6.0)).fold(0.0, f32::max) + 30.0;
    let (lx, ly) = (ax + aw - lw - 4.0, ay + ah - 4.0 - 2.0 * 9.0 - 4.0);
    h.rect(lx, ly, lw, 2.0 * 9.0 + 4.0, Some((gris(0.8), 0.5)), Some(gris(1.0)));
    for (k, (t, c, d)) in items.iter().enumerate() {
        let yy = ly + 4.0 + (1 - k) as f32 * 9.0 + 2.0;
        h.polilinea(&[(lx + 4.0, yy + 2.0), (lx + 20.0, yy + 2.0)], c.clone(), 0.8, *d);
        h.texto(f, 6.0, lx + 24.0, yy, 0.0, t);
    }
}

fn hoy() -> String {
    // fecha civil UTC desde el reloj del sistema (algoritmo de Howard Hinnant)
    let dias = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86400).unwrap_or(0) as i64;
    let z = dias + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn tabla_simple(nombre: &str, columnas: &[&str], filas: Vec<Vec<String>>) -> Tabla {
    Tabla {
        nombre: nombre.into(),
        columnas: columnas.iter().map(|c| c.to_string()).collect(),
        filas: filas.into_iter().map(|f| f.into_iter().map(crate::salidas::Celda::Txt).collect()).collect(),
    }
}

/// Memoria de cálculo mecánico (A4 apaisado), mismas secciones que la de Python.
pub fn memoria_pdf(ruta: &Path, r: &Resultado, p: &Proyecto, x: &Extras) -> Result<PathBuf, String> {
    let mut doc = PdfDocument::new(&format!("Memoria de cálculo - {}", p.nombre));
    doc.metadata.info.author = "LineCraft".into();
    let (normal, negrita) = (fuente(&mut doc, NORMAL)?, fuente(&mut doc, NEGRITA)?);
    let (ancho, alto) = (297.0 * MM, 210.0 * MM);
    let mut fl = Flujo { normal, negrita, ancho, alto, margen: 12.0 * MM, hojas: Vec::new(), y: 0.0 };
    fl.salto();
    let c = &p.conductor;
    let fuente_de = |k: &str, d: &str| p.fuentes.get(k).cloned().unwrap_or_else(|| d.to_string());
    let modelo = match p.modelo {
        crate::mecanica::Modelo::Catenaria => "catenaria",
        crate::mecanica::Modelo::Parabola => "parabola",
    };
    fl.titulo(&format!("Memoria de cálculo mecánico — {}", p.nombre));
    fl.parrafo(&[(&format!("Generado con LineCraft (Suite Eléctrica) el {}. Modelo de curva: {modelo}. \
        Conductores por circuito: {}.", hoy(), p.n_conductores), false)]);
    fl.parrafo(&[
        ("Resultado global: ", false),
        (if cumple(r, x.verificacion, x.alertas) { "CUMPLE" } else { "NO CUMPLE" }, true),
        (&format!(" — {} aviso(s).", r.avisos.len()), false),
    ]);
    fl.encabezado("1. Datos del conductor");
    let s = |v: &str| v.to_string();
    fl.tabla(&tabla_simple("c", &["Parámetro", "Valor"], vec![
        vec![s("Conductor"), c.nombre.clone()],
        vec![s("Sección (mm²)"), format!("{:.2}", c.seccion * 1e6)],
        vec![s("Diámetro (mm)"), format!("{:.2}", c.diametro * 1e3)],
        vec![s("Peso (N/m)"), format!("{:.4}", c.peso)],
        vec![s("Tiro de rotura (N)"), format!("{:.0}", c.carga_rotura)],
        vec![s("Módulo de elasticidad (MPa)"), format!("{:.0}", c.modulo_elasticidad / 1e6)],
        vec![s("Coef. dilatación (1/°C)"), formato_e2(c.coef_dilatacion)],
        vec![s("Fuente"), if c.fuente.is_empty() { s("—") } else { c.fuente.clone() }],
    ]), 7.5);
    fl.encabezado("2. Parámetros de diseño y fuentes");
    fl.tabla(&tabla_simple("p", &["Parámetro", "Valor", "Fuente"], vec![
        vec![s("Condición inicial"), format!("{}: {} % rotura", p.hip_inicial, formato_g(p.tiro_inicial_pct)), fuente_de("tiro_inicial_pct_rotura", "—")],
        vec![s("Tiro máximo admisible"), format!("{} % rotura", formato_g(p.tiro_maximo_pct)), fuente_de("tiro_maximo_pct_rotura", "—")],
        vec![s("Distancia mínima al terreno"), format!("{:.2} m", p.distancia_minima_terreno), fuente_de("distancia_minima_terreno_m", "—")],
        vec![s("Hipótesis de flecha máxima"), p.hip_flecha_maxima.clone(), fuente_de("hipotesis", "—")],
        vec![s("Altura de amarre"), format!("{:.2} m", p.altura_amarre), s("Diseño")],
        vec![s("Vano máximo / mínimo"), format!("{} / {} m", formato_g(p.vano_maximo), formato_g(p.vano_minimo)), s("Diseño")],
        vec![s("Ángulo de retenida"), format!("{}° con la horizontal", formato_g(p.angulo_retenida)), fuente_de("angulo_retenida_deg", "Diseño")],
    ]), 7.5);
    fl.espacio(3.0 * MM);
    let num = |v: f64| crate::salidas::texto_python(v);
    fl.tabla(&tabla_simple("h", &["Hipótesis", "Temp. (°C)", "Viento (Pa)", "Hielo (mm)", "Dens. hielo (kg/m³)", "Altitud (msnm)"],
        p.hipotesis.iter().map(|h| vec![
            h.nombre.clone(), num(h.temperatura), num(h.presion_viento), num(h.espesor_hielo * 1e3), num(h.densidad_hielo),
            if h.altitud_min == f64::NEG_INFINITY && h.altitud_max == f64::INFINITY { s("todas") }
            else { format!("{}–{}", formato_g(h.altitud_min.max(0.0)), formato_g(h.altitud_max)) },
        ]).collect()), 7.5);
    fl.encabezado("3. Fórmulas");
    for t in [
        "Cambio de estado: σ2²·[σ2 − σ1 + a²·w1²·E/(24·A²·σ1²) + α·E·(t2 − t1)] = a²·w2²·E/(24·A²), con a = vano regulador √(Σa³/Σa).",
        "Catenaria: C = H/w; y(x) = C·[cosh((x − xm)/C) − cosh(xm/C)]; xm = a/2 − C·asinh(h/(2C·sinh(a/2C))). Parábola: f = w·a²/(8H).",
        "Cargas unitarias: hielo ρ·g·π·e·(d + e); viento p·(d + 2e). Flecha vertical = flecha resultante × w_vertical/w_resultante.",
        "Cargas en estructura (n conductores): transversal n·[(H_atrás + H_adelante)·sen(θ/2) + w_viento·a_viento·cos(θ/2)]; \
         longitudinal n·|H_adelante − H_atrás|·cos(θ/2); retenida F_h/cos(φ). Detalle y fuentes en docs/linecraft.md.",
    ] {
        fl.parrafo(&[(t, false)]);
    }
    fl.encabezado("4. Perfil");
    let (w, h) = (fl.util(), fl.util() * 0.35);
    fl.reservar(h);
    fl.y -= h;
    let (x0, y0) = (fl.margen, fl.y);
    {
        let Flujo { normal, hojas, .. } = &mut fl;
        figura_perfil(hojas.last_mut().unwrap(), normal, r, p, x0, y0, w, h);
    }
    let tablas = tablas_linea(r, p, x);
    let titulo = |k: &str| match k {
        "Estructuras" => "Estructuras",
        "Mecanico" => "Cálculo mecánico por tramo",
        "Vanos" => "Vanos, flechas y distancias de seguridad",
        "Cargas" => "Cargas en estructuras",
        "Verificacion" => "Verificación de postes y retenidas",
        "Metrado" => "Metrado",
        "Alertas" => "Alertas de diseño (exceso sobre el permisible)",
        "Flechado" => "Tabla de flechado (tendido, sin viento)",
        _ => "Avisos",
    };
    let orden = ["Estructuras", "Mecanico", "Vanos", "Cargas", "Verificacion", "Alertas", "Flechado", "Metrado", "Avisos"];
    let mut k = 5;
    for clave in orden {
        if let Some(tb) = tablas.iter().find(|t| t.nombre == clave) {
            fl.encabezado(&format!("{k}. {}", titulo(clave)));
            fl.tabla(tb, 6.5);
            k += 1;
        }
    }
    let Flujo { hojas, .. } = fl;
    guardar(&mut doc, hojas, ancho, alto, ruta)
}

/// `f"{x:.2e}"` de Python (exponente con signo y dos cifras).
fn formato_e2(x: f64) -> String {
    let s = format!("{x:.2e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// Escalas normalizadas del perfil (denominador).
pub const ESCALAS: [u32; 10] = [500, 1000, 2000, 2500, 5000, 10000, 20000, 25000, 50000, 100000];

/// Menor escala normalizada con la que el perfil cabe en `ancho_max_mm`.
pub fn escala_automatica(longitud: f64, ancho_max_mm: f64) -> u32 {
    ESCALAS.iter().copied().find(|&e| longitud / e as f64 * 1000.0 <= ancho_max_mm).unwrap_or(100000)
}

/// Perfil longitudinal a escala real (imprimir al 100 %), como `topocraft.salidas.perfil_pdf`.
pub fn perfil_pdf(ruta: &Path, perfil: &Perfil, esc_h: Option<u32>, esc_v: Option<u32>, titulo: &str) -> Result<(u32, u32), String> {
    let l = perfil.longitud();
    let esc_h = esc_h.unwrap_or_else(|| escala_automatica(l, 900.0));
    let esc_v = esc_v.unwrap_or((esc_h / 10).max(1));
    let zbot = cota_referencia(perfil);
    let zmax = perfil.cota.iter().copied().filter(|z| !z.is_nan()).fold(f64::NEG_INFINITY, f64::max);
    let mut ztop = ((zmax + 5.0) / 5.0).ceil() * 5.0;
    let w_mm = l / esc_h as f64 * 1000.0;
    let mut h_mm = (ztop - zbot) / esc_v as f64 * 1000.0;
    if h_mm < 40.0 {
        ztop = zbot + 40.0 * esc_v as f64 / 1000.0;
        h_mm = 40.0;
    }
    let (ml, mr, mt, banda, mb) = (30.0, 10.0, 15.0, 40.0, 10.0);
    let (ancho, alto) = (ml + w_mm + mr, mb + banda + h_mm + mt);
    let mut doc = PdfDocument::new(titulo);
    let f = fuente(&mut doc, NORMAL)?;
    let mut h = Hoja::nueva();
    let m = |v: f64| (v * MM as f64) as f32;
    let px = |s: f64| m(ml + s / l * w_mm);
    let py = |z: f64| m(mb + banda + (z - zbot) / (ztop - zbot) * h_mm);
    let (ax, ay, aw, ah) = (m(ml), m(mb + banda), m(w_mm), m(h_mm));
    // cuadrícula y escala vertical
    let dz = paso_redondo(ztop - zbot, 8.0);
    let mut z = (zbot / dz).ceil() * dz;
    while z <= ztop + 1e-9 {
        h.linea((ax, py(z)), (ax + aw, py(z)), gris(0.8), 0.3);
        h.texto_alineado(&f, 6.0, ax - 3.5, py(z), 0.0, 1.0, 0.5, &formato_g(z));
        z += dz;
    }
    let ds = paso_redondo(l, 6.0);
    let mut s = 0.0;
    while s <= l + 1e-9 {
        h.linea((px(s), ay), (px(s), ay + ah), gris(0.8), 0.3);
        s += ds;
    }
    let tono = |c: u32| {
        let k = |v: u32| 1.0 - 0.6 * (1.0 - v as f32 / 255.0);
        rgb(k((c >> 16) & 255), k((c >> 8) & 255), k(c & 255))
    };
    for t in tramos_validos(&perfil.progresiva, &perfil.cota) {
        let mut poly: Vec<(f32, f32)> = t.iter().map(|q| (px(q[0]), py(q[1]))).collect();
        poly.push((px(t[t.len() - 1][0]), py(zbot)));
        poly.push((px(t[0][0]), py(zbot)));
        h.relleno(&poly, tono(0xd9c7a3));
        h.polilinea(&t.iter().map(|q| (px(q[0]), py(q[1]))).collect::<Vec<_>>(), hex(0x5a3e1b), 1.0, false);
    }
    h.texto(&f, 8.0, ax, ay + ah + 6.0, 0.0, &format!("{titulo}   Esc. H 1:{esc_h}   V 1:{esc_v}"));
    h.texto_alineado(&f, 7.0, ax - 24.0, ay + ah / 2.0, 90.0, 0.5, 0.0, "Cota (m)");
    // banda de datos
    let (by, bh) = (m(mb), m(banda));
    h.linea((ax, by + bh / 2.0), (ax + aw, by + bh / 2.0), gris(0.0), 0.4);
    h.texto_alineado(&f, 6.0, m(2.0), by + 0.75 * bh, 0.0, 0.0, 0.5, "PROGRESIVA");
    h.texto_alineado(&f, 6.0, m(2.0), by + 0.25 * bh, 0.0, 0.0, 0.5, "COTA TERRENO");
    let min_sep = 3.5 * esc_h as f64 / 1000.0;
    let mut ultimo = f64::NEG_INFINITY;
    for (s, z, _) in principales(perfil) {
        if s - ultimo < min_sep && (s - l).abs() > 1e-9 * l.max(1.0) {
            continue;
        }
        ultimo = s;
        h.linea((px(s), by), (px(s), by + bh), gris(0.6), 0.3);
        h.texto_alineado(&f, 5.0, px(s), by + 0.75 * bh, 90.0, 0.5, 0.5, &formato_progresiva(s, 2));
        if !z.is_nan() {
            h.texto_alineado(&f, 5.0, px(s), by + 0.25 * bh, 90.0, 0.5, 0.5, &format!("{z:.2}"));
            h.linea((px(s), py(zbot)), (px(s), py(z)), gris(0.7), 0.3);
        }
    }
    h.rect(ax, ay, aw, ah, Some((gris(0.0), 0.8)), None);
    h.rect(ax, by, aw, bh, Some((gris(0.0), 0.8)), None);
    guardar(&mut doc, vec![h], m(ancho), m(alto), ruta)?;
    Ok((esc_h, esc_v))
}
