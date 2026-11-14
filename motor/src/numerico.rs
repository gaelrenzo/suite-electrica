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

/// Formato `{x:g}` de Python (6 cifras significativas, sin ceros finales).
pub fn formato_g(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let e = format!("{x:.5e}"); // mantisa redondeada a 6 cifras
    let (m, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let quitar = |t: String| if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t };
    if (-4..6).contains(&exp) {
        quitar(format!("{x:.*}", (5 - exp) as usize))
    } else {
        format!("{}e{}{:02}", quitar(m.to_string()), if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}

#[cfg(test)]
mod pruebas {
    use super::formato_g;

    #[test]
    fn como_python() {
        for (x, e) in [(18.0, "18"), (18.5, "18.5"), (-5.0, "-5"), (0.123456789, "0.123457"), (1234567.0, "1.23457e+06"),
            (0.0001, "0.0001"), (0.00001, "1e-05"), (3402.5, "3402.5"), (100000.0, "100000"), (999999.5, "1e+06")]
        {
            assert_eq!(formato_g(x), e, "{x}");
        }
    }
}
