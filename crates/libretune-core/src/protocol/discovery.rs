//! ECU auto-discovery (spec §15.4 — "discoverEcu" probe matrix).
//!
//! Sweeps the configured serial ports across a baud-rate × query-command
//! matrix, classifies each response, and returns a list of candidate ECUs.
//!
//! Implementation notes:
//! * Pure-Rust, transport-agnostic. The probe takes a channel-opener closure
//!   (`OpenPort`) so unit tests can plug in a [`MockChannel`](#tests) and CI
//!   doesn't need a real serial port. The Tauri-facing wrapper passes
//!   [`super::serial::open_port`].
//! * Per-port timing follows the spec recommendation:
//!   - 1500 ms post-open settle (DTR/Arduino reset)
//!   - 40 ms pre-write spacer
//!   - 600 ms read window per query
//! * Classification rules (spec §15.4.2):
//!   - `b0 == b1 == b2 == query` → echo (port loops back, not an ECU).
//!   - `b0 & 0xE0 == 0xE0 && b1 & 0xF0 == 0 && b2 == b'>'` → MS bootloader.
//!   - First byte `0x01` → BigStuff3.
//!   - Reply starts with `$GP` → GPS receiver (reject).
//!   - 20-byte ASCII reply → MegaSquirt-family signature (Speeduino,
//!     rusEFI/FOME/epicEFI all reply with a printable signature here).
//!   - Anything else → `Unknown`.
//!
//! Public surface: [`discover_ecu`], [`DiscoveryResult`], [`DiscoveredEcu`],
//! [`DiscoveryConfig`].

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use super::stream::CommunicationChannel;

/// Default baud-rate sweep used when an INI doesn't override it (spec §15.4.1).
pub const DEFAULT_SEARCH_BAUD_RATES: &[u32] = &[115200, 921600, 460800, 230400, 57600, 9600];

/// Default query commands used when an INI doesn't override
/// `deviceSearchQueryCommands`. These are the conventional MS/Speeduino/rusEFI
/// signature requests.
pub const DEFAULT_SEARCH_QUERIES: &[u8] = b"QS";

/// Settle delay after opening a port — Arduino-based ECUs reset on DTR
/// transition and need ~1.5 s to come back up (spec §15.4).
pub const POST_OPEN_SETTLE: Duration = Duration::from_millis(1500);

/// Spacer between port-open and first write.
pub const PRE_WRITE_DELAY: Duration = Duration::from_millis(40);

/// Read window per query attempt.
pub const READ_WINDOW: Duration = Duration::from_millis(600);

/// Tunables for [`discover_ecu`]. Defaults match spec §15.4.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub baud_rates: Vec<u32>,
    pub query_commands: Vec<u8>,
    pub post_open_settle: Duration,
    pub pre_write_delay: Duration,
    pub read_window: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            baud_rates: DEFAULT_SEARCH_BAUD_RATES.to_vec(),
            query_commands: DEFAULT_SEARCH_QUERIES.to_vec(),
            post_open_settle: POST_OPEN_SETTLE,
            pre_write_delay: PRE_WRITE_DELAY,
            read_window: READ_WINDOW,
        }
    }
}

/// Classification of what was found on a port at a given baud (spec §15.4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiscoveryResult {
    /// 20-byte ASCII signature reply — likely an MS-protocol ECU
    /// (Speeduino / rusEFI / FOME / epicEFI / MS2 / MS3).
    Signature {
        baud: u32,
        query: u8,
        signature: String,
    },
    /// Megasquirt-family bootloader reply (firmware update mode).
    Bootloader { baud: u32, query: u8 },
    /// BigStuff3 reply (first byte `0x01`).
    BigStuff3 { baud: u32, query: u8 },
    /// Echo — the bytes we wrote came right back. Almost always means the
    /// port is looped back or there is no device. **Not** a candidate.
    Echo { baud: u32, query: u8 },
    /// GPS receiver streaming NMEA. Explicitly rejected (spec §15.4.2).
    GpsReceiver { baud: u32 },
    /// Got *some* bytes but couldn't classify them.
    Unknown {
        baud: u32,
        query: u8,
        bytes: Vec<u8>,
    },
    /// The port opened but returned nothing across all queries at this baud.
    Silent { baud: u32 },
    /// Couldn't even open the port.
    OpenFailed { baud: u32, error: String },
}

impl DiscoveryResult {
    /// True if this result represents a discovered ECU we'd want to surface
    /// in the UI (signature, bootloader, or BigStuff3).
    pub fn is_candidate(&self) -> bool {
        matches!(
            self,
            DiscoveryResult::Signature { .. }
                | DiscoveryResult::Bootloader { .. }
                | DiscoveryResult::BigStuff3 { .. }
        )
    }
}

/// One port's discovery outcome — keeps the port name plus every per-baud
/// classification so the UI can render a complete probe report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredEcu {
    pub port: String,
    pub results: Vec<DiscoveryResult>,
}

impl DiscoveredEcu {
    /// Best (highest-priority candidate) result for this port, if any.
    /// Priority: `Signature` > `Bootloader` > `BigStuff3`.
    pub fn best(&self) -> Option<&DiscoveryResult> {
        self.results
            .iter()
            .find(|r| matches!(r, DiscoveryResult::Signature { .. }))
            .or_else(|| {
                self.results
                    .iter()
                    .find(|r| matches!(r, DiscoveryResult::Bootloader { .. }))
            })
            .or_else(|| {
                self.results
                    .iter()
                    .find(|r| matches!(r, DiscoveryResult::BigStuff3 { .. }))
            })
    }
}

/// Channel-opener used by the probe. Allows tests to inject a mock channel.
pub type OpenPort =
    dyn FnMut(&str, u32) -> std::io::Result<Box<dyn CommunicationChannel>> + Send;

/// Probe each port in `ports` against the baud × query matrix in `config`.
///
/// Returns one [`DiscoveredEcu`] per requested port (order preserved).
/// Stops probing further bauds on a port as soon as a candidate is found.
pub fn discover_ecu(
    ports: &[String],
    config: &DiscoveryConfig,
    open: &mut OpenPort,
) -> Vec<DiscoveredEcu> {
    ports
        .iter()
        .map(|p| probe_port(p, config, open))
        .collect()
}

fn probe_port(port: &str, config: &DiscoveryConfig, open: &mut OpenPort) -> DiscoveredEcu {
    let mut results = Vec::new();

    'baud: for &baud in &config.baud_rates {
        let mut channel = match open(port, baud) {
            Ok(ch) => ch,
            Err(e) => {
                results.push(DiscoveryResult::OpenFailed {
                    baud,
                    error: e.to_string(),
                });
                continue 'baud;
            }
        };

        // Settle (Arduino DTR reset) + read-timeout for the probe window.
        thread::sleep(config.post_open_settle);
        let _ = channel.set_timeout(config.read_window);
        let _ = channel.clear_input_buffer();

        let mut got_anything_this_baud = false;

        for &query in &config.query_commands {
            thread::sleep(config.pre_write_delay);
            let _ = channel.clear_input_buffer();

            if channel.write_all(&[query]).is_err() {
                continue;
            }
            let _ = channel.flush();

            let bytes = read_window(&mut *channel, config.read_window);
            if bytes.is_empty() {
                continue;
            }
            got_anything_this_baud = true;

            let result = classify(baud, query, &bytes);

            // GPS rejection: stop probing this port — clearly not an ECU.
            let was_gps = matches!(result, DiscoveryResult::GpsReceiver { .. });
            let is_candidate = result.is_candidate();
            results.push(result);
            if was_gps {
                return DiscoveredEcu {
                    port: port.to_string(),
                    results,
                };
            }
            if is_candidate {
                // Found something useful at this baud; don't waste time on the
                // remaining baud rates.
                return DiscoveredEcu {
                    port: port.to_string(),
                    results,
                };
            }
        }

        if !got_anything_this_baud {
            results.push(DiscoveryResult::Silent { baud });
        }
    }

    DiscoveredEcu {
        port: port.to_string(),
        results,
    }
}

fn read_window(channel: &mut dyn CommunicationChannel, window: Duration) -> Vec<u8> {
    let deadline = Instant::now() + window;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 256];
    while Instant::now() < deadline {
        match channel.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                // Heuristic: if we already have >= 20 bytes, that's enough to
                // classify a signature reply.
                if buf.len() >= 32 {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) => break,
        }
    }
    buf
}

fn classify(baud: u32, query: u8, bytes: &[u8]) -> DiscoveryResult {
    // GPS NMEA stream — "$GP..." is unmistakable, check first.
    if bytes.len() >= 3 && &bytes[..3] == b"$GP" {
        return DiscoveryResult::GpsReceiver { baud };
    }

    // Echo: the entirety of what we got is just our query byte repeated.
    // (Some loopback adapters echo once, others repeat — accept both.)
    if !bytes.is_empty() && bytes.iter().all(|&b| b == query) {
        return DiscoveryResult::Echo { baud, query };
    }

    // MS bootloader: first 3 bytes match the well-known pattern (spec §15.4.2).
    if bytes.len() >= 3
        && bytes[0] & 0xE0 == 0xE0
        && bytes[1] & 0xF0 == 0
        && bytes[2] == b'>'
    {
        return DiscoveryResult::Bootloader { baud, query };
    }

    // BigStuff3: first byte is 0x01.
    if bytes[0] == 0x01 {
        return DiscoveryResult::BigStuff3 { baud, query };
    }

    // 20-byte ASCII reply → likely an MS-family signature.
    // We accept anything 8..=64 bytes that's mostly printable as a candidate
    // signature; real signatures range from "Speeduino 2024.10" (~17 bytes) to
    // longer rusEFI strings.
    let printable = bytes
        .iter()
        .filter(|b| b.is_ascii_graphic() || **b == b' ')
        .count();
    if bytes.len() >= 8 && printable * 100 / bytes.len() >= 70 {
        let signature = String::from_utf8_lossy(bytes).trim().to_string();
        return DiscoveryResult::Signature {
            baud,
            query,
            signature,
        };
    }

    DiscoveryResult::Unknown {
        baud,
        query,
        bytes: bytes.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};

    /// A scripted in-memory channel: returns canned responses based on the
    /// last-written byte. Used to exercise the probe matrix without any real
    /// serial port.
    struct MockChannel {
        rx: Vec<u8>,
        last_query: Option<u8>,
        responses: Arc<Mutex<dyn FnMut(u8) -> Vec<u8> + Send>>,
    }

    impl io::Read for MockChannel {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.rx.is_empty() {
                if let Some(q) = self.last_query.take() {
                    self.rx = (self.responses.lock().unwrap())(q);
                }
            }
            if self.rx.is_empty() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no data"));
            }
            let n = buf.len().min(self.rx.len());
            buf[..n].copy_from_slice(&self.rx[..n]);
            self.rx.drain(..n);
            Ok(n)
        }
    }

    impl io::Write for MockChannel {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if let Some(&b) = buf.first() {
                self.last_query = Some(b);
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl CommunicationChannel for MockChannel {
        fn set_timeout(&mut self, _t: Duration) -> io::Result<()> {
            Ok(())
        }
        fn clear_input_buffer(&mut self) -> io::Result<()> {
            self.rx.clear();
            Ok(())
        }
        fn clear_output_buffer(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn try_clone(&self) -> io::Result<Box<dyn CommunicationChannel>> {
            Err(io::Error::other("mock"))
        }
        fn bytes_to_read(&mut self) -> io::Result<u32> {
            Ok(self.rx.len() as u32)
        }
    }

    fn fast_config() -> DiscoveryConfig {
        DiscoveryConfig {
            baud_rates: vec![115200],
            query_commands: vec![b'Q', b'S'],
            post_open_settle: Duration::from_millis(0),
            pre_write_delay: Duration::from_millis(0),
            read_window: Duration::from_millis(20),
        }
    }

    fn opener<F: FnMut(u8) -> Vec<u8> + Send + 'static>(f: F) -> Box<OpenPort> {
        let responses: Arc<Mutex<dyn FnMut(u8) -> Vec<u8> + Send>> = Arc::new(Mutex::new(f));
        Box::new(move |_port: &str, _baud: u32| {
            Ok(Box::new(MockChannel {
                rx: Vec::new(),
                last_query: None,
                responses: responses.clone(),
            }) as Box<dyn CommunicationChannel>)
        })
    }

    #[test]
    fn classifies_speeduino_signature() {
        let mut open = opener(|q| {
            if q == b'S' {
                b"Speeduino 2024.10  ".to_vec()
            } else {
                Vec::new()
            }
        });
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        assert_eq!(out.len(), 1);
        let best = out[0].best().expect("candidate");
        match best {
            DiscoveryResult::Signature { signature, .. } => {
                assert!(signature.starts_with("Speeduino"));
            }
            other => panic!("expected Signature, got {:?}", other),
        }
    }

    #[test]
    fn classifies_bootloader() {
        let mut open = opener(|_q| vec![0xE0, 0x05, b'>']);
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        assert!(matches!(
            out[0].best(),
            Some(DiscoveryResult::Bootloader { .. })
        ));
    }

    #[test]
    fn classifies_bigstuff3() {
        let mut open = opener(|_q| vec![0x01, 0xAA, 0xBB, 0xCC]);
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        assert!(matches!(
            out[0].best(),
            Some(DiscoveryResult::BigStuff3 { .. })
        ));
    }

    #[test]
    fn rejects_gps() {
        let mut open = opener(|_q| b"$GPGGA,123519,4807.038".to_vec());
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        assert!(out[0]
            .results
            .iter()
            .any(|r| matches!(r, DiscoveryResult::GpsReceiver { .. })));
        assert!(out[0].best().is_none());
    }

    #[test]
    fn detects_echo() {
        let mut open = opener(|q| vec![q, q, q, q]);
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        // Echo on every query → no candidate.
        assert!(out[0].best().is_none());
        assert!(out[0]
            .results
            .iter()
            .all(|r| matches!(r, DiscoveryResult::Echo { .. })));
    }

    #[test]
    fn silent_port_reports_silent() {
        let mut open = opener(|_q| Vec::new());
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        assert!(out[0]
            .results
            .iter()
            .any(|r| matches!(r, DiscoveryResult::Silent { .. })));
    }

    #[test]
    fn open_failure_recorded() {
        let mut open: Box<OpenPort> = Box::new(|_p: &str, _b: u32| {
            Err(io::Error::other("nope"))
        });
        let cfg = fast_config();
        let out = discover_ecu(&["fake0".into()], &cfg, &mut *open);
        assert!(out[0]
            .results
            .iter()
            .any(|r| matches!(r, DiscoveryResult::OpenFailed { .. })));
    }
}
