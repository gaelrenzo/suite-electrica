//! Salidas tabulares (planillas Excel) de LineCraft, TopoCraft, GridCraft y PresuCraft.
//!
//! Port de `linecraft/salidas.py` (`tablas`, `planilla_excel`), `topocraft/salidas.py`
//! (`perfil_excel`), `gridcraft/cli.py` (`planilla_excel`) y `presucraft/presupuesto.py`
//! (`excel`): mismas hojas, columnas (con unidades) y redondeos. Los planos DXF están en
//! `dxf.rs` y las memorias PDF en `pdf.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::alertas::Alerta;
use crate::catalogos::{FilaMetrado, FilaVerificacion};
use crate::flechado::FilaFlechado;
use crate::geo::{Perfil, QUIEBRE};
use crate::linea::{linea_conductor, Proyecto, Resultado};
use crate::numerico::{formato_g, redondear};
use crate::presupuesto::ResultadoPresupuesto;
use crate::red::ResultadoRed;

/// Celda de una tabla (`Num(NaN)` = vacía, como `NaN`/`None` en pandas).
#[derive(Clone, Debug, PartialEq)]
pub enum Celda {
    Num(f64),
    /// Columna entera de pandas (`int64`): se muestra sin decimales.
    Ent(i64),
    Txt(String),
}

impl Celda {
    /// Texto como lo muestra la memoria PDF (`str(x)` de Python, vacío si NaN).
    pub fn texto(&self) -> String {
        match self {
            Celda::Txt(t) => t.clone(),
            Celda::Num(x) if x.is_nan() => String::new(),
            Celda::Num(x) => texto_python(*x),
            Celda::Ent(i) => i.to_string(),
        }
    }
}

/// `str(float)` de Python para valores redondeados (`12.0`, `0.125`, `-3.5`).
pub fn texto_python(x: f64) -> String {
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let s = format!("{x}");
    if x.abs() >= 1e16 || (x != 0.0 && x.abs() < 1e-4) {
        return format!("{x:e}");
    }
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn n(x: f64) -> Celda {
    Celda::Num(x)
}

fn rd(x: f64, d: usize) -> Celda {
    Celda::Num(if x.is_finite() { redondear(x, d) } else { x })
}

fn t(s: impl Into<String>) -> Celda {
    Celda::Txt(s.into())
}

fn opt(x: Option<f64>, d: usize) -> Celda {
    x.map_or(n(f64::NAN), |v| rd(v, d))
}

/// Entero como lo guarda pandas (columna `int64`).
fn ent(x: usize) -> Celda {
    Celda::Ent(x as i64)
}

/// Hoja: nombre, encabezados y filas.
#[derive(Clone, Debug, PartialEq)]
pub struct Tabla {
    pub nombre: String,
    pub columnas: Vec<String>,
    pub filas: Vec<Vec<Celda>>,
}

impl Tabla {
    fn nueva(nombre: &str, columnas: &[&str]) -> Self {
        Tabla { nombre: nombre.into(), columnas: columnas.iter().map(|c| c.to_string()).collect(), filas: Vec::new() }
    }

    /// Tabla de una columna con un mensaje si no hay filas (como `["Sin avisos"]`).
    fn lista(nombre: &str, columna: &str, items: &[String], vacio: &str) -> Self {
        let mut tb = Tabla::nueva(nombre, &[columna]);
        if items.is_empty() {
            tb.filas.push(vec![t(vacio)]);
        } else {
            tb.filas.extend(items.iter().map(|a| vec![t(a.clone())]));
        }
        tb
    }

    /// Índice de una columna por nombre.
    pub fn col(&self, nombre: &str) -> Option<usize> {
        self.columnas.iter().position(|c| c == nombre)
    }
}

/// Progresiva `km+mmm.dd` (como `core.units.formato_progresiva`).
pub fn formato_progresiva(s_m: f64, decimales: usize) -> String {
    let s = redondear(s_m.max(0.0), decimales);
    let mut km = (s / 1000.0).floor();
    let mut resto = redondear(s - km * 1000.0, decimales);
    if resto >= 1000.0 {
        km += 1.0;
        resto = redondear(resto - 1000.0, decimales);
    }
    let ancho = 3 + if decimales > 0 { decimales + 1 } else { 0 };
    format!("{}+{:0ancho$.decimales$}", km as i64, resto)
}

/// Cota de comparación bajo el terreno mínimo, múltiplo de 5 m (`cota_referencia`).
pub fn cota_referencia(p: &Perfil) -> f64 {
    let zmin = p.cota.iter().copied().filter(|z| !z.is_nan()).fold(f64::INFINITY, f64::min);
    ((zmin - 5.0) / 5.0).floor() * 5.0
}

/// Tramos del terreno sin NaN con al menos 2 puntos (`_tramos_validos`).
pub fn tramos_validos(s: &[f64], z: &[f64]) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    let mut actual = Vec::new();
    for (&si, &zi) in s.iter().zip(z) {
        if zi.is_nan() {
            if !actual.is_empty() {
                out.push(std::mem::take(&mut actual));
            }
        } else {
            actual.push([si, zi]);
        }
    }
    if !actual.is_empty() {
        out.push(actual);
    }
    out.retain(|t| t.len() >= 2);
    out
}

/// Progresivas, cotas y tipos de los puntos principales (sin quiebres del TIN).
pub fn principales(p: &Perfil) -> Vec<(f64, f64, char)> {
    (0..p.progresiva.len()).filter(|&i| p.tipo[i] != QUIEBRE).map(|i| (p.progresiva[i], p.cota[i], p.tipo[i])).collect()
}

/// Conductor en la hipótesis de flecha máxima por vano: (s, z − desplazar), paso 2 m.
pub fn curvas_vanos(r: &Resultado, p: &Proyecto, desplazar: f64) -> Vec<(Vec<f64>, Vec<f64>)> {
    r.vanos
        .iter()
        .map(|v| {
            let (ea, eb) = (&r.estructuras[v.a], &r.estructuras[v.a + 1]);
            let e = r.tramos[v.tramo].estado(&p.hip_flecha_maxima).expect("hipótesis de flecha máxima");
            let (s, zc, _) = linea_conductor(&p.perfil, ea.progresiva, eb.progresiva, ea.cota_amarre(), eb.cota_amarre(),
                e.h, e.w_resultante, p.modelo, e.w_vertical / e.w_resultante, 2.0);
            (s, zc.into_iter().map(|z| z - desplazar).collect())
        })
        .collect()
}

/// `Resultado.cumple` de Python: vanos, tramos, verificación y alertas.
pub fn cumple(r: &Resultado, ver: Option<&[FilaVerificacion]>, alertas: Option<&[Alerta]>) -> bool {
    r.vanos.iter().all(|v| v.cumple)
        && r.tramos.iter().all(|t| t.estados.iter().all(|e| e.cumple))
        && ver.map_or(true, |v| v.iter().all(|f| f.cumple))
        && alertas.map_or(true, |a| a.is_empty())
}

/// Datos opcionales del cálculo completo (catálogos, flechado y alertas).
#[derive(Clone, Copy, Default)]
pub struct Extras<'a> {
    pub verificacion: Option<&'a [FilaVerificacion]>,
    pub metrado: Option<&'a [FilaMetrado]>,
    pub flechado: &'a [FilaFlechado],
    pub alertas: Option<&'a [Alerta]>,
}

/// Tablas de resultados de LineCraft (`linecraft.salidas.tablas`), en el orden de las hojas.
pub fn tablas_linea(r: &Resultado, p: &Proyecto, x: &Extras) -> Vec<Tabla> {
    let est = &r.estructuras;
    let mut te = Tabla::nueva("Estructuras", &[
        "Estructura", "Tipo", "Progresiva", "Distancia (m)", "Este (m)", "Norte (m)", "Cota terreno (m)",
        "Altura amarre (m)", "Cota amarre (m)", "Deflexión (°)", "Armado", "Poste", "Vano atrás (m)", "Vano adelante (m)",
    ]);
    for (i, e) in est.iter().enumerate() {
        te.filas.push(vec![
            t(e.codigo()), t(e.tipo.como_str()), t(formato_progresiva(e.progresiva, 2)), rd(e.progresiva, 2), rd(e.este, 3),
            rd(e.norte, 3), rd(e.cota_terreno, 2), rd(e.altura_amarre, 2), rd(e.cota_amarre(), 2), rd(e.angulo, 2),
            t(e.armado.clone()), t(e.poste.clone()),
            if i > 0 { rd(r.vanos[i - 1].longitud, 2) } else { n(f64::NAN) },
            if i < r.vanos.len() { rd(r.vanos[i].longitud, 2) } else { n(f64::NAN) },
        ]);
    }

    let hf = &p.hip_flecha_maxima;
    let mut cols_v: Vec<String> = ["Vano", "Longitud (m)", "Desnivel amarres (m)"].iter().map(|s| s.to_string()).collect();
    if let Some(v) = r.vanos.first() {
        cols_v.extend(v.flechas.iter().map(|(k, _)| format!("Flecha {k} (m)")));
    }
    cols_v.push(format!("Flecha medio vano {hf} (m)"));
    cols_v.extend(["Despeje mín. (m)", "En progresiva", "Exigido (m)", "Cumple"].iter().map(|s| s.to_string()));
    let mut tv = Tabla { nombre: "Vanos".into(), columnas: cols_v, filas: Vec::new() };
    for v in &r.vanos {
        let mut f = vec![t(format!("{} - {}", est[v.a].codigo(), est[v.a + 1].codigo())), rd(v.longitud, 2), rd(v.desnivel, 2)];
        f.extend(v.flechas.iter().map(|(_, x)| rd(*x, 3)));
        f.extend([rd(v.flecha_medio, 3), rd(v.despeje, 2), t(formato_progresiva(v.s_despeje, 2)), n(p.distancia_minima_terreno),
            t(if v.cumple { "SÍ" } else { "NO" })]);
        tv.filas.push(f);
    }

    let mut tm = Tabla::nueva("Mecanico", &[
        "Tramo", "Vano regulador (m)", "Hipótesis", "Temp. (°C)", "w vertical (N/m)", "w viento (N/m)",
        "w resultante (N/m)", "Tiro horizontal (N)", "Tiro horizontal (kgf)", "Tiro máx. soporte (N)", "% rotura",
        "Límite (%)", "Cumple",
    ]);
    for tr in &r.tramos {
        for e in &tr.estados {
            tm.filas.push(vec![
                t(format!("{} - {}", est[tr.inicio].codigo(), est[tr.fin].codigo())), rd(tr.vano_regulador, 2),
                t(e.hipotesis.clone()), n(e.temperatura), rd(e.w_vertical, 4), rd(e.w_viento, 4), rd(e.w_resultante, 4),
                rd(e.h, 1), rd(e.h / crate::conductor::G, 2), rd(e.tiro_maximo, 1), rd(e.pct_rotura, 2), n(p.tiro_maximo_pct),
                t(if e.cumple { "SÍ" } else { "NO" }),
            ]);
        }
    }

    let codigo: HashMap<usize, String> = est.iter().map(|e| (e.numero, e.codigo())).collect();
    let mut tc = Tabla::nueva("Cargas", &[
        "Estructura", "Hipótesis", "Vano viento (m)", "Vano peso (m)", "F transversal (N)", "F longitudinal (N)",
        "F vertical (N)", "F horizontal (N)", "Momento base (N·m)", "Tiro retenida (N)",
    ]);
    for c in &r.cargas {
        tc.filas.push(vec![
            t(codigo[&c.estructura].clone()), t(c.hipotesis.clone()), rd(c.vano_viento, 2), rd(c.vano_peso, 2),
            rd(c.transversal, 1), rd(c.longitudinal, 1), rd(c.vertical, 1), rd(c.horizontal, 1), rd(c.momento_base, 1),
            rd(c.tiro_retenida, 1),
        ]);
    }

    let mut out = vec![te, tv, tm, tc];
    if let Some(ver) = x.verificacion {
        let mut tb = Tabla::nueva("Verificacion", &[
            "Estructura", "Tipo", "Armado", "Poste", "F horizontal máx. (N)", "F punta equiv. (N)", "MRN (N·m)",
            "FS poste", "Retenidas", "Tiro por retenida (N)", "FS retenida", "Cumple", "Observación",
        ]);
        let fs = |v: Option<f64>| match v {
            Some(x) if x.is_infinite() => t("—"),
            other => opt(other, 2),
        };
        for f in ver {
            tb.filas.push(vec![
                t(f.estructura.clone()), t(f.tipo.clone()), t(f.armado.clone()), t(f.poste.clone()), rd(f.f_horizontal, 1),
                opt(f.f_punta, 1), opt(f.mrn, 1), fs(f.fs_poste), ent(f.retenidas), opt(f.tiro_por_retenida, 1),
                fs(f.fs_retenida), t(if f.cumple { "SÍ" } else { "NO" }), t(f.observacion.clone()),
            ]);
        }
        out.push(tb);
    }
    if let Some(met) = x.metrado {
        let mut tb = Tabla::nueva("Metrado", &["Ítem", "Código", "Descripción", "Unidad", "Cantidad"]);
        for m in met {
            tb.filas.push(vec![t(m.item.clone()), t(m.codigo.clone()), t(m.descripcion.clone()), t(m.unidad.clone()), n(m.cantidad)]);
        }
        out.push(tb);
    }
    out.push(tabla_flechado(x.flechado));
    if let Some(al) = x.alertas {
        if al.is_empty() {
            out.push(Tabla::lista("Alertas", "Alerta", &[], "Sin alertas"));
        } else {
            let mut tb = Tabla::nueva("Alertas", &["Elemento", "Código", "Exceso (%)", "Detalle", "Crítica"]);
            for a in al {
                tb.filas.push(vec![t(a.elemento.clone()), t(a.codigo.clone()), n(a.exceso), t(a.detalle.clone()),
                    t(if a.critica { "SÍ" } else { "" })]);
            }
            out.push(tb);
        }
    }
    out.push(Tabla::lista("Avisos", "Aviso", &r.avisos, "Sin avisos"));
    out
}

fn tabla_flechado(filas: &[FilaFlechado]) -> Tabla {
    let mut cols: Vec<String> =
        ["Tramo", "Vano", "Vano regulador (m)", "Vano (m)", "Desnivel (m)", "EDS (%)"].iter().map(|s| s.to_string()).collect();
    // pandas: columnas en el orden en que aparecen (todas las filas usan las mismas temperaturas por tramo)
    let mut idx: HashMap<String, usize> = HashMap::new();
    for f in filas {
        for (tt, _, _) in &f.valores {
            for c in [format!("H {} °C (kgf)", formato_g(*tt)), format!("Flecha {} °C (m)", formato_g(*tt))] {
                if !idx.contains_key(&c) {
                    idx.insert(c.clone(), cols.len());
                    cols.push(c);
                }
            }
        }
    }
    let mut tb = Tabla { nombre: "Flechado".into(), columnas: cols, filas: Vec::new() };
    for f in filas {
        let mut fila = vec![n(f64::NAN); tb.columnas.len()];
        fila[..6].clone_from_slice(&[t(f.tramo.clone()), t(f.vano.clone()), rd(f.vano_regulador, 2), rd(f.longitud, 2),
            rd(f.desnivel, 2), n(f.eds_pct)]);
        for (tt, h, fl) in &f.valores {
            fila[idx[&format!("H {} °C (kgf)", formato_g(*tt))]] = rd(*h, 2);
            fila[idx[&format!("Flecha {} °C (m)", formato_g(*tt))]] = rd(*fl, 3);
        }
        tb.filas.push(fila);
    }
    tb
}

/// Hojas `Perfil` y `Datos` de TopoCraft (`perfil_excel`).
pub fn tablas_perfil(p: &Perfil, datos: &[(String, Celda)]) -> Vec<Tabla> {
    let mut tb = Tabla::nueva("Perfil", &["Progresiva", "Distancia (m)", "Este (m)", "Norte (m)", "Cota terreno (m)", "Tipo"]);
    for i in 0..p.progresiva.len() {
        tb.filas.push(vec![
            t(formato_progresiva(p.progresiva[i], 2)), rd(p.progresiva[i], 3), rd(p.este[i], 3), rd(p.norte[i], 3),
            rd(p.cota[i], 3), t(p.tipo[i].to_string()),
        ]);
    }
    let mut info = Tabla::nueva("Datos", &["Dato", "Valor"]);
    info.filas.push(vec![t("Longitud (m)"), rd(p.longitud(), 3)]);
    info.filas.push(vec![t("Tipo V/E/T"), t("V vértice del eje, E estación, T quiebre del TIN")]);
    info.filas.extend(datos.iter().map(|(k, v)| vec![t(k.clone()), v.clone()]));
    vec![tb, info]
}

/// Hojas de GridCraft (`gridcraft.cli.planilla_excel`).
pub fn tablas_red(r: &ResultadoRed) -> Vec<Tabla> {
    let mut res = Tabla::nueva("Resumen", &["Parámetro", "Valor"]);
    res.filas.extend(r.resumen.iter().map(|(k, v)| vec![t(k.clone()), n(*v)]));
    let con_ap = r.tramos.iter().any(|x| x.ap.is_some());
    let mut cols = vec![
        "Nodo", "Padre", "Long. (m)", "Formación", "Lotes", "P SP (kW)", "P acum. (kW)", "K (Ω/km)", "I (A)",
        "ΔV tramo (V)", "ΔV acum. (V)", "%ΔV", "Pérdidas (W)", "I máx. (A)",
    ];
    if con_ap {
        cols.extend(["P acum. AP (kW)", "I AP (A)", "%ΔV AP", "Pérdidas AP (W)"]);
    }
    let mut tr = Tabla::nueva("CaidaTension", &cols);
    for x in &r.tramos {
        let mut f = vec![
            t(x.nodo.clone()), t(x.padre.clone()), n(x.longitud_m), t(x.formacion.clone()), n(x.lotes), n(x.p_sp_kw),
            n(x.p_acum_kw), n(x.k_ohm_km), n(x.i_a), n(x.dv_tramo_v), n(x.dv_acum_v), n(x.dv_pct), n(x.perdidas_w),
            x.i_max_a.map_or(n(f64::NAN), n),
        ];
        if con_ap {
            let a = x.ap.unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
            f.extend([n(a.0), n(a.1), n(a.2), n(a.3)]);
        }
        tr.filas.push(f);
    }
    let al = if r.alertas.is_empty() {
        Tabla::lista("Alertas", "Alerta", &[], "Sin alertas")
    } else {
        let mut tb = Tabla::nueva("Alertas", &["Elemento", "Código", "Exceso (%)", "Detalle"]);
        tb.filas.extend(r.alertas.iter().map(|a| vec![t(a.elemento.clone()), t(a.codigo.clone()), n(a.exceso), t(a.detalle.clone())]));
        tb
    };
    vec![res, tr, al, Tabla::lista("Avisos", "Aviso", &r.avisos, "Sin avisos")]
}

/// Hojas de PresuCraft (`presucraft.presupuesto.excel`).
pub fn tablas_presupuesto(r: &ResultadoPresupuesto) -> Vec<Tabla> {
    let mut res = Tabla::nueva("Resumen", &[
        "Rubro", "Suministro", "Montaje", "Transporte", "Costo directo", "Gastos generales", "Utilidad", "Subtotal", "IGV", "Total",
    ]);
    for f in &r.resumen {
        res.filas.push(vec![t(f.rubro.clone()), r2(f.suministro), r2(f.montaje), r2(f.transporte), r2(f.costo_directo),
            r2(f.gastos_generales), r2(f.utilidad), r2(f.subtotal), r2(f.igv), r2(f.total)]);
    }
    let mut su = Tabla::nueva("Suministro", &["Rubro", "Código", "Descripción", "Unidad", "Cantidad", "Precio", "Parcial", "Peso (kg)", "Transporte"]);
    for f in &r.suministro {
        su.filas.push(vec![t(f.rubro.clone()), t(f.codigo.clone()), t(f.descripcion.clone()), t(f.unidad.clone()), rd(f.cantidad, 3),
            opt(f.precio, 3), rd(f.parcial, 3), rd(f.peso_kg, 3), rd(f.transporte, 3)]);
    }
    let mut mo = Tabla::nueva("Montaje", &["Rubro", "UC", "Partida", "Unidad", "Cantidad", "Precio", "Parcial"]);
    for f in &r.montaje {
        mo.filas.push(vec![t(f.rubro.clone()), t(f.uc.clone()), t(f.partida.clone()), t(f.unidad.clone()), rd(f.cantidad, 2),
            opt(f.precio, 2), rd(f.parcial, 2)]);
    }
    let mut pl = Tabla::nueva("PlanillaUC", &["Rubro", "UC", "Cantidad", "Suministro unit.", "Montaje unit.", "Peso unit. (kg)", "Transporte unit.", "Total"]);
    for f in &r.planilla_uc {
        pl.filas.push(vec![t(f.rubro.clone()), t(f.uc.clone()), n(f.cantidad), r2(f.suministro_unit), r2(f.montaje_unit),
            rd(f.peso_unit, 3), r2(f.transporte_unit), r2(f.total)]);
    }
    vec![res, su, mo, pl, Tabla::lista("Avisos", "Aviso", &r.avisos, "Sin avisos")]
}

fn r2(x: f64) -> Celda {
    rd(x, 2)
}

/// Escribe las tablas como hojas de un `.xlsx` (encabezado en negrita con borde, como pandas).
pub fn escribir_excel(ruta: &Path, tablas: &[Tabla]) -> Result<PathBuf, String> {
    use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};
    if let Some(d) = ruta.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let mut wb = Workbook::new();
    let enc = Format::new().set_bold().set_border(FormatBorder::Thin).set_align(FormatAlign::Center).set_align(FormatAlign::Top);
    let err = |e: rust_xlsxwriter::XlsxError| e.to_string();
    for tb in tablas {
        let ws = wb.add_worksheet();
        ws.set_name(&tb.nombre).map_err(err)?;
        for (j, c) in tb.columnas.iter().enumerate() {
            ws.write_string_with_format(0, j as u16, c, &enc).map_err(err)?;
        }
        for (i, f) in tb.filas.iter().enumerate() {
            for (j, c) in f.iter().enumerate() {
                let (fi, cj) = (i as u32 + 1, j as u16);
                match c {
                    Celda::Num(x) if x.is_finite() => {
                        ws.write_number(fi, cj, *x).map_err(err)?;
                    }
                    Celda::Num(x) if x.is_infinite() => {
                        ws.write_string(fi, cj, if *x > 0.0 { "inf" } else { "-inf" }).map_err(err)?;
                    }
                    Celda::Num(_) => {}
                    Celda::Ent(v) => {
                        ws.write_number(fi, cj, *v as f64).map_err(err)?;
                    }
                    Celda::Txt(s) if s.is_empty() => {}
                    Celda::Txt(s) => {
                        ws.write_string(fi, cj, s).map_err(err)?;
                    }
                }
            }
        }
    }
    wb.save(ruta).map_err(err)?;
    Ok(ruta.to_path_buf())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn progresiva_y_textos() {
        assert_eq!(formato_progresiva(1250.5, 2), "1+250.50");
        assert_eq!(formato_progresiva(999.996, 2), "1+000.00");
        assert_eq!(formato_progresiva(0.0, 2), "0+000.00");
        assert_eq!(texto_python(12.0), "12.0");
        assert_eq!(texto_python(0.125), "0.125");
    }
}
