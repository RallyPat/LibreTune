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
