//! WGS84 geográficas ↔ UTM (hemisferio sur, zonas del Perú) sin PROJ.
//!
//! Proyección transversa de Mercator por las series de Krüger hasta n⁶ (Karney 2011, el
//! mismo método `etmerc` de PROJ): error < 1 µm dentro de la zona. Port de `core/geo/utm.py`.

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
const K0: f64 = 0.9996;
const FE: f64 = 500_000.0;
const FN_SUR: f64 = 10_000_000.0;

/// Meridiano central (°) de la zona UTM `"17S"`, `"18S"`, `"19S"` (o cualquier `NNS`/`NNN`).
fn zona(z: &str) -> Result<(f64, bool), String> {
    let z = z.trim().to_uppercase();
    let (num, hem) = z.split_at(z.len().saturating_sub(1));
    let n: i32 = num.parse().map_err(|_| format!("Zona UTM no válida: {z:?}"))?;
    if !(1..=60).contains(&n) || !(hem == "S" || hem == "N") {
        return Err(format!("Zona UTM no válida: {z:?}"));
    }
    Ok((-183.0 + 6.0 * n as f64, hem == "S"))
}

/// Zona UTM (hemisferio sur) que contiene la longitud dada.
pub fn zona_desde_longitud(lon: f64) -> String {
    format!("{}S", ((lon + 180.0) / 6.0).floor() as i32 + 1)
}

struct Coef {
    a_rect: f64,
    alfa: [f64; 6],
    beta: [f64; 6],
}

fn coef() -> Coef {
    let n = F / (2.0 - F);
    let (n2, n3, n4, n5, n6) = (n * n, n.powi(3), n.powi(4), n.powi(5), n.powi(6));
    let a_rect = A / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
    let alfa = [
        n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0 + 41.0 * n4 / 180.0 - 127.0 * n5 / 288.0 + 7891.0 * n6 / 37800.0,
        13.0 * n2 / 48.0 - 3.0 * n3 / 5.0 + 557.0 * n4 / 1440.0 + 281.0 * n5 / 630.0 - 1983433.0 * n6 / 1935360.0,
        61.0 * n3 / 240.0 - 103.0 * n4 / 140.0 + 15061.0 * n5 / 26880.0 + 167603.0 * n6 / 181440.0,
        49561.0 * n4 / 161280.0 - 179.0 * n5 / 168.0 + 6601661.0 * n6 / 7257600.0,
        34729.0 * n5 / 80640.0 - 3418889.0 * n6 / 1995840.0,
        212378941.0 * n6 / 319334400.0,
    ];
    let beta = [
        n / 2.0 - 2.0 * n2 / 3.0 + 37.0 * n3 / 96.0 - n4 / 360.0 - 81.0 * n5 / 512.0 + 96199.0 * n6 / 604800.0,
        n2 / 48.0 + n3 / 15.0 - 437.0 * n4 / 1440.0 + 46.0 * n5 / 105.0 - 1118711.0 * n6 / 3870720.0,
        17.0 * n3 / 480.0 - 37.0 * n4 / 840.0 - 209.0 * n5 / 4480.0 + 5569.0 * n6 / 90720.0,
        4397.0 * n4 / 161280.0 - 11.0 * n5 / 504.0 - 830251.0 * n6 / 7257600.0,
        4583.0 * n5 / 161280.0 - 108847.0 * n6 / 3991680.0,
        20648693.0 * n6 / 638668800.0,
    ];
    Coef { a_rect, alfa, beta }
}

/// Longitud/latitud WGS84 (grados) → este/norte UTM (m).
pub fn geograficas_a_utm(lon: f64, lat: f64, zona_utm: &str) -> Result<(f64, f64), String> {
    let (lon0, sur) = zona(zona_utm)?;
    let c = coef();
    let e = (F * (2.0 - F)).sqrt();
    let phi = lat.to_radians();
    let lam = (lon - lon0).to_radians();
    let t = (phi.sin().atanh() - e * (e * phi.sin()).atanh()).sinh();
    let xi_p = t.atan2(lam.cos());
    let eta_p = (lam.sin() / (1.0 + t * t).sqrt()).atanh();
    let (mut xi, mut eta) = (xi_p, eta_p);
    for (j, a) in c.alfa.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi += a * (k * xi_p).sin() * (k * eta_p).cosh();
        eta += a * (k * xi_p).cos() * (k * eta_p).sinh();
    }
    let este = FE + K0 * c.a_rect * eta;
    let norte = K0 * c.a_rect * xi + if sur { FN_SUR } else { 0.0 };
    Ok((este, norte))
}

/// Este/norte UTM (m) → longitud/latitud WGS84 (grados).
pub fn utm_a_geograficas(este: f64, norte: f64, zona_utm: &str) -> Result<(f64, f64), String> {
    let (lon0, sur) = zona(zona_utm)?;
    let c = coef();
    let e = (F * (2.0 - F)).sqrt();
    let xi = (norte - if sur { FN_SUR } else { 0.0 }) / (K0 * c.a_rect);
    let eta = (este - FE) / (K0 * c.a_rect);
    let (mut xi_p, mut eta_p) = (xi, eta);
    for (j, b) in c.beta.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi_p -= b * (k * xi).sin() * (k * eta).cosh();
        eta_p -= b * (k * xi).cos() * (k * eta).sinh();
    }
    let chi = (xi_p.sin() / eta_p.cosh()).asin();
    let lam = eta_p.sinh().atan2(xi_p.cos());
    // latitud geodésica desde la conforme: iteración de Newton sobre tan φ
    let tau_p = chi.tan();
    let mut tau = tau_p;
    for _ in 0..10 {
        let sigma = (e * (e * tau / (1.0 + tau * tau).sqrt()).atanh()).sinh();
        let tau_i = tau * (1.0 + sigma * sigma).sqrt() - sigma * (1.0 + tau * tau).sqrt();
        let d = (tau_p - tau_i) / (1.0 + tau_i * tau_i).sqrt() * (1.0 + (1.0 - e * e) * tau * tau)
            / ((1.0 - e * e) * (1.0 + tau * tau).sqrt());
        tau += d;
        if d.abs() < 1e-14 {
            break;
        }
    }
    Ok((lon0 + lam.to_degrees(), tau.atan().to_degrees()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        for (lon, lat) in [(-70.0, -15.8), (-76.9, -12.05), (-80.5, -3.6), (-69.2, -18.0)] {
            let z = zona_desde_longitud(lon);
            let (e, n) = geograficas_a_utm(lon, lat, &z).unwrap();
            let (lo, la) = utm_a_geograficas(e, n, &z).unwrap();
            assert!((lo - lon).abs() < 1e-10 && (la - lat).abs() < 1e-10, "{lon} {lat}: {lo} {la}");
        }
        assert_eq!(zona_desde_longitud(-70.0), "19S");
        assert!(zona("99S").is_err());
    }
}
