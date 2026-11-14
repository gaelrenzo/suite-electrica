//! UTM propio vs pyproj (PROJ) en las zonas 17S, 18S y 19S.

use suite_motor::utm::{geograficas_a_utm, utm_a_geograficas};

#[test]
fn igual_a_pyproj() {
    let mut max_err: f64 = 0.0;
    for l in include_str!("utm.txt").lines().filter(|l| !l.trim().is_empty()) {
        let t: Vec<&str> = l.split_whitespace().collect();
        let (lon, lat): (f64, f64) = (t[0].parse().unwrap(), t[1].parse().unwrap());
        let (e, n): (f64, f64) = (t[3].parse().unwrap(), t[4].parse().unwrap());
        let (er, nr) = geograficas_a_utm(lon, lat, t[2]).unwrap();
        max_err = max_err.max((er - e).abs()).max((nr - n).abs());
        let (lo, la) = utm_a_geograficas(e, n, t[2]).unwrap();
        assert!((lo - lon).abs() < 1e-9 && (la - lat).abs() < 1e-9, "inversa {l}: {lo} {la}");
    }
    assert!(max_err < 1e-5, "error máximo {max_err} m");
}
