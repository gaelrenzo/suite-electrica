//! Pestañas TopoCraft, GridCraft y PresuCraft.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
use suite_motor::documento::metrado_presupuesto;
use suite_motor::salidas::{escribir_excel, tablas_perfil, tablas_presupuesto, Celda, Tabla};
use suite_motor::tin::es_mayor;
use suite_motor::{dxf, pdf, presupuesto, red};

use suite_motor::catalogos::ArchivoCsv;

use crate::{abrir_externo, elegir_carpeta, num, tablas, ver_tabla, App};

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
    let caj = |titulo: &str, lamina: &str, nota: &str| dxf::Cajetin {
        proyecto: app.doc.nombre.clone(),
        titulo: titulo.into(),
        autor: app.doc.autor.clone(),
        lamina: lamina.into(),
        nota: nota.into(),
    };
    let (c_plano, c_perfil) = (caj("PLANO TOPOGRÁFICO", "PT-01", ""), caj("PERFIL LONGITUDINAL DEL EJE", "PT-02", "EXAG. V 10x"));
    let (eq, mayor) = (app.doc.topo.equidistancia, app.doc.topo.mayor_cada);
    let eje = app.doc.eje.clone();
    let r = app.doc.topografia().and_then(|t| {
        let mut d = dxf::construir_plano_topo(&t.puntos, Some(&t.tin), &t.curvas, eq, mayor, eje.as_deref(), 1.0);
        d.lamina(&c_plano);
        let mut out = vec![d.guardar(&c.join("plano.dxf"))?];
        if let Some(p) = &t.perfil {
            pdf::perfil_pdf(&c.join("perfil.pdf"), p, None, None, "PERFIL LONGITUDINAL")?;
            out.push(c.join("perfil.pdf"));
            out.push(escribir_excel(&c.join("perfil.xlsx"), &tablas_perfil(p, &[("Proyecto".into(), Celda::Txt(nombre))]))?);
            let mut d = dxf::construir_perfil_topo(p, 10.0);
            d.lamina(&c_perfil);
            out.push(d.guardar(&c.join("perfil.dxf"))?);
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
    Presu,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ver {
    Parametros,
    Dato(usize),
    Resultado,
}

pub struct ModuloJson {
    tipo: Tipo,
    pub ruta: Option<PathBuf>,
    tablas: Vec<Tabla>,
    resumen: String,
    hoja: usize,
    presu: Option<presupuesto::ResultadoPresupuesto>,
    datos: Vec<ArchivoCsv>,
    params: serde_json::Map<String, serde_json::Value>,
    params_mod: bool,
    ver: Ver,
}

/// Tablas del presupuesto editables en la app: (clave del JSON, título).
const TABLAS_PRESU: [(&str, &str); 6] = [
    ("suministros", "Suministros"),
    ("unidades", "Unidades (UC)"),
    ("montaje", "Montaje"),
    ("metrado", "Metrado"),
    ("apu", "APU"),
    ("fp_agrupacion", "Agrupación FP"),
];
const CABECERA_APU: &str = "partida,descripcion,unidad,rendimiento,tipo,insumo,unidad_insumo,cuadrilla,cantidad,precio,iu,base";
const CABECERA_FP: &str = "iu,similar,monomio";

const PLANTILLAS_PRESU: [(&str, &str); 5] = [
    ("presupuesto.json", include_str!("../../../suite_electrica/plantillas/presu_presupuesto.json")),
    ("suministros.csv", include_str!("../../../suite_electrica/plantillas/presu_suministros.csv")),
    ("unidades.csv", include_str!("../../../suite_electrica/plantillas/presu_unidades.csv")),
    ("montaje.csv", include_str!("../../../suite_electrica/plantillas/presu_montaje.csv")),
    ("metrado.csv", include_str!("../../../suite_electrica/plantillas/presu_metrado.csv")),
];

impl ModuloJson {
    pub fn nuevo(tipo: Tipo) -> Self {
        ModuloJson {
            tipo,
            ruta: None,
            tablas: Vec::new(),
            resumen: String::new(),
            hoja: 0,
            presu: None,
            datos: Vec::new(),
            params: Default::default(),
            params_mod: false,
            ver: Ver::Parametros,
        }
    }

    fn archivo(&self) -> &'static str {
        match self.tipo {
            Tipo::Presu => "presupuesto.json",
        }
    }

    /// Lee el JSON y las tablas CSV que nombra (descarta lo no guardado).
    fn cargar_datos(&mut self) -> Result<(), String> {
        let ruta = self.ruta.clone().ok_or("Abra o cree un proyecto")?;
        let t = std::fs::read_to_string(&ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let v: serde_json::Value = serde_json::from_str(&t).map_err(|e| format!("{}: {e}", ruta.display()))?;
        self.params = v.as_object().cloned().unwrap_or_default();
        let base = ruta.parent().unwrap_or(Path::new("."));
        self.datos.clear();
        for (k, titulo) in TABLAS_PRESU {
            if let Some(f) = self.params.get(k).and_then(|v| v.as_str()) {
                let mut a = ArchivoCsv::abrir(titulo, &base.join(f))?;
                if k == "suministros" {
                    a.asegurar_columna("iu"); // índice unificado, para la fórmula polinómica
                }
                self.datos.push(a);
            }
        }
        self.params_mod = false;
        if let Ver::Dato(k) = self.ver {
            if k >= self.datos.len() {
                self.ver = Ver::Parametros;
            }
        }
        Ok(())
    }

    /// Guarda las tablas y parámetros modificados; devuelve cuántos archivos escribió.
    fn guardar_datos(&mut self) -> Result<usize, String> {
        let mut n = 0;
        for a in self.datos.iter_mut().filter(|a| a.modificado) {
            a.guardar()?;
            n += 1;
        }
        if self.params_mod {
            let ruta = self.ruta.clone().ok_or("Sin proyecto")?;
            let t = serde_json::to_string_pretty(&serde_json::Value::Object(self.params.clone())).map_err(|e| e.to_string())?;
            std::fs::write(&ruta, t + "\n").map_err(|e| format!("{}: {e}", ruta.display()))?;
            self.params_mod = false;
            n += 1;
        }
        Ok(n)
    }

    fn pendientes(&self) -> bool {
        self.params_mod || self.datos.iter().any(|a| a.modificado)
    }

    /// Crea una tabla vacía (solo cabecera) y la enlaza en el JSON.
    fn crear_tabla(&mut self, clave: &str, archivo: &str, cabecera: &str) -> Result<(), String> {
        let ruta = self.ruta.clone().ok_or("Sin proyecto")?;
        let destino = ruta.parent().unwrap_or(Path::new(".")).join(archivo);
        if !destino.exists() {
            std::fs::write(&destino, format!("{cabecera}\n")).map_err(|e| format!("{}: {e}", destino.display()))?;
        }
        self.params.insert(clave.into(), serde_json::json!(archivo));
        self.params_mod = true;
        self.guardar_datos()?;
        self.cargar_datos()?;
        if let Some(k) = self.datos.iter().position(|a| a.ruta == destino) {
            self.ver = Ver::Dato(k);
        }
        Ok(())
    }

    /// Guarda lo editado, calcula y vuelve a leer las tablas.
    fn calcular(&mut self) -> Result<String, String> {
        let ruta = self.ruta.clone().ok_or("Abra o cree un proyecto")?;
        self.guardar_datos()?;
        match self.tipo {
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
        self.cargar_datos()?;
        Ok(self.resumen.clone())
    }

    fn exportar(&self, c: &Path) -> Result<Vec<PathBuf>, String> {
        match self.tipo {
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
    std::fs::write(destino, lineas.join("\n") + "\n").map_err(|e| format!("{}: {e}", destino.display()))
}

fn metrado_a_presupuesto(app: &mut App, de_red: bool) {
    let (csv, rubro, origen) = if de_red {
        let Some(rd) = &app.red.red else {
            app.error = Some("Abra o dibuje primero la red en GridCraft".into());
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
    let r = app.presu.guardar_datos().and_then(|_| fusionar_metrado(&destino, &csv, rubro)).and_then(|_| app.presu.calcular());
    if let Some(m) = app.intentar(r) {
        app.estado = format!("Metrado de {origen} (rubro {rubro}) escrito en {} — {m}", destino.display());
    }
}

pub fn panel_json(app: &mut App, ui: &mut egui::Ui, tipo: Tipo) {
    egui::Panel::top(match tipo {
        Tipo::Presu => "presu_barra",
    })
    .show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            let archivo = modulo(app, tipo).archivo();
            if ui.button(format!("Abrir {archivo}…")).clicked() {
                if let Some(r) = rfd::FileDialog::new().add_filter("Proyecto", &["json"]).pick_file() {
                    abrir_presu(app, tipo, r);
                }
            }
            if ui.button("Nuevo (carpeta con plantillas)…").clicked() {
                if let Some(c) = elegir_carpeta("Carpeta del proyecto", None) {
                    let res = modulo(app, tipo).crear(&c);
                    if let Some(r) = app.intentar(res) {
                        abrir_presu(app, tipo, r);
                        app.estado = format!("Plantillas creadas en {} — edite las tablas en «Datos» (valores y fuentes) y pulse Calcular", c.display());
                    }
                }
            }
            let hay = modulo(app, tipo).ruta.is_some();
            let pend = modulo(app, tipo).pendientes();
            if ui.add_enabled(pend, egui::Button::new("Guardar")).clicked() {
                let res = modulo(app, tipo).guardar_datos();
                if let Some(n) = app.intentar(res) {
                    app.estado = format!("Guardados {n} archivo(s)");
                }
            }
            if ui.add_enabled(pend, egui::Button::new("Descartar cambios")).clicked() {
                let res = modulo(app, tipo).cargar_datos();
                if app.intentar(res).is_some() {
                    app.estado = "Cambios descartados".into();
                }
            }
            if ui.add_enabled(hay, egui::Button::new(egui::RichText::new("Calcular").strong())).on_hover_text("Guarda lo editado y calcula").clicked() {
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
        if m.ruta.is_none() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Datos:").strong());
            ui.selectable_value(&mut m.ver, Ver::Parametros, if m.params_mod { "Parámetros *" } else { "Parámetros" });
            for (k, a) in m.datos.iter().enumerate() {
                ui.selectable_value(&mut m.ver, Ver::Dato(k), if a.modificado { format!("{} *", a.nombre) } else { a.nombre.clone() });
            }
            if !m.tablas.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Resultados:").strong());
                for (k, t) in m.tablas.iter().enumerate() {
                    if ui.selectable_label(m.ver == Ver::Resultado && m.hoja == k, &t.nombre).clicked() {
                        m.ver = Ver::Resultado;
                        m.hoja = k;
                    }
                }
            }
        });
        ui.separator();
        let mut crear = None;
        match m.ver {
            Ver::Parametros => crear = parametros_presu(ui, m),
            Ver::Dato(k) => {
                let a = &mut m.datos[k];
                ui.label(egui::RichText::new(a.ruta.display().to_string()).small());
                if tablas::editar(ui, &format!("presu_dato_{k}"), &a.columnas, &mut a.filas, &mut a.filtro) {
                    a.modificado = true;
                }
            }
            Ver::Resultado => {
                let id = format!("{}_{}", m.archivo(), m.hoja);
                ver_tabla(ui, &m.tablas[m.hoja], &id);
            }
        }
        if let Some((clave, archivo, cab)) = crear {
            let res = modulo(app, tipo).crear_tabla(clave, archivo, cab);
            app.intentar(res);
        }
    });
}

pub fn abrir_presu(app: &mut App, tipo: Tipo, r: PathBuf) {
    let m = modulo(app, tipo);
    m.ruta = Some(r.clone());
    m.tablas.clear();
    m.presu = None;
    m.resumen.clear();
    let res = m.cargar_datos();
    if app.intentar(res).is_none() {
        return;
    }
    let res = modulo(app, tipo).calcular();
    if let Some(msg) = app.intentar(res) {
        modulo(app, tipo).ver = Ver::Resultado;
        app.estado = format!("{} — {msg}", r.display());
    }
}

/// Parámetros del presupuesto (claves del JSON). Devuelve una tabla a crear, si se pidió.
fn parametros_presu(ui: &mut egui::Ui, m: &mut ModuloJson) -> Option<(&'static str, &'static str, &'static str)> {
    use serde_json::json;
    let mut cambio = false;
    let mut crear = None;
    egui::ScrollArea::vertical().id_salt("presu_param").show(ui, |ui| {
        egui::Grid::new("presu_param_g").num_columns(2).show(ui, |ui| {
            for (k, t) in [("nombre", "Nombre"), ("moneda", "Moneda")] {
                let mut v = m.params.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                ui.label(t);
                if ui.text_edit_singleline(&mut v).changed() {
                    m.params.insert(k.into(), json!(v));
                    cambio = true;
                }
                ui.end_row();
            }
            for (k, t, suf, vel) in [
                ("flete_por_kg", "Flete por kg", "", 0.01),
                ("gastos_generales_pct", "Gastos generales", " %", 0.1),
                ("utilidad_pct", "Utilidad", " %", 0.1),
                ("igv_pct", "IGV", " %", 0.1),
            ] {
                let mut x = m.params.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
                if num(ui, t, &mut x, vel, suf) {
                    m.params.insert(k.into(), json!(x));
                    cambio = true;
                }
            }
        });
        ui.separator();
        ui.label(egui::RichText::new("Fuentes de los valores (cite la norma, cotización o estudio de mercado)").strong());
        let mut fuentes = m.params.get("fuentes").and_then(|v| v.as_object()).cloned().unwrap_or_default();
        egui::Grid::new("presu_fuentes").num_columns(2).show(ui, |ui| {
            for k in ["flete_por_kg", "gastos_generales_pct", "utilidad_pct", "igv_pct"] {
                let mut v = fuentes.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                ui.label(k);
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(420.0)).changed() {
                    fuentes.insert(k.into(), json!(v));
                    cambio = true;
                }
                ui.end_row();
            }
        });
        if cambio {
            m.params.insert("fuentes".into(), serde_json::Value::Object(fuentes));
        }
        ui.separator();
        ui.horizontal(|ui| {
            if !m.params.contains_key("apu") && ui.button("Crear tabla de APU").on_hover_text("apu.csv: análisis de precios unitarios (reemplazan el precio de montaje)").clicked() {
                crear = Some(("apu", "apu.csv", CABECERA_APU));
            }
            if !m.params.contains_key("fp_agrupacion") && ui.button("Crear agrupación de la fórmula polinómica").on_hover_text("fp_agrupacion.csv: IU, similar y monomio").clicked() {
                crear = Some(("fp_agrupacion", "fp_agrupacion.csv", CABECERA_FP));
            }
        });
    });
    if cambio {
        m.params_mod = true;
    }
    crear
}
