//! Definition-driven encoding of the simulator's realtime frame.
//!
//! The physics produce a [`ChannelValues`] snapshot; this module writes it
//! into the output-channel block at whatever offsets and types the loaded
//! INI's `[OutputChannels]` declares. Raw encoding is the inverse of
//! TunerStudio's `physical = (raw + translate) * scale`, but out-of-range
//! values clamp instead of erroring — the simulator must stay graceful and
//! never panic its thread.

use crate::ini::{DataType, EcuDefinition, Endianness, OutputChannel};
use std::collections::HashMap;

/// One tick's physical values, produced by the engine model.
///
/// Field semantics follow the Speeduino `[OutputChannels]` names they feed.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ChannelValues {
    pub(crate) secl: u8,
    pub(crate) rpm: i32,
    pub(crate) map_kpa: i32,
    pub(crate) baro_kpa: i32,
    /// Coolant temperature in °C (the raw sensor channel adds the
    /// firmware's +40 offset).
    pub(crate) coolant_c: i32,
    /// Intake air temperature in °C (same +40 raw convention).
    pub(crate) iat_c: i32,
    pub(crate) tps_percent: i32,
    /// Battery voltage in V × 10.
    pub(crate) battery_dv: i32,
    pub(crate) advance_deg: i32,
    pub(crate) afr_target: f64,
    /// The simulator's "measured" AFR — equal to `afr_target` until the
    /// tune's VE table disagrees with the engine's hidden true-VE surface.
    pub(crate) afr: f64,
    /// Speeduino's `egoCorrection` is 100-centered (100 = no trim); the
    /// simulator never trims, so this stays at 100.
    pub(crate) ego_correction: f64,
    pub(crate) running: bool,
    pub(crate) cranking: bool,
}

impl ChannelValues {
    /// Physical value for a named scalar channel, per the Speeduino
    /// `[OutputChannels]` naming. Unknown names are `None` — their block
    /// bytes are left untouched.
    fn scalar(&self, name: &str) -> Option<f64> {
        Some(match name {
            "secl" => f64::from(self.secl),
            "rpm" => f64::from(self.rpm),
            "map" => f64::from(self.map_kpa),
            "baro" => f64::from(self.baro_kpa),
            // Raw temperature sensors carry the firmware's +40 °C offset
            // (the INI pairs them with `{ raw - 40 }` computed channels).
            "coolantRaw" => f64::from(self.coolant_c + 40),
            "iatRaw" => f64::from(self.iat_c + 40),
            "coolant" => f64::from(self.coolant_c),
            "iat" => f64::from(self.iat_c),
            "tps" | "throttle" => f64::from(self.tps_percent),
            "batteryVoltage" => f64::from(self.battery_dv) / 10.0,
            "advance" => f64::from(self.advance_deg),
            "afrTarget" => self.afr_target,
            "afr" => self.afr,
            "egoCorrection" => self.ego_correction,
            // Speeduino `engine` bitfield: BIT_ENGINE_RUN=0, BIT_ENGINE_CRANK=1.
            "engine" => f64::from(u8::from(self.running) | (u8::from(self.cranking) << 1)),
            _ => return None,
        })
    }

    /// Value for a named bits channel (a flag view over an existing byte).
    fn bits(&self, name: &str) -> Option<u64> {
        Some(match name {
            "running" => u64::from(self.running),
            "crank" | "cranking" => u64::from(self.cranking),
            _ => return None,
        })
    }
}

/// Encode every recognized channel into `block` at its INI-declared offset
/// and type. Pure serialization of an already-sampled snapshot — safe to
/// re-run without advancing the model.
pub(crate) fn encode_channels(
    channels: &HashMap<String, OutputChannel>,
    endian: Endianness,
    values: &ChannelValues,
    block: &mut [u8],
) {
    for channel in channels.values() {
        // Computed channels are client-side expressions over the scalar
        // bytes — nothing of theirs exists on the wire.
        if channel.expression.is_some() {
            continue;
        }
        let offset = channel.offset as usize;
        if channel.data_type == DataType::Bits {
            let (Some(lo), Some(count)) = (channel.bit_position, channel.bit_count) else {
                continue;
            };
            if count == 0 {
                continue;
            }
            if let Some(value) = values.bits(&channel.name) {
                write_bits(block, offset, lo, lo + count - 1, value);
            }
            continue;
        }
        if let Some(physical) = values.scalar(&channel.name) {
            // Exact inverse of OutputChannel::raw_to_display
            // (`physical = raw * scale + translate`).
            let raw = if channel.scale != 0.0 {
                ((physical - channel.translate) / channel.scale).round()
            } else {
                0.0
            };
            write_scalar(block, offset, channel.data_type, endian, raw);
        }
    }
}

/// Block size: the INI's `ochBlockSize`, else — the documented fallback when
/// the INI omits it — the smallest block covering every declared channel.
pub(crate) fn block_size(def: &EcuDefinition) -> usize {
    let declared = def.protocol.och_block_size as usize;
    if declared > 0 {
        return declared;
    }
    def.output_channels
        .values()
        .filter(|channel| channel.expression.is_none())
        .filter_map(|channel| (channel.offset as usize).checked_add(width(channel.data_type)))
        .max()
        .unwrap_or(0)
}

/// Byte width of a storage type. `String` has no fixed width here and is
/// never written by the simulator, so it reports 0.
pub(crate) fn width(kind: DataType) -> usize {
    match kind {
        DataType::U08 | DataType::S08 | DataType::Bits => 1,
        DataType::U16 | DataType::S16 => 2,
        DataType::U32 | DataType::S32 | DataType::F32 => 4,
        DataType::F64 => 8,
        DataType::String => 0,
    }
}

/// Write one raw scalar at `offset`, clamped into its storage range. An
/// offset past the block end is skipped, never a panic.
fn write_scalar(block: &mut [u8], offset: usize, kind: DataType, endian: Endianness, raw: f64) {
    let raw = if raw.is_finite() { raw } else { 0.0 };
    let bytes: Vec<u8> = match kind {
        DataType::U08 | DataType::Bits => vec![raw.clamp(0.0, f64::from(u8::MAX)) as u8],
        DataType::S08 => vec![(raw.clamp(f64::from(i8::MIN), f64::from(i8::MAX)) as i8) as u8],
        DataType::U16 => endian_bytes(
            &(raw.clamp(0.0, f64::from(u16::MAX)) as u16).to_be_bytes(),
            endian,
        ),
        DataType::S16 => endian_bytes(
            &(raw.clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16).to_be_bytes(),
            endian,
        ),
        DataType::U32 => endian_bytes(
            &(raw.clamp(0.0, f64::from(u32::MAX)) as u32).to_be_bytes(),
            endian,
        ),
        DataType::S32 => endian_bytes(
            &(raw.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32).to_be_bytes(),
            endian,
        ),
        DataType::F32 => endian_bytes(&(raw as f32).to_be_bytes(), endian),
        DataType::F64 => endian_bytes(&raw.to_be_bytes(), endian),
        DataType::String => return,
    };
    let Some(end) = offset.checked_add(bytes.len()) else {
        return;
    };
    if let Some(dst) = block.get_mut(offset..end) {
        dst.copy_from_slice(&bytes);
    }
}

/// Reorder big-endian source bytes into the target endianness.
fn endian_bytes(be: &[u8], endian: Endianness) -> Vec<u8> {
    match endian {
        Endianness::Big => be.to_vec(),
        Endianness::Little => be.iter().rev().copied().collect(),
    }
}

/// Read-modify-write a bit range, preserving the neighboring bits.
///
/// `DataType::Bits` decodes from a single byte and ignores endianness (see
/// `DataType::read_from_bytes`), so this writes one byte to match exactly.
/// Malformed ranges and out-of-block offsets are skipped.
fn write_bits(block: &mut [u8], offset: usize, bit_lo: u8, bit_hi: u8, value: u64) {
    if bit_lo > bit_hi || bit_hi >= 8 {
        return;
    }
    let Some(byte) = block.get_mut(offset) else {
        return;
    };
    let mask = (1u8 << (bit_hi - bit_lo + 1)) - 1;
    *byte = (*byte & !(mask << bit_lo)) | (((value as u8) & mask) << bit_lo);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_channel(name: &str, data_type: DataType, offset: u16) -> OutputChannel {
        OutputChannel {
            name: name.to_string(),
            data_type,
            offset,
            scale: 1.0,
            translate: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn encoded_scalar_round_trips_through_the_ecu_definitions_own_decoder() {
        // Arrange: a channel whose scale/translate are both non-trivial, so
        // a swapped formula cannot pass by accident.
        let mut channel = scalar_channel("rpm", DataType::U16, 2);
        channel.scale = 0.5;
        channel.translate = 100.0;
        let mut channels = HashMap::new();
        channels.insert(channel.name.clone(), channel.clone());
        let values = ChannelValues {
            rpm: 3000,
            ..Default::default()
        };
        let mut block = vec![0u8; 8];

        // Act
        encode_channels(&channels, Endianness::Little, &values, &mut block);

        // Assert: the decoder must read back what the physics put in.
        let decoded = channel
            .parse(&block, Endianness::Little)
            .expect("channel decodes");
        assert_eq!(decoded, 3000.0);
    }

    #[test]
    fn bits_channel_round_trips_and_preserves_neighboring_bits() {
        let mut channel = scalar_channel("running", DataType::Bits, 0);
        channel.bit_position = Some(3);
        channel.bit_count = Some(1);
        let mut channels = HashMap::new();
        channels.insert(channel.name.clone(), channel.clone());
        let values = ChannelValues {
            running: true,
            ..Default::default()
        };
        let mut block = vec![0b1010_0101u8];

        encode_channels(&channels, Endianness::Little, &values, &mut block);

        assert_eq!(block[0], 0b1010_1101, "only bit 3 may change");
        assert_eq!(channel.parse(&block, Endianness::Little), Some(1.0));
    }

    #[test]
    fn out_of_block_offsets_are_skipped_rather_than_panicking() {
        let mut channels = HashMap::new();
        channels.insert("rpm".to_string(), scalar_channel("rpm", DataType::U16, 7));
        let values = ChannelValues {
            rpm: 1000,
            ..Default::default()
        };
        let mut block = vec![0u8; 8];

        encode_channels(&channels, Endianness::Little, &values, &mut block);

        assert_eq!(
            block,
            vec![0u8; 8],
            "a channel past the block end writes nothing"
        );
    }

    #[test]
    fn over_range_values_clamp_instead_of_wrapping() {
        let mut block = vec![0u8; 1];
        write_scalar(&mut block, 0, DataType::U08, Endianness::Little, 300.0);
        assert_eq!(block[0], 255);
    }

    #[test]
    fn big_endian_u16_writes_most_significant_byte_first() {
        let mut block = vec![0u8; 2];
        write_scalar(&mut block, 0, DataType::U16, Endianness::Big, 4_660.0);
        assert_eq!(block, vec![0x12, 0x34]);
        let mut block = vec![0u8; 2];
        write_scalar(&mut block, 0, DataType::U16, Endianness::Little, 4_660.0);
        assert_eq!(block, vec![0x34, 0x12]);
    }

    #[test]
    fn computed_channels_are_never_written_to_the_wire() {
        let mut channel = scalar_channel("rpm", DataType::U16, 0);
        channel.expression = Some("rpm * 2".to_string());
        let mut channels = HashMap::new();
        channels.insert(channel.name.clone(), channel);
        let values = ChannelValues {
            rpm: 3000,
            ..Default::default()
        };
        let mut block = vec![0u8; 4];

        encode_channels(&channels, Endianness::Little, &values, &mut block);

        assert_eq!(block, vec![0u8; 4]);
    }
}
