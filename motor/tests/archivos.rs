//! Lectura por formato (CSV, DXF, KML, GPX, MapSource, Excel RLT, TPG) y limpieza: Rust vs Python.
//! Archivos válidos y problemáticos en `tests/archivos/`, esperado en `tests/archivos_esperado.txt`.

mod comun;

use std::collections::BTreeMap;
use std::path::PathBuf;

use comun::n;
use suite_motor::entrada::{angulo_grados, importar, leer, leer_perfil_tabular, leer_polilinea, limpiar, Punto};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("archivos")
}

fn sin_dir(msg: &str) -> String {
    let d = dir().display().to_string();
    msg.replace(&format!("{d}\\"), "").replace(&format!("{d}/"), "")
}

fn igual(o: f64, e: f64) -> bool {
    (o.is_nan() && e.is_nan()) || (o - e).abs() <= 1e-6 * e.abs().max(1.0)
}

fn cmp_puntos(nombre: &str, obtenidos: &[Punto], esperado: &[Vec<String>]) {
    assert_eq!(obtenidos.len(), esperado.len(), "{nombre}: cantidad de puntos");
    for (p, e) in obtenidos.iter().zip(esperado) {
        assert_eq!(p.punto, e[0], "{nombre}: nombre");
        for (k, (o, x)) in [(p.este, &e[1]), (p.norte, &e[2]), (p.cota, &e[3])].iter().enumerate() {
            assert!(igual(*o, n(x)), "{nombre} {} coord{k}: rust {o} python {x}", p.punto);
        }
        assert_eq!(p.descripcion, e[4], "{nombre} {}: descripción", p.punto);
    }
}

#[test]
fn formatos_validos_y_problematicos() {
    let texto = include_str!("archivos_esperado.txt");
    // agrupa por archivo
    let mut ptos: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let mut reps: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut errores: BTreeMap<String, String> = BTreeMap::new();
    let mut rutas: BTreeMap<String, Vec<(String, Vec<[f64; 3]>)>> = BTreeMap::new();
    let mut perfiles: BTreeMap<String, Vec<(f64, f64, String)>> = BTreeMap::new();
    let mut ejes: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();
    let mut otros = Vec::new();
    for l in texto.lines().filter(|l| !l.is_empty()) {
        let (tipo, resto) = l.split_once(' ').unwrap();
        let (arch, dato) = resto.split_once(' ').unwrap_or((resto, ""));
        let (arch, dato) = (arch.to_string(), dato.to_string());
        match tipo {
            "pto" => ptos.entry(arch).or_default().push(dato.split('|').map(String::from).collect()),
            "rep" => {
                let (k, v) = dato.split_once(' ').unwrap_or((&dato, ""));
                reps.entry(arch).or_default().push((k.into(), v.into()));
            }
            "error" => {
                errores.insert(arch, dato);
            }
            "ruta" => {
                let mut t = dato.split_whitespace();
                let nombre = t.next().unwrap().replace('_', " ");
                let v = t.map(|c| {
                    let x: Vec<f64> = c.split(',').map(n).collect();
                    [x[0], x[1], x[2]]
                });
                rutas.entry(arch).or_default().push((nombre, v.collect()));
            }
            "perfil" => {
                let t: Vec<&str> = dato.split_whitespace().collect();
                perfiles.entry(arch).or_default().push((n(t[0]), n(t[1]), t[2].into()));
            }
            "eje" | "polilinea" => {
                let v = dato.split_whitespace().map(|c| {
                    let (a, b) = c.split_once(',').unwrap();
                    [n(a), n(b)]
                });
                ejes.insert(arch, v.collect());
            }
            _ => otros.push(l.to_string()),
        }
    }

    // lectura + limpieza
    for arch in ["csv_encabezado.csv", "csv_pnezd.csv", "csv_puntoycoma.csv", "csv_problemas.csv", "csv_grados.csv",
        "puntos.dxf", "sin_puntos.dxf", "mapsource.txt", "mapsource_grados.txt"]
    {
        let r = leer(&dir().join(arch)).and_then(|pts| {
            cmp_puntos(arch, &pts, ptos.get(arch).map(|v| v.as_slice()).unwrap_or(&[]));
            limpiar(&pts, 0.001, 0.001, false)
        });
        match (r, errores.get(arch)) {
            (Err(e), Some(esp)) => assert_eq!(sin_dir(&e), *esp, "{arch}: mensaje de error"),
            (Err(e), None) => panic!("{arch}: error inesperado {e}"),
            (Ok(_), Some(esp)) => panic!("{arch}: se esperaba el error {esp}"),
            (Ok((_, rep)), None) => {
                for (k, v) in &reps[arch] {
                    let obtenido = match k.as_str() {
                        "invalidos" => rep.invalidos.join(","),
                        "duplicados" => rep.duplicados.join(","),
                        "conflictos" => rep.conflictos.join(","),
                        "nombres_repetidos" => rep.nombres_repetidos.join(","),
                        "atipicos" => rep.atipicos.join(","),
                        "fuera_zona" => rep.fuera_zona.join(","),
                        "cotas_fuera_rango" => rep.cotas_fuera_rango.join(","),
                        "n_salida" => rep.n_salida.to_string(),
                        otro => panic!("clave {otro}"),
                    };
                    assert_eq!(&obtenido, v, "{arch}: {k}");
                }
            }
        }
    }

    // puntos + rutas
    for arch in ["puntos.kml", "puntos.gpx", "rlt_utm.xlsx"] {
        let (pts, rs) = importar(&dir().join(arch), None).unwrap_or_else(|e| panic!("{arch}: {e}"));
        cmp_puntos(arch, &pts, ptos.get(arch).map(|v| v.as_slice()).unwrap_or(&[]));
        let esp = rutas.get(arch).cloned().unwrap_or_default();
        assert_eq!(rs.len(), esp.len(), "{arch}: rutas");
        for ((nom, v), (enom, ev)) in rs.iter().zip(&esp) {
            assert_eq!(nom, enom, "{arch}: nombre de ruta");
            assert_eq!(v.len(), ev.len(), "{arch} {nom}: vértices");
            for (a, b) in v.iter().zip(ev) {
                assert!((0..3).all(|k| igual(a[k], b[k])), "{arch} {nom}: {a:?} vs {b:?}");
            }
        }
    }

    // perfiles tabulares
    for arch in ["rlt_acumulada.xlsx", "perfil.tpg", "perfil_sin_encabezado.tpg"] {
        let (p, eje) = leer_perfil_tabular(&dir().join(arch)).unwrap_or_else(|e| panic!("{arch}: {e}"));
        let esp = &perfiles[arch];
        assert_eq!(p.progresiva.len(), esp.len(), "{arch}: puntos del perfil");
        for (k, (s, z, t)) in esp.iter().enumerate() {
            assert!(igual(p.progresiva[k], *s) && igual(p.cota[k], *z), "{arch}[{k}]");
            assert_eq!(p.tipo[k].to_string(), *t, "{arch}[{k}] tipo");
        }
        let ee = &ejes[arch];
        assert_eq!(eje.len(), ee.len(), "{arch}: vértices del eje");
        for (a, b) in eje.iter().zip(ee) {
            assert!(igual(a[0], b[0]) && igual(a[1], b[1]), "{arch} eje {a:?} vs {b:?}");
        }
    }

    // eje DXF y ángulos
    let eje = leer_polilinea(&dir().join("eje.dxf"), None).unwrap();
    assert_eq!(eje, ejes["eje.dxf"]);
    for l in otros.iter().filter(|l| l.starts_with("angulo ")) {
        let t: Vec<&str> = l.split_whitespace().collect();
        let arg = if t[1] == "_" { String::new() } else { t[1].replace('_', " ") };
        assert!(igual(angulo_grados(&arg).unwrap(), n(t[2])), "ángulo {arg}");
    }
}
