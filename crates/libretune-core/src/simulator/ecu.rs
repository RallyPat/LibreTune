//! The virtual ECU itself: a [`CommunicationChannel`] that answers the real
//! serial protocol.
//!
//! Wire semantics follow Speeduino's `comms.cpp` / `comms_legacy.cpp`: the
//! `Q`/`S`/`A` handshake, `p`/`M`/`b` page read/write/burn, and the
//! `r`+0x30 realtime window, in both the plain and CRC-enveloped framings.
//! Because it is a real channel, [`crate::protocol::Connection`] talks to it
//! with the same code path it uses for a serial port — the simulator is not
//! a special case anywhere above this module.

use super::engine::{EngineMode, SimEngine};
use super::ve_model::{self, VeContext};
use crate::ecu::EcuMemory;
use crate::ini::EcuDefinition;
use crate::protocol::{CommunicationChannel, EnvelopeOrder, Packet};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Speeduino's `SERIAL_RC_OK`.
const RC_OK: u8 = 0x00;
/// Speeduino's `SERIAL_RC_BURN_OK`.
const RC_BURN_OK: u8 = 0x04;
/// The only `r` sub-command understood: "send output channels".
const SUBCMD_OUTPUT_CHANNELS: u8 = 0x30;

/// Page RAM plus its flash image.
///
/// Writes land in RAM; `burn` commits RAM to flash; `reboot` restores RAM
/// from flash. That is what makes "you forgot to burn" reproducible in the
/// simulator instead of only on real hardware.
struct PageMemory {
    ram: EcuMemory,
    flash: EcuMemory,
}

impl PageMemory {
    fn new(def: &EcuDefinition) -> Self {
        Self {
            ram: EcuMemory::from_definition(def),
            flash: EcuMemory::from_definition(def),
        }
    }

    fn read(&self, page: u8, offset: u16, count: u16) -> Vec<u8> {
        let mut out = self
            .ram
            .read_bytes(page, offset, count)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        // A read past the end of a page zero-pads rather than failing: the
        // firmware answers a fixed-size block whatever the request.
        out.resize(count as usize, 0);
        out
    }

    fn write(&mut self, page: u8, offset: u16, value: &[u8]) {
        self.ram.write_bytes(page, offset, value);
    }

    fn burn(&mut self, page: u8) {
        if let Some(data) = self.ram.get_page(page) {
            let copy = data.to_vec();
            self.flash.load_page(page, copy);
        }
    }

    fn reboot(&mut self) {
        for page in 0..self.flash.page_count() {
            if let Some(data) = self.flash.get_page(page) {
                let copy = data.to_vec();
                self.ram.load_page(page, copy);
            }
        }
    }
}

/// State shared between the simulator handle and every channel opened on it.
struct Pipe {
    cmd_buf: VecDeque<u8>,
    rsp_buf: VecDeque<u8>,
    /// Simulates an unplugged cable: reads and writes fail while set.
    dropped: bool,
    /// Firmware second counter — byte 0 of the realtime frame.
    secl: u8,
    memory: PageMemory,
    och_block: Vec<u8>,
    /// Whether the first realtime request has been answered, which is what
    /// resets `secl` to a known origin (comms.cpp's `firstCommsRequest`).
    first_och_done: bool,
    engine: Option<SimEngine>,
    /// Retained so each tick can re-resolve the `[VeAnalyze]`-bound veTable
    /// against current page memory.
    definition: Option<EcuDefinition>,
    envelope: EnvelopeOrder,
    /// What `Q` and `S` answer. Taken from the definition so a handshake
    /// against the very INI the simulator was built from succeeds; a
    /// hardcoded signature would only ever match Speeduino.
    signature: String,
    version: String,
    /// Whether realtime requests advance the engine off the wall clock.
    /// Permanently disabled once a caller drives time explicitly, so tests
    /// never race the clock.
    auto_tick: bool,
    last_auto_tick: Option<Instant>,
}

impl Pipe {
    fn new(def: Option<&EcuDefinition>) -> Self {
        let engine = def.map(SimEngine::new);
        Self {
            cmd_buf: VecDeque::new(),
            rsp_buf: VecDeque::new(),
            dropped: false,
            secl: 0,
            memory: def
                .map(PageMemory::new)
                .unwrap_or_else(|| PageMemory::new(&EcuDefinition::default())),
            och_block: engine
                .as_ref()
                .map(|e| e.och_block().to_vec())
                .unwrap_or_default(),
            first_och_done: false,
            engine,
            definition: def.cloned(),
            envelope: EnvelopeOrder::BigEndian,
            signature: def
                .map(|d| d.signature.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| EcuSimulator::DEFAULT_SIGNATURE.to_string()),
            version: def
                .map(|d| d.version_info.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| EcuSimulator::DEFAULT_VERSION.to_string()),
            auto_tick: true,
            last_auto_tick: None,
        }
    }

    fn ve_context(&self) -> Option<VeContext> {
        ve_model::ve_context(self.definition.as_ref()?, &self.memory.ram)
    }

    /// The first realtime request after boot resets `secl`, so a tuner's
    /// stay-alive counter starts from a known origin.
    fn on_och_request(&mut self) {
        if self.first_och_done {
            return;
        }
        self.first_och_done = true;
        self.secl = 0;
        if let Some(engine) = self.engine.as_mut() {
            engine.reset_secl();
            self.och_block.clear();
            self.och_block.extend_from_slice(engine.och_block());
        }
    }

    /// Advance the engine by the wall-clock time since the last request, so
    /// ordinary polling animates the model. Without this the simulator would
    /// replay one stale frame forever, since the engine has no clock of its
    /// own.
    fn auto_tick(&mut self) {
        if !self.auto_tick {
            return;
        }
        let ctx = self.ve_context();
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        engine.set_ve_context(ctx);
        let now = Instant::now();
        let dt = now.duration_since(self.last_auto_tick.unwrap_or(now));
        engine.tick(dt);
        self.och_block.clear();
        self.och_block.extend_from_slice(engine.och_block());
        self.last_auto_tick = Some(now);
    }
}

/// Total byte length of the plain-protocol command at the front of `buf`, or
/// `None` when too few bytes have arrived to even tell.
fn plain_command_len(buf: &VecDeque<u8>) -> Option<usize> {
    match *buf.front()? {
        b'p' | b'r' => Some(7),
        b'b' => Some(3),
        b'M' => {
            if buf.len() < 7 {
                return None;
            }
            let count = u16::from_le_bytes([buf[5], buf[6]]) as usize;
            Some(7 + count)
        }
        // 'Q' / 'S' / 'A' and anything unrecognized.
        _ => Some(1),
    }
}

/// `(page, offset, count)` from a `p`/`M` command. The page identifier's
/// high byte is always zero on this protocol, so only the low byte is read.
fn parse_page_offset_count(bytes: &[u8]) -> Option<(u8, u16, u16)> {
    let page = *bytes.get(2)?;
    let offset = u16::from_le_bytes([*bytes.get(3)?, *bytes.get(4)?]);
    let count = u16::from_le_bytes([*bytes.get(5)?, *bytes.get(6)?]);
    Some((page, offset, count))
}

fn parse_page(bytes: &[u8]) -> Option<u8> {
    bytes.get(2).copied()
}

/// `(offset, len)` from an `r` command: `['r', canId, 0x30, offset, len]`.
/// Byte 1 is the CAN id, discarded exactly as the firmware discards it.
fn parse_och_window(bytes: &[u8]) -> Option<(u16, u16)> {
    if *bytes.get(2)? != SUBCMD_OUTPUT_CHANNELS {
        return None;
    }
    let offset = u16::from_le_bytes([*bytes.get(3)?, *bytes.get(4)?]);
    let len = u16::from_le_bytes([*bytes.get(5)?, *bytes.get(6)?]);
    Some((offset, len))
}

/// Window `len` bytes at `offset` out of the realtime block, zero-padding
/// past the end. An out-of-range window must never panic the channel.
fn och_window(block: &[u8], offset: u16, len: u16) -> Vec<u8> {
    let len = len as usize;
    let start = (offset as usize).min(block.len());
    let end = start.saturating_add(len).min(block.len());
    let mut out = block[start..end].to_vec();
    out.resize(len, 0);
    out
}

/// Dispatch one command, returning the response payload without any
/// envelope.
///
/// `enveloped` selects the status-byte convention. Under msEnvelope_1.0
/// every response payload is `rc(1) || data`, so the CRC framing prefixes a
/// return code on all of them; the plain framing carries none, and there
/// writes and burns are unacknowledged entirely.
fn respond(cmd: &[u8], pipe: &mut Pipe, enveloped: bool) -> Vec<u8> {
    let ok = |body: Vec<u8>| -> Vec<u8> {
        if enveloped {
            let mut v = vec![RC_OK];
            v.extend(body);
            v
        } else {
            body
        }
    };
    match cmd.first().copied().unwrap_or(0) {
        b'Q' => {
            let mut v = pipe.signature.as_bytes().to_vec();
            v.push(0);
            ok(v)
        }
        b'S' => {
            let mut v = pipe.version.as_bytes().to_vec();
            v.push(0);
            ok(v)
        }
        b'A' => ok(vec![pipe.secl]),
        b'p' => match parse_page_offset_count(cmd) {
            Some((page, offset, count)) => ok(pipe.memory.read(page, offset, count)),
            None => ok(Vec::new()),
        },
        b'M' => {
            if let Some((page, offset, count)) = parse_page_offset_count(cmd) {
                let value = cmd.get(7..7 + count as usize).unwrap_or(&[]);
                pipe.memory.write(page, offset, value);
            }
            if enveloped {
                vec![RC_OK]
            } else {
                Vec::new()
            }
        }
        b'b' => {
            if let Some(page) = parse_page(cmd) {
                pipe.memory.burn(page);
            }
            if enveloped {
                vec![RC_BURN_OK]
            } else {
                Vec::new()
            }
        }
        b'r' => match parse_och_window(cmd) {
            Some((offset, len)) => {
                pipe.on_och_request();
                pipe.auto_tick();
                ok(och_window(&pipe.och_block, offset, len))
            }
            None => ok(Vec::new()),
        },
        _ => vec![0],
    }
}

/// Drain buffered commands, dispatch them, and queue their responses.
fn process(pipe: &Arc<Mutex<Pipe>>) {
    let mut p = pipe.lock().unwrap_or_else(|e| e.into_inner());
    if p.dropped {
        return;
    }
    while !p.cmd_buf.is_empty() {
        if *p.cmd_buf.front().unwrap() == 0x00 {
            // CRC envelope: [len(2), payload.., crc(4)]
            if p.cmd_buf.len() < 2 {
                break;
            }
            let mut len_bytes = [0u8; 2];
            len_bytes.copy_from_slice(&[p.cmd_buf[0], p.cmd_buf[1]]);
            let plen = p.envelope.read_u16(&len_bytes) as usize;
            if p.cmd_buf.len() < 2 + plen + 4 {
                break;
            }
            let _ = p.cmd_buf.drain(..2);
            let payload: Vec<u8> = p.cmd_buf.drain(..plen).collect();
            let _ = p.cmd_buf.drain(..4);
            let response = respond(&payload, &mut p, true);
            let order = p.envelope;
            let framed = Packet::new(response).to_bytes_ordered(order);
            p.rsp_buf.extend(framed);
            return;
        }
        let Some(len) = plain_command_len(&p.cmd_buf) else {
            break;
        };
        if p.cmd_buf.len() < len {
            break;
        }
        let cmd: Vec<u8> = p.cmd_buf.drain(..len).collect();
        let out = respond(&cmd, &mut p, false);
        p.rsp_buf.extend(out);
    }
}

/// A channel onto a running [`EcuSimulator`], usable anywhere a serial or
/// TCP channel is.
pub struct SimulatorChannel {
    pipe: Arc<Mutex<Pipe>>,
}

impl Read for SimulatorChannel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut p = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
        if p.dropped {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "simulated link is down",
            ));
        }
        let n = buf.len().min(p.rsp_buf.len());
        for slot in buf.iter_mut().take(n) {
            *slot = p.rsp_buf.pop_front().expect("length checked above");
        }
        Ok(n)
    }
}

impl Write for SimulatorChannel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        {
            let mut p = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
            if p.dropped {
                return Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "simulated link is down",
                ));
            }
            p.cmd_buf.extend(buf);
        }
        process(&self.pipe);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl CommunicationChannel for SimulatorChannel {
    /// The simulator answers instantly, so there is nothing to time out.
    fn set_timeout(&mut self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }

    fn clear_input_buffer(&mut self) -> io::Result<()> {
        self.pipe
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rsp_buf
            .clear();
        Ok(())
    }

    fn clear_output_buffer(&mut self) -> io::Result<()> {
        self.pipe
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cmd_buf
            .clear();
        Ok(())
    }

    fn try_clone(&self) -> io::Result<Box<dyn CommunicationChannel>> {
        Ok(Box::new(SimulatorChannel {
            pipe: Arc::clone(&self.pipe),
        }))
    }

    fn bytes_to_read(&mut self) -> io::Result<u32> {
        Ok(self
            .pipe
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rsp_buf
            .len() as u32)
    }
}

/// A virtual ECU. Open channels onto it with [`Self::channel`].
pub struct EcuSimulator {
    pipe: Arc<Mutex<Pipe>>,
}

impl EcuSimulator {
    /// Answered by a simulator built without a definition, or when the INI
    /// declares no signature of its own.
    pub const DEFAULT_SIGNATURE: &'static str = "speeduino 202504-dev";
    pub const DEFAULT_VERSION: &'static str = "Speeduino 2025.04-dev";

    /// A handshake-only simulator with no pages and no engine: page commands
    /// are no-ops and realtime windows zero-fill. Use
    /// [`Self::from_definition`] once the INI geometry is known.
    pub fn new() -> Self {
        Self {
            pipe: Arc::new(Mutex::new(Pipe::new(None))),
        }
    }

    /// A simulator backed by `definition`: zero-filled RAM and flash per
    /// declared page, plus an engine writing the `[OutputChannels]` frame.
    pub fn from_definition(definition: &EcuDefinition) -> Self {
        Self {
            pipe: Arc::new(Mutex::new(Pipe::new(Some(definition)))),
        }
    }

    /// Open a channel onto this simulator.
    pub fn channel(&self) -> SimulatorChannel {
        SimulatorChannel {
            pipe: Arc::clone(&self.pipe),
        }
    }

    /// Advance the engine by `dt` and refresh the realtime frame.
    ///
    /// The engine has no wall clock, so this call is what moves its
    /// simulated time. Calling it even once hands time-keeping to the caller
    /// and permanently disables the wall-clock auto-tick, so a test driving
    /// time by hand cannot race the clock.
    pub fn tick_engine(&self, dt: Duration) {
        let mut guard = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
        let p = &mut *guard;
        p.auto_tick = false;
        let ctx = p.ve_context();
        if let Some(engine) = p.engine.as_mut() {
            engine.set_ve_context(ctx);
            engine.tick(dt);
            p.och_block.clear();
            p.och_block.extend_from_slice(engine.och_block());
        }
    }

    /// Force an operating mode — the only way to trigger a wide-open-throttle
    /// pull, which the state machine never enters on its own. The mode's
    /// targets load immediately and the state machine carries on from there.
    pub fn set_mode(&self, mode: EngineMode) {
        let mut p = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(engine) = p.engine.as_mut() {
            engine.set_mode(mode);
        }
    }

    /// Simulate the cable being unplugged (and plugged back in).
    pub fn set_link_dropped(&self, dropped: bool) {
        self.pipe.lock().unwrap_or_else(|e| e.into_inner()).dropped = dropped;
    }

    /// Advance the second counter, wrapping at 255.
    pub fn advance_secl(&self, delta: u8) {
        let mut p = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
        p.secl = p.secl.wrapping_add(delta);
    }

    /// Simulate a reboot: un-burned RAM writes are lost, burned bytes
    /// survive, and the next realtime request counts as the first again.
    pub fn reboot(&self) {
        let mut p = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
        p.memory.reboot();
        p.secl = 0;
        p.first_och_done = false;
        p.last_auto_tick = None;
    }
}

impl Default for EcuSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ini::{DataType, Endianness, OutputChannel};

    fn definition() -> EcuDefinition {
        let mut def = EcuDefinition::default();
        def.endianness = Endianness::Little;
        def.page_sizes = vec![32, 16];
        def.n_pages = 2;
        def.protocol.och_block_size = 16;
        def.output_channels.insert(
            "rpm".to_string(),
            OutputChannel {
                name: "rpm".to_string(),
                data_type: DataType::U16,
                offset: 4,
                scale: 1.0,
                ..Default::default()
            },
        );
        def
    }

    fn read_n(channel: &mut SimulatorChannel, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        let got = channel.read(&mut buf).expect("read succeeds");
        buf.truncate(got);
        buf
    }

    #[test]
    fn answers_the_signature_query_on_the_plain_protocol() {
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();

        channel.write_all(b"Q").expect("write succeeds");

        let response = read_n(&mut channel, 64);
        assert_eq!(
            &response[..response.len() - 1],
            EcuSimulator::DEFAULT_SIGNATURE.as_bytes(),
            "signature is NUL-terminated"
        );
    }

    #[test]
    fn a_definition_backed_simulator_answers_that_definitions_signature() {
        // A hardcoded signature would fail the handshake for every ECU that
        // isn't Speeduino, silently taking demo mode down its fallback path.
        let mut def = definition();
        def.signature = "epicEFI 1.2.3".to_string();
        let sim = EcuSimulator::from_definition(&def);
        let mut channel = sim.channel();

        channel.write_all(b"Q").expect("write succeeds");

        let response = read_n(&mut channel, 64);
        assert_eq!(&response[..response.len() - 1], b"epicEFI 1.2.3");
    }

    #[test]
    fn page_writes_are_readable_back_at_the_same_offset() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        // 'M': page 1, offset 2, count 3, then the value bytes.
        let mut write = vec![b'M', 0, 1, 2, 0, 3, 0];
        write.extend([0xAA, 0xBB, 0xCC]);
        channel.write_all(&write).expect("write succeeds");
        assert_eq!(
            read_n(&mut channel, 8),
            Vec::<u8>::new(),
            "writes are unacknowledged"
        );

        channel
            .write_all(&[b'p', 0, 1, 2, 0, 3, 0])
            .expect("write succeeds");
        assert_eq!(read_n(&mut channel, 3), vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn a_reboot_loses_unburned_writes_but_keeps_burned_ones() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        channel
            .write_all(&[b'M', 0, 0, 0, 0, 1, 0, 0x11])
            .expect("write succeeds");
        channel.write_all(&[b'b', 0, 0]).expect("burn succeeds");
        channel
            .write_all(&[b'M', 0, 0, 1, 0, 1, 0, 0x22])
            .expect("write succeeds");

        sim.reboot();

        channel
            .write_all(&[b'p', 0, 0, 0, 0, 2, 0])
            .expect("read succeeds");
        assert_eq!(
            read_n(&mut channel, 2),
            vec![0x11, 0x00],
            "burned byte survives, un-burned one is lost"
        );
    }

    #[test]
    fn realtime_window_is_served_from_the_engine_and_zero_pads_past_the_end() {
        let sim = EcuSimulator::from_definition(&definition());
        sim.tick_engine(Duration::from_secs(2));
        let mut channel = sim.channel();

        // 'r': canId 0, subcmd 0x30, offset 0, len 20 (block is 16).
        channel
            .write_all(&[b'r', 0, 0x30, 0, 0, 20, 0])
            .expect("write succeeds");

        let frame = read_n(&mut channel, 20);
        assert_eq!(frame.len(), 20, "the window is always the requested length");
        assert_eq!(&frame[16..], &[0, 0, 0, 0], "past the block it zero-pads");
    }

    #[test]
    fn an_unknown_realtime_subcommand_is_answered_rather_than_panicking() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        channel
            .write_all(&[b'r', 0, 0x99, 0, 0, 4, 0])
            .expect("write succeeds");

        assert_eq!(read_n(&mut channel, 8), Vec::<u8>::new());
    }

    #[test]
    fn crc_enveloped_commands_are_answered_in_the_same_framing() {
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();

        let request = Packet::new(b"Q".to_vec()).to_bytes();
        channel.write_all(&request).expect("write succeeds");

        let response = read_n(&mut channel, 128);
        let packet = Packet::from_bytes(&response).expect("response is a valid packet");
        assert_eq!(
            packet.payload[0], RC_OK,
            "enveloped replies carry a return code"
        );
        assert_eq!(
            &packet.payload[1..packet.payload.len() - 1],
            EcuSimulator::DEFAULT_SIGNATURE.as_bytes(),
            "the signature follows the return code"
        );
    }

    #[test]
    fn a_partial_command_waits_for_the_rest_instead_of_being_dispatched() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        // Half of a 'p' command.
        channel.write_all(&[b'p', 0, 0]).expect("write succeeds");
        assert_eq!(
            read_n(&mut channel, 8),
            Vec::<u8>::new(),
            "nothing dispatched yet"
        );

        channel.write_all(&[0, 0, 2, 0]).expect("write succeeds");
        assert_eq!(
            read_n(&mut channel, 2).len(),
            2,
            "completed command answers"
        );
    }

    #[test]
    fn a_dropped_link_fails_reads_and_writes_until_restored() {
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();
        sim.set_link_dropped(true);

        assert!(
            channel.write_all(b"Q").is_err(),
            "writes fail while unplugged"
        );
        let mut buf = [0u8; 4];
        assert!(
            channel.read(&mut buf).is_err(),
            "reads fail while unplugged"
        );

        sim.set_link_dropped(false);
        assert!(
            channel.write_all(b"Q").is_ok(),
            "and recover when plugged back in"
        );
    }

    #[test]
    fn bytes_to_read_reports_what_is_waiting() {
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();
        assert_eq!(channel.bytes_to_read().expect("query succeeds"), 0);

        channel.write_all(b"Q").expect("write succeeds");

        let waiting = channel.bytes_to_read().expect("query succeeds") as usize;
        assert_eq!(waiting, EcuSimulator::DEFAULT_SIGNATURE.len() + 1);
    }

    #[test]
    fn cloned_channels_share_one_simulator() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut first = sim.channel();
        let mut second = first.try_clone().expect("clone succeeds");

        first
            .write_all(&[b'M', 0, 0, 0, 0, 1, 0, 0x7F])
            .expect("write succeeds");
        second
            .write_all(&[b'p', 0, 0, 0, 0, 1, 0])
            .expect("write succeeds");

        let mut buf = [0u8; 1];
        second.read(&mut buf).expect("read succeeds");
        assert_eq!(buf[0], 0x7F, "the clone sees the first channel's write");
    }
}
