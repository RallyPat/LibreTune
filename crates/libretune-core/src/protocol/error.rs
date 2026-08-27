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

    /// Connection was closed/cancelled mid-operation (e.g. user-initiated disconnect
    /// interrupted a blocking read). Used by the cancellation mechanism (Issue #71).
    #[error("Connection closed")]
    ConnectionClosed,

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

    /// A write was read straight back and the ECU did not hold what was sent.
    /// Distinct from a failed write: the command was accepted, so nothing else
    /// on the wire reports a problem, and the tune the tuner is looking at no
    /// longer matches the tune the engine is running.
    #[error(
        "ECU did not store what was written to page {page} offset {offset}: \
         sent {expected:02X?}, read back {actual:02X?}"
    )]
    WriteVerificationFailed {
        page: u8,
        offset: u16,
        expected: u8,
        actual: u8,
    },

    /// A whole page was written and the ECU's own CRC32 of it disagrees with a
    /// CRC of the bytes that were sent. Distinct from
    /// [`ProtocolError::WriteVerificationFailed`], which compares one byte
    /// read straight back: this compares an entire page as the ECU assembled
    /// it, which is the failure a chunked bulk write can produce while every
    /// individual frame is acknowledged.
    #[error(
        "ECU's CRC32 of page {page} does not match what was written:          sent {expected:08X}, ECU reports {actual:08X}"
    )]
    PageCrcMismatch {
        page: u8,
        expected: u32,
        actual: u32,
    },

    /// A write went out but the read-back could not complete, so the write is
    /// unverified. On a legacy ECU this is itself the corruption signature: a
    /// buffer-overrun ECU consumes the read command as table data and answers
    /// with silence. Must not be treated as "offline" — the write was sent.
    #[error(
        "write to page {page} offset {offset} could not be verified \
         (read-back failed: {reason}) — treat the write as unconfirmed"
    )]
    WriteVerificationUnavailable {
        page: u8,
        offset: u16,
        reason: String,
    },

    #[error("Port not found: {0}")]
    PortNotFound(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
