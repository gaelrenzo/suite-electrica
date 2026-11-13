//! Núcleo de cálculo de Suite Eléctrica en Rust.
//!
//! Port de `suite_electrica.linecraft.{conductor, mecanica}` (Python). Cada función
//! se valida contra los resultados de la versión Python con `tests/referencia.json`
//! (generado por `motor/generar_referencia.py`), que a su vez está validada contra REDLIN.

pub mod conductor;
pub mod mecanica;
pub mod numerico;
