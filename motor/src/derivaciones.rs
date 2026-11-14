//! Derivaciones: cargas del ramal sumadas en la estructura de derivación de la principal.
//! Port de `linecraft/derivaciones.py` (suma de magnitudes, conservadora).

use crate::linea::{Proyecto, Resultado};

/// Distancia máxima (m) entre el inicio del eje del ramal y la estructura de derivación.
pub const TOL_DERIVACION: f64 = 2.0;

fn unitario(v: [f64; 2]) -> [f64; 2] {
    let n = v[0].hypot(v[1]);
    if n > 0.0 { [v[0] / n, v[1] / n] } else { v }
}

/// Dirección unitaria del eje longitudinal de la estructura `i` (bisectriz de los vanos).
pub fn eje_longitudinal(r: &Resultado, i: usize) -> [f64; 2] {
    let p = |k: usize| [r.estructuras[k].este, r.estructuras[k].norte];
    let mut s = [0.0, 0.0];
    if i > 0 {
        let d = unitario([p(i)[0] - p(i - 1)[0], p(i)[1] - p(i - 1)[1]]);
        s = [s[0] + d[0], s[1] + d[1]];
    }
    if i + 1 < r.estructuras.len() {
        let d = unitario([p(i + 1)[0] - p(i)[0], p(i + 1)[1] - p(i)[1]]);
        s = [s[0] + d[0], s[1] + d[1]];
    }
    unitario(s)
}

/// Fila de cargas agregadas por una derivación.
#[derive(Clone, Debug)]
pub struct FilaDerivacion {
    pub estructura: String,
    pub ramal: String,
    pub hipotesis: String,
    pub beta: f64,
    pub mas_longitudinal: f64,
    pub mas_transversal: f64,
    pub mas_vertical: f64,
    pub horizontal_total: f64,
}

/// Suma las cargas de la primera estructura del ramal a la estructura `desde` de la principal.
pub fn combinar(
    principal: &mut Resultado, pp: &Proyecto, ramal: &Resultado, nombre: &str, desde: &str,
) -> Result<Vec<FilaDerivacion>, String> {
    let i = principal
        .estructuras
        .iter()
        .position(|e| e.codigo() == desde)
        .ok_or_else(|| format!("Derivación {nombre}: la principal no tiene la estructura {desde}"))?;
    let e = principal.estructuras[i].clone();
    let r0 = &ramal.estructuras[0];
    let dist = (r0.este - e.este).hypot(r0.norte - e.norte);
    if dist > TOL_DERIVACION {
        return Err(format!("Derivación {nombre}: el eje empieza a {dist:.1} m de {desde} (máx. {TOL_DERIVACION} m)"));
    }
    let u_p = eje_longitudinal(principal, i);
    let r1 = &ramal.estructuras[1];
    let u_r = unitario([r1.este - r0.este, r1.norte - r0.norte]);
    let beta = (u_p[0] * u_r[0] + u_p[1] * u_r[1]).clamp(-1.0, 1.0).acos().to_degrees();
    let phi = pp.angulo_retenida.to_radians();
    let mut filas = Vec::new();
    for c in principal.cargas.iter_mut().filter(|c| c.estructura == e.numero) {
        let Some(d) = ramal.cargas.iter().find(|d| d.estructura == r0.numero && d.hipotesis == c.hipotesis) else {
            continue;
        };
        let dl = (d.longitudinal * beta.to_radians().cos()).abs();
        let dtr = (d.longitudinal * beta.to_radians().sin()).abs() + d.transversal;
        c.longitudinal += dl;
        c.transversal += dtr;
        c.vertical += d.vertical;
        let fh = c.transversal.hypot(c.longitudinal);
        c.horizontal = fh;
        c.momento_base = fh * e.altura_amarre;
        c.tiro_retenida = if c.tiro_retenida != 0.0 || d.tiro_retenida != 0.0 { fh / phi.cos() } else { 0.0 };
        filas.push(FilaDerivacion {
            estructura: desde.into(),
            ramal: nombre.into(),
            hipotesis: c.hipotesis.clone(),
            beta,
            mas_longitudinal: dl,
            mas_transversal: dtr,
            mas_vertical: d.vertical,
            horizontal_total: fh,
        });
    }
    Ok(filas)
}
