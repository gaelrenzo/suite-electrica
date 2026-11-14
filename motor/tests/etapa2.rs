//! Etapa 2: GridCraft (red radial) y PresuCraft (presupuesto) — Rust vs Python.

mod comun;

use std::collections::HashMap;

use comun::n;
use suite_motor::presupuesto::{self, Presupuesto};
use suite_motor::red::{self, Red};

/// (parámetros, fuentes, tablas csv, resultados).
type Partes = (HashMap<String, String>, HashMap<String, String>, HashMap<String, String>, Vec<Vec<String>>, Vec<(f64, f64)>);

fn partes(texto: &str) -> Partes {
    let (mut par, mut fue, mut csv, mut res, mut tfs) = (HashMap::new(), HashMap::new(), HashMap::<String, String>::new(), Vec::new(), Vec::new());
    let mut en_res = false;
    for l in texto.lines() {
        if l.starts_with("== resultados") {
            en_res = true;
            continue;
        }
        if en_res {
            res.push(l.split_whitespace().map(String::from).collect());
            continue;
        }
        let (k, resto) = l.split_once(' ').unwrap_or((l, ""));
        match k {
            "param" => {
                let (a, b) = resto.split_once(' ').unwrap();
                par.insert(a.into(), b.into());
            }
            "fuente" => {
                let (a, b) = resto.split_once(' ').unwrap();
                fue.insert(a.into(), b.replace('_', " "));
            }
            "tabla_fs" => {
                let t: Vec<f64> = resto.split_whitespace().map(n).collect();
                tfs.push((t[0], t[1]));
            }
            "csv" => {
                let (a, b) = resto.split_once(' ').unwrap();
                let e = csv.entry(a.into()).or_default();
                e.push_str(b);
                e.push('\n');
            }
            _ => panic!("clave {k}"),
        }
    }
    (par, fue, csv, res, tfs)
}

fn cerca(o: f64, e: f64, tol: f64, etq: &str) {
    if e.is_nan() {
        assert!(o.is_nan(), "{etq}: rust {o} python nan");
        return;
    }
    assert!((o - e).abs() <= tol * e.abs().max(1.0), "{etq}: rust {o} python {e}");
}

fn comparar_red(texto: &str) {
    let (par, fue, csv, res, tfs) = partes(texto);
    let (nodos, conductores) = red::leer_tablas(&csv["nodos"], &csv["conductores"]).unwrap();
    let g = |k: &str| n(&par[k]);
    let r = Red {
        nombre: "Red secundaria - ejemplo".into(),
        sistema: par["sistema"].clone(),
        tension_v: g("tension_V"),
        nodos,
        conductores,
        calificacion_kw_lote: g("calificacion_kw_lote"),
        fs_sp: g("fs_sp"),
        fp_sp: g("fp_sp"),
        fs_ce: g("fs_ce"),
        kw_lampara: g("kw_lampara"),
        fs_ap: g("fs_ap"),
        fp_ap: g("fp_ap"),
        tension_ap_v: Some(g("tension_ap_V")).filter(|&v| v != 0.0),
        tension_fn_v: None,
        caida_max_pct: g("caida_max_pct"),
        perdidas_max_pct: g("perdidas_max_pct"),
        factor_carga: g("factor_carga"),
        transformador_kva: g("transformador_kva"),
        factor_sobrecarga: g("factor_sobrecarga"),
        tabla_fs: tfs,
        fuentes: fue,
    };
    let out = red::calcular(&r).unwrap();
    let (mut it, mut ia) = (0, 0);
    for t in &res {
        match t[0].as_str() {
            "tramo" => {
                let x = &out.tramos[it];
                assert_eq!((x.nodo.as_str(), x.padre.as_str()), (t[1].as_str(), t[2].as_str()));
                let v = [x.longitud_m, x.p_sp_kw, x.p_acum_kw, x.k_ohm_km, x.i_a, x.dv_tramo_v, x.dv_acum_v, x.dv_pct, x.perdidas_w];
                for (k, val) in v.iter().enumerate() {
                    cerca(*val, n(&t[3 + k]), 1e-9, &format!("{} col{k}", x.nodo));
                }
                let ap = x.ap.unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
                for (k, val) in [ap.0, ap.1, ap.2, ap.3].iter().enumerate() {
                    cerca(*val, n(&t[12 + k]), 1e-9, &format!("{} ap{k}", x.nodo));
                }
                it += 1;
            }
            "resumen" => {
                let k = t[1].replace('_', " ");
                cerca(out.resumen(&k).unwrap_or_else(|| panic!("falta {k}")), n(&t[2]), 1e-9, &k);
            }
            "alerta" => {
                let a = &out.alertas[ia];
                assert_eq!((a.elemento.replace(' ', "_"), a.codigo.as_str()), (t[1].clone(), t[2].as_str()));
                cerca(a.exceso, n(&t[3]), 1e-9, "exceso");
                ia += 1;
            }
            "avisos" => assert_eq!(out.avisos.len(), t[1].parse::<usize>().unwrap(), "{:?}", out.avisos),
            _ => panic!(),
        }
    }
    assert_eq!((it, ia), (out.tramos.len(), out.alertas.len()));
}

#[test]
fn red_ejemplo() {
    comparar_red(include_str!("red_ejemplo.txt"));
}

#[test]
fn red_alertas() {
    comparar_red(include_str!("red_alertas.txt"));
}

fn comparar_presu(texto: &str) {
    let (par, fue, csv, _, _) = partes(texto);
    let (suministros, unidades, montaje, metrado) =
        presupuesto::leer_tablas(&csv["suministros"], &csv["unidades"], &csv["montaje"], &csv["metrado"]).unwrap();
    let p = Presupuesto {
        nombre: "p".into(),
        suministros,
        unidades,
        montaje,
        metrado,
        flete_por_kg: n(&par["flete_por_kg"]),
        gastos_generales_pct: n(&par["gastos_generales_pct"]),
        utilidad_pct: n(&par["utilidad_pct"]),
        igv_pct: n(&par["igv_pct"]),
        moneda: "S/".into(),
        fuentes: fue,
    };
    let out = presupuesto::calcular(&p).unwrap();
    let cuerpo: Vec<&str> = texto.lines().skip_while(|l| !l.starts_with("== resultados")).skip(1).collect();
    let (mut ir, mut is, mut iu) = (0, 0, 0);
    let mut avisos = Vec::new();
    for l in cuerpo {
        let t: Vec<&str> = l.split_whitespace().collect();
        match t[0] {
            "resumen" => {
                let x = &out.resumen[ir];
                assert_eq!(x.rubro, t[1]);
                let v = [x.suministro, x.montaje, x.transporte, x.costo_directo, x.gastos_generales, x.utilidad, x.subtotal, x.igv, x.total];
                // montos redondeados al céntimo: pandas suma por pares y Rust en orden, y un
                // total que cae en medio céntimo puede redondear distinto (±0,01)
                for (k, val) in v.iter().enumerate() {
                    let e = n(t[2 + k]);
                    assert!((val - e).abs() <= 0.0101, "{} col{k}: rust {val} python {e}", x.rubro);
                }
                ir += 1;
            }
            "sum" => {
                let x = &out.suministro[is];
                assert_eq!((x.rubro.as_str(), x.codigo.as_str()), (t[1], t[2]));
                for (k, val) in [x.cantidad, x.parcial, x.peso_kg, x.transporte].iter().enumerate() {
                    cerca(*val, n(t[3 + k]), 1e-9, &format!("{} {k}", x.codigo));
                }
                is += 1;
            }
            "uc" => {
                let q: Vec<&str> = l[3..].split('|').collect();
                let x = &out.planilla_uc[iu];
                assert_eq!((x.rubro.as_str(), x.uc.as_str()), (q[0], q[1]));
                for (val, e, etq) in [(x.suministro_unit, q[2], "s"), (x.montaje_unit, q[3], "m"), (x.total, q[4], "total")] {
                    let e = n(e);
                    assert!((val - e).abs() <= 0.0101, "{} {etq}: rust {val} python {e}", x.uc); // ±1 céntimo
                }
                iu += 1;
            }
            "aviso" => avisos.push(l[6..].to_string()),
            _ => panic!(),
        }
    }
    assert_eq!((ir, is, iu), (out.resumen.len(), out.suministro.len(), out.planilla_uc.len()));
    assert_eq!(out.avisos, avisos);
}

#[test]
fn presu_ejemplo() {
    comparar_presu(include_str!("presu_ejemplo.txt"));
}

#[test]
fn presu_variante() {
    comparar_presu(include_str!("presu_variante.txt"));
}
