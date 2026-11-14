//! Raíces de funciones escalares (sin dependencias externas).

/// Redondeo a `d` decimales como `round()` de Python: sobre el valor binario exacto
/// (formato decimal correctamente redondeado), no `(x·10^d).round()`.
pub fn redondear(x: f64, d: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.d$}").parse().unwrap_or(x)
}

/// Raíz de `f` en `[a, b]` con `f(a)` y `f(b)` de signo opuesto (método de Brent).
///
/// Devuelve `None` si el intervalo no encierra un cambio de signo.
pub fn brent<F: Fn(f64) -> f64>(f: F, mut a: f64, mut b: f64, tol: f64) -> Option<f64> {
    let (mut fa, mut fb) = (f(a), f(b));
    if fa == 0.0 {
        return Some(a);
    }
    if fb == 0.0 {
        return Some(b);
    }
    if fa.signum() == fb.signum() {
        return None;
    }
    let (mut c, mut fc) = (a, fa);
    let (mut d, mut e) = (b - a, b - a);
    for _ in 0..200 {
        if fb.signum() == fc.signum() {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * tol;
        let xm = 0.5 * (c - b);
        if xm.abs() <= tol1 || fb == 0.0 {
            return Some(b);
        }
        if e.abs() >= tol1 && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (mut p, mut q);
            if a == c {
                p = 2.0 * xm * s;
                q = 1.0 - s;
            } else {
                let qq = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * xm * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            if 2.0 * p < (3.0 * xm * q - (tol1 * q).abs()).min((e * q).abs()) {
                e = d;
                d = p / q;
            } else {
                d = xm;
                e = d;
            }
        } else {
            d = xm;
            e = d;
        }
        a = b;
        fa = fb;
        b += if d.abs() > tol1 { d } else { tol1.copysign(xm) };
        fb = f(b);
    }
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raiz_de_coseno() {
        let r = brent(|x| x.cos() - x, 0.0, 1.0, 1e-14).unwrap();
        assert!((r - 0.739_085_133_215_160_6).abs() < 1e-12);
        assert!(brent(|x| x * x + 1.0, -1.0, 1.0, 1e-9).is_none());
    }
}
