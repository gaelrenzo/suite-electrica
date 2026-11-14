//! PresuCraft con APU y fórmula polinómica: el montaje toma el precio de su APU y la fórmula
//! reparte suministro, montaje (por insumo), transporte y GG+U por índice unificado.

use std::path::PathBuf;

use suite_motor::presupuesto::{cargar, calcular};

#[test]
fn presupuesto_con_apu_y_formula() {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("presu_apu");
    std::fs::create_dir_all(&d).unwrap();
    let w = |n: &str, t: &str| std::fs::write(d.join(n), t).unwrap();
    w("presupuesto.json", r#"{"nombre":"p","suministros":"s.csv","unidades":"u.csv","montaje":"m.csv","metrado":"me.csv",
        "apu":"apu.csv","fp_agrupacion":"fp.csv","flete_por_kg":0.5,"gastos_generales_pct":10,"utilidad_pct":5,"igv_pct":18,
        "fuentes":{"flete_por_kg":"prueba","gastos_generales_pct":"prueba","utilidad_pct":"prueba","igv_pct":"prueba"}}"#);
    w("s.csv", "codigo,descripcion,unidad,precio,peso_kg,fuente,iu\nP8,Poste 8 m,und,400,500,prueba,62\nCAB,Cable,m,10,0.2,prueba,7\n");
    w("u.csv", "uc,suministro,cantidad\nPOSTE,P8,1\nRED,CAB,1000\n");
    w("m.csv", "uc,descripcion,unidad,precio,fuente\nPOSTE,Izaje de poste,und,999,prueba\nRED,Tendido,km,500,prueba\n");
    w("me.csv", "rubro,uc,cantidad\nRS,POSTE,10\nRS,RED,2\n");
    // izaje: rendimiento 4 und/día; MO 0,25·8/4 = 0,5 h -> 0,5·50 = 25 y 2·8/4 = 4 h -> 4·20 = 80; herramientas 5 % de 105
    w("apu.csv", "partida,descripcion,unidad,rendimiento,tipo,insumo,unidad_insumo,cuadrilla,cantidad,precio,iu,base
POSTE,Izaje de poste,und,4,MO,Capataz,h-h,0.25,,50,47,
POSTE,,,,MO,Peón,h-h,2,,20,47,
POSTE,,,,EQ,Herramientas,%mo,,5,,37,
POSTE,,,,MAQ,Camión grúa,h-m,0.5,,200,49,
");
    w("fp.csv", "iu,similar,monomio\n37,47,\n47,,J\n49,,E\n32,,E\n62,,P\n7,,AL\n39,,GU\n");
    let p = cargar(&d.join("presupuesto.json")).unwrap();
    let r = calcular(&p).unwrap();
    let izaje = r.montaje.iter().find(|m| m.uc == "POSTE").unwrap();
    // 25 + 80 + 5,25 + 1·200 = 310,25
    assert_eq!(izaje.precio, Some(310.25));
    assert!(r.avisos.iter().any(|a| a.contains("999.00 reemplazado por su APU 310.25")), "{:?}", r.avisos);
    let f = r.formula.as_ref().unwrap();
    let suma: f64 = f.monomios.iter().map(|m| m.coeficiente).sum();
    assert!((suma - 1.0).abs() < 1e-9, "suma {suma}");
    let total = r.resumen.last().unwrap();
    // el total de la fórmula es el costo sin IGV: suministro + montaje + transporte + GG + U
    let base = total.suministro + total.montaje + total.transporte + total.gastos_generales + total.utilidad;
    assert!((f.total - base).abs() < 0.02, "{} vs {base}", f.total);
    // montaje sin APU (RED) queda con índice desconocido y aviso
    assert!(r.avisos.iter().any(|a| a.contains("montaje sin APU") && a.contains("RED")));
    let hojas: Vec<String> = suite_motor::salidas::tablas_presupuesto(&r).iter().map(|t| t.nombre.clone()).collect();
    assert!(hojas.contains(&"APU".to_string()) && hojas.contains(&"FormulaPolinomica".to_string()));
}
