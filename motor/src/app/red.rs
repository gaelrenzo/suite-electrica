//! Pestaña GridCraft: la red se dibuja y edita en planta (como en REDCAD) y se calcula en el
//! mismo plano. Herramientas: seleccionar/mover (el vano se ajusta a las coordenadas), agregar
//! poste (cuelga del seleccionado), eliminar. Fondo: un DXF y/o la topografía del proyecto.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoint, PlotPoints, Points, Text};
use suite_motor::red::{self, longitud_plano, ConductorRed, Nodo, Red, ResultadoRed};
use suite_motor::salidas::{escribir_excel, tablas_red_con_metrado, Tabla};
use suite_motor::dxf;

use crate::{abrir_externo, elegir_carpeta, num, ver_tabla, App};

const PLANTILLAS: [(&str, &str); 3] = [
    ("red.json", include_str!("../../../suite_electrica/plantillas/red_proyecto.json")),
    ("nodos.csv", include_str!("../../../suite_electrica/plantillas/red_nodos.csv")),
    ("conductores_bt.csv", include_str!("../../../suite_electrica/plantillas/conductores_bt.csv")),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Seleccionar,
    Agregar,
    Eliminar,
}

pub struct EstadoRed {
    pub ruta: Option<PathBuf>,
    pub red: Option<Red>,
    pub resultado: Option<ResultadoRed>,
    tablas: Vec<Tabla>,
    resumen: String,
    hoja: usize,
    sel: Option<usize>,
    modo: Modo,
    arrastre: Option<usize>,
    mover_con_raton: bool,
    fondo: Vec<Vec<[f64; 2]>>,
    ver_topo: bool,
    conductor_nuevo: String,
    obsoleto: bool,
    pila: Vec<Vec<Nodo>>,
    pub modificado: bool,
}

impl Default for EstadoRed {
    fn default() -> Self {
        EstadoRed {
            ruta: None,
            red: None,
            resultado: None,
            tablas: Vec::new(),
            resumen: String::new(),
            hoja: 0,
            sel: None,
            modo: Modo::Seleccionar,
            arrastre: None,
            mover_con_raton: true,
            fondo: Vec::new(),
            ver_topo: true,
            conductor_nuevo: String::new(),
            obsoleto: false,
            pila: Vec::new(),
            modificado: false,
        }
    }
}

impl EstadoRed {
    fn apilar(&mut self) {
        if let Some(r) = &self.red {
            self.pila.push(r.nodos.clone());
            if self.pila.len() > 100 {
                self.pila.remove(0);
            }
        }
        self.obsoleto = true;
        self.modificado = true;
    }

    fn deshacer(&mut self) {
        if let (Some(r), Some(n)) = (&mut self.red, self.pila.pop()) {
            r.nodos = n;
            self.sel = None;
            self.obsoleto = true;
        }
    }

    pub fn abrir(&mut self, ruta: &Path) -> Result<(), String> {
        let r = red::cargar(ruta)?;
        self.conductor_nuevo = r.conductores.first().map(|c| c.formacion.clone()).unwrap_or_default();
        self.red = Some(r);
        self.ruta = Some(ruta.to_path_buf());
        self.resultado = None;
        self.tablas.clear();
        self.sel = None;
        self.pila.clear();
        self.modificado = false;
        self.calcular().map(|_| ())
    }

    pub fn calcular(&mut self) -> Result<String, String> {
        let rd = self.red.as_ref().ok_or("Abra o cree una red")?;
        let r = red::calcular(rd)?;
        let g = |k: &str| r.resumen.iter().find(|x| x.0 == k).map_or(f64::NAN, |x| x.1);
        self.resumen = format!(
            "{} — %ΔV máx. {:.2} %, M.D. {:.2} kW, pérdidas {:.2} %, {} alerta(s)",
            if r.alertas.is_empty() { "CUMPLE" } else { "NO CUMPLE" },
            g("%ΔV máx."),
            g("M.D. total (kW)"),
            g("Pérdidas (% M.D.)"),
            r.alertas.len()
        );
        self.tablas = tablas_red_con_metrado(rd, &r);
        self.hoja = self.hoja.min(self.tablas.len().saturating_sub(1));
        self.resultado = Some(r);
        self.obsoleto = false;
        Ok(self.resumen.clone())
    }

    pub fn guardar(&mut self) -> Result<PathBuf, String> {
        let (rd, ruta) = (self.red.as_ref().ok_or("No hay red")?, self.ruta.clone().ok_or("Sin archivo")?);
        red::guardar(rd, &ruta)?;
        self.modificado = false;
        Ok(ruta)
    }

    fn exportar(&self, c: &Path) -> Result<Vec<PathBuf>, String> {
        let (rd, r) = (self.red.as_ref().ok_or("No hay red")?, self.resultado.as_ref().ok_or("Calcule primero")?);
        let mut out = vec![escribir_excel(&c.join("caida_tension.xlsx"), &tablas_red_con_metrado(rd, r))?];
        if let Some(mut d) = dxf::construir_plano_red(rd, r, 2.0)? {
            d.lamina(&dxf::Cajetin {
                proyecto: rd.nombre.clone(),
                titulo: "RED SECUNDARIA - CAÍDA DE TENSIÓN".into(),
                lamina: "RS-01".into(),
                ..Default::default()
            });
            out.push(d.guardar(&c.join("plano_red.dxf"))?);
        }
        Ok(out)
    }
}

fn crear(carpeta: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    for (n, t) in PLANTILLAS {
        let r = carpeta.join(n);
        if !r.exists() {
            std::fs::write(&r, t).map_err(|e| format!("{}: {e}", r.display()))?;
        }
    }
    Ok(carpeta.join("red.json"))
}

/// Recalcula la longitud de los tramos del nodo `i` y de sus hijos según el plano.
fn ajustar_longitudes(rd: &mut Red, i: usize) {
    let nombre = rd.nodos[i].nodo.clone();
    let afectados: Vec<usize> = (0..rd.nodos.len()).filter(|&k| k == i || rd.nodos[k].padre == nombre).collect();
    for k in afectados {
        if let Some(l) = longitud_plano(rd, k) {
            rd.nodos[k].longitud_m = (l * 100.0).round() / 100.0;
        }
    }
}

fn nombre_libre(rd: &Red) -> String {
    (1..).map(|k| format!("{k:02}")).find(|n| !rd.nodos.iter().any(|x| &x.nodo == n)).unwrap()
}

fn color_caida(pct: f64, max: f64) -> egui::Color32 {
    if pct <= 0.6 * max {
        egui::Color32::from_rgb(46, 160, 67)
    } else if pct <= max {
        egui::Color32::from_rgb(214, 162, 0)
    } else {
        egui::Color32::from_rgb(210, 40, 40)
    }
}

fn barra(app: &mut App, ui: &mut egui::Ui) {
    let st = &mut app.red;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Abrir red.json…").clicked() {
            if let Some(r) = rfd::FileDialog::new().add_filter("Red de GridCraft", &["json"]).pick_file() {
                let res = st.abrir(&r);
                if let Err(e) = res {
                    app.error = Some(e);
                } else {
                    app.estado = format!("Red abierta: {}", r.display());
                }
                return;
            }
        }
        if ui.button("Nueva red…").on_hover_text("Crea una carpeta con plantillas (no sobrescribe) y la abre").clicked() {
            if let Some(c) = elegir_carpeta("Carpeta de la red", None) {
                match crear(&c).and_then(|r| st.abrir(&r).map(|_| r)) {
                    Ok(r) => app.estado = format!("Red creada en {} — dibuje los postes en el plano", r.display()),
                    Err(e) => app.error = Some(e),
                }
                return;
            }
        }
        let hay = st.red.is_some();
        if ui.add_enabled(hay && st.ruta.is_some(), egui::Button::new("Guardar")).clicked() {
            match st.guardar() {
                Ok(r) => app.estado = format!("Red guardada en {}", r.display()),
                Err(e) => app.error = Some(e),
            }
        }
        if ui.add_enabled(hay, egui::Button::new(egui::RichText::new("Calcular").strong())).clicked() {
            match st.calcular() {
                Ok(m) => app.estado = m,
                Err(e) => app.error = Some(e),
            }
        }
        if ui.add_enabled(st.resultado.is_some(), egui::Button::new("Exportar…")).clicked() {
            if let Some(c) = elegir_carpeta("Carpeta de salida (GridCraft)", st.ruta.as_deref().and_then(Path::parent)) {
                match st.exportar(&c) {
                    Ok(out) => {
                        app.estado = format!("Exportados {} archivo(s) en {}", out.len(), c.display());
                        abrir_externo(&c);
                    }
                    Err(e) => app.error = Some(e),
                }
            }
        }
        if ui.add_enabled(!st.pila.is_empty(), egui::Button::new("Deshacer")).clicked() {
            st.deshacer();
        }
        ui.separator();
        if ui.button("Fondo DXF…").on_hover_text("Plano de lotes o calles como referencia").clicked() {
            if let Some(r) = rfd::FileDialog::new().add_filter("DXF", &["dxf"]).pick_file() {
                match suite_motor::entrada::lineas_dxf(&r) {
                    Ok(l) => {
                        app.estado = format!("Fondo: {} líneas de {}", l.len(), r.display());
                        st.fondo = l;
                    }
                    Err(e) => app.error = Some(e),
                }
            }
        }
        ui.checkbox(&mut st.ver_topo, "Topografía");
    });
    ui.horizontal(|ui| {
        ui.label("Herramienta:");
        ui.selectable_value(&mut st.modo, Modo::Seleccionar, "Seleccionar / mover");
        ui.selectable_value(&mut st.modo, Modo::Agregar, "Agregar poste");
        ui.selectable_value(&mut st.modo, Modo::Eliminar, "Eliminar poste");
        ui.separator();
        ui.checkbox(&mut st.mover_con_raton, "Arrastrar mueve postes").on_hover_text("Desactívelo para desplazar la vista. Rueda = zoom.");
        if let Some(rd) = &st.red {
            ui.label("Conductor de los vanos nuevos:");
            egui::ComboBox::from_id_salt("cond_nuevo").selected_text(st.conductor_nuevo.as_str()).show_ui(ui, |ui| {
                for c in &rd.conductores {
                    ui.selectable_value(&mut st.conductor_nuevo, c.formacion.clone(), &c.formacion);
                }
            });
        }
    });
    if !st.resumen.is_empty() {
        ui.label(egui::RichText::new(if st.obsoleto { format!("{} (red modificada: pulse Calcular)", st.resumen) } else { st.resumen.clone() }).strong());
    }
}

fn plano(app: &mut App, ui: &mut egui::Ui, alto: f32) {
    let topo: Option<(Vec<Vec<[f64; 2]>>, Vec<[f64; 2]>)> = if app.red.ver_topo {
        app.doc.topografia().ok().map(|t| {
            (
                t.curvas.iter().map(|c| c.xy.clone()).collect(),
                t.puntos.iter().map(|p| [p.este, p.norte]).collect(),
            )
        })
    } else {
        None
    };
    let st = &mut app.red;
    let Some(rd) = &mut st.red else {
        ui.centered_and_justified(|ui| ui.label("Abra una red (red.json) o cree una nueva y dibuje los postes en el plano."));
        return;
    };
    let max = if rd.caida_max_pct > 0.0 { rd.caida_max_pct } else { 5.0 };
    let pct: std::collections::HashMap<String, f64> = st
        .resultado
        .as_ref()
        .filter(|_| !st.obsoleto)
        .map(|r| r.tramos.iter().map(|t| (t.nodo.clone(), t.dv_pct)).collect())
        .unwrap_or_default();
    let pos = |n: &Nodo| Some([n.este?, n.norte?]);
    let mut accion: Option<Box<dyn FnOnce(&mut EstadoRed)>> = None;
    let (modo, sel, mover) = (st.modo, st.sel, st.mover_con_raton);
    let arrastre = st.arrastre;
    let nodos = rd.nodos.clone();
    let plot = Plot::new("plano_red").legend(Legend::default()).data_aspect(1.0).height(alto).x_axis_label("Este (m)").y_axis_label("Norte (m)")
        .allow_drag(!(mover && modo == Modo::Seleccionar)).allow_double_click_reset(false);
    plot.show(ui, |pu| {
        for (k, l) in st.fondo.iter().enumerate() {
            pu.line(Line::new(if k == 0 { "Fondo DXF" } else { "" }, PlotPoints::from(l.clone())).color(egui::Color32::from_gray(140)).width(0.6));
        }
        if let Some((curvas, puntos)) = &topo {
            for (k, c) in curvas.iter().enumerate() {
                pu.line(Line::new(if k == 0 { "Curvas de nivel" } else { "" }, PlotPoints::from(c.clone())).color(egui::Color32::from_rgb(150, 110, 60)).width(0.5));
            }
            pu.points(Points::new("", puntos.clone()).radius(1.0).color(egui::Color32::from_gray(120)));
        }
        // tramos
        for n in &nodos {
            let (Some(a), Some(p)) = (pos(n), nodos.iter().find(|x| x.nodo == n.padre).and_then(pos)) else { continue };
            let color = pct.get(&n.nodo).map_or(egui::Color32::from_rgb(70, 130, 220), |&x| color_caida(x, max));
            pu.line(Line::new("", PlotPoints::from(vec![p, a])).color(color).width(3.0));
            pu.text(Text::new("", PlotPoint::new((a[0] + p[0]) / 2.0, (a[1] + p[1]) / 2.0),
                egui::RichText::new(format!("{:.0} m", n.longitud_m)).small()).color(egui::Color32::from_gray(170)));
        }
        // postes
        for (i, n) in nodos.iter().enumerate() {
            let Some(a) = pos(n) else { continue };
            let raiz = n.padre.is_empty();
            let c = if sel == Some(i) { egui::Color32::from_rgb(230, 126, 34) } else if raiz { egui::Color32::from_rgb(200, 60, 200) } else { egui::Color32::WHITE };
            pu.points(Points::new("", vec![a]).radius(if raiz { 7.0 } else { 4.5 }).color(c).filled(true));
            let etiqueta = match pct.get(&n.nodo) {
                Some(x) => format!("{} ({x:.1} %)", n.nodo),
                None => n.nodo.clone(),
            };
            pu.text(Text::new("", PlotPoint::new(a[0], a[1]), egui::RichText::new(etiqueta).small()).anchor(egui::Align2::LEFT_BOTTOM).color(c));
        }
        // interacción
        let resp = pu.response().clone();
        let puntero = pu.pointer_coordinate();
        let cerca = |x: Option<PlotPoint>| -> Option<usize> {
            let x = x?;
            let px = pu.transform().position_from_point(&x);
            nodos
                .iter()
                .enumerate()
                .filter_map(|(i, n)| Some((i, pu.transform().position_from_point(&PlotPoint::new(n.este?, n.norte?)).distance(px))))
                .filter(|(_, d)| *d < 12.0)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(i, _)| i)
        };
        match modo {
            Modo::Seleccionar => {
                if resp.clicked() {
                    let i = cerca(puntero);
                    accion = Some(Box::new(move |st: &mut EstadoRed| st.sel = i));
                }
                if mover && resp.drag_started() {
                    if let Some(i) = cerca(puntero) {
                        accion = Some(Box::new(move |st: &mut EstadoRed| {
                            st.apilar();
                            st.arrastre = Some(i);
                            st.sel = Some(i);
                        }));
                    }
                }
                if let (Some(i), Some(x), true) = (arrastre, puntero, resp.dragged()) {
                    accion = Some(Box::new(move |st: &mut EstadoRed| {
                        if let Some(rd) = &mut st.red {
                            rd.nodos[i].este = Some(x.x);
                            rd.nodos[i].norte = Some(x.y);
                            ajustar_longitudes(rd, i);
                        }
                    }));
                }
                if resp.drag_stopped() {
                    accion = Some(Box::new(|st: &mut EstadoRed| st.arrastre = None));
                }
            }
            Modo::Agregar => {
                if resp.clicked() {
                    if let Some(x) = puntero {
                        accion = Some(Box::new(move |st: &mut EstadoRed| {
                            st.apilar();
                            let cond = st.conductor_nuevo.clone();
                            let Some(rd) = &mut st.red else { return };
                            let raiz_existe = rd.nodos.iter().any(|n| n.padre.is_empty());
                            let padre = match st.sel.filter(|&i| i < rd.nodos.len()) {
                                Some(i) => rd.nodos[i].nodo.clone(),
                                None if raiz_existe => rd.nodos.iter().find(|n| n.padre.is_empty()).unwrap().nodo.clone(),
                                None => String::new(),
                            };
                            let nombre = if padre.is_empty() { "SED".to_string() } else { nombre_libre(rd) };
                            rd.nodos.push(Nodo {
                                nodo: nombre,
                                conductor: if padre.is_empty() { String::new() } else { cond },
                                padre,
                                este: Some(x.x),
                                norte: Some(x.y),
                                ..Default::default()
                            });
                            let i = rd.nodos.len() - 1;
                            ajustar_longitudes(rd, i);
                            st.sel = Some(i);
                        }));
                    }
                }
            }
            Modo::Eliminar => {
                if resp.clicked() {
                    if let Some(i) = cerca(puntero) {
                        accion = Some(Box::new(move |st: &mut EstadoRed| {
                            let Some(rd) = &st.red else { return };
                            let nombre = rd.nodos[i].nodo.clone();
                            if rd.nodos.iter().any(|n| n.padre == nombre) {
                                st.resumen = format!("{nombre} tiene derivaciones: elimine primero los postes que cuelgan de él");
                                return;
                            }
                            st.apilar();
                            if let Some(rd) = &mut st.red {
                                rd.nodos.remove(i);
                            }
                            st.sel = None;
                        }));
                    }
                }
            }
        }
    });
    if let Some(f) = accion {
        f(st);
    }
}

fn propiedades(app: &mut App, ui: &mut egui::Ui) {
    let st = &mut app.red;
    let Some(rd) = &mut st.red else { return };
    let mut cambio = false;
    egui::ScrollArea::vertical().id_salt("prop_red").show(ui, |ui| {
        if let Some(i) = st.sel.filter(|&i| i < rd.nodos.len()) {
            ui.heading("Poste / nodo");
            let otros: Vec<String> = rd.nodos.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, n)| n.nodo.clone()).collect();
            let formaciones: Vec<String> = rd.conductores.iter().map(|c| c.formacion.clone()).collect();
            let viejo = rd.nodos[i].nodo.clone();
            let mut renombrar = None;
            {
                let n = &mut rd.nodos[i];
                egui::Grid::new("nodo").num_columns(2).show(ui, |ui| {
                    ui.label("Nombre");
                    let mut nombre = n.nodo.clone();
                    if ui.text_edit_singleline(&mut nombre).lost_focus() && nombre.trim() != viejo && !nombre.trim().is_empty() {
                        renombrar = Some(nombre.trim().to_string());
                    }
                    ui.end_row();
                    if !n.padre.is_empty() {
                        ui.label("Padre");
                        egui::ComboBox::from_id_salt("padre").selected_text(n.padre.as_str()).show_ui(ui, |ui| {
                            for o in &otros {
                                cambio |= ui.selectable_value(&mut n.padre, o.clone(), o).changed();
                            }
                        });
                        ui.end_row();
                        ui.label("Conductor");
                        egui::ComboBox::from_id_salt("cond").selected_text(n.conductor.as_str()).show_ui(ui, |ui| {
                            for f in &formaciones {
                                cambio |= ui.selectable_value(&mut n.conductor, f.clone(), f).changed();
                            }
                        });
                        ui.end_row();
                        cambio |= num(ui, "Longitud del vano", &mut n.longitud_m, 0.5, " m");
                    }
                    cambio |= num(ui, "Lotes (SP)", &mut n.lotes, 1.0, "");
                    cambio |= num(ui, "Carga especial", &mut n.kw_ce, 0.1, " kW");
                    cambio |= num(ui, "Lámparas (AP)", &mut n.lamparas, 1.0, "");
                    let mut e = n.este.unwrap_or(0.0);
                    let mut no = n.norte.unwrap_or(0.0);
                    if num(ui, "Este", &mut e, 0.5, " m") {
                        n.este = Some(e);
                        cambio = true;
                    }
                    if num(ui, "Norte", &mut no, 0.5, " m") {
                        n.norte = Some(no);
                        cambio = true;
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new("Estructura (metrado)").strong());
                let e = &mut n.estructura;
                egui::Grid::new("estr").num_columns(2).show(ui, |ui| {
                    for (t, v) in [("Armado", &mut e.armado), ("Soporte", &mut e.soporte), ("Puesta a tierra", &mut e.pat), ("Caja derivación", &mut e.caja),
                        ("Pastoral", &mut e.pastoral), ("Retenida", &mut e.retenida)]
                    {
                        ui.label(t);
                        cambio |= ui.add(egui::TextEdit::singleline(v).desired_width(120.0)).changed();
                        ui.end_row();
                    }
                    cambio |= num(ui, "N.° de retenidas", &mut e.n_retenidas, 1.0, "");
                    ui.label("Acometidas").on_hover_text("C corta, L larga, X extra larga; 3 trifásica; M con murete. Ej.: C;C;L3");
                    let mut a = e.acometidas.join(";");
                    if ui.add(egui::TextEdit::singleline(&mut a).desired_width(120.0)).changed() {
                        e.acometidas = a.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                        cambio = true;
                    }
                    ui.end_row();
                });
            }
            if let Some(nuevo) = renombrar {
                if rd.nodos.iter().any(|n| n.nodo == nuevo) {
                    st.resumen = format!("Ya existe un nodo {nuevo}");
                } else {
                    for n in &mut rd.nodos {
                        if n.padre == viejo {
                            n.padre = nuevo.clone();
                        }
                    }
                    rd.nodos[i].nodo = nuevo;
                    cambio = true;
                }
            }
            if !rd.nodos[i].padre.is_empty() && ui.button("Longitud según el plano").clicked() {
                if let Some(l) = longitud_plano(rd, i) {
                    rd.nodos[i].longitud_m = (l * 100.0).round() / 100.0;
                    cambio = true;
                }
            }
            ui.separator();
        } else {
            ui.label("Seleccione un poste en el plano para ver y editar sus datos.");
            ui.separator();
        }
        egui::CollapsingHeader::new("Parámetros de la red").default_open(st.sel.is_none()).show(ui, |ui| {
            egui::Grid::new("par_red").num_columns(2).show(ui, |ui| {
                ui.label("Nombre");
                cambio |= ui.text_edit_singleline(&mut rd.nombre).changed();
                ui.end_row();
                ui.label("Sistema");
                egui::ComboBox::from_id_salt("sist").selected_text(rd.sistema.as_str()).show_ui(ui, |ui| {
                    for s in ["3F", "1F"] {
                        cambio |= ui.selectable_value(&mut rd.sistema, s.to_string(), s).changed();
                    }
                });
                ui.end_row();
                cambio |= num(ui, "Tensión (fase-fase)", &mut rd.tension_v, 1.0, " V");
                let mut vfn = rd.tension_fn_v.unwrap_or(0.0);
                if num(ui, "Tensión fase-neutro (0 = auto)", &mut vfn, 1.0, " V") {
                    rd.tension_fn_v = (vfn > 0.0).then_some(vfn);
                    cambio = true;
                }
                cambio |= num(ui, "Calificación por lote", &mut rd.calificacion_kw_lote, 0.01, " kW");
                cambio |= num(ui, "F. simultaneidad SP", &mut rd.fs_sp, 0.01, "");
                cambio |= num(ui, "Factor de potencia SP", &mut rd.fp_sp, 0.01, "");
                cambio |= num(ui, "F. simultaneidad CE", &mut rd.fs_ce, 0.01, "");
                cambio |= num(ui, "Potencia por lámpara", &mut rd.kw_lampara, 0.005, " kW");
                cambio |= num(ui, "Factor de potencia AP", &mut rd.fp_ap, 0.01, "");
                cambio |= num(ui, "Caída máxima", &mut rd.caida_max_pct, 0.1, " %");
                cambio |= num(ui, "Pérdidas máximas", &mut rd.perdidas_max_pct, 0.1, " %");
                cambio |= num(ui, "Factor de carga", &mut rd.factor_carga, 0.01, "");
                cambio |= num(ui, "Transformador", &mut rd.transformador_kva, 1.0, " kVA");
            });
        });
        egui::CollapsingHeader::new("Conductores").show(ui, |ui| {
            egui::Grid::new("conds").striped(true).num_columns(5).show(ui, |ui| {
                for t in ["Formación", "K (Ω/km)", "R (Ω/km)", "Fases", "I máx. (A)"] {
                    ui.label(egui::RichText::new(t).small().strong());
                }
                ui.end_row();
                for c in rd.conductores.iter_mut() {
                    ui.label(&c.formacion);
                    for v in [&mut c.k_ohm_km, &mut c.r_ohm_km, &mut c.fases, &mut c.corriente_max_a] {
                        let mut x = v.unwrap_or(0.0);
                        if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                            *v = Some(x);
                            cambio = true;
                        }
                    }
                    ui.end_row();
                }
            });
            if ui.button("Agregar formación").clicked() {
                rd.conductores.push(ConductorRed { formacion: format!("F{}", rd.conductores.len() + 1), ..Default::default() });
                cambio = true;
            }
        });
    });
    if cambio {
        st.obsoleto = true;
        st.modificado = true;
    }
}

pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::top("red_barra").show(ui, |ui| barra(app, ui));
    egui::Panel::right("red_prop").resizable(true).default_size(330.0).show(ui, |ui| propiedades(app, ui));
    egui::CentralPanel::default().show(ui, |ui| {
        let hay_tablas = !app.red.tablas.is_empty();
        let alto = if hay_tablas { (ui.available_height() * 0.62).max(260.0) } else { ui.available_height() };
        plano(app, ui, alto);
        // Supr: elimina el poste seleccionado (si no tiene derivaciones)
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) && !ui.ctx().egui_wants_keyboard_input() {
            let st = &mut app.red;
            if let (Some(i), Some(rd)) = (st.sel, &st.red) {
                let nombre = rd.nodos[i].nodo.clone();
                if rd.nodos.iter().any(|n| n.padre == nombre) {
                    st.resumen = format!("{nombre} tiene derivaciones: elimine primero los postes que cuelgan de él");
                } else {
                    st.apilar();
                    if let Some(rd) = &mut st.red {
                        rd.nodos.remove(i);
                    }
                    st.sel = None;
                }
            }
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            app.red.deshacer();
        }
        let st = &mut app.red;
        if hay_tablas {
            ui.horizontal_wrapped(|ui| {
                for (k, t) in st.tablas.iter().enumerate() {
                    ui.selectable_value(&mut st.hoja, k, &t.nombre);
                }
            });
            ui.separator();
            let t = st.tablas[st.hoja].clone();
            ver_tabla(ui, &t, &format!("red_{}", t.nombre));
        }
    });
}
