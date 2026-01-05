//! TunerStudio-compatible dashboard and gauge file format support.
//!
//! This module implements parsing and writing of TunerStudio's .dash and .gauge
//! XML file formats, enabling full compatibility with existing dashboard files.

mod types;
mod parser;
mod writer;

pub use types::*;
pub use parser::*;
pub use writer::*;
