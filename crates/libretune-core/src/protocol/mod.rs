//! Serial Protocol Communication
//!
//! Implements the Megasquirt/Speeduino serial protocol for ECU communication.
//!
//! Supports both legacy ASCII protocol and modern binary protocol with CRC32.

pub mod serial;
pub mod commands;
pub mod command_builder;
mod packet;
mod connection;
mod error;

pub use error::ProtocolError;
pub use connection::{Connection, ConnectionState, ConnectionConfig};
pub use commands::Command;
pub use packet::{Packet, PacketBuilder};
pub use command_builder::CommandBuilder;
pub use serial::{list_ports, open_port, configure_port, clear_buffers, PortInfo};

/// Default baud rate for ECU communication
pub const DEFAULT_BAUD_RATE: u32 = 115200;

/// Default timeout for responses in milliseconds
/// Increased from 1000ms to 2000ms to accommodate USB/ECU latency observed during handshakes.
pub const DEFAULT_TIMEOUT_MS: u64 = 2000;

/// Maximum packet size
pub const MAX_PACKET_SIZE: usize = 8192;
