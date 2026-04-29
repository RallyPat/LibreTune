//! ECU response codes per TunerStudio msEnvelope_1.0 specification (§15.2).
//!
//! Every reply frame's first payload byte is a response code. Success codes
//! are in the `0x00..=0x06` range; the `0x80` bit indicates an error.
//! Codes `0x94`/`0x95` carry a user-readable message in the remaining
//! payload bytes.

use serde::{Deserialize, Serialize};

/// ECU response code categories per spec §15.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseCode {
    /// 0x00 — Normal data response
    Ok,
    /// 0x01 — Table response
    OkTable,
    /// 0x02 — Burn complete
    OkBurn,
    /// 0x03 — Controller refused settings (payload contains user message; requires power cycle)
    SettingsError,
    /// 0x04 — variant OK
    OkAux,
    /// 0x05 — variant OK
    OkAux2,
    /// 0x06 — variant OK
    OkAux3,
    /// 0x80 — Controller reported an under-run
    Underrun,
    /// 0x81 — Controller reported an over-run
    Overrun,
    /// 0x82 — Controller reported a CRC mismatch
    CrcMismatch,
    /// 0x83 — Controller reported an unrecognized command
    UnrecognizedCommand,
    /// 0x84 — Controller reported value out of range
    OutOfRange,
    /// 0x85 — Controller reporting BUSY
    Busy,
    /// 0x86 — Controller reported flash locked
    FlashLocked,
    /// 0x87 — Sequence failure 1
    SeqFailure1,
    /// 0x88 — Sequence failure 2
    SeqFailure2,
    /// 0x89 — CAN queue full
    CanQueueFull,
    /// 0x8A — CAN timeout
    CanTimeout,
    /// 0x8B — CAN failure
    CanFailure,
    /// 0x8C — Parity error
    ParityError,
    /// 0x8D — Framing error
    FramingError,
    /// 0x8E — Serial noise
    SerialNoise,
    /// 0x8F — txmode range error
    TxmodeRange,
    /// 0x90 — Unknown serial error
    UnknownSerialError,
    /// 0x91 — Too many bad requests for unavailable CAN id
    TooManyBadCan,
    /// 0x92 — A controller is responding, but not at the project assigned CAN ID
    CanDeviceUnavailable,
    /// 0x93 — High speed runtime table not set
    HighSpeedTableUnset,
    /// 0x94 — Generic error (payload contains user message)
    GenericError,
    /// 0x95 — Critical error (payload contains user message)
    CriticalError,
    /// Any other response code with the high bit set
    UndefinedError(u8),
}

impl ResponseCode {
    /// Parse a response code byte per spec §15.2.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => Self::Ok,
            0x01 => Self::OkTable,
            0x02 => Self::OkBurn,
            0x03 => Self::SettingsError,
            0x04 => Self::OkAux,
            0x05 => Self::OkAux2,
            0x06 => Self::OkAux3,
            0x80 => Self::Underrun,
            0x81 => Self::Overrun,
            0x82 => Self::CrcMismatch,
            0x83 => Self::UnrecognizedCommand,
            0x84 => Self::OutOfRange,
            0x85 => Self::Busy,
            0x86 => Self::FlashLocked,
            0x87 => Self::SeqFailure1,
            0x88 => Self::SeqFailure2,
            0x89 => Self::CanQueueFull,
            0x8A => Self::CanTimeout,
            0x8B => Self::CanFailure,
            0x8C => Self::ParityError,
            0x8D => Self::FramingError,
            0x8E => Self::SerialNoise,
            0x8F => Self::TxmodeRange,
            0x90 => Self::UnknownSerialError,
            0x91 => Self::TooManyBadCan,
            0x92 => Self::CanDeviceUnavailable,
            0x93 => Self::HighSpeedTableUnset,
            0x94 => Self::GenericError,
            0x95 => Self::CriticalError,
            other => Self::UndefinedError(other),
        }
    }

    /// Raw byte value for this response code.
    pub fn as_byte(&self) -> u8 {
        match self {
            Self::Ok => 0x00,
            Self::OkTable => 0x01,
            Self::OkBurn => 0x02,
            Self::SettingsError => 0x03,
            Self::OkAux => 0x04,
            Self::OkAux2 => 0x05,
            Self::OkAux3 => 0x06,
            Self::Underrun => 0x80,
            Self::Overrun => 0x81,
            Self::CrcMismatch => 0x82,
            Self::UnrecognizedCommand => 0x83,
            Self::OutOfRange => 0x84,
            Self::Busy => 0x85,
            Self::FlashLocked => 0x86,
            Self::SeqFailure1 => 0x87,
            Self::SeqFailure2 => 0x88,
            Self::CanQueueFull => 0x89,
            Self::CanTimeout => 0x8A,
            Self::CanFailure => 0x8B,
            Self::ParityError => 0x8C,
            Self::FramingError => 0x8D,
            Self::SerialNoise => 0x8E,
            Self::TxmodeRange => 0x8F,
            Self::UnknownSerialError => 0x90,
            Self::TooManyBadCan => 0x91,
            Self::CanDeviceUnavailable => 0x92,
            Self::HighSpeedTableUnset => 0x93,
            Self::GenericError => 0x94,
            Self::CriticalError => 0x95,
            Self::UndefinedError(b) => *b,
        }
    }

    /// Whether this code represents a successful response.
    pub fn is_ok(&self) -> bool {
        matches!(
            self,
            Self::Ok | Self::OkTable | Self::OkBurn | Self::OkAux | Self::OkAux2 | Self::OkAux3
        )
    }

    /// Whether this code represents an error (high bit set).
    pub fn is_error(&self) -> bool {
        (self.as_byte() & 0x80) != 0 || matches!(self, Self::SettingsError)
    }

    /// Whether this code carries a user-readable message in its payload.
    pub fn carries_payload_message(&self) -> bool {
        matches!(
            self,
            Self::SettingsError | Self::GenericError | Self::CriticalError
        )
    }

    /// Default human-readable message per spec §15.2.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::OkTable => "OK (table response)",
            Self::OkBurn => "Burn complete",
            Self::SettingsError => {
                "Controller refused settings (power cycle required to clear)"
            }
            Self::OkAux | Self::OkAux2 | Self::OkAux3 => "OK",
            Self::Underrun => "Controller Reported an Under-run",
            Self::Overrun => "Controller Reported an Over-run",
            Self::CrcMismatch => "Controller Reported a CRC Mismatch",
            Self::UnrecognizedCommand => "Controller Reported a Unrecognized Command",
            Self::OutOfRange => "Controller Reported a Out of Range",
            Self::Busy => "Controller reporting BUSY",
            Self::FlashLocked => "Controller Reported Flash Locked",
            Self::SeqFailure1 => "Controller Reported Sequence Failure 1",
            Self::SeqFailure2 => "Controller Reported Sequence Failure 2",
            Self::CanQueueFull => "Controller Reported CAN Queue full",
            Self::CanTimeout => "Controller Reported CAN Timeout",
            Self::CanFailure => "Controller Reported CAN Failure",
            Self::ParityError => "Controller Reported Parity Error",
            Self::FramingError => "Controller Reported Framing Error",
            Self::SerialNoise => "Controller Reported Serial Noise",
            Self::TxmodeRange => "Controller Reported txmode range error",
            Self::UnknownSerialError => "Controller Reported Unknown Serial Error",
            Self::TooManyBadCan => "Too Many Bad Requests for unavailable CAN ID",
            Self::CanDeviceUnavailable => {
                "A Controller is responding, but not at the project assigned CAN ID"
            }
            Self::HighSpeedTableUnset => "High speed runtime table not set",
            Self::GenericError => "Controller reported a generic error",
            Self::CriticalError => "Controller reported a critical error",
            Self::UndefinedError(_) => "Controller reported an undefined error code",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_known_codes() {
        for byte in [
            0x00u8, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86,
            0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x91, 0x92, 0x93, 0x94,
            0x95,
        ] {
            assert_eq!(ResponseCode::from_byte(byte).as_byte(), byte);
        }
    }

    #[test]
    fn undefined_error_preserves_byte() {
        assert_eq!(ResponseCode::from_byte(0xAB).as_byte(), 0xAB);
    }

    #[test]
    fn is_ok_and_is_error() {
        assert!(ResponseCode::Ok.is_ok());
        assert!(!ResponseCode::Ok.is_error());
        assert!(ResponseCode::OkBurn.is_ok());
        assert!(ResponseCode::CrcMismatch.is_error());
        assert!(!ResponseCode::CrcMismatch.is_ok());
        assert!(ResponseCode::SettingsError.is_error());
        assert!(ResponseCode::UndefinedError(0xFE).is_error());
    }

    #[test]
    fn payload_message_codes() {
        assert!(ResponseCode::SettingsError.carries_payload_message());
        assert!(ResponseCode::GenericError.carries_payload_message());
        assert!(ResponseCode::CriticalError.carries_payload_message());
        assert!(!ResponseCode::Busy.carries_payload_message());
    }
}
