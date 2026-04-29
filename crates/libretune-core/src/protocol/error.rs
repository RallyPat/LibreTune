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

    /// ECU returned a structured error response per msEnvelope_1.0 spec §15.2
    #[error("ECU error 0x{code:02x}: {message}")]
    EcuStatusError {
        /// Raw response code byte
        code: u8,
        /// Human-readable message (default per spec, or payload-derived for 0x03/0x94/0x95)
        message: String,
    },

    #[error("Buffer overflow: packet too large")]
    BufferOverflow,

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Port not found: {0}")]
    PortNotFound(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
