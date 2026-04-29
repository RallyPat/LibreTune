//! TunerStudio `.tuneView` file format support.
//!
//! Parses and writes `.tuneView` XML files (namespace
//! `http://www.EFIAnalytics.com/:tuneView`). These files describe a single
//! tuning view: a layout of `tuneComp` elements (table editors, settings
//! panels, selectable tables, embedded dashboards via `EncodedDashboard`,
//! etc.) along with bibliography metadata, a version stanza, and an
//! optional base64 preview image.
//!
//! The model is intentionally a lossless property-bag (preserving element
//! order, the `type` attribute, any extra attributes, and text content)
//! so round-trip parse → write → parse produces an equal structure for
//! all stock TS sample files.

mod parser;
mod types;
mod writer;

pub use parser::*;
pub use types::*;
pub use writer::*;
