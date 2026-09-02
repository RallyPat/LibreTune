//! Connection management
//!
//! Handles the connection lifecycle and command execution with the ECU.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::calibration::{self, CalibrationTable};
use super::stream::{CommunicationChannel, SerialChannel, TcpChannel};
use super::{
    commands::{BurnParams, ReadMemoryParams, WriteMemoryParams},
    serial::{clear_buffers, configure_port, list_ports, open_port, PortInfo},
    Command, CommandBuilder, EnvelopeOrder, Packet, ProtocolError, DEFAULT_BAUD_RATE,
    DEFAULT_TIMEOUT_MS,
};
use crate::ini::{AdaptiveTiming, AdaptiveTimingConfig, EcuType, Endianness, ProtocolSettings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Extra quiet time added to the declared burn window, on top of
/// `pageActivationDelay`. See [`Connection::burn`] for why the declared value
/// on its own is not enough.
const BURN_SETTLE_MS: u64 = 600;

/// Minimum quiet time after a legacy write frame before the ECU will service
/// anything else. See [`Connection::inter_frame_delay`].
const LEGACY_WRITE_SETTLE_MS: u64 = 30;

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

/// Determine whether a response payload contains a leading status byte and return the data portion.
///
/// msEnvelope_1.0 responses *should* always start with a status byte (0x00 = success).
/// However, some rusEFI/epicEFI firmware variants omit the status byte from OCH/Burst
/// responses, sending exactly `expected_data_len` bytes of raw channel data instead.
///
/// Detection strategy (in priority order):
///   1. payload.len() == expected_data_len + 1 AND payload[0] == 0  → status byte present, strip it
///   2. payload.len() == expected_data_len                           → no status byte, use as-is
///   3. payload[0] == 0                                              → assume status byte, strip it
///   4. otherwise                                                    → use full payload (best-effort)
fn strip_status_byte(payload: &[u8], expected_data_len: usize, label: &str) -> Vec<u8> {
    if payload.is_empty() {
        return Vec::new();
    }
    if expected_data_len > 0 {
        if payload.len() == expected_data_len + 1 && payload[0] == 0 {
            // Status byte present and indicates success
            return payload[1..].to_vec();
        }
        if payload.len() == expected_data_len {
            // No status byte — firmware sent raw data directly
            return payload.to_vec();
        }
        if payload.len() == expected_data_len + 1 && payload[0] != 0 {
            let code = super::ResponseCode::from_byte(payload[0]);
            tracing::warn!(
                "{} response: ECU status=0x{:02x} ({}), using data anyway",
                label,
                payload[0],
                code.message()
            );
            return payload[1..].to_vec();
        }
    }
    // Fallback: use old behaviour (strip if starts with 0x00, else keep all)
    if payload[0] == 0 {
        payload[1..].to_vec()
    } else {
        // Don't error — just return the full payload; channel offsets will be relative to byte 0
        tracing::warn!(
            "{} response: unexpected first byte 0x{:02x} (expected_len={}), using full payload",
            label,
            payload[0],
            expected_data_len
        );
        payload.to_vec()
    }
}

/// Inspect a modern-protocol write response for an ECU-reported error status.
///
/// Per msEnvelope_1.0 §15.2, every reply frame's first payload byte is a
/// response code. `write_memory` used to discard the response entirely once
/// its CRC checked out, so a write the ECU rejected (out of range, flash
/// locked, busy, settings refused) was silently reported to the rest of the
/// app as a success. Mirrors the same status-byte check already used for
/// console command ('E') responses in `send_console_command_modern`.
///
/// An empty payload is treated as success rather than an error: some
/// ECU/INI combinations don't echo a status byte on write acks at all (the
/// same leniency `strip_status_byte` already applies on the read side), and
/// the CRC already confirmed the frame arrived intact.
fn check_write_response_status(response: &Packet) -> Result<(), ProtocolError> {
    let Some(&status) = response.payload.first() else {
        return Ok(());
    };
    let code = super::ResponseCode::from_byte(status);
    if !code.is_error() {
        return Ok(());
    }
    let message = if code.carries_payload_message() && response.payload.len() > 1 {
        String::from_utf8_lossy(&response.payload[1..]).into_owned()
    } else {
        code.message().to_string()
    };
    Err(ProtocolError::EcuStatusError {
        code: status,
        message,
    })
}

/// Drain up to `remaining` bytes from the channel within `deadline`, discarding all data.
///
/// Called after a partial-read timeout inside `send_packet` to flush the rest of the
/// ECU's response from the OS TCP/serial receive buffer so the next request starts at
/// a clean packet boundary.
fn drain_input_with_timeout(
    channel: &mut Box<dyn CommunicationChannel>,
    remaining: usize,
    deadline: Duration,
    poll_ms: u64,
) {
    let start = std::time::Instant::now();
    let mut buf = [0u8; 256];
    let mut drained = 0usize;
    while drained < remaining && start.elapsed() < deadline {
        let available = channel.bytes_to_read().unwrap_or(0) as usize;
        if available == 0 {
            std::thread::sleep(Duration::from_millis(poll_ms));
            continue;
        }
        let to_read = std::cmp::min(available, std::cmp::min(remaining - drained, buf.len()));
        match channel.read(&mut buf[..to_read]) {
            Ok(n) => drained += n,
            Err(_) => break,
        }
    }
    // After draining the known remainder, do one more flush to catch any trailing bytes
    let _ = channel.clear_input_buffer();
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
    channel: &mut Box<dyn CommunicationChannel>,
    data: &[u8],
    baud_rate: u32,
    min_wait_ms: Option<u64>,
) -> Result<(), std::io::Error> {
    // Write the data - this goes to the kernel's tty output buffer
    channel.write_all(data)?;

    // Guard against zero baud rate
    let safe_baud = if baud_rate == 0 { 115200 } else { baud_rate };

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

    tracing::debug!(
        "write_and_wait: wrote {} bytes, waiting {}ms for transmission (baud={}, min={})",
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
    channel: &mut Box<dyn CommunicationChannel>,
    data: &[u8],
    _baud_rate: u32,
    _min_wait_ms: Option<u64>,
) -> Result<(), std::io::Error> {
    channel.write_all(data)?;
    channel.flush()
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

/// Connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    Serial,
    Tcp,
}

/// Connection runtime packet selection override
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePacketMode {
    Auto,
    ForceBurst,
    ForceOCH,
    Disabled,
}

/// Choice of runtime fetch command
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RuntimeFetch {
    Burst(String),
    OCH(String),
}

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Connection type
    pub connection_type: ConnectionType,
    /// Serial port name
    pub port_name: String,
    /// Baud rate
    pub baud_rate: u32,
    /// TCP host address (for TCP connection)
    pub tcp_host: Option<String>,
    /// TCP port (for TCP connection)
    pub tcp_port: Option<u16>,
    /// Use modern protocol with CRC
    pub use_modern_protocol: bool,
    /// Response timeout in milliseconds
    pub timeout_ms: u64,
    /// Optional override for runtime packet selection
    pub runtime_packet_mode: RuntimePacketMode,
    /// Accept non-spec CRC scopes/byte-orders during packet decode (msEnvelope_1.0 §15.1).
    /// Default `false`. Enable per-project for ECUs that ship quirky firmware.
    pub permissive_crc: bool,
    /// Auto-burn the previous page before any write that targets a different page (spec §6.2).
    /// Prevents partial-flash corruption on power loss. Default `true`.
    pub auto_burn_on_page_change: bool,
    /// Auto-burn when a settings dialog closes (spec §6.2).
    /// Frontend signals dialog-close via `flush_pending_burn`. Default `true`.
    pub auto_burn_on_close_dialog: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            connection_type: ConnectionType::Serial,
            port_name: String::new(),
            baud_rate: DEFAULT_BAUD_RATE,
            tcp_host: None,
            tcp_port: None,
            use_modern_protocol: true,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            runtime_packet_mode: RuntimePacketMode::Auto,
            permissive_crc: false,
            auto_burn_on_page_change: true,
            auto_burn_on_close_dialog: true,
        }
    }
}

/// ECU connection with INI-driven protocol
pub struct Connection {
    /// Communication channel (Serial or TCP)
    channel: Option<Box<dyn CommunicationChannel>>,
    /// Current connection state
    state: ConnectionState,
    /// Connection configuration
    config: ConnectionConfig,
    /// ECU signature (after handshake)
    signature: Option<String>,
    /// Use modern protocol (detected from INI or ECU response)
    use_modern_protocol: bool,
    /// Byte order of the CRC envelope's length/CRC fields. Speeduino is
    /// little-endian, rusEFI/msEnvelope big-endian; set from ProtocolSettings
    /// and corrected by the handshake's flip-retry if the INI's ECU-type
    /// detection guessed wrong.
    envelope_order: EnvelopeOrder,
    /// Protocol settings from INI file (optional, for INI-driven communication)
    protocol_settings: Option<ProtocolSettings>,
    /// Command builder for formatting commands
    command_builder: CommandBuilder,
    /// ECU endianness
    endianness: Endianness,
    /// Adaptive timing state (experimental - dynamically adjusts communication speed)
    adaptive_timing: Option<AdaptiveTiming>,
    /// Metrics: cumulative bytes/packets sent & received
    tx_bytes: u64,
    rx_bytes: u64,
    tx_packets: u64,
    rx_packets: u64,
    /// Page targeted by the most recent successful write, used by the auto-burn-on-page-change
    /// safety policy (msEnvelope_1.0 spec §6.2). `None` after construction or after a burn.
    last_written_page: Option<u8>,
    /// Every page written since it was last burned.
    ///
    /// `last_written_page` tracks only the most recent one, which is all the
    /// auto-burn-on-page-change policy needs. A user-initiated burn has to know
    /// about all of them: Speeduino burns ONE page per command (the INI gives
    /// fifteen `burnCommand` entries, one per page, each taking the page number),
    /// so burning "the tune" means burning each page that has changed.
    dirty_pages: std::collections::BTreeSet<u8>,
    /// ECU type detected from the INI signature (Issue #71).
    /// Drives conservative runtime-command selection for Speeduino/MS2/MS3.
    ecu_type: EcuType,
    /// Cancellation flag shared with the owner (Tauri AppState). When set, all
    /// blocking I/O polling loops abort early so `disconnect()` can complete even
    /// while another thread is mid-read (Issue #71: "disconnect does nothing").
    cancel: Arc<AtomicBool>,
}

impl Connection {
    /// Create a new connection (not yet connected)
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            channel: None,
            state: ConnectionState::Disconnected,
            config,
            signature: None,
            use_modern_protocol: true,
            envelope_order: EnvelopeOrder::BigEndian,
            protocol_settings: None,
            // When no INI is loaded, default to big-endian command parameters (safe default;
            // overridden to match the INI endianness when with_protocol/set_protocol is called).
            command_builder: CommandBuilder::new(false),
            endianness: Endianness::Little,
            adaptive_timing: None,
            tx_bytes: 0,
            rx_bytes: 0,
            tx_packets: 0,
            rx_packets: 0,
            last_written_page: None,
            dirty_pages: std::collections::BTreeSet::new(),
            ecu_type: EcuType::Unknown,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a connection with protocol settings from INI file
    pub fn with_protocol(
        config: ConnectionConfig,
        protocol: ProtocolSettings,
        endianness: Endianness,
    ) -> Self {
        let use_modern = protocol.uses_modern_protocol();

        // Protocol command arguments (%2o, %2c, %2i) must match the INI
        // endianness.  rusEFI / epicEFI / FOME use little-endian command
        // parameters (the INI declares `endianness = little`), while Speeduino
        // and MS2/MS3 use big-endian command parameters.
        let cmd_le = endianness == Endianness::Little;
        // msEnvelope_1.0 default; the handshake's flip-retry adapts to a
        // firmware that genuinely frames the other way.
        let envelope_order = EnvelopeOrder::BigEndian;
        Self {
            channel: None,
            state: ConnectionState::Disconnected,
            config,
            signature: None,
            use_modern_protocol: use_modern,
            envelope_order,
            protocol_settings: Some(protocol),
            command_builder: CommandBuilder::new(cmd_le),
            endianness,
            adaptive_timing: None,
            tx_bytes: 0,
            rx_bytes: 0,
            tx_packets: 0,
            rx_packets: 0,
            last_written_page: None,
            dirty_pages: std::collections::BTreeSet::new(),
            ecu_type: EcuType::Unknown,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set protocol settings after connection (for signature matching)
    pub fn set_protocol(&mut self, protocol: ProtocolSettings, endianness: Endianness) {
        self.use_modern_protocol = protocol.uses_modern_protocol();
        // Use LE command parameters when INI specifies little-endian (rusEFI/epicEFI/FOME).
        self.command_builder = CommandBuilder::new(endianness == Endianness::Little);
        self.endianness = endianness;
        self.envelope_order = EnvelopeOrder::BigEndian;
        self.protocol_settings = Some(protocol);
    }

    /// Set the detected ECU type (called after the INI is loaded, Issue #71).
    /// Drives conservative runtime-command selection for Speeduino/MS2/MS3.
    pub fn set_ecu_type(&mut self, ecu_type: EcuType) {
        self.ecu_type = ecu_type;
    }

    /// Get the detected ECU type.
    pub fn ecu_type(&self) -> EcuType {
        self.ecu_type
    }

    /// Get a clone of the cancellation handle. The owner (e.g. Tauri AppState)
    /// can call `request_cancel()` on its clone to interrupt in-flight blocking
    /// I/O so `disconnect()` is not blocked by a streaming task mid-read.
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// Request cancellation of any in-flight blocking I/O. Safe to call from a
    /// different thread than the one performing the read (Issue #71).
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Get cumulative tx/rx bytes and packet counters
    pub fn get_counters(&self) -> (u64, u64, u64, u64) {
        (
            self.tx_bytes,
            self.rx_bytes,
            self.tx_packets,
            self.rx_packets,
        )
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
        tracing::info!(
            "Adaptive timing enabled (multiplier={:.1}x, range={}–{}ms)",
            multiplier,
            min_ms,
            max_ms
        );
    }

    /// Disable adaptive timing
    pub fn disable_adaptive_timing(&mut self) {
        if let Some(timing) = &mut self.adaptive_timing {
            timing.set_enabled(false);
        }
        tracing::info!("Adaptive timing disabled");
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

        // Open communication channel
        let mut channel: Box<dyn CommunicationChannel> = match self.config.connection_type {
            ConnectionType::Serial => {
                // Open serial port
                let mut port = open_port(&self.config.port_name, Some(self.config.baud_rate))?;
                configure_port(port.as_mut())?;
                clear_buffers(port.as_mut())?;
                Box::new(SerialChannel::new(port))
            }
            ConnectionType::Tcp => {
                let host = self.config.tcp_host.as_deref().unwrap_or("localhost");
                let port = self.config.tcp_port.unwrap_or(29001);
                let addr = format!("{}:{}", host, port);
                tracing::info!("Connecting to ECU via TCP: {}", addr);
                let stream = TcpStream::connect(&addr)
                    .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;
                stream.set_nodelay(true).ok();
                Box::new(TcpChannel::new(stream))
            }
        };

        // Wait for ECU stabilization after port open
        // Use INI-specified delay_after_port_open, or default 1000ms for Arduino bootloader
        let port_open_delay = self
            .protocol_settings
            .as_ref()
            .map(|p| p.delay_after_port_open)
            .unwrap_or(1000);
        tracing::debug!(
            "connect: waiting {}ms after port open for ECU stabilization (from INI)",
            port_open_delay
        );
        std::thread::sleep(Duration::from_millis(port_open_delay as u64));

        // Clear any garbage data that arrived during delay
        channel.clear_input_buffer().ok();
        // Small additional delay after clearing
        std::thread::sleep(Duration::from_millis(20));

        self.channel = Some(channel);

        // Perform handshake
        match self.handshake() {
            Ok(signature) => {
                self.signature = Some(signature);
                self.state = ConnectionState::Connected;
                Ok(())
            }
            Err(e) => {
                self.state = ConnectionState::Error;
                self.channel = None;
                Err(e)
            }
        }
    }

    /// Disconnect from the ECU
    ///
    /// Sets the cancellation flag first so any blocking I/O polling loop running
    /// in another thread (e.g. the realtime stream task) aborts its current
    /// read promptly instead of waiting for the full timeout (Issue #71).
    pub fn disconnect(&mut self) {
        // Signal in-flight blocking reads to abort.
        self.cancel.store(true, Ordering::Relaxed);
        self.channel = None;
        self.signature = None;
        self.state = ConnectionState::Disconnected;
        // Reset the flag so a reconnect starts clean.
        self.cancel.store(false, Ordering::Relaxed);
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

        tracing::debug!(
            "handshake: query_cmd = {:?}, ini_uses_modern = {}",
            query_cmd,
            ini_uses_modern
        );

        let cmd_bytes = parse_command_string(&query_cmd);
        let cmd_byte = cmd_bytes.first().copied().unwrap_or(b'Q');

        // STRATEGY: Try CRC protocol first if INI specifies it (faster for compatible ECUs)
        // Then fall back to legacy. This prioritizes modern protocol for speed.

        if ini_uses_modern {
            tracing::debug!("handshake: trying CRC protocol first");

            // Clear buffers before CRC attempt
            if let Some(channel) = self.channel.as_mut() {
                let _ = channel.clear_input_buffer();
            }

            // Try the dialect's declared envelope byte order first, then the
            // flipped order. The ECU-type detection sets the right order for
            // known dialects (Speeduino little-endian, rusEFI big-endian), but
            // a wrong guess used to cost the entire CRC path: both sides
            // misparse each other's length field as 256 and time out (D1).
            let first_order = self.envelope_order;
            let orders = [
                first_order,
                match first_order {
                    EnvelopeOrder::BigEndian => EnvelopeOrder::LittleEndian,
                    EnvelopeOrder::LittleEndian => EnvelopeOrder::BigEndian,
                },
            ];
            for (attempt, order) in orders.into_iter().enumerate() {
                self.envelope_order = order;
                if attempt > 0 {
                    tracing::debug!(
                        "handshake: retrying CRC with flipped envelope order {:?}",
                        order
                    );
                    // If the first frame's length was misparsed, the firmware
                    // is still inside its ~400 ms SERIAL_TIMEOUT waiting for a
                    // payload that will never come — bytes sent now are eaten
                    // as that payload. Let the window expire before retrying.
                    std::thread::sleep(Duration::from_millis(450));
                    if let Some(channel) = self.channel.as_mut() {
                        let _ = channel.clear_input_buffer();
                    }
                }
                let packet = Packet::new(cmd_bytes.clone());
                if let Ok(response_packet) = self.send_packet(packet) {
                    tracing::debug!(
                        "handshake: CRC protocol succeeded (envelope order {:?})",
                        order
                    );
                    self.use_modern_protocol = true;

                    // Handle status byte: response may start with 0x00 (success)
                    let payload = &response_packet.payload;
                    let signature_bytes = if !payload.is_empty() && payload[0] == 0 {
                        &payload[1..]
                    } else {
                        payload.as_slice()
                    };

                    let signature = String::from_utf8_lossy(signature_bytes).trim().to_string();
                    tracing::debug!("handshake: CRC success, signature = {:?}", signature);
                    return Ok(signature);
                }
            }
            // Neither order worked — restore the declared order for any later
            // attempts and fall through to legacy.
            self.envelope_order = first_order;
            tracing::debug!("handshake: CRC protocol failed in both byte orders, trying legacy");
        }

        // Try legacy protocol (raw ASCII command)
        tracing::debug!(
            "handshake: trying legacy protocol, sending byte 0x{:02x}",
            cmd_byte
        );

        // Clear buffers before legacy attempt
        if let Some(channel) = self.channel.as_mut() {
            let _ = channel.clear_input_buffer();
        }

        match self.send_raw_command(&[cmd_byte]) {
            Ok(response) => {
                tracing::debug!("handshake: legacy succeeded, {} bytes", response.len());
                self.use_modern_protocol = false;
                let signature = String::from_utf8_lossy(&response).trim().to_string();
                tracing::debug!("handshake: legacy success, signature = {:?}", signature);
                Ok(signature)
            }
            Err(e) => {
                tracing::debug!("handshake: legacy failed ({:?})", e);

                // If INI doesn't specify modern and legacy failed, try CRC as last resort
                if !ini_uses_modern {
                    tracing::debug!("handshake: trying CRC as fallback");

                    if let Some(channel) = self.channel.as_mut() {
                        let _ = channel.clear_input_buffer();
                    }
                    std::thread::sleep(Duration::from_millis(50));

                    let packet = Packet::new(cmd_bytes);
                    if let Ok(response_packet) = self.send_packet(packet) {
                        tracing::debug!("handshake: CRC fallback succeeded");
                        self.use_modern_protocol = true;

                        let payload = &response_packet.payload;
                        let signature_bytes = if !payload.is_empty() && payload[0] == 0 {
                            &payload[1..]
                        } else {
                            payload.as_slice()
                        };

                        let signature = String::from_utf8_lossy(signature_bytes).trim().to_string();
                        tracing::debug!(
                            "handshake: CRC fallback success, signature = {:?}",
                            signature
                        );
                        return Ok(signature);
                    }
                }

                Err(e)
            }
        }
    }

    /// Send raw bytes and get response (for initial handshake)
    /// Uses non-blocking reads with bytes_to_read() polling for reliable timeout behavior
    fn send_raw_command(&mut self, cmd: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        self.send_raw_command_expecting(cmd, None)
    }

    /// Whether enough bytes have arrived to stop reading without waiting for
    /// the line to fall quiet.
    ///
    /// Split out from the read loop so the decision can be tested directly:
    /// the loop itself needs a live serial channel, and getting this wrong
    /// returns a truncated message rather than an obvious error.
    fn response_is_complete_impl(received: usize, expected: Option<usize>) -> bool {
        match expected {
            // A declared length of zero means "unknown", not "expect nothing" —
            // an INI that omits ochBlockSize parses as 0, and treating that as
            // complete would return an empty response immediately.
            Some(want) if want > 0 => received >= want,
            _ => false,
        }
    }

    /// As [`send_raw_command`](Self::send_raw_command), but returns as soon as
    /// `expected_len` bytes have arrived instead of waiting out the
    /// inter-character timeout.
    ///
    /// Without a length the loop can only tell a message has ended by seeing
    /// the line go quiet, so every response costs the full inter-character
    /// timeout on top of its transmission time. For a repeated poll that
    /// dominates: an INI declaring `ochBlockSize = 130` at 115200 baud takes
    /// about 11 ms to transmit those bytes and then waits 100 ms to be sure
    /// nothing follows, capping realtime updates near 8 Hz regardless of the
    /// rate requested.
    ///
    /// `expected_len` is only supplied where the length is declared by the INI
    /// rather than guessed. If more bytes than expected are in flight, the
    /// remainder is discarded by the `clear_input_buffer` at the top of the
    /// next command, so an early return cannot desync the following exchange.
    /// When `None`, or when the peer sends fewer bytes than promised, the
    /// inter-character timeout still terminates the read exactly as before.
    fn send_raw_command_expecting(
        &mut self,
        cmd: &[u8],
        expected_len: Option<usize>,
    ) -> Result<Vec<u8>, ProtocolError> {
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
        // Clone the cancel handle before borrowing the channel so the cancel check
        // in the read loop does not conflict with the mutable channel borrow.
        let cancel = Arc::clone(&self.cancel);

        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;

        tracing::debug!("send_raw_command: clearing buffers before send");
        // Clear any stale data in buffers
        let _ = channel.clear_input_buffer();
        let _ = channel.clear_output_buffer();

        tracing::debug!(
            "send_raw_command: sending {} bytes: {:02x?}",
            cmd.len(),
            cmd
        );

        // Start timing for adaptive timing
        let send_start = Instant::now();

        // Send command bytes and wait for transmission
        // Use write_and_wait which avoids the blocking tcdrain issue
        write_and_wait(channel, cmd, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        tracing::debug!(
            "send_raw_command: command sent, timeout={}ms, inter_char={}ms",
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
                tracing::debug!("send_raw_command: overall timeout reached");
                break;
            }

            // Cancellation check (Issue #71): abort promptly if disconnect() was called
            // while this blocking read is in flight.
            if cancel.load(Ordering::Relaxed) {
                tracing::debug!("send_raw_command: cancelled by disconnect");
                self.reset_adaptive_timing_on_error();
                return Err(ProtocolError::ConnectionClosed);
            }

            // Check how many bytes are available without blocking
            let available = match channel.bytes_to_read() {
                Ok(n) => n,
                Err(e) => {
                    tracing::debug!("send_raw_command: bytes_to_read error: {}", e);
                    return Err(ProtocolError::SerialError(e.to_string()));
                }
            };

            if available > 0 {
                let to_read = std::cmp::min(available as usize, buffer.len());
                match channel.read(&mut buffer[..to_read]) {
                    Ok(0) => {
                        tracing::debug!("send_raw_command: read returned 0 (EOF)");
                        break;
                    }
                    Ok(n) => {
                        response.extend_from_slice(&buffer[..n]);
                        last_data_time = Instant::now();
                        tracing::debug!(
                            "send_raw_command: read {} bytes, total = {}, data = {:02x?}",
                            n,
                            response.len(),
                            &buffer[..n]
                        );
                        // The whole message is present, so there is nothing to
                        // gain by waiting for the line to fall quiet.
                        if Self::response_is_complete_impl(response.len(), expected_len) {
                            tracing::debug!(
                                "send_raw_command: got expected {:?} bytes, returning early",
                                expected_len
                            );
                            break;
                        }
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        // Non-blocking, continue polling
                    }
                    Err(e) => {
                        tracing::debug!("send_raw_command: read error: {}", e);
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
                    tracing::debug!("send_raw_command: inter-character timeout, message complete");
                    break;
                }
                std::thread::sleep(Duration::from_millis(poll_interval));
            }
        }

        let elapsed = send_start.elapsed();
        tracing::debug!(
            "send_raw_command: completed with {} bytes in {}ms: {:?}",
            response.len(),
            elapsed.as_millis(),
            String::from_utf8_lossy(&response)
        );

        if response.is_empty() {
            self.reset_adaptive_timing_on_error();
            return Err(ProtocolError::Timeout);
        }

        // Record rx bytes/packets for metrics
        self.rx_bytes = self.rx_bytes.saturating_add(response.len() as u64);
        self.rx_packets = self.rx_packets.saturating_add(1);

        // Record response time for adaptive timing
        self.record_response_time(elapsed);

        Ok(response)
    }

    /// Send raw bytes WITHOUT waiting for response (for burn commands)
    /// ECUs typically don't respond during flash write operations
    fn send_raw_command_no_response(&mut self, cmd: &[u8]) -> Result<(), ProtocolError> {
        let baud_rate = self.config.baud_rate;
        let min_wait = Some(self.get_effective_min_wait());

        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;

        tracing::debug!(
            "send_raw_command_no_response: sending {} bytes: {:02x?}",
            cmd.len(),
            cmd
        );

        // Send command bytes and wait for transmission to complete
        write_and_wait(channel, cmd, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        tracing::debug!("send_raw_command_no_response: command sent, not waiting for response");

        Ok(())
    }

    /// Send CRC packet WITHOUT waiting for response (for burn commands)
    fn send_packet_no_response(&mut self, packet: Packet) -> Result<(), ProtocolError> {
        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;
        let bytes = packet.to_bytes_ordered(self.envelope_order);

        tracing::debug!("send_packet_no_response: sending {} bytes", bytes.len());

        self.tx_bytes = self.tx_bytes.saturating_add(bytes.len() as u64);
        self.tx_packets = self.tx_packets.saturating_add(1);
        channel
            .write_all(&bytes)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;
        channel
            .flush()
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        tracing::debug!("send_packet_no_response: packet sent, not waiting for response");

        Ok(())
    }

    /// Send a legacy (ASCII) command and get response
    #[allow(dead_code)]
    fn send_legacy_command(&mut self, cmd: Command) -> Result<Vec<u8>, ProtocolError> {
        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;

        // Send single command byte
        let legacy_bytes = [cmd.legacy_byte()];
        self.tx_bytes = self.tx_bytes.saturating_add(legacy_bytes.len() as u64);
        self.tx_packets = self.tx_packets.saturating_add(1);
        channel
            .write_all(&legacy_bytes)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;
        channel
            .flush()
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        // Read response with timeout
        let mut response = Vec::new();
        let mut buffer = [0u8; 256];
        let start = Instant::now();
        let timeout = Duration::from_millis(cmd.timeout_ms());

        loop {
            match channel.read(&mut buffer) {
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

        // Record rx metrics
        self.rx_bytes = self.rx_bytes.saturating_add(response.len() as u64);
        if !response.is_empty() {
            self.rx_packets = self.rx_packets.saturating_add(1);
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
        // Clone the cancel handle before borrowing the channel so the disjoint
        // borrow of self.cancel does not conflict with the mutable channel borrow.
        let cancel = Arc::clone(&self.cancel);

        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;

        // NOTE: Do NOT call clear_input_buffer() here. This is a length-prefixed framed
        // protocol; every response is fully consumed by read_exact_timeout. On fast local
        // TCP connections, clearing between packets can accidentally drain the response that
        // already arrived, desynchronizing the stream and producing CRC mismatches.

        // Start timing for adaptive timing
        let send_start = Instant::now();

        // Send packet and wait for transmission
        let bytes = packet.to_bytes_ordered(self.envelope_order);
        // Trace the actual bytes (capped) so a lost capture can be reconstructed
        // from the session log — the framed path only counted bytes before, so
        // tooth/composite payloads left no trace. Legacy path already does this.
        tracing::trace!("send_packet: tx {} bytes: {:02x?}", bytes.len(), &bytes[..bytes.len().min(64)]);
        // Use write_and_wait which avoids the blocking tcdrain issue
        self.tx_bytes = self.tx_bytes.saturating_add(bytes.len() as u64);
        self.tx_packets = self.tx_packets.saturating_add(1);
        write_and_wait(channel, &bytes, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        // Helper to read exact bytes with timeout
        // Uses bytes_to_read() polling to avoid blocking read() calls on Linux
        fn read_exact_timeout(
            channel: &mut Box<dyn CommunicationChannel>,
            buf: &mut [u8],
            timeout: Duration,
            poll_ms: u64,
            cancel: &AtomicBool,
        ) -> Result<(), ProtocolError> {
            let start = Instant::now();
            let mut offset = 0;

            while offset < buf.len() {
                if start.elapsed() > timeout {
                    tracing::warn!(
                        "read_exact_timeout: timed out after reading {} of {} bytes",
                        offset,
                        buf.len()
                    );
                    return Err(ProtocolError::Timeout);
                }

                // Cancellation check (Issue #71): abort promptly if disconnect() was called.
                if cancel.load(Ordering::Relaxed) {
                    tracing::debug!("read_exact_timeout: cancelled by disconnect");
                    return Err(ProtocolError::ConnectionClosed);
                }

                // Check how many bytes are available
                let available = channel
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
                match channel.read(&mut buf[offset..offset + to_read]) {
                    Ok(0) => {
                        tracing::warn!("read_exact_timeout: EOF after {} bytes", offset);
                        return Err(ProtocolError::Timeout);
                    }
                    Ok(n) => {
                        offset += n;
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!("read_exact_timeout: error: {}", e);
                        return Err(ProtocolError::SerialError(e.to_string()));
                    }
                }
            }
            Ok(())
        }

        // Read response header (2 bytes for length)
        let mut header = [0u8; 2];
        if let Err(e) = read_exact_timeout(channel, &mut header, timeout, poll_interval_ms, &cancel)
        {
            // Drain any buffered bytes first (uses channel borrow), then reset timing
            let _ = channel.clear_input_buffer();
            self.reset_adaptive_timing_on_error();
            return Err(e);
        }

        // Parse length
        // Length field byte order follows the ECU dialect: Speeduino frames
        // little-endian, rusEFI/msEnvelope big-endian. Misreading it turns a
        // 1-byte response into a 256-byte wait (D1).
        let length = self.envelope_order.read_u16(&header) as usize;
        if length > super::MAX_PACKET_SIZE {
            tracing::warn!(
                "send_packet: response length {} exceeds MAX_PACKET_SIZE",
                length
            );
            let _ = channel.clear_input_buffer();
            return Err(ProtocolError::BufferOverflow);
        }

        // Read payload + CRC
        let mut payload_and_crc = vec![0u8; length + 4];
        if let Err(e) = read_exact_timeout(
            channel,
            &mut payload_and_crc,
            timeout,
            poll_interval_ms,
            &cancel,
        ) {
            // Drain the rest of the packet body (uses channel borrow), then reset timing
            drain_input_with_timeout(
                channel,
                length + 4,
                Duration::from_millis(500),
                poll_interval_ms,
            );
            self.reset_adaptive_timing_on_error();
            return Err(e);
        }

        // Record response time for adaptive timing
        let elapsed = send_start.elapsed();
        self.record_response_time(elapsed);

        // Reconstruct full packet for parsing
        let mut full_packet = Vec::with_capacity(2 + length + 4);
        full_packet.extend_from_slice(&header);
        full_packet.extend_from_slice(&payload_and_crc);

        // Track received bytes/packets for metrics display
        self.rx_bytes = self.rx_bytes.saturating_add(full_packet.len() as u64);
        self.rx_packets = self.rx_packets.saturating_add(1);
        // Trace the raw response (capped) before CRC parsing, so it survives in
        // the log even when CRC validation subsequently fails.
        tracing::trace!("send_packet: rx {} bytes: {:02x?}", full_packet.len(), &full_packet[..full_packet.len().min(64)]);

        // If CRC parsing fails, the full packet was already consumed from the TCP
        // stream (exact bytes read = 2 + length + 4), so the stream IS aligned.
        // No drain needed on CRC mismatch — just return the error.
        Packet::from_bytes_ordered(
            &full_packet,
            self.envelope_order,
            self.config.permissive_crc,
        )
    }

    /// Decide which runtime fetch command to use (Burst vs OCH)
    /// Decide which runtime fetch command to use (Burst vs OCH)
    ///
    /// **Issue #71 fix**: For Speeduino / MegaSquirt (MS2/MS3), the Burst ('A')
    /// command is the well-tested, high-throughput path (observed ~1 KB/sec).
    /// Auto mode previously could silently switch to OCH based on loose
    /// heuristics (maxUnusedRuntimeRange, slow-link, adaptive-timing averages),
    /// collapsing throughput to ~13 B/sec and stalling gauges. These ECUs now
    /// stay on Burst unless the user explicitly forces OCH.
    ///
    /// For rusEFI / FOME / epicEFI (little-endian, msEnvelope_1.0), OCH is the
    /// standard modern realtime path, so the existing heuristics are retained.
    pub fn choose_runtime_command(&self) -> (RuntimeFetch, String) {
        // Respect explicit overrides
        let forced = self.config.runtime_packet_mode;
        let burst_cmd = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.burst_get_command.clone())
            .unwrap_or_else(|| "A".to_string());
        let och_cmd_opt = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.och_get_command.clone());

        if forced == RuntimePacketMode::ForceBurst {
            // A Speeduino in new-comms mode ignores a framed 'A' entirely
            // (zero bytes back), so honouring this override verbatim yields a
            // permanently empty stream with no error. Prefer OCH there and
            // say so; the override still means Burst everywhere it works.
            if self.use_modern_protocol {
                if let Some(och) = och_cmd_opt.clone() {
                    return (
                        RuntimeFetch::OCH(och),
                        "force: ForceBurst (burst is a no-op in new-comms; using OCH)".to_string(),
                    );
                }
            }
            return (
                RuntimeFetch::Burst(burst_cmd),
                "force: ForceBurst".to_string(),
            );
        }
        if forced == RuntimePacketMode::ForceOCH {
            if let Some(och) = och_cmd_opt.clone() {
                return (RuntimeFetch::OCH(och), "force: ForceOCH".to_string());
            } else {
                return (
                    RuntimeFetch::Burst(burst_cmd),
                    "force: ForceOCH (no OCH cmd, fallback to burst)".to_string(),
                );
            }
        }
        if forced == RuntimePacketMode::Disabled {
            return (
                RuntimeFetch::Burst(burst_cmd),
                "override: Disabled".to_string(),
            );
        }

        // === Auto mode ===
        //
        // For Speeduino / MS2 / MS3 (big-endian, classic MegaSquirt lineage),
        // Burst ('A') is the canonical high-throughput realtime path. The OCH
        // heuristics below were observed to mis-select OCH on real Speeduino
        // 202501 hardware, dropping throughput from ~1 KB/sec to ~13 B/sec
        // (Issue #71). Lock these ECUs to Burst in Auto mode.
        let burst_ecu = matches!(
            self.ecu_type,
            EcuType::Speeduino | EcuType::MS2 | EcuType::MS3 | EcuType::Unknown
        );

        // ...but only while talking legacy. Once the CRC handshake succeeds the
        // ECU is in new-comms mode, where Burst's bare 'A' is not a command at
        // all: a Speeduino 2025.01.4 answered an unframed 'A' with a framed
        // error (`00 01 | 80 | crc32`) and ignored a *framed* 'A' entirely
        // (zero bytes back). New-comms fetches runtime data with the INI's
        // ochGetCommand — `r $tsCanId 0x30 %2o %2c` — so use OCH there.
        // Scoped to the burst lineage: rusEFI-family ECUs keep their
        // existing heuristic chain below (they are always modern-protocol,
        // and their burst path is framed and answered).
        if self.use_modern_protocol && burst_ecu {
            if let Some(och) = och_cmd_opt.clone() {
                return (
                    RuntimeFetch::OCH(och),
                    "auto: OCH (modern protocol negotiated)".to_string(),
                );
            }
        }

        // Unknown ECU type: also default to Burst to be safe. Only rusEFI-lineage
        // ECUs (detected from the INI signature) are allowed to auto-select OCH.
        if burst_ecu {
            return (
                RuntimeFetch::Burst(burst_cmd),
                format!("auto: Burst (ecu={})", self.ecu_type.display_name()),
            );
        }

        // rusEFI / FOME / epicEFI: apply the original heuristics to choose OCH.

        // 1) INI hint: maxUnusedRuntimeRange > 0 => prefer OCH if available
        if let Some(p) = &self.protocol_settings {
            if p.max_unused_runtime_range > 0 {
                if let Some(och) = och_cmd_opt.clone() {
                    return (
                        RuntimeFetch::OCH(och),
                        "ini hint: maxUnusedRuntimeRange".to_string(),
                    );
                }
            }
        }

        // 2) Port name heuristic
        if self.is_slow_link() {
            if let Some(och) = och_cmd_opt.clone() {
                return (RuntimeFetch::OCH(och), "heuristic: slow link".to_string());
            }
        }

        // 3) Adaptive timing heuristic
        if let Some((avg, _count)) = self.adaptive_timing_stats() {
            let avg_ms = avg.as_millis() as u64;
            if avg_ms > 50 {
                if let Some(och) = och_cmd_opt.clone() {
                    return (
                        RuntimeFetch::OCH(och),
                        format!("adaptive: avg={}ms", avg_ms),
                    );
                }
            }
        }

        // Default: use burst
        (RuntimeFetch::Burst(burst_cmd), "default: burst".to_string())
    }

    /// Determine if the configured port looks like a slow link (bluetooth, tcp, rfcomm)
    pub(crate) fn is_slow_link(&self) -> bool {
        let pn = self.config.port_name.to_lowercase();
        if pn.contains("rfcomm")
            || pn.contains("bluetooth")
            || pn.contains("tcp")
            || pn.contains("telnet")
            || pn.contains("wifi")
        {
            return true;
        }
        // Baud-rate heuristic: low baud suggests slow link
        if self.config.baud_rate < 57600 {
            return true;
        }
        false
    }

    /// Get real-time data from ECU
    pub fn get_realtime_data(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let (choice, _reason) = self.choose_runtime_command();

        // Safety net (Issue #71): if Auto/forced OCH was selected but the INI
        // did not declare a valid och_block_size, the OCH command cannot be
        // framed correctly and the ECU will return a mis-sized response that
        // stalls the stream. Fall back to Burst instead of guessing 256 bytes.
        let choice = match &choice {
            RuntimeFetch::OCH(_) => {
                let och_block_size = self
                    .protocol_settings
                    .as_ref()
                    .map(|p| p.och_block_size)
                    .unwrap_or(0);
                if och_block_size == 0 {
                    tracing::debug!(
                        "get_realtime_data: OCH selected but och_block_size=0, \
                         falling back to Burst to avoid stream stall (Issue #71)"
                    );
                    let burst_cmd = self
                        .protocol_settings
                        .as_ref()
                        .and_then(|p| p.burst_get_command.clone())
                        .unwrap_or_else(|| "A".to_string());
                    RuntimeFetch::Burst(burst_cmd)
                } else {
                    choice
                }
            }
            _ => choice,
        };

        match choice {
            RuntimeFetch::Burst(cmd) => {
                // Frame the request whenever the handshake actually negotiated
                // CRC. `use_modern_protocol` is authoritative here: the
                // handshake clears it on legacy fallback, so a true value means
                // the ECU answered a framed command and is now in new-comms
                // mode — where a bare command byte is rejected.
                //
                // This used to force the raw byte for Speeduino/MS2/MS3/Unknown
                // on the belief that they "do not accept a CRC-framed Burst
                // request". That belief formed while CRC never negotiated on
                // those ECUs. Once it did (Speeduino 2025.01.4, big-endian
                // envelope), the override became the bug: the ECU sat in
                // new-comms answering every bare 'A' with a framed error
                // (`00 01 | 80 | crc32`), so realtime data froze at ~14 B/s and
                // every value went stale while the UI still showed Connected.
                if self.use_modern_protocol {
                    let expected_len = self
                        .protocol_settings
                        .as_ref()
                        .map(|p| p.och_block_size as usize)
                        .unwrap_or(0);
                    let cmd_bytes = cmd.as_bytes().to_vec();
                    let packet = Packet::new(cmd_bytes);
                    let response = self.send_packet(packet)?;
                    let payload = &response.payload;
                    Ok(strip_status_byte(payload, expected_len, "Burst"))
                } else {
                    let cmd_byte = cmd.as_bytes().first().copied().unwrap_or(b'A');
                    // This is the hot path — it repeats for every realtime
                    // update — and the INI declares exactly how many bytes the
                    // ECU will send, so the read need not wait out the
                    // inter-character timeout to discover the message ended.
                    let expected_len = self
                        .protocol_settings
                        .as_ref()
                        .map(|p| p.och_block_size as usize)
                        .filter(|n| *n > 0);
                    self.send_raw_command_expecting(&[cmd_byte], expected_len)
                }
            }
            RuntimeFetch::OCH(cmd) => {
                // OCH: expect block response of och_block_size; send command accordingly
                let cmd_bytes = if cmd.contains('%') {
                    // If format string provided (e.g. "O%2o%2c"), build command with proper values
                    let block_size = self
                        .protocol_settings
                        .as_ref()
                        .map(|p| {
                            if p.och_block_size > 0 {
                                p.och_block_size
                            } else {
                                0
                            }
                        })
                        .unwrap_or(0) as u16;

                    if block_size == 0 {
                        tracing::warn!("get_realtime_data: OCH selected but block size is 0! Defaulting to 256.");
                        // Fallback to 256 if 0, but log warning
                    }
                    let effective_size = if block_size > 0 { block_size } else { 256 };

                    self.command_builder
                        .build_och_command(&cmd, effective_size)?
                } else {
                    // Otherwise assume raw ASCII command (e.g. "A" or "O")
                    cmd.as_bytes().to_vec()
                };

                // Log the final command bytes for debugging
                {
                    static OCH_LOG_COUNT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let n = OCH_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if n < 3 || n.is_multiple_of(100) {
                        tracing::warn!(
                            "tick={} use_modern={} cmd_bytes={:02x?} och_block_size={}",
                            n,
                            self.use_modern_protocol,
                            cmd_bytes,
                            self.protocol_settings
                                .as_ref()
                                .map(|p| p.och_block_size)
                                .unwrap_or(0),
                        );
                    }
                }

                if self.use_modern_protocol {
                    let expected_och_len = self
                        .protocol_settings
                        .as_ref()
                        .map(|p| p.och_block_size as usize)
                        .unwrap_or(0);
                    let packet = Packet::new(cmd_bytes);
                    let response = self.send_packet(packet)?;
                    let payload = &response.payload;
                    Ok(strip_status_byte(payload, expected_och_len, "OCH"))
                } else {
                    // For legacy protocol, usually single byte command
                    // If cmd_bytes > 1, send all bytes (rare case for legacy but possible)
                    if cmd_bytes.len() > 1 {
                        self.send_raw_command(&cmd_bytes)
                    } else {
                        let cmd_byte = cmd_bytes.first().copied().unwrap_or(b'A');
                        self.send_raw_command(&[cmd_byte])
                    }
                }
            }
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
            // Legacy protocol: the reply length is exactly the requested
            // count, so let the read return as soon as it has arrived instead
            // of waiting out the inter-character timeout — the read-back
            // verify does one of these per written chunk, under the
            // connection lock, while the realtime poll waits.
            self.send_raw_command_expecting(&cmd, Some(params.length as usize))
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

    /// Temporarily disable auto-burn-on-page-change (for bulk multi-page writes).
    pub fn set_auto_burn_on_page_change(&mut self, enabled: bool) {
        self.config.auto_burn_on_page_change = enabled;
    }

    /// Drop any unread RX bytes so the next framed command starts clean.
    pub fn clear_rx_buffer(&mut self) {
        if let Some(channel) = self.channel.as_mut() {
            let _ = channel.clear_input_buffer();
        }
    }

    /// The largest payload one write frame may carry.
    ///
    /// `blockingFactor` is the ECU's serial buffer less the envelope — Speeduino
    /// spells this out in its own INI ("257-6=251"). The command header (byte,
    /// identifier, offset, count) is carried *inside* that payload, so it comes
    /// off the top as well.
    ///
    /// Both `write_memory` and `write_page` chunk, so they must agree: when
    /// `write_page` split at the full blocking factor it handed `write_memory`
    /// chunks 8 bytes too large, which then re-split every one of them into a
    /// full frame plus an 8-byte runt — an extra round-trip and an extra pacing
    /// delay per chunk on every bulk page write.
    fn effective_write_chunk(&self) -> usize {
        let blocking_factor = self
            .protocol_settings
            .as_ref()
            .map(|p| p.blocking_factor)
            .unwrap_or(256)
            .max(1) as usize;
        blocking_factor.saturating_sub(8).max(1)
    }

    /// How long to leave the link idle after handing the ECU a write frame of
    /// `frame_len` bytes, before sending it anything else.
    ///
    /// A legacy write is answered by silence, so the host has no signal that
    /// the ECU has finished with a frame and must simply wait long enough. Two
    /// things set how long. The frame's own wire time is a floor — on a real
    /// UART a 250-byte frame occupies a 115200 link for about 22 ms, and USB
    /// CDC, which every modern Speeduino is reached through, delivers it at USB
    /// speed instead and removes that natural spacing. On top of that the
    /// firmware needs a service window to apply the bytes it just received;
    /// until it has, the next command on the wire is consumed as table data.
    ///
    /// The window was measured directly against the bench simulator on
    /// 19 Aug 2026, replaying the frames with no app in the loop and sweeping
    /// the gap: at 10 ms the following page read was swallowed as table data
    /// and the read returned nothing; 20 ms was the first clean value; 30 ms
    /// and above were clean, which is what the raw-protocol reference writer
    /// has always used and why it has never reproduced the corruption. Note
    /// that this is not the same as the chunking bug — LibreTune's frames were
    /// correctly sized and 22.8 ms apart, and the *last* frame's 10.5 ms tail
    /// gap alone was enough to corrupt the table.
    ///
    /// A modern CRC ECU acknowledges the write, and that acknowledgement is
    /// proof it is done with the frame, so there is nothing to guess at there.
    fn inter_frame_delay(&self, frame_len: usize) -> Duration {
        let baud = self.config.baud_rate.max(1) as u64;
        // 10 bits per byte on the wire: 8 data, start, stop.
        let wire_ms = (frame_len as u64 * 10 * 1000).div_ceil(baud);
        let ini_ms = self.get_effective_min_wait();
        let floor = if self.use_modern_protocol {
            5
        } else {
            LEGACY_WRITE_SETTLE_MS
        };
        let settle = ini_ms.max(floor);
        // On unix, write_and_wait has already slept out the transmit time, so
        // the ECU-side quiet window is simply `settle`. Elsewhere the write
        // returns as soon as the OS takes the bytes: over USB CDC the wire
        // time is negligible, but through a real UART bridge (FTDI, BT-SPP)
        // the frame is still draining — taking max() there would leave only
        // settle-minus-wire-time of true quiet, inside the measured
        // corruption zone. Summing is cheap insurance: at worst it doubles a
        // ~22 ms allowance per 250-byte frame.
        if cfg!(unix) {
            Duration::from_millis(wire_ms.max(settle))
        } else {
            Duration::from_millis(wire_ms + settle)
        }
    }

    /// Write a full page to ECU, respecting blocking factor (same chunking as `read_page`).
    pub fn write_page(&mut self, page: u8, data: &[u8]) -> Result<(), ProtocolError> {
        let chunk_size = self.effective_write_chunk();

        let mut offset = 0usize;
        while offset < data.len() {
            let end = (offset + chunk_size).min(data.len());
            let chunk = &data[offset..end];
            let params = WriteMemoryParams {
                page,
                offset: offset as u16,
                data: chunk.to_vec(),
                can_id: 0,
            };

            // Retry transient Windows USB/serial timeouts (e.g. os error 121).
            let mut last_err = None;
            for attempt in 0..3 {
                match self.write_memory(params.clone()) {
                    Ok(()) => {
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        let transient = msg.contains("121")
                            || msg.to_ascii_lowercase().contains("timeout")
                            || msg.to_ascii_lowercase().contains("semaphore");
                        if !transient || attempt == 2 {
                            return Err(e);
                        }
                        tracing::warn!(
                            "write_page: transient error on page {} offset {} (attempt {}): {}",
                            page,
                            offset,
                            attempt + 1,
                            msg
                        );
                        self.clear_rx_buffer();
                        std::thread::sleep(Duration::from_millis(50 * (attempt as u64 + 1)));
                        last_err = Some(e);
                    }
                }
            }
            if let Some(e) = last_err {
                return Err(e);
            }

            offset = end;
            // write_memory paces each frame it sends; a second delay here
            // would only double the cost of every bulk page write.
        }

        // Confirm the ECU holds what was just sent. A bulk page write had no
        // verification of any kind: write_memory is fire-and-forget on legacy,
        // and write_memory_verified deliberately returns early on the modern
        // protocol because each frame is acknowledged - but a frame ack says
        // the bytes arrived, not that the page assembled into what was meant.
        if let Err(e) = self.verify_page_crc(page, data) {
            match e {
                ProtocolError::PageCrcMismatch { .. } => return Err(e),
                // No declared command, or the ECU would not answer: the write
                // itself succeeded, so warn rather than failing it. Reporting a
                // completed write as failed would be its own kind of wrong.
                other => tracing::warn!(
                    "write_page: page {} could not be CRC-verified: {}",
                    page,
                    other
                ),
            }
        }

        Ok(())
    }

    /// Ask the ECU for a page's CRC32 and compare it with `expected`.
    ///
    /// The INI declares the command per page (`crc32CheckCommand = "d%2i"` on
    /// Speeduino) and the firmware implements it, returning a return code
    /// followed by a big-endian CRC32 of the page as the ECU currently holds
    /// it. Nothing called it: `build_crc_command` had no callers anywhere in
    /// the tree, so a bulk write went out entirely unchecked.
    ///
    /// One three-byte command per page against re-reading the page in full -
    /// 2,592 bytes across fifteen pages on this ECU.
    ///
    /// Verified against a Speeduino 202501: the value it returns is a standard
    /// reflected CRC-32 of exactly the bytes `read_page` gives back, matching
    /// on every page tested.
    pub fn verify_page_crc(&mut self, page: u8, expected: &[u8]) -> Result<(), ProtocolError> {
        let Some(format) = self
            .protocol_settings
            .as_ref()
            .and_then(|p| p.crc32_check_commands.get(page as usize).cloned())
            .filter(|f| !f.is_empty())
        else {
            return Err(ProtocolError::ProtocolError(format!(
                "no crc32CheckCommand declared for page {page}"
            )));
        };

        // Take the identifier the same way the read and write paths do rather
        // than deriving it. `get_page_identifier` decodes the INI's declared
        // bytes little-endian and `build_command` re-encodes them the same way,
        // so the two inversions cancel and the bytes leave in the order the
        // firmware expects. Computing `page + 1` here instead produced
        // `[64, 01, 00]` where the ECU wanted `[64, 00, 01]`, and it answered
        // by not answering at all.
        let page_id = self.get_page_identifier(page);
        let cmd =
            self.command_builder
                .build_crc_command(&format, page_id, 0, expected.len() as u16)?;

        self.clear_rx_buffer();
        let reply = self.send_raw_bytes_with_response(&cmd, self.get_effective_timeout())?;
        if reply.len() < 4 {
            return Err(ProtocolError::ProtocolError(format!(
                "page {page} CRC reply was {} bytes, expected at least 4",
                reply.len()
            )));
        }
        // Big-endian, like every other multi-byte value this firmware writes.
        let reported = u32::from_be_bytes([reply[0], reply[1], reply[2], reply[3]]);

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(expected);
        let local = hasher.finalize();

        if reported == local {
            Ok(())
        } else {
            Err(ProtocolError::PageCrcMismatch {
                page,
                expected: local,
                actual: reported,
            })
        }
    }

    /// Write memory to ECU using INI-defined command format.
    ///
    /// Payloads larger than the INI's `blockingFactor` are split into
    /// sequential writes here rather than trusting callers to pre-chunk. The
    /// ECU's serial RX buffer is exactly what `blockingFactor` declares (257
    /// bytes minus protocol overhead on a Mega2560); a longer frame gets its
    /// tail silently dropped by the ECU, which then consumes the next bytes on
    /// the wire as if they were table data — observed on a running engine as a
    /// partially-applied, corrupted VE table (drove visibly jerkily) when a
    /// 263-byte full-table write went out unchunked. Six command paths call
    /// this directly with unbounded payloads, so the guard belongs here, not
    /// in each caller.
    pub fn write_memory(&mut self, params: WriteMemoryParams) -> Result<(), ProtocolError> {
        let max_data = self.effective_write_chunk();
        if params.data.len() > max_data {
            let mut offset = params.offset as usize;
            let mut remaining = params.data.as_slice();
            while !remaining.is_empty() {
                let take = remaining.len().min(max_data);
                let (head, tail) = remaining.split_at(take);
                self.write_memory(WriteMemoryParams {
                    can_id: params.can_id,
                    page: params.page,
                    offset: offset as u16,
                    data: head.to_vec(),
                })?;
                offset += take;
                remaining = tail;
                // Pacing is handled once, after the frame actually goes out.
            }
            return Ok(());
        }

        // Auto-burn safety policy (spec §6.2): if the previous write targeted a different
        // page, burn it to flash before writing to the new page. Prevents partial-flash
        // corruption on power loss.
        if self.config.auto_burn_on_page_change {
            if let Some(prev_page) = self.last_written_page {
                if prev_page != params.page {
                    tracing::info!(
                        "auto-burn: page change {} -> {}, burning previous page first",
                        prev_page,
                        params.page
                    );
                    self.burn(BurnParams {
                        page: prev_page,
                        can_id: params.can_id,
                    })?;
                }
            }
        }

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
        let cmd_len = cmd.len();

        let result = if self.use_modern_protocol {
            // Modern protocol: wrap in CRC packet
            let packet = Packet::new(cmd);
            let response = self.send_packet(packet)?;
            check_write_response_status(&response)
        } else {
            // Legacy protocol: value writes (Speeduino 'M', MS 'w') define no
            // response, exactly like legacy burn above. Waiting for one stalls
            // every write for the full serial timeout (~2 s), and the timeout
            // is then misreported as a write failure — single edits appear to
            // fail (they actually landed), and write_page's retry loop treats
            // the phantom failure as fatal and aborts bulk writes partway.
            self.send_raw_command_no_response(&cmd)?;
            Ok(())
        };

        if result.is_ok() {
            self.last_written_page = Some(params.page);
            self.dirty_pages.insert(params.page);
            // Leave the link idle long enough for the ECU to drain this frame
            // before anything else — the next chunk, a burn, a read-back, or a
            // realtime poll — is put on the wire. Nothing downstream can tell
            // that the buffer is still full, because a legacy write is
            // answered by silence either way.
            std::thread::sleep(self.inter_frame_delay(cmd_len));
        }
        result
    }

    /// As [`write_memory`](Self::write_memory), but read the region straight
    /// back and fail if the ECU is not holding what was sent.
    ///
    /// A legacy-protocol write is fire-and-forget: the ECU sends no status, so
    /// `write_memory` returning `Ok` means only "the bytes left the host". That
    /// is not the same as "the ECU stored them", and the difference has already
    /// bitten this project — an oversized frame had its tail dropped and the
    /// following write consumed as data, leaving a corrupted ignition table in
    /// RAM while both writes reported success. Chunking closes that particular
    /// hole; reading back is what catches the next one.
    pub fn write_memory_verified(
        &mut self,
        params: WriteMemoryParams,
    ) -> Result<(), ProtocolError> {
        let page = params.page;
        let offset = params.offset;
        let expected = params.data.clone();
        self.write_memory(params)?;

        // A modern-protocol write is acknowledged frame by frame
        // (`check_write_response_status`), so the ECU has already confirmed
        // storage; reading back would double the traffic for no added signal.
        // Legacy is answered by silence, which is what the read-back is for.
        if self.use_modern_protocol || expected.is_empty() {
            return Ok(());
        }

        // Read back in the same frame sizes the write used, so a mismatch
        // points at a real ECU-side difference rather than an oversized read.
        // From here on a failure is not "offline": the write already went out,
        // and an ECU that ate the read command as table data answers exactly
        // like a dead link — silence. Report it as unverified, never as a
        // routine connection warning.
        let chunk = self.effective_write_chunk();
        let mut actual = Vec::with_capacity(expected.len());
        let mut at = offset as usize;
        while actual.len() < expected.len() {
            let take = chunk.min(expected.len() - actual.len());
            let part = self
                .read_memory(ReadMemoryParams {
                    page,
                    offset: at as u16,
                    length: take as u16,
                    can_id: 0,
                })
                .map_err(|e| ProtocolError::WriteVerificationUnavailable {
                    page,
                    offset: at as u16,
                    reason: e.to_string(),
                })?;
            if part.len() < take {
                return Err(ProtocolError::WriteVerificationUnavailable {
                    page,
                    offset: at as u16,
                    reason: format!("read-back returned {} of {take} bytes", part.len()),
                });
            }
            actual.extend_from_slice(&part[..take]);
            at += take;
        }

        if let Some(i) = (0..expected.len()).find(|&i| expected[i] != actual[i]) {
            return Err(ProtocolError::WriteVerificationFailed {
                page,
                offset: offset.saturating_add(i as u16),
                expected: expected[i],
                actual: actual[i],
            });
        }

        Ok(())
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
            tracing::debug!("burn: page {} has empty burn command, skipping", page);
            return Ok(());
        }

        // Build command using INI format string (use page_id, not page index)
        let cmd = self
            .command_builder
            .build_burn_command(&burn_format, page_id)?;

        tracing::debug!(
            "burn: sending burn command for page {}, format='{}', cmd = {:02x?}",
            page,
            burn_format,
            cmd
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
        let declared = self
            .protocol_settings
            .as_ref()
            .map(|p| p.page_activation_delay.max(2000))
            .unwrap_or(2000) as u64;
        // The ECU starts its busy window when it *processes* the burn, not when
        // we put the command on the wire, so waiting exactly the declared
        // window can return while the ECU is still deaf. During that window it
        // is not servicing serial at all: bytes pile into the 257-byte receive
        // buffer unread. Measured on the bench 18 Aug 2026 — a 2000 ms wait
        // against a 2000 ms window let the next table write's first frame land
        // inside the window, where it sat unserviced until the second frame
        // overflowed the buffer, losing 13 bytes and leaving the ECU consuming
        // following commands as table data. The raw-protocol reference writer
        // has always waited 2.6 s here and has never reproduced it.
        let settle = BURN_SETTLE_MS;
        tracing::debug!(
            "burn: waiting {}ms ({}ms declared + {}ms settle) for flash write to complete",
            declared + settle,
            declared,
            settle
        );
        std::thread::sleep(Duration::from_millis(declared + settle));
        // Anything that arrived while the ECU was deaf is noise to us and, more
        // importantly, may be a partial frame to it.
        self.clear_rx_buffer();

        tracing::debug!("burn: flash write complete for page {}", page);
        // Successful burn clears the auto-burn-on-page-change tracker.
        if self.last_written_page == Some(params.page) {
            self.last_written_page = None;
        }
        self.dirty_pages.remove(&params.page);
        Ok(())
    }

    /// Flush any pending auto-burn (called by the UI when a settings dialog closes,
    /// per spec §6.2 `autoBurnOnCloseDialog`). Burns the most recently written page
    /// if `auto_burn_on_close_dialog` is enabled and a write is pending.
    pub fn flush_pending_burn(&mut self) -> Result<(), ProtocolError> {
        if !self.config.auto_burn_on_close_dialog {
            return Ok(());
        }
        if let Some(page) = self.last_written_page {
            tracing::info!("auto-burn: dialog close, burning page {}", page);
            self.burn(BurnParams { page, can_id: 0 })?;
        }
        Ok(())
    }

    /// Convenience method to burn all pages to flash
    /// Burn every page that has been written since it was last burned.
    ///
    /// This used to send a single burn for page 0, on the assumption that "most
    /// ECUs burn all RAM to flash with a single command". Speeduino does not,
    /// and says so in its own INI: fifteen `burnCommand` entries, one per page,
    /// each formatted `b%2i` with the page number. So the Burn button committed
    /// page 0 and nothing else - the VE table (page 2) and the ignition table
    /// (page 3) were never reachable by it at all.
    ///
    /// The failure was invisible in the worst way: the burn reported success,
    /// the tables read back correctly from RAM, and the values only disappeared
    /// on the next power cycle. On one car the ignition table reached flash
    /// solely because an unrelated auto-burn-on-page-change happened to catch
    /// it, while the VE table written seconds later did not.
    ///
    /// With nothing dirty this burns page 0 alone, preserving the old behaviour
    /// for a caller that just wants to poke the ECU.
    pub fn send_burn_command(&mut self) -> Result<(), ProtocolError> {
        let pages: Vec<u8> = if self.dirty_pages.is_empty() {
            vec![0]
        } else {
            self.dirty_pages.iter().copied().collect()
        };
        tracing::info!(
            ?pages,
            "burn: committing every page written since the last burn"
        );
        for page in pages {
            self.burn(BurnParams { can_id: 0, page })?;
        }
        Ok(())
    }

    /// Pages written since they were last burned. Empty means nothing is
    /// waiting in RAM that a power cycle would discard.
    pub fn dirty_pages(&self) -> Vec<u8> {
        self.dirty_pages.iter().copied().collect()
    }

    /// Write one Speeduino temperature calibration curve (CLT or IAT).
    ///
    /// `temps_c` are the sensor temperatures (°C) at the 32 ADC bins the
    /// firmware assigns — sample your sensor curve at
    /// [`calibration::temperature_calibration_bins`]`(self.is_modern_protocol())`.
    ///
    /// See the [`calibration`] module docs for the verified wire format.
    pub fn write_temperature_calibration(
        &mut self,
        table: CalibrationTable,
        temps_c: &[f64; calibration::TEMP_CALIBRATION_POINTS],
    ) -> Result<(), ProtocolError> {
        if table == CalibrationTable::O2 {
            return Err(ProtocolError::ProtocolError(
                "O2 table takes AFR data; use write_o2_calibration".to_string(),
            ));
        }
        let wire = calibration::encode_temperature_calibration(temps_c);
        self.write_calibration_wire(table, &wire)
    }

    /// Write the Speeduino O2/AFR sensor calibration curve.
    ///
    /// `afr` holds the AFR reading for each of the 1024 10-bit ADC counts
    /// (0 V .. 5 V). Values are stored as AFR × 10 in one byte, so the
    /// usable range is 0.0–25.5 AFR.
    pub fn write_o2_calibration(
        &mut self,
        afr: &[f64; calibration::O2_CALIBRATION_WIRE_BYTES],
    ) -> Result<(), ProtocolError> {
        let wire = calibration::encode_o2_calibration(afr);
        self.write_calibration_wire(CalibrationTable::O2, &wire)
    }

    /// Read the CRC32 the ECU stored for a calibration page (`k` command).
    ///
    /// Modern protocol only — the legacy command set has no calibration CRC
    /// (or any calibration read-back at all), so legacy writes are
    /// necessarily unverified, exactly as they are in TunerStudio.
    pub fn read_calibration_crc(&mut self, table: CalibrationTable) -> Result<u32, ProtocolError> {
        if !self.use_modern_protocol {
            return Err(ProtocolError::ProtocolError(
                "calibration CRC ('k') requires the CRC protocol; the legacy \
                 command set has no calibration read-back"
                    .to_string(),
            ));
        }
        let payload = vec![b'k', 0x00, table.id()];
        let response = self.send_packet(Packet::new(payload))?;
        let data = &response.payload;
        if data.len() < 5 || data[0] != 0 {
            return Err(ProtocolError::ProtocolError(format!(
                "calibration CRC read failed, response: {:02x?}",
                data
            )));
        }
        // Firmware sends reverse_bytes(crc) → big-endian on the wire.
        Ok(u32::from_be_bytes([data[1], data[2], data[3], data[4]]))
    }

    /// Send pre-encoded calibration bytes to the ECU over whichever protocol
    /// path is active, and verify where the protocol allows it.
    fn write_calibration_wire(
        &mut self,
        table: CalibrationTable,
        wire: &[u8],
    ) -> Result<(), ProtocolError> {
        // The `t` calibration command is Speeduino-specific (verified against
        // firmware tag 202501). Refuse on ECUs known to speak something else
        // rather than corrupt their command stream.
        match self.ecu_type {
            EcuType::Speeduino | EcuType::Unknown => {}
            other => {
                return Err(ProtocolError::ProtocolError(format!(
                    "sensor calibration write is only implemented for \
                     Speeduino (connected ECU type: {:?})",
                    other
                )));
            }
        }

        let expected_len = match table {
            CalibrationTable::O2 => calibration::O2_CALIBRATION_WIRE_BYTES,
            _ => calibration::TEMP_CALIBRATION_WIRE_BYTES,
        };
        if wire.len() != expected_len {
            return Err(ProtocolError::ProtocolError(format!(
                "calibration table {:?} takes {} bytes, got {}",
                table,
                expected_len,
                wire.len()
            )));
        }

        if self.use_modern_protocol {
            self.write_calibration_modern(table, wire)?;
            // The modern path can verify: compare the ECU's stored CRC with
            // the CRC of exactly the bytes we sent.
            let expected = calibration::calibration_crc32(wire);
            let stored = self.read_calibration_crc(table)?;
            if stored != expected {
                return Err(ProtocolError::ProtocolError(format!(
                    "calibration verify failed for {:?}: ECU stored CRC \
                     {:08x}, expected {:08x}",
                    table, stored, expected
                )));
            }
            Ok(())
        } else {
            self.write_calibration_legacy(table, wire)
        }
    }

    /// Legacy path: `'t'`, table id, raw data stream. No ACK exists, so the
    /// only failure modes visible here are serial-layer errors.
    fn write_calibration_legacy(
        &mut self,
        table: CalibrationTable,
        wire: &[u8],
    ) -> Result<(), ProtocolError> {
        tracing::info!(
            "write_calibration_legacy: table {:?} ({} bytes)",
            table,
            wire.len()
        );
        // The legacy path has no ACK and no read-back, so this is the one
        // calibration write whose success we cannot confirm. On firmware
        // newer than 202501 it is worse than unverified: legacy comms were
        // made read-only, and the firmware consumes the command and the whole
        // data stream while writing nothing at all, reporting no error. There
        // is nothing on the wire to distinguish that from success, so say so
        // rather than let the UI report a clean write.
        tracing::warn!(
            "calibration written over the legacy protocol: the ECU sends no \
             acknowledgement and offers no read-back, so this write is \
             unverified. Firmware newer than 202501 ignores legacy \
             calibration writes entirely. Connect with the CRC protocol to \
             get a verified write."
        );
        self.send_raw_command_no_response(&[b't', table.id()])?;

        // Pace the data out in small chunks. The firmware blocks inside
        // receiveCalibration() actively draining, but the temperature path
        // performs an EEPROM write per value pair *while receiving*, and the
        // Mega's RX ring is only 257 bytes — the same buffer whose overflow
        // corrupted VE tables via oversized `M` frames. Chunking + the
        // inter-write delay keeps the ring comfortably below capacity.
        let inter_chunk_ms = self.get_effective_min_wait().max(5);
        for chunk in wire.chunks(128) {
            self.send_raw_command_no_response(chunk)?;
            std::thread::sleep(Duration::from_millis(inter_chunk_ms));
        }

        // writeCalibration() burns the table to EEPROM with no completion
        // signal; give it time before the caller sends anything else.
        // (EEPROM update of a full table is a few hundred ms on a Mega2560.)
        let settle_ms = self
            .protocol_settings
            .as_ref()
            .map(|p| p.page_activation_delay as u64)
            .unwrap_or(0)
            .max(500);
        std::thread::sleep(Duration::from_millis(settle_ms));
        Ok(())
    }

    /// Modern path: one `'t'` envelope per chunk, each ACKed. Header fields
    /// are big-endian (unlike `'M'` — see the [`calibration`] module docs).
    fn write_calibration_modern(
        &mut self,
        table: CalibrationTable,
        wire: &[u8],
    ) -> Result<(), ProtocolError> {
        let chunk_size = match table {
            // EEPROM burn triggers when offset reaches 1023, so the O2 table
            // must arrive as 4 × 256.
            CalibrationTable::O2 => calibration::O2_CALIBRATION_CHUNK,
            // Any length other than 64 is rejected with RANGE_ERR.
            _ => calibration::TEMP_CALIBRATION_WIRE_BYTES,
        };

        for (i, chunk) in wire.chunks(chunk_size).enumerate() {
            let offset = i * chunk_size;
            let mut payload = Vec::with_capacity(7 + chunk.len());
            payload.push(b't');
            payload.push(0x00); // canId slot; ignored by the firmware
            payload.push(table.id());
            payload.extend_from_slice(&(offset as u16).to_be_bytes());
            payload.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
            payload.extend_from_slice(chunk);

            let response = self.send_packet(Packet::new(payload))?;
            let status = response.payload.first().copied().unwrap_or(0xFF);
            if status != 0 {
                let code = super::ResponseCode::from_byte(status);
                return Err(ProtocolError::ProtocolError(format!(
                    "calibration chunk at offset {} rejected: 0x{:02x} ({})",
                    offset,
                    status,
                    code.message()
                )));
            }
        }
        Ok(())
    }

    /// Send raw bytes to ECU (for controller commands)
    /// This is used by commandButton widgets to send arbitrary commands
    /// WARNING: These commands bypass normal memory synchronization
    ///
    /// On modern CRC-framed ECUs (msEnvelope_1.0), the INI command bytes are the
    /// packet payload only — TunerStudio wraps them in the protocol envelope.
    pub fn send_raw_bytes(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if bytes.is_empty() {
            return Ok(());
        }
        tracing::debug!(
            "send_raw_bytes: sending {} bytes: {:02x?} (modern={})",
            bytes.len(),
            bytes,
            self.use_modern_protocol
        );
        if self.use_modern_protocol {
            let packet = Packet::new(bytes.to_vec());
            self.send_packet_no_response(packet)
        } else {
            self.send_raw_command_no_response(bytes)
        }
    }

    /// Send raw bytes to ECU and read back the response.
    ///
    /// Sends command bytes, waits up to `timeout` for the response, using an
    /// inter-character timeout of 50ms to detect end of transmission.
    /// Returns the raw response bytes.
    /// Send a raw command and return the reply payload.
    ///
    /// Frames the command when the INI declares an envelope
    /// (`messageEnvelopeFormat`), exactly as [`Self::send_raw_bytes`] already
    /// does. The two used to disagree, and the consequences were not subtle.
    ///
    /// Speeduino locks legacy commands out for the rest of the session the
    /// first time a CRC-framed command parses (`BIT_STATUS4_ALLOW_LEGACY_COMMS`),
    /// so on a modern link an unframed `T` was not read as a command at all -
    /// its `0x54` became the high byte of a two-byte length, and the firmware
    /// then waited on a payload thousands of bytes long that was never coming.
    ///
    /// On a legacy link it reached `legacySerialCommand`, which answers a tooth
    /// log from `sendToothLog_legacy` - marked `/* Blocking */` in the firmware,
    /// and measured on the bench at 45 ms of stalled main loop for 508 bytes.
    /// Ignition schedules are set from that loop, so a capture repeating every
    /// 250 ms stops spark for 45 ms out of every 250, which is what a misfire
    /// on a running engine looks like. The framed path yields every four bytes
    /// (`SERIAL_TRANSMIT_TOOTH_INPROGRESS`) and does not stall it.
    ///
    /// Framing also fixes the read length. The envelope carries its own
    /// two-byte count, so the reply is read to a declared size instead of
    /// guessed at by the inter-character silence timer below - which ended a
    /// chunked transfer at the first pause and returned 137 bytes of 508.
    ///
    /// The INI's own `dataLength` is deliberately NOT used for this: the
    /// Speeduino 202501 file declares it as 508 for the tooth logger ("in
    /// bytes ... (not used)") and 127 for the composite logger ("Number of
    /// records"), so the same key carries different units on adjacent blocks.
    pub fn send_raw_bytes_with_response(
        &mut self,
        bytes: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, ProtocolError> {
        if self.use_modern_protocol {
            let reply = self.send_packet(Packet::new(bytes.to_vec()))?;
            // A framed reply leads with a return code; the legacy answer to the
            // same command does not. Strip it here so both paths hand callers
            // the same thing - otherwise every record is shifted one byte and
            // the whole log decodes as garbage. `expected_data_len` is unknown
            // for a raw command, so 0 selects the helper's leading-zero rule.
            let payload = strip_status_byte(&reply.payload, 0, "raw command");
            tracing::debug!(
                "send_raw_bytes_with_response: framed reply, {} payload bytes -> {} after status byte",
                reply.payload.len(),
                payload.len()
            );
            return Ok(payload);
        }

        let baud_rate = self.config.baud_rate;
        let min_wait = Some(self.get_effective_min_wait());

        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;

        // Clear buffers
        let _ = channel.clear_input_buffer();

        tracing::debug!(
            "send_raw_bytes_with_response: sending {} bytes: {:02x?}",
            bytes.len(),
            bytes
        );

        // Send command
        write_and_wait(channel, bytes, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        // Read response with inter-character timeout detection
        let mut response = Vec::new();
        let mut buffer = [0u8; 4096];
        let start = Instant::now();
        let mut last_data_time = Instant::now();
        let inter_char_timeout = Duration::from_millis(50);

        loop {
            if start.elapsed() > timeout {
                tracing::debug!(
                    "send_raw_bytes_with_response: overall timeout reached ({} bytes read)",
                    response.len()
                );
                break;
            }

            // If we have data and haven't received more in inter_char_timeout, we're done
            if !response.is_empty() && last_data_time.elapsed() > inter_char_timeout {
                tracing::debug!(
                    "send_raw_bytes_with_response: inter-char timeout, done ({} bytes)",
                    response.len()
                );
                break;
            }

            let available = match channel.bytes_to_read() {
                Ok(n) => n,
                Err(e) => {
                    tracing::debug!("send_raw_bytes_with_response: bytes_to_read error: {}", e);
                    if !response.is_empty() {
                        break;
                    }
                    return Err(ProtocolError::SerialError(e.to_string()));
                }
            };

            if available > 0 {
                let to_read = std::cmp::min(available as usize, buffer.len());
                match channel.read(&mut buffer[..to_read]) {
                    Ok(0) => break,
                    Ok(n) => {
                        response.extend_from_slice(&buffer[..n]);
                        last_data_time = Instant::now();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        if !response.is_empty() {
                            break;
                        }
                    }
                    Err(e) => return Err(ProtocolError::SerialError(e.to_string())),
                }
            } else {
                // No data available, brief sleep to avoid busy loop
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        self.rx_bytes = self.rx_bytes.saturating_add(response.len() as u64);
        if !response.is_empty() {
            self.rx_packets = self.rx_packets.saturating_add(1);
        }

        tracing::debug!(
            "send_raw_bytes_with_response: got {} bytes response",
            response.len()
        );

        Ok(response)
    }

    /// Send a text console command to the ECU (rusEFI/FOME/epicEFI only)
    ///
    /// For modern (CRC) protocol:
    ///   Uses the rusEFI two-step console protocol:
    ///   1. Send 'E' (TS_EXECUTE) + command text as a CRC-framed packet
    ///   2. Poll 'G' (TS_GET_TEXT) to retrieve buffered text output
    ///
    /// For legacy protocol:
    ///   Sends raw text + newline and reads back response until inter-char timeout
    ///
    /// Returns the response as a String (with trailing whitespace trimmed)
    pub fn send_console_command(
        &mut self,
        cmd: &super::commands::ConsoleCommand,
    ) -> Result<String, ProtocolError> {
        tracing::debug!(
            "send_console_command: sending '{}' (modern={})",
            cmd.command,
            self.use_modern_protocol
        );

        if self.use_modern_protocol {
            self.send_console_command_modern(cmd)
        } else {
            self.send_console_command_legacy(cmd)
        }
    }

    /// Send console command using the modern CRC binary protocol.
    ///
    /// rusEFI console protocol:
    /// - 'E' (0x45) = TS_EXECUTE: Send text command. ECU executes it and responds with bare TS_RESPONSE_OK.
    /// - 'G' (0x47) = TS_GET_TEXT: Poll buffered text output. ECU responds with status + text data.
    ///
    /// rusEFI text output format:
    ///   Each efiPrintf() call produces: `msg`message text`` (backtick-delimited, "msg" protocol tag).
    ///   Other protocols include `wave_chart`...``, table data, etc.
    ///   We drain stale output before executing, then parse only `msg` entries from the response.
    fn send_console_command_modern(
        &mut self,
        cmd: &super::commands::ConsoleCommand,
    ) -> Result<String, ProtocolError> {
        // Step 0: Drain any stale buffered text from the ECU.
        // The ECU accumulates ALL efiPrintf output (boot messages, periodic status, wave charts, etc.)
        // since the last 'G' poll. If we don't drain first, we'll get everything mixed in.
        tracing::debug!("send_console_command_modern: draining stale text buffer");
        self.drain_text_buffer();

        // Step 1: Send 'E' + command text as CRC-framed packet
        let mut payload = Vec::with_capacity(1 + cmd.command.len());
        payload.push(b'E'); // TS_EXECUTE command byte
        payload.extend_from_slice(cmd.command.as_bytes());

        let packet = Packet::new(payload);
        let response = self.send_packet(packet)?;

        // Verify the 'E' response - should be TS_RESPONSE_OK (status byte 0)
        if !response.payload.is_empty() && response.payload[0] != 0 {
            let status = response.payload[0];
            let code = super::ResponseCode::from_byte(status);
            tracing::warn!(
                "send_console_command_modern: 'E' command returned status 0x{:02x} ({})",
                status,
                code.message()
            );
            if code == super::ResponseCode::UnrecognizedCommand {
                return Err(ProtocolError::ProtocolError(
                    "ECU does not support console commands (unrecognized 'E' command)".to_string(),
                ));
            }
            if code.is_error() {
                let message = if code.carries_payload_message() && response.payload.len() > 1 {
                    String::from_utf8_lossy(&response.payload[1..]).into_owned()
                } else {
                    code.message().to_string()
                };
                return Err(ProtocolError::EcuStatusError {
                    code: status,
                    message,
                });
            }
        }

        tracing::debug!("send_console_command_modern: 'E' command accepted, polling text output");

        // Step 2: Poll 'G' (TS_GET_TEXT) to retrieve the command output
        // The ECU buffers console output; we may need to poll multiple times.
        let mut collected_text = String::new();
        let poll_timeout = Duration::from_millis(cmd.get_timeout_ms());
        let poll_start = Instant::now();
        let max_polls = 10;
        let mut empty_polls = 0;

        // Give the ECU a moment to execute the command and buffer output
        std::thread::sleep(Duration::from_millis(50));

        for poll_idx in 0..max_polls {
            if poll_start.elapsed() > poll_timeout {
                tracing::debug!("send_console_command_modern: poll timeout reached");
                break;
            }

            // Send 'G' command
            let get_text_packet = Packet::new(vec![b'G']); // TS_GET_TEXT
            match self.send_packet(get_text_packet) {
                Ok(text_response) => {
                    // Response payload: [status_byte][text_data...]
                    let text_data = if text_response.payload.len() > 1
                        && text_response.payload[0] == 0
                    {
                        &text_response.payload[1..]
                    } else if text_response.payload.is_empty() || text_response.payload[0] == 0 {
                        // Empty response or just status byte
                        &[]
                    } else {
                        // No status byte prefix (some firmware variations)
                        &text_response.payload[..]
                    };

                    if text_data.is_empty() {
                        empty_polls += 1;
                        tracing::debug!(
                            "send_console_command_modern: poll {} empty (empty_count={})",
                            poll_idx,
                            empty_polls
                        );
                        // If we already have text and get an empty poll, output is complete
                        if !collected_text.is_empty() || empty_polls >= 2 {
                            break;
                        }
                        // Wait a bit more for output to accumulate
                        std::thread::sleep(Duration::from_millis(50));
                    } else {
                        let text_chunk = String::from_utf8_lossy(text_data);
                        tracing::debug!(
                            "send_console_command_modern: poll {} got {} bytes",
                            poll_idx,
                            text_data.len(),
                        );
                        collected_text.push_str(&text_chunk);
                        empty_polls = 0;
                        // Brief pause before next poll to let more output accumulate
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "send_console_command_modern: 'G' poll {} failed: {:?}",
                        poll_idx,
                        e
                    );
                    // If we already have some text, return what we have
                    if !collected_text.is_empty() {
                        break;
                    }
                    // Otherwise, the ECU might not support 'G'
                    return Err(e);
                }
            }
        }

        // Step 3: Parse the rusEFI text output format into readable lines.
        // Raw format: msg`message text`msg`another message`wave_chart`data`...
        // We extract msg entries and format them as newline-separated text.
        let result = Self::parse_rusefi_text_output(&collected_text);

        tracing::debug!(
            "send_console_command_modern: parsed {} bytes from {} raw bytes",
            result.len(),
            collected_text.len(),
        );

        // Record metrics
        self.tx_packets = self.tx_packets.saturating_add(2); // E + G packets
        self.rx_packets = self.rx_packets.saturating_add(2);

        if result.is_empty() {
            // Command was accepted but produced no output
            Ok("(command accepted, no output)".to_string())
        } else {
            Ok(result)
        }
    }

    /// Drain the ECU's text output buffer by sending 'G' until empty.
    /// This discards stale boot messages, periodic status, wave charts, etc.
    fn drain_text_buffer(&mut self) {
        for drain_idx in 0..3 {
            let get_text_packet = Packet::new(vec![b'G']);
            match self.send_packet(get_text_packet) {
                Ok(response) => {
                    let data_len = if response.payload.len() > 1 && response.payload[0] == 0 {
                        response.payload.len() - 1
                    } else {
                        0
                    };
                    tracing::debug!(
                        "drain_text_buffer: poll {} drained {} bytes",
                        drain_idx,
                        data_len
                    );
                    if data_len == 0 {
                        break; // Buffer is empty
                    }
                    // Brief sleep to let ECU swap buffers
                    std::thread::sleep(Duration::from_millis(30));
                }
                Err(e) => {
                    tracing::warn!("drain_text_buffer: poll {} failed: {:?}", drain_idx, e);
                    break;
                }
            }
        }
    }

    /// Parse rusEFI text output format into human-readable lines.
    ///
    /// rusEFI uses backtick (`) as LOG_DELIMITER and protocol tags like "msg", "wave_chart", etc.
    /// Format: `protocol_tag`message content`protocol_tag`message content`...`
    ///
    /// We extract only `msg` entries (the standard efiPrintf output) and format
    /// them as newline-separated text. Other protocol tags (wave_chart, table data,
    /// outpin, etc.) are filtered out to keep console output clean.
    fn parse_rusefi_text_output(raw: &str) -> String {
        if raw.is_empty() {
            return String::new();
        }

        // Known rusEFI protocol tags that we want to display as console messages
        const MSG_TAGS: &[&str] = &["msg", "emu"];

        // Known tags we want to silently filter out
        const FILTER_TAGS: &[&str] = &[
            "wave_chart",
            "outpin",
            "t|d_",
            "map|u",
            "maf|u",
            "maf|d",
            "hpfp|d",
            "hpfp|u",
            "hpfp2|d",
            "pfp|u",
            "wave",
            "VVT|",
        ];

        let mut lines = Vec::new();

        // Split on backtick delimiter — the rusEFI LOG_DELIMITER
        let parts: Vec<&str> = raw.split('`').collect();

        // The format alternates: [tag][content][tag][content]...
        // parts[0] = protocol tag (e.g., "msg")
        // parts[1] = message content
        // parts[2] = next protocol tag
        // parts[3] = next message content
        // etc.
        let mut i = 0;
        while i + 1 < parts.len() {
            let tag = parts[i].trim();
            let content = parts[i + 1];
            i += 2;

            if tag.is_empty() && content.is_empty() {
                continue;
            }

            // Check if this is a message tag we should display
            let is_msg_tag = MSG_TAGS.iter().any(|t| tag.eq_ignore_ascii_case(t));

            if is_msg_tag {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
                continue;
            }

            // Check if this is a known filterable tag
            let is_filter_tag = FILTER_TAGS.iter().any(|t| tag.starts_with(t));

            if is_filter_tag {
                // Silently skip
                continue;
            }

            // Unknown tag — if it has non-trivial content, show it as-is
            // (some rusEFI commands output with custom tags)
            if !tag.is_empty() && !content.trim().is_empty() {
                let trimmed = content.trim();
                // Only show if it looks like readable text (not binary table data)
                if trimmed.len() > 2 && trimmed.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                    lines.push(format!("[{}] {}", tag, trimmed));
                }
            }
        }

        // If nothing was parsed (maybe the output doesn't use backtick format),
        // return the raw text with some basic cleanup
        if lines.is_empty() && !raw.trim().is_empty() {
            return raw.trim().to_string();
        }

        lines.join("\n")
    }

    /// Send console command using legacy raw text protocol (for ECUs without CRC framing)
    fn send_console_command_legacy(
        &mut self,
        cmd: &super::commands::ConsoleCommand,
    ) -> Result<String, ProtocolError> {
        let baud_rate = self.config.baud_rate;
        let min_wait = Some(self.get_effective_min_wait());
        let timeout = Duration::from_millis(cmd.get_timeout_ms());
        let inter_char_timeout = self.get_effective_inter_char_timeout();

        let channel = self.channel.as_mut().ok_or(ProtocolError::NotConnected)?;

        // Clear buffers before sending
        let _ = channel.clear_input_buffer();
        let _ = channel.clear_output_buffer();

        // Convert command to bytes (adds newline) and send
        let cmd_bytes = cmd.to_bytes();
        write_and_wait(channel, &cmd_bytes, baud_rate, min_wait)
            .map_err(|e| ProtocolError::SerialError(e.to_string()))?;

        tracing::debug!("send_console_command_legacy: command sent, waiting for response");

        // Read response with timeout
        let mut response = Vec::new();
        let mut buffer = [0u8; 512];
        let start = Instant::now();
        let mut last_data_time = Instant::now();

        loop {
            if start.elapsed() > timeout {
                tracing::debug!("send_console_command_legacy: overall timeout reached");
                break;
            }

            let available = match channel.bytes_to_read() {
                Ok(n) => n,
                Err(e) => {
                    tracing::debug!("send_console_command_legacy: bytes_to_read error: {}", e);
                    return Err(ProtocolError::SerialError(e.to_string()));
                }
            };

            if available > 0 {
                let to_read = std::cmp::min(available as usize, buffer.len());
                match channel.read(&mut buffer[..to_read]) {
                    Ok(0) => break,
                    Ok(n) => {
                        response.extend_from_slice(&buffer[..n]);
                        last_data_time = Instant::now();
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        // Non-blocking, continue
                    }
                    Err(e) => {
                        return Err(ProtocolError::SerialError(e.to_string()));
                    }
                }
            } else if response.is_empty() {
                std::thread::sleep(Duration::from_millis(1));
            } else if last_data_time.elapsed() > inter_char_timeout {
                break;
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        let response_str = String::from_utf8_lossy(&response).trim().to_string();

        tracing::debug!(
            "send_console_command_legacy: received {} bytes: '{}'",
            response.len(),
            response_str
        );

        if response_str.is_empty() {
            return Err(ProtocolError::Timeout);
        }

        // Record metrics
        self.tx_bytes = self.tx_bytes.saturating_add(cmd_bytes.len() as u64);
        self.tx_packets = self.tx_packets.saturating_add(1);
        self.rx_bytes = self.rx_bytes.saturating_add(response.len() as u64);
        self.rx_packets = self.rx_packets.saturating_add(1);

        Ok(response_str)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {

    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    /// The early-return condition is the whole safety surface of the
    /// expected-length read: returning too soon yields a truncated message,
    /// and never returning early gives back the 100 ms per poll this exists to
    /// avoid. The loop needs a live serial channel, so the decision is tested
    /// directly instead.
    mod expected_length_reads {
        use super::super::Connection;

        #[test]
        fn waits_for_the_line_to_quiet_when_no_length_is_declared() {
            // Handshakes and any command whose reply length is not declared
            // must behave exactly as before: never stop early.
            assert!(!Connection::response_is_complete_impl(0, None));
            assert!(!Connection::response_is_complete_impl(130, None));
            assert!(!Connection::response_is_complete_impl(usize::MAX, None));
        }

        #[test]
        fn a_declared_length_of_zero_means_unknown_not_empty() {
            // An INI that omits ochBlockSize parses it as 0. Treating that as
            // "expect nothing" would return an empty response on the first
            // poll, breaking realtime data entirely.
            assert!(!Connection::response_is_complete_impl(0, Some(0)));
            assert!(!Connection::response_is_complete_impl(50, Some(0)));
        }

        #[test]
        fn returns_once_the_declared_length_has_arrived() {
            // ochBlockSize = 130 on a real Speeduino INI.
            assert!(!Connection::response_is_complete_impl(0, Some(130)));
            assert!(!Connection::response_is_complete_impl(129, Some(130)));
            assert!(Connection::response_is_complete_impl(130, Some(130)));
        }

        #[test]
        fn a_longer_than_declared_response_still_terminates() {
            // If the ECU sends more than the INI promises, stop at the declared
            // length rather than reading on: the surplus is discarded by the
            // clear_input_buffer at the start of the next command, so it cannot
            // desync the following exchange.
            assert!(Connection::response_is_complete_impl(131, Some(130)));
            assert!(Connection::response_is_complete_impl(500, Some(130)));
        }

        #[test]
        fn a_short_response_falls_back_to_the_inter_character_timeout() {
            // A truncated or interrupted reply never satisfies the length, so
            // the loop keeps its original exit path and the caller still sees
            // whatever arrived rather than hanging.
            assert!(!Connection::response_is_complete_impl(1, Some(130)));
            assert!(!Connection::response_is_complete_impl(64, Some(130)));
        }
    }

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
    fn write_response_status_ok_on_empty_payload() {
        // Some ECU/INI combos don't echo a status byte on write acks; the CRC
        // already confirmed the frame arrived intact, so treat this as success
        // rather than an error, matching strip_status_byte's read-side leniency.
        let response = Packet::new(vec![]);
        assert!(check_write_response_status(&response).is_ok());
    }

    #[test]
    fn write_response_status_ok_on_success_code() {
        let response = Packet::new(vec![0x00]); // ResponseCode::Ok
        assert!(check_write_response_status(&response).is_ok());

        let response = Packet::new(vec![0x00, 0xAB, 0xCD]); // Ok plus trailing echoed data
        assert!(check_write_response_status(&response).is_ok());
    }

    #[test]
    fn write_response_status_rejects_ecu_reported_error() {
        // 0x84 = OutOfRange. Before this fix, write_memory discarded the
        // response entirely and reported this write as Ok(()) regardless.
        let response = Packet::new(vec![0x84]);
        let err = check_write_response_status(&response).unwrap_err();
        match err {
            ProtocolError::EcuStatusError { code, message } => {
                assert_eq!(code, 0x84);
                assert_eq!(message, super::super::ResponseCode::OutOfRange.message());
            }
            other => panic!("expected EcuStatusError, got {other:?}"),
        }
    }

    #[test]
    fn write_response_status_extracts_payload_message_for_generic_error() {
        // 0x94 = GenericError, which per spec carries a user-readable message
        // in the remaining payload bytes rather than using the default text.
        let mut payload = vec![0x94];
        payload.extend_from_slice(b"flash write failed");
        let response = Packet::new(payload);
        let err = check_write_response_status(&response).unwrap_err();
        match err {
            ProtocolError::EcuStatusError { code, message } => {
                assert_eq!(code, 0x94);
                assert_eq!(message, "flash write failed");
            }
            other => panic!("expected EcuStatusError, got {other:?}"),
        }
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

    /// `write_page` and `write_memory` both chunk, so they must use the same
    /// budget. Speeduino declares `blockingFactor = 251` and pages up to 384
    /// bytes: splitting at the full 251 handed `write_memory` a chunk 8 bytes
    /// over its own limit, which re-split it into 243 + an 8-byte runt frame.
    #[test]
    fn page_and_memory_writes_chunk_to_the_same_size() {
        let core = Arc::new(Mutex::new(MegaCore::new()));
        let mut conn = speeduino_conn(&core);
        assert_eq!(
            conn.effective_write_chunk(),
            243,
            "251 less the 7-byte command header, plus one byte of margin"
        );

        // A 288-byte page must reach the wire as exactly two frames — 243+7
        // and 45+7 header bytes — with no runt. Asserting on the simulated
        // ECU's actual bursts is what catches `write_page` splitting at the
        // raw blocking factor again: that put a 258-byte frame on the wire
        // (over the 257-byte ring) followed by an 8-byte runt.
        let data: Vec<u8> = (0..288u32).map(|i| i as u8).collect();
        conn.write_page(3, &data).expect("write_page");

        let c = core.lock().unwrap();
        assert_eq!(c.bursts, vec![250, 52], "wire frames (data + 7B header)");
        assert_eq!(c.dropped, 0, "no frame may overrun the Mega RX ring");
        assert_eq!(&c.page(3)[..288], &data[..], "page must arrive intact");
    }

    // ---------------------------------------------------------------------
    // Mega2560 RX-limit regression (bench simulator acceptance behavior 5)
    // ---------------------------------------------------------------------

    /// Speeduino's own INI: "Serial buffer is 257 bytes and there are 6 bytes
    /// of overhead ... payload is therefore 257-6=251".
    const MEGA_RX_CAPACITY: usize = 257;
    const SPEEDUINO_BLOCKING_FACTOR: u32 = 251;

    /// What the `M` handler is in the middle of when the next byte arrives.
    enum MegaState {
        Idle,
        /// Bytes gathered after the `M`: page, offset, count, big-endian pairs.
        Header(Vec<u8>),
        /// The same six fields after an `r`, which answers with page bytes.
        ReadHeader(Vec<u8>),
        Data {
            page: u16,
            at: usize,
            left: usize,
        },
    }

    /// The Mega2560's serial front end as the bench simulator models it
    /// (`LibreTune-test/sim-ecu`, `MEGA_RX_CAPACITY 257`, acceptance behavior 5).
    ///
    /// Two properties matter and neither is obvious from the protocol spec.
    /// The RX ring holds 257 bytes and drops the rest of an over-long burst
    /// without complaint, and the `M` handler is a state machine that keeps
    /// taking whatever bytes arrive next as table data when a frame promised
    /// more than turned up. One oversized write therefore corrupts the page it
    /// was aimed at *and* eats the command behind it, with nothing on the wire
    /// to say so.
    struct MegaCore {
        pages: HashMap<u16, Vec<u8>>,
        state: MegaState,
        /// Length of each burst the host handed to the port.
        bursts: Vec<usize>,
        /// Bytes lost to the ring, summed over all bursts.
        dropped: usize,
        /// Idle time on the link before each burst — how long this ECU was
        /// given to drain the previous frame.
        gaps: Vec<Duration>,
        last_burst: Option<Instant>,
        /// Bytes waiting to go back to the host.
        out: VecDeque<u8>,
        /// Store the complement of whatever is written to this (page, offset),
        /// standing in for any reason the ECU might not hold what was sent.
        corrupt_at: Option<(u16, usize)>,
    }

    impl MegaCore {
        fn new() -> Self {
            Self {
                pages: HashMap::new(),
                state: MegaState::Idle,
                bursts: Vec::new(),
                dropped: 0,
                gaps: Vec::new(),
                last_burst: None,
                out: VecDeque::new(),
                corrupt_at: None,
            }
        }

        fn page(&self, page: u16) -> &[u8] {
            self.pages.get(&page).map(|p| p.as_slice()).unwrap_or(&[])
        }

        /// One burst from the host: fill the ring, drop the overflow, then let
        /// the command handler drain what survived.
        fn burst(&mut self, bytes: &[u8]) {
            let now = Instant::now();
            self.gaps
                .push(self.last_burst.map_or(Duration::ZERO, |t| now - t));
            self.last_burst = Some(now);
            self.bursts.push(bytes.len());
            let kept = bytes.len().min(MEGA_RX_CAPACITY);
            self.dropped += bytes.len() - kept;
            for &b in &bytes[..kept] {
                self.step(b);
            }
            // Legacy writes are fire-and-forget, but the line is never truly
            // silent; the host reads *something* back and calls the write good.
            if self.out.is_empty() {
                self.out.push_back(0x00);
            }
        }

        fn step(&mut self, b: u8) {
            self.state = match std::mem::replace(&mut self.state, MegaState::Idle) {
                MegaState::Idle if b == b'M' => MegaState::Header(Vec::with_capacity(6)),
                MegaState::Idle if b == b'r' => MegaState::ReadHeader(Vec::with_capacity(6)),
                MegaState::Idle => MegaState::Idle,
                MegaState::Header(mut h) => {
                    h.push(b);
                    if h.len() < 6 {
                        MegaState::Header(h)
                    } else {
                        MegaState::Data {
                            page: u16::from_be_bytes([h[0], h[1]]),
                            at: u16::from_be_bytes([h[2], h[3]]) as usize,
                            left: u16::from_be_bytes([h[4], h[5]]) as usize,
                        }
                    }
                }
                MegaState::ReadHeader(mut h) => {
                    h.push(b);
                    if h.len() < 6 {
                        MegaState::ReadHeader(h)
                    } else {
                        let page = u16::from_be_bytes([h[0], h[1]]);
                        let at = u16::from_be_bytes([h[2], h[3]]) as usize;
                        let count = u16::from_be_bytes([h[4], h[5]]) as usize;
                        let img = self.pages.entry(page).or_insert_with(|| vec![0u8; 1024]);
                        let end = (at + count).min(img.len());
                        let slice = img[at.min(end)..end].to_vec();
                        self.out.extend(slice);
                        MegaState::Idle
                    }
                }
                MegaState::Data { page, at, left } => {
                    let flip = self.corrupt_at == Some((page, at));
                    let img = self.pages.entry(page).or_insert_with(|| vec![0u8; 1024]);
                    if at < img.len() {
                        img[at] = if flip { !b } else { b };
                    }
                    if left > 1 {
                        MegaState::Data {
                            page,
                            at: at + 1,
                            left: left - 1,
                        }
                    } else {
                        MegaState::Idle
                    }
                }
            };
        }
    }

    struct MegaChannel(Arc<Mutex<MegaCore>>);

    impl Write for MegaChannel {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().burst(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Read for MegaChannel {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut core = self.0.lock().unwrap();
            let n = core.out.len().min(buf.len());
            for slot in buf.iter_mut().take(n) {
                *slot = core.out.pop_front().unwrap();
            }
            Ok(n)
        }
    }

    impl CommunicationChannel for MegaChannel {
        fn set_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
            Ok(())
        }
        fn clear_input_buffer(&mut self) -> std::io::Result<()> {
            // A real port drops whatever is still sitting in RX, and the
            // command path relies on that: a fire-and-forget write leaves its
            // unread ack behind, which would otherwise arrive as the first
            // byte of the next read's payload.
            self.0.lock().unwrap().out.clear();
            Ok(())
        }
        fn clear_output_buffer(&mut self) -> std::io::Result<()> {
            Ok(())
        }
        fn try_clone(&self) -> std::io::Result<Box<dyn CommunicationChannel>> {
            Ok(Box::new(MegaChannel(Arc::clone(&self.0))))
        }
        fn bytes_to_read(&mut self) -> std::io::Result<u32> {
            Ok(self.0.lock().unwrap().out.len() as u32)
        }
    }

    /// One `M` frame carrying a whole 16x16 table, exactly as the pre-fix code
    /// built it: 7 header bytes plus 256 data bytes.
    fn oversized_table_frame(page: u16, table: &[u8]) -> Vec<u8> {
        let mut f = vec![b'M'];
        f.extend_from_slice(&page.to_be_bytes());
        f.extend_from_slice(&0u16.to_be_bytes());
        f.extend_from_slice(&(table.len() as u16).to_be_bytes());
        f.extend_from_slice(table);
        f
    }

    fn speeduino_conn(core: &Arc<Mutex<MegaCore>>) -> Connection {
        let mut conn = Connection::new(ConnectionConfig::default());
        conn.channel = Some(Box::new(MegaChannel(Arc::clone(core))));
        conn.config.auto_burn_on_page_change = false;

        let mut proto = ProtocolSettings::default();
        proto.blocking_factor = SPEEDUINO_BLOCKING_FACTOR;
        proto.inter_write_delay = 0;
        proto.block_read_timeout = 100;
        proto.page_chunk_write_commands = vec!["M%2i%2o%2c%v".to_string(); 8];
        proto.page_read_commands = vec!["r%2i%2o%2c".to_string(); 8];
        // Speeduino's legacy framing puts these fields on the wire big-endian;
        // the observed corruption bytes (4D 00 02 ...) only decode that way.
        conn.set_protocol(proto, Endianness::Big);
        conn.use_modern_protocol = false;
        conn
    }

    /// Characterisation test of the HARNESS, not of production code: it drives
    /// the simulated Mega directly with the frame the pre-fix code used to
    /// send, bypassing `Connection` entirely. It exists so the regression
    /// tests below mean something — a harness that cannot see the bug proves
    /// nothing. It cannot fail from reverting the fix; the tests that can are
    /// `table_writes_survive_the_mega_rx_limit` and its siblings.
    /// Expected signature is what the bench recorded on 18 Aug 2026: six
    /// bytes dropped, the next command's header written into the ignition
    /// table's top row, and the write behind it lost entirely.
    #[test]
    fn mega_rx_limit_eats_the_frame_tail_and_the_next_command() {
        let core = Arc::new(Mutex::new(MegaCore::new()));

        let ignition: Vec<u8> = (0..256u32).map(|i| 40 + (i % 30) as u8).collect();
        let ve: Vec<u8> = (0..256u32).map(|i| 60 + (i % 40) as u8).collect();

        {
            let mut c = core.lock().unwrap();
            c.burst(&oversized_table_frame(1, &ignition)); // 263 bytes
            c.burst(&oversized_table_frame(2, &ve)); // 263 bytes
        }

        let c = core.lock().unwrap();
        assert_eq!(c.bursts, vec![263, 263]);
        assert_eq!(c.dropped, 12, "6 bytes lost off the tail of each frame");

        // The last six cells of the ignition table hold the next command's
        // header instead of spark advance: 'M', page 2 big-endian, offset 0,
        // and the high byte of the 256-byte count. Raw 0x4D, 0x00 and 0x02 are
        // 37, -40 and -38 degrees once the table's +40 offset comes off — a
        // detonation-relevant edit nobody asked for, in the 100 kPa row.
        assert_eq!(&c.page(1)[250..256], &[0x4D, 0x00, 0x02, 0x00, 0x00, 0x01]);
        assert_ne!(&c.page(1)[250..256], &ignition[250..256]);

        // ...and the VE write never happened at all.
        assert!(
            c.page(2).is_empty() || c.page(2)[..256].iter().all(|&b| b == 0),
            "the following write should have been consumed as table data"
        );
    }

    /// The fix: no matter how large a payload a caller hands `write_memory`,
    /// nothing longer than the Mega can hold reaches the wire, so both tables
    /// land byte-exact. Reintroducing the bug (dropping the chunking branch in
    /// `write_memory`) turns this into the corruption pinned above.
    #[test]
    fn table_writes_survive_the_mega_rx_limit() {
        let core = Arc::new(Mutex::new(MegaCore::new()));
        let mut conn = speeduino_conn(&core);

        let ignition: Vec<u8> = (0..256u32).map(|i| 40 + (i % 30) as u8).collect();
        let ve: Vec<u8> = (0..256u32).map(|i| 60 + (i % 40) as u8).collect();

        for (page, table) in [(1u8, &ignition), (2u8, &ve)] {
            conn.write_memory(WriteMemoryParams {
                can_id: 0,
                page,
                offset: 0,
                data: table.clone(),
            })
            .expect("write_memory should succeed");
        }

        let c = core.lock().unwrap();
        assert!(
            c.bursts.iter().all(|&n| n <= MEGA_RX_CAPACITY),
            "a frame overran the Mega's {MEGA_RX_CAPACITY}-byte buffer: {:?}",
            c.bursts
        );
        assert_eq!(c.dropped, 0, "no byte may be dropped by the ECU ring");
        assert_eq!(&c.page(1)[..256], &ignition[..], "ignition table corrupted");
        assert_eq!(&c.page(2)[..256], &ve[..], "VE table lost or corrupted");

        // Correctly sized frames are not enough on their own: the bench sweep
        // showed a following command sent 10.5 ms after a write frame gets
        // eaten as table data. Every frame here must be followed by at least
        // the ECU's measured service window.
        let settle = Duration::from_millis(LEGACY_WRITE_SETTLE_MS);
        for (i, gap) in c.gaps.iter().enumerate().skip(1) {
            let prev = c.bursts[i - 1];
            assert!(
                *gap >= settle,
                "frame {i} followed a {prev}-byte frame after only {gap:?}, \
                 inside the ECU's {settle:?} service window"
            );
        }
    }

    /// The pacing rule. The bench sweep put the ECU's service window at 20 ms
    /// (10 ms corrupts, 20 ms is the first clean value), so a small trailing
    /// frame must not be allowed to scale the delay down — that 20-byte second
    /// chunk followed by a read 10.5 ms later is exactly what corrupted the
    /// ignition table with correctly sized frames.
    #[test]
    fn legacy_write_frames_are_paced_for_the_ecus_service_window() {
        let core = Arc::new(Mutex::new(MegaCore::new()));
        let mut conn = speeduino_conn(&core);

        assert_eq!(conn.config.baud_rate, 115_200, "test assumes a 115200 link");
        let settle = Duration::from_millis(LEGACY_WRITE_SETTLE_MS);
        assert!(
            settle >= Duration::from_millis(20),
            "below the measured floor"
        );
        assert!(conn.inter_frame_delay(250) >= settle);
        assert!(
            conn.inter_frame_delay(20) >= settle,
            "a short trailing frame still needs the full service window"
        );

        // A slow link needs proportionally more: wire time takes over.
        conn.config.baud_rate = 9_600;
        assert!(conn.inter_frame_delay(250) >= Duration::from_millis(260));

        // The INI's own inter-write delay still wins when it is the larger.
        conn.config.baud_rate = 115_200;
        let mut proto = ProtocolSettings::default();
        proto.blocking_factor = SPEEDUINO_BLOCKING_FACTOR;
        proto.inter_write_delay = 100;
        conn.set_protocol(proto, Endianness::Big);
        conn.use_modern_protocol = false;
        // On unix the wire time is already slept out by write_and_wait, so the
        // delay is exactly the INI value; elsewhere the frame's ~1 ms wire
        // time is added on top. Either way the INI value must dominate an
        // 8-byte frame's wire time — and not be doubled or dropped.
        let d = conn.inter_frame_delay(8);
        assert!(
            d >= Duration::from_millis(100) && d <= Duration::from_millis(105),
            "INI inter-write delay must win for a short frame, got {d:?}"
        );

        // A CRC ECU acknowledges the write, so it needs no blind wait.
        conn.use_modern_protocol = true;
        let mut proto = ProtocolSettings::default();
        proto.blocking_factor = SPEEDUINO_BLOCKING_FACTOR;
        conn.set_protocol(proto, Endianness::Big);
        conn.use_modern_protocol = true;
        assert!(conn.inter_frame_delay(20) < settle);
    }

    /// Chunking closes the overrun we know about. The read-back is what makes
    /// the next silent divergence visible, so it has to actually catch one:
    /// a single byte the ECU did not store as sent must fail the write rather
    /// than return Ok, and it must name the cell.
    #[test]
    fn verified_write_catches_a_byte_the_ecu_did_not_store() {
        let core = Arc::new(Mutex::new(MegaCore::new()));
        core.lock().unwrap().corrupt_at = Some((1, 100));
        let mut conn = speeduino_conn(&core);

        let table: Vec<u8> = (0..256u32).map(|i| 40 + (i % 30) as u8).collect();
        let err = conn
            .write_memory_verified(WriteMemoryParams {
                can_id: 0,
                page: 1,
                offset: 0,
                data: table.clone(),
            })
            .expect_err("a byte the ECU did not store must fail the write");

        match err {
            ProtocolError::WriteVerificationFailed {
                page,
                offset,
                expected,
                actual,
            } => {
                assert_eq!((page, offset), (1, 100));
                assert_eq!(expected, table[100]);
                assert_eq!(actual, !table[100]);
            }
            other => panic!("expected a verification failure, got {other:?}"),
        }
    }

    /// The same write, with nothing wrong at the ECU, must pass silently —
    /// otherwise the check would be unusable in the table editor.
    #[test]
    fn verified_write_passes_when_the_ecu_agrees() {
        let core = Arc::new(Mutex::new(MegaCore::new()));
        let mut conn = speeduino_conn(&core);

        let table: Vec<u8> = (0..256u32).map(|i| 40 + (i % 30) as u8).collect();
        conn.write_memory_verified(WriteMemoryParams {
            can_id: 0,
            page: 1,
            offset: 0,
            data: table.clone(),
        })
        .expect("an honest ECU must verify clean");

        assert_eq!(&core.lock().unwrap().page(1)[..256], &table[..]);
    }

    #[test]
    fn test_choose_runtime_command_rfcomm() {
        // rusEFI keeps the slow-link heuristic (Issue #71: Speeduino/Unknown now
        // stay on Burst regardless of link speed).
        let mut cfg = ConnectionConfig::default();
        cfg.port_name = "rfcomm0".to_string();
        let mut conn = Connection::new(cfg);
        conn.set_ecu_type(EcuType::RusEFI);
        let mut proto = ProtocolSettings::default();
        proto.och_get_command = Some("O".to_string());
        proto.burst_get_command = Some("A".to_string());
        conn.set_protocol(proto, Endianness::Little);
        let (choice, reason) = conn.choose_runtime_command();
        match choice {
            RuntimeFetch::OCH(cmd) => assert_eq!(cmd, "O"),
            _ => panic!("Expected OCH choice, got {:?}", choice),
        }
        assert!(
            reason.contains("heuristic")
                || reason.contains("ini hint")
                || reason.contains("slow")
                || reason.contains("adaptive")
        );
    }

    /// Issue #71: Speeduino (and MS2/MS3/Unknown) must stay on Burst in Auto mode
    /// even when the INI declares maxUnusedRuntimeRange, a slow link, or slow
    /// adaptive-timing averages — these heuristics previously collapsed
    /// throughput to ~13 B/sec on real Speeduino 202501 hardware.
    #[test]
    fn test_speeduino_auto_stays_on_burst() {
        for ecu in [
            EcuType::Speeduino,
            EcuType::MS2,
            EcuType::MS3,
            EcuType::Unknown,
        ] {
            // Slow link (rfcomm) + INI hint + slow adaptive timing all set.
            let mut cfg = ConnectionConfig::default();
            cfg.port_name = "rfcomm0".to_string();
            let mut conn = Connection::new(cfg);
            conn.set_ecu_type(ecu);
            let mut proto = ProtocolSettings::default();
            proto.och_get_command = Some("O".to_string());
            proto.burst_get_command = Some("A".to_string());
            proto.max_unused_runtime_range = 999; // would normally trigger OCH
            conn.set_protocol(proto, Endianness::Big);
            // Slow adaptive timing average that would normally trigger OCH.
            conn.enable_adaptive_timing(None);
            conn.record_response_time(std::time::Duration::from_millis(200));
            conn.record_response_time(std::time::Duration::from_millis(180));

            let (choice, reason) = conn.choose_runtime_command();
            match &choice {
                RuntimeFetch::Burst(cmd) => assert_eq!(cmd, "A"),
                _ => panic!("{:?} should use Burst in Auto, got {:?}", ecu, choice),
            }
            assert!(
                reason.contains("Burst"),
                "unexpected reason for {:?}: {}",
                ecu,
                reason
            );
        }
    }

    /// Issue #71: ForceOCH must still override the Speeduino Burst default so the
    /// user can manually select OCH when appropriate.
    #[test]
    fn test_speeduino_force_och_override() {
        let mut cfg = ConnectionConfig::default();
        cfg.runtime_packet_mode = RuntimePacketMode::ForceOCH;
        let mut conn = Connection::new(cfg);
        conn.set_ecu_type(EcuType::Speeduino);
        let mut proto = ProtocolSettings::default();
        proto.och_get_command = Some("O".to_string());
        proto.burst_get_command = Some("A".to_string());
        conn.set_protocol(proto, Endianness::Big);
        let (choice, _) = conn.choose_runtime_command();
        match choice {
            RuntimeFetch::OCH(cmd) => assert_eq!(cmd, "O"),
            _ => panic!("ForceOCH should override Speeduino Burst default"),
        }
    }

    #[test]
    fn test_force_modes() {
        let mut cfg = ConnectionConfig::default();
        cfg.runtime_packet_mode = RuntimePacketMode::ForceOCH;
        let mut conn = Connection::new(cfg.clone());
        let mut proto = ProtocolSettings::default();
        proto.och_get_command = Some("O".to_string());
        proto.burst_get_command = Some("A".to_string());
        conn.set_protocol(proto, Endianness::Little);
        let (choice, _) = conn.choose_runtime_command();
        match choice {
            RuntimeFetch::OCH(cmd) => assert_eq!(cmd, "O"),
            _ => panic!("Expected OCH due to ForceOCH"),
        }

        let mut cfg2 = ConnectionConfig::default();
        cfg2.runtime_packet_mode = RuntimePacketMode::ForceBurst;
        let conn2 = Connection::new(cfg2);
        let (choice2, _) = conn2.choose_runtime_command();
        match choice2 {
            RuntimeFetch::Burst(cmd) => assert_eq!(cmd, "A".to_string()),
            _ => panic!("Expected Burst due to ForceBurst"),
        }
    }

    #[test]
    fn test_adaptive_switch_to_och() {
        // rusEFI keeps the adaptive-timing heuristic (Issue #71).
        let cfg = ConnectionConfig::default();
        let mut conn = Connection::new(cfg);
        conn.set_ecu_type(EcuType::RusEFI);
        let mut proto = ProtocolSettings::default();
        proto.och_get_command = Some("O".to_string());
        proto.burst_get_command = Some("A".to_string());
        conn.set_protocol(proto, Endianness::Little);

        // enable adaptive timing, record slow responses
        conn.enable_adaptive_timing(None);
        conn.record_response_time(std::time::Duration::from_millis(200));
        conn.record_response_time(std::time::Duration::from_millis(180));
        let (choice, reason) = conn.choose_runtime_command();
        match choice {
            RuntimeFetch::OCH(cmd) => assert_eq!(cmd, "O"),
            _ => panic!("Expected OCH due to adaptive timing, got {:?}", choice),
        }
        assert!(reason.starts_with("adaptive") || reason.contains("avg"));
    }

    #[test]
    fn test_parse_rusefi_text_output_basic_msg() {
        // Single msg entry
        let raw = "msg`Hello from ECU`";
        let result = Connection::parse_rusefi_text_output(raw);
        assert_eq!(result, "Hello from ECU");
    }

    #[test]
    fn test_parse_rusefi_text_output_multiple_msgs() {
        // Multiple msg entries concatenated
        let raw = "msg`First message`msg`Second message`msg`Third message`";
        let result = Connection::parse_rusefi_text_output(raw);
        assert_eq!(result, "First message\nSecond message\nThird message");
    }

    #[test]
    fn test_parse_rusefi_text_output_filters_wave_chart() {
        // wave_chart and table data should be filtered out
        let raw = "msg`RPM=1200`wave_chart`some chart data here`msg`emu: running`";
        let result = Connection::parse_rusefi_text_output(raw);
        assert_eq!(result, "RPM=1200\nemu: running");
    }

    #[test]
    fn test_parse_rusefi_text_output_filters_table_data() {
        let raw = "msg`Status OK`t|d_123`456`msg`Done`";
        let result = Connection::parse_rusefi_text_output(raw);
        assert_eq!(result, "Status OK\nDone");
    }

    #[test]
    fn test_parse_rusefi_text_output_empty_input() {
        assert_eq!(Connection::parse_rusefi_text_output(""), "");
    }

    #[test]
    fn test_parse_rusefi_text_output_no_backticks() {
        // Plain text without backtick delimiters should be returned as-is
        let raw = "Some plain text response";
        let result = Connection::parse_rusefi_text_output(raw);
        assert_eq!(result, "Some plain text response");
    }

    #[test]
    fn test_parse_rusefi_text_output_real_boot_sequence() {
        // Simulated rusEFI boot output (truncated)
        let raw = "msg`custom board hello from simulator`msg`Storage INT_FLASH registered`msg`Flash: Reading storage ID 1 @0x1 ... 33984 bytes`msg`emu: RPM=1200`wave_chart`r1200`";
        let result = Connection::parse_rusefi_text_output(raw);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "custom board hello from simulator");
        assert_eq!(lines[1], "Storage INT_FLASH registered");
        assert!(lines[2].contains("Flash: Reading storage ID 1"));
        assert_eq!(lines[3], "emu: RPM=1200");
    }

    #[test]
    fn test_parse_rusefi_text_output_emu_tag() {
        // "emu" is a recognized message tag
        let raw = "emu`RPM=1200`emu`shape update for ch0`";
        let result = Connection::parse_rusefi_text_output(raw);
        assert_eq!(result, "RPM=1200\nshape update for ch0");
    }
}

#[cfg(test)]
mod burn_page_tests {
    /// Speeduino's INI declares one `burnCommand` per page, each taking the page
    /// number. A burn that only ever sends page 0 therefore commits the main
    /// config page and leaves every table in RAM - where it reads back correctly
    /// until the next power cycle throws it away.
    #[test]
    fn dirty_pages_accumulate_and_clear() {
        let mut dirty: std::collections::BTreeSet<u8> = Default::default();
        // two table writes, on the pages the VE and ignition tables really use
        dirty.insert(2);
        dirty.insert(3);
        assert_eq!(dirty.iter().copied().collect::<Vec<_>>(), vec![2, 3]);

        // burning page 3 must not clear page 2
        dirty.remove(&3);
        assert_eq!(
            dirty.iter().copied().collect::<Vec<_>>(),
            vec![2],
            "burning one page must leave the others dirty - this is the whole bug"
        );
        dirty.remove(&2);
        assert!(dirty.is_empty());
    }

    /// With nothing written, the old single-page behaviour is preserved so a
    /// caller that just wants to poke the ECU still can.
    #[test]
    fn an_empty_dirty_set_still_burns_page_zero() {
        let dirty: std::collections::BTreeSet<u8> = Default::default();
        let pages: Vec<u8> = if dirty.is_empty() {
            vec![0]
        } else {
            dirty.iter().copied().collect()
        };
        assert_eq!(pages, vec![0]);
    }

    /// Pages come out in order, so a burn sequence is deterministic and a failure
    /// part-way through leaves a predictable state rather than an arbitrary one.
    #[test]
    fn pages_burn_in_ascending_order() {
        let mut dirty: std::collections::BTreeSet<u8> = Default::default();
        for p in [7u8, 2, 15, 3, 0] {
            dirty.insert(p);
        }
        assert_eq!(
            dirty.iter().copied().collect::<Vec<_>>(),
            vec![0, 2, 3, 7, 15]
        );
    }
}

#[cfg(test)]
mod page_crc_tests {
    use super::*;

    /// The ECU's `d` command returns a standard reflected CRC-32 of the page
    /// bytes. Confirmed against a Speeduino 202501 on every page tested: the
    /// value it reports equals a CRC of exactly what `read_page` gives back.
    #[test]
    fn the_local_crc_matches_the_convention_the_ecu_uses() {
        // Values cross-checked against the firmware's calculatePageCRC32 on a
        // real ECU, and against zlib.
        let mut h = crc32fast::Hasher::new();
        h.update(b"123456789");
        assert_eq!(
            h.finalize(),
            0xCBF4_3926,
            "not the standard CRC-32 check value"
        );
    }

    /// A CRC catches what a per-frame acknowledgement cannot: every frame of a
    /// chunked page write can be acked while the page still assembles into
    /// something other than what was sent.
    #[test]
    fn one_flipped_bit_changes_the_crc() {
        let page: Vec<u8> = (0..288u16).map(|i| (i % 251) as u8).collect();
        let mut corrupt = page.clone();
        corrupt[144] ^= 0x01;

        let crc = |d: &[u8]| {
            let mut h = crc32fast::Hasher::new();
            h.update(d);
            h.finalize()
        };
        assert_ne!(
            crc(&page),
            crc(&corrupt),
            "a single bit flip must not collide"
        );
    }

    /// A page with no declared command reports that, rather than silently
    /// passing - "not checked" must never read as "checked and fine".
    #[test]
    fn an_undeclared_page_is_reported_not_skipped() {
        let mut conn = Connection::new(ConnectionConfig::default());
        let err = conn
            .verify_page_crc(3, &[0u8; 8])
            .expect_err("no protocol settings means no declared command");
        let msg = err.to_string();
        assert!(
            msg.contains("crc32CheckCommand") && msg.contains('3'),
            "error should name the missing command and the page: {msg}"
        );
    }
}
