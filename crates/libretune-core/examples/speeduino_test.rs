//! Speeduino ECU Communication Test Tool
//!
//! A standalone tool to test and debug serial communication with Speeduino ECUs.
//! Tests both legacy (ASCII) and modern (CRC32) protocols with configurable timing.
//!
//! Usage:
//!   cargo run --example speeduino_test -- [OPTIONS] [PORT]
//!
//! Options:
//!   --port PORT       Serial port (default: /dev/ttyACM0)
//!   --baud RATE       Baud rate (default: 115200)
//!   --delay MS        Delay after port open in ms (default: 1000)
//!   --timeout MS      Read timeout in ms (default: 2000)
//!   --legacy-only     Only test legacy protocol
//!   --crc-only        Only test CRC protocol
//!   --no-dtr          Don't set DTR high (for testing)
//!   --fast            Use minimal delays for speed testing

use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let mut port_name = "/dev/ttyACM0".to_string();
    let mut baud_rate = 115200u32;
    let mut delay_after_open = 1000u64;
    let mut timeout_ms = 2000u64;
    let mut test_legacy = true;
    let mut test_crc = true;
    let mut set_dtr = true;
    let mut fast_mode = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                i += 1;
                if i < args.len() {
                    port_name = args[i].clone();
                }
            }
            "--baud" | "-b" => {
                i += 1;
                if i < args.len() {
                    baud_rate = args[i].parse().unwrap_or(115200);
                }
            }
            "--delay" | "-d" => {
                i += 1;
                if i < args.len() {
                    delay_after_open = args[i].parse().unwrap_or(1000);
                }
            }
            "--timeout" | "-t" => {
                i += 1;
                if i < args.len() {
                    timeout_ms = args[i].parse().unwrap_or(2000);
                }
            }
            "--legacy-only" => {
                test_crc = false;
            }
            "--crc-only" => {
                test_legacy = false;
            }
            "--no-dtr" => {
                set_dtr = false;
            }
            "--fast" => {
                fast_mode = true;
                delay_after_open = 100;
                timeout_ms = 500;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            arg if !arg.starts_with('-') => {
                port_name = arg.to_string();
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
            }
        }
        i += 1;
    }

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Speeduino ECU Communication Test Tool              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Port:              {}", port_name);
    println!("  Baud rate:         {}", baud_rate);
    println!("  Delay after open:  {}ms", delay_after_open);
    println!("  Read timeout:      {}ms", timeout_ms);
    println!("  Set DTR high:      {}", set_dtr);
    println!("  Test legacy:       {}", test_legacy);
    println!("  Test CRC:          {}", test_crc);
    println!("  Fast mode:         {}", fast_mode);
    println!();

    // Open port
    println!("Opening serial port...");
    let mut port = match serialport::new(&port_name, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to open port: {}", e);
            eprintln!("   Make sure the port exists and you have permission (dialout group)");
            return;
        }
    };
    println!("✓ Port opened");

    // Configure port
    if let Err(e) = configure_port(port.as_mut(), set_dtr) {
        eprintln!("❌ Failed to configure port: {}", e);
        return;
    }
    println!("✓ Port configured (8N1, no flow control)");

    // Clear buffers
    if let Err(e) = port.clear(serialport::ClearBuffer::All) {
        eprintln!("⚠ Failed to clear buffers: {}", e);
    }

    // Wait for ECU stabilization
    println!("Waiting {}ms for ECU stabilization...", delay_after_open);
    std::thread::sleep(Duration::from_millis(delay_after_open));

    // Clear buffers again
    let _ = port.clear(serialport::ClearBuffer::All);
    std::thread::sleep(Duration::from_millis(50));

    println!();
    println!("═══════════════════════════════════════════════════════════════");

    // Test legacy protocol
    if test_legacy {
        println!();
        println!("📡 Testing LEGACY protocol (raw ASCII commands)...");
        println!();

        // Test 'Q' command (query signature)
        test_legacy_command(&mut port, b'Q', "Query signature", timeout_ms);

        // Test 'F' command (protocol version - always allowed)
        test_legacy_command(&mut port, b'F', "Protocol version", timeout_ms);

        // Test 'S' command (signature - rusEFI style)
        test_legacy_command(&mut port, b'S', "Signature (rusEFI)", timeout_ms);
    }

    // Test CRC protocol
    if test_crc {
        println!();
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        println!("📡 Testing CRC protocol (msEnvelope_1.0)...");
        println!();

        // Test 'Q' command with CRC wrapper
        test_crc_command(&mut port, &[b'Q'], "Query signature (CRC)", timeout_ms);

        // Test 'F' command with CRC wrapper
        test_crc_command(&mut port, &[b'F'], "Protocol version (CRC)", timeout_ms);

        // Test 'S' command with CRC wrapper
        test_crc_command(&mut port, &[b'S'], "Signature (CRC)", timeout_ms);
    }

    // Dynamic timing test
    if !fast_mode && test_legacy {
        println!();
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        println!("⏱  Testing dynamic timing (finding minimum working delay)...");
        println!();

        test_dynamic_timing(&mut port, timeout_ms);
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("                         Tests Complete                         ");
    println!("═══════════════════════════════════════════════════════════════");
}

fn print_help() {
    println!("Speeduino ECU Communication Test Tool");
    println!();
    println!("Usage: speeduino_test [OPTIONS] [PORT]");
    println!();
    println!("Options:");
    println!("  --port, -p PORT     Serial port (default: /dev/ttyACM0)");
    println!("  --baud, -b RATE     Baud rate (default: 115200)");
    println!("  --delay, -d MS      Delay after port open (default: 1000)");
    println!("  --timeout, -t MS    Read timeout (default: 2000)");
    println!("  --legacy-only       Only test legacy protocol");
    println!("  --crc-only          Only test CRC protocol");
    println!("  --no-dtr            Don't set DTR high");
    println!("  --fast              Minimal delays for speed testing");
    println!("  --help, -h          Show this help");
}

fn configure_port(port: &mut dyn SerialPort, set_dtr: bool) -> Result<(), String> {
    port.set_data_bits(serialport::DataBits::Eight)
        .map_err(|e| e.to_string())?;
    port.set_parity(serialport::Parity::None)
        .map_err(|e| e.to_string())?;
    port.set_stop_bits(serialport::StopBits::One)
        .map_err(|e| e.to_string())?;
    port.set_flow_control(serialport::FlowControl::None)
        .map_err(|e| e.to_string())?;

    if set_dtr {
        // Set DTR high to maintain connection and prevent Arduino reset
        if let Err(e) = port.write_data_terminal_ready(true) {
            eprintln!("  ⚠ Failed to set DTR: {} (continuing anyway)", e);
        } else {
            println!("  ✓ DTR set high");
        }

        // Set RTS high for proper signaling
        if let Err(e) = port.write_request_to_send(true) {
            eprintln!("  ⚠ Failed to set RTS: {} (continuing anyway)", e);
        } else {
            println!("  ✓ RTS set high");
        }
    }

    Ok(())
}

fn test_legacy_command(
    port: &mut Box<dyn SerialPort>,
    cmd: u8,
    description: &str,
    timeout_ms: u64,
) {
    println!(
        "  Testing '{}' (0x{:02X}) - {}...",
        cmd as char, cmd, description
    );

    // Clear buffers
    let _ = port.clear(serialport::ClearBuffer::All);
    std::thread::sleep(Duration::from_millis(10));

    // Send command
    let start = Instant::now();
    if let Err(e) = port.write_all(&[cmd]) {
        println!("    ❌ Write failed: {}", e);
        return;
    }
    if let Err(e) = port.flush() {
        println!("    ❌ Flush failed: {}", e);
        return;
    }

    // Read response
    let response = read_with_timeout(port, timeout_ms);
    let elapsed = start.elapsed();

    if response.is_empty() {
        println!("    ❌ No response (timeout after {:?})", elapsed);
    } else {
        let as_string = String::from_utf8_lossy(&response);
        println!("    ✅ Got {} bytes in {:?}", response.len(), elapsed);
        println!("       Hex:    {:02x?}", response);
        println!("       String: {:?}", as_string.trim());
    }
}

fn test_crc_command(
    port: &mut Box<dyn SerialPort>,
    payload: &[u8],
    description: &str,
    timeout_ms: u64,
) {
    println!("  Testing {:02x?} - {}...", payload, description);

    // Build CRC packet
    let packet = build_crc_packet(payload);
    println!("    Sending packet: {:02x?}", packet);

    // Clear buffers
    let _ = port.clear(serialport::ClearBuffer::All);
    std::thread::sleep(Duration::from_millis(10));

    // Send packet
    let start = Instant::now();
    if let Err(e) = port.write_all(&packet) {
        println!("    ❌ Write failed: {}", e);
        return;
    }
    if let Err(e) = port.flush() {
        println!("    ❌ Flush failed: {}", e);
        return;
    }

    // Read response header (2 bytes length)
    let mut header = [0u8; 2];
    match read_exact_with_timeout(port, &mut header, timeout_ms) {
        Ok(elapsed_header) => {
            let length = u16::from_be_bytes(header) as usize;
            println!(
                "    Got header in {:?}: length = {}",
                elapsed_header, length
            );

            if length > 1024 {
                println!("    ❌ Invalid length (too large)");
                return;
            }

            // Read payload + CRC (4 bytes)
            let mut payload_and_crc = vec![0u8; length + 4];
            match read_exact_with_timeout(port, &mut payload_and_crc, timeout_ms) {
                Ok(_elapsed_payload) => {
                    let elapsed = start.elapsed();
                    let payload_data = &payload_and_crc[..length];
                    let crc_bytes = &payload_and_crc[length..];

                    // Verify CRC
                    let received_crc = u32::from_be_bytes([
                        crc_bytes[0],
                        crc_bytes[1],
                        crc_bytes[2],
                        crc_bytes[3],
                    ]);
                    let expected_crc = calculate_crc(payload_data);

                    if received_crc == expected_crc {
                        let as_string = String::from_utf8_lossy(payload_data);
                        println!("    ✅ Got {} bytes in {:?} (CRC valid)", length, elapsed);
                        println!("       Payload: {:02x?}", payload_data);
                        println!("       String:  {:?}", as_string.trim());
                    } else {
                        println!("    ⚠ Got {} bytes but CRC mismatch!", length);
                        println!("       Expected CRC: 0x{:08X}", expected_crc);
                        println!("       Received CRC: 0x{:08X}", received_crc);
                        println!("       Payload: {:02x?}", payload_data);
                    }
                }
                Err(e) => {
                    println!(
                        "    ❌ Failed to read payload: {} (after {:?})",
                        e,
                        start.elapsed()
                    );
                }
            }
        }
        Err(e) => {
            // Maybe it responded in legacy mode?
            let elapsed = start.elapsed();
            println!("    ❌ No CRC header received: {} (after {:?})", e, elapsed);

            // Check if there's any data at all (legacy response?)
            std::thread::sleep(Duration::from_millis(100));
            let available = port.bytes_to_read().unwrap_or(0);
            if available > 0 {
                let mut buf = vec![0u8; available as usize];
                if let Ok(n) = port.read(&mut buf) {
                    println!(
                        "    ℹ Found {} bytes (maybe legacy response?): {:02x?}",
                        n,
                        &buf[..n]
                    );
                    println!("       As string: {:?}", String::from_utf8_lossy(&buf[..n]));
                }
            }
        }
    }
}

fn test_dynamic_timing(port: &mut Box<dyn SerialPort>, base_timeout_ms: u64) {
    let delays = [0, 10, 25, 50, 100, 200, 500, 1000];

    for delay in delays {
        println!("  Testing with {}ms pre-send delay...", delay);

        // Clear and wait
        let _ = port.clear(serialport::ClearBuffer::All);
        std::thread::sleep(Duration::from_millis(delay));

        // Send 'Q' command
        let start = Instant::now();
        if port.write_all(&[b'Q']).is_err() {
            println!("    ❌ Write failed");
            continue;
        }
        let _ = port.flush();

        // Read response
        let response = read_with_timeout(port, base_timeout_ms);
        let elapsed = start.elapsed();

        if response.is_empty() {
            println!("    ❌ No response");
        } else if response.starts_with(b"speeduino") {
            println!("    ✅ Success! {} bytes in {:?}", response.len(), elapsed);
            println!("       → Minimum working delay: {}ms", delay);
            return;
        } else {
            println!(
                "    ⚠ Got {} bytes but unexpected: {:?}",
                response.len(),
                String::from_utf8_lossy(&response)
            );
        }

        // Small delay between tests
        std::thread::sleep(Duration::from_millis(100));
    }

    println!("  ⚠ Could not find working delay");
}

fn read_with_timeout(port: &mut Box<dyn SerialPort>, timeout_ms: u64) -> Vec<u8> {
    let mut response = Vec::new();
    let mut buffer = [0u8; 256];
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let inter_char_timeout = Duration::from_millis(100);
    let mut last_data_time = Instant::now();

    loop {
        if start.elapsed() > timeout {
            break;
        }

        match port.bytes_to_read() {
            Ok(available) if available > 0 => {
                let to_read = std::cmp::min(available as usize, buffer.len());
                match port.read(&mut buffer[..to_read]) {
                    Ok(n) if n > 0 => {
                        response.extend_from_slice(&buffer[..n]);
                        last_data_time = Instant::now();
                    }
                    _ => {}
                }
            }
            Ok(_) => {
                if !response.is_empty() && last_data_time.elapsed() > inter_char_timeout {
                    // No more data coming
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    response
}

fn read_exact_with_timeout(
    port: &mut Box<dyn SerialPort>,
    buf: &mut [u8],
    timeout_ms: u64,
) -> Result<Duration, String> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut offset = 0;

    while offset < buf.len() {
        if start.elapsed() > timeout {
            return Err(format!("timeout after {} of {} bytes", offset, buf.len()));
        }

        match port.bytes_to_read() {
            Ok(available) if available > 0 => {
                let to_read = std::cmp::min(available as usize, buf.len() - offset);
                match port.read(&mut buf[offset..offset + to_read]) {
                    Ok(n) if n > 0 => {
                        offset += n;
                    }
                    Ok(_) => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(_) => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(start.elapsed())
}

fn build_crc_packet(payload: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(2 + payload.len() + 4);

    // Length (2 bytes, big-endian)
    let len = payload.len() as u16;
    packet.push((len >> 8) as u8);
    packet.push((len & 0xFF) as u8);

    // Payload
    packet.extend_from_slice(payload);

    // CRC32 (4 bytes, big-endian)
    let crc = calculate_crc(payload);
    packet.push((crc >> 24) as u8);
    packet.push((crc >> 16) as u8);
    packet.push((crc >> 8) as u8);
    packet.push(crc as u8);

    packet
}

fn calculate_crc(data: &[u8]) -> u32 {
    // Use crc32fast for consistent CRC calculation
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}
