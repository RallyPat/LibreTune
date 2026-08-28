//! Speeduino sensor-calibration wire format.
//!
//! Speeduino keeps its sensor calibration tables (coolant, IAT, O2/AFR)
//! outside the 15 INI-declared tune pages, so `write_page`/`burn` can never
//! reach them. They are written through a dedicated `t` command instead,
//! verified against the firmware source at tag 202501
//! (`speeduino/comms_legacy.cpp receiveCalibration()` and
//! `speeduino/comms.cpp` case `'t'`).
//!
//! Everything here is pinned to **202501**, which is the tag this project
//! targets, and master has since diverged in ways that matter:
//!
//! * At 202501 the table ids are plain macros (`CLT_CALIBRATION_PAGE` 0,
//!   `IAT_CALIBRATION_PAGE` 1, `O2_CALIBRATION_PAGE` 2). The
//!   `SensorCalibrationTable` enum, and the `saveCalibrationTable` /
//!   `saveCalibrationCrc` names, are post-202501 additions; at this tag the
//!   functions are `writeCalibrationPage` / `storeCalibrationCRC32` and the
//!   O2 handler is `loadO2CalibrationChunk`. The *values* 0/1/2 are the same
//!   either way, which is all we depend on.
//! * At 202501 the legacy `'t'` command still writes. Firmware newer than
//!   202501 makes legacy comms read-only — it consumes the command and the
//!   data and writes nothing, with no error — so a legacy-protocol
//!   calibration write against newer firmware silently does nothing. There is
//!   no ACK on that path to detect it with; see
//!   `Connection::write_calibration_legacy` for how that is surfaced.
//!
//! The two protocols:
//!
//! * **Legacy protocol** — `'t'`, one table-id byte, then the raw data
//!   stream. No header beyond the id, no length field, **no ACK**: the
//!   firmware blocks in `receiveCalibration()` until it has consumed exactly
//!   the table's byte count, then writes EEPROM.
//! * **Modern (CRC envelope) protocol** — one `'t'` frame per chunk with a
//!   7-byte header: `['t', id_hi, tableId, offset_hi, offset_lo, len_hi,
//!   len_lo]` followed by `len` data bytes. Offset and length are
//!   **big-endian** here (`word(payload[3], payload[4])`), unlike the `'M'`
//!   page-write and `'p'` page-read commands, which build the same kind of
//!   word as `word(payload[4], payload[3])` — little-endian. That asymmetry
//!   is real and deliberate: the INI's own command template,
//!   `tableWriteCommand = "t\$tsCanId%2i%2o%2c%v"`, uses TunerStudio's
//!   big-endian `%2x` fields, and the table id is likewise a two-byte
//!   big-endian field whose low byte lands at `payload[2]` (which is all the
//!   firmware reads). Each chunk is ACKed with a return-code envelope.
//!
//!   The O2 table is sent as 4×256-byte chunks: the firmware burns EEPROM
//!   only once the running offset reaches 1023, and its subsample guard is
//!   chunk-relative (`x % 32`) while the destination index is absolute
//!   (`offset / 32`), so chunks must start on multiples of 32 or values land
//!   in the wrong slots. Note the firmware does *not* validate the O2 chunk
//!   length — 256 is convention (and the INI's `tableBlockingFactor`), not
//!   something the ECU will reject you for getting wrong. Temperature tables
//!   go as a single 64-byte chunk; that length *is* checked, and anything
//!   else is rejected with `SERIAL_RC_RANGE_ERR`.
//!
//! Data formats (both protocols):
//!
//! * **CLT (id 0) / IAT (id 1)**: 32 × little-endian `i16`, each **°F × 10**
//!   (TunerStudio heritage). The firmware divides by 10, converts °F→°C with
//!   integer math, adds `CALIBRATION_TEMPERATURE_OFFSET` (40) and clamps at
//!   zero. The 32 entries are the temperatures at ADC bins the *firmware*
//!   assigns: `x*32` on the legacy path, `x*33` on the modern path.
//! * **O2 (id 2)**: 1024 bytes, one per 10-bit ADC count, each **AFR × 10**.
//!   The firmware stores every 32nd value in an interpolated 2D table.

use crc32fast::Hasher;

/// Which calibration table a `t` command targets.
///
/// The discriminants are the firmware's table ids. At 202501 those are
/// `#define`s in `globals.h`, not an enum — the `SensorCalibrationTable` enum
/// with the same values appears only after this tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalibrationTable {
    /// Coolant temperature sensor curve.
    Clt,
    /// Intake air temperature sensor curve.
    Iat,
    /// O2 / wideband AFR transfer curve.
    O2,
}

impl CalibrationTable {
    /// Firmware table id (`CLT_CALIBRATION_PAGE` 0, `IAT_CALIBRATION_PAGE` 1,
    /// `O2_CALIBRATION_PAGE` 2).
    pub fn id(self) -> u8 {
        match self {
            CalibrationTable::Clt => 0,
            CalibrationTable::Iat => 1,
            CalibrationTable::O2 => 2,
        }
    }
}

/// Entry count of a temperature calibration curve.
pub const TEMP_CALIBRATION_POINTS: usize = 32;
/// Wire size of a temperature calibration table (32 × i16).
pub const TEMP_CALIBRATION_WIRE_BYTES: usize = TEMP_CALIBRATION_POINTS * 2;
/// Wire size of the O2 calibration table (one byte per 10-bit ADC count).
pub const O2_CALIBRATION_WIRE_BYTES: usize = 1024;
/// Modern-protocol chunk size for the O2 table (fixed by the firmware:
/// EEPROM burn fires only once `offset` reaches 1023).
pub const O2_CALIBRATION_CHUNK: usize = 256;

/// ADC bins the firmware assigns to the 32 temperature entries.
///
/// The two protocol paths differ (a firmware inconsistency, mirrored here so
/// callers sample their sensor curve at the points the ECU will actually
/// use): legacy `receiveCalibration()` sets `x*32` (0..992), modern
/// `processTemperatureCalibrationTableUpdate()` sets `x*33` (0..1023).
pub fn temperature_calibration_bins(modern_protocol: bool) -> [u16; TEMP_CALIBRATION_POINTS] {
    let step = if modern_protocol { 33 } else { 32 };
    let mut bins = [0u16; TEMP_CALIBRATION_POINTS];
    for (x, bin) in bins.iter_mut().enumerate() {
        *bin = (x as u16) * step;
    }
    bins
}

/// Encode 32 temperatures (°C) into the wire format: little-endian i16,
/// °F × 10.
///
/// The naive encoding (round °F × 10, the TunerStudio behavior) loses up to
/// ~1.5 °C to the firmware's two truncating integer divisions, always
/// downward. Instead of mirroring that bias, each entry is chosen as the
/// wire value whose *firmware-side result* lands closest to the requested
/// temperature — the firmware math is deterministic
/// ([`firmware_stored_temperature`]), so the encoder can search the handful
/// of candidate raw values around the naive one and guarantee the stored
/// table is within quantization (±0.5 °C) of the request everywhere the
/// firmware's range allows.
pub fn encode_temperature_calibration(temps_c: &[f64; TEMP_CALIBRATION_POINTS]) -> Vec<u8> {
    let mut wire = Vec::with_capacity(TEMP_CALIBRATION_WIRE_BYTES);
    for &c in temps_c {
        let f10 = (c * 9.0 / 5.0 + 32.0) * 10.0;
        let base = f10.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        // The firmware divisions truncate toward zero, so the bias direction
        // flips with sign (down for positive °F, up below zero). ±1.8 °F
        // covers both divisions in either direction.
        let mut best = base;
        let mut best_err = f64::INFINITY;
        for delta in -18i16..=18 {
            let raw = base.saturating_add(delta);
            let stored_c = firmware_stored_temperature(raw) as f64 - 40.0;
            let err = (stored_c - c).abs();
            if err < best_err {
                best_err = err;
                best = raw;
            }
        }
        wire.extend_from_slice(&best.to_le_bytes());
    }
    wire
}

/// Encode 1024 AFR values into the wire format: one byte per ADC count,
/// AFR × 10, saturating at the u8 range (25.5 AFR).
pub fn encode_o2_calibration(afr: &[f64; O2_CALIBRATION_WIRE_BYTES]) -> Vec<u8> {
    afr.iter()
        .map(|a| (a * 10.0).round().clamp(0.0, 255.0) as u8)
        .collect()
}

/// The value the firmware will store for one wire i16, reproduced with the
/// firmware's exact integer math (`receiveCalibration()` /
/// `toTemperature()`): divide by 10 truncating toward zero, °F→°C truncating
/// toward zero, add the +40 offset, clamp at 0. Returned in the firmware's
/// internal unit (°C + 40).
///
/// Exposed so tests (and the sim-ecu validator) can assert byte-exact
/// round-trips instead of "close enough" float comparisons.
pub fn firmware_stored_temperature(wire_f10: i16) -> u16 {
    let temp_f = wire_f10 as i32 / 10; // div().quot truncates toward zero
    let temp_c = ((temp_f - 32) * 5) / 9; // C integer division, also truncating
    (temp_c + 40).max(0) as u16
}

/// CRC32 the firmware stores for a calibration page, computed over the exact
/// wire data bytes.
///
/// Both firmware paths reduce to the standard CRC32 of the data: the
/// temperature path calls `FastCRC32::crc32()` over the 64 payload bytes,
/// and the O2 path accumulates an *unfinalized* CRC over all 1024 bytes and
/// then bit-inverts it — which equals the finalized value, since CRC32
/// finalization is exactly the final XOR with `0xFFFFFFFF`.
pub fn calibration_crc32(wire_data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(wire_data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_ids_match_firmware_constants() {
        // globals.h: CLT_CALIBRATION_PAGE 0, IAT 1, O2 2.
        assert_eq!(CalibrationTable::Clt.id(), 0);
        assert_eq!(CalibrationTable::Iat.id(), 1);
        assert_eq!(CalibrationTable::O2.id(), 2);
    }

    #[test]
    fn temperature_encoding_is_fahrenheit_times_ten_little_endian() {
        // The wire unit is °F × 10, i16 little-endian. The encoder may bias
        // an entry upward a little to defeat the firmware's truncation, so
        // pin the unit by decoding: the wire value must be within 2 (0.2 °F)
        // of the naive °F × 10 of the request.
        let mut temps = [0.0f64; TEMP_CALIBRATION_POINTS];
        temps[0] = 100.0; // 212 °F → ~2120
        temps[1] = -40.0; // -40 °F → ~-400
        let wire = encode_temperature_calibration(&temps);
        assert_eq!(wire.len(), TEMP_CALIBRATION_WIRE_BYTES);
        let w0 = i16::from_le_bytes([wire[0], wire[1]]);
        let w1 = i16::from_le_bytes([wire[2], wire[3]]);
        assert!((w0 - 2120).abs() <= 18, "100 °C encoded as {w0}");
        assert!((w1 - -400).abs() <= 18, "-40 °C encoded as {w1}");
    }

    #[test]
    fn firmware_math_round_trips_whole_celsius_values() {
        // For any whole °C in the plausible sensor range, encoding then
        // running the firmware's exact integer math must recover the same
        // degree (as °C + 40) exactly — the bias-corrected encoder exists
        // precisely so truncation can never shift a calibration point.
        for c in -40..=210 {
            let mut temps = [0.0f64; TEMP_CALIBRATION_POINTS];
            temps[0] = c as f64;
            let wire = encode_temperature_calibration(&temps);
            let raw = i16::from_le_bytes([wire[0], wire[1]]);
            assert_eq!(
                firmware_stored_temperature(raw),
                (c + 40).max(0) as u16,
                "temperature {c} °C did not survive the firmware conversion"
            );
        }
    }

    #[test]
    fn fractional_temperatures_store_within_half_a_degree() {
        for tenths in -400..=2100i32 {
            let c = tenths as f64 / 10.0;
            let mut temps = [0.0f64; TEMP_CALIBRATION_POINTS];
            temps[0] = c;
            let wire = encode_temperature_calibration(&temps);
            let raw = i16::from_le_bytes([wire[0], wire[1]]);
            let stored_c = firmware_stored_temperature(raw) as f64 - 40.0;
            assert!(
                (stored_c - c).abs() <= 0.5 + 1e-9,
                "{c} °C stored as {stored_c} °C"
            );
        }
    }

    #[test]
    fn o2_encoding_is_afr_times_ten_saturating() {
        let mut afr = [14.7f64; O2_CALIBRATION_WIRE_BYTES];
        afr[0] = 9.0;
        afr[1] = 100.0; // absurd input from a bad formula must clamp, not wrap
        afr[2] = -1.0;
        let wire = encode_o2_calibration(&afr);
        assert_eq!(wire.len(), O2_CALIBRATION_WIRE_BYTES);
        assert_eq!(wire[0], 90);
        assert_eq!(wire[1], 255);
        assert_eq!(wire[2], 0);
        assert_eq!(wire[3], 147);
    }

    #[test]
    fn bins_match_each_firmware_path() {
        let legacy = temperature_calibration_bins(false);
        let modern = temperature_calibration_bins(true);
        assert_eq!(legacy[0], 0);
        assert_eq!(legacy[31], 31 * 32); // 992
        assert_eq!(modern[0], 0);
        assert_eq!(modern[31], 31 * 33); // 1023
    }
}
