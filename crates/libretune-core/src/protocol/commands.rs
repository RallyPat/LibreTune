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
        !matches!(
            self,
            Command::WriteMemory | Command::BurnToFlash | Command::SelectPage
        )
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

/// Console command for rusEFI/FOME/epicEFI text-based console I/O
/// These commands are sent as plain text strings (typically ASCII) to the ECU's
/// text-based console interface and receive text-based responses.
#[derive(Debug, Clone)]
pub struct ConsoleCommand {
    /// The text command to send (e.g., "help", "status", "set someVar 100")
    pub command: String,
    /// Timeout for this specific command (ms). If None, uses default 1000ms
    pub timeout_ms: Option<u64>,
}

impl ConsoleCommand {
    /// Create a new console command
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            timeout_ms: None,
        }
    }

    /// Create a console command with custom timeout
    pub fn with_timeout(command: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            command: command.into(),
            timeout_ms: Some(timeout_ms),
        }
    }

    /// Get effective timeout for this command
    pub fn get_timeout_ms(&self) -> u64 {
        self.timeout_ms.unwrap_or(1000)
    }

    /// Convert command to bytes, appending newline for transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.command.as_bytes().to_vec();
        bytes.push(b'\n'); // Append newline for ECU to detect end of command
        bytes
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

    #[test]
    fn test_console_command_creation() {
        let cmd = ConsoleCommand::new("help");
        assert_eq!(cmd.command, "help");
        assert_eq!(cmd.timeout_ms, None);
        assert_eq!(cmd.get_timeout_ms(), 1000);
    }

    #[test]
    fn test_console_command_with_timeout() {
        let cmd = ConsoleCommand::with_timeout("status", 2000);
        assert_eq!(cmd.command, "status");
        assert_eq!(cmd.get_timeout_ms(), 2000);
    }

    #[test]
    fn test_console_command_to_bytes() {
        let cmd = ConsoleCommand::new("help");
        let bytes = cmd.to_bytes();
        assert_eq!(bytes, b"help\n".to_vec());
    }
}
