//! Connection management
//!
//! Handles the connection lifecycle and command execution with the ECU.

use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use super::{
    commands::{BurnParams, ReadMemoryParams, WriteMemoryParams},
    serial::{clear_buffers, configure_port, list_ports, open_port, PortInfo},
    Command, CommandBuilder, Packet, ProtocolError, DEFAULT_BAUD_RATE, DEFAULT_TIMEOUT_MS,
};
use crate::ini::{AdaptiveTiming, AdaptiveTimingConfig, Endianness, ProtocolSettings};

/// Parse a command string with escape sequences into raw bytes
/// Handles: \xNN (hex), \n, \r, \t, \\, \0, and regular characters
fn parse_command_string(s: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'x' | b'X' => {
                    // Hex escape: \xNN
                    if i + 3 < bytes.len() {
                        if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 2..i + 4]) {
                            if let Ok(byte_val) = u8::from_str_radix(hex_str, 16) {
                                result.push(byte_val);
                                i += 4;
                                continue;
                            }
                        }
                    }
                    // Invalid hex, treat as literal
                    result.push(bytes[i]);
                    i += 1;
                }
                b'n' => {
                    result.push(b'\n');
                    i += 2;
                }
                b'r' => {
                    result.push(b'\r');
                    i += 2;
                }
                b't' => {
                    result.push(b'\t');
                    i += 2;
                }
                b'\\' => {
                    result.push(b'\\');
                    i += 2;
                }
                b'0' => {
                    result.push(0);
                    i += 2;
                }
                _ => {
                    // Unknown escape, treat backslash as literal
                    result.push(bytes[i]);
                    i += 1;
                }
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    result
}

/// Write bytes to serial port and ensure they are transmitted.
/// Since the serialport crate's flush() calls tcdrain which blocks in this environment,
/// we use write_all + a calculated time delay based on baud rate.
/// The key insight is that write_all() on a serial port writes directly to the kernel
/// buffer (not userspace), so we just need to wait for the hardware to transmit.
///
/// `min_wait_ms` allows caller to specify minimum wait (for adaptive timing).
/// If None, uses a conservative 10ms minimum.
#[cfg(target_family = "unix")]
fn write_and_wait(
    port: &mut Box<dyn SerialPort>,
    data: &[u8],
    baud_rate: u32,
    min_wait_ms: Option<u64>,
) -> Result<(), std::io::Error> {
    // Write the data - this goes to the kernel's tty output buffer
    port.write_all(data)?;

    // Guard against zero baud rate
    let safe_baud = if baud_rate == 0 {
        eprintln!("[WARN] write_and_wait: baud_rate is 0, defaulting to 115200");
        115200
    } else {
        baud_rate
    };

    // Calculate transmission time at the given baud rate
    // Each byte = 10 bits (1 start + 8 data + 1 stop)
    let bits = (data.len() * 10) as u64;
    let bit_time_ns = 1_000_000_000u64 / (safe_baud as u64);
    let transmit_time_ns = bits * bit_time_ns;
    let transmit_time_ms = transmit_time_ns / 1_000_000;

    // Add margin: kernel buffer processing + USB latency
    // Use caller-specified minimum or default to 10ms (was 50ms, reduced for speed)
    let min_ms = min_wait_ms.unwrap_or(10);
    let wait_ms = std::cmp::max(min_ms, transmit_time_ms + 5);

    eprintln!(
        "[DEBUG] write_and_wait: wrote {} bytes, waiting {}ms for transmission (baud={}, min={})",
        data.len(),
        wait_ms,
        safe_baud,
        min_ms
    );

    std::thread::sleep(std::time::Duration::from_millis(wait_ms));

    Ok(())
}

/// Non-Unix systems: use write_all with flush
#[cfg(not(target_family = "unix"))]
fn write_and_wait(
    port: &mut Box<dyn SerialPort>,
    data: &[u8],
    _baud_rate: u32,
    _min_wait_ms: Option<u64>,
) -> Result<(), std::io::Error> {
    port.write_all(data)?;
    port.flush()
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connecting (handshake in progress)
    Connecting,
    /// Connected and ready
    Connected,
    /// Connection error
    Error,
}

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Serial port name
    pub port_name: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Use modern protocol with CRC
    pub use_modern_protocol: bool,
    /// Response timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            port_name: String::new(),
            baud_rate: DEFAULT_BAUD_RATE,
            use_modern_protocol: true,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

/// ECU connection with INI-driven protocol
pub struct Connection {
    /// Serial port handle
    port: Option<Box<dyn SerialPort>>,
    /// Current connection state
    state: ConnectionState,
    /// Connection configuration
    config: ConnectionConfig,
    /// ECU signature (after handshake)
    signature: Option<String>,
    /// Use modern protocol (detected from INI or ECU response)
    use_modern_protocol: bool,
    /// Protocol settings from INI file (optional, for INI-driven communication)
    protocol_settings: Option<ProtocolSettings>,
    /// Command builder for formatting commands
    command_builder: CommandBuilder,
    /// ECU endianness
    endianness: Endianness,
    /// Adaptive timing state (experimental - dynamically adjusts communication speed)
    adaptive_timing: Option<AdaptiveTiming>,
}

impl Connection {
    /// Create a new connection (not yet connected)
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            port: None,
            state: ConnectionState::Disconnected,
            config,
            signature: None,
            use_modern_protocol: true,
            protocol_settings: None,
            command_builder: CommandBuilder::new(true), // Default little-endian for rusEFI
            endianness: Endianness::Little,
            adaptive_timing: None,
        }
    }

    /// Create a connection with protocol settings from INI file
    pub fn with_protocol(
        config: ConnectionConfig,
        protocol: ProtocolSettings,
        endianness: Endianness,
    ) -> Self {
        let use_little_endian = endianness == Endianness::Little;
        let use_modern = protocol.uses_modern_protocol();
        Self {
            port: None,
            state: ConnectionState::Disconnected,
            config,
            signature: None,
            use_modern_protocol: use_modern,
            protocol_settings: Some(protocol),
            command_builder: CommandBuilder::new(use_little_endian),
            endianness,
            adaptive_timing: None,
        }
    }

    /// Set protocol settings after connection (for signature matching)
    pub fn set_protocol(&mut self, protocol: ProtocolSettings, endianness: Endianness) {
        self.use_modern_protocol = protocol.uses_modern_protocol();
        self.command_builder = CommandBuilder::new(endianness == Endianness::Little);
        self.endianness = endianness;
        self.protocol_settings = Some(protocol);
    }

    /// Enable adaptive timing with optional custom config
    /// When enabled, communication delays are dynamically adjusted based on measured ECU response times
    pub fn enable_adaptive_timing(&mut self, config: Option<AdaptiveTimingConfig>) {
        let cfg = config.unwrap_or_default();
        let multiplier = cfg.multiplier;
        let min_ms = cfg.min_timeout_ms;
        let max_ms = cfg.max_timeout_ms;

        let mut timing = AdaptiveTiming::new(cfg);
        timing.set_enabled(true);
        self.adaptive_timing = Some(timing);
        eprintln!(
            "[INFO] Adaptive timing enabled (multiplier={:.1}x, range={}–{}ms)",
            multiplier, min_ms, max_ms
        );
    }

    /// Disable adaptive timing
    pub fn disable_adaptive_timing(&mut self) {
        if let Some(timing) = &mut self.adaptive_timing {
            timing.set_enabled(false);
        }
        eprintln!("[INFO] Adaptive timing disabled");
    }

    /// Get adaptive timing stats for diagnostics
    pub fn adaptive_timing_stats(&self) -> Option<(Duration, usize)> {
        self.adaptive_timing
            .as_ref()
            .and_then(|t| t.average_response_time().map(|avg| (avg, t.sample_count())))
    }

    /// Check if adaptive timing is enabled
    pub fn is_adaptive_timing_enabled(&self) -> bool {
        self.adaptive_timing
            .as_ref()
            .map(|t| t.is_enabled())
            .unwrap_or(false)
    }

    /// List available serial ports
    pub fn list_ports() -> Vec<PortInfo> {
        list_ports()
    }

    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get ECU signature (if connected)
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }

    /// Check if using modern CRC protocol
    pub fn is_modern_protocol(&self) -> bool {
        self.use_modern_protocol
    }

    /// Get protocol settings if available
    pub fn protocol(&self) -> Option<&ProtocolSettings> {
        self.protocol_settings.as_ref()
    }

    /// Get effective timeout - uses adaptive timing if enabled, otherwise INI or config default
    fn get_effective_timeout(&self) -> Duration {
        if let Some(timing) = &self.adaptive_timing {
            if timing.is_enabled() {
                return timing.get_timeout();
            }
        }
        // Fall back to INI block_read_timeout or config timeout_ms
        let timeout_ms = self
            .protocol_settings
            .as_ref()
            .map(|p| p.block_read_timeout as u64)
            .unwrap_or(self.config.timeout_ms);
        Duration::from_millis(timeout_ms)
    }

    /// Get effective inter-character timeout
    fn get_effective_inter_char_timeout(&self) -> Duration {
        if let Some(timing) = &self.adaptive_timing {
            if timing.is_enabled() {
                return timing.get_inter_char_timeout();
            }
        }
        // Default: 1/4 of block_read_timeout, min 25ms, max 100ms
        let base_ms = self
            .protocol_settings
            .as_ref()
            .map(|p| p.block_read_timeout as u64)
            .unwrap_or(1000);
        let inter_char_ms = (base_ms / 4).clamp(25, 100);
        Duration::from_millis(inter_char_ms)
    }

    /// Get effective minimum wait time for write_and_wait
    fn get_effective_min_wait(&self) -> u64 {
        if let Some(timing) = &self.adaptive_timing {
            if timing.is_enabled() {
                return timing.get_min_wait().as_millis() as u64;
            }
        }
        // Default: use inter_write_delay from INI, or 10ms minimum
        self.protocol_settings
            .as_ref()
            .map(|p| (p.inter_write_delay as u64).max(10))
            .unwrap_or(10)
    }

    /// Record a response time for adaptive timing
    fn record_response_time(&mut self, elapsed: Duration) {
        if let Some(timing) = &mut self.adaptive_timing {
            timing.record_response_time(elapsed);
        }
    }

    /// Reset adaptive timing on error (back off to conservative values)
    fn reset_adaptive_timing_on_error(&mut self) {
        if let Some(timing) = &mut self.adaptive_timing {
            timing.reset_on_error();
        }
    }

    /// Connect to the ECU
    pub fn connect(&mut self) -> Result<(), ProtocolError> {
        if self.state == ConnectionState::Connected {
            return Err(ProtocolError::AlreadyConnected);
        }

        self.state = ConnectionState::Connecting;

        // Open serial port
        let mut port = open_port(&self.config.port_name, Some(self.config.baud_rate))?;
        configure_port(port.as_mut())?;
        clear_buffers(port.as_mut())?;

        // Wait for ECU stabilization after port open
        // Use INI-specified delay_after_port_open, or default 1000ms for Arduino bootloader
        let port_open_delay = self
            .protocol_settings
            .as_ref()
            .map(|p| p.delay_after_port_open)
            .unwrap_or(1000);
        eprintln!(
            "[DEBUG] connect: waiting {}ms after port open for ECU stabilization (from INI)",
            port_open_delay
        );
        std::thread::sleep(Duration::from_millis(port_open_delay as u64));

        // Clear any garbage data that arrived during delay
        clear_buffers(port.as_mut())?;
        // Small additional delay after clearing
        std::thread::sleep(Duration::from_millis(20));

        self.port = Some(port);

        // Perform handshake
        match self.handshake() {
            Ok(signature) => {
                self.signature = Some(signature);
                self.state = ConnectionState::Connected;
                Ok(())
            }
            Err(e) => {
                self.state = ConnectionState::Error;
                self.port = None;
                Err(e)
            }
        }
    }

    /// Disconnect from the ECU
    pub fn disconnect(&mut self) {
        self.port = None;
        self.signature = None;
        self.state = ConnectionState::Disconnected;
    }

    /// Perform handshake and get ECU signature
    fn handshake(&mut self) -> Result<String, ProtocolError> {
        // Get query command from protocol settings or use default
        // rusEFI uses 'S' (Signature), Speeduino/MegaSquirt uses 'Q' (Query)
        let query_cmd = self
            .protocol_settings
            .as_ref()
            .map(|p| p.query_command.clone())
            .unwrap_or_else(|| "S".to_string());

        // Check if INI specifies modern CRC protocol
        let ini_uses_modern = self
            .protocol_settings
            .as_ref()
            .map(|p| p.uses_modern_protocol())
            .unwrap_or(false);

        eprintln!(
            "[DEBUG] handshake: query_cmd = {:?}, ini_uses_modern = {}",
            query_cmd, ini_uses_modern
        );

        let cmd_bytes = parse_command_string(&query_cmd);
        let cmd_byte = cmd_bytes.first().copied().unwrap_or(b'Q');

        // STRATEGY: Try CRC protocol first if INI specifies it (faster for compatible ECUs)
        // Then fall back to legacy. This prioritizes modern protocol for speed.

        if ini_uses_modern {
            eprintln!("[DEBUG] handshake: trying CRC protocol first");

            // Clear buffers before CRC attempt
            if let Some(port) = self.port.as_mut() {
                let _ = clear_buffers(port.as_mut());
            }

            let packet = Packet::new(cmd_bytes.clone());
            if let Ok(response_packet) = self.send_packet(packet) {
                eprintln!("[DEBUG] handshake: CRC protocol succeeded");
                self.use_modern_protocol = true;

                // Handle status byte: response may start with 0x00 (success)
                let payload = &response_packet.payload;
                let signature_bytes = if !payload.is_empty() && payload[0] == 0 {
                    &payload[1..]
                } else {
                    payload.as_slice()
                };

                let signature = String::from_utf8_lossy(signature_bytes).trim().to_string();
                eprintln!(
                    "[DEBUG] handshake: CRC success, signature = {:?}",
                    signature
                );
                return Ok(signature);
            } else {
                eprintln!("[DEBUG] handshake: CRC protocol failed, trying legacy");
            }
        }

        // Try legacy protocol (raw ASCII command)
        eprintln!(
            "[DEBUG] handshake: trying legacy protocol, sending byte 0x{:02x}",
            cmd_byte
        );

        // Clear buffers before legacy attempt
        if let Some(port) = self.port.as_mut() {
            let _ = clear_buffers(port.as_mut());
        }

        match self.send_raw_command(&[cmd_byte]) {
            Ok(response) => {
                eprintln!(
                    "[DEBUG] handshake: legacy succeeded, {} bytes",
                    response.len()
                );
                self.use_modern_protocol = false;
                let signature = String::from_utf8_lossy(&response).trim().to_string();
                eprintln!(
                    "[DEBUG] handshake: legacy success, signature = {:?}",
                    signature
                );
                Ok(signature)
            }
            Err(e) => {
                eprintln!("[DEBUG] handshake: legacy failed ({:?})", e);

                // If INI doesn't specify modern and legacy failed, try CRC as last resort
                if !ini_uses_modern {
                    eprintln!("[DEBUG] handshake: trying CRC as fallback");

                    if let Some(port) = self.port.as_mut() {
                        let _ = clear_buffers(port.as_mut());
                    }
                    std::thread::sleep(Duration::from_millis(50));

                    let packet = Packet::new(cmd_bytes);
                    if let Ok(response_packet) = self.send_packet(packet) {
                        eprintln!("[DEBUG] handshake: CRC fallback succeeded");
                        self.use_modern_protocol = true;

                        let payload = &response_packet.payload;
                        let signature_bytes = if !payload.is_empty() && payload[0] == 0 {
                            &payload[1..]
                        } else {
                            payload.as_slice()
                        };

                        let signature = String::from_utf8_lossy(signature_bytes).trim().to_string();
                        eprintln!(
                            "[DEBUG] handshake: CRC fallback success, signature = {:?}",
                            signature
                        );
                        return Ok(signature);
                    }
                }

                return Err(e);
            }
        }
    }

    /// Send raw bytes and get response (for initial handshake)
    /// Uses non-blocking reads with bytes_to_read() polling for reliable timeout behavior
    fn send_raw_command(&mut self, cmd: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        // Get timing parameters before borrowing port
        let baud_rate = self.config.baud_rate;
        let min_wait = Some(self.get_effective_min_wait());
        let timeout = self.get_effective_timeout();
        let inter_char_timeout = self.get_effective_inter_char_timeout();
        let poll_interval = if self.is_adaptive_timing_enabled() {
            1
        } else {
            2
        };

        let port = self.port.as_mut().ok_or(ProtocolError::NotConnected)?;

        eprintln!("[DEBUG] send_raw_command: clearing buffers before send");
        // Clear any stale data in buffers
        let _ = port.clear(serialport::ClearBuffer::All);

        eprintln!(
            "[DEBUG] send_raw_command: sending {} bytes: {:02x?}",
            cmd.len(),
            cmd
        );

        // Start timing for adaptive timing
        let send_start = Instant::now();

        // Send command bytes and wait for transmission
        // Use write_and_wait which avoids the blocking tcdrain issue
        write_and_wait(port, cmd, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        eprintln!(
            "[DEBUG] send_raw_command: command sent, timeout={}ms, inter_char={}ms",
            timeout.as_millis(),
            inter_char_timeout.as_millis()
        );

        // Read response with timeout using bytes_to_read() polling
        let mut response = Vec::new();
        let mut buffer = [0u8; 512];
        let start = Instant::now();
        let mut last_data_time = Instant::now();

        loop {
            if start.elapsed() > timeout {
                eprintln!("[DEBUG] send_raw_command: overall timeout reached");
                break;
            }

            // Check how many bytes are available without blocking
            let available = match port.bytes_to_read() {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("[DEBUG] send_raw_command: bytes_to_read error: {}", e);
                    return Err(ProtocolError::SerialError(e.to_string()));
                }
            };

            if available > 0 {
                let to_read = std::cmp::min(available as usize, buffer.len());
                match port.read(&mut buffer[..to_read]) {
                    Ok(0) => {
                        eprintln!("[DEBUG] send_raw_command: read returned 0 (EOF)");
                        break;
                    }
                    Ok(n) => {
                        response.extend_from_slice(&buffer[..n]);
                        last_data_time = Instant::now();
                        eprintln!(
                            "[DEBUG] send_raw_command: read {} bytes, total = {}, data = {:02x?}",
                            n,
                            response.len(),
                            &buffer[..n]
                        );
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        // Non-blocking, continue polling
                    }
                    Err(e) => {
                        eprintln!("[DEBUG] send_raw_command: read error: {}", e);
                        self.reset_adaptive_timing_on_error();
                        return Err(ProtocolError::SerialError(e.to_string()));
                    }
                }
            } else if response.is_empty() {
                // No data yet, poll at configured interval
                std::thread::sleep(Duration::from_millis(poll_interval));
            } else {
                // We have some data - check inter-character timeout
                if last_data_time.elapsed() > inter_char_timeout {
                    eprintln!(
                        "[DEBUG] send_raw_command: inter-character timeout, message complete"
                    );
                    break;
                }
                std::thread::sleep(Duration::from_millis(poll_interval));
            }
        }

        let elapsed = send_start.elapsed();
        eprintln!(
            "[DEBUG] send_raw_command: completed with {} bytes in {}ms: {:?}",
            response.len(),
            elapsed.as_millis(),
            String::from_utf8_lossy(&response)
        );

        if response.is_empty() {
            self.reset_adaptive_timing_on_error();
            return Err(ProtocolError::Timeout);
        }

        // Record response time for adaptive timing
        self.record_response_time(elapsed);

        Ok(response)
    }

    /// Send raw bytes WITHOUT waiting for response (for burn commands)
    /// ECUs typically don't respond during flash write operations
    fn send_raw_command_no_response(&mut self, cmd: &[u8]) -> Result<(), ProtocolError> {
        let baud_rate = self.config.baud_rate;
        let min_wait = Some(self.get_effective_min_wait());

        let port = self.port.as_mut().ok_or(ProtocolError::NotConnected)?;

        eprintln!(
            "[DEBUG] send_raw_command_no_response: sending {} bytes: {:02x?}",
            cmd.len(),
            cmd
        );

        // Send command bytes and wait for transmission to complete
        write_and_wait(port, cmd, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        eprintln!("[DEBUG] send_raw_command_no_response: command sent, not waiting for response");

        Ok(())
    }

    /// Send CRC packet WITHOUT waiting for response (for burn commands)
    fn send_packet_no_response(&mut self, packet: Packet) -> Result<(), ProtocolError> {
        let port = self.port.as_mut().ok_or(ProtocolError::NotConnected)?;
        let bytes = packet.to_bytes();

        eprintln!(
            "[DEBUG] send_packet_no_response: sending {} bytes",
            bytes.len()
        );

        port.write_all(&bytes)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;
        port.flush()
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        eprintln!("[DEBUG] send_packet_no_response: packet sent, not waiting for response");

        Ok(())
    }

    /// Send a legacy (ASCII) command and get response
    #[allow(dead_code)]
    fn send_legacy_command(&mut self, cmd: Command) -> Result<Vec<u8>, ProtocolError> {
        let port = self.port.as_mut().ok_or(ProtocolError::NotConnected)?;

        // Send single command byte
        port.write_all(&[cmd.legacy_byte()])
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;
        port.flush()
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        // Read response with timeout
        let mut response = Vec::new();
        let mut buffer = [0u8; 256];
        let start = Instant::now();
        let timeout = Duration::from_millis(cmd.timeout_ms());

        loop {
            match port.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    response.extend_from_slice(&buffer[..n]);
                    // Give a brief moment for more data
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    if response.is_empty() && start.elapsed() < timeout {
                        continue;
                    }
                    break;
                }
                Err(e) => return Err(ProtocolError::SerialError(e.to_string())),
            }

            if start.elapsed() > timeout {
                break;
            }
        }

        if response.is_empty() && cmd.expects_response() {
            return Err(ProtocolError::Timeout);
        }

        Ok(response)
    }

    /// Send a modern protocol packet and get response
    fn send_packet(&mut self, packet: Packet) -> Result<Packet, ProtocolError> {
        // Get timing parameters before borrowing port
        let baud_rate = self.config.baud_rate;
        let min_wait = Some(self.get_effective_min_wait());
        let timeout = self.get_effective_timeout();
        let poll_interval_ms = if self.is_adaptive_timing_enabled() {
            1
        } else {
            2
        };

        let port = self.port.as_mut().ok_or(ProtocolError::NotConnected)?;

        // Clear buffers before sending
        eprintln!("[DEBUG] send_packet: clearing buffers");
        let _ = port.clear(serialport::ClearBuffer::All);

        // Start timing for adaptive timing
        let send_start = Instant::now();

        // Send packet and wait for transmission
        let bytes = packet.to_bytes();
        eprintln!(
            "[DEBUG] send_packet: sending {} bytes: {:02x?}",
            bytes.len(),
            bytes
        );
        // Use write_and_wait which avoids the blocking tcdrain issue
        write_and_wait(port, &bytes, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        eprintln!(
            "[DEBUG] send_packet: packet sent, timeout={}ms",
            timeout.as_millis()
        );

        // Helper to read exact bytes with timeout
        // Uses bytes_to_read() polling to avoid blocking read() calls on Linux
        fn read_exact_timeout(
            port: &mut Box<dyn SerialPort>,
            buf: &mut [u8],
            timeout: Duration,
            poll_ms: u64,
        ) -> Result<(), ProtocolError> {
            let start = Instant::now();
            let mut offset = 0;

            while offset < buf.len() {
                if start.elapsed() > timeout {
                    eprintln!(
                        "[DEBUG] read_exact_timeout: timed out after reading {} of {} bytes",
                        offset,
                        buf.len()
                    );
                    return Err(ProtocolError::Timeout);
                }

                // Check how many bytes are available
                let available = port
                    .bytes_to_read()
                    .map_err(|e| ProtocolError::SerialError(e.to_string()))?
                    as usize;

                if available == 0 {
                    // No data available, sleep briefly and try again
                    std::thread::sleep(Duration::from_millis(poll_ms));
                    continue;
                }

                // Read available bytes (up to what we need)
                let to_read = std::cmp::min(available, buf.len() - offset);
                match port.read(&mut buf[offset..offset + to_read]) {
                    Ok(0) => {
                        eprintln!("[DEBUG] read_exact_timeout: EOF after {} bytes", offset);
                        return Err(ProtocolError::Timeout);
                    }
                    Ok(n) => {
                        eprintln!(
                            "[DEBUG] read_exact_timeout: read {} bytes: {:02x?}",
                            n,
                            &buf[offset..offset + n]
                        );
                        offset += n;
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[DEBUG] read_exact_timeout: error: {}", e);
                        return Err(ProtocolError::SerialError(e.to_string()));
                    }
                }
            }
            Ok(())
        }

        // Cast port to the right type for bytes_to_read()
        let port_box = port;

        // Read response header (2 bytes for length)
        let mut header = [0u8; 2];
        eprintln!("[DEBUG] send_packet: waiting for response header...");
        if let Err(e) = read_exact_timeout(port_box, &mut header, timeout, poll_interval_ms) {
            self.reset_adaptive_timing_on_error();
            return Err(e);
        }
        eprintln!("[DEBUG] send_packet: got header {:02x?}", header);

        // Parse length
        let length = u16::from_be_bytes(header) as usize;
        eprintln!("[DEBUG] send_packet: response length = {}", length);
        if length > super::MAX_PACKET_SIZE {
            return Err(ProtocolError::BufferOverflow);
        }

        // Read payload + CRC
        let mut payload_and_crc = vec![0u8; length + 4];
        if let Err(e) =
            read_exact_timeout(port_box, &mut payload_and_crc, timeout, poll_interval_ms)
        {
            self.reset_adaptive_timing_on_error();
            return Err(e);
        }

        // Record response time for adaptive timing
        let elapsed = send_start.elapsed();
        eprintln!("[DEBUG] send_packet: complete in {}ms", elapsed.as_millis());
        self.record_response_time(elapsed);

        // Reconstruct full packet for parsing
        let mut full_packet = Vec::with_capacity(2 + length + 4);
        full_packet.extend_from_slice(&header);
        full_packet.extend_from_slice(&payload_and_crc);

        Packet::from_bytes(&full_packet)
    }

    /// Get real-time data from ECU
    pub fn get_realtime_data(&mut self) -> Result<Vec<u8>, ProtocolError> {
        // Use burst command from INI if available
        let burst_cmd = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.burst_get_command.as_deref())
            .unwrap_or("A");

        if self.use_modern_protocol {
            // Modern protocol: wrap command in CRC packet
            let cmd_bytes = burst_cmd.as_bytes().to_vec();
            let packet = Packet::new(cmd_bytes);
            let response = self.send_packet(packet)?;
            Ok(response.payload)
        } else {
            // Legacy protocol: send raw command byte
            let cmd_byte = burst_cmd.as_bytes().first().copied().unwrap_or(b'A');
            self.send_raw_command(&[cmd_byte])
        }
    }

    /// Get page identifier for a page index (used in protocol commands)
    /// Returns the 16-bit page identifier from INI, or page index if not defined
    fn get_page_identifier(&self, page_index: u8) -> u16 {
        self.protocol_settings
            .as_ref()
            .and_then(|p| p.page_identifiers.get(page_index as usize))
            .map(|bytes| {
                // Page identifier is stored as raw bytes, interpret as little-endian u16
                if bytes.len() >= 2 {
                    u16::from_le_bytes([bytes[0], bytes[1]])
                } else if bytes.len() == 1 {
                    bytes[0] as u16
                } else {
                    page_index as u16
                }
            })
            .unwrap_or(page_index as u16)
    }

    /// Read memory from ECU using INI-defined command format
    pub fn read_memory(&mut self, params: ReadMemoryParams) -> Result<Vec<u8>, ProtocolError> {
        let page = params.page as usize;

        // Get page identifier (may differ from page index)
        let page_id = self.get_page_identifier(params.page);

        // Get read command format from INI settings
        let read_format = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.page_read_commands.get(page).cloned())
            .unwrap_or_else(|| "R%2i%2o%2c".to_string());

        if read_format.is_empty() {
            return Err(ProtocolError::ProtocolError(format!(
                "No read command for page {}",
                page
            )));
        }

        // Build command using INI format string (use page_id, not page index)
        let cmd = self.command_builder.build_read_command(
            &read_format,
            page_id,
            params.offset,
            params.length,
        )?;

        if self.use_modern_protocol {
            // Modern protocol: wrap in CRC packet
            let packet = Packet::new(cmd);
            let response = self.send_packet(packet)?;

            // rusEFI response format: status byte (0 = success) + data
            let payload = &response.payload;
            if payload.is_empty() {
                return Err(ProtocolError::InvalidResponse);
            }

            let status = payload[0];
            if status != 0 {
                return Err(ProtocolError::ProtocolError(format!(
                    "Read error, status: {}",
                    status
                )));
            }

            Ok(payload[1..].to_vec())
        } else {
            // Legacy protocol: send raw command
            self.send_raw_command(&cmd)
        }
    }

    /// Read a full page from ECU, respecting blocking factor
    pub fn read_page(&mut self, page: u8) -> Result<Vec<u8>, ProtocolError> {
        let page_size = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.page_sizes.get(page as usize).copied())
            .unwrap_or(0);

        if page_size == 0 {
            return Err(ProtocolError::ProtocolError(format!(
                "Unknown page size for page {}",
                page
            )));
        }

        let blocking_factor = self
            .protocol_settings
            .as_ref()
            .map(|p| p.blocking_factor)
            .unwrap_or(256);

        let mut data = Vec::with_capacity(page_size as usize);
        let mut offset = 0u16;

        while (offset as u32) < page_size {
            let remaining = page_size - offset as u32;
            let chunk_size = remaining.min(blocking_factor) as u16;

            let params = ReadMemoryParams {
                page,
                offset,
                length: chunk_size,
                can_id: 0,
            };

            let chunk = self.read_memory(params)?;
            data.extend_from_slice(&chunk);
            offset += chunk_size;
        }

        Ok(data)
    }

    /// Write memory to ECU using INI-defined command format
    pub fn write_memory(&mut self, params: WriteMemoryParams) -> Result<(), ProtocolError> {
        let page = params.page as usize;

        // Get page identifier (may differ from page index)
        let page_id = self.get_page_identifier(params.page);

        // Get write command format from INI settings
        let write_format = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.page_chunk_write_commands.get(page).cloned())
            .unwrap_or_else(|| "C%2i%2o%2c%v".to_string());

        if write_format.is_empty() {
            return Err(ProtocolError::ProtocolError(format!(
                "No write command for page {}",
                page
            )));
        }

        // Build command using INI format string (use page_id, not page index)
        let cmd = self.command_builder.build_write_command(
            &write_format,
            page_id,
            params.offset,
            &params.data,
        )?;

        if self.use_modern_protocol {
            // Modern protocol: wrap in CRC packet
            let packet = Packet::new(cmd);
            let _response = self.send_packet(packet)?;
            Ok(())
        } else {
            // Legacy protocol: send raw command
            self.send_raw_command(&cmd)?;
            Ok(())
        }
    }

    /// Burn current page to flash using INI-defined command format
    pub fn burn(&mut self, params: BurnParams) -> Result<(), ProtocolError> {
        let page = params.page as usize;

        // Get page identifier (may differ from page index)
        let page_id = self.get_page_identifier(params.page);

        // Get burn command format from INI settings
        let burn_format = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.burn_commands.get(page).cloned())
            .unwrap_or_else(|| "B%2i".to_string());

        // Empty burn command means page is not burnable (already in flash or read-only)
        if burn_format.is_empty() {
            eprintln!(
                "[DEBUG] burn: page {} has empty burn command, skipping",
                page
            );
            return Ok(());
        }

        // Build command using INI format string (use page_id, not page index)
        let cmd = self
            .command_builder
            .build_burn_command(&burn_format, page_id)?;

        eprintln!(
            "[DEBUG] burn: sending burn command for page {}, format='{}', cmd = {:02x?}",
            page, burn_format, cmd
        );

        // Send burn command WITHOUT waiting for response
        // ECUs typically don't respond during flash write operations
        // The INI format "B%2i", "" has empty response string indicating no response expected
        if self.use_modern_protocol {
            // Modern protocol: wrap in CRC packet but don't wait for response
            let packet = Packet::new(cmd);
            self.send_packet_no_response(packet)?;
        } else {
            // Legacy protocol: send raw command without expecting response
            self.send_raw_command_no_response(&cmd)?;
        }

        // Wait for flash write to complete
        // Flash writes typically take 1-3 seconds depending on ECU
        // Use page_activation_delay as minimum, but ensure at least 2 seconds for safety
        let delay = self
            .protocol_settings
            .as_ref()
            .map(|p| p.page_activation_delay.max(2000))
            .unwrap_or(2000);

        eprintln!(
            "[DEBUG] burn: waiting {}ms for flash write to complete",
            delay
        );
        std::thread::sleep(Duration::from_millis(delay as u64));

        eprintln!("[DEBUG] burn: flash write complete for page {}", page);
        Ok(())
    }

    /// Convenience method to burn all pages to flash
    pub fn send_burn_command(&mut self) -> Result<(), ProtocolError> {
        // Burn page 0 (main configuration page)
        // Most ECUs burn all RAM to flash with a single command
        self.burn(BurnParams { can_id: 0, page: 0 })
    }

    /// Send raw bytes to ECU (for controller commands)
    /// This is used by commandButton widgets to send arbitrary commands
    /// WARNING: These commands bypass normal memory synchronization
    pub fn send_raw_bytes(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if bytes.is_empty() {
            return Ok(());
        }
        eprintln!(
            "[DEBUG] send_raw_bytes: sending {} bytes: {:02x?}",
            bytes.len(),
            bytes
        );
        self.send_raw_command_no_response(bytes)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        assert_eq!(config.baud_rate, DEFAULT_BAUD_RATE);
        assert!(config.use_modern_protocol);
    }

    #[test]
    fn test_connection_state() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config);
        assert_eq!(conn.state(), ConnectionState::Disconnected);
        assert!(conn.signature().is_none());
    }

    #[test]
    fn test_parse_command_string_hex_escapes() {
        // Test basic hex escape
        let result = parse_command_string(r"\x0f");
        assert_eq!(result, vec![0x0f]);

        // Test multiple hex escapes
        let result = parse_command_string(r"\x00\x0f\x14");
        assert_eq!(result, vec![0x00, 0x0f, 0x14]);

        // Test mixed content (like MS2Extra query command)
        let result = parse_command_string(r"r\x00\x0f\x00\x00\x00\x14");
        assert_eq!(result, vec![b'r', 0x00, 0x0f, 0x00, 0x00, 0x00, 0x14]);
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn test_parse_command_string_other_escapes() {
        assert_eq!(parse_command_string(r"\n"), vec![b'\n']);
        assert_eq!(parse_command_string(r"\r"), vec![b'\r']);
        assert_eq!(parse_command_string(r"\t"), vec![b'\t']);
        assert_eq!(parse_command_string(r"\\"), vec![b'\\']);
        assert_eq!(parse_command_string(r"\0"), vec![0]);
    }

    #[test]
    fn test_parse_command_string_plain_text() {
        assert_eq!(parse_command_string("Q"), vec![b'Q']);
        assert_eq!(parse_command_string("S"), vec![b'S']);
        assert_eq!(parse_command_string("Hello"), b"Hello".to_vec());
    }
}
