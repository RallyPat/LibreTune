//! Virtual ECU for hardware-free development, demos and CI.
//!
//! Unlike [`crate::demo`], which fabricates sensor readings at the top of the
//! stack, this simulator answers the real serial protocol: it holds a page
//! image, responds to the commands a controller would, and drives its output
//! channels from an engine model. Client code connects to it exactly as it
//! connects to hardware.

mod ecu;
mod engine;
mod och_codec;
mod ve_model;

pub use ecu::{EcuSimulator, SimulatorChannel};
pub use engine::EngineMode;
