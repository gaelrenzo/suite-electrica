//! Pestaña LineCraft: parámetros, perfil interactivo, planilla editable y resultados.
//!
//! En el perfil: clic = seleccionar estructura; arrastrar = moverla (no las de inicio, fin ni
//! vértices); doble clic = agregar una estructura; menú contextual (clic derecho) = agregar,
//! eliminar o cambiar tipo. Cada edición recalcula la línea.

use std::collections::BTreeMap;

use eframe::egui;
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoint, PlotPoints, Text};
use suite_motor::conductor::{Conductor, G};
use suite_motor::documento::FilaHipotesis;
use suite_motor::proyectos::leer_catalogo_conductores;
use suite_motor::salidas::{curvas_vanos, escribir_excel, formato_progresiva, tablas_linea, Tabla};
use suite_motor::{dxf, pdf};

use crate::{abrir_externo, elegir_carpeta, num, ver_tabla, App};

const CONDUCTORES: &str = include_str!("../../../suite_electrica/plantillas/conductores.csv");
const TIPOS: [&str; 5] = ["", "FIN", "ANCLAJE", "ANGULO", "SUSPENSION"];

pub struct EstadoLinea {
    pub seleccion: Option<usize>,
    arrastre: Option<(usize, f64)>,
    menu_en: Option<f64>,
    hoja: String,
    mover_con_raton: bool,
    catalogo: BTreeMap<String, Conductor>,
    tablas: Vec<Tabla>,
    cat_abierto: bool,
    cat_hoja: String,
    cat_filtro: String,
}

impl Default for EstadoLinea {
    fn default() -> Self {
        EstadoLinea {
            seleccion: None,
            arrastre: None,
            menu_en: None,
            hoja: "Estructuras".into(),
            mover_con_raton: true,
            catalogo: leer_catalogo_conductores(CONDUCTORES).unwrap_or_default(),
            tablas: Vec::new(),
            cat_abierto: false,
            cat_hoja: "armados".into(),
            cat_filtro: String::new(),
        }
    }
}

/// Calcula (F6); sin estructuras, primero las ubica automáticamente.
pub fn calcular(app: &mut App) {
    let r = app.doc.calcular();
    poner(app, r, "Cálculo");
}

fn poner(app: &mut App, r: Result<suite_motor::documento::Calculo, String>, que: &str) {
    if let Some(c) = app.intentar(r) {
        let res = &c.resultado;
        let dmin = res.vanos.iter().map(|v| v.despeje).fold(f64::INFINITY, f64::min);
        app.estado = format!(
            "{que}: {} estructuras, {} vanos — despeje mínimo {dmin:.2} m — {} — {} alerta(s), {} aviso(s)",
            res.estructuras.len(),
            res.vanos.len(),
            if c.cumple() { "CUMPLE" } else { "NO CUMPLE" },
            c.alertas.len(),
            res.avisos.len()
        );
        app.linea.tablas = tablas_linea(res, &c.proyecto, &c.extras());
        app.calculo = Some(c);
        app.obsoleto = false;
        if app.linea.seleccion.is_some_and(|i| i >= app.doc.estructuras.len()) {
            app.linea.seleccion = None;
        }
    }
}

pub fn deshacer(app: &mut App) {
    if app.doc.deshacer() {
        calcular(app);
        app.estado = format!("Deshecho. {}", app.estado);
    }
}

/// Aplica una edición del documento y recalcula.
fn editar(app: &mut App, r: Result<(), String>) {
    if app.intentar(r).is_some() {
        calcular(app);
    }
}

fn exportar(app: &mut App) {
    if app.calculo.is_none() || app.obsoleto {
        calcular(app);
    }
    let Some(c) = &app.calculo else { return };
    let Some(dir) = elegir_carpeta("Carpeta de salida (LineCraft)", app.doc.ruta.as_deref().and_then(std::path::Path::parent)) else { return };
    let (r, p) = (&c.resultado, &c.proyecto);
    let res = (|| {
        Ok::<_, String>(vec![
            dxf::perfil_linea(&dir.join("perfil_linea.dxf"), r, p, 10.0)?,
            escribir_excel(&dir.join("planilla_estructuras.xlsx"), &tablas_linea(r, p, &c.extras()))?,
            pdf::memoria_pdf(&dir.join("memoria_calculo.pdf"), r, p, &c.extras())?,
        ])
    })();
    if let Some(out) = app.intentar(res) {
        app.estado = format!("Exportados {} archivos en {}", out.len(), dir.display());
        abrir_externo(&dir);
    }
}

fn datos(app: &mut App, ui: &mut egui::Ui) {
    let mut cambio = false;
    let hips: Vec<String> = app.doc.hipotesis.iter().map(|h| h.nombre.clone()).filter(|n| !n.trim().is_empty()).collect();
    egui::CollapsingHeader::new("Proyecto").default_open(true).show(ui, |ui| {
        egui::Grid::new("proy").num_columns(2).show(ui, |ui| {
            ui.label("Nombre");
            cambio |= ui.text_edit_singleline(&mut app.doc.nombre).changed();
            ui.end_row();
            ui.label("Autor");
            cambio |= ui.text_edit_singleline(&mut app.doc.autor).changed();
            ui.end_row();
            ui.label("Zona UTM");
            cambio |= ui.text_edit_singleline(&mut app.doc.zona_utm).changed();
            ui.end_row();
        });
    });
    egui::CollapsingHeader::new("Parámetros de diseño").default_open(true).show(ui, |ui| {
        let l = &mut app.doc.linea;
        egui::Grid::new("lin").num_columns(2).show(ui, |ui| {
            let combo = |ui: &mut egui::Ui, id: &str, v: &mut String, opciones: &[String]| {
                let mut c = false;
                egui::ComboBox::from_id_salt(id).selected_text(v.as_str()).show_ui(ui, |ui| {
                    for o in opciones {
                        c |= ui.selectable_value(v, o.clone(), o).changed();
                    }
                });
                c
            };
            ui.label("Sistema (armados DGE)");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("fases").selected_text(match l.fases { 1 => "Monofásico", 2 => "Bifásico", _ => "Trifásico" }).show_ui(ui, |ui| {
                    for (f, t) in [(1, "Monofásico"), (2, "Bifásico"), (3, "Trifásico")] {
                        if ui.selectable_value(&mut l.fases, f, t).changed() {
                            l.n_conductores = f;
                            cambio = true;
                        }
                    }
                });
                cambio |= ui.checkbox(&mut l.neutro, "neutro corrido").changed();
            });
            ui.end_row();
            ui.label("Conductores por circuito");
            cambio |= ui.add(egui::DragValue::new(&mut l.n_conductores).range(1..=12)).changed();
            ui.end_row();
            ui.label("Modelo de curva");
            cambio |= combo(ui, "modelo", &mut l.modelo, &["catenaria".into(), "parabola".into()]);
            ui.end_row();
            ui.label("Hipótesis inicial (EDS)");
            cambio |= combo(ui, "hip_ini", &mut l.hip_inicial, &hips);
            ui.end_row();
            cambio |= num(ui, "Tiro inicial", &mut l.tiro_inicial_pct, 0.1, " % rotura");
            ui.label("Hipótesis de flecha máxima");
            cambio |= combo(ui, "hip_fl", &mut l.hip_flecha_maxima, &hips);
            ui.end_row();
            cambio |= num(ui, "Tiro máximo", &mut l.tiro_maximo_pct, 0.1, " % rotura");
            cambio |= num(ui, "Distancia mínima al terreno", &mut l.distancia_minima_terreno, 0.05, " m");
            cambio |= num(ui, "Altura de amarre", &mut l.altura_amarre, 0.05, " m");
            cambio |= num(ui, "Vano máximo", &mut l.vano_maximo, 1.0, " m");
            cambio |= num(ui, "Vano mínimo", &mut l.vano_minimo, 1.0, " m");
            cambio |= num(ui, "Ángulo de retenida", &mut l.angulo_retenida, 0.5, "°");
            ui.label("Condición final");
            cambio |= combo(ui, "cond_final", &mut l.condicion_final, &["creep".into(), "inicial".into()]);
            ui.end_row();
            ui.label("Estado final de los vanos");
            cambio |= combo(ui, "est_final", &mut l.estado_final_vanos, &["tramo".into(), "por vano".into()]);
            ui.end_row();
            cambio |= num(ui, "FS poste", &mut l.fs_poste, 0.05, "");
            cambio |= num(ui, "FS retenida", &mut l.fs_retenida, 0.05, "");
            ui.label("Retenida (código)");
            cambio |= ui.text_edit_singleline(&mut l.retenida).changed();
            ui.end_row();
            cambio |= num(ui, "Desperdicio de conductor", &mut l.desperdicio_pct, 0.1, " %");
        });
        ui.label(egui::RichText::new("Alertas (0 = no verificar; citar la fuente en «Fuentes»)").small());
        egui::Grid::new("alertas").num_columns(2).show(ui, |ui| {
            cambio |= num(ui, "Tensión", &mut l.tension_kv, 0.1, " kV");
            cambio |= num(ui, "Separación: k·U", &mut l.sep_k_tension, 0.001, " m/kV");
            cambio |= num(ui, "Separación: k·√f", &mut l.sep_k_flecha, 0.01, "");
            cambio |= num(ui, "Ángulo de salida mín.", &mut l.angulo_salida_min, 0.5, "°");
            cambio |= num(ui, "Ángulo de salida máx.", &mut l.angulo_salida_max, 0.5, "°");
            cambio |= num(ui, "Carga máx. por espiga", &mut l.tiro_max_espiga, 10.0, " N");
            cambio |= num(ui, "EDS máx. (vibraciones)", &mut l.eds_max_pct, 0.1, " %");
        });
    });
    if ui.button("Cargar catálogos de la plantilla").on_hover_text(
        "Reemplaza postes, armados, retenidas y materiales del proyecto por los de la plantilla (armados DGE 1Φ, 2Φ, 3Φ con y sin neutro). Se guarda al guardar el proyecto.",
    ).clicked() {
        app.doc.catalogos = suite_motor::documento::Documento::nuevo().catalogos;
        cambio = true;
    }
    if ui.button("Editar catálogos…").on_hover_text("Postes, armados, retenidas y materiales del proyecto").clicked() {
        app.linea.cat_abierto = true;
    }
    cambio |= ventana_catalogos(app, &ui.ctx().clone());
    egui::CollapsingHeader::new("Conductor").show(ui, |ui| {
        let c = &mut app.doc.conductor;
        egui::ComboBox::from_id_salt("cond").selected_text(c.nombre.as_str()).show_ui(ui, |ui| {
            for (n, cc) in &app.linea.catalogo {
                if ui.selectable_label(&c.nombre == n, n).clicked() {
                    *c = cc.clone();
                    cambio = true;
                }
            }
        });
        let mut v = [c.seccion * 1e6, c.diametro * 1e3, c.peso / G, c.carga_rotura / 1e3, c.modulo_elasticidad / 1e6, c.coef_dilatacion * 1e6];
        let mut ch = false;
        egui::Grid::new("cond_g").num_columns(2).show(ui, |ui| {
            for (k, (t, suf, vel)) in [("Sección", " mm²", 0.1), ("Diámetro", " mm", 0.01), ("Masa", " kg/m", 0.001),
                ("Tiro de rotura", " kN", 0.01), ("Módulo de elasticidad", " MPa", 10.0), ("Coef. dilatación", " ×10⁻⁶/°C", 0.1)]
                .iter()
                .enumerate()
            {
                ch |= num(ui, t, &mut v[k], *vel, suf);
            }
            ui.label("Fuente");
            ch |= ui.text_edit_singleline(&mut c.fuente).changed();
            ui.end_row();
        });
        if ch {
            c.seccion = v[0] * 1e-6;
            c.diametro = v[1] * 1e-3;
            c.peso = v[2] * G;
            c.carga_rotura = v[3] * 1e3;
            c.modulo_elasticidad = v[4] * 1e6;
            c.coef_dilatacion = v[5] * 1e-6;
            cambio = true;
        }
    });
    egui::CollapsingHeader::new("Hipótesis de cálculo").show(ui, |ui| {
        let mut quitar = None;
        egui::Grid::new("hips").striped(true).num_columns(8).show(ui, |ui| {
            for t in ["Nombre", "T (°C)", "Viento (Pa)", "Hielo (mm)", "ρ hielo", "Alt. mín.", "Alt. máx.", ""] {
                ui.label(egui::RichText::new(t).small().strong());
            }
            ui.end_row();
            for (k, h) in app.doc.hipotesis.iter_mut().enumerate() {
                cambio |= ui.add(egui::TextEdit::singleline(&mut h.nombre).desired_width(90.0)).changed();
                for v in [&mut h.temperatura_c, &mut h.presion_viento_pa, &mut h.espesor_hielo_mm, &mut h.densidad_hielo_kg_m3] {
                    cambio |= ui.add(egui::DragValue::new(v).speed(0.5)).changed();
                }
                for v in [&mut h.altitud_min_m, &mut h.altitud_max_m] {
                    let mut t = v.map(|x| x.to_string()).unwrap_or_default();
                    if ui.add(egui::TextEdit::singleline(&mut t).desired_width(45.0).hint_text("—")).changed() {
                        *v = t.replace(',', ".").trim().parse().ok();
                        cambio = true;
                    }
                }
                if ui.small_button("✖").on_hover_text("Quitar hipótesis").clicked() {
                    quitar = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some(k) = quitar {
            app.doc.hipotesis.remove(k);
            cambio = true;
        }
        if ui.button("Agregar hipótesis").clicked() {
            app.doc.hipotesis.push(FilaHipotesis {
                nombre: format!("H{}", app.doc.hipotesis.len() + 1),
                temperatura_c: 15.0,
                presion_viento_pa: 0.0,
                espesor_hielo_mm: 0.0,
                densidad_hielo_kg_m3: 0.0,
                altitud_min_m: None,
                altitud_max_m: None,
            });
            cambio = true;
        }
    });
    egui::CollapsingHeader::new("Fuentes de los parámetros normativos").show(ui, |ui| {
        egui::Grid::new("fuentes").num_columns(2).striped(true).show(ui, |ui| {
            for (k, v) in app.doc.fuentes.iter_mut() {
                let pendiente = v.trim().is_empty() || v.to_uppercase().contains("PENDIENTE") || v.to_uppercase().contains("EJEMPLO");
                ui.label(if pendiente { egui::RichText::new(k.as_str()).color(egui::Color32::from_rgb(200, 120, 0)) } else { egui::RichText::new(k.as_str()) });
                cambio |= ui.add(egui::TextEdit::singleline(v).desired_width(260.0)).changed();
                ui.end_row();
            }
        });
    });
    if cambio {
        app.doc.modificado = true;
        app.obsoleto = true;
    }
}

/// Perfil con terreno, conductor, despeje y estructuras; devuelve la acción del usuario.
enum Accion {
    Ninguna,
    Mover(usize, f64),
    Agregar(f64),
}

fn perfil(app: &mut App, ui: &mut egui::Ui, alto: f32) -> Accion {
    let Some(c) = &app.calculo else {
        ui.centered_and_justified(|ui| {
            ui.label("Importe puntos y eje en TopoCraft y pulse Calcular (F6): las estructuras se ubican automáticamente.")
        });
        return Accion::Ninguna;
    };
    let (r, p) = (&c.resultado, &c.proyecto);
    let st = &mut app.linea;
    let terreno: Vec<[f64; 2]> = p.perfil.progresiva.iter().zip(&p.perfil.cota).filter(|(_, z)| !z.is_nan()).map(|(s, z)| [*s, *z]).collect();
    let zmin = terreno.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min);
    let cond = curvas_vanos(r, p, 0.0);
    let desp = curvas_vanos(r, p, p.distancia_minima_terreno);
    let mut accion = Accion::Ninguna;
    let tinta = ui.visuals().strong_text_color();
    let plot = Plot::new("perfil_linea")
        .legend(Legend::default())
        .height(alto)
        .x_axis_label("Progresiva (m)")
        .y_axis_label("Cota (m)")
        .allow_drag(!st.mover_con_raton)
        .allow_double_click_reset(false);
    let resp = plot.show(ui, |pu| {
        pu.line(Line::new("Terreno", PlotPoints::from(terreno)).color(egui::Color32::from_rgb(90, 62, 27)).fill((zmin - 5.0) as f32).fill_alpha(0.35));
        for (k, ((s, z), (_, zd))) in cond.iter().zip(&desp).enumerate() {
            let pts = |zz: &Vec<f64>| PlotPoints::from(s.iter().zip(zz).map(|(a, b)| [*a, *b]).collect::<Vec<_>>());
            pu.line(Line::new(if k == 0 { format!("Conductor ({})", p.hip_flecha_maxima) } else { String::new() }, pts(z))
                .color(egui::Color32::from_rgb(192, 57, 43)).width(1.5));
            pu.line(Line::new(if k == 0 { format!("Despeje mínimo {:.2} m", p.distancia_minima_terreno) } else { String::new() }, pts(zd))
                .color(egui::Color32::from_rgb(142, 68, 173)).style(LineStyle::dashed_dense()));
        }
        for (i, e) in r.estructuras.iter().enumerate() {
            let sel = st.seleccion == Some(i);
            let s = match st.arrastre {
                Some((j, s)) if j == i => s,
                _ => e.progresiva,
            };
            let color = if sel { egui::Color32::from_rgb(230, 126, 34) } else { tinta };
            pu.line(Line::new("", PlotPoints::from(vec![[s, e.cota_terreno], [s, e.cota_amarre()]])).color(color).width(if sel { 4.0 } else { 2.5 }));
            pu.text(Text::new("", PlotPoint::new(s, e.cota_amarre()), egui::RichText::new(e.codigo()).small()).anchor(egui::Align2::CENTER_BOTTOM).color(color));
        }
        // interacción
        let resp = pu.response().clone();
        let cerca = |x: Option<PlotPoint>| -> Option<usize> {
            let x = x?;
            let px = pu.transform().position_from_point(&x).x;
            r.estructuras
                .iter()
                .enumerate()
                .map(|(i, e)| (i, (pu.transform().position_from_point(&PlotPoint::new(e.progresiva, x.y)).x - px).abs()))
                .filter(|(_, d)| *d < 10.0)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(i, _)| i)
        };
        let puntero = pu.pointer_coordinate();
        if resp.clicked() {
            st.seleccion = cerca(puntero);
        }
        if resp.double_clicked() {
            if let Some(x) = puntero {
                accion = Accion::Agregar(x.x);
            }
        }
        if resp.secondary_clicked() {
            st.menu_en = puntero.map(|x| x.x);
            if let Some(i) = cerca(puntero) {
                st.seleccion = Some(i);
            }
        }
        if st.mover_con_raton {
            if resp.drag_started() {
                st.arrastre = cerca(puntero).map(|i| (i, r.estructuras[i].progresiva));
                st.seleccion = st.arrastre.map(|a| a.0);
            }
            if let (true, Some((i, _)), Some(x)) = (resp.dragged(), st.arrastre, puntero) {
                st.arrastre = Some((i, x.x.clamp(0.0, p.perfil.longitud())));
            }
            if resp.drag_stopped() {
                if let Some((i, s)) = st.arrastre.take() {
                    accion = Accion::Mover(i, s);
                }
            }
        }
    });
    let sel = app.linea.seleccion;
    let menu_en = app.linea.menu_en;
    let mut post: Option<Box<dyn FnOnce(&mut App)>> = None;
    resp.response.context_menu(|ui| {
        if let Some(s) = menu_en {
            if ui.button(format!("Agregar estructura en {}", formato_progresiva(s, 2))).clicked() {
                post = Some(Box::new(move |app: &mut App| {
                    let r = app.doc.agregar_estructura(s).map(|i| app.linea.seleccion = Some(i));
                    editar(app, r);
                }));
                ui.close();
            }
        }
        if let Some(i) = sel {
            ui.separator();
            ui.label(format!("Estructura {}", i + 1));
            if ui.button("Eliminar").clicked() {
                post = Some(Box::new(move |app: &mut App| {
                    let r = app.doc.eliminar_estructura(i);
                    app.linea.seleccion = None;
                    editar(app, r);
                }));
                ui.close();
            }
            ui.menu_button("Cambiar tipo", |ui| {
                for t in TIPOS {
                    if ui.button(if t.is_empty() { "Automático (según geometría)" } else { t }).clicked() {
                        post = Some(Box::new(move |app: &mut App| {
                            let r = app.doc.cambiar_estructura(i, "tipo", t);
                            editar(app, r);
                        }));
                        ui.close();
                    }
                }
            });
        }
    });
    if let Some(f) = post {
        f(app);
    }
    accion
}

fn planilla(app: &mut App, ui: &mut egui::Ui) {
    let cat = app.doc.catalogos_obj().unwrap_or_default();
    let armados: Vec<String> = std::iter::once(String::new()).chain(cat.armados.iter().map(|a| a.codigo.clone())).collect();
    let postes: Vec<String> = std::iter::once(String::new()).chain(cat.postes.iter().map(|p| p.codigo.clone())).collect();
    let res = app.calculo.as_ref().map(|c| c.resultado.estructuras.clone()).unwrap_or_default();
    let mut cambio: Option<(usize, &'static str, String)> = None;
    let mut seleccion = app.linea.seleccion;
    egui::ScrollArea::both().id_salt("planilla").auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("planilla_g").striped(true).num_columns(11).show(ui, |ui| {
            for t in ["Código", "Progresiva", "Tipo (vacío = auto)", "Tipo calculado", "Deflexión (°)", "Armado", "Poste", "Altura amarre (m)", "Retenidas", "Cota terreno", ""] {
                ui.label(egui::RichText::new(t).strong());
            }
            ui.end_row();
            for (i, e) in app.doc.estructuras.iter().enumerate() {
                let bloq = app.doc.bloqueada(i);
                let rr = res.get(i);
                if ui.selectable_label(seleccion == Some(i), rr.map(|x| x.codigo()).unwrap_or_else(|| format!("E-{:02}", i + 1))).clicked() {
                    seleccion = Some(i);
                }
                ui.label(formato_progresiva(e.progresiva_m, 2)).on_hover_text(if bloq { "Inicio, fin o vértice: fija" } else { "Arrastre en el perfil para mover" });
                for (campo, valor, opciones) in [("tipo", &e.tipo, TIPOS.iter().map(|s| s.to_string()).collect::<Vec<_>>()), ("armado", &e.armado, armados.clone()), ("poste", &e.poste, postes.clone())] {
                    if campo == "armado" {
                        ui.label(rr.map(|x| x.tipo.como_str()).unwrap_or(""));
                        ui.label(rr.map(|x| format!("{:.2}", x.angulo)).unwrap_or_default());
                    }
                    egui::ComboBox::from_id_salt(format!("{campo}{i}")).selected_text(if valor.is_empty() { "—" } else { valor.as_str() }).width(110.0).show_ui(ui, |ui| {
                        for o in &opciones {
                            if ui.selectable_label(o == valor, if o.is_empty() { "—" } else { o.as_str() }).clicked() && o != valor {
                                cambio = Some((i, campo, o.clone()));
                            }
                        }
                    });
                }
                for (campo, v) in [("altura_amarre_m", e.altura_amarre_m), ("retenidas", e.retenidas)] {
                    let mut t = v.map(|x| x.to_string()).unwrap_or_default();
                    let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(60.0).hint_text("auto"));
                    if r.lost_focus() && t != v.map(|x| x.to_string()).unwrap_or_default() {
                        cambio = Some((i, campo, t));
                    }
                }
                ui.label(rr.map(|x| format!("{:.2}", x.cota_terreno)).unwrap_or_default());
                ui.end_row();
            }
        });
    });
    app.linea.seleccion = seleccion;
    if let Some((i, campo, v)) = cambio {
        let r = app.doc.cambiar_estructura(i, campo, &v);
        editar(app, r);
    }
}

pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::left("linea_datos").resizable(true).default_size(360.0).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new("Calcular (F6)").strong())).clicked() {
                    calcular(app);
                }
                if ui.button("Ubicar automático").on_hover_text("Reubica todas las estructuras y asigna catálogos").clicked() {
                    let r = app.doc.ubicar_automatico();
                    poner(app, r, "Ubicación automática");
                }
                if ui.button("Estructuras mínimas").on_hover_text("Solo inicio, fin y vértices del eje").clicked() {
                    let r = app.doc.estructuras_minimas();
                    editar(app, r);
                }
                if ui.button("Asignar catálogos").on_hover_text("Armado y poste a las estructuras sin asignar").clicked() {
                    let r = app.doc.asignar_catalogos(true);
                    poner(app, r, "Catálogos asignados");
                }
                if ui.add_enabled(app.doc.puede_deshacer(), egui::Button::new("Deshacer (Ctrl+Z)")).clicked() {
                    deshacer(app);
                }
                if ui.button("Exportar…").on_hover_text("Perfil DXF, planilla Excel y memoria PDF").clicked() {
                    exportar(app);
                }
            });
            if app.obsoleto {
                ui.colored_label(egui::Color32::from_rgb(200, 120, 0), "Datos modificados: pulse Calcular (F6)");
            }
            ui.separator();
            datos(app, ui);
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.linea.mover_con_raton, "Mover estructuras con el ratón")
                .on_hover_text("Desactívelo para desplazar la vista arrastrando. Rueda = zoom.");
            ui.label(egui::RichText::new("Clic: seleccionar · Doble clic: agregar · Clic derecho: menú · Supr: eliminar").small());
        });
        let alto = (ui.available_height() * 0.5).max(220.0);
        match perfil(app, ui, alto) {
            Accion::Mover(i, s) => {
                let r = app.doc.mover_estructura(i, s).map(|_| ());
                editar(app, r);
            }
            Accion::Agregar(s) => {
                let r = app.doc.agregar_estructura(s).map(|i| app.linea.seleccion = Some(i));
                editar(app, r);
            }
            Accion::Ninguna => {}
        }
        if let Some(i) = app.linea.seleccion {
            if ui.input(|x| x.key_pressed(egui::Key::Delete)) && !ui.ctx().egui_wants_keyboard_input() {
                let r = app.doc.eliminar_estructura(i);
                app.linea.seleccion = None;
                editar(app, r);
            }
        }
        ui.separator();
        let mut hojas = vec!["Estructuras".to_string()];
        hojas.extend(app.linea.tablas.iter().skip(1).map(|t| t.nombre.clone()));
        ui.horizontal_wrapped(|ui| {
            for h in &hojas {
                ui.selectable_value(&mut app.linea.hoja, h.clone(), h.as_str());
            }
        });
        ui.separator();
        if app.linea.hoja == "Estructuras" {
            planilla(app, ui);
        } else if let Some(t) = app.linea.tablas.iter().find(|t| t.nombre == app.linea.hoja) {
            let t = t.clone();
            ver_tabla(ui, &t, &format!("lc_{}", t.nombre));
        }
    });
}

/// Ventana de edición de los catálogos del proyecto (se guardan en el `.sel`).
fn ventana_catalogos(app: &mut App, ctx: &egui::Context) -> bool {
    if !app.linea.cat_abierto {
        return false;
    }
    let mut abierto = true;
    let mut cambio = false;
    let (doc, st) = (&mut app.doc, &mut app.linea);
    egui::Window::new("Catálogos del proyecto").open(&mut abierto).default_size([980.0, 560.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for n in doc.catalogos.keys() {
                if ui.selectable_label(&st.cat_hoja == n, n).clicked() {
                    st.cat_hoja = n.clone();
                }
            }
        });
        ui.label(egui::RichText::new("Se guardan con el proyecto (.sel). Tras editar, use «Asignar catálogos» y Calcular.").small());
        if let Some(t) = doc.catalogos.get_mut(&st.cat_hoja) {
            cambio |= crate::tablas::editar(ui, &format!("cat_{}", st.cat_hoja), &t.columnas, &mut t.filas, &mut st.cat_filtro);
        }
    });
    st.cat_abierto = abierto;
    cambio
}
