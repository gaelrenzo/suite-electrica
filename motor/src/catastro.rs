//! Catastro: predios afectados por la faja de servidumbre de la línea.
//!
//! Los predios son polígonos (DXF: polilíneas cerradas; el nombre sale del TEXT que cae dentro).
//! Por predio: tramo del eje que lo cruza (progresivas y longitud) y área de faja dentro del predio.
//! El ancho de la faja lo fija el usuario con su fuente (CNE Suministro), la suite no trae uno.

/// Predio del catastro.
#[derive(Debug, Clone, PartialEq)]
pub struct Predio {
    pub nombre: String,
    pub xy: Vec<[f64; 2]>,
}

/// Afectación de un predio por la faja de servidumbre.
#[derive(Debug, Clone, PartialEq)]
pub struct FilaPredio {
    pub nombre: String,
    /// Progresivas del eje dentro del predio (NaN si solo lo toca la faja, no el eje).
    pub progresiva_ini: f64,
    pub progresiva_fin: f64,
    pub longitud_eje_m: f64,
    pub area_faja_m2: f64,
    pub area_predio_m2: f64,
}

/// Punto dentro del polígono (regla par-impar).
pub fn dentro(p: [f64; 2], poli: &[[f64; 2]]) -> bool {
    let mut d = false;
    let n = poli.len();
    for i in 0..n {
        let (a, b) = (poli[i], poli[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            d = !d;
        }
    }
    d
}

/// Área del polígono (fórmula del área de Gauss).
pub fn area(poli: &[[f64; 2]]) -> f64 {
    let n = poli.len();
    (0..n).map(|i| poli[i][0] * poli[(i + 1) % n][1] - poli[(i + 1) % n][0] * poli[i][1]).sum::<f64>().abs() / 2.0
}

/// Predios que toca la faja de `ancho_m` centrada en el eje. Muestreo en celdas de `paso` m a lo
/// largo × ancho/`n_ancho` a lo ancho.
/// ponytail: en los vértices del eje las celdas de los dos tramos se superponen o dejan una cuña
/// (error ≈ ancho²·tan(deflexión/2)/2 por vértice); para un área exacta haría falta recortar
/// polígonos (faja como polígono ∩ predio).
pub fn afectados(eje: &[[f64; 2]], predios: &[Predio], ancho_m: f64, paso: f64) -> Vec<FilaPredio> {
    let n_ancho = ((ancho_m / paso).ceil() as usize).max(1);
    let w = ancho_m / n_ancho as f64;
    let cajas: Vec<[f64; 4]> = predios
        .iter()
        .map(|p| p.xy.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |c, q| {
            [c[0].min(q[0]), c[1].min(q[1]), c[2].max(q[0]), c[3].max(q[1])]
        }))
        .collect();
    let en = |k: usize, q: [f64; 2]| {
        let c = cajas[k];
        q[0] >= c[0] && q[0] <= c[2] && q[1] >= c[1] && q[1] <= c[3] && dentro(q, &predios[k].xy)
    };
    let mut acc: Vec<(f64, f64, f64, f64)> = vec![(f64::NAN, f64::NAN, 0.0, 0.0); predios.len()];
    let mut s0 = 0.0;
    for t in eje.windows(2) {
        let (dx, dy) = (t[1][0] - t[0][0], t[1][1] - t[0][1]);
        let l = dx.hypot(dy);
        if l == 0.0 {
            continue;
        }
        let (ux, uy) = (dx / l, dy / l);
        let pasos = (l / paso).ceil() as usize;
        let h = l / pasos as f64;
        for j in 0..pasos {
            let d = (j as f64 + 0.5) * h;
            let c = [t[0][0] + ux * d, t[0][1] + uy * d];
            for (k, a) in acc.iter_mut().enumerate() {
                if en(k, c) {
                    let s = s0 + d;
                    a.0 = if a.0.is_nan() { s - h / 2.0 } else { a.0.min(s - h / 2.0) };
                    a.1 = if a.1.is_nan() { s + h / 2.0 } else { a.1.max(s + h / 2.0) };
                    a.2 += h;
                }
                for m in 0..n_ancho {
                    let o = -ancho_m / 2.0 + (m as f64 + 0.5) * w;
                    if en(k, [c[0] - uy * o, c[1] + ux * o]) {
                        a.3 += h * w;
                    }
                }
            }
        }
        s0 += l;
    }
    predios
        .iter()
        .zip(acc)
        .filter(|(_, a)| a.3 > 0.0)
        .map(|(p, a)| FilaPredio {
            nombre: p.nombre.clone(),
            progresiva_ini: a.0,
            progresiva_fin: a.1,
            longitud_eje_m: a.2,
            area_faja_m2: a.3,
            area_predio_m2: area(&p.xy),
        })
        .collect()
}

/// Hoja «Predios» (para la app y el Excel).
pub fn tabla_predios(f: &[FilaPredio], ancho_m: f64) -> crate::salidas::Tabla {
    use crate::salidas::Celda::{Num, Txt};
    let r2 = |x: f64| Num((x * 100.0).round() / 100.0);
    let prog = |x: f64| Txt(if x.is_nan() { "—".into() } else { crate::salidas::formato_progresiva(x, 2) });
    crate::salidas::Tabla {
        nombre: "Predios".into(),
        columnas: ["Predio", "Desde", "Hasta", "Eje dentro (m)", &format!("Faja de {ancho_m} m (m²)"), "Área del predio (m²)", "% del predio"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        filas: f
            .iter()
            .map(|p| vec![
                Txt(p.nombre.clone()), prog(p.progresiva_ini), prog(p.progresiva_fin), r2(p.longitud_eje_m), r2(p.area_faja_m2),
                r2(p.area_predio_m2), r2(if p.area_predio_m2 > 0.0 { 100.0 * p.area_faja_m2 / p.area_predio_m2 } else { f64::NAN }),
            ])
            .collect(),
    }
}
