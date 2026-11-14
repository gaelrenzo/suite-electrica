//! Perfil longitudinal y geometría del eje en planta (port de `core/geo/perfil.py`).

/// Vértice del eje.
pub const VERTICE: char = 'V';
/// Estación a paso regular.
pub const ESTACION: char = 'E';
/// Cruce con arista del TIN.
pub const QUIEBRE: char = 'T';

/// Perfil longitudinal (m). `cota` puede ser NaN fuera del modelo de terreno.
#[derive(Clone, Debug, Default)]
pub struct Perfil {
    pub progresiva: Vec<f64>,
    pub este: Vec<f64>,
    pub norte: Vec<f64>,
    pub cota: Vec<f64>,
    pub tipo: Vec<char>,
}

impl Perfil {
    /// Longitud total del eje (m).
    pub fn longitud(&self) -> f64 {
        *self.progresiva.last().unwrap_or(&0.0)
    }

    /// Cota interpolada en la progresiva `s` (como `np.interp`: extremos constantes).
    pub fn cota_en(&self, s: f64) -> f64 {
        interp(s, &self.progresiva, &self.cota)
    }
}

/// Interpolación lineal con la semántica de `numpy.interp` (fuera de rango: valor extremo).
pub fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    let n = xp.len();
    if n == 0 {
        return f64::NAN;
    }
    if x <= xp[0] {
        return fp[0];
    }
    if x >= xp[n - 1] {
        return fp[n - 1];
    }
    // primer índice con xp[j] > x
    let j = xp.partition_point(|&v| v <= x);
    let (x0, x1, y0, y1) = (xp[j - 1], xp[j], fp[j - 1], fp[j]);
    if x1 == x0 {
        return y1;
    }
    let t = (x - x0) / (x1 - x0);
    y0 + t * (y1 - y0)
}

/// Progresiva acumulada (m) de cada vértice del eje.
pub fn progresivas_eje(eje: &[[f64; 2]]) -> Vec<f64> {
    let mut acum = vec![0.0];
    for w in eje.windows(2) {
        let d = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        acum.push(acum.last().unwrap() + d);
    }
    acum
}

/// Coordenadas en planta de la progresiva `s` sobre el eje.
pub fn punto_en_progresiva(eje: &[[f64; 2]], s: f64) -> [f64; 2] {
    let acum = progresivas_eje(eje);
    let xs: Vec<f64> = eje.iter().map(|p| p[0]).collect();
    let ys: Vec<f64> = eje.iter().map(|p| p[1]).collect();
    [interp(s, &acum, &xs), interp(s, &acum, &ys)]
}

/// Deflexión (grados, + izquierda) en cada vértice; 0 en los extremos.
pub fn angulos_eje(eje: &[[f64; 2]]) -> Vec<f64> {
    let rumbos: Vec<f64> = eje.windows(2).map(|w| (w[1][1] - w[0][1]).atan2(w[1][0] - w[0][0])).collect();
    let mut out = vec![0.0];
    for r in rumbos.windows(2) {
        let pi = std::f64::consts::PI;
        let d = (r[1] - r[0] + pi).rem_euclid(2.0 * pi) - pi;
        out.push(d.to_degrees());
    }
    out.push(0.0);
    if eje.len() < 2 {
        out.truncate(eje.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interp_como_numpy() {
        let xp = [0.0, 10.0, 20.0];
        let fp = [1.0, 3.0, 2.0];
        assert_eq!(interp(-5.0, &xp, &fp), 1.0);
        assert_eq!(interp(25.0, &xp, &fp), 2.0);
        assert!((interp(5.0, &xp, &fp) - 2.0).abs() < 1e-12);
        assert!((interp(15.0, &xp, &fp) - 2.5).abs() < 1e-12);
        assert_eq!(interp(10.0, &xp, &fp), 3.0);
    }

    #[test]
    fn angulos_y_progresivas() {
        let eje = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]];
        assert_eq!(progresivas_eje(&eje), vec![0.0, 100.0, 200.0]);
        let a = angulos_eje(&eje);
        assert!((a[1] - 90.0).abs() < 1e-9 && a[0] == 0.0 && a[2] == 0.0);
        let p = punto_en_progresiva(&eje, 150.0);
        assert!((p[0] - 100.0).abs() < 1e-12 && (p[1] - 50.0).abs() < 1e-12);
    }
}

/// Progresivas donde el eje cruza aristas del TIN.
fn cruces_aristas(eje: &[[f64; 2]], tin: &crate::tin::Tin) -> Vec<f64> {
    let acum = progresivas_eje(eje);
    let seg = tin.segmentos_aristas();
    let mut res = Vec::new();
    for i in 0..eje.len().saturating_sub(1) {
        let (p, q) = (eje[i], eje[i + 1]);
        let r = [q[0] - p[0], q[1] - p[1]];
        let largo = r[0].hypot(r[1]);
        if largo == 0.0 {
            continue;
        }
        let (lo, hi) = ([p[0].min(q[0]), p[1].min(q[1])], [p[0].max(q[0]), p[1].max(q[1])]);
        for (a0, a1) in &seg {
            if a0[0].max(a1[0]) < lo[0] || a0[1].max(a1[1]) < lo[1] || a0[0].min(a1[0]) > hi[0] || a0[1].min(a1[1]) > hi[1] {
                continue;
            }
            let d = [a1[0] - a0[0], a1[1] - a0[1]];
            let den = r[0] * d[1] - r[1] * d[0];
            if den.abs() <= 1e-12 {
                continue;
            }
            let w = [a0[0] - p[0], a0[1] - p[1]];
            let t = (w[0] * d[1] - w[1] * d[0]) / den;
            let u = (w[0] * r[1] - w[1] * r[0]) / den;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                res.push(acum[i] + t * largo);
            }
        }
    }
    res
}

/// Perfil del terreno a lo largo del eje: vértices, estaciones cada `paso` y quiebres del TIN.
pub fn perfil_longitudinal(tin: &crate::tin::Tin, eje: &[[f64; 2]], paso: f64) -> Result<Perfil, String> {
    if eje.len() < 2 {
        return Err("El eje necesita al menos 2 vértices".into());
    }
    if paso <= 0.0 {
        return Err("El paso debe ser positivo".into());
    }
    let acum = progresivas_eje(eje);
    let longitud = *acum.last().unwrap();
    let prioridad = |t: char| match t {
        VERTICE => 0,
        ESTACION => 1,
        _ => 2,
    };
    let mut s: Vec<(f64, char)> = acum.iter().map(|&a| (a, VERTICE)).collect();
    let n_est = (longitud / paso).ceil() as usize; // np.arange(0, longitud, paso)
    s.extend((0..n_est).map(|k| (k as f64 * paso, ESTACION)).filter(|x| x.0 < longitud));
    s.extend(cruces_aristas(eje, tin).into_iter().map(|c| (c, QUIEBRE)));
    s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(prioridad(a.1).cmp(&prioridad(b.1))));
    let mut p = Perfil::default();
    let mut anterior = f64::NEG_INFINITY;
    for (k, (si, t)) in s.into_iter().enumerate() {
        // como `np.diff(s) > 1e-6`: compara con el inmediato anterior (aunque se haya descartado)
        let conservar = k == 0 || si - anterior > 1e-6;
        anterior = si;
        if !conservar {
            continue; // conserva el de mayor prioridad en coincidencias
        }
        let xy = punto_en_progresiva(eje, si);
        p.progresiva.push(si);
        p.este.push(xy[0]);
        p.norte.push(xy[1]);
        p.cota.push(tin.cota(xy[0], xy[1]));
        p.tipo.push(t);
    }
    Ok(p)
}

/// Progresiva del punto del eje más cercano a `p` y la distancia de `p` al eje (m).
pub fn proyectar_en_eje(eje: &[[f64; 2]], p: [f64; 2]) -> (f64, f64) {
    let acum = progresivas_eje(eje);
    let mut mejor = (0.0, f64::INFINITY);
    for (k, w) in eje.windows(2).enumerate() {
        let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((p[0] - w[0][0]) * dx + (p[1] - w[0][1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let d = (w[0][0] + t * dx - p[0]).hypot(w[0][1] + t * dy - p[1]);
        if d < mejor.1 {
            mejor = (acum[k] + t * l2.sqrt(), d);
        }
    }
    mejor
}

#[cfg(test)]
mod prueba_proyeccion {
    #[test]
    fn proyecta_sobre_el_tramo_mas_cercano() {
        let eje = [[0.0, 0.0], [100.0, 0.0], [100.0, 50.0]];
        assert_eq!(super::proyectar_en_eje(&eje, [40.0, 3.0]), (40.0, 3.0));
        assert_eq!(super::proyectar_en_eje(&eje, [104.0, 20.0]), (120.0, 4.0));
        assert_eq!(super::proyectar_en_eje(&eje, [-5.0, 0.0]).0, 0.0);
        let s = 130.0;
        let p = super::punto_en_progresiva(&eje, s);
        assert!((super::proyectar_en_eje(&eje, p).0 - s).abs() < 1e-9);
    }
}
