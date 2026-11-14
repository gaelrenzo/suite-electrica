//! Suite Eléctrica: aplicación de escritorio (un documento `.sel` y una pestaña por módulo).
//!
//! Port de `suite_electrica/gui` (PySide6) a egui. Flujos: TopoCraft (importar puntos y eje →
//! procesar → revisar planta y perfil → exportar), LineCraft (calcular → editar estructuras en el
//! perfil o la planilla → revisar resultados → exportar), GridCraft y PresuCraft (proyecto JSON →
//! calcular → revisar → exportar).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod linea;
mod otros;
mod red;
mod tablas;

use std::path::{Path, PathBuf};

use eframe::egui;
use suite_motor::documento::{Calculo, Documento};
use suite_motor::salidas::{Celda, Tabla};

pub const REPO: &str = "https://github.com/gaelrenzo/suite-electrica";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pestana {
    Topo,
    Linea,
    Grid,
    Presu,
}

pub struct App {
    pub doc: Documento,
    pub pestana: Pestana,
    pub estado: String,
    pub error: Option<String>,
    pub calculo: Option<Calculo>,
    /// Datos cambiados desde el último cálculo.
    pub obsoleto: bool,
    pub linea: linea::EstadoLinea,
    pub red: red::EstadoRed,
    pub presu: otros::ModuloJson,
    cerrar: Option<bool>,
    titulo: String,
    /// Archivos abiertos o guardados hace poco (.sel, red.json, presupuesto.json).
    recientes: Vec<PathBuf>,
    /// Última ruta vista de cada módulo (documento, red, presupuesto), para registrar recientes.
    vistos: [Option<PathBuf>; 3],
    ayuda: bool,
}

const MAX_RECIENTES: usize = 8;

fn ruta_recientes() -> Option<PathBuf> {
    std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(|d| PathBuf::from(d).join("SuiteElectrica").join("recientes.txt"))
}

fn leer_recientes() -> Vec<PathBuf> {
    let Some(t) = ruta_recientes().and_then(|r| std::fs::read_to_string(r).ok()) else { return Vec::new() };
    t.lines().map(PathBuf::from).filter(|p| p.exists()).take(MAX_RECIENTES).collect()
}

const GUIA: &str = "\
FLUJO DE TRABAJO
1. TopoCraft: importar puntos (CSV, TXT, DXF, KML, GPX, Excel) y el eje; procesar (TIN, curvas, perfil).
2. LineCraft: Calcular (F6) ubica las estructuras; editarlas en el perfil, la planta o la planilla;
   revisar vanos, cargas, verificación y alertas; Exportar (perfil DXF con lámina, Excel, memoria PDF).
3. GridCraft: Nueva red o abrir red.json; dibujar los postes en planta (Agregar poste cuelga del
   seleccionado), completar sus datos en el panel derecho; Calcular colorea los tramos por %ΔV.
4. PresuCraft: editar las tablas en «Datos»; traer el metrado de LineCraft (LP) o GridCraft (RS);
   Calcular guarda y calcula; Exportar a Excel.

ATAJOS
Ctrl+N nuevo proyecto · Ctrl+O abrir · Ctrl+S guardar (el módulo de la pestaña activa)
F6 calcular (pestaña activa) · Ctrl+Z deshacer (LineCraft y GridCraft) · Supr eliminar estructura/poste
Rueda: zoom · Arrastrar: mueve el elemento (o la vista, si se desactiva «mover con el ratón»)

VALORES NORMATIVOS
Las plantillas traen valores marcados PENDIENTE o EJEMPLO: cite la fuente (CNE, normas DGE,
cotizaciones) en «Fuentes» antes de usar los resultados en un proyecto real.";

impl App {
    fn new(doc: Documento) -> Self {
        App {
            doc,
            pestana: Pestana::Topo,
            estado: "Listo — importe puntos y eje en TopoCraft o abra un proyecto (Ctrl+O)".into(),
            error: None,
            calculo: None,
            obsoleto: false,
            linea: linea::EstadoLinea::default(),
            red: red::EstadoRed::default(),
            presu: otros::ModuloJson::nuevo(otros::Tipo::Presu),
            cerrar: None,
            titulo: String::new(),
            recientes: leer_recientes(),
            vistos: [None, None, None],
            ayuda: false,
        }
    }

    /// Ejecuta una acción y muestra su error, si lo hay.
    pub fn intentar<T>(&mut self, r: Result<T, String>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.estado = format!("Error: {e}");
                self.error = Some(e);
                None
            }
        }
    }

    pub fn poner_documento(&mut self, doc: Documento) {
        self.doc = doc;
        self.calculo = None;
        self.obsoleto = false;
        self.linea = linea::EstadoLinea::default();
    }

    /// Registra un archivo reciente (primero de la lista) y guarda la lista.
    fn recordar(&mut self, r: &Path) {
        let r = std::fs::canonicalize(r).unwrap_or_else(|_| r.to_path_buf());
        let r = PathBuf::from(r.to_string_lossy().trim_start_matches(r"\\?\").to_string());
        self.recientes.retain(|x| x != &r);
        self.recientes.insert(0, r);
        self.recientes.truncate(MAX_RECIENTES);
        if let Some(f) = ruta_recientes() {
            let _ = f.parent().map(std::fs::create_dir_all);
            let t: Vec<String> = self.recientes.iter().map(|p| p.display().to_string()).collect();
            let _ = std::fs::write(f, t.join("\n"));
        }
    }

    /// Abre un `.sel` (LineCraft/TopoCraft), un presupuesto o una red (JSON) según su contenido.
    pub fn abrir_ruta(&mut self, r: PathBuf) {
        let es_json = r.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if !es_json {
            if let Some(d) = self.intentar(Documento::abrir(&r)) {
                self.poner_documento(d);
                self.estado = format!("Abierto {}", r.display());
            }
        } else if std::fs::read_to_string(&r).is_ok_and(|t| t.contains("\"suministros\"")) {
            otros::abrir_presu(self, otros::Tipo::Presu, r);
            self.pestana = Pestana::Presu;
        } else {
            let res = self.red.abrir(&r);
            if self.intentar(res).is_some() {
                self.pestana = Pestana::Grid;
                self.estado = format!("Red abierta: {}", r.display());
            }
        }
    }

    /// Hay cambios sin guardar en el proyecto, la red o el presupuesto.
    fn hay_cambios(&self) -> bool {
        self.doc.modificado || self.red.modificado || self.presu.pendientes()
    }

    /// Guarda todo lo modificado; `false` si algo falló o se canceló.
    fn guardar_todo(&mut self) -> bool {
        if self.red.modificado {
            let r = self.red.guardar();
            if self.intentar(r).is_none() {
                return false;
            }
        }
        if self.presu.pendientes() {
            let r = self.presu.guardar_datos();
            if self.intentar(r).is_none() {
                return false;
            }
        }
        !self.doc.modificado || self.guardar(false)
    }

    /// Ctrl+S: guarda el módulo de la pestaña activa.
    fn guardar_activo(&mut self) {
        match self.pestana {
            Pestana::Grid if self.red.red.is_some() => {
                let r = self.red.guardar();
                if let Some(p) = self.intentar(r) {
                    self.estado = format!("Red guardada en {}", p.display());
                }
            }
            Pestana::Presu if self.presu.ruta.is_some() => {
                let r = self.presu.guardar_datos();
                if let Some(n) = self.intentar(r) {
                    self.estado = format!("Presupuesto: {n} archivo(s) guardado(s)");
                }
            }
            _ => {
                self.guardar(false);
            }
        }
    }

    /// F6: calcula el módulo de la pestaña activa.
    fn calcular_activo(&mut self) {
        match self.pestana {
            Pestana::Grid => {
                let r = self.red.calcular();
                if let Some(m) = self.intentar(r) {
                    self.estado = m;
                }
            }
            Pestana::Presu => {
                let r = self.presu.calcular();
                if let Some(m) = self.intentar(r) {
                    self.estado = m;
                }
            }
            _ => linea::calcular(self),
        }
    }

    fn nuevo(&mut self) {
        self.poner_documento(Documento::nuevo());
        self.estado = "Proyecto nuevo".into();
    }

    fn abrir(&mut self) {
        if let Some(r) = rfd::FileDialog::new()
            .add_filter("Proyecto Suite Eléctrica", &["sel"])
            .add_filter("Red de GridCraft o presupuesto (JSON)", &["json"])
            .pick_file()
        {
            self.abrir_ruta(r);
        }
    }

    pub fn guardar(&mut self, como: bool) -> bool {
        let ruta = if como || self.doc.ruta.is_none() {
            match rfd::FileDialog::new()
                .add_filter("Proyecto Suite Eléctrica", &["sel"])
                .set_file_name(format!("{}.sel", self.doc.nombre))
                .save_file()
            {
                Some(r) => Some(r),
                None => return false,
            }
        } else {
            None
        };
        let res = self.doc.guardar(ruta.as_deref());
        match self.intentar(res) {
            Some(r) => {
                self.estado = format!("Guardado {}", r.display());
                true
            }
            None => false,
        }
    }

    fn importar_json(&mut self) {
        if let Some(r) = rfd::FileDialog::new().add_filter("Proyecto LineCraft", &["json"]).pick_file() {
            if let Some(d) = self.intentar(Documento::desde_linecraft_json(&r)) {
                self.poner_documento(d);
                self.pestana = Pestana::Linea;
                self.estado = format!("Importado {} — pulse Calcular (F6)", r.display());
            }
        }
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Archivo", |ui| {
                if ui.button("Nuevo proyecto (Ctrl+N)").clicked() {
                    ui.close();
                    self.nuevo();
                }
                if ui.button("Abrir… (Ctrl+O)").clicked() {
                    ui.close();
                    self.abrir();
                }
                ui.add_enabled_ui(!self.recientes.is_empty(), |ui| {
                    ui.menu_button("Abrir reciente", |ui| {
                        for r in self.recientes.clone() {
                            if ui.button(r.display().to_string()).clicked() {
                                ui.close();
                                self.abrir_ruta(r);
                            }
                        }
                    });
                });
                if ui.button("Guardar (Ctrl+S)").on_hover_text("Guarda el módulo de la pestaña activa").clicked() {
                    ui.close();
                    self.guardar_activo();
                }
                if ui.button("Guardar todo").clicked() {
                    ui.close();
                    if self.guardar_todo() {
                        self.estado = "Todo guardado".into();
                    }
                }
                if ui.button("Guardar como…").clicked() {
                    ui.close();
                    self.guardar(true);
                }
                ui.separator();
                if ui.button("Importar puntos topográficos…").clicked() {
                    ui.close();
                    otros::importar_puntos(self);
                }
                if ui.button("Importar eje de línea…").clicked() {
                    ui.close();
                    otros::importar_eje(self);
                }
                if ui.button("Importar proyecto LineCraft (.json)…").clicked() {
                    ui.close();
                    self.importar_json();
                }
                ui.separator();
                if ui.button("Salir").clicked() {
                    ui.close();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Ayuda", |ui| {
                if ui.button("Guía rápida y atajos").clicked() {
                    ui.close();
                    self.ayuda = true;
                }
                ui.separator();
                ui.label(format!("Suite Eléctrica {} (Rust)", env!("CARGO_PKG_VERSION")));
                ui.label("Software libre (MIT). Los valores normativos deben citar su fuente.");
                ui.hyperlink_to("Repositorio y documentación", REPO);
            });
        });
    }

    fn atajos(&mut self, ui: &mut egui::Ui) {
        let (n, o, s, f6, z) = ui.input(|i| {
            let c = i.modifiers.command;
            (c && i.key_pressed(egui::Key::N), c && i.key_pressed(egui::Key::O), c && i.key_pressed(egui::Key::S),
             i.key_pressed(egui::Key::F6), c && i.key_pressed(egui::Key::Z))
        });
        if n {
            self.nuevo();
        }
        if o {
            self.abrir();
        }
        if s {
            self.guardar_activo();
        }
        if f6 {
            self.calcular_activo();
        }
        if z && self.pestana == Pestana::Linea && !ui.ctx().egui_wants_keyboard_input() {
            linea::deshacer(self);
        }
    }

    fn dialogos(&mut self, ctx: &egui::Context) {
        if self.ayuda {
            egui::Window::new("Guía rápida").open(&mut self.ayuda).default_width(720.0).show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| ui.label(egui::RichText::new(GUIA).monospace()));
                ui.hyperlink_to("Documentación completa en el repositorio", REPO);
            });
        }
        if let Some(e) = self.error.clone() {
            let m = egui::Modal::new(egui::Id::unique("error")).show(ctx, |ui| {
                ui.set_max_width(520.0);
                ui.heading("No se pudo completar la operación");
                ui.label(e);
                ui.add_space(8.0);
                ui.button("Aceptar").clicked()
            });
            if m.inner || m.should_close() {
                self.error = None;
            }
        }
        if self.cerrar == Some(false) {
            let m = egui::Modal::new(egui::Id::unique("cerrar")).show(ctx, |ui| {
                ui.heading("Hay cambios sin guardar");
                let mut que = Vec::new();
                if self.doc.modificado {
                    que.push(format!("el proyecto «{}»", self.doc.nombre));
                }
                if self.red.modificado {
                    que.push("la red de GridCraft".to_string());
                }
                if self.presu.pendientes() {
                    que.push("las tablas del presupuesto".to_string());
                }
                ui.label(format!("¿Guardar {} antes de salir?", que.join(", ")));
                ui.horizontal(|ui| {
                    if ui.button("Guardar").clicked() {
                        return Some(1);
                    }
                    if ui.button("Salir sin guardar").clicked() {
                        return Some(2);
                    }
                    if ui.button("Cancelar").clicked() {
                        return Some(0);
                    }
                    None
                })
                .inner
            });
            match m.inner {
                Some(1) => {
                    if self.guardar_todo() {
                        self.cerrar = Some(true);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Some(2) => {
                    self.cerrar = Some(true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Some(_) => self.cerrar = None,
                None if m.should_close() => self.cerrar = None,
                None => {}
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && self.hay_cambios() && self.cerrar != Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cerrar = Some(false);
        }
        let titulo = format!("Suite Eléctrica — {}{}", self.doc.nombre, if self.doc.modificado { " *" } else { "" });
        if titulo != self.titulo {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(titulo.clone()));
            self.titulo = titulo;
        }
        self.atajos(ui);
        let rutas = [self.doc.ruta.clone(), self.red.ruta.clone(), self.presu.ruta.clone()];
        for (k, r) in rutas.into_iter().enumerate() {
            if r.is_some() && r != self.vistos[k] {
                self.recordar(r.as_deref().unwrap());
                self.vistos[k] = r;
            }
        }
        egui::Panel::top("menu").show(ui, |ui| self.menus(ui));
        egui::Panel::top("pestanas").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (p, t) in [
                    (Pestana::Topo, "TopoCraft · Topografía"),
                    (Pestana::Linea, "LineCraft · Línea MT"),
                    (Pestana::Grid, "GridCraft · Redes"),
                    (Pestana::Presu, "PresuCraft · Presupuesto"),
                ] {
                    ui.selectable_value(&mut self.pestana, p, egui::RichText::new(t).strong());
                }
            });
        });
        egui::Panel::bottom("estado").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.estado);
            });
        });
        match self.pestana {
            Pestana::Topo => otros::panel_topo(self, ui),
            Pestana::Linea => linea::panel(self, ui),
            Pestana::Grid => red::panel(self, ui),
            Pestana::Presu => otros::panel_json(self, ui, otros::Tipo::Presu),
        }
        self.dialogos(&ctx);
    }
}

// ---------------------------------------------------------------- utilidades de interfaz

/// Tabla de resultados (solo lectura) con encabezado y filas alternadas.
pub fn ver_tabla(ui: &mut egui::Ui, tb: &Tabla, id: &str) {
    egui::ScrollArea::both().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new(id).striped(true).num_columns(tb.columnas.len()).show(ui, |ui| {
            for c in &tb.columnas {
                ui.label(egui::RichText::new(c).strong());
            }
            ui.end_row();
            for f in &tb.filas {
                for c in f {
                    let t = c.texto();
                    match c {
                        Celda::Txt(s) if s == "NO" => ui.colored_label(egui::Color32::from_rgb(200, 40, 40), t),
                        Celda::Num(_) | Celda::Ent(_) => ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(t)).inner,
                        _ => ui.label(t),
                    };
                }
                ui.end_row();
            }
        });
    });
}

/// Elige una carpeta de salida.
pub fn elegir_carpeta(titulo: &str, inicial: Option<&Path>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(titulo);
    if let Some(i) = inicial {
        d = d.set_directory(i);
    }
    d.pick_folder()
}

/// Abre un archivo o carpeta con el programa del sistema.
pub fn abrir_externo(ruta: &Path) {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(ruta).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(ruta).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(ruta).spawn();
    let _ = r;
}

/// Campo numérico con etiqueta (devuelve true si cambió).
pub fn num(ui: &mut egui::Ui, etiqueta: &str, v: &mut f64, velocidad: f64, sufijo: &str) -> bool {
    ui.label(etiqueta);
    let r = ui.add(egui::DragValue::new(v).speed(velocidad).suffix(sufijo)).changed();
    ui.end_row();
    r
}

fn main() -> eframe::Result {
    // `suite-electrica proyecto.sel` abre el proyecto; `suite-electrica red.json`, la red en GridCraft
    let arg = std::env::args().nth(1).map(PathBuf::from);
    let es_json = arg.as_ref().is_some_and(|r| r.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")));
    let doc = arg.as_ref().filter(|_| !es_json).and_then(|r| Documento::abrir(r).ok()).unwrap_or_default();
    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 760.0]).with_min_inner_size([900.0, 600.0]).with_maximized(true),
        ..Default::default()
    };
    let mut app = App::new(doc);
    if let Some(r) = arg.filter(|_| es_json) {
        app.abrir_ruta(r);
    }
    eframe::run_native("Suite Eléctrica", opciones, Box::new(|_cc| Ok(Box::new(app))))
}
