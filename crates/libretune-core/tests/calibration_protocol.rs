//! End-to-end tests for Speeduino sensor-calibration writes, run against an
//! in-process TCP ECU that mirrors the 202501 firmware's comms handling
//! byte-for-byte (`comms_legacy.cpp receiveCalibration()` for the legacy
//! path, `comms.cpp` case `'t'`/`'k'` for the CRC-envelope path — the same
//! sources the bench sim-ecu was built from).
//!
//! The fixture stores what a real ECU would store — including the firmware's
//! truncating °F→°C integer math and every-32nd-sample O2 decimation — so
//! the assertions check the values that would actually land in the ECU's
//! tables, not just the bytes we happened to put on the wire.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libretune_core::ini::{Endianness, ProtocolSettings};
use libretune_core::protocol::calibration::{
    calibration_crc32, firmware_stored_temperature, temperature_calibration_bins,
    O2_CALIBRATION_WIRE_BYTES, TEMP_CALIBRATION_POINTS,
};
use libretune_core::protocol::{
    CalibrationTable, Connection, ConnectionConfig, ConnectionType,
};

const SIGNATURE: &str = "speeduino 202501";

/// What the simulated firmware ended up storing, per table id.
#[derive(Default)]
struct SimState {
    /// (values as the firmware stores them, bins) per temperature table id.
    temp_tables: std::collections::HashMap<u8, (Vec<u16>, Vec<u16>)>,
    /// O2: 32 stored 8-bit values + bins.
    o2_values: Vec<u8>,
    o2_bins: Vec<u16>,
    /// Raw data bytes received per table id (for CRC bookkeeping).
    raw: std::collections::HashMap<u8, Vec<u8>>,
    /// Offsets seen in modern 't' chunks, in arrival order.
    modern_chunk_offsets: Vec<u16>,
}

/// Firmware `receiveCalibration()` temperature math, reproduced exactly:
/// i16 → /10 (trunc) → (°F−32)*5/9 (trunc) → +40 → clamp ≥ 0.
fn firmware_temp_math(raw: i16) -> u16 {
    let t = raw as i32 / 10;
    let c = ((t - 32) * 5) / 9;
    (c + 40).max(0) as u16
}

fn read_exact(stream: &mut TcpStream, n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    stream.read_exact(&mut buf).expect("sim: short read");
    buf
}

/// Legacy-protocol ECU: raw single-letter commands, no ACKs.
fn run_legacy_sim(mut stream: TcpStream, state: Arc<Mutex<SimState>>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut cmd = [0u8; 1];
    while stream.read_exact(&mut cmd).is_ok() {
        match cmd[0] {
            b'Q' => {
                stream.write_all(SIGNATURE.as_bytes()).unwrap();
            }
            b'S' => {
                stream.write_all(b"Speeduino 2025.01.4").unwrap();
            }
            b't' => {
                let table_id = read_exact(&mut stream, 1)[0];
                let mut st = state.lock().unwrap();
                if table_id == 2 {
                    // 1024 bytes, keep every 32nd (comms_legacy.cpp ~1224-1235)
                    let data = read_exact(&mut stream, 1024);
                    st.o2_values = data.iter().step_by(32).copied().collect();
                    st.o2_bins = (0..1024).step_by(32).map(|x| x as u16).collect();
                    st.raw.insert(table_id, data);
                } else {
                    // 32 × 16-bit LE (comms_legacy.cpp ~1240-1258)
                    let data = read_exact(&mut stream, 64);
                    let mut values = Vec::with_capacity(32);
                    let mut bins = Vec::with_capacity(32);
                    for x in 0..32usize {
                        let raw = i16::from_le_bytes([data[2 * x], data[2 * x + 1]]);
                        values.push(firmware_temp_math(raw));
                        bins.push((x as u16) * 32);
                    }
                    st.temp_tables.insert(table_id, (values, bins));
                    st.raw.insert(table_id, data);
                }
            }
            // Anything else: consumed silently, like the firmware default arm.
            _ => {}
        }
    }
}

/// CRC-envelope ECU: 2-byte BE length, payload, 4-byte BE CRC32 (msEnvelope,
/// as `Packet::to_bytes()` frames it and `comms.cpp` consumes it).
fn run_modern_sim(mut stream: TcpStream, state: Arc<Mutex<SimState>>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    let send = |stream: &mut TcpStream, payload: &[u8]| {
        let mut frame = Vec::with_capacity(payload.len() + 6);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend_from_slice(payload);
        frame.extend_from_slice(&calibration_crc32(payload).to_be_bytes());
        stream.write_all(&frame).unwrap();
    };

    let mut len_buf = [0u8; 2];
    while stream.read_exact(&mut len_buf).is_ok() {
        let len = u16::from_be_bytes(len_buf) as usize;
        let payload = read_exact(&mut stream, len);
        let crc = u32::from_be_bytes(read_exact(&mut stream, 4).try_into().unwrap());
        assert_eq!(
            crc,
            calibration_crc32(&payload),
            "sim: frame CRC mismatch from client"
        );

        match payload.first() {
            Some(b'Q') => {
                let mut reply = vec![0u8];
                reply.extend_from_slice(SIGNATURE.as_bytes());
                send(&mut stream, &reply);
            }
            Some(b't') => {
                // comms.cpp case 't': [1]=canId, [2]=table,
                // offset/length BIG-endian at [3..4]/[5..6], data at [7..].
                let table_id = payload[2];
                let offset = u16::from_be_bytes([payload[3], payload[4]]);
                let length = u16::from_be_bytes([payload[5], payload[6]]) as usize;
                let data = &payload[7..];
                assert_eq!(data.len(), length, "sim: length field vs data mismatch");

                let mut st = state.lock().unwrap();
                st.modern_chunk_offsets.push(offset);
                if table_id == 2 {
                    // loadO2CalibrationChunk(): every 32nd value kept.
                    let raw = st.raw.entry(table_id).or_default();
                    assert_eq!(raw.len(), offset as usize, "sim: out-of-order O2 chunk");
                    raw.extend_from_slice(data);
                    if raw.len() >= 1024 {
                        let raw = raw.clone();
                        st.o2_values = raw.iter().step_by(32).copied().collect();
                        st.o2_bins = (0..1024).step_by(32).map(|x| x as u16).collect();
                    }
                    send(&mut stream, &[0u8]); // SERIAL_RC_OK
                } else if length == 64 {
                    // processTemperatureCalibrationTableUpdate(): bins x*33.
                    let mut values = Vec::with_capacity(32);
                    let mut bins = Vec::with_capacity(32);
                    for x in 0..32usize {
                        let raw = i16::from_le_bytes([data[2 * x], data[2 * x + 1]]);
                        values.push(firmware_temp_math(raw));
                        bins.push((x as u16) * 33);
                    }
                    st.temp_tables.insert(table_id, (values, bins));
                    st.raw.insert(table_id, data.to_vec());
                    send(&mut stream, &[0u8]); // SERIAL_RC_OK
                } else {
                    send(&mut stream, &[0x84u8]); // SERIAL_RC_RANGE_ERR
                }
            }
            Some(b'k') => {
                // CRC of the stored calibration page = CRC32 of the raw data
                // bytes received for it (see calibration.rs docs for why both
                // firmware paths reduce to the standard CRC32).
                let table_id = payload[2];
                let st = state.lock().unwrap();
                let crc = st.raw.get(&table_id).map(|d| calibration_crc32(d));
                let mut reply = vec![0u8];
                reply.extend_from_slice(&crc.unwrap_or(0).to_be_bytes());
                send(&mut stream, &reply);
            }
            _ => send(&mut stream, &[0x83u8]), // SERIAL_RC_UKWN_ERR
        }
    }
}

/// Spin up a sim, connect a real `Connection` to it over TCP, and return both.
fn connect(modern: bool) -> (Connection, Arc<Mutex<SimState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(Mutex::new(SimState::default()));

    let sim_state = Arc::clone(&state);
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_nodelay(true).ok();
        if modern {
            run_modern_sim(stream, sim_state);
        } else {
            run_legacy_sim(stream, sim_state);
        }
    });

    let config = ConnectionConfig {
        connection_type: ConnectionType::Tcp,
        tcp_host: Some("127.0.0.1".to_string()),
        tcp_port: Some(port),
        timeout_ms: 2000,
        ..Default::default()
    };
    let settings = ProtocolSettings {
        message_envelope_format: modern.then(|| "msEnvelope_1.0".to_string()),
        query_command: "Q".to_string(),
        delay_after_port_open: 0,
        block_read_timeout: 500,
        inter_write_delay: 0,
        page_activation_delay: 0,
        ..Default::default()
    };
    let mut conn = Connection::with_protocol(config, settings, Endianness::Big);
    conn.connect().expect("connect to sim");
    assert_eq!(conn.signature(), Some(SIGNATURE));
    assert_eq!(conn.is_modern_protocol(), modern);
    (conn, state)
}

/// A plausible NTC curve sampled at the given ADC bins: hot at low ADC,
/// cold at high ADC, spanning the clamp boundary at the top end.
fn thermistor_curve(bins: &[u16; TEMP_CALIBRATION_POINTS]) -> [f64; TEMP_CALIBRATION_POINTS] {
    let mut temps = [0.0f64; TEMP_CALIBRATION_POINTS];
    for (i, &bin) in bins.iter().enumerate() {
        temps[i] = 150.0 - (bin as f64) * (195.0 / 1023.0); // 150 °C .. ≈ -45 °C
    }
    temps
}

/// The linear wideband curve from the drive-verified ground-offset fix:
/// 0 V → 9.69 AFR, 5 V → 19.80 AFR.
fn spartan_offset_curve() -> [f64; O2_CALIBRATION_WIRE_BYTES] {
    let mut afr = [0.0f64; O2_CALIBRATION_WIRE_BYTES];
    for (adc, v) in afr.iter_mut().enumerate() {
        *v = 9.69 + (adc as f64 / 1023.0) * (19.80 - 9.69);
    }
    afr
}

#[test]
fn legacy_temperature_calibration_stores_firmware_exact_values() {
    let (mut conn, state) = connect(false);
    let bins = temperature_calibration_bins(false);
    let temps = thermistor_curve(&bins);

    conn.write_temperature_calibration(CalibrationTable::Clt, &temps)
        .expect("legacy CLT write");

    let st = state.lock().unwrap();
    let (values, sim_bins) = st.temp_tables.get(&0).expect("CLT table received");
    assert_eq!(values.len(), 32);
    assert_eq!(sim_bins.as_slice(), &bins[..], "legacy bins are x*32");

    // The encoder pre-compensates the firmware's truncating integer math, so
    // every stored value must land within quantization (±0.5 °C) of the
    // requested temperature.
    for (i, &stored) in values.iter().enumerate() {
        let requested_c = temps[i];
        let stored_c = stored as i32 - 40;
        assert!(
            (stored_c as f64 - requested_c).abs() <= 0.5 + 1e-9,
            "bin {i}: requested {requested_c:.2} °C, firmware stored {stored_c} °C"
        );
    }
}

#[test]
fn legacy_o2_calibration_streams_1024_bytes_and_firmware_keeps_every_32nd() {
    let (mut conn, state) = connect(false);
    let afr = spartan_offset_curve();

    conn.write_o2_calibration(&afr).expect("legacy O2 write");

    let st = state.lock().unwrap();
    assert_eq!(st.raw.get(&2).map(|d| d.len()), Some(1024));
    assert_eq!(st.o2_values.len(), 32);
    assert_eq!(st.o2_bins[1], 32);
    for (i, &stored) in st.o2_values.iter().enumerate() {
        let adc = i * 32;
        let expected = (afr[adc] * 10.0).round() as u8;
        assert_eq!(
            stored, expected,
            "ADC {adc}: firmware kept {stored}, expected AFR×10 = {expected}"
        );
    }
    // Endpoint sanity for the ground-offset use case: 0 V reads 9.7. The
    // firmware's last kept sample is ADC 992 (not 1023), so the top stored
    // value is the curve at 992/1023 of full scale: 19.49 → 195.
    assert_eq!(st.o2_values[0], 97);
    assert_eq!(*st.o2_values.last().unwrap(), 195);
}

#[test]
fn legacy_has_no_calibration_crc_command() {
    let (mut conn, _state) = connect(false);
    assert!(conn.read_calibration_crc(CalibrationTable::O2).is_err());
}

#[test]
fn modern_temperature_calibration_single_64_byte_chunk_verified_by_crc() {
    let (mut conn, state) = connect(true);
    let bins = temperature_calibration_bins(true);
    let temps = thermistor_curve(&bins);

    // write_temperature_calibration internally verifies via 'k'; an Ok here
    // means the ECU-stored CRC matched the bytes we sent.
    conn.write_temperature_calibration(CalibrationTable::Iat, &temps)
        .expect("modern IAT write + CRC verify");

    let st = state.lock().unwrap();
    assert_eq!(st.modern_chunk_offsets, vec![0], "one chunk at offset 0");
    let (values, sim_bins) = st.temp_tables.get(&1).expect("IAT table received");
    assert_eq!(sim_bins.as_slice(), &bins[..], "modern bins are x*33");
    for (i, &stored) in values.iter().enumerate() {
        // The client encoded °F×10; the fixture applied the firmware's exact
        // conversion. Cross-check against the shared helper.
        let wire = i16::from_le_bytes([
            st.raw[&1][2 * i],
            st.raw[&1][2 * i + 1],
        ]);
        assert_eq!(stored, firmware_stored_temperature(wire));
    }
}

#[test]
fn modern_o2_calibration_goes_out_as_four_256_byte_chunks() {
    let (mut conn, state) = connect(true);
    let afr = spartan_offset_curve();

    conn.write_o2_calibration(&afr).expect("modern O2 write");

    let st = state.lock().unwrap();
    assert_eq!(
        st.modern_chunk_offsets,
        vec![0, 256, 512, 768],
        "firmware burns EEPROM only when the offset reaches 1023, so the \
         table must arrive as exactly these chunks"
    );
    assert_eq!(st.o2_values.len(), 32);
    assert_eq!(st.o2_values[0], 97);
}
