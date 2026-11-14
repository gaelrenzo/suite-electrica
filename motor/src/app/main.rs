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
}

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

    fn nuevo(&mut self) {
        self.poner_documento(Documento::nuevo());
        self.estado = "Proyecto nuevo".into();
    }

    fn abrir(&mut self) {
        if let Some(r) = rfd::FileDialog::new().add_filter("Proyecto Suite Eléctrica", &["sel"]).pick_file() {
            if let Some(d) = self.intentar(Documento::abrir(&r)) {
                self.poner_documento(d);
                self.estado = format!("Abierto {}", r.display());
            }
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
                if ui.button("Guardar (Ctrl+S)").clicked() {
                    ui.close();
                    self.guardar(false);
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
            self.guardar(false);
        }
        if f6 {
            linea::calcular(self);
        }
        if z && self.pestana == Pestana::Linea && !ui.ctx().egui_wants_keyboard_input() {
            linea::deshacer(self);
        }
    }

    fn dialogos(&mut self, ctx: &egui::Context) {
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
                ui.label(format!("¿Guardar «{}» antes de salir?", self.doc.nombre));
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
                    if self.guardar(false) {
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
        if ctx.input(|i| i.viewport().close_requested()) && self.doc.modificado && self.cerrar != Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cerrar = Some(false);
        }
        let titulo = format!("Suite Eléctrica — {}{}", self.doc.nombre, if self.doc.modificado { " *" } else { "" });
        if titulo != self.titulo {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(titulo.clone()));
            self.titulo = titulo;
        }
        self.atajos(ui);
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
        match app.red.abrir(&r) {
            Ok(()) => {
                app.pestana = Pestana::Grid;
                app.estado = format!("Red abierta: {}", r.display());
            }
            Err(e) => app.error = Some(e),
        }
    }
    eframe::run_native("Suite Eléctrica", opciones, Box::new(|_cc| Ok(Box::new(app))))
}
