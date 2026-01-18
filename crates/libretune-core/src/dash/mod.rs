//! TS-compatible dashboard and gauge file format support.
//!
//! This module implements parsing and writing of TS .dash and .gauge
//! XML file formats, enabling full compatibility with existing dashboard files.

mod parser;
mod templates;
mod types;
mod writer;

pub use parser::*;
pub use templates::*;
pub use types::*;
pub use writer::*;
