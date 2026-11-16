//! Transporte por rutas (PresuCraft): el flete por kg sale de los tramos (distancia × costo por
//! t·km / 1000) y reemplaza al `flete_por_kg` fijo del JSON.

use std::path::PathBuf;

use suite_motor::presupuesto::{calcular, cargar, flete_ruta, leer_transporte_csv};

#[test]
fn flete_desde_la_ruta() {
    let t = leer_transporte_csv(
        "# ejemplo\ntramo,via,distancia_km,costo_t_km,fuente\nLima-Juliaca,asfaltada,1300,0.25,COTIZACION\nJuliaca-Capachica,afirmada,\"60,5\",0.8,\nCapachica-obra,acémila,2,12,PENDIENTE\n",
    )
    .unwrap();
    assert_eq!(t.len(), 3);
    assert_eq!(t[1].distancia_km, 60.5);
    // (1300·0.25 + 60.5·0.8 + 2·12) / 1000 = (325 + 48.4 + 24) / 1000
    assert!((flete_ruta(&t) - 0.3974).abs() < 1e-12);
    assert!(leer_transporte_csv("tramo,via,distancia_km,costo_t_km,fuente\nA,x,diez,1,\n").unwrap_err().contains("diez"));

    let ej = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("examples").join("presucraft");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("presu_transporte");
    std::fs::create_dir_all(&dir).unwrap();
    for f in std::fs::read_dir(&ej).unwrap() {
        let f = f.unwrap().path();
        std::fs::copy(&f, dir.join(f.file_name().unwrap())).unwrap();
    }
    let base = calcular(&cargar(&dir.join("presupuesto.json")).unwrap()).unwrap();
    let mut cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("presupuesto.json")).unwrap()).unwrap();
    cfg["transporte"] = "transporte.csv".into();
    std::fs::write(dir.join("presupuesto.json"), cfg.to_string()).unwrap();
    std::fs::write(dir.join("transporte.csv"), "tramo,via,distancia_km,costo_t_km,fuente\nRuta,asfaltada,1000,0.6,EJEMPLO\n").unwrap();
    let p = cargar(&dir.join("presupuesto.json")).unwrap();
    assert!((p.flete_por_kg - 0.6).abs() < 1e-12);
    let r = calcular(&p).unwrap();
    let peso: f64 = r.suministro.iter().map(|s| s.peso_kg).sum();
    let tr = |x: &suite_motor::presupuesto::ResultadoPresupuesto| x.resumen.iter().find(|f| f.rubro == "TOTAL").unwrap().transporte;
    assert!((tr(&r) - peso * 0.6).abs() < 1e-6, "transporte = peso × flete de la ruta");
    assert!(tr(&r) != tr(&base));
    assert!(r.avisos.iter().any(|a| a.contains("tramo 'Ruta'")));
    assert!(!r.avisos.iter().any(|a| a.contains("'flete_por_kg'")), "con ruta no se avisa del flete fijo");
    assert!(suite_motor::salidas::tablas_presupuesto(&r).iter().any(|t| t.nombre == "Transporte"));
}

/// Varias rutas (como la hoja «Flete T» de PRESIO): cada suministro paga el flete de su origen.
#[test]
fn flete_por_origen_del_suministro() {
    use suite_motor::presupuesto::{calcular, fletes, ComponenteUc, ItemMetrado, Presupuesto, Suministro};
    let t = leer_transporte_csv(
        "ruta,tramo,via,distancia_km,costo_t_km,precio_kg,fuente\nLima,,,,,0.26,PRESIO\nJuliaca,carretera,asfaltada,40,0.5,,X\nJuliaca,trocha,afirmada,10,2,,X\n",
    )
    .unwrap();
    let f = fletes(&t);
    assert!((f["Lima"] - 0.26).abs() < 1e-12 && (f["Juliaca"] - 0.04).abs() < 1e-12);
    let su = |c: &str, peso: f64, ruta: &str| Suministro { codigo: c.into(), peso_kg: Some(peso), precio: Some(0.0), ruta: ruta.into(), ..Default::default() };
    let mut p = Presupuesto {
        suministros: vec![su("CUT", 13.8, "Lima"), su("POSTE", 1000.0, "Juliaca")],
        unidades: vec![
            ComponenteUc { uc: "U1".into(), suministro: "CUT".into(), cantidad: Some(1.0) },
            ComponenteUc { uc: "U1".into(), suministro: "POSTE".into(), cantidad: Some(1.0) },
        ],
        metrado: vec![ItemMetrado { rubro: "LP".into(), uc: "U1".into(), cantidad: Some(2.0) }],
        transporte: t,
        ..Default::default()
    };
    let r = calcular(&p).unwrap();
    let tr = |c: &str| r.suministro.iter().find(|x| x.codigo == c).unwrap().transporte;
    assert!((tr("CUT") - 2.0 * 3.588).abs() < 1e-9, "13,8 kg × 0,26 = 3,588 por unidad (como PRESIO)");
    assert!((tr("POSTE") - 2.0 * 40.0).abs() < 1e-9);
    assert!((r.planilla_uc[0].transporte_unit - 43.59).abs() < 1e-9, "por UC: 3,588 + 40");
    // una ruta que no existe, o varias rutas y un suministro sin ruta: error claro
    p.suministros[0].ruta = "Cusco".into();
    assert!(calcular(&p).unwrap_err().contains("Cusco"));
    p.suministros[0].ruta.clear();
    assert!(calcular(&p).unwrap_err().contains("varias rutas"));
}
