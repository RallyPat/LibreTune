//! Small helper functions extracted from lib.rs.

use libretune_core::ini::{DataType, Endianness};

/// Parse a runtime packet mode string into enum
pub(crate) fn parse_runtime_packet_mode(mode: &str) -> libretune_core::protocol::RuntimePacketMode {
    use libretune_core::protocol::RuntimePacketMode as Rpm;
    match mode {
        "ForceBurst" => Rpm::ForceBurst,
        "ForceOCH" => Rpm::ForceOCH,
        "Disabled" => Rpm::Disabled,
        _ => Rpm::Auto,
    }
}

/// Returns 0xFF if bits >= 8, otherwise (1u8 << bits) - 1.
#[allow(dead_code)]
#[inline]
pub(crate) fn bit_mask_u8(bits: u8) -> u8 {
    if bits >= 8 {
        0xFF
    } else {
        (1u8 << bits) - 1
    }
}

/// Helper to write stream diagnostic logs to /tmp/libretune-stream.log
pub(crate) fn stream_log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/libretune-stream.log")
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let _ = writeln!(f, "[{:.3}] {}", now.as_secs_f64(), msg);
    }
}

/// Global tracker for who currently holds the connection lock.
/// Used for diagnostics only — helps identify which command is blocking the stream.
static CONN_LOCK_HOLDER: std::sync::Mutex<&str> = std::sync::Mutex::new("(none)");

pub(crate) fn set_conn_lock_holder(who: &'static str) {
    if let Ok(mut guard) = CONN_LOCK_HOLDER.lock() {
        *guard = who;
    }
}

pub(crate) fn get_conn_lock_holder() -> String {
    CONN_LOCK_HOLDER
        .lock()
        .map(|g| g.to_string())
        .unwrap_or_else(|_| "(poisoned)".to_string())
}

/// Read a raw numeric value from bytes based on data type.
///
/// `endianness` is the byte order the definition declares — rusEFI/FOME INIs
/// are little-endian, so this must never be assumed. Decoding goes through
/// [`DataType::read_from_bytes`] so multi-byte reads stay in one place.
pub(crate) fn read_raw_value(
    bytes: &[u8],
    data_type: &DataType,
    endianness: Endianness,
) -> Result<f64, String> {
    match data_type {
        // Strings carry no numeric value.
        DataType::String => Ok(0.0),
        _ => data_type
            .read_from_bytes(bytes, 0, endianness)
            .ok_or_else(|| format!("Insufficient data for {:?}", data_type)),
    }
}

#[cfg(test)]
mod read_raw_value_tests {
    use super::read_raw_value;
    use libretune_core::ini::{DataType, Endianness};

    #[test]
    fn reads_u16_in_both_endiannesses() {
        let bytes = [0x12u8, 0x34];
        assert_eq!(
            read_raw_value(&bytes, &DataType::U16, Endianness::Big).unwrap(),
            0x1234_u16 as f64
        );
        assert_eq!(
            read_raw_value(&bytes, &DataType::U16, Endianness::Little).unwrap(),
            0x3412_u16 as f64
        );
    }

    #[test]
    fn reads_s16_in_both_endiannesses() {
        let bytes = [0xFFu8, 0xFE];
        assert_eq!(
            read_raw_value(&bytes, &DataType::S16, Endianness::Big).unwrap(),
            -2.0
        );
        assert_eq!(
            read_raw_value(&bytes, &DataType::S16, Endianness::Little).unwrap(),
            -257.0
        );
    }

    #[test]
    fn reads_u32_in_both_endiannesses() {
        let bytes = [0x00u8, 0x00, 0x01, 0x00];
        assert_eq!(
            read_raw_value(&bytes, &DataType::U32, Endianness::Big).unwrap(),
            256.0
        );
        assert_eq!(
            read_raw_value(&bytes, &DataType::U32, Endianness::Little).unwrap(),
            65_536.0
        );
    }

    #[test]
    fn reads_s32_in_both_endiannesses() {
        let bytes = [0xFFu8, 0xFF, 0xFF, 0xFE];
        assert_eq!(
            read_raw_value(&bytes, &DataType::S32, Endianness::Big).unwrap(),
            -2.0
        );
        assert_eq!(
            read_raw_value(&bytes, &DataType::S32, Endianness::Little).unwrap(),
            -16_777_217.0
        );
    }

    #[test]
    fn reads_f32_in_both_endiannesses() {
        // 1.0f32 is 0x3F800000 big-endian.
        let big = [0x3Fu8, 0x80, 0x00, 0x00];
        let little = [0x00u8, 0x00, 0x80, 0x3F];
        assert_eq!(
            read_raw_value(&big, &DataType::F32, Endianness::Big).unwrap(),
            1.0
        );
        assert_eq!(
            read_raw_value(&little, &DataType::F32, Endianness::Little).unwrap(),
            1.0
        );
    }

    #[test]
    fn single_byte_types_ignore_endianness() {
        let bytes = [0x80u8];
        assert_eq!(
            read_raw_value(&bytes, &DataType::U08, Endianness::Little).unwrap(),
            128.0
        );
        assert_eq!(
            read_raw_value(&bytes, &DataType::S08, Endianness::Big).unwrap(),
            -128.0
        );
    }

    #[test]
    fn errors_when_the_slice_is_too_short_for_the_type() {
        let err = read_raw_value(&[0x01u8], &DataType::U16, Endianness::Little)
            .expect_err("one byte cannot hold a U16");
        assert!(err.contains("U16"), "unexpected error: {err}");
    }
}
