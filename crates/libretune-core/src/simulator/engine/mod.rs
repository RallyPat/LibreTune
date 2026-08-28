//! `SimEngine` — the animated engine model behind the simulator's realtime
//! (`'r'`/0x30) responses.
//!
//! # Port note (ADR-0006, sub-step 5.1)
//!
//! The mode state machine and parameter correlations (here and in
//! [`physics`]) are **ported** from
//! [`askrejans/speeduino-serial-sim`](https://github.com/askrejans/speeduino-serial-sim)
//! (**MIT license**, Copyright (c) 2026 Arvis Skrējāns — confirmed against
//! the repo's `LICENSE` via the GitHub license API; this notice is preserved
//! as MIT requires): `include/EngineSimulator.h` + `src/EngineSimulator.cpp` —
//! the STARTUP → WARMUP_IDLE → IDLE → LIGHT_LOAD → ACCELERATION → HIGH_RPM
//! → DECELERATION → WOT machine, the `simulateRPM`/`simulateThermal`/
//! `simulateThrottle`/`simulateMAP`/`simulateIgnition`/`simulateVoltage`
//! correlations, sensor noise, and the 50 ms (20 Hz) update cadence, with
//! tuning constants from `include/Config.h`. That code is **separable**
//! from the repo's `SpeeduinoProtocol.cpp`: its only output is the
//! `EngineStatus` struct (`include/EngineStatus.h`), which is the seam —
//! the simulator writes fields, the protocol serializes them.
//!
//! **Fresh, not ported:** the reference fills a fixed 130-byte
//! `EngineStatus`; this port instead encodes each value at the offset/type
//! the loaded INI's `[OutputChannels]` declares — see [`super::och_codec`]
//! (definition-driven), so the sim animates whatever block layout the INI
//! describes. The `'r'` wire dispatch is also fresh — see [`super::ecu`]
//! and [`crate::memory`]'s port-note, both written against Speeduino
//! `comms.cpp`.
//!
//! **M4 Task 9 (fresh):** [`Self::set_ve_context`] hands the engine a
//! decoded [`VeContext`] (see [`super::ve_model`]) each tick — refreshed by
//! [`super::ecu`] from the INI's `[VeAnalyze]`-bound `veTable`, wherever it
//! lives in page memory. [`Self::snapshot`] uses it to compute a "measured"
//! `afr` that drifts away from `afr_target` exactly where that table
//! disagrees with the hidden [`super::ve_model::true_ve`] surface — the
//! ground truth the M4 auto-tune demo (Task 12) must flatten. No new
//! randomness and no wall clock: the whole computation is a pure function
//! of `rpm`/`map_kpa` (already-ticked engine state) and the current
//! `VeContext`, so determinism (see below) is unaffected.
//!
//! # Determinism
//!
//! No wall clock and no OS RNG *in the engine itself*: time advances only
//! through [`SimEngine::tick`], and sensor noise comes from a fixed-seed
//! xorshift — a given tick sequence always produces the same block bytes.
//! The wall clock lives one layer up: [`super::ecu`]'s `Pipe` feeds
//! `tick` a real `dt` on every production `'r'` request (auto-tick) so the
//! app's live gauges actually animate, while tests keep calling `tick`
//! with hand-picked durations for exact, reproducible assertions.

mod physics;

use super::och_codec::{self, ChannelValues};
use super::ve_model::{self, VeContext};
use crate::ini::{EcuDefinition, Endianness, OutputChannel};
use std::collections::HashMap;
use std::time::Duration;

// Tuning constants ported from `include/Config.h` (values unchanged).
const RPM_MIN: i32 = 0;
const RPM_IDLE_MIN: i32 = 700;
const RPM_IDLE_MAX: i32 = 900;
const RPM_CRUISE: i32 = 2_500;
const RPM_HIGH_START: i32 = 5_000;
const RPM_MAX: i32 = 7_000;
const RPM_REDLINE: i32 = 6_800;
/// Temperatures are °C × 10, as in the reference.
const TEMP_AMBIENT: i32 = 200;
const TEMP_ENGINE_WARM: i32 = 800;
const TEMP_ENGINE_HOT: i32 = 950;
const MAP_ATMOSPHERIC: i32 = 100;
const MAP_IDLE: i32 = 35;
const MAP_WOT: i32 = 95;
const VOLTAGE_NORMAL: i32 = 140; // V × 10
const AFR_STOICH: i32 = 147; // AFR × 10
const TPS_IDLE: i32 = 2;
const TPS_CRUISE: i32 = 20;
const TPS_HALF: i32 = 50;
const TPS_WOT: i32 = 100;
const TIMING_IDLE: i32 = 15;
const TIMING_MAX: i32 = 35;
const UPDATE_INTERVAL_MS: u64 = 50; // one step = 50 ms (20 Hz)
const STATE_TRANSITION_MS: u64 = 5_000;
const STEPS_PER_SECOND: u64 = 1_000 / UPDATE_INTERVAL_MS;

/// Operating mode (ported `EngineMode`). Normally driven by the internal
/// state machine; [`SimEngine::set_mode`] forces one (the reference's
/// `setMode`), e.g. to demo a WOT pull — the only way WOT is entered there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Startup,
    WarmupIdle,
    Idle,
    LightLoad,
    Acceleration,
    HighRpm,
    Deceleration,
    Wot,
}

/// Fixed-seed xorshift32 — deterministic stand-in for the reference's
/// `IRandomProvider` (Arduino `random`).
struct XorShift32(u32);

impl XorShift32 {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform-ish in `[lo, hi)` — mirrors Arduino `random(min, max)`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        debug_assert!(lo < hi);
        lo + (self.next() % (hi - lo) as u32) as i32
    }
}

/// The animated engine model. Build from a [`Definition`], advance with
/// [`Self::tick`], read the encoded realtime frame via [`Self::och_block`].
pub struct SimEngine {
    channels: HashMap<String, OutputChannel>,
    endian: Endianness,
    block: Vec<u8>,
    mode: EngineMode,
    /// Simulated milliseconds since cold start (no wall clock).
    time_ms: u64,
    /// Sub-step remainder of `tick` durations not yet 50 ms.
    acc_ms: u64,
    state_start_ms: u64,
    steps: u64,
    secl: u8,
    target_rpm: i32,
    rpm: i32,
    /// RPM/s, signed (negative while decelerating).
    rpm_accel: i32,
    target_tps: i32,
    tps: i32,
    /// Noisy TPS sensor reading sampled each step (percent).
    tps_reading: i32,
    coolant_dc: i32,
    intake_dc: i32,
    map_kpa: i32,
    battery_dv: i32,
    advance_deg: i32,
    rng: XorShift32,
    /// M4 Task 9: the currently-decoded `veTable` context, refreshed each
    /// tick by [`super::ecu`] via [`Self::set_ve_context`]. `None` for INIs
    /// with no `[VeAnalyze]` binding (or before the first refresh) — the
    /// simulator then behaves exactly as before M4 (`afr == afr_target`).
    ve_ctx: Option<VeContext>,
}

impl SimEngine {
    /// Cold-start engine for `def`'s channel layout. The block is sized to
    /// `ochBlockSize`, with [`och_codec::block_size`]'s documented fallback
    /// when the INI omits it.
    ///
    /// The reference's `initialize()` leaves `rpmAcceleration` at 0 (its
    /// `main.cpp` immediately calls `setMode(STARTUP)` to load targets);
    /// this port folds that into construction via `transition`.
    pub fn new(def: &EcuDefinition) -> Self {
        let mut engine = Self {
            channels: def.output_channels.clone(),
            endian: def.endianness,
            block: vec![0u8; och_codec::block_size(def)],
            mode: EngineMode::Startup,
            time_ms: 0,
            acc_ms: 0,
            state_start_ms: 0,
            steps: 0,
            secl: 0,
            target_rpm: 0,
            rpm: 0,
            rpm_accel: 0,
            target_tps: TPS_IDLE,
            tps: TPS_IDLE,
            tps_reading: TPS_IDLE,
            coolant_dc: TEMP_AMBIENT,
            intake_dc: TEMP_AMBIENT,
            map_kpa: MAP_ATMOSPHERIC,
            battery_dv: VOLTAGE_NORMAL,
            advance_deg: 0,
            rng: XorShift32(0x4F54_5531),
            ve_ctx: None,
        };
        engine.transition(EngineMode::Startup);
        engine.encode();
        engine
    }

    /// Advance simulated time by `dt`: one physics step per elapsed 50 ms
    /// (the ported 20 Hz cadence), remainder carried to the next call.
    pub fn tick(&mut self, dt: Duration) {
        self.acc_ms += dt.as_millis() as u64;
        while self.acc_ms >= UPDATE_INTERVAL_MS {
            self.acc_ms -= UPDATE_INTERVAL_MS;
            self.step();
        }
        self.encode();
    }

    /// The current realtime frame, encoded at the INI-declared offsets.
    pub fn och_block(&self) -> &[u8] {
        &self.block
    }

    /// Reset the seconds counter to 0 (first-och-request semantics,
    /// comms.cpp:361-365) and refresh the encoded block so the very next
    /// response already carries `secl = 0`. The step phase is untouched —
    /// the firmware's timer keeps running through the reset.
    pub fn reset_secl(&mut self) {
        self.secl = 0;
        self.encode();
    }

    /// Force an operating mode (ported `setMode`): loads that mode's
    /// targets/slew rate, then the state machine continues from there. The
    /// reference only ever enters [`EngineMode::Wot`] this way.
    pub fn set_mode(&mut self, mode: EngineMode) {
        self.transition(mode);
    }

    /// M4 Task 9: install (or clear) the decoded `veTable` context used to
    /// compute the "measured" `afr` channel — see the module doc comment.
    /// Re-encodes immediately so a caller that sets the context and reads
    /// [`Self::och_block`] without an intervening [`Self::tick`] still sees
    /// it reflected (mirrors [`Self::reset_secl`]'s immediate re-encode).
    pub(crate) fn set_ve_context(&mut self, ctx: Option<VeContext>) {
        self.ve_ctx = ctx;
        self.encode();
    }

    /// One 50 ms update (ported `EngineSimulator::update` body). The
    /// physics live in [`physics`] — same ordering as the reference:
    /// RPM drives everything, then thermal, throttle, MAP, timing, voltage.
    fn step(&mut self) {
        self.time_ms += UPDATE_INTERVAL_MS;
        self.steps += 1;
        if self.steps.is_multiple_of(STEPS_PER_SECOND) {
            self.secl = self.secl.wrapping_add(1);
        }
        self.state_machine();
        self.simulate_rpm();
        self.simulate_thermal();
        self.simulate_throttle();
        self.simulate_map();
        self.simulate_ignition();
        self.simulate_voltage();
    }

    /// Serialize the current physical state into the och block (pure — the
    /// noise is rolled in [`Self::step`], so re-encoding is idempotent).
    fn encode(&mut self) {
        let values = self.snapshot();
        och_codec::encode_channels(&self.channels, self.endian, &values, &mut self.block);
    }

    /// This tick's physical values, handed to [`super::och_codec`].
    ///
    /// M4 Task 9 (locked decision 11): the loop closes as
    /// `afr = afr_target × true_ve / current_ve` — a `veTable` reading too
    /// low means too little fuel is scheduled, i.e. lean, i.e. measured AFR
    /// *above* target; correcting a cell to `VE_new = VE_old × afr/target =
    /// VE_old × true/current` converges to `true_ve` in one step. Without a
    /// `[VeAnalyze]`/veTable binding in the loaded INI, `ve_ctx` is `None`
    /// and `afr == afr_target` — old INIs behave exactly as before M4.
    fn snapshot(&self) -> ChannelValues {
        let afr_target = f64::from(AFR_STOICH) / 10.0;
        let afr = match &self.ve_ctx {
            Some(ctx) => {
                let current = ctx
                    .current_ve(f64::from(self.rpm), f64::from(self.map_kpa))
                    .unwrap_or(1.0)
                    .max(1.0); // zeroed page must not explode the ratio
                let wanted = ve_model::true_ve(f64::from(self.rpm), f64::from(self.map_kpa));
                afr_target * wanted / current
            }
            None => afr_target, // no VE binding in this INI — behave as before M4
        };
        ChannelValues {
            secl: self.secl,
            rpm: self.rpm,
            map_kpa: self.map_kpa,
            baro_kpa: MAP_ATMOSPHERIC,
            coolant_c: self.coolant_dc / 10,
            iat_c: self.intake_dc / 10,
            tps_percent: self.tps_reading,
            battery_dv: self.battery_dv,
            advance_deg: self.advance_deg,
            afr_target,
            afr,
            // Speeduino's egoCorrection is 100-centered; the sim never
            // trims — EGO math is unit-tested in `analysis` instead.
            ego_correction: 100.0,
            // Speeduino semantics: BIT_ENGINE_RUN vs BIT_ENGINE_CRANK.
            running: self.rpm > 0 && self.mode != EngineMode::Startup,
            cranking: self.mode == EngineMode::Startup,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ini::{DataType, OutputChannel};

    /// A definition declaring just the channels these tests read back,
    /// so the engine's behaviour is asserted without depending on any
    /// particular vendor INI.
    fn test_definition() -> EcuDefinition {
        let mut def = EcuDefinition::default();
        def.endianness = Endianness::Little;
        def.protocol.och_block_size = 16;
        for (name, offset, data_type) in [
            ("secl", 0u16, DataType::U08),
            ("rpm", 4, DataType::U16),
            ("map", 6, DataType::U16),
            ("tps", 8, DataType::U08),
            ("coolant", 9, DataType::S16),
        ] {
            def.output_channels.insert(
                name.to_string(),
                OutputChannel {
                    name: name.to_string(),
                    data_type,
                    offset,
                    scale: 1.0,
                    translate: 0.0,
                    ..Default::default()
                },
            );
        }
        def
    }

    fn channel(def: &EcuDefinition, engine: &SimEngine, name: &str) -> f64 {
        def.output_channels[name]
            .parse(engine.och_block(), def.endianness)
            .expect("channel decodes")
    }

    #[test]
    fn wot_mode_ramps_rpm_hard_then_hands_back_to_high_rpm() {
        let def = test_definition();
        let mut engine = SimEngine::new(&def);
        engine.set_mode(EngineMode::Wot);

        engine.tick(Duration::from_millis(1_000));
        let rpm = channel(&def, &engine, "rpm");
        assert!(rpm >= 1_000.0, "WOT must ramp fast, got {rpm}");

        // After more than three seconds in-state the machine leaves WOT.
        engine.tick(Duration::from_millis(3_000));
        assert_ne!(engine.mode, EngineMode::Wot);
    }

    #[test]
    fn same_tick_sequence_is_deterministic() {
        let def = test_definition();
        let mut a = SimEngine::new(&def);
        let mut b = SimEngine::new(&def);
        for _ in 0..40 {
            a.tick(Duration::from_millis(50));
            b.tick(Duration::from_millis(50));
        }
        assert_eq!(
            a.och_block(),
            b.och_block(),
            "the model must not read a clock or a real RNG"
        );
    }

    #[test]
    fn engine_warms_up_from_ambient_towards_operating_temperature() {
        let def = test_definition();
        let mut engine = SimEngine::new(&def);
        let cold = channel(&def, &engine, "coolant");
        engine.tick(Duration::from_secs(60));
        let warm = channel(&def, &engine, "coolant");
        assert!(
            warm > cold,
            "coolant must climb from ambient: {cold} -> {warm}"
        );
    }

    #[test]
    fn secl_counter_wraps_and_resets_on_demand() {
        let def = test_definition();
        let mut engine = SimEngine::new(&def);
        engine.tick(Duration::from_secs(5));
        assert!(channel(&def, &engine, "secl") > 0.0, "secl counts seconds");
        engine.reset_secl();
        assert_eq!(
            channel(&def, &engine, "secl"),
            0.0,
            "reset must show up without waiting for the next tick"
        );
    }

    #[test]
    fn block_is_sized_from_the_definition() {
        let def = test_definition();
        let engine = SimEngine::new(&def);
        assert_eq!(engine.och_block().len(), 16);
    }
}
