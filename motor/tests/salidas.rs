//! Etapa 4: salidas Rust (Excel, DXF, PDF) frente a las de Python (`tests/salidas_python/`,
//! generadas por `generar_referencia_salidas.py`).
//!
//! * Excel: mismas hojas en el mismo orden, mismos encabezados y celdas (números a 1e-9, montos
//!   de PresuCraft a ±0,01 por el orden de suma; ver docs/PLAN_RUST.md).
//! * DXF: mismas capas y colores; mismas entidades en el mismo orden, por capa, con la misma
//!   geometría (1e-6 m) y textos. LWPOLYLINE de Python = POLYLINE 2D de Rust (R12).
//! * PDF: se generan aquí; el texto se compara con `python motor/comparar_salidas.py`.
//!
//! Las salidas de Rust quedan en `motor/target/tmp/salidas_rust/`.

mod comun;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use calamine::{Data, Reader};
use suite_motor::alertas::{alertas, ConfigAlertas};
use suite_motor::catalogos::{elegir_poste, fuerzas_maximas, metrado, verificar, Catalogos};
use suite_motor::flechado::tabla_flechado;
use suite_motor::geo::perfil_longitudinal;
use suite_motor::linea::{calcular, Proyecto};
use suite_motor::proyectos::cargar_proyecto_json;
use suite_motor::salidas::{escribir_excel, tablas_linea, tablas_perfil, tablas_presupuesto, tablas_red, Extras};
use suite_motor::tin::{niveles, Tin};
use suite_motor::{dxf, entrada::Punto, pdf, presupuesto, red};

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn py(nombre: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("salidas_python").join(nombre)
}

fn rs(nombre: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("salidas_rust");
    std::fs::create_dir_all(&d).unwrap();
    d.join(nombre)
}

// ------------------------------------------------------------------ Excel

fn hojas(ruta: &Path) -> Vec<(String, Vec<Vec<Data>>)> {
    let mut wb = calamine::open_workbook_auto(ruta).unwrap_or_else(|e| panic!("{}: {e}", ruta.display()));
    let nombres = wb.sheet_names().to_vec();
    nombres
        .into_iter()
        .map(|n| {
            let r = wb.worksheet_range(&n).unwrap();
            (n, r.rows().map(|f| f.to_vec()).collect())
        })
        .collect()
}

fn numero(d: &Data) -> Option<f64> {
    match d {
        Data::Float(x) => Some(*x),
        Data::Int(i) => Some(*i as f64),
        _ => None,
    }
}

fn vacia(d: &Data) -> bool {
    matches!(d, Data::Empty) || matches!(d, Data::String(s) if s.is_empty())
}

fn comparar_excel(nombre: &str, tol_abs: f64) {
    let (a, b) = (hojas(&rs(nombre)), hojas(&py(nombre)));
    let na: Vec<&str> = a.iter().map(|h| h.0.as_str()).collect();
    let nb: Vec<&str> = b.iter().map(|h| h.0.as_str()).collect();
    assert_eq!(na, nb, "{nombre}: hojas");
    let mut celdas = 0;
    for ((hoja, fa), (_, fb)) in a.iter().zip(&b) {
        assert_eq!(fa.len(), fb.len(), "{nombre}/{hoja}: filas");
        for (i, (ra, rb)) in fa.iter().zip(fb).enumerate() {
            let n = ra.len().max(rb.len());
            for j in 0..n {
                let (x, y) = (ra.get(j).unwrap_or(&Data::Empty), rb.get(j).unwrap_or(&Data::Empty));
                let etq = format!("{nombre}/{hoja} fila {} col {}: rust {x:?} python {y:?}", i + 1, j + 1);
                match (numero(x), numero(y)) {
                    (Some(p), Some(q)) => assert!((p - q).abs() <= tol_abs.max(1e-9 * q.abs()), "{etq}"),
                    _ if vacia(x) && vacia(y) => {}
                    _ => assert_eq!(x.to_string(), y.to_string(), "{etq}"),
                }
                celdas += 1;
            }
        }
    }
    println!("{nombre}: {} hojas, {celdas} celdas iguales", a.len());
}

// ------------------------------------------------------------------ DXF

#[derive(Debug)]
struct Ent {
    tipo: String,
    capa: String,
    nums: Vec<f64>,
    texto: String,
}

fn decodificar(s: &str) -> String {
    let mut out = String::new();
    let mut resto = s;
    while let Some(k) = resto.find("\\U+") {
        out.push_str(&resto[..k]);
        let hexa = &resto[k + 3..k + 7];
        out.push(char::from_u32(u32::from_str_radix(hexa, 16).unwrap()).unwrap());
        resto = &resto[k + 7..];
    }
    out + resto
}

/// Entidades normalizadas y capas (nombre → color) de un DXF ASCII.
fn leer_dxf(ruta: &Path) -> (Vec<Ent>, BTreeMap<String, i64>) {
    let texto = String::from_utf8_lossy(&std::fs::read(ruta).unwrap()).into_owned();
    let l: Vec<&str> = texto.lines().collect();
    let pares: Vec<(i32, &str)> = l.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].trim().parse().unwrap(), c[1].trim())).collect();
    let mut capas = BTreeMap::new();
    let mut bloques: Vec<(String, Vec<(i32, String)>)> = Vec::new();
    for &(c, v) in &pares {
        if c == 0 {
            bloques.push((v.to_string(), Vec::new()));
        } else if let Some(b) = bloques.last_mut() {
            b.1.push((c, v.to_string()));
        }
    }
    // capas
    for (t, p) in &bloques {
        if t == "LAYER" {
            let nombre = p.iter().find(|x| x.0 == 2).map(|x| x.1.clone()).unwrap_or_default();
            let color = p.iter().find(|x| x.0 == 62).and_then(|x| x.1.parse().ok()).unwrap_or(7);
            if nombre != "0" && !nombre.eq_ignore_ascii_case("Defpoints") {
                capas.insert(nombre, color);
            }
        }
    }
    // entidades del espacio modelo (desde la sección ENTITIES)
    let ini = bloques.iter().position(|b| b.0 == "SECTION" && b.1.iter().any(|x| x.0 == 2 && x.1 == "ENTITIES")).unwrap();
    let mut ents = Vec::new();
    let mut i = ini + 1;
    let g = |p: &Vec<(i32, String)>, c: i32| p.iter().find(|x| x.0 == c).and_then(|x| x.1.parse::<f64>().ok());
    let todos = |p: &Vec<(i32, String)>, c: i32| p.iter().filter(|x| x.0 == c).map(|x| x.1.parse::<f64>().unwrap()).collect::<Vec<_>>();
    while i < bloques.len() && bloques[i].0 != "ENDSEC" {
        let (t, p) = &bloques[i];
        let capa = p.iter().find(|x| x.0 == 8).map(|x| x.1.clone()).unwrap_or_default();
        let mut e = Ent { tipo: t.clone(), capa, nums: Vec::new(), texto: String::new() };
        match t.as_str() {
            "POINT" => e.nums = vec![g(p, 10).unwrap(), g(p, 20).unwrap(), g(p, 30).unwrap_or(0.0)],
            "LINE" => e.nums = [10, 20, 11, 21].iter().map(|&c| g(p, c).unwrap()).collect(),
            "CIRCLE" => e.nums = [10, 20, 40].iter().map(|&c| g(p, c).unwrap()).collect(),
            "3DFACE" => e.nums = [10, 20, 30, 11, 21, 31, 12, 22, 32, 13, 23, 33].iter().map(|&c| g(p, c).unwrap()).collect(),
            "TEXT" => {
                e.nums = vec![g(p, 10).unwrap(), g(p, 20).unwrap(), g(p, 40).unwrap(), g(p, 50).unwrap_or(0.0)];
                e.texto = decodificar(&p.iter().find(|x| x.0 == 1).map(|x| x.1.clone()).unwrap_or_default());
            }
            "LWPOLYLINE" => {
                e.tipo = "POLILINEA".into();
                let (xs, ys) = (todos(p, 10), todos(p, 20));
                e.nums = vec![g(p, 38).unwrap_or(0.0), (g(p, 70).unwrap_or(0.0) as i64 & 1) as f64];
                e.nums.extend(xs.iter().zip(&ys).flat_map(|(x, y)| [*x, *y]));
            }
            "POLYLINE" => {
                e.tipo = "POLILINEA".into();
                e.nums = vec![g(p, 30).unwrap_or(0.0), (g(p, 70).unwrap_or(0.0) as i64 & 1) as f64];
                i += 1;
                while bloques[i].0 == "VERTEX" {
                    e.nums.extend([g(&bloques[i].1, 10).unwrap(), g(&bloques[i].1, 20).unwrap()]);
                    i += 1;
                }
                assert_eq!(bloques[i].0, "SEQEND");
            }
            otro => panic!("{}: entidad no esperada {otro}", ruta.display()),
        }
        ents.push(e);
        i += 1;
    }
    (ents, capas)
}

fn comparar_dxf(nombre: &str) {
    let (mut ea, ca) = leer_dxf(&rs(nombre));
    let (mut eb, cb) = leer_dxf(&py(nombre));
    assert_eq!(ca, cb, "{nombre}: capas y colores");
    // TIN: mismos triángulos, pero qhull y spade los enumeran en otro orden y con otro vértice
    // inicial; se comparan como conjunto (vértices ordenados)
    let caras = |e: &mut Vec<Ent>| {
        let mut c: Vec<Vec<[i64; 3]>> = e
            .iter()
            .filter(|x| x.tipo == "3DFACE")
            .map(|x| {
                let mut v: Vec<[i64; 3]> = x.nums.chunks(3).take(3).map(|p| [0, 1, 2].map(|k| (p[k] * 1e6).round() as i64)).collect();
                v.sort();
                v
            })
            .collect();
        c.sort();
        e.retain(|x| x.tipo != "3DFACE");
        c
    };
    let (fa, fb) = (caras(&mut ea), caras(&mut eb));
    assert_eq!(fa, fb, "{nombre}: triángulos del TIN");
    if !fa.is_empty() {
        println!("{nombre}: {} caras 3D iguales (como conjunto)", fa.len());
    }
    // curvas de nivel: misma geometría; el punto de inicio depende del orden de los triángulos
    let es_curva = |x: &Ent| x.capa.starts_with("CURVAS_");
    let curvas = |e: &[Ent]| {
        let mut c: Vec<(String, Vec<i64>)> = e
            .iter()
            .filter(|x| es_curva(x) && x.tipo == "POLILINEA")
            .map(|x| {
                let mut v: Vec<[i64; 2]> = x.nums[2..].chunks(2).map(|p| [(p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64]).collect();
                if x.nums[1] == 1.0 || v.first() == v.last() {
                    if v.first() == v.last() {
                        v.pop();
                    }
                    let k = (0..v.len()).min_by_key(|&i| v[i]).unwrap();
                    v.rotate_left(k);
                    if v.len() > 2 && v[v.len() - 1] < v[1] {
                        v[1..].reverse();
                    }
                } else if v.last() < v.first() {
                    v.reverse();
                }
                let mut n = vec![(x.nums[0] * 1e6).round() as i64, x.nums[1] as i64];
                n.extend(v.into_iter().flatten());
                (x.capa.clone(), n)
            })
            .collect();
        c.sort();
        c
    };
    let (cva, cvb) = (curvas(&ea), curvas(&eb));
    assert_eq!(cva.len(), cvb.len(), "{nombre}: cantidad de curvas de nivel");
    for (a, b) in cva.iter().zip(&cvb) {
        assert_eq!(a.0, b.0, "{nombre}: capa de curva");
        assert_eq!(a.1.len(), b.1.len(), "{nombre}: vértices de curva {}", a.1[0]);
        assert!(a.1.iter().zip(&b.1).all(|(x, y)| (x - y).abs() <= 1), "{nombre}: curva {}", a.1[0]);
    }
    // rótulos: mismo texto, altura y capa, y sobre su curva
    let sobre = |t: &Ent, e: &[Ent]| {
        e.iter().filter(|x| x.tipo == "POLILINEA" && x.capa == t.capa && (x.nums[0] - comun::n(&t.texto)).abs() < 1e-9).any(|c| {
            c.nums[2..].chunks(2).collect::<Vec<_>>().windows(2).any(|w| {
                let (a, b) = (w[0], w[1]);
                let (mx, my) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
                (mx - t.nums[0]).abs() < 1e-6 && (my - t.nums[1]).abs() < 1e-6
            })
        })
    };
    let rot = |e: &[Ent]| {
        let mut r: Vec<(String, String, i64)> =
            e.iter().filter(|x| es_curva(x) && x.tipo == "TEXT").map(|x| (x.capa.clone(), x.texto.clone(), (x.nums[2] * 1e6) as i64)).collect();
        r.sort();
        r
    };
    assert_eq!(rot(&ea), rot(&eb), "{nombre}: rótulos de curvas");
    assert!(ea.iter().filter(|x| es_curva(x) && x.tipo == "TEXT").all(|t| sobre(t, &ea)), "{nombre}: rótulo fuera de su curva");
    if !cva.is_empty() {
        println!("{nombre}: {} curvas de nivel iguales (como geometría) y {} rótulos sobre su curva", cva.len(), rot(&ea).len());
    }
    ea.retain(|x| !es_curva(x));
    eb.retain(|x| !es_curva(x));
    assert_eq!(ea.len(), eb.len(), "{nombre}: cantidad de entidades");
    let mut por_capa: BTreeMap<String, usize> = BTreeMap::new();
    for (k, (a, b)) in ea.iter().zip(&eb).enumerate() {
        assert_eq!((&a.tipo, &a.capa), (&b.tipo, &b.capa), "{nombre}: entidad {k}");
        assert_eq!(a.texto, b.texto, "{nombre}: texto de la entidad {k}");
        assert_eq!(a.nums.len(), b.nums.len(), "{nombre}: vértices de la entidad {k} ({})", a.tipo);
        for (x, y) in a.nums.iter().zip(&b.nums) {
            assert!((x - y).abs() <= 1e-6, "{nombre}: entidad {k} {} {}: rust {x} python {y}", a.tipo, a.capa);
        }
        *por_capa.entry(a.capa.clone()).or_default() += 1;
    }
    println!("{nombre}: {} entidades iguales {por_capa:?}", ea.len());
}

// ------------------------------------------------------------------ casos

fn proyecto(json: &str, volcado: &str) -> Proyecto {
    // perfil y eje del volcado de Python: la malla regular de ejemplo admite otra triangulación
    // (ver docs/PLAN_RUST.md); así ambas versiones dibujan el mismo terreno
    let mut p = cargar_proyecto_json(&raiz().join("examples").join("linecraft").join(json)).unwrap();
    let caso = comun::leer(volcado);
    p.perfil = caso.p.perfil;
    p.eje = caso.p.eje;
    p
}

fn salidas_linea(nombre: &str, p: &Proyecto, catalogos: bool) {
    let mut r = calcular(p).unwrap();
    let (mut ver, mut met, al);
    if catalogos {
        let postes: Vec<&str> = include_str!("catalogos_auto.txt").lines().filter_map(|l| l.strip_prefix("postes ")).collect();
        let base = Catalogos::plantilla();
        let mut cat = Catalogos::desde_csv(&postes.join("\n"), "codigo\n", "codigo\n", "armado\n").unwrap();
        cat.armados = base.armados;
        cat.retenidas = base.retenidas;
        cat.materiales = base.materiales;
        let fm = fuerzas_maximas(&r);
        for e in r.estructuras.iter_mut() {
            e.armado = cat.elegir_armado(e.tipo, e.angulo);
            let (fh, mb, _) = fm.get(&e.numero).copied().unwrap_or((0.0, 0.0, 0.0));
            e.poste = elegir_poste(&cat, e, fh, mb, 2.0);
        }
        r.estructuras[2].poste = "M 12/C6".into();
        ver = Some(verificar(&r, p, &cat, 2.0, 2.0, None, &BTreeMap::new()));
        met = Some(metrado(&r, p, &cat, 3.0, None, &BTreeMap::new()));
        let cfg = ConfigAlertas {
            fs_poste: 4.0, fs_retenida: 3.0, tension_kv: 22.9, sep_k_tension: 0.04, sep_k_flecha: 0.3,
            angulo_salida_min: -20.0, angulo_salida_max: 5.0, tiro_max_espiga: 2000.0, eds_max_pct: 10.0,
        };
        al = alertas(&r, p, Some(&cat), ver.as_deref().unwrap(), &cfg);
    } else {
        ver = None;
        met = None;
        al = alertas(&r, p, None, &[], &ConfigAlertas::desde_mapa(&p.alertas));
    }
    let fl = tabla_flechado(&r, p, 20.0, 5.0).unwrap();
    let x = Extras { verificacion: ver.as_deref(), metrado: met.as_deref(), flechado: &fl, alertas: Some(&al) };
    escribir_excel(&rs(&format!("{nombre}_planilla.xlsx")), &tablas_linea(&r, p, &x)).unwrap();
    dxf::perfil_linea(&rs(&format!("{nombre}_perfil.dxf")), &r, p, 10.0).unwrap();
    pdf::memoria_pdf(&rs(&format!("{nombre}_memoria.pdf")), &r, p, &x).unwrap();
    ver.take();
    met.take();
}

#[test]
fn linea_auto_con_catalogos() {
    let p = proyecto("proyecto.json", include_str!("linea_auto.txt"));
    salidas_linea("linea_auto", &p, true);
    comparar_excel("linea_auto_planilla.xlsx", 0.0);
    comparar_dxf("linea_auto_perfil.dxf");
}

#[test]
fn linea_manual_cli() {
    let p = proyecto("proyecto_manual.json", include_str!("linea_manual.txt"));
    salidas_linea("linea_manual", &p, false);
    comparar_excel("linea_manual_planilla.xlsx", 0.0);
    comparar_dxf("linea_manual_perfil.dxf");
}

#[test]
fn topocraft() {
    let txt = include_str!("tin.txt");
    let xyz: Vec<[f64; 3]> = txt
        .lines()
        .filter_map(|l| l.strip_prefix("punto "))
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(comun::n).collect();
            [v[0], v[1], v[2]]
        })
        .collect();
    let eje: Vec<[f64; 2]> = txt
        .lines()
        .find_map(|l| l.strip_prefix("eje sin "))
        .unwrap()
        .split_whitespace()
        .map(|c| {
            let (a, b) = c.split_once(',').unwrap();
            [comun::n(a), comun::n(b)]
        })
        .collect();
    let tin = Tin::nuevo(&xyz, None).unwrap();
    let z = xyz.iter().map(|p| p[2]);
    let (zmin, zmax) = (z.clone().fold(f64::INFINITY, f64::min), z.fold(f64::NEG_INFINITY, f64::max));
    let curvas = tin.curvas_nivel(&niveles(zmin, zmax, 2.5).unwrap());
    let pts: Vec<Punto> = xyz
        .iter()
        .enumerate()
        .map(|(i, p)| Punto { punto: (i + 1).to_string(), este: p[0], norte: p[1], cota: p[2], descripcion: String::new() })
        .collect();
    dxf::plano_topo(&rs("topo_plano.dxf"), &pts, Some(&tin), &curvas, 2.5, 5, Some(&eje), 1.0).unwrap();
    let pf = perfil_longitudinal(&tin, &eje, 20.0).unwrap();
    dxf::perfil_topo(&rs("topo_perfil.dxf"), &pf, 10.0).unwrap();
    escribir_excel(&rs("topo_perfil.xlsx"), &tablas_perfil(&pf, &[])).unwrap();
    pdf::perfil_pdf(&rs("topo_perfil.pdf"), &pf, None, None, "PERFIL LONGITUDINAL").unwrap();
    comparar_dxf("topo_plano.dxf");
    comparar_dxf("topo_perfil.dxf");
    comparar_excel("topo_perfil.xlsx", 0.0);
}

#[test]
fn gridcraft() {
    let red = red::cargar(&raiz().join("examples").join("gridcraft").join("red.json")).unwrap();
    let r = red::calcular(&red).unwrap();
    escribir_excel(&rs("red_planilla.xlsx"), &tablas_red(&r)).unwrap();
    assert!(dxf::plano_red(&rs("red_plano.dxf"), &red, &r, 2.0).unwrap().is_some());
    comparar_excel("red_planilla.xlsx", 0.0);
    comparar_dxf("red_plano.dxf");
}

#[test]
fn presucraft() {
    let p = presupuesto::cargar(&raiz().join("examples").join("presucraft").join("presupuesto.json")).unwrap();
    let r = presupuesto::calcular(&p).unwrap();
    escribir_excel(&rs("presu.xlsx"), &tablas_presupuesto(&r)).unwrap();
    comparar_excel("presu.xlsx", 0.01 + 1e-9);
}
