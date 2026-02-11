//! Protocol errors

use thiserror::Error;

/// Errors that can occur during protocol communication
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Serial port error: {0}")]
    SerialError(String),

    #[error("Connection timeout")]
    Timeout,

    #[error("Not connected to ECU")]
    NotConnected,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Already connected")]
    AlreadyConnected,

    #[error("Invalid response from ECU")]
    InvalidResponse,

    #[error("CRC mismatch: expected {expected:#010x}, got {actual:#010x}")]
    CrcMismatch { expected: u32, actual: u32 },

    #[error("Signature mismatch: expected '{expected}', got '{actual}'")]
    SignatureMismatch { expected: String, actual: String },

    #[error("ECU returned error code: {0}")]
    EcuError(u8),

    #[error("Buffer overflow: packet too large")]
    BufferOverflow,

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Port not found: {0}")]
    PortNotFound(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
