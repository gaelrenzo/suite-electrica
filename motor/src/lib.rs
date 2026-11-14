//! Núcleo de cálculo de Suite Eléctrica en Rust.
//!
//! Port de `suite_electrica` (Python). Cada módulo se valida contra los resultados de la
//! versión Python (`tests/referencia*.txt`, generados por `motor/generar_referencia*.py`),
//! que a su vez está validada contra REDLIN.

pub mod alertas;
pub mod catalogos;
pub mod conductor;
pub mod derivaciones;
pub mod flechado;
pub mod geo;
pub mod linea;
pub mod mecanica;
pub mod numerico;
pub mod presupuesto;
pub mod red;
