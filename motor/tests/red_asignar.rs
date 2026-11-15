//! Asignar la misma estructura a varios postes de GridCraft (`red::asignar_estructura`).

use suite_motor::red::{asignar_estructura, leer_tablas, metrado, CampoEstructura, EstructuraBt, Red};

fn red() -> Red {
    let nodos = "nodo,padre,longitud_m,conductor,lotes,kw_ce,lamparas,armado,soporte,n_soportes,pat,caja,pastoral,retenida,n_retenidas,conectores,acometidas
SED,,,,0,0,0,,,,,,,,,,
01,SED,30,3x16,1,0,0,E1,8/200,1,PAT-1,,,,,,C
02,01,30,3x16,1,0,0,,,,,,,,,,
03,02,30,3x16,1,0,0,E6/S,,,,,,RI-BT,2,,L
";
    let (n, c) = leer_tablas(nodos, "formacion,k_ohm_km,r_ohm_km\n3x16,3.553,1.91\n").unwrap();
    Red {
        nombre: "a".into(),
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

fn cuenta(r: &Red, codigo: &str) -> f64 {
    metrado(r).iter().filter(|f| f.codigo == codigo).map(|f| f.cantidad).sum()
}

#[test]
fn copia_solo_los_campos_elegidos() {
    let mut r = red();
    let modelo = EstructuraBt {
        armado: "E3/S".into(),
        soporte: "9/300".into(),
        n_soportes: 1.0,
        pat: "PAT-X".into(),
        retenida: "RV-BT".into(),
        n_retenidas: 1.0,
        acometidas: vec!["X".into()],
        ..Default::default()
    };
    let n = asignar_estructura(&mut r, &[1, 2, 3, 99], &modelo, &[CampoEstructura::Armado, CampoEstructura::Soporte]);
    assert_eq!(n, 3);
    for i in 1..=3 {
        assert_eq!(r.nodos[i].estructura.armado, "E3/S");
        assert_eq!(r.nodos[i].estructura.soporte, "9/300");
        assert_eq!(r.nodos[i].estructura.n_soportes, 1.0);
    }
    // lo no elegido queda igual
    assert_eq!(r.nodos[1].estructura.pat, "PAT-1");
    assert_eq!(r.nodos[1].estructura.acometidas, vec!["C".to_string()]);
    assert_eq!(r.nodos[3].estructura.retenida, "RI-BT");
    assert_eq!(r.nodos[3].estructura.n_retenidas, 2.0);
    // la SED no estaba en la lista
    assert_eq!(r.nodos[0].estructura, EstructuraBt::default());
    assert_eq!(cuenta(&r, "E3/S"), 3.0);
    assert_eq!(cuenta(&r, "E1"), 0.0);
}

#[test]
fn retenida_con_su_numero_y_sin_cambios_no_cuenta() {
    let mut r = red();
    let modelo = EstructuraBt { retenida: "RI-BT".into(), n_retenidas: 2.0, ..Default::default() };
    // 03 ya tiene RI-BT x2: no cambia
    assert_eq!(asignar_estructura(&mut r, &[2, 3], &modelo, &[CampoEstructura::Retenida]), 1);
    assert_eq!(r.nodos[2].estructura.retenida, "RI-BT");
    assert_eq!(r.nodos[2].estructura.n_retenidas, 2.0);
    // modelo vacío borra el campo
    assert_eq!(asignar_estructura(&mut r, &[1], &EstructuraBt::default(), &[CampoEstructura::Pat]), 1);
    assert!(r.nodos[1].estructura.pat.is_empty());
    assert_eq!(asignar_estructura(&mut r, &[1], &modelo, &[]), 0);
}
