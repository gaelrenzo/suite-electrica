//! Tabla de tendido (flechado). Port de `linecraft/salidas.tabla_flechado`.

use crate::conductor::{carga_unitaria, Hipotesis, G};
use crate::linea::{Proyecto, Resultado};
use crate::mecanica::{cambio_estado, Curva};

/// Fila por vano: tiro horizontal (kgf) y flecha a medio vano (m) por temperatura.
#[derive(Clone, Debug)]
pub struct FilaFlechado {
    pub tramo: String,
    pub vano: String,
    pub vano_regulador: f64,
    pub longitud: f64,
    pub desnivel: f64,
    pub eds_pct: f64,
    /// (temperatura °C, H kgf, flecha a medio vano m), sin redondear.
    pub valores: Vec<(f64, f64, f64)>,
}

/// Condición de templado (EDS, sin viento ni creep), `t_templado ± amplitud` cada `paso` °C.
pub fn tabla_flechado(r: &Resultado, p: &Proyecto, amplitud: f64, paso: f64) -> Result<Vec<FilaFlechado>, String> {
    let c = &p.conductor;
    let est = &r.estructuras;
    let mut filas = Vec::new();
    for t in &r.tramos {
        let z = est[t.inicio..=t.fin].iter().map(|e| e.cota_terreno).sum::<f64>() / (t.fin - t.inicio + 1) as f64;
        let h0 = p.hip(&p.hip_inicial, Some(z))?;
        let w = carga_unitaria(c, &Hipotesis::nueva("templado", h0.temperatura, 0.0))?.vertical;
        let s1 = p.tiro_inicial_pct / 100.0 * c.carga_rotura / c.seccion;
        let unico = t.fin - t.inicio == 1;
        // np.arange(t0 − amplitud, t0 + amplitud + 1e-9, paso)
        let (ini, fin) = (h0.temperatura - amplitud, h0.temperatura + amplitud + 1e-9);
        let n = ((fin - ini) / paso).ceil() as usize;
        let temps: Vec<f64> = (0..n).map(|k| ini + k as f64 * paso).collect();
        for i in t.inicio..t.fin {
            let v = &r.vanos[i];
            let mut valores = Vec::with_capacity(temps.len());
            for &tt in &temps {
                let h = cambio_estado(s1, w, h0.temperatura, w, tt, t.vano_regulador, c, if unico { v.desnivel } else { 0.0 })?
                    * c.seccion;
                let cv = Curva::nueva(p.modelo, v.longitud, v.desnivel, h, w);
                valores.push((tt, h / G, cv.flecha_medio()));
            }
            filas.push(FilaFlechado {
                tramo: format!("{}-{}", est[t.inicio].codigo(), est[t.fin].codigo()),
                vano: format!("{}-{}", est[i].codigo(), est[i + 1].codigo()),
                vano_regulador: t.vano_regulador,
                longitud: v.longitud,
                desnivel: v.desnivel,
                eds_pct: p.tiro_inicial_pct,
                valores,
            });
        }
    }
    Ok(filas)
}
