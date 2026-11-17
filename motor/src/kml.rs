//! Exportación a KML (Google Earth, «Ver en Planeta» de TOPOMAGIC) y de puntos a CSV.
//! Las coordenadas del proyecto (UTM) se convierten a geográficas WGS84 de la zona del proyecto.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::utm::utm_a_geograficas;

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Documento KML en construcción.
pub struct Kml {
    nombre: String,
    zona: String,
    cuerpo: String,
}

impl Kml {
    pub fn nuevo(nombre: &str, zona_utm: &str) -> Self {
        Kml { nombre: nombre.into(), zona: zona_utm.into(), cuerpo: String::new() }
    }

    fn coords(&self, xy: &[[f64; 2]]) -> Result<String, String> {
        let mut s = String::new();
        for p in xy {
            let (lon, lat) = utm_a_geograficas(p[0], p[1], &self.zona)?;
            let _ = write!(s, "{lon:.8},{lat:.8},0 ");
        }
        Ok(s.trim_end().to_string())
    }

    /// Línea (eje, tramo de red) con color `aabbggrr` de KML y ancho en píxeles.
    pub fn linea(&mut self, nombre: &str, xy: &[[f64; 2]], color: &str, ancho: f64) -> Result<(), String> {
        let c = self.coords(xy)?;
        let _ = write!(
            self.cuerpo,
            "<Placemark><name>{}</name><Style><LineStyle><color>{color}</color><width>{ancho}</width></LineStyle></Style>\
             <LineString><tessellate>1</tessellate><coordinates>{c}</coordinates></LineString></Placemark>\n",
            xml(nombre)
        );
        Ok(())
    }

    pub fn punto(&mut self, nombre: &str, xy: [f64; 2], descripcion: &str) -> Result<(), String> {
        let c = self.coords(&[xy])?;
        let _ = write!(
            self.cuerpo,
            "<Placemark><name>{}</name><description>{}</description><Point><coordinates>{c}</coordinates></Point></Placemark>\n",
            xml(nombre),
            xml(descripcion)
        );
        Ok(())
    }

    /// Polígono cerrado (predio).
    pub fn poligono(&mut self, nombre: &str, xy: &[[f64; 2]]) -> Result<(), String> {
        let mut v = xy.to_vec();
        if v.first() != v.last() {
            v.push(v[0]);
        }
        let c = self.coords(&v)?;
        let _ = write!(
            self.cuerpo,
            "<Placemark><name>{}</name><Style><PolyStyle><color>4dc87832</color></PolyStyle></Style>\
             <Polygon><outerBoundaryIs><LinearRing><coordinates>{c}</coordinates></LinearRing></outerBoundaryIs></Polygon></Placemark>\n",
            xml(nombre)
        );
        Ok(())
    }

    pub fn texto(&self) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document><name>{}</name>\n{}</Document></kml>\n",
            xml(&self.nombre),
            self.cuerpo
        )
    }

    pub fn guardar(&self, ruta: &Path) -> Result<PathBuf, String> {
        std::fs::write(ruta, self.texto()).map_err(|e| format!("{}: {e}", ruta.display()))?;
        Ok(ruta.to_path_buf())
    }
}

/// Puntos topográficos a CSV (punto, este, norte, cota, descripcion), legible por «Importar puntos».
pub fn escribir_puntos_csv(ruta: &Path, pts: &[crate::entrada::Punto]) -> Result<PathBuf, String> {
    let mut w = csv::Writer::from_path(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let err = |e: csv::Error| e.to_string();
    w.write_record(["punto", "este", "norte", "cota", "descripcion"]).map_err(err)?;
    for p in pts {
        w.write_record([p.punto.clone(), p.este.to_string(), p.norte.to_string(), p.cota.to_string(), p.descripcion.clone()]).map_err(err)?;
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok(ruta.to_path_buf())
}
