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
