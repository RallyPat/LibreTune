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
use crate::ini::{EcuDefinition, Endianness};
use crate::protocol::{CommunicationChannel, EnvelopeOrder, Packet};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Speeduino's `SERIAL_RC_OK`.
const RC_OK: u8 = 0x00;
/// Speeduino's `SERIAL_RC_BURN_OK`.
const RC_BURN_OK: u8 = 0x04;
/// Speeduino's `SERIAL_RC_CRC_ERR` — frame arrived, checksum disagreed.
const RC_CRC_ERR: u8 = 0x83;
/// Speeduino's `SERIAL_RC_RANGE_ERR` — the addressed page/offset does not exist.
const RC_RANGE_ERR: u8 = 0x84;
/// Speeduino's `SERIAL_RC_UKWN_ERR`. Distinct from [`RC_OK`], which answering
/// an unknown command with `0x00` would be indistinguishable from.
const RC_UNKNOWN_ERR: u8 = 0x89;
/// How long an incomplete frame may sit *without progress* before it is
/// discarded, mirroring Speeduino's `SERIAL_TIMEOUT`. Without it a client
/// that abandons a frame mid-write desynchronises every later command,
/// because the leftover bytes stay glued to the front of the next one.
///
/// Every write that adds bytes restarts the clock, so this is an idle
/// timeout rather than a budget for the whole frame: a large frame arriving
/// in steady chunks is not a stalled one, however long it takes in total.
const FRAME_TIMEOUT: Duration = Duration::from_millis(400);
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

    /// A [`PageMemory::new`] carrying a plausible starting tune.
    ///
    /// Blank flash is not a tune: its veTable has degenerate axes and 0 %
    /// cells, which makes the AFR the model reports meaningless and gives
    /// AutoTune nothing it will accept. Seeding both RAM and flash means a
    /// reboot lands back on the same starting tune, exactly as real
    /// hardware would.
    fn seeded(def: &EcuDefinition) -> Self {
        let mut memory = Self::new(def);
        if ve_model::seed_ve_table(def, &mut memory.ram) {
            for page in 0..memory.ram.page_count() {
                if let Some(data) = memory.ram.get_page(page) {
                    let copy = data.to_vec();
                    memory.flash.load_page(page, copy);
                }
            }
        }
        memory
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

    /// Whether the write landed. A refused write must not be acknowledged as
    /// success, or a client silently believes memory it never changed.
    fn write(&mut self, page: u8, offset: u16, value: &[u8]) -> bool {
        self.ram.write_bytes(page, offset, value)
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
    /// The command this ECU answers with its signature. Speeduino asks with
    /// `Q` and uses `S` for the version; the rusEFI family asks with `S`.
    /// Taken from the INI so the simulator matches whichever it was built
    /// from.
    query_cmd: u8,
    /// Whether `%2o`/`%2c`/`%2i` command parameters are little-endian. The
    /// rusEFI family declares `endianness = little` and sends them that way;
    /// Speeduino and MS2/MS3 send them big-endian.
    cmd_le: bool,
    /// Whether realtime requests advance the engine off the wall clock.
    /// Permanently disabled once a caller drives time explicitly, so tests
    /// never race the clock.
    auto_tick: bool,
    last_auto_tick: Option<Instant>,
    /// Wire page identifiers, indexed by logical page.
    ///
    /// An INI's `pageIdentifier` entries are literal byte strings, not
    /// numbers — the bundled demo declares `\x00\x00`, `\x00\x01`,
    /// `\x00\x02`. The client turns each into a `u16` and sends it as a
    /// `%2i` parameter, so the simulator has to reverse the same mapping
    /// rather than assume the identifier equals the page index.
    page_ids: Vec<u16>,
    /// Set once a CRC-framed request has been decoded successfully.
    ///
    /// From then on every request is a frame. Without this latch a partially
    /// arrived frame whose first payload byte happens to be a command letter
    /// (`0x41` = `'A'` is also the high byte of length 0x4100) is consumed as
    /// a bare command, and the rest of the frame corrupts the next one.
    enveloped_session: bool,
    /// When the current incomplete frame last made progress. Reset by every
    /// write that adds bytes, so [`FRAME_TIMEOUT`] measures silence.
    partial_since: Option<Instant>,
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
                .map(PageMemory::seeded)
                .unwrap_or_else(|| PageMemory::new(&EcuDefinition::default())),
            och_block: engine
                .as_ref()
                .map(|e| e.och_block().to_vec())
                .unwrap_or_default(),
            first_och_done: false,
            engine,
            definition: def.cloned(),
            // Only a starting hint: [`take_frame`] re-latches this from
            // whichever order actually validates on the wire, because the
            // client itself probes both during the handshake.
            envelope: def
                .map(|d| match d.endianness {
                    Endianness::Little => EnvelopeOrder::LittleEndian,
                    Endianness::Big => EnvelopeOrder::BigEndian,
                })
                .unwrap_or(EnvelopeOrder::BigEndian),
            signature: def
                .map(|d| d.signature.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| EcuSimulator::DEFAULT_SIGNATURE.to_string()),
            cmd_le: def
                .map(|d| d.endianness == Endianness::Little)
                .unwrap_or(false),
            query_cmd: def
                .and_then(|d| d.protocol.query_command.bytes().next())
                .unwrap_or(b'Q'),
            version: def
                .map(|d| d.version_info.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| EcuSimulator::DEFAULT_VERSION.to_string()),
            auto_tick: true,
            last_auto_tick: None,
            page_ids: def.map(page_identifiers).unwrap_or_default(),
            enveloped_session: false,
            partial_since: None,
        }
    }

    /// Logical page for a wire identifier, or `None` when the dialect
    /// declares identifiers and this is not one of them.
    fn resolve_page(&self, id: u16) -> Option<u8> {
        if let Some(index) = self.page_ids.iter().position(|known| *known == id) {
            return u8::try_from(index).ok();
        }
        // An INI that declares no identifiers addresses pages by index.
        if self.page_ids.is_empty() {
            return u8::try_from(id).ok();
        }
        None
    }

    /// Drop an incomplete frame that stopped arriving, so the next command
    /// starts from a clean buffer.
    fn expire_partial_frame(&mut self) {
        let stale = self
            .partial_since
            .is_some_and(|since| since.elapsed() >= FRAME_TIMEOUT);
        if stale {
            self.cmd_buf.clear();
            self.partial_since = None;
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

/// The wire identifier of each page, decoded exactly the way the client
/// encodes it in [`crate::protocol::Connection::get_page_identifier`]: the
/// INI's raw identifier bytes read as a little-endian `u16`.
fn page_identifiers(def: &EcuDefinition) -> Vec<u16> {
    def.protocol
        .page_identifiers
        .iter()
        .enumerate()
        .map(|(index, bytes)| match bytes.len() {
            0 => index as u16,
            1 => u16::from(bytes[0]),
            _ => u16::from_le_bytes([bytes[0], bytes[1]]),
        })
        .collect()
}

/// Whether `b` starts a plain-protocol command this ECU understands.
///
/// Only ever consulted for a buffer that failed to decode as a CRC frame,
/// and only before the session has latched into framed mode — on its own it
/// cannot tell a command letter from a length byte that happens to share its
/// value.
fn is_command_byte(b: u8) -> bool {
    matches!(
        b,
        b'Q' | b'S' | b'A' | b'p' | b'M' | b'b' | b'r' | b'R' | b'C' | b'B' | b'k' | b'O'
    )
}

/// Total byte length of the plain-protocol command at the front of `buf`, or
/// `None` when too few bytes have arrived to even tell.
fn plain_command_len(buf: &VecDeque<u8>, cmd_le: bool) -> Option<usize> {
    match *buf.front()? {
        // cmd + %2i + %2o + %2c, and 'r' + canId + subcmd + %2o + %2c.
        b'p' | b'r' | b'R' | b'k' => Some(7),
        // 'O' + %2o + %2c
        b'O' => Some(5),
        // cmd + %2i
        b'b' | b'B' => Some(3),
        // cmd + %2i + %2o + %2c + %v
        b'M' | b'C' => {
            if buf.len() < 7 {
                return None;
            }
            let count = if cmd_le {
                u16::from_le_bytes([buf[5], buf[6]])
            } else {
                u16::from_be_bytes([buf[5], buf[6]])
            } as usize;
            Some(7 + count)
        }
        // 'Q' / 'S' / 'A' and anything unrecognized.
        _ => Some(1),
    }
}

/// Decode a two-byte command parameter at `at`, honouring the dialect's
/// command-parameter byte order.
fn param_u16(bytes: &[u8], at: usize, cmd_le: bool) -> Option<u16> {
    let pair = [*bytes.get(at)?, *bytes.get(at + 1)?];
    Some(if cmd_le {
        u16::from_le_bytes(pair)
    } else {
        u16::from_be_bytes(pair)
    })
}

/// `(page identifier, offset, count)` from a `p`/`M` command, or their
/// rusEFI-family spellings `R`/`C`.
///
/// The identifier is returned as it arrived; only [`Pipe::resolve_page`]
/// knows which logical page it names. Truncating it to a `u8` here is what
/// made every page but the first unreachable — the demo INI's page 1 is
/// identifier `0x0100`, not `1`.
fn parse_page_offset_count(bytes: &[u8], cmd_le: bool) -> Option<(u16, u16, u16)> {
    Some((
        param_u16(bytes, 1, cmd_le)?,
        param_u16(bytes, 3, cmd_le)?,
        param_u16(bytes, 5, cmd_le)?,
    ))
}

fn parse_page(bytes: &[u8], cmd_le: bool) -> Option<u16> {
    param_u16(bytes, 1, cmd_le)
}

/// `(offset, len)` from an `r` command: `['r', canId, 0x30, offset, len]`.
/// Byte 1 is the CAN id, discarded exactly as the firmware discards it.
fn parse_och_window(bytes: &[u8], cmd_le: bool) -> Option<(u16, u16)> {
    if *bytes.get(2)? != SUBCMD_OUTPUT_CHANNELS {
        return None;
    }
    Some((param_u16(bytes, 3, cmd_le)?, param_u16(bytes, 5, cmd_le)?))
}

/// `(offset, len)` from an `O%2o%2c` command — the realtime request the
/// rusEFI family's INIs declare via `ochGetCommand`.
fn parse_och_get(bytes: &[u8], cmd_le: bool) -> Option<(u16, u16)> {
    Some((param_u16(bytes, 1, cmd_le)?, param_u16(bytes, 3, cmd_le)?))
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
    // The plain framing has no status byte at all, so a refusal there is
    // simply silence — the same thing the firmware does.
    let status = |code: u8| -> Vec<u8> {
        if enveloped {
            vec![code]
        } else {
            Vec::new()
        }
    };
    let cmd_byte = cmd.first().copied().unwrap_or(0);
    // The INI's own query command wins: answering `S` with a version string
    // would fail every rusEFI-family handshake, which asks with `S`.
    if cmd_byte == pipe.query_cmd {
        return ok(pipe.signature.as_bytes().to_vec());
    }
    match cmd_byte {
        b'Q' => ok(pipe.signature.as_bytes().to_vec()),
        b'S' => ok(pipe.version.as_bytes().to_vec()),
        // Speeduino's 'A' is a whole-frame realtime request, and it is also
        // the default burst command when an INI declares none. Answering one
        // byte would starve any client that falls back to it.
        b'A' => {
            pipe.on_och_request();
            pipe.auto_tick();
            if pipe.och_block.is_empty() {
                ok(vec![pipe.secl])
            } else {
                ok(pipe.och_block.clone())
            }
        }
        b'p' | b'R' => match parse_page_offset_count(cmd, pipe.cmd_le)
            .and_then(|(id, offset, count)| Some((pipe.resolve_page(id)?, offset, count)))
        {
            Some((page, offset, count)) => ok(pipe.memory.read(page, offset, count)),
            None => status(RC_RANGE_ERR),
        },
        b'M' | b'C' => {
            let wrote = parse_page_offset_count(cmd, pipe.cmd_le)
                .and_then(|(id, offset, count)| Some((pipe.resolve_page(id)?, offset, count)))
                .is_some_and(|(page, offset, count)| {
                    let value = cmd.get(7..7 + count as usize).unwrap_or(&[]);
                    pipe.memory.write(page, offset, value)
                });
            if enveloped {
                vec![if wrote { RC_OK } else { RC_RANGE_ERR }]
            } else {
                Vec::new()
            }
        }
        b'b' | b'B' => {
            let burned = parse_page(cmd, pipe.cmd_le)
                .and_then(|id| pipe.resolve_page(id))
                .map(|page| {
                    pipe.memory.burn(page);
                })
                .is_some();
            if enveloped {
                vec![if burned { RC_BURN_OK } else { RC_RANGE_ERR }]
            } else {
                Vec::new()
            }
        }
        b'k' => match parse_page_offset_count(cmd, pipe.cmd_le)
            .and_then(|(id, offset, count)| Some((pipe.resolve_page(id)?, offset, count)))
        {
            Some((page, offset, count)) => {
                let mut hasher = crc32fast::Hasher::new();
                hasher.update(&pipe.memory.read(page, offset, count));
                ok(hasher.finalize().to_be_bytes().to_vec())
            }
            None => status(RC_RANGE_ERR),
        },
        b'O' => match parse_och_get(cmd, pipe.cmd_le) {
            Some((offset, len)) => {
                pipe.on_och_request();
                pipe.auto_tick();
                ok(och_window(&pipe.och_block, offset, len))
            }
            None => ok(Vec::new()),
        },
        b'r' => match parse_och_window(cmd, pipe.cmd_le) {
            Some((offset, len)) => {
                pipe.on_och_request();
                pipe.auto_tick();
                ok(och_window(&pipe.och_block, offset, len))
            }
            None => ok(Vec::new()),
        },
        _ => status(RC_UNKNOWN_ERR),
    }
}

/// What the front of the input buffer turned out to be.
enum Framed {
    /// A complete frame whose CRC checked out, in the order that validated.
    Enveloped {
        payload: Vec<u8>,
        order: EnvelopeOrder,
    },
    /// A complete frame whose CRC did not check out. Consumed and refused —
    /// never dispatched, or a corrupted write would mutate RAM and be
    /// acknowledged as success.
    BadCrc(EnvelopeOrder),
    /// A bare command in the plain framing.
    Plain(Vec<u8>),
    /// Not enough bytes yet to decide.
    NeedMore,
}

/// Take whatever is at the front of the input buffer.
///
/// A complete, CRC-valid envelope wins outright in either byte order, which
/// is the only discriminator that cannot be fooled: guessing from the first
/// byte cannot distinguish the command letter `A` from the high byte of
/// length `0x4100`, and guessing the order wrongly parses a length of 1 as
/// 256 and waits forever. Once a frame has validated, the session is framed
/// for good — a partially arrived frame is then never mistaken for a bare
/// command.
fn take_frame(pipe: &mut Pipe) -> Framed {
    let hint = pipe.envelope;
    let other = match hint {
        EnvelopeOrder::BigEndian => EnvelopeOrder::LittleEndian,
        EnvelopeOrder::LittleEndian => EnvelopeOrder::BigEndian,
    };
    let bytes = pipe.cmd_buf.make_contiguous();
    if bytes.is_empty() {
        return Framed::NeedMore;
    }

    for order in [hint, other] {
        let Ok(packet) = Packet::from_bytes_ordered(bytes, order, false) else {
            continue;
        };
        // A zero-length payload carries no command, and its CRC32 is zero —
        // so six zero bytes of line noise decode as a "valid" frame. Latching
        // the session on that would disable the plain protocol for good.
        if packet.payload.is_empty() {
            continue;
        }
        let total = packet.encoded_size();
        let _ = pipe.cmd_buf.drain(..total);
        return Framed::Enveloped {
            payload: packet.payload,
            order,
        };
    }

    let first = pipe.cmd_buf[0];
    if !pipe.enveloped_session && is_command_byte(first) {
        let cmd_le = pipe.cmd_le;
        match plain_command_len(&pipe.cmd_buf, cmd_le) {
            // Before the session has latched, a command letter is only
            // trusted when it accounts for every buffered byte. A frame
            // whose length field starts with one — big-endian 0x4100 leads
            // with ASCII 'A' — always leaves a tail behind, and consuming
            // just the letter would poison the rest of that frame.
            Some(len) if len == pipe.cmd_buf.len() => {
                return Framed::Plain(pipe.cmd_buf.drain(..len).collect());
            }
            _ => return Framed::NeedMore,
        }
    }

    // Not a plain command, so it can only be a frame. A frame that is
    // complete in *either* order must have failed its CRC above, and has to
    // be consumed and refused — leaving it buffered would stall the link
    // until the timeout silently dropped it.
    let bytes = pipe.cmd_buf.make_contiguous();
    if bytes.len() >= 2 {
        let complete = [hint, other].into_iter().filter_map(|order| {
            let total = 2 + order.read_u16(&bytes[..2]) as usize + 4;
            (bytes.len() >= total).then_some((total, order))
        });
        // The shortest complete reading is the frame that actually ended
        // here; a longer one would still be mid-flight.
        if let Some((total, order)) = complete.min_by_key(|(total, _)| *total) {
            let _ = pipe.cmd_buf.drain(..total);
            return Framed::BadCrc(order);
        }
    }
    Framed::NeedMore
}

/// Drain buffered commands, dispatch them, and queue their responses.
fn process(pipe: &Arc<Mutex<Pipe>>) {
    let mut p = pipe.lock().unwrap_or_else(|e| e.into_inner());
    if p.dropped {
        return;
    }
    // Keep draining: a client that pipelines several frames before reading
    // must get an answer to every one of them, not just the first.
    while !p.cmd_buf.is_empty() {
        match take_frame(&mut p) {
            Framed::Enveloped { payload, order } => {
                p.envelope = order;
                p.enveloped_session = true;
                let response = respond(&payload, &mut p, true);
                let framed = Packet::new(response).to_bytes_ordered(order);
                p.rsp_buf.extend(framed);
            }
            Framed::BadCrc(order) => {
                let framed = Packet::new(vec![RC_CRC_ERR]).to_bytes_ordered(order);
                p.rsp_buf.extend(framed);
            }
            Framed::Plain(cmd) => {
                let out = respond(&cmd, &mut p, false);
                p.rsp_buf.extend(out);
            }
            Framed::NeedMore => {
                if p.partial_since.is_none() {
                    p.partial_since = Some(Instant::now());
                }
                return;
            }
        }
        p.partial_since = None;
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
        if p.rsp_buf.is_empty() && !buf.is_empty() {
            // `Ok(0)` into a non-empty buffer means end of stream, and
            // `read_exact_timeout` turns that into an immediate timeout. The
            // link is fine — there is just nothing queued yet.
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "no simulated response queued",
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
            // Before the new bytes land: an abandoned frame still sitting in
            // the buffer would otherwise stay glued to the front of this one.
            p.expire_partial_frame();
            if !buf.is_empty() {
                // This write is progress, so the idle clock restarts.
                p.partial_since = None;
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

    /// Back-date the incomplete-frame timer so the next request sees it as
    /// expired, instead of a test sleeping out the real timeout.
    #[cfg(test)]
    pub(crate) fn expire_frame_buffer_for_test(&self) {
        let mut pipe = self.pipe.lock().unwrap_or_else(|e| e.into_inner());
        pipe.partial_since = Instant::now().checked_sub(FRAME_TIMEOUT);
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
        def.page_sizes = vec![512, 16];
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

    /// Read up to `n` bytes, treating "nothing queued" as an empty read.
    ///
    /// The channel reports an empty response buffer as `WouldBlock`, not
    /// `Ok(0)` — `Ok(0)` is end of stream, and the connection's read loop
    /// turns that into an immediate timeout.
    fn read_n(channel: &mut SimulatorChannel, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        match channel.read(&mut buf) {
            Ok(got) => {
                buf.truncate(got);
                buf
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Vec::new(),
            Err(error) => panic!("read failed: {error}"),
        }
    }

    #[test]
    fn six_zero_bytes_do_not_latch_the_session_into_framed_mode() {
        // A zero-length payload's CRC32 is zero, so `00 00 00 00 00 00`
        // decodes as a "valid" frame. Latching on it disabled the plain
        // protocol for the rest of the session, and a legacy client never
        // got its signature back.
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();

        channel
            .write_all(&[0, 0, 0, 0, 0, 0])
            .expect("noise is accepted");
        let _ = read_n(&mut channel, 64);

        channel.write_all(b"Q").expect("write succeeds");
        assert_eq!(
            read_n(&mut channel, 64),
            EcuSimulator::DEFAULT_SIGNATURE.as_bytes(),
            "line noise must not disable the plain protocol"
        );
    }

    #[test]
    fn a_bad_crc_frame_in_the_other_byte_order_is_still_consumed_and_refused() {
        // Frames are validated in both orders, but completeness used to be
        // measured only in the hinted one: a complete-but-corrupt frame in
        // the other order was neither answered nor consumed, so it stalled
        // the link until the idle timeout silently dropped it.
        let mut def = definition();
        def.endianness = Endianness::Little; // hint = little-endian
        let sim = EcuSimulator::from_definition(&def);
        let mut channel = sim.channel();

        // A complete big-endian frame (length 1, payload `Q`) with a
        // deliberately wrong CRC. Little-endian reads its length as 256.
        channel
            .write_all(&[0x00, 0x01, b'Q', 0xFF, 0xFF, 0xFF, 0xFF])
            .expect("frame accepted");

        let response = read_n(&mut channel, 64);
        assert!(
            !response.is_empty(),
            "a complete corrupt frame must be answered, not left to rot"
        );
        let packet = Packet::from_bytes_ordered(&response, EnvelopeOrder::BigEndian, false)
            .expect("the refusal is framed");
        assert_eq!(packet.payload, vec![RC_CRC_ERR]);
    }

    #[test]
    fn a_frame_arriving_in_steady_chunks_is_not_timed_out() {
        // The idle timer must measure silence, not total assembly time. Real
        // gaps are used deliberately: back-dating the timer would trip the
        // fixed code too, and prove nothing about which quantity is measured.
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        // Each gap is comfortably under FRAME_TIMEOUT, but they sum past it.
        let gap = FRAME_TIMEOUT * 3 / 4;
        let frame = Packet::new(b"Q".to_vec()).to_bytes();
        let chunks: Vec<&[u8]> = frame.chunks(3).collect();
        assert!(
            gap * (chunks.len() as u32 - 1) > FRAME_TIMEOUT,
            "the gaps must add up past the timeout for this to prove anything"
        );
        for (index, chunk) in chunks.iter().enumerate() {
            if index > 0 {
                std::thread::sleep(gap);
            }
            channel.write_all(chunk).expect("chunk accepted");
        }

        let packet = Packet::from_bytes(&read_n(&mut channel, 128))
            .expect("the reassembled frame is answered");
        assert_eq!(
            &packet.payload[1..],
            EcuSimulator::DEFAULT_SIGNATURE.as_bytes()
        );
    }

    #[test]
    fn a_command_letter_with_a_tail_behind_it_waits_instead_of_being_split() {
        // Before the session latches, `0x41` is both the command `A` and the
        // high byte of a big-endian length of 0x4100. Consuming just the
        // letter poisons the rest of the frame; the tail is the tell.
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        channel
            .write_all(&[b'A', 0x00, 0x11, 0x22, 0x33])
            .expect("chunk accepted");

        assert_eq!(
            read_n(&mut channel, 64),
            Vec::<u8>::new(),
            "a command letter followed by unexplained bytes must not dispatch"
        );
    }

    #[test]
    fn an_empty_channel_reports_not_ready_rather_than_end_of_stream() {
        // `Ok(0)` into a non-empty buffer is EOF by contract, and
        // `read_exact_timeout` turns EOF into an immediate timeout instead of
        // polling again.
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();

        let mut buf = [0u8; 8];
        let error = channel.read(&mut buf).expect_err("nothing is queued yet");

        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn an_unknown_command_is_refused_with_a_code_that_is_not_success() {
        // Answering `vec![0]` made "I do not know this command" identical to
        // `RC_OK`, so a client saw every typo as a successful no-op.
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();

        let request = Packet::new(b"\x01".to_vec()).to_bytes();
        channel.write_all(&request).expect("write succeeds");

        let response = read_n(&mut channel, 64);
        let packet = Packet::from_bytes(&response).expect("a frame comes back");
        assert_eq!(packet.payload, vec![RC_UNKNOWN_ERR]);
        assert_ne!(packet.payload[0], RC_OK);
    }

    #[test]
    fn a_write_outside_the_page_is_refused_instead_of_acknowledged() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        // Page 1 is 16 bytes; ask to write 32 at offset 0.
        let count: u16 = 32;
        let mut payload = vec![b'C'];
        payload.extend(1u16.to_le_bytes());
        payload.extend(0u16.to_le_bytes());
        payload.extend(count.to_le_bytes());
        payload.extend(std::iter::repeat_n(0x11u8, count as usize));
        channel
            .write_all(&Packet::new(payload).to_bytes())
            .expect("write succeeds");

        let response = read_n(&mut channel, 64);
        let packet = Packet::from_bytes(&response).expect("a frame comes back");
        assert_eq!(
            packet.payload,
            vec![RC_RANGE_ERR],
            "a refused write must not report success"
        );
    }

    #[test]
    fn an_abandoned_partial_frame_does_not_poison_the_next_command() {
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        // Latch into framed mode, then start a frame and walk away.
        channel
            .write_all(&Packet::new(b"Q".to_vec()).to_bytes())
            .expect("handshake accepted");
        let _ = read_n(&mut channel, 128);
        channel
            .write_all(&[0x00, 0x20, 0x51])
            .expect("stub accepted");
        assert_eq!(read_n(&mut channel, 16), Vec::<u8>::new());

        sim.expire_frame_buffer_for_test();

        channel
            .write_all(&Packet::new(b"Q".to_vec()).to_bytes())
            .expect("second handshake accepted");
        let packet =
            Packet::from_bytes(&read_n(&mut channel, 128)).expect("the next frame is answered");
        assert_eq!(
            &packet.payload[1..],
            EcuSimulator::DEFAULT_SIGNATURE.as_bytes(),
            "leftover bytes must not be glued onto the next frame"
        );
    }

    #[test]
    fn answers_the_signature_query_on_the_plain_protocol() {
        let sim = EcuSimulator::new();
        let mut channel = sim.channel();

        channel.write_all(b"Q").expect("write succeeds");

        let response = read_n(&mut channel, 64);
        assert_eq!(response, EcuSimulator::DEFAULT_SIGNATURE.as_bytes());
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
        assert_eq!(response, b"epicEFI 1.2.3");
    }

    #[test]
    fn page_writes_are_readable_back_at_the_same_offset() {
        // Page ids are two-byte command parameters, so they follow the
        // definition's endianness — little here, hence [page, 0].
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        // 'M': page 1, offset 2, count 3, then the value bytes.
        let mut write = vec![b'M', 1, 0, 2, 0, 3, 0];
        write.extend([0xAA, 0xBB, 0xCC]);
        channel.write_all(&write).expect("write succeeds");
        assert_eq!(
            read_n(&mut channel, 8),
            Vec::<u8>::new(),
            "writes are unacknowledged"
        );

        channel
            .write_all(&[b'p', 1, 0, 2, 0, 3, 0])
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
            &packet.payload[1..],
            EcuSimulator::DEFAULT_SIGNATURE.as_bytes(),
            "the signature follows the return code"
        );
    }

    #[test]
    fn an_enveloped_payload_over_255_bytes_is_still_read_as_a_frame() {
        // The length field's high byte is only zero while payloads stay
        // small. A page-write chunk is thousands of bytes, so telling frames
        // apart by that byte misreads every real write as a command.
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();
        let count: u16 = 300;
        let mut payload = vec![b'M', 0, 0];
        payload.extend(0u16.to_le_bytes());
        payload.extend(count.to_le_bytes());
        payload.extend(std::iter::repeat_n(0xABu8, count as usize));
        assert!(
            payload.len() > 255,
            "the payload must cross the byte boundary"
        );

        channel
            .write_all(&Packet::new(payload).to_bytes())
            .expect("write succeeds");

        let response = read_n(&mut channel, 64);
        let packet = Packet::from_bytes(&response).expect("a frame comes back");
        assert_eq!(packet.payload, vec![RC_OK], "the write is acknowledged");
    }

    #[test]
    fn the_rusefi_command_spellings_reach_the_same_handlers() {
        // The rusEFI family declares R/C/B where Speeduino uses p/M/b; an
        // INI-driven client sends whichever its definition names.
        let sim = EcuSimulator::from_definition(&definition());
        let mut channel = sim.channel();

        channel
            .write_all(&[b'C', 1, 0, 0, 0, 1, 0, 0x5A])
            .expect("write succeeds");
        channel
            .write_all(&[b'R', 1, 0, 0, 0, 1, 0])
            .expect("read succeeds");

        assert_eq!(read_n(&mut channel, 1), vec![0x5A]);
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
        assert_eq!(waiting, EcuSimulator::DEFAULT_SIGNATURE.len());
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
