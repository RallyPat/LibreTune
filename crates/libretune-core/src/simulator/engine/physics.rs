//! The ported physics of [`SimEngine`]: mode transitions and per-step
//! parameter correlations. Ported from `askrejans/speeduino-serial-sim`
//! `src/EngineSimulator.cpp` (MIT, Copyright (c) 2026 Arvis Skrējāns) — see
//! the [`crate::engine`] module doc for the full port-note / license record.
//! Function-by-function mapping:
//! `state_machine` ← `updateStateMachine`, `transition` ←
//! `transitionToMode`, `simulate_*` ← `simulate*`, `interpolate` /
//! `map_value` ← the same-named helpers.

use super::{
    EngineMode, SimEngine, MAP_ATMOSPHERIC, MAP_IDLE, MAP_WOT, RPM_CRUISE, RPM_HIGH_START,
    RPM_IDLE_MAX, RPM_IDLE_MIN, RPM_MAX, RPM_MIN, RPM_REDLINE, STATE_TRANSITION_MS, TEMP_AMBIENT,
    TEMP_ENGINE_HOT, TEMP_ENGINE_WARM, TIMING_IDLE, TIMING_MAX, TPS_CRUISE, TPS_HALF, TPS_IDLE,
    TPS_WOT, UPDATE_INTERVAL_MS, VOLTAGE_NORMAL,
};

impl SimEngine {
    /// Ported `updateStateMachine` — mode transitions with their original
    /// dwell times, thresholds, and random branches.
    pub(super) fn state_machine(&mut self) {
        let in_state = self.time_ms - self.state_start_ms;
        match self.mode {
            EngineMode::Startup => {
                if in_state > 1_000 && self.rpm > RPM_IDLE_MIN / 2 {
                    self.transition(EngineMode::WarmupIdle);
                }
            }
            EngineMode::WarmupIdle => {
                if self.coolant_dc > 600 {
                    self.transition(EngineMode::Idle);
                }
            }
            EngineMode::Idle => {
                if in_state > STATE_TRANSITION_MS {
                    let r = self.rng.range(0, 100);
                    if r < 30 {
                        self.transition(EngineMode::LightLoad);
                    } else if r < 35 {
                        self.transition(EngineMode::Acceleration);
                    }
                }
            }
            EngineMode::LightLoad => {
                if in_state > STATE_TRANSITION_MS {
                    let r = self.rng.range(0, 100);
                    if r < 40 {
                        self.transition(EngineMode::Acceleration);
                    } else if r < 70 {
                        self.transition(EngineMode::Deceleration);
                    } else {
                        self.transition(EngineMode::Idle);
                    }
                }
            }
            EngineMode::Acceleration => {
                if self.rpm > RPM_HIGH_START {
                    self.transition(EngineMode::HighRpm);
                } else if in_state > 3_000 && self.rng.range(0, 100) < 30 {
                    self.transition(EngineMode::LightLoad);
                }
            }
            EngineMode::HighRpm => {
                if in_state > 2_000 {
                    self.transition(EngineMode::Deceleration);
                }
            }
            EngineMode::Deceleration => {
                if self.rpm < RPM_IDLE_MAX + 200 {
                    self.transition(EngineMode::Idle);
                }
            }
            EngineMode::Wot => {
                if in_state > 3_000 || self.rpm > RPM_REDLINE {
                    self.transition(EngineMode::HighRpm);
                }
            }
        }
    }

    /// Ported `transitionToMode` — per-mode targets and RPM slew rates.
    pub(super) fn transition(&mut self, mode: EngineMode) {
        self.mode = mode;
        self.state_start_ms = self.time_ms;
        let (target_rpm, target_tps, accel) = match mode {
            EngineMode::Startup => (RPM_IDLE_MIN + 200, TPS_IDLE + 5, 500),
            EngineMode::WarmupIdle => (RPM_IDLE_MIN + 150, TPS_IDLE + 3, 100),
            EngineMode::Idle => (RPM_IDLE_MIN + self.rng.range(-50, 50), TPS_IDLE, 50),
            EngineMode::LightLoad => (
                RPM_CRUISE + self.rng.range(-300, 300),
                TPS_CRUISE + self.rng.range(-5, 10),
                200,
            ),
            EngineMode::Acceleration => (
                RPM_HIGH_START + self.rng.range(-500, 500),
                TPS_HALF + self.rng.range(10, 40),
                1_000,
            ),
            EngineMode::HighRpm => (
                RPM_REDLINE - self.rng.range(100, 500),
                TPS_WOT - self.rng.range(0, 20),
                500,
            ),
            EngineMode::Deceleration => (RPM_IDLE_MAX + self.rng.range(0, 500), TPS_IDLE, -800),
            EngineMode::Wot => (RPM_REDLINE, TPS_WOT, 1_500),
        };
        self.target_rpm = target_rpm;
        self.target_tps = target_tps;
        self.rpm_accel = accel;
    }

    /// Ported `simulateRPM`: slew toward target at `rpm_accel` RPM/s, clamp
    /// to range, idle fluctuation noise. (The reference's unsigned math can
    /// wrap below 0 at idle; `i32` + clamp keeps the same behavior safely.)
    pub(super) fn simulate_rpm(&mut self) {
        let delta = self.rpm_accel * UPDATE_INTERVAL_MS as i32 / 1_000;
        if self.rpm < self.target_rpm {
            self.rpm += delta;
            if self.rpm > self.target_rpm {
                self.rpm = self.target_rpm;
            }
        } else if self.rpm > self.target_rpm {
            self.rpm += delta;
            if self.rpm < self.target_rpm {
                self.rpm = self.target_rpm;
            }
        }
        self.rpm = self.rpm.clamp(RPM_MIN, RPM_MAX);
        if matches!(self.mode, EngineMode::Idle | EngineMode::WarmupIdle) {
            self.rpm = (self.rpm + self.rng.range(-10, 10)).clamp(RPM_MIN, RPM_MAX);
        }
    }

    /// Ported `simulateThermal`: coolant slews toward a mode-dependent
    /// target with thermal inertia (monotone warm-up from cold); intake
    /// tracks engine-bay heat minus airflow cooling.
    pub(super) fn simulate_thermal(&mut self) {
        let target = match self.mode {
            EngineMode::Wot | EngineMode::HighRpm => TEMP_ENGINE_HOT,
            EngineMode::Idle | EngineMode::WarmupIdle => TEMP_ENGINE_WARM - 50,
            _ => TEMP_ENGINE_WARM,
        };
        self.coolant_dc = interpolate(self.coolant_dc, target, 5);

        let mut intake_target = TEMP_AMBIENT + (self.coolant_dc - TEMP_AMBIENT) / 4;
        if self.rpm > RPM_CRUISE {
            intake_target -= (self.rpm - RPM_CRUISE) / 50;
        }
        self.intake_dc = interpolate(self.intake_dc, intake_target, 10);
    }

    /// Ported `simulateThrottle`: smooth pedal response + ±1 % sensor noise.
    pub(super) fn simulate_throttle(&mut self) {
        self.tps = interpolate(self.tps, self.target_tps, 20);
        self.tps_reading = (self.tps + self.rng.range(-1, 2)).clamp(0, 100);
    }

    /// Ported `simulateMAP`: manifold pressure correlated with throttle and
    /// RPM (idle vacuum → near-atmospheric at WOT), ±2 kPa noise.
    pub(super) fn simulate_map(&mut self) {
        let mut base = if self.tps < 10 {
            MAP_IDLE + (self.rpm - RPM_IDLE_MIN) / 20
        } else if self.tps > 80 {
            MAP_WOT - (RPM_MAX - self.rpm) / 100
        } else {
            map_value(self.tps, 10, 80, MAP_IDLE + 10, MAP_WOT - 5)
        };
        if self.rpm > RPM_HIGH_START {
            base += (self.rpm - RPM_HIGH_START) / 100;
        }
        self.map_kpa = (base + self.rng.range(-2, 3)).clamp(0, MAP_ATMOSPHERIC);
    }

    /// Ported `simulateIgnition`/`calculateIgnitionAdvance`: more advance at
    /// higher RPM and light load, pulled back at high load.
    pub(super) fn simulate_ignition(&mut self) {
        let load = self.map_kpa * 100 / MAP_ATMOSPHERIC;
        let mut advance = TIMING_IDLE;
        if self.rpm > 1_000 {
            advance += (self.rpm - 1_000) / 200;
        }
        if load > 80 {
            advance -= (load - 80) / 4;
        } else if load < 40 {
            advance += (40 - load) / 8;
        }
        self.advance_deg = advance.clamp(5, TIMING_MAX);
    }

    /// Ported `simulateVoltage`: 10.0 V while cranking, 14.0 V charging,
    /// ±0.3 V noise.
    pub(super) fn simulate_voltage(&mut self) {
        let base = if self.mode == EngineMode::Startup {
            100
        } else {
            VOLTAGE_NORMAL
        };
        self.battery_dv = base + self.rng.range(-3, 4);
    }
}

/// Ported `interpolate`: rate-limited slew (percent of remaining delta per
/// step, minimum 1 unit) — gives the thermal/throttle curves their inertia.
fn interpolate(current: i32, target: i32, rate: i32) -> i32 {
    let delta = target - current;
    let mut step = delta * rate / 100;
    if step == 0 && delta != 0 {
        step = if delta > 0 { 1 } else { -1 };
    }
    current + step
}

/// Ported Arduino `map()` (linear rescale).
fn map_value(x: i32, in_min: i32, in_max: i32, out_min: i32, out_max: i32) -> i32 {
    (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}
