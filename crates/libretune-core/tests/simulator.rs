//! End-to-end checks that the virtual ECU is reachable through the ordinary
//! `Connection` path, driven by the real INI demo mode ships with.

use libretune_core::ini::EcuDefinition;
use libretune_core::protocol::{Connection, ConnectionConfig};
use libretune_core::simulator::{EcuSimulator, EngineMode};
use std::path::PathBuf;
use std::time::Duration;

/// The INI demo mode actually loads, so these tests fail if the simulator
/// stops coping with a real-world definition rather than a tidy fixture.
fn demo_definition() -> EcuDefinition {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../libretune-app/src-tauri/resources/demo.ini");
    EcuDefinition::from_file(&path).expect("the bundled demo INI parses")
}

fn connect(def: &EcuDefinition) -> Connection {
    let simulator = EcuSimulator::from_definition(def);
    let mut connection = Connection::with_protocol(
        ConnectionConfig::default(),
        def.protocol.clone(),
        def.endianness,
    );
    connection
        .connect_with_channel(Box::new(simulator.channel()))
        .expect("the simulator completes the handshake");
    connection
}

#[test]
fn a_connection_handshakes_against_the_simulator_and_reports_its_signature() {
    let def = demo_definition();

    let connection = connect(&def);

    assert_eq!(
        connection.signature(),
        Some(def.signature.as_str()),
        "the simulator identifies as the ECU its definition describes"
    );
}

#[test]
fn a_connection_reads_the_whole_realtime_block_the_way_the_app_does() {
    // Driven through Connection rather than raw bytes: the demo INI asks for
    // realtime with `ochGetCommand = "O%2o%2c"`, and a simulator that only
    // answered the other realtime command would hand back a single byte here
    // while every raw-bytes test still passed.
    let def = demo_definition();
    let mut connection = connect(&def);

    let block = connection
        .get_realtime_data()
        .expect("the simulator answers the INI's own realtime command");

    assert_eq!(
        block.len(),
        def.protocol.och_block_size as usize,
        "a short frame means the realtime command was not understood"
    );
}

#[test]
fn a_connection_reads_back_a_page_it_wrote() {
    let def = demo_definition();
    let mut connection = connect(&def);
    let page = 0u8;
    let original = connection.read_page(page).expect("page reads");
    let mut changed = original.clone();
    changed[0] = original[0].wrapping_add(1);

    connection.write_page(page, &changed).expect("page writes");

    assert_eq!(
        connection.read_page(page).expect("page reads back"),
        changed,
        "the simulator must store what was written to it"
    );
}

#[test]
fn the_realtime_block_animates_over_time() {
    let def = demo_definition();
    let simulator = EcuSimulator::from_definition(&def);

    simulator.tick_engine(Duration::from_millis(50));
    let mut channel = simulator.channel();
    let first = read_realtime(&mut channel);
    simulator.tick_engine(Duration::from_secs(3));
    let later = read_realtime(&mut channel);

    assert_ne!(
        first, later,
        "three seconds of engine time must change the frame"
    );
}

#[test]
fn a_wide_open_throttle_pull_raises_rpm() {
    let def = demo_definition();
    let simulator = EcuSimulator::from_definition(&def);
    let rpm = def
        .output_channels
        .get("RPMValue")
        .expect("the demo INI declares an rpm channel")
        .clone();

    simulator.tick_engine(Duration::from_secs(2));
    let mut channel = simulator.channel();
    let idle = rpm
        .parse(&read_realtime(&mut channel), def.endianness)
        .expect("rpm decodes");

    simulator.set_mode(EngineMode::Wot);
    simulator.tick_engine(Duration::from_secs(2));
    let pulling = rpm
        .parse(&read_realtime(&mut channel), def.endianness)
        .expect("rpm decodes");

    assert!(
        pulling > idle,
        "WOT must pull rpm up from idle: {idle} -> {pulling}"
    );
}

/// Read the whole output-channel block off the simulator with one `r`
/// request, the way the realtime stream does.
fn read_realtime(channel: &mut libretune_core::simulator::SimulatorChannel) -> Vec<u8> {
    use std::io::{Read, Write};
    let len: u16 = 64;
    let mut request = vec![b'r', 0, 0x30, 0, 0];
    request.extend(len.to_le_bytes());
    channel.write_all(&request).expect("request is accepted");
    let mut block = vec![0u8; len as usize];
    channel.read_exact(&mut block).expect("the block arrives");
    block
}

#[test]
fn every_page_the_demo_ini_declares_is_readable_and_writable() {
    // The INI's `pageIdentifier` entries are literal byte strings — `\x00\x00`,
    // `\x00\x01`, `\x00\x02` — not page indices. Reading the identifier as a
    // number and truncating it to a `u8` left every page but the first
    // unreachable: reads came back blank and writes were acknowledged
    // without storing anything.
    let def = demo_definition();
    let mut connection = connect(&def);
    assert!(def.n_pages > 1, "the demo INI must declare several pages");

    for page in 0..def.n_pages as u8 {
        let original = connection
            .read_page(page)
            .unwrap_or_else(|e| panic!("page {page} reads: {e}"));
        assert_eq!(
            original.len(),
            def.page_sizes[page as usize] as usize,
            "page {page} came back the wrong size"
        );

        let mut changed = original.clone();
        changed[0] = original[0].wrapping_add(1);
        let last = changed.len() - 1;
        changed[last] = original[last].wrapping_add(1);
        connection
            .write_page(page, &changed)
            .unwrap_or_else(|e| panic!("page {page} writes: {e}"));

        assert_eq!(
            connection.read_page(page).expect("page reads back"),
            changed,
            "page {page} did not store what was written to it"
        );
    }
}

#[test]
fn a_little_endian_framed_request_is_answered_in_the_same_order() {
    // Speeduino frames its envelopes little-endian. Assuming big-endian
    // parses a payload length of 1 as 256 and waits for bytes that never
    // come, deadlocking the handshake in mutual timeouts.
    use libretune_core::protocol::{EnvelopeOrder, Packet};
    use std::io::{Read, Write};

    let def = demo_definition();
    let simulator = EcuSimulator::from_definition(&def);
    let mut channel = simulator.channel();
    let query = def.protocol.query_command.as_bytes().to_vec();

    let request = Packet::new(query).to_bytes_ordered(EnvelopeOrder::LittleEndian);
    channel.write_all(&request).expect("request is accepted");

    let mut response = vec![0u8; 256];
    let read = channel.read(&mut response).expect("a reply is queued");
    response.truncate(read);
    let packet = Packet::from_bytes_ordered(&response, EnvelopeOrder::LittleEndian, false)
        .expect("the reply is framed in the order the request used");
    assert_eq!(
        String::from_utf8_lossy(&packet.payload[1..]).trim(),
        def.signature,
        "a little-endian handshake must return the signature"
    );
}

#[test]
fn a_frame_whose_length_byte_looks_like_a_command_is_not_split() {
    // A big-endian payload of 0x4100 bytes puts `0x41` — ASCII 'A', the
    // burst realtime command — in the length field's high byte. Deciding
    // "frame or bare command" from that byte consumes it as a command and
    // glues the rest of the frame onto whatever arrives next.
    use libretune_core::protocol::{EnvelopeOrder, Packet};
    use std::io::{Read, Write};

    let def = demo_definition();
    let simulator = EcuSimulator::from_definition(&def);
    let mut channel = simulator.channel();

    // Latch the session into framed mode the way a real handshake does.
    let handshake = Packet::new(def.protocol.query_command.as_bytes().to_vec())
        .to_bytes_ordered(EnvelopeOrder::BigEndian);
    channel.write_all(&handshake).expect("handshake accepted");
    let mut drain = vec![0u8; 256];
    let _ = channel.read(&mut drain).expect("handshake answered");

    // `C` + %2i + %2o + %2c + payload, sized so the whole frame payload is
    // exactly 0x4100 bytes.
    let count: u16 = 0x4100 - 7;
    let mut payload = vec![b'C'];
    payload.extend(0u16.to_le_bytes()); // page identifier 0
    payload.extend(0u16.to_le_bytes()); // offset
    payload.extend(count.to_le_bytes());
    payload.extend(std::iter::repeat_n(0x5Au8, count as usize));
    assert_eq!(payload.len(), 0x4100);

    let frame = Packet::new(payload).to_bytes_ordered(EnvelopeOrder::BigEndian);
    assert_eq!(frame[0], b'A', "the length high byte must be the collision");

    // Split mid-frame: the simulator must wait rather than dispatch 'A'.
    let (head, tail) = frame.split_at(16);
    channel.write_all(head).expect("first chunk accepted");
    let mut early = vec![0u8; 64];
    match channel.read(&mut early) {
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        other => panic!("an incomplete frame must not be answered yet: {other:?}"),
    }
    channel.write_all(tail).expect("second chunk accepted");

    let mut response = vec![0u8; 64];
    let read = channel.read(&mut response).expect("the frame is answered");
    response.truncate(read);
    let packet = Packet::from_bytes_ordered(&response, EnvelopeOrder::BigEndian, false)
        .expect("one well-formed reply, not a fragment");
    assert_eq!(packet.payload, vec![0x00], "the write is acknowledged");
}

#[test]
fn a_frame_with_a_broken_crc_is_refused_without_touching_memory() {
    // Discarding the CRC and dispatching anyway means a corrupted page write
    // mutates RAM and is answered with success — the one failure a
    // simulator-backed test suite exists to catch.
    use libretune_core::protocol::{EnvelopeOrder, Packet};
    use std::io::{Read, Write};

    let def = demo_definition();
    let simulator = EcuSimulator::from_definition(&def);
    let mut channel = simulator.channel();

    let read_first_byte = |channel: &mut libretune_core::simulator::SimulatorChannel| -> u8 {
        let mut request = vec![b'R'];
        request.extend(0u16.to_le_bytes());
        request.extend(0u16.to_le_bytes());
        request.extend(1u16.to_le_bytes());
        let frame = Packet::new(request).to_bytes_ordered(EnvelopeOrder::BigEndian);
        channel.write_all(&frame).expect("read request accepted");
        let mut buf = vec![0u8; 64];
        let n = channel.read(&mut buf).expect("read answered");
        buf.truncate(n);
        let packet = Packet::from_bytes_ordered(&buf, EnvelopeOrder::BigEndian, false)
            .expect("read reply is a frame");
        packet.payload[1]
    };

    let before = read_first_byte(&mut channel);

    let mut write = vec![b'C'];
    write.extend(0u16.to_le_bytes());
    write.extend(0u16.to_le_bytes());
    write.extend(1u16.to_le_bytes());
    write.push(before.wrapping_add(0x37));
    let mut frame = Packet::new(write).to_bytes_ordered(EnvelopeOrder::BigEndian);
    let last = frame.len() - 1;
    frame[last] ^= 0xFF;

    channel.write_all(&frame).expect("frame accepted");
    let mut buf = vec![0u8; 64];
    let n = channel.read(&mut buf).expect("a refusal comes back");
    buf.truncate(n);
    let packet = Packet::from_bytes_ordered(&buf, EnvelopeOrder::BigEndian, false)
        .expect("the refusal is itself well framed");
    assert_eq!(packet.payload, vec![0x83], "a CRC error must be reported");

    assert_eq!(
        read_first_byte(&mut channel),
        before,
        "a frame that failed its CRC must not have written anything"
    );
}

#[test]
fn the_simulated_lambda_lands_in_a_range_autotune_will_accept() {
    // A blank veTable decodes into a structurally valid but meaningless
    // context, which drove `current_ve` to 1 and lambda to tens — the
    // U16 x 1e-4 channel then clamped at 6.5535 and AutoTune, reading
    // anything above 2 as an AFR, rejected every sample below its 10.0 rail.
    let def = demo_definition();
    let simulator = EcuSimulator::from_definition(&def);
    let lambda = def
        .output_channels
        .get("lambdaValue")
        .expect("the demo INI declares lambdaValue")
        .clone();

    simulator.tick_engine(Duration::from_secs(2));
    let mut channel = simulator.channel();
    let value = lambda
        .parse(&read_realtime_block(&mut channel, &def), def.endianness)
        .expect("lambda decodes");

    assert!(
        (0.5..1.6).contains(&value),
        "lambda must be a plausible mixture, got {value}"
    );
}

/// Read the whole declared realtime block, not just its first 64 bytes.
fn read_realtime_block(
    channel: &mut libretune_core::simulator::SimulatorChannel,
    def: &EcuDefinition,
) -> Vec<u8> {
    use std::io::{Read, Write};
    // `%2c` is two bytes on the wire; `och_block_size` is a u32, and sending
    // it whole appends two stray bytes the ECU never asked for.
    let len = u16::try_from(def.protocol.och_block_size).expect("the demo block fits a u16");
    let mut request = vec![b'r', 0, 0x30, 0, 0];
    request.extend(len.to_le_bytes());
    channel.write_all(&request).expect("request is accepted");
    let mut block = vec![0u8; len as usize];
    channel.read_exact(&mut block).expect("the block arrives");
    block
}
