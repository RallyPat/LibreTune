//! Base Map Generator
//!
//! Generates safe starting tunes from user-provided engine specifications.
//! Produces VE tables, ignition tables, enrichment curves, and auxiliary settings
//! based on engine displacement, injector size, cylinder count, and aspiration type.

mod engine_spec;
pub mod generator;

pub use engine_spec::{Aspiration, EngineSpec, FuelType, IgnitionMode, InjectionMode, StrokeType};
pub use generator::{AccelEnrichConfig, BaseMap, IacConfig};
