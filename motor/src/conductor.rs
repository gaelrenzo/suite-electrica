//! Conductor, hipótesis climáticas y cargas unitarias (port de `linecraft/conductor.py`).

/// Aceleración de la gravedad (m/s²).
pub const G: f64 = 9.80665;

/// Propiedades del conductor en SI.
#[derive(Clone, Debug, Default)]
pub struct Conductor {
    pub nombre: String,
    /// Fuente de los datos (catálogo).
    pub fuente: String,
    /// Sección real (m²).
    pub seccion: f64,
    /// Diámetro exterior (m).
    pub diametro: f64,
    /// Peso propio (N/m).
    pub peso: f64,
    /// Tiro de rotura (N).
    pub carga_rotura: f64,
    /// Módulo de elasticidad final (Pa).
    pub modulo_elasticidad: f64,
    /// Coeficiente de dilatación (1/°C).
    pub coef_dilatacion: f64,
    /// Curva σ(ε) inicial: σ kg/mm² = a0 + a1 ε + … (ε en %). Vacía = no hay.
    pub curva_inicial: Vec<f64>,
    /// Curva después de fluencia lenta (creep). Vacía = sin creep.
    pub curva_creep: Vec<f64>,
}

/// Condición climática de cálculo.
#[derive(Clone, Debug)]
pub struct Hipotesis {
    pub nombre: String,
    /// Temperatura del conductor (°C).
    pub temperatura: f64,
    /// Presión de viento (Pa).
    pub presion_viento: f64,
    /// Espesor radial de hielo (m).
    pub espesor_hielo: f64,
    /// Densidad del hielo (kg/m³).
    pub densidad_hielo: f64,
    pub altitud_min: f64,
    pub altitud_max: f64,
}

impl Hipotesis {
    pub fn nueva(nombre: &str, temperatura: f64, presion_viento: f64) -> Self {
        Hipotesis {
            nombre: nombre.to_string(),
            temperatura,
            presion_viento,
            espesor_hielo: 0.0,
            densidad_hielo: 0.0,
            altitud_min: f64::NEG_INFINITY,
            altitud_max: f64::INFINITY,
        }
    }
}

/// Cargas por unidad de longitud (N/m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CargaUnitaria {
    pub vertical: f64,
    pub viento: f64,
    /// Presión de viento de la hipótesis (Pa).
    pub presion: f64,
}

impl CargaUnitaria {
    /// `sqrt(vertical² + viento²)` (N/m).
    pub fn resultante(&self) -> f64 {
        self.vertical.hypot(self.viento)
    }
}

/// Cargas unitarias: hielo `ρ g π e (d + e)`, viento `p (d + 2e)`.
pub fn carga_unitaria(c: &Conductor, h: &Hipotesis) -> Result<CargaUnitaria, String> {
    let e = h.espesor_hielo;
    if e > 0.0 && h.densidad_hielo <= 0.0 {
        return Err(format!("Hipótesis {}: falta densidad del hielo", h.nombre));
    }
    let hielo = if e > 0.0 { h.densidad_hielo * G * std::f64::consts::PI * e * (c.diametro + e) } else { 0.0 };
    Ok(CargaUnitaria {
        vertical: c.peso + hielo,
        viento: h.presion_viento * (c.diametro + 2.0 * e),
        presion: h.presion_viento,
    })
}
