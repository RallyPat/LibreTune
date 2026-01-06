//! ECU Memory Model
//!
//! Manages the ECU's memory state, providing typed access to constants and tables.

mod memory;
mod shadow;
mod values;

pub use memory::EcuMemory;
pub use shadow::ShadowMemory;
pub use values::Value;
