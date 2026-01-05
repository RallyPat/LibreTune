//! Demo Mode - Simulated ECU data generator for testing
//!
//! Generates realistic engine sensor data for UI testing without a real ECU connection.
//! Simulates an engine idling at ~850 RPM with random throttle blips.

use std::collections::HashMap;
use rand::SeedableRng;
use rand::Rng;
use rand::rngs::StdRng;

/// Demo ECU simulator that generates realistic engine sensor data
pub struct DemoSimulator {
    /// Time when simulation started (ms)
    start_time_ms: u64,
    /// Last update time (ms)
    last_update_ms: u64,
    /// Time of next throttle blip (ms from start)
    next_blip_at_ms: u64,
    /// Current blip state
    blip_state: BlipState,
    /// Current RPM (smoothed)
    current_rpm: f64,
    /// Target RPM for current blip
    blip_target_rpm: f64,
    /// Random number generator (thread-safe)
    rng: StdRng,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BlipState {
    /// Engine idling normally
    Idle,
    /// Throttle opening, RPM rising
    RampUp { start_ms: u64 },
    /// At peak RPM, holding
    Hold { start_ms: u64 },
    /// Throttle closing, RPM falling
    RampDown { start_ms: u64 },
}

impl Default for DemoSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoSimulator {
    /// Create a new demo simulator
    pub fn new() -> Self {
        let mut rng = StdRng::from_entropy();
        let first_blip = rng.gen_range(8000..15000); // 8-15 seconds
        
        Self {
            start_time_ms: 0,
            last_update_ms: 0,
            next_blip_at_ms: first_blip,
            blip_state: BlipState::Idle,
            current_rpm: 850.0,
            blip_target_rpm: 0.0,
            rng,
        }
    }

    /// Update simulation and generate current sensor values
    /// 
    /// # Arguments
    /// * `elapsed_ms` - Milliseconds since simulation started
    /// 
    /// # Returns
    /// HashMap of channel names to their current values
    pub fn update(&mut self, elapsed_ms: u64) -> HashMap<String, f64> {
        if self.start_time_ms == 0 {
            self.start_time_ms = elapsed_ms;
        }
        
        let sim_time = elapsed_ms - self.start_time_ms;
        let delta_ms = if self.last_update_ms > 0 {
            elapsed_ms.saturating_sub(self.last_update_ms)
        } else {
            0
        };
        self.last_update_ms = elapsed_ms;
        
        // Update blip state machine
        self.update_blip_state(sim_time);
        
        // Calculate target RPM based on state
        let target_rpm = self.calculate_target_rpm(sim_time);
        
        // Smooth RPM changes
        let rpm_rate = if target_rpm > self.current_rpm { 8000.0 } else { 3000.0 }; // RPM/sec
        let max_change = rpm_rate * (delta_ms as f64 / 1000.0);
        let rpm_diff = target_rpm - self.current_rpm;
        self.current_rpm += rpm_diff.clamp(-max_change, max_change);
        
        // Add idle wobble
        let t = sim_time as f64 / 1000.0;
        let idle_wobble = if matches!(self.blip_state, BlipState::Idle) {
            20.0 * (t * 2.5).sin() + 10.0 * (t * 7.3).sin()
        } else {
            0.0
        };
        let rpm = (self.current_rpm + idle_wobble).max(0.0);
        
        // Generate correlated sensor values
        let mut data = HashMap::new();
        
        // RPM
        data.insert("rpm".to_string(), rpm);
        data.insert("RPM".to_string(), rpm);
        
        // TPS - throttle position
        let tps = match self.blip_state {
            BlipState::Idle => 1.5 + 1.0 * (t * 0.3).sin().abs(),
            BlipState::RampUp { .. } => {
                let progress = (self.current_rpm - 850.0) / (self.blip_target_rpm - 850.0);
                2.0 + progress * 45.0 // Up to ~47% throttle
            }
            BlipState::Hold { .. } => 45.0 + 5.0 * (t * 3.0).sin(),
            BlipState::RampDown { .. } => {
                let progress = (self.current_rpm - 850.0) / (self.blip_target_rpm - 850.0);
                2.0 + progress * 40.0
            }
        };
        data.insert("tps".to_string(), tps.clamp(0.0, 100.0));
        data.insert("TPS".to_string(), tps.clamp(0.0, 100.0));
        data.insert("throttle".to_string(), tps.clamp(0.0, 100.0));
        
        // MAP - manifold absolute pressure (inversely related to RPM at idle/part throttle)
        let map = if rpm < 1000.0 {
            35.0 + 10.0 * (1.0 - rpm / 1000.0)
        } else {
            25.0 + (rpm / 100.0) // Higher at higher RPM with load
        };
        data.insert("map".to_string(), map.clamp(20.0, 105.0));
        data.insert("MAP".to_string(), map.clamp(20.0, 105.0));
        data.insert("fuelLoad".to_string(), map.clamp(20.0, 105.0));
        
        // AFR - oscillating around stoichiometric
        let afr = 14.7 + 0.3 * (t * 1.5).sin() + 0.1 * (t * 4.7).sin();
        data.insert("afr".to_string(), afr);
        data.insert("AFR".to_string(), afr);
        data.insert("AFRValue".to_string(), afr);
        
        // Lambda
        let lambda = afr / 14.7;
        data.insert("lambda".to_string(), lambda);
        
        // Target AFR
        let afr_target = if rpm < 1500.0 { 14.7 } else { 13.5 };
        data.insert("afrTarget".to_string(), afr_target);
        
        // Coolant temperature - warming up over time
        let coolant = 20.0 + 70.0 * (1.0 - (-t / 120.0).exp());
        data.insert("coolant".to_string(), coolant);
        data.insert("CLT".to_string(), coolant);
        data.insert("coolantTemperature".to_string(), coolant);
        
        // Intake air temperature - stable
        let iat = 25.0 + 5.0 * (t * 0.1).sin();
        data.insert("iat".to_string(), iat);
        data.insert("IAT".to_string(), iat);
        
        // Battery voltage - stable with small ripple
        let battery = 13.8 + 0.2 * (t * 0.5).sin();
        data.insert("batteryVoltage".to_string(), battery);
        data.insert("vBatt".to_string(), battery);
        
        // Ignition advance - increases with RPM
        let advance = 10.0 + (rpm / 200.0) + 3.0 * (t * 0.7).sin();
        data.insert("advance".to_string(), advance.clamp(-10.0, 45.0));
        data.insert("timing".to_string(), advance.clamp(-10.0, 45.0));
        
        // Injector pulse width - varies with load
        let pw = 2.5 + (rpm / 1000.0) * 1.5 + (tps / 50.0) * 2.0;
        data.insert("pulseWidth".to_string(), pw);
        data.insert("actualLastInjection".to_string(), pw);
        
        // VE (volumetric efficiency)
        let ve = 75.0 + (rpm / 100.0) + 10.0 * (tps / 50.0);
        data.insert("VE1".to_string(), ve.clamp(0.0, 150.0));
        data.insert("veCurr".to_string(), ve.clamp(0.0, 150.0));
        
        // EGO correction - oscillating around 100%
        let ego = 100.0 + 5.0 * (t * 2.0).sin();
        data.insert("egoCorrection".to_string(), ego);
        
        // Sync status - always synced in demo
        data.insert("hasSync".to_string(), 1.0);
        data.insert("running".to_string(), 1.0);
        data.insert("engine".to_string(), 1.0);
        
        // Dwell
        let dwell = 3.0 + 0.5 * (rpm / 2000.0);
        data.insert("dwell".to_string(), dwell);
        
        // Barometric pressure
        data.insert("baro".to_string(), 101.0);
        data.insert("baroPressure".to_string(), 101.0);
        
        data
    }

    /// Update the blip state machine
    fn update_blip_state(&mut self, sim_time: u64) {
        const RAMP_UP_MS: u64 = 300;
        const HOLD_MS: u64 = 200;
        const RAMP_DOWN_MS: u64 = 800;

        match self.blip_state {
            BlipState::Idle => {
                if sim_time >= self.next_blip_at_ms {
                    // Start a new blip
                    self.blip_target_rpm = self.rng.gen_range(2000.0..4000.0);
                    self.blip_state = BlipState::RampUp { start_ms: sim_time };
                }
            }
            BlipState::RampUp { start_ms } => {
                if sim_time >= start_ms + RAMP_UP_MS {
                    self.blip_state = BlipState::Hold { start_ms: sim_time };
                }
            }
            BlipState::Hold { start_ms } => {
                if sim_time >= start_ms + HOLD_MS {
                    self.blip_state = BlipState::RampDown { start_ms: sim_time };
                }
            }
            BlipState::RampDown { start_ms } => {
                if sim_time >= start_ms + RAMP_DOWN_MS {
                    // Back to idle, schedule next blip
                    self.blip_state = BlipState::Idle;
                    let next_interval = self.rng.gen_range(8000..15000);
                    self.next_blip_at_ms = sim_time + next_interval;
                }
            }
        }
    }

    /// Calculate target RPM based on current blip state
    fn calculate_target_rpm(&self, sim_time: u64) -> f64 {
        const RAMP_UP_MS: u64 = 300;
        const RAMP_DOWN_MS: u64 = 800;
        const IDLE_RPM: f64 = 850.0;

        match self.blip_state {
            BlipState::Idle => IDLE_RPM,
            BlipState::RampUp { start_ms } => {
                let progress = ((sim_time - start_ms) as f64 / RAMP_UP_MS as f64).min(1.0);
                IDLE_RPM + (self.blip_target_rpm - IDLE_RPM) * progress
            }
            BlipState::Hold { .. } => self.blip_target_rpm,
            BlipState::RampDown { start_ms } => {
                let progress = ((sim_time - start_ms) as f64 / RAMP_DOWN_MS as f64).min(1.0);
                self.blip_target_rpm + (IDLE_RPM - self.blip_target_rpm) * progress
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_creates_data() {
        let mut sim = DemoSimulator::new();
        let data = sim.update(0);
        
        assert!(data.contains_key("rpm"));
        assert!(data.contains_key("map"));
        assert!(data.contains_key("afr"));
        assert!(data.contains_key("coolant"));
    }

    #[test]
    fn test_idle_rpm_range() {
        let mut sim = DemoSimulator::new();
        
        // Run for a few "seconds" at idle
        for ms in (0..3000).step_by(100) {
            let data = sim.update(ms);
            let rpm = data.get("rpm").unwrap();
            // Should be around idle with some wobble
            assert!(*rpm > 700.0 && *rpm < 1000.0, "RPM {} out of idle range", rpm);
        }
    }

    #[test]
    fn test_coolant_warmup() {
        let mut sim = DemoSimulator::new();
        
        // First call initializes start_time_ms
        let data_start = sim.update(1000);
        let coolant_start = *data_start.get("coolant").unwrap();
        
        // After 60 "seconds" (61000ms since we started at 1000)
        let data_later = sim.update(61_000);
        let coolant_later = *data_later.get("coolant").unwrap();
        
        assert!(coolant_later > coolant_start, "Coolant should warm up over time: {} vs {}", coolant_start, coolant_later);
        assert!(coolant_later < 90.0, "Coolant shouldn't be fully warm yet at 60s");
    }
}
