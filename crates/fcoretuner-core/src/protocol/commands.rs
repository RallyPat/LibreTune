//! Protocol commands
//!
//! Defines the commands supported by the Megasquirt/Speeduino protocol.

use serde::{Deserialize, Serialize};

/// Protocol commands for ECU communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Query ECU signature ('Q' command)
    QuerySignature,

    /// Get real-time data ('A' command)
    GetRealtimeData,

    /// Read from ECU memory ('R' command)
    ReadMemory,

    /// Write to ECU memory ('W' command)
    WriteMemory,

    /// Burn current page to flash ('B' command)
    BurnToFlash,

    /// Get CRC of a page ('C' command)
    GetCrc,

    /// Get full status ('S' command)
    GetStatus,

    /// Page select (legacy protocol)
    SelectPage,

    /// Test communication ('I' - identity/info in some firmware)
    TestCommunication,

    /// Send CAN message (for CAN-enabled ECUs)
    CanMessage,
}

impl Command {
    /// Get the legacy (single-character) command byte
    pub fn legacy_byte(&self) -> u8 {
        match self {
            Command::QuerySignature => b'Q',
            Command::GetRealtimeData => b'A',
            Command::ReadMemory => b'R',
            Command::WriteMemory => b'W',
            Command::BurnToFlash => b'B',
            Command::GetCrc => b'C',
            Command::GetStatus => b'S',
            Command::SelectPage => b'P',
            Command::TestCommunication => b'I',
            Command::CanMessage => b'M',
        }
    }

    /// Get the modern protocol command character
    pub fn modern_char(&self) -> char {
        self.legacy_byte() as char
    }

    /// Check if this command expects a response
    pub fn expects_response(&self) -> bool {
        match self {
            Command::WriteMemory | Command::BurnToFlash | Command::SelectPage => false,
            _ => true,
        }
    }

    /// Get the expected response timeout in milliseconds
    pub fn timeout_ms(&self) -> u64 {
        match self {
            Command::BurnToFlash => 3000,    // Burning takes longer
            Command::GetRealtimeData => 100, // Should be fast
            _ => 1000,                       // Default timeout
        }
    }
}

/// Read memory command parameters
#[derive(Debug, Clone, Copy)]
pub struct ReadMemoryParams {
    /// Page to read from
    pub page: u8,
    /// Offset within page
    pub offset: u16,
    /// Number of bytes to read
    pub length: u16,
    /// CAN ID for CAN-enabled ECUs (0 for local)
    pub can_id: u8,
}

impl ReadMemoryParams {
    pub fn new(page: u8, offset: u16, length: u16) -> Self {
        Self {
            page,
            offset,
            length,
            can_id: 0,
        }
    }
}

/// Write memory command parameters
#[derive(Debug, Clone)]
pub struct WriteMemoryParams {
    /// Page to write to
    pub page: u8,
    /// Offset within page
    pub offset: u16,
    /// Data to write
    pub data: Vec<u8>,
    /// CAN ID for CAN-enabled ECUs (0 for local)
    pub can_id: u8,
}

impl WriteMemoryParams {
    pub fn new(page: u8, offset: u16, data: Vec<u8>) -> Self {
        Self {
            page,
            offset,
            data,
            can_id: 0,
        }
    }
}

/// Burn command parameters
#[derive(Debug, Clone, Copy)]
pub struct BurnParams {
    /// Page to burn
    pub page: u8,
    /// CAN ID for CAN-enabled ECUs (0 for local)
    pub can_id: u8,
}

impl BurnParams {
    pub fn new(page: u8) -> Self {
        Self { page, can_id: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_bytes() {
        assert_eq!(Command::QuerySignature.legacy_byte(), b'Q');
        assert_eq!(Command::GetRealtimeData.legacy_byte(), b'A');
        assert_eq!(Command::BurnToFlash.legacy_byte(), b'B');
    }

    #[test]
    fn test_command_response() {
        assert!(Command::QuerySignature.expects_response());
        assert!(!Command::BurnToFlash.expects_response());
    }
}
