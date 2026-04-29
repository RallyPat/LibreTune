//! TS-compatible dashboard and gauge file format support.
//!
//! This module implements parsing and writing of TS `.dash` and `.gauge`
//! XML file formats. The runtime types in `types.rs` (`DashFile`,
//! `GaugeConfig`, `IndicatorConfig`, ...) are LibreTune's single canonical
//! dashboard model — the same structures are used for in-memory state,
//! Tauri command payloads, and TS XML round-trip.

mod parser;
mod templates;
mod types;
mod validation;
mod writer;

pub use parser::*;
pub use templates::*;
pub use types::*;
pub use validation::*;
pub use writer::*;
