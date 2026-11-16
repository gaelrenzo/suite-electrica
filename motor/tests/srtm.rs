//! Relieve SRTM: hoja `.hgt` sintética (1201², cota = f(fila, columna)), recorte alrededor del eje y
//! conversión a UTM.

use std::path::PathBuf;

use suite_motor::documento::Documento;
use suite_motor::entrada::{esquina_hgt, leer_hgt};
use suite_motor::utm::{geograficas_a_utm, utm_a_geograficas};

const N: usize = 1201;

fn cota(f: usize, c: usize) -> i16 {
    (3800 + (f % 97) + 2 * (c % 53)) as i16
}

#[test]
fn hoja_srtm_recortada_al_eje() {
    assert_eq!(esquina_hgt("S16W070.hgt").unwrap(), (-16, -70));
    assert_eq!(esquina_hgt("n05e012.HGT").unwrap(), (5, 12));
    assert!(esquina_hgt("terreno.hgt").is_err());

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("srtm");
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("S16W070.hgt");
    let mut b = Vec::with_capacity(N * N * 2);
    for f in 0..N {
        for c in 0..N {
            let z = if f == 600 && c == 600 { i16::MIN } else { cota(f, c) };
            b.extend_from_slice(&z.to_be_bytes());
        }
    }
    std::fs::write(&ruta, &b).unwrap();

    // eje de ~1 km en el centro de la hoja (lat −15,5°, lon −69,5°), zona 19S
    let (e0, n0) = geograficas_a_utm(-69.5, -15.5, "19S").unwrap();
    let mut doc = Documento::nuevo();
    doc.zona_utm = "19S".into();
    assert!(doc.importar_srtm(&ruta, 300.0).unwrap_err().contains("eje"));
    doc.eje = Some(vec![[e0, n0], [e0 + 800.0, n0 + 600.0]]);
    let k = doc.importar_srtm(&ruta, 300.0).unwrap();
    // franja de 600 m de ancho en un eje de 1000 m, muestras de ~89 × 92 m (3") → del orden de 100
    assert!((60..=160).contains(&k), "{k} puntos");
    for p in &doc.puntos {
        // dentro de la franja de 300 m del eje
        assert!(suite_motor::geo::proyectar_en_eje(doc.eje.as_ref().unwrap(), [p.este, p.norte]).1 <= 300.0);
        assert!(p.este >= e0 - 300.0 && p.este <= e0 + 1100.0 && p.norte >= n0 - 300.0 && p.norte <= n0 + 900.0);
        // la cota corresponde a la muestra de esas coordenadas
        let (lon, lat) = utm_a_geograficas(p.este, p.norte, "19S").unwrap();
        let f = ((-15.0 - lat) * 1200.0).round() as usize;
        let c = ((lon + 70.0) * 1200.0).round() as usize;
        assert_eq!(p.cota, cota(f, c) as f64, "{}", p.punto);
    }
    // el vacío del centro (f = c = 600, justo en −15,5°/−69,5°) no aparece
    assert!(!doc.puntos.iter().any(|p| p.punto == "SRTM-600-600"));
    assert!(doc.fuentes.iter().any(|(k, v)| k == "topografia" && v.contains("SRTM")));
    // fuera de la hoja
    assert!(leer_hgt(&ruta, "19S", [e0 + 3e5, n0, e0 + 3e5 + 10.0, n0 + 10.0]).is_err());
}
