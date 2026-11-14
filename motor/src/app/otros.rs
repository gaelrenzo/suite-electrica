//! Pestañas TopoCraft, GridCraft y PresuCraft.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
use suite_motor::documento::metrado_presupuesto;
use suite_motor::salidas::{escribir_excel, tablas_perfil, tablas_presupuesto, tablas_red_con_metrado, Celda, Tabla};
use suite_motor::tin::es_mayor;
use suite_motor::{dxf, pdf, presupuesto, red};

use crate::{abrir_externo, elegir_carpeta, num, ver_tabla, App};

const FORMATOS_PUNTOS: &[&str] = &["csv", "txt", "dxf", "kml", "gpx", "xls", "xlsx"];
const FORMATOS_EJE: &[&str] = &["dxf", "csv", "txt", "kml", "gpx", "xls", "xlsx"];

pub fn importar_puntos(app: &mut App) {
    if let Some(r) = rfd::FileDialog::new().add_filter("Puntos (CSV, TXT, DXF, KML, GPX, Excel)", FORMATOS_PUNTOS).pick_file() {
        let res = app.doc.importar_puntos(&r);
        if app.intentar(res).is_some() {
            app.calculo = None;
            app.estado = format!("{} puntos importados de {}", app.doc.puntos.len(), r.display());
        }
    }
}

pub fn importar_eje(app: &mut App) {
    if let Some(r) = rfd::FileDialog::new().add_filter("Eje (DXF, CSV, KML, GPX, Excel)", FORMATOS_EJE).pick_file() {
        let res = app.doc.importar_eje(&r, None);
        if app.intentar(res).is_some() {
            app.calculo = None;
            let n = app.doc.eje.as_ref().map_or(0, |e| e.len());
            app.estado = format!("Eje de {n} vértices importado de {}", r.display());
        }
    }
}

fn procesar(app: &mut App) {
    let r = app.doc.topografia().map(|t| {
        let rep = &t.reporte;
        format!(
            "Topografía: {} de {} puntos válidos, {} triángulos, {} curvas{}",
            rep.n_salida,
            rep.n_entrada,
            t.tin.activos.iter().filter(|a| **a).count(),
            t.curvas.len(),
            t.perfil.as_ref().map(|p| format!(", perfil de {:.2} m", p.longitud())).unwrap_or_default()
        )
    });
    if let Some(m) = app.intentar(r) {
        app.estado = m;
    }
}

fn exportar_topo(app: &mut App) {
    let Some(c) = elegir_carpeta("Carpeta de salida (TopoCraft)", app.doc.ruta.as_deref().and_then(Path::parent)) else { return };
    let nombre = app.doc.nombre.clone();
    let (eq, mayor) = (app.doc.topo.equidistancia, app.doc.topo.mayor_cada);
    let eje = app.doc.eje.clone();
    let r = app.doc.topografia().and_then(|t| {
        let mut out = vec![dxf::plano_topo(&c.join("plano.dxf"), &t.puntos, Some(&t.tin), &t.curvas, eq, mayor, eje.as_deref(), 1.0)?];
        if let Some(p) = &t.perfil {
            pdf::perfil_pdf(&c.join("perfil.pdf"), p, None, None, "PERFIL LONGITUDINAL")?;
            out.push(c.join("perfil.pdf"));
            out.push(escribir_excel(&c.join("perfil.xlsx"), &tablas_perfil(p, &[("Proyecto".into(), Celda::Txt(nombre))]))?);
            out.push(dxf::perfil_topo(&c.join("perfil.dxf"), p, 10.0)?);
        }
        Ok(out)
    });
    if let Some(out) = app.intentar(r) {
        app.estado = format!("Exportados {} archivos en {}", out.len(), c.display());
        abrir_externo(&c);
    }
}

pub fn panel_topo(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::left("topo_datos").resizable(true).default_size(300.0).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Datos");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Importar puntos…").clicked() {
                    importar_puntos(app);
                }
                if ui.button("Importar eje…").clicked() {
                    importar_eje(app);
                }
            });
            ui.label(format!("Puntos: {}", app.doc.puntos.len()));
            ui.label(match &app.doc.eje {
                Some(e) => format!("Eje: {} vértices", e.len()),
                None => "Eje: sin importar".into(),
            });
            ui.separator();
            ui.heading("Parámetros");
            let mut cambio = false;
            egui::Grid::new("topo_param").num_columns(2).show(ui, |ui| {
                let t = &mut app.doc.topo;
                cambio |= num(ui, "Equidistancia", &mut t.equidistancia, 0.1, " m");
                ui.label("Curva mayor cada");
                cambio |= ui.add(egui::DragValue::new(&mut t.mayor_cada).range(1..=50)).changed();
                ui.end_row();
                cambio |= num(ui, "Paso del perfil", &mut t.paso_perfil, 1.0, " m");
                ui.label("Lado máximo del TIN");
                ui.horizontal(|ui| {
                    let mut con = t.max_lado.is_some();
                    if ui.checkbox(&mut con, "").changed() {
                        t.max_lado = if con { Some(100.0) } else { None };
                        cambio = true;
                    }
                    if let Some(m) = &mut t.max_lado {
                        cambio |= ui.add(egui::DragValue::new(m).speed(1.0).suffix(" m")).changed();
                    }
                });
                ui.end_row();
                ui.label("Quitar cotas atípicas");
                cambio |= ui.checkbox(&mut t.quitar_atipicos, "").changed();
                ui.end_row();
            });
            if cambio {
                app.doc.topo.equidistancia = app.doc.topo.equidistancia.max(0.01);
                app.doc.topo.paso_perfil = app.doc.topo.paso_perfil.max(0.5);
                app.doc.cambio_topografia();
                app.calculo = None;
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new("Procesar").strong())).clicked() {
                    procesar(app);
                }
                if ui.button("Exportar…").clicked() {
                    exportar_topo(app);
                }
            });
            ui.label("Exporta plano.dxf y, con eje, perfil.pdf, perfil.xlsx y perfil.dxf.");
            ui.separator();
            if let Ok(t) = app.doc.topografia() {
                ui.heading("Limpieza");
                let r = &t.reporte;
                for (k, v) in [
                    ("Inválidos (vacíos o no numéricos)", &r.invalidos),
                    ("Duplicados", &r.duplicados),
                    ("Conflictos (misma posición, otra cota)", &r.conflictos),
                    ("Nombres repetidos", &r.nombres_repetidos),
                    ("Cotas atípicas", &r.atipicos),
                    ("Fuera de zona UTM", &r.fuera_zona),
                    ("Cotas fuera de rango", &r.cotas_fuera_rango),
                ] {
                    if !v.is_empty() {
                        ui.label(format!("{k}: {}", v.join(", ")));
                    }
                }
                ui.label(format!("Puntos válidos: {} de {}", r.n_salida, r.n_entrada));
            } else if !app.doc.puntos.is_empty() {
                ui.label("Pulse Procesar para limpiar los puntos y generar el TIN.");
            }
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let alto = ui.available_height();
        let (eq, mayor) = (app.doc.topo.equidistancia, app.doc.topo.mayor_cada);
        let eje = app.doc.eje.clone();
        let Ok(t) = app.doc.topografia() else {
            ui.centered_and_justified(|ui| ui.label("Importe puntos topográficos (CSV, TXT, DXF, KML, GPX o Excel RLT) y un eje."));
            return;
        };
        let pts: Vec<[f64; 2]> = t.puntos.iter().map(|p| [p.este, p.norte]).collect();
        Plot::new("planta").legend(Legend::default()).data_aspect(1.0).height(alto * 0.6).x_axis_label("Este (m)").y_axis_label("Norte (m)").show(ui, |pu| {
            pu.points(Points::new("Puntos", pts).radius(1.5).color(egui::Color32::GRAY));
            let (mut nm, mut nn) = (true, true);
            for c in &t.curvas {
                let mut xy = c.xy.clone();
                if c.cerrada && !xy.is_empty() {
                    xy.push(xy[0]);
                }
                let m = es_mayor(c.cota, eq, mayor);
                let nombre = match (m, nm, nn) {
                    (true, true, _) => "Curvas mayores",
                    (false, _, true) => "Curvas menores",
                    _ => "",
                };
                if m { nm = false } else { nn = false }
                pu.line(Line::new(nombre, PlotPoints::from(xy)).color(egui::Color32::from_rgb(140, 90, 40)).width(if m { 1.6 } else { 0.6 }));
            }
            if let Some(e) = eje {
                pu.line(Line::new("Eje", PlotPoints::from(e)).color(egui::Color32::RED).width(2.0));
            }
        });
        if let Some(p) = &t.perfil {
            let terreno: Vec<[f64; 2]> = p.progresiva.iter().zip(&p.cota).filter(|(_, z)| !z.is_nan()).map(|(s, z)| [*s, *z]).collect();
            let zmin = terreno.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min);
            Plot::new("perfil_topo").x_axis_label("Progresiva (m)").y_axis_label("Cota (m)").show(ui, |pu| {
                pu.line(Line::new("Terreno", PlotPoints::from(terreno)).color(egui::Color32::from_rgb(90, 62, 27)).fill((zmin - 5.0) as f32).fill_alpha(0.4));
            });
        }
    });
}

// ---------------------------------------------------------------- GridCraft y PresuCraft

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Grid,
    Presu,
}

pub struct ModuloJson {
    tipo: Tipo,
    pub ruta: Option<PathBuf>,
    tablas: Vec<Tabla>,
    resumen: String,
    hoja: usize,
    red: Option<(red::Red, red::ResultadoRed)>,
    presu: Option<presupuesto::ResultadoPresupuesto>,
}

const PLANTILLAS_GRID: [(&str, &str); 3] = [
    ("red.json", include_str!("../../../suite_electrica/plantillas/red_proyecto.json")),
    ("nodos.csv", include_str!("../../../suite_electrica/plantillas/red_nodos.csv")),
    ("conductores_bt.csv", include_str!("../../../suite_electrica/plantillas/conductores_bt.csv")),
];
const PLANTILLAS_PRESU: [(&str, &str); 5] = [
    ("presupuesto.json", include_str!("../../../suite_electrica/plantillas/presu_presupuesto.json")),
    ("suministros.csv", include_str!("../../../suite_electrica/plantillas/presu_suministros.csv")),
    ("unidades.csv", include_str!("../../../suite_electrica/plantillas/presu_unidades.csv")),
    ("montaje.csv", include_str!("../../../suite_electrica/plantillas/presu_montaje.csv")),
    ("metrado.csv", include_str!("../../../suite_electrica/plantillas/presu_metrado.csv")),
];

impl ModuloJson {
    pub fn nuevo(tipo: Tipo) -> Self {
        ModuloJson { tipo, ruta: None, tablas: Vec::new(), resumen: String::new(), hoja: 0, red: None, presu: None }
    }

    fn archivo(&self) -> &'static str {
        match self.tipo {
            Tipo::Grid => "red.json",
            Tipo::Presu => "presupuesto.json",
        }
    }

    fn calcular(&mut self) -> Result<String, String> {
        let ruta = self.ruta.clone().ok_or("Abra o cree un proyecto")?;
        match self.tipo {
            Tipo::Grid => {
                let rd = red::cargar(&ruta)?;
                let r = red::calcular(&rd)?;
                let g = |k: &str| r.resumen.iter().find(|x| x.0 == k).map_or(f64::NAN, |x| x.1);
                self.resumen = format!(
                    "{} — %ΔV máx. {:.2} %, M.D. {:.2} kW, {} alerta(s)",
                    if r.alertas.is_empty() { "CUMPLE" } else { "NO CUMPLE" },
                    g("%ΔV máx."),
                    g("M.D. total (kW)"),
                    r.alertas.len()
                );
                self.tablas = tablas_red_con_metrado(&rd, &r);
                self.red = Some((rd, r));
            }
            Tipo::Presu => {
                let p = presupuesto::cargar(&ruta)?;
                let r = presupuesto::calcular(&p)?;
                let t = r.resumen.iter().find(|f| f.rubro == "TOTAL");
                self.resumen = t.map(|t| format!("Total {} {} (CD {})", p.moneda, miles(t.total), miles(t.costo_directo))).unwrap_or_default();
                self.tablas = tablas_presupuesto(&r);
                self.presu = Some(r);
            }
        }
        self.hoja = self.hoja.min(self.tablas.len().saturating_sub(1));
        Ok(self.resumen.clone())
    }

    fn exportar(&self, c: &Path) -> Result<Vec<PathBuf>, String> {
        match self.tipo {
            Tipo::Grid => {
                let (rd, r) = self.red.as_ref().ok_or("Calcule primero")?;
                let mut out = vec![escribir_excel(&c.join("caida_tension.xlsx"), &tablas_red_con_metrado(rd, r))?];
                if let Some(d) = dxf::plano_red(&c.join("plano_red.dxf"), rd, r, 2.0)? {
                    out.push(d);
                }
                Ok(out)
            }
            Tipo::Presu => {
                let r = self.presu.as_ref().ok_or("Calcule primero")?;
                Ok(vec![escribir_excel(&c.join("presupuesto.xlsx"), &tablas_presupuesto(r))?])
            }
        }
    }

    /// Copia las plantillas a la carpeta (sin sobrescribir) y devuelve la ruta del proyecto.
    fn crear(&self, carpeta: &Path) -> Result<PathBuf, String> {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
        let lista: &[(&str, &str)] = match self.tipo {
            Tipo::Grid => &PLANTILLAS_GRID,
            Tipo::Presu => &PLANTILLAS_PRESU,
        };
        for (n, t) in lista {
            let r = carpeta.join(n);
            if !r.exists() {
                std::fs::write(&r, t).map_err(|e| format!("{}: {e}", r.display()))?;
            }
        }
        Ok(carpeta.join(self.archivo()))
    }
}

/// `1,234,567.89` (formato `{:,.2f}` de Python).
fn miles(x: f64) -> String {
    let s = format!("{:.2}", x.abs());
    let (ent, dec) = s.split_once('.').unwrap();
    let mut o = String::new();
    for (i, c) in ent.chars().enumerate() {
        if i > 0 && (ent.len() - i) % 3 == 0 {
            o.push(',');
        }
        o.push(c);
    }
    format!("{}{o}.{dec}", if x < 0.0 { "-" } else { "" })
}

fn modulo(app: &mut App, tipo: Tipo) -> &mut ModuloJson {
    match tipo {
        Tipo::Grid => &mut app.grid,
        Tipo::Presu => &mut app.presu,
    }
}

/// Reemplaza en `metrado.csv` las filas del rubro por las nuevas (conserva los demás rubros).
fn fusionar_metrado(destino: &Path, nuevo: &str, rubro: &str) -> Result<(), String> {
    let mut lineas: Vec<String> = vec!["rubro,uc,cantidad".into()];
    if let Ok(t) = std::fs::read_to_string(destino) {
        lineas.extend(
            t.lines()
                .skip_while(|l| l.starts_with('#') || l.trim().is_empty())
                .skip(1)
                .filter(|l| !l.trim().is_empty() && l.split(',').next().map(str::trim) != Some(rubro))
                .map(String::from),
        );
    }
    lineas.extend(nuevo.lines().skip(1).map(String::from));
    std::fs::write(destino, lineas.join("
") + "
").map_err(|e| format!("{}: {e}", destino.display()))
}

fn metrado_a_presupuesto(app: &mut App, de_red: bool) {
    let (csv, rubro, origen) = if de_red {
        let Some((rd, _)) = &app.grid.red else {
            app.error = Some("Calcule primero la red en GridCraft".into());
            return;
        };
        (suite_motor::red::metrado_presupuesto(&red::metrado(rd), "RS"), "RS", "GridCraft")
    } else {
        let Some(c) = &app.calculo else {
            app.error = Some("Calcule primero la línea en LineCraft".into());
            return;
        };
        (metrado_presupuesto(&c.metrado, "LP"), "LP", "LineCraft")
    };
    let Some(ruta) = app.presu.ruta.clone() else {
        app.error = Some("Abra o cree primero un presupuesto".into());
        return;
    };
    let destino = ruta.parent().unwrap_or(Path::new(".")).join("metrado.csv");
    let r = fusionar_metrado(&destino, &csv, rubro).and_then(|_| app.presu.calcular());
    if let Some(m) = app.intentar(r) {
        app.estado = format!("Metrado de {origen} (rubro {rubro}) escrito en {} — {m}", destino.display());
    }
}

pub fn panel_json(app: &mut App, ui: &mut egui::Ui, tipo: Tipo) {
    egui::Panel::top(match tipo {
        Tipo::Grid => "grid_barra",
        Tipo::Presu => "presu_barra",
    })
    .show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            let archivo = modulo(app, tipo).archivo();
            if ui.button(format!("Abrir {archivo}…")).clicked() {
                if let Some(r) = rfd::FileDialog::new().add_filter("Proyecto", &["json"]).pick_file() {
                    modulo(app, tipo).ruta = Some(r);
                    let res = modulo(app, tipo).calcular();
                    if let Some(m) = app.intentar(res) {
                        app.estado = m;
                    }
                }
            }
            if ui.button("Nuevo (carpeta con plantillas)…").clicked() {
                if let Some(c) = elegir_carpeta("Carpeta del proyecto", None) {
                    let res = modulo(app, tipo).crear(&c);
                    if let Some(r) = app.intentar(res) {
                        modulo(app, tipo).ruta = Some(r.clone());
                        app.estado = format!("Plantillas creadas en {} — edite los CSV y el JSON (valores y fuentes) y pulse Calcular", c.display());
                    }
                }
            }
            let hay = modulo(app, tipo).ruta.is_some();
            if ui.add_enabled(hay, egui::Button::new(egui::RichText::new("Calcular").strong())).clicked() {
                let res = modulo(app, tipo).calcular();
                if let Some(m) = app.intentar(res) {
                    app.estado = m;
                }
            }
            let calculado = !modulo(app, tipo).tablas.is_empty();
            if ui.add_enabled(calculado, egui::Button::new("Exportar…")).clicked() {
                if let Some(c) = elegir_carpeta("Carpeta de salida", modulo(app, tipo).ruta.as_deref().and_then(Path::parent)) {
                    let res = modulo(app, tipo).exportar(&c);
                    if let Some(out) = app.intentar(res) {
                        app.estado = format!("Exportados {} archivo(s) en {}", out.len(), c.display());
                        abrir_externo(&c);
                    }
                }
            }
            if ui.add_enabled(hay, egui::Button::new("Abrir carpeta")).clicked() {
                if let Some(d) = modulo(app, tipo).ruta.as_deref().and_then(Path::parent) {
                    abrir_externo(d);
                }
            }
            if tipo == Tipo::Presu && ui.button("Metrado desde LineCraft").on_hover_text("Rubro LP de metrado.csv").clicked() {
                metrado_a_presupuesto(app, false);
            }
            if tipo == Tipo::Presu && ui.button("Metrado desde GridCraft").on_hover_text("Rubro RS de metrado.csv").clicked() {
                metrado_a_presupuesto(app, true);
            }
        });
        let m = modulo(app, tipo);
        ui.label(match &m.ruta {
            Some(r) => format!("Proyecto: {}", r.display()),
            None => "Sin proyecto: abra un JSON o cree una carpeta con plantillas.".into(),
        });
        if !m.resumen.is_empty() {
            ui.label(egui::RichText::new(&m.resumen).strong());
        }
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let m = modulo(app, tipo);
        if m.tablas.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            for (k, t) in m.tablas.iter().enumerate() {
                ui.selectable_value(&mut m.hoja, k, &t.nombre);
            }
        });
        ui.separator();
        let id = format!("{}_{}", m.archivo(), m.hoja);
        ver_tabla(ui, &m.tablas[m.hoja], &id);
    });
}
