//! Editor de tablas de texto (CSV del presupuesto y catálogos): buscar, editar celdas, agregar,
//! duplicar y eliminar filas. Las filas se dibujan solo las visibles (catálogos de miles de filas).

use eframe::egui;

/// Edita `filas` (una cadena por columna). Devuelve `true` si cambió algo.
pub fn editar(ui: &mut egui::Ui, id: &str, columnas: &[String], filas: &mut Vec<Vec<String>>, filtro: &mut String) -> bool {
    let mut cambio = false;
    ui.horizontal(|ui| {
        ui.label("Buscar:");
        ui.add(egui::TextEdit::singleline(filtro).desired_width(200.0));
        if ui.button("Agregar fila").clicked() {
            filas.push(vec![String::new(); columnas.len()]);
            filtro.clear();
            cambio = true;
        }
        ui.label(egui::RichText::new(format!("{} filas", filas.len())).small());
    });
    let f = filtro.to_lowercase();
    let visibles: Vec<usize> =
        (0..filas.len()).filter(|&i| f.is_empty() || filas[i].iter().any(|c| c.to_lowercase().contains(&f))).collect();
    let anchos: Vec<f32> = (0..columnas.len())
        .map(|c| {
            let n = filas.iter().map(|r| r.get(c).map_or(0, |s| s.chars().count())).max().unwrap_or(0);
            (n.max(columnas[c].chars().count()) as f32 * 7.0 + 14.0).clamp(48.0, 380.0)
        })
        .collect();
    let alto = ui.spacing().interact_size.y;
    // (fila, true = duplicar / false = eliminar)
    let mut accion: Option<(usize, bool)> = None;
    egui::ScrollArea::horizontal().id_salt((id, "h")).auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(52.0);
            for (c, a) in columnas.iter().zip(&anchos) {
                ui.add_sized([*a, alto], egui::Label::new(egui::RichText::new(c).strong()).truncate());
            }
        });
        egui::ScrollArea::vertical().id_salt((id, "v")).auto_shrink([true, false]).show_rows(ui, alto, visibles.len(), |ui, rango| {
            for &i in &visibles[rango] {
                ui.horizontal(|ui| {
                    if ui.small_button("+").on_hover_text("Duplicar fila").clicked() {
                        accion = Some((i, true));
                    }
                    if ui.small_button("×").on_hover_text("Eliminar fila").clicked() {
                        accion = Some((i, false));
                    }
                    let fila = &mut filas[i];
                    fila.resize(columnas.len(), String::new());
                    for (c, a) in anchos.iter().enumerate() {
                        cambio |= ui.add_sized([*a, alto], egui::TextEdit::singleline(&mut fila[c])).changed();
                    }
                });
            }
        });
    });
    match accion {
        Some((i, true)) => {
            let copia = filas[i].clone();
            filas.insert(i + 1, copia);
            cambio = true;
        }
        Some((i, false)) => {
            filas.remove(i);
            cambio = true;
        }
        None => {}
    }
    cambio
}
