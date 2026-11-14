//! Metrado de GridCraft (criterio de REDCAD): armados con multiplicador y suma, soportes, PAT,
//! retenidas, cajas, pastorales, iluminación, conectores, acometidas y conductor en km.
//! Validado contra el «Metrado de Redes Secundarias» de REDCAD con `comparar-redcad --metrado`.

use suite_motor::red::{leer_tablas, metrado, metrado_presupuesto, partes_armado};

#[test]
fn metrado_de_estructuras_y_conductor() {
    assert_eq!(partes_armado("2E3/S"), vec![(2.0, "E3/S".to_string())]);
    assert_eq!(partes_armado("E3+E3/S"), vec![(1.0, "E3".to_string()), (1.0, "E3/S".to_string())]);
    assert_eq!(partes_armado("2E'4/S"), vec![(2.0, "E'4/S".to_string())]);
    let nodos = "nodo,padre,longitud_m,conductor,lotes,kw_ce,lamparas,armado,soporte,n_soportes,pat,caja,pastoral,retenida,n_retenidas,conectores,acometidas
SED,,,,0,0,0,,,,,,,,,,
01,SED,31.62,3x16+1x16/25,2,0,1,E1,8/200,1,,C380-220/6S,AG,RI-BT,1,,C;C3
02,01,29.34,3x16+1x16/25,0,0,0,E6/S,8/200,1,PAT-1,,,RI-BT,2,CFF35-16:3:SP;CFF35-16:1:AP,
03,02,93.59,2x16/25,1,0,0,2E3/S,8/200,1,,,,,,,LM;X
";
    let cond = "formacion,k_ohm_km,r_ohm_km\n3x16+1x16/25,3.553,1.91\n2x16/25,4.098,1.91\n";
    let (n, _) = leer_tablas(nodos, cond).unwrap();
    let red = suite_motor::red::Red {
        nombre: "m".into(),
        sistema: "3F".into(),
        tension_v: 380.0,
        nodos: n,
        conductores: Vec::new(),
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
    };
    let m = metrado(&red);
    let q = |c: &str| m.iter().find(|x| x.codigo == c).map(|x| x.cantidad).unwrap_or(0.0);
    for (c, v) in [
        ("E1", 1.0), ("E6/S", 1.0), ("E3/S", 2.0), ("8/200", 3.0), ("PAT-1", 1.0), ("RI-BT", 3.0), ("C380-220/6S", 1.0), ("AG", 1.0),
        ("Puntos de iluminación", 1.0), ("CFF35-16-SP", 3.0), ("CFF35-16-AP", 1.0), ("Acometidas Cortas", 2.0),
        ("Acometidas Largas", 1.0), ("Acometidas Extra Largas", 1.0), ("Acometidas 1φ", 3.0), ("Acometidas 3φ", 1.0),
        ("Muretes en acometidas largas", 1.0),
    ] {
        assert_eq!(q(c), v, "{c}");
    }
    assert!((q("3x16+1x16/25") - 0.06096).abs() < 1e-12);
    assert!((q("2x16/25") - 0.09359).abs() < 1e-12);
    let csv = metrado_presupuesto(&m, "RS");
    assert!(csv.starts_with("rubro,uc,cantidad\n") && csv.contains("RS,E3/S,2\n"));
}
