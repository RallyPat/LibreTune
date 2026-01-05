//! ECU Memory Model
//!
//! Manages the ECU's memory state, providing typed access to constants and tables.

mod memory;
mod values;
mod shadow;

pub use memory::EcuMemory;
pub use values::Value;
pub use shadow::ShadowMemory;
