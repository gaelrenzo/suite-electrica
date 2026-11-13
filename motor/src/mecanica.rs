//! Mecánica del conductor: cambio de estado, creep, catenaria, parábola y vano regulador.
//!
//! Port de `suite_electrica/linecraft/mecanica.py`. Convención: vano horizontal `a` (m)
//! entre A (x = 0) y B (x = a), desnivel `h = z_B − z_A`, tiro horizontal `H` (N), carga
//! unitaria `w` (N/m); `y(x)` medido respecto de A.

use crate::conductor::Conductor;
use crate::numerico::brent;

/// Pa por kg/mm² (unidades de las curvas esfuerzo-deformación de catálogo).
pub const KG_MM2: f64 = 9.80665e6;

/// Esfuerzo horizontal final `σ2` (Pa) por la ecuación de cambio de estado.
///
/// `σ2² [σ2 − σ1 + a² w1² E / (24 A² σ1²) + α E (t2 − t1)] = a² w2² E / (24 A²)`.
/// Con `desnivel ≠ 0` (tramo de un vano inclinado) usa la forma exacta con longitud de
/// catenaria y elongación elástica `H/EA` (validada contra REDLIN).
pub fn cambio_estado(
    sigma1: f64, w1: f64, t1: f64, w2: f64, t2: f64, vano: f64, c: &Conductor, desnivel: f64,
) -> Result<f64, String> {
    if sigma1 <= 0.0 || vano <= 0.0 {
        return Err("σ1 y el vano deben ser positivos".into());
    }
    if desnivel != 0.0 {
        return cambio_estado_inclinado(sigma1, w1, t1, w2, t2, vano, desnivel, c);
    }
    let (a, area, e) = (vano, c.seccion, c.modulo_elasticidad);
    let k = a * a * w1 * w1 * e / (24.0 * area * area * sigma1 * sigma1) - sigma1
        + c.coef_dilatacion * e * (t2 - t1);
    let m = a * a * w2 * w2 * e / (24.0 * area * area);
    // f(σ) = σ²(σ + k) − m: negativa en [0, max(0, −k)], creciente después → raíz única.
    let f = |s: f64| s * s * (s + k) - m;
    let lo = (-k).max(0.0);
    let mut hi = lo.max(1.0) * 2.0;
    while f(hi) <= 0.0 {
        hi *= 2.0;
        if !hi.is_finite() {
            return Err("Sin solución positiva en la ecuación de cambio de estado".into());
        }
    }
    brent(f, lo, hi, 1e-12 * hi).ok_or_else(|| "Sin solución en el cambio de estado".into())
}

fn cambio_estado_inclinado(
    sigma1: f64, w1: f64, t1: f64, w2: f64, t2: f64, a: f64, h: f64, c: &Conductor,
) -> Result<f64, String> {
    let area = c.seccion;
    let ea = c.modulo_elasticidad * area;
    let l0 = |hh: f64, w: f64| {
        let cc = hh / w;
        h.hypot(2.0 * cc * (a / (2.0 * cc)).sinh()) / (1.0 + hh / ea)
    };
    let h1 = sigma1 * area;
    let objetivo = l0(h1, w1) * (1.0 + c.coef_dilatacion * (t2 - t1));
    // cota inferior C = a/40 (evita overflow de sinh), como en Python.
    brent(|hh| l0(hh, w2) - objetivo, w2 * a / 40.0, 100.0 * h1 + 1e5, 1e-6)
        .map(|hh| hh / area)
        .ok_or_else(|| "Sin solución en el cambio de estado (vano inclinado)".into())
}

/// ε (%) de la curva `σ(ε)` para `σ` (kg/mm²), en su rama creciente.
pub fn deformacion(poli: &[f64], sigma_kg: f64) -> Result<f64, String> {
    let f = |x: f64| poli.iter().enumerate().map(|(k, a)| a * x.powi(k as i32)).sum::<f64>() - sigma_kg;
    let (mut x0, mut x1) = (-0.01, 0.01);
    while f(x1) < 0.0 {
        if x1 > 5.0 {
            return Err(format!("σ = {sigma_kg:.2} kg/mm² fuera de la curva esfuerzo-deformación"));
        }
        x0 = x1;
        x1 += 0.01;
    }
    brent(f, x0, x1, 2e-12).ok_or_else(|| "Curva esfuerzo-deformación sin raíz".into())
}

/// Alargamiento permanente por fluencia lenta (adimensional), método gráfico de Alcoa.
///
/// `Δε = [ε_c(σf) − σf/E] − [ε_i(σ1) − σ1/E]` con `σf` iterado por cambio de estado.
/// Sin curva de creep devuelve 0.
pub fn deformacion_creep(
    c: &Conductor, sigma1: f64, w1: f64, t1: f64, vano: f64, desnivel: f64,
) -> Result<f64, String> {
    if c.curva_creep.is_empty() {
        return Ok(0.0);
    }
    let e = c.modulo_elasticidad / KG_MM2;
    let s1 = sigma1 / KG_MM2;
    let base = if c.curva_inicial.is_empty() {
        0.0
    } else {
        deformacion(&c.curva_inicial, s1)? / 100.0 - s1 / e
    };
    let (mut sf, mut de) = (s1, 0.0);
    for _ in 0..100 {
        de = (deformacion(&c.curva_creep, sf)? / 100.0 - sf / e - base).max(0.0);
        let nuevo = cambio_estado(sigma1, w1, t1, w1, t1 + de / c.coef_dilatacion, vano, c, desnivel)? / KG_MM2;
        if (nuevo - sf).abs() < 1e-9 {
            break;
        }
        sf = nuevo;
    }
    Ok(de)
}

/// Vano regulador `sqrt(Σa³ / Σa)` (m).
pub fn vano_regulador(vanos: &[f64]) -> Result<f64, String> {
    if vanos.is_empty() || vanos.iter().any(|&a| a <= 0.0) {
        return Err("Vanos deben ser positivos".into());
    }
    let (s3, s1): (f64, f64) = vanos.iter().fold((0.0, 0.0), |(s3, s1), a| (s3 + a * a * a, s1 + a));
    Ok((s3 / s1).sqrt())
}

/// Modelo de curva del conductor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modelo {
    Catenaria,
    Parabola,
}

/// Curva del conductor en un vano (catenaria o parábola).
#[derive(Clone, Copy, Debug)]
pub struct Curva {
    pub modelo: Modelo,
    pub a: f64,
    pub h: f64,
    pub tiro_h: f64,
    pub w: f64,
    c: f64,
    xm: f64,
}

impl Curva {
    pub fn nueva(modelo: Modelo, a: f64, h: f64, tiro_h: f64, w: f64) -> Self {
        let c = tiro_h / w;
        let xm = match modelo {
            Modelo::Catenaria => a / 2.0 - c * (h / (2.0 * c * (a / (2.0 * c)).sinh())).asinh(),
            Modelo::Parabola => a / 2.0 - tiro_h * h / (w * a),
        };
        Curva { modelo, a, h, tiro_h, w, c, xm }
    }

    /// Parámetro de la catenaria `C = H / w` (m).
    pub fn parametro(&self) -> f64 {
        self.c
    }

    /// Abscisa del punto más bajo respecto de A (puede caer fuera del vano).
    pub fn x_vertice(&self) -> f64 {
        self.xm
    }

    /// Altura del conductor respecto de A.
    pub fn y(&self, x: f64) -> f64 {
        match self.modelo {
            Modelo::Catenaria => self.c * (((x - self.xm) / self.c).cosh() - (self.xm / self.c).cosh()),
            Modelo::Parabola => self.h * x / self.a - self.w * x * (self.a - x) / (2.0 * self.tiro_h),
        }
    }

    /// Flecha máxima (paralela a la cuerda) (m).
    pub fn flecha(&self) -> f64 {
        match self.modelo {
            Modelo::Catenaria => {
                let x = (self.xm + self.c * (self.h / self.a).asinh()).clamp(0.0, self.a);
                self.h * x / self.a - self.y(x)
            }
            Modelo::Parabola => self.w * self.a * self.a / (8.0 * self.tiro_h),
        }
    }

    /// Flecha a medio vano (criterio de REDLIN) (m).
    pub fn flecha_medio(&self) -> f64 {
        self.h / 2.0 - self.y(self.a / 2.0)
    }

    /// Tiro en el soporte más exigido (N).
    pub fn tiro_maximo(&self) -> f64 {
        match self.modelo {
            Modelo::Catenaria => self.tiro_h * (self.xm.abs().max((self.a - self.xm).abs()) / self.c).cosh(),
            Modelo::Parabola => {
                let s = self.h / self.a;
                let d = self.w * self.a / (2.0 * self.tiro_h);
                let p = (s - d).abs().max((s + d).abs());
                self.tiro_h * (1.0 + p * p).sqrt()
            }
        }
    }

    /// Longitud del conductor (catenaria) (m).
    pub fn longitud(&self) -> f64 {
        self.h.hypot(2.0 * self.c * (self.a / (2.0 * self.c)).sinh())
    }
}
