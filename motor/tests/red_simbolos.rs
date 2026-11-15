//! Simbología de GridCraft en planta: retenidas, pastorales, luminarias y tipo de poste
//! (`red::simbolos_red`) y su dibujo en el DXF (`dxf::simbologia_red`).

use suite_motor::dxf::{simbologia_red, Dxf};
use suite_motor::red::{leer_tablas, simbolos_red, Red};

fn red() -> Red {
    // SED(0,0) → 01(30,0) → 02(60,0) → 03(60,30): 01 en alineamiento, 02 en ángulo de 90°,
    // 03 terminal
    let nodos = "nodo,padre,longitud_m,conductor,lotes,kw_ce,lamparas,armado,soporte,n_soportes,pat,caja,pastoral,retenida,n_retenidas,conectores,acometidas,este,norte
SED,,,,0,0,0,,,,,,,,,,,0,0
01,SED,30,3x16,1,0,1,E1,8/200,1,,,AG,,,,,30,0
02,01,30,3x16,1,0,0,E6,8/300,2,,,,RI-BT,1,,,60,0
03,02,30,3x16,1,0,2,E5,8/200,1,,,AG,RI-BT,2,,,60,30
";
    let (n, c) = leer_tablas(nodos, "formacion,k_ohm_km,r_ohm_km\n3x16,3.553,1.91\n").unwrap();
    Red {
        nombre: "s".into(),
        sistema: "3F".into(),
        tension_v: 380.0,
        nodos: n,
        conductores: c,
        calificacion_kw_lote: 0.0,
        fs_sp: 1.0,
        fp_sp: 1.0,
        fs_ce: 1.0,
        kw_lampara: 0.0,
        fs_ap: 1.0,
        fp_ap: 1.0,
        tension_ap_v: None,
        tension_fn_v: None,
        caida_max_pct: 0.0,
        perdidas_max_pct: 0.0,
        factor_carga: 0.0,
        transformador_kva: 0.0,
        factor_sobrecarga: 1.0,
        tabla_fs: Vec::new(),
        fuentes: Default::default(),
    }
}

fn cerca(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

#[test]
fn geometria_de_los_simbolos() {
    let r = red();
    assert!(r.nodos.iter().all(|n| n.este.is_some()), "el CSV debe traer coordenadas");
    let s = simbolos_red(&r, 1.0);
    assert_eq!(s.sed, Some([0.0, 0.0]));
    // 02 en ángulo (vecinos hacia -x y +y): retenida hacia afuera del ángulo, (1,-1)/√2 · 3
    let k = 3.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert_eq!(s.retenidas.len(), 3);
    assert!(cerca(s.retenidas[0][0], [60.0, 0.0]) && cerca(s.retenidas[0][1], [60.0 + k, -k]));
    // 03 terminal (vecino hacia -y): dos retenidas en prolongación del vano, ±12,5°
    for (ret, signo) in s.retenidas[1..].iter().zip([-1.0, 1.0]) {
        let (dx, dy) = (ret[1][0] - 60.0, ret[1][1] - 30.0);
        assert!((dx.hypot(dy) - 3.0).abs() < 1e-9);
        assert!((dx.atan2(dy).to_degrees() + signo * 12.5).abs() < 1e-9, "{dx} {dy}");
    }
    // 01 en alineamiento: pastoral perpendicular al vano; 03: opuesto a la retenida
    assert_eq!(s.pastorales.len(), 2);
    assert!(cerca(s.pastorales[0][1], [30.0, -1.5]));
    assert!(cerca(s.pastorales[1][1], [60.0, 28.5]));
    assert_eq!(s.luminarias.iter().map(|l| l.1).collect::<Vec<_>>(), vec![1.0, 2.0]);
    let tipos: Vec<&str> = s.tipos.iter().map(|t| t.1.as_str()).collect();
    assert_eq!(tipos, vec!["8/200", "2x8/300", "8/200"]);
}

#[test]
fn dxf_con_capas_y_leyenda() {
    let r = red();
    let mut d = Dxf::nuevo();
    d.linea("RED", [0.0, 0.0], [60.0, 30.0]);
    simbologia_red(&mut d, &r, 2.0);
    let t = d.contenido();
    for capa in ["SIMB_SED", "SIMB_RETENIDA", "SIMB_ALUMBRADO", "SIMB_POSTE", "LEYENDA"] {
        assert!(t.contains(capa), "falta la capa {capa}");
    }
    for txt in ["LEYENDA", "Retenida y anclaje", "2x8/300", "2L"] {
        assert!(t.contains(txt), "falta el texto {txt}");
    }
}
