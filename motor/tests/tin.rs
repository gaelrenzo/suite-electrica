//! TIN (spade) vs Python (qhull): triangulación, cotas, curvas de nivel y perfil longitudinal.

mod comun;

use comun::n;
use suite_motor::geo::perfil_longitudinal;
use suite_motor::tin::Tin;

#[test]
fn igual_a_python() {
    let texto = include_str!("tin.txt");
    let filas: Vec<Vec<&str>> = texto.lines().map(|l| l.split_whitespace().collect()).collect();
    let xyz: Vec<[f64; 3]> =
        filas.iter().filter(|t| t[0] == "punto").map(|t| [n(t[1]), n(t[2]), n(t[3])]).collect();
    for (tag, max_lado) in [("sin", None), ("max35", Some(35.0))] {
        let tin = Tin::nuevo(&xyz, max_lado).unwrap();
        let de = |k: &'static str| filas.iter().filter(move |t| t[0] == k && t[1] == tag);
        let t = de("tin").next().unwrap();
        assert_eq!(tin.triangulos.len(), t[2].parse::<usize>().unwrap(), "{tag}: triángulos");
        assert_eq!(tin.activos.iter().filter(|&&a| a).count(), t[3].parse::<usize>().unwrap(), "{tag}: activos");
        assert_eq!(tin.aristas.len(), t[4].parse::<usize>().unwrap(), "{tag}: aristas");
        for c in de("cota") {
            let (z, e) = (tin.cota(n(c[2]), n(c[3])), n(c[4]));
            assert!(
                (z.is_nan() && e.is_nan()) || (z - e).abs() <= 1e-9 * e.abs().max(1.0),
                "{tag} cota ({}, {}): rust {z} python {e}",
                c[2],
                c[3]
            );
        }
        for c in de("curvas") {
            let cs = tin.curvas_nivel(&[n(c[2])]);
            assert_eq!(cs.len(), c[3].parse::<usize>().unwrap(), "{tag} nivel {}", c[2]);
            let l: f64 = cs.iter().map(|x| x.longitud()).sum();
            assert!((l - n(c[4])).abs() <= 1e-7 * n(c[4]).max(1.0), "{tag} nivel {} longitud {l} vs {}", c[2], c[4]);
            assert_eq!(cs.iter().filter(|x| x.cerrada).count(), c[5].parse::<usize>().unwrap());
        }
        let eje: Vec<[f64; 2]> = de("eje")
            .next()
            .unwrap()[2..]
            .iter()
            .map(|p| {
                let (a, b) = p.split_once(',').unwrap();
                [n(a), n(b)]
            })
            .collect();
        let pf = perfil_longitudinal(&tin, &eje, 20.0).unwrap();
        let esperado: Vec<&Vec<&str>> = de("perfil").collect();
        assert_eq!(pf.progresiva.len(), esperado.len(), "{tag}: puntos del perfil");
        for (k, e) in esperado.iter().enumerate() {
            assert!((pf.progresiva[k] - n(e[2])).abs() < 1e-7, "{tag} s[{k}]");
            let (z, ez) = (pf.cota[k], n(e[3]));
            assert!((z.is_nan() && ez.is_nan()) || (z - ez).abs() < 1e-7, "{tag} z[{k}] {z} vs {ez}");
            assert_eq!(pf.tipo[k].to_string(), e[4], "{tag} tipo[{k}]");
        }
    }
}
