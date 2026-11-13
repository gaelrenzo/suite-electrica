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
