//! Base map generator — creates a driveable starting tune from engine specs
//!
//! Generates conservative VE tables, ignition maps, enrichment curves, and
//! IAC settings tailored to the specified engine configuration. The resulting
//! base map is designed to be safe (slightly rich, conservative timing) so
//! the engine can start and idle for fine-tuning.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::engine_spec::{Aspiration, EngineSpec, FuelType, StrokeType};

/// Acceleration enrichment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelEnrichConfig {
    /// TPS-based accel enrichment threshold (%/sec)
    pub tps_threshold: f64,
    /// Enrichment amount (% extra fuel)
    pub enrichment_pct: f64,
    /// Enrichment duration in engine cycles
    pub duration_cycles: u8,
    /// Taper rate (% reduction per cycle)
    pub taper_pct: f64,
}

impl Default for AccelEnrichConfig {
    fn default() -> Self {
        Self {
            tps_threshold: 10.0,
            enrichment_pct: 30.0,
            duration_cycles: 3,
            taper_pct: 50.0,
        }
    }
}

/// Idle air control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacConfig {
    /// IAC valve opening at cold start (0-100%)
    pub cold_start_pct: f64,
    /// IAC valve opening when warm and idle (0-100%)
    pub warm_idle_pct: f64,
    /// Coolant temperature at which engine is "warm" (°C)
    pub warm_threshold_c: f64,
}

impl Default for IacConfig {
    fn default() -> Self {
        Self {
            cold_start_pct: 70.0,
            warm_idle_pct: 20.0,
            warm_threshold_c: 75.0,
        }
    }
}

/// A complete base map ready to be applied to a tune
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseMap {
    /// The engine spec this map was generated for
    pub engine_spec: EngineSpec,

    /// RPM axis bins (typically 16 values)
    pub rpm_bins: Vec<f64>,

    /// Load axis bins in kPa (typically 16 values)
    pub load_bins: Vec<f64>,

    /// VE table values (row-major: load rows × rpm columns)
    pub ve_table: Vec<Vec<f64>>,

    /// Ignition advance table in degrees BTDC (row-major)
    pub ignition_table: Vec<Vec<f64>>,

    /// AFR target table (row-major)
    pub afr_table: Vec<Vec<f64>>,

    /// Cranking enrichment curve: (coolant_temp_C, enrichment_%)
    pub cranking_enrichment: Vec<(f64, f64)>,

    /// Warmup enrichment curve: (coolant_temp_C, enrichment_%)
    pub warmup_enrichment: Vec<(f64, f64)>,

    /// Acceleration enrichment settings
    pub accel_enrichment: AccelEnrichConfig,

    /// IAC settings
    pub iac: IacConfig,

    /// Prime pulse width in ms (at 20°C)
    pub prime_pulse_ms: f64,

    /// Calculated reqFuel value
    pub req_fuel: f64,

    /// Scalar constants to write: name -> displayed value
    pub scalars: HashMap<String, f64>,
}

/// Generate RPM axis bins with logarithmic-like spacing
///
/// Lower RPMs get finer resolution since that's where most tuning matters.
pub fn generate_rpm_bins(idle_rpm: u16, redline_rpm: u16, count: usize) -> Vec<f64> {
    let idle = idle_rpm as f64;
    let redline = redline_rpm as f64;

    if count <= 1 {
        return vec![idle];
    }

    // Use a mild exponential curve for more resolution at low RPM
    let mut bins = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f64 / (count - 1) as f64;
        // Quadratic bias toward lower RPM
        let biased_t = t.powf(1.3);
        let rpm = idle + (redline - idle) * biased_t;
        // Round to nearest 100
        let rounded = (rpm / 100.0).round() * 100.0;
        bins.push(rounded.max(idle));
    }

    // Ensure first bin is idle and last is redline (rounded to 100)
    bins[0] = (idle / 100.0).round() * 100.0;
    let last = bins.len() - 1;
    bins[last] = (redline / 100.0).round() * 100.0;

    // Remove duplicates (can happen at low end)
    bins.dedup();
    while bins.len() < count {
        // Fill in any gaps with interpolated values
        let last_val = *bins.last().unwrap();
        bins.push(last_val + 100.0);
    }
    bins.truncate(count);

    bins
}

/// Generate load (MAP) axis bins in kPa
pub fn generate_load_bins(max_load_kpa: f64, count: usize) -> Vec<f64> {
    if count <= 1 {
        return vec![max_load_kpa];
    }

    let min_load = 15.0; // Deep vacuum
    let mut bins = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f64 / (count - 1) as f64;
        let kpa = min_load + (max_load_kpa - min_load) * t;
        // Round to nearest integer
        bins.push(kpa.round());
    }

    bins[0] = min_load;
    let last = bins.len() - 1;
    bins[last] = max_load_kpa.round();
    bins.dedup();
    while bins.len() < count {
        let last_val = *bins.last().unwrap();
        bins.push(last_val + 5.0);
    }
    bins.truncate(count);
    bins
}

/// Generate a conservative VE table
///
/// Strategy:
/// - Low RPM / low load: ~35-45% VE (vacuum / idle)
/// - Mid RPM / mid load: ~55-75% VE (cruising)
/// - High RPM / high load: ~75-90% VE (WOT)
/// - Slightly rich everywhere for safety
pub fn generate_ve_table(spec: &EngineSpec, rpm_bins: &[f64], load_bins: &[f64]) -> Vec<Vec<f64>> {
    let rows = load_bins.len();
    let cols = rpm_bins.len();
    let idle = spec.idle_rpm as f64;
    let redline = spec.redline_rpm as f64;
    let max_load = spec.max_load_kpa();

    let mut table = vec![vec![0.0; cols]; rows];

    for (r, &load) in load_bins.iter().enumerate() {
        for (c, &rpm) in rpm_bins.iter().enumerate() {
            // Normalize rpm and load to 0..1 range
            let rpm_norm = ((rpm - idle) / (redline - idle)).clamp(0.0, 1.0);
            let load_norm = ((load - 15.0) / (max_load - 15.0)).clamp(0.0, 1.0);

            // Base VE curve
            let base_ve = match spec.aspiration {
                Aspiration::NA => {
                    // NA engines: 30% at idle/vacuum, peaking ~85% at high load/mid RPM
                    let rpm_factor = 1.0 - (rpm_norm - 0.6).powi(2) * 0.3;
                    30.0 + 55.0 * load_norm * rpm_factor
                }
                Aspiration::Turbo | Aspiration::Supercharged => {
                    // Boosted: higher VE above atmospheric, reaching 90-100% under boost
                    let rpm_factor = 1.0 - (rpm_norm - 0.55).powi(2) * 0.25;
                    let boost_factor = if load > 101.0 {
                        1.0 + (load_norm - 0.5).max(0.0) * 0.3
                    } else {
                        1.0
                    };
                    28.0 + 62.0 * load_norm * rpm_factor * boost_factor
                }
            };

            // Clamp to valid VE range
            table[r][c] = base_ve.clamp(15.0, 120.0).round();
        }
    }

    table
}

/// Generate a conservative ignition advance table
///
/// Strategy:
/// - Low RPM / high load: conservative (15-20° BTDC)
/// - High RPM: more advance (25-35°)
/// - Boosted: reduce advance at high load to prevent knock
/// - 2-stroke: less advance overall
pub fn generate_ignition_table(
    spec: &EngineSpec,
    rpm_bins: &[f64],
    load_bins: &[f64],
) -> Vec<Vec<f64>> {
    let rows = load_bins.len();
    let cols = rpm_bins.len();
    let idle = spec.idle_rpm as f64;
    let redline = spec.redline_rpm as f64;
    let max_load = spec.max_load_kpa();

    let stroke_factor = match spec.stroke_type {
        StrokeType::FourStroke => 1.0,
        StrokeType::TwoStroke => 0.75, // Less advance for 2-stroke
    };

    let mut table = vec![vec![0.0; cols]; rows];

    for (r, &load) in load_bins.iter().enumerate() {
        for (c, &rpm) in rpm_bins.iter().enumerate() {
            let rpm_norm = ((rpm - idle) / (redline - idle)).clamp(0.0, 1.0);
            let load_norm = ((load - 15.0) / (max_load - 15.0)).clamp(0.0, 1.0);

            // Base timing: increases with RPM, decreases with load
            let rpm_advance = 10.0 + 22.0 * rpm_norm; // 10° at idle → 32° at redline
            let load_retard = load_norm * 8.0; // Up to 8° less at WOT

            let mut advance = (rpm_advance - load_retard) * stroke_factor;

            // Extra retard for boosted engines above atmospheric
            if matches!(spec.aspiration, Aspiration::Turbo | Aspiration::Supercharged)
                && load > 101.0
            {
                let boost_pct = (load - 101.0) / (max_load - 101.0);
                advance -= boost_pct * 8.0; // Up to 8° more retard under boost
            }

            // Idle timing: fixed ~15° for stability
            if rpm < idle * 1.2 && load < 50.0 {
                advance = advance.max(12.0).min(18.0);
            }

            table[r][c] = advance.clamp(0.0, 45.0).round();
        }
    }

    table
}

/// Generate AFR target table
///
/// Strategy:
/// - Idle: slightly rich of stoich (14.0 for gasoline)
/// - Cruise: lean for economy (15.0-15.5 for gasoline)
/// - WOT: safe rich (12.0-12.5 for gasoline)
/// - Transition zones smoothly interpolated
pub fn generate_afr_table(
    spec: &EngineSpec,
    rpm_bins: &[f64],
    load_bins: &[f64],
) -> Vec<Vec<f64>> {
    let rows = load_bins.len();
    let cols = rpm_bins.len();
    let idle = spec.idle_rpm as f64;
    let redline = spec.redline_rpm as f64;
    let max_load = spec.max_load_kpa();
    let stoich = spec.fuel_type.stoich_afr();
    let wot_afr = spec.safe_wot_afr();

    let mut table = vec![vec![0.0; cols]; rows];

    for (r, &load) in load_bins.iter().enumerate() {
        for (c, &rpm) in rpm_bins.iter().enumerate() {
            let rpm_norm = ((rpm - idle) / (redline - idle)).clamp(0.0, 1.0);
            let load_norm = ((load - 15.0) / (max_load - 15.0)).clamp(0.0, 1.0);

            let afr = if load_norm > 0.85 {
                // High load: rich for safety
                wot_afr
            } else if load_norm < 0.35 && rpm_norm < 0.3 {
                // Idle/cruise: slightly rich of stoich
                stoich * 0.97
            } else if load_norm < 0.5 {
                // Light cruise: slightly lean for economy
                stoich * 1.02
            } else {
                // Transition: interpolate between cruise and WOT
                let blend = ((load_norm - 0.5) / 0.35).clamp(0.0, 1.0);
                let cruise_afr = stoich * 1.0;
                cruise_afr + (wot_afr - cruise_afr) * blend
            };

            // Round to 1 decimal
            table[r][c] = (afr * 10.0).round() / 10.0;
        }
    }

    table
}

/// Generate cranking enrichment curve
///
/// Returns (coolant_temp_C, enrichment_%) pairs
pub fn generate_cranking_enrichment(fuel_type: &FuelType) -> Vec<(f64, f64)> {
    // Alcohol fuels need more cranking enrichment
    let factor = match fuel_type {
        FuelType::Gasoline => 1.0,
        FuelType::E85 => 1.4,
        FuelType::E100 => 1.6,
        FuelType::Methanol => 1.8,
        FuelType::LPG => 0.8,
    };

    vec![
        (-40.0, 400.0 * factor),
        (-20.0, 350.0 * factor),
        (0.0, 280.0 * factor),
        (20.0, 200.0 * factor),
        (40.0, 150.0 * factor),
        (60.0, 120.0 * factor),
        (80.0, 100.0 * factor),
        (100.0, 90.0 * factor),
    ]
}

/// Generate warmup enrichment curve (WUE)
///
/// Returns (coolant_temp_C, enrichment_%) pairs
/// 100% = no enrichment (fully warm)
pub fn generate_warmup_enrichment(fuel_type: &FuelType) -> Vec<(f64, f64)> {
    let factor = match fuel_type {
        FuelType::Gasoline => 1.0,
        FuelType::E85 => 1.2,
        FuelType::E100 => 1.3,
        FuelType::Methanol => 1.4,
        FuelType::LPG => 0.9,
    };

    vec![
        (-40.0, 180.0 * factor),
        (-20.0, 165.0 * factor),
        (0.0, 150.0 * factor),
        (20.0, 135.0 * factor),
        (40.0, 120.0 * factor),
        (60.0, 110.0 * factor),
        (80.0, 100.0), // Fully warm — no enrichment
        (100.0, 100.0),
    ]
}

/// Generate prime pulse width based on temperature
pub fn generate_prime_pulse_ms(fuel_type: &FuelType, injector_size_cc: f64) -> f64 {
    // Larger injectors need shorter pulse; alcohol fuels need more
    let base = match fuel_type {
        FuelType::Gasoline => 3.0,
        FuelType::E85 => 4.5,
        FuelType::E100 => 5.0,
        FuelType::Methanol => 6.0,
        FuelType::LPG => 2.0,
    };
    // Scale inversely with injector size (calibrated around 440cc)
    let size_factor = 440.0 / injector_size_cc.max(100.0);
    (base * size_factor).clamp(0.5, 15.0)
}

/// Generate a complete base map from engine specs
pub fn generate_base_map(spec: &EngineSpec) -> BaseMap {
    let table_size = 16; // Standard 16x16 table

    let rpm_bins = generate_rpm_bins(spec.idle_rpm, spec.redline_rpm, table_size);
    let load_bins = generate_load_bins(spec.max_load_kpa(), table_size);

    let ve_table = generate_ve_table(spec, &rpm_bins, &load_bins);
    let ignition_table = generate_ignition_table(spec, &rpm_bins, &load_bins);
    let afr_table = generate_afr_table(spec, &rpm_bins, &load_bins);

    let cranking_enrichment = generate_cranking_enrichment(&spec.fuel_type);
    let warmup_enrichment = generate_warmup_enrichment(&spec.fuel_type);
    let accel_enrichment = AccelEnrichConfig::default();

    let iac = IacConfig {
        cold_start_pct: match spec.fuel_type {
            FuelType::E85 | FuelType::E100 | FuelType::Methanol => 80.0,
            _ => 70.0,
        },
        warm_idle_pct: 20.0,
        warm_threshold_c: 75.0,
    };

    let prime_pulse_ms = generate_prime_pulse_ms(&spec.fuel_type, spec.injector_size_cc);
    let req_fuel = spec.compute_req_fuel();

    // Build scalar constants map
    let mut scalars = HashMap::new();
    scalars.insert("reqFuel".to_string(), req_fuel);

    // Injection mode (Speeduino convention: 0=simultaneous, 1=sequential)
    use super::engine_spec::InjectionMode;
    let inj_mode_val = match spec.injection_mode {
        InjectionMode::Simultaneous => 0.0,
        InjectionMode::Sequential => 1.0,
        InjectionMode::Batch => 2.0,
        InjectionMode::ThrottleBody => 3.0,
    };
    scalars.insert("injType".to_string(), inj_mode_val);

    // Ignition mode (Speeduino: 0=wasted spark, 1=COP, 2=distributor)
    use super::engine_spec::IgnitionMode;
    let ign_mode_val = match spec.ignition_mode {
        IgnitionMode::WastedSpark => 0.0,
        IgnitionMode::CoilOnPlug => 1.0,
        IgnitionMode::Distributor => 2.0,
    };
    scalars.insert("sparkMode".to_string(), ign_mode_val);

    // Number of cylinders
    scalars.insert("nCylinders".to_string(), spec.cylinder_count as f64);

    // Stoichiometric ratio
    scalars.insert("stoich".to_string(), spec.fuel_type.stoich_afr());

    BaseMap {
        engine_spec: spec.clone(),
        rpm_bins,
        load_bins,
        ve_table,
        ignition_table,
        afr_table,
        cranking_enrichment,
        warmup_enrichment,
        accel_enrichment,
        iac,
        prime_pulse_ms,
        req_fuel,
        scalars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basemap::engine_spec::{Aspiration, EngineSpec, FuelType};

    #[test]
    fn test_generate_rpm_bins() {
        let bins = generate_rpm_bins(800, 6500, 16);
        assert_eq!(bins.len(), 16);
        assert!(bins[0] >= 800.0);
        assert!((bins[15] - 6500.0).abs() < 100.0);
        // Should be monotonically increasing
        for i in 1..bins.len() {
            assert!(bins[i] >= bins[i - 1], "RPM bins must be increasing");
        }
    }

    #[test]
    fn test_generate_load_bins() {
        let bins = generate_load_bins(105.0, 16);
        assert_eq!(bins.len(), 16);
        assert!((bins[0] - 15.0).abs() < 0.01);
        assert!((bins[15] - 105.0).abs() < 0.01);
        for i in 1..bins.len() {
            assert!(bins[i] >= bins[i - 1], "Load bins must be increasing");
        }
    }

    #[test]
    fn test_generate_load_bins_turbo() {
        let bins = generate_load_bins(250.0, 16);
        assert_eq!(bins.len(), 16);
        assert!((bins[15] - 250.0).abs() < 0.01);
    }

    #[test]
    fn test_generate_ve_table_na() {
        let spec = EngineSpec::default();
        let rpm = generate_rpm_bins(spec.idle_rpm, spec.redline_rpm, 16);
        let load = generate_load_bins(spec.max_load_kpa(), 16);
        let ve = generate_ve_table(&spec, &rpm, &load);
        assert_eq!(ve.len(), 16);
        assert_eq!(ve[0].len(), 16);
        // Idle/vacuum should be low VE
        assert!(ve[0][0] < 50.0, "Idle VE should be low: {}", ve[0][0]);
        // WOT/mid RPM should be higher
        assert!(ve[15][8] > 50.0, "WOT VE should be high: {}", ve[15][8]);
    }

    #[test]
    fn test_generate_ignition_table() {
        let spec = EngineSpec::default();
        let rpm = generate_rpm_bins(spec.idle_rpm, spec.redline_rpm, 16);
        let load = generate_load_bins(spec.max_load_kpa(), 16);
        let ign = generate_ignition_table(&spec, &rpm, &load);
        assert_eq!(ign.len(), 16);
        // All values should be positive (advance, not retard)
        for row in &ign {
            for &val in row {
                assert!(val >= 0.0 && val <= 45.0, "Timing out of range: {}", val);
            }
        }
        // Higher RPM should generally have more advance
        assert!(
            ign[8][15] > ign[8][0],
            "High RPM should have more advance than low"
        );
    }

    #[test]
    fn test_generate_afr_table() {
        let spec = EngineSpec::default();
        let rpm = generate_rpm_bins(spec.idle_rpm, spec.redline_rpm, 16);
        let load = generate_load_bins(spec.max_load_kpa(), 16);
        let afr = generate_afr_table(&spec, &rpm, &load);
        assert_eq!(afr.len(), 16);
        // WOT should be richer than cruise
        let top_row = &afr[15]; // High load
        let mid_row = &afr[8]; // Mid load
        let wot_afr = top_row[8];
        let cruise_afr = mid_row[3];
        assert!(
            wot_afr < cruise_afr,
            "WOT AFR ({}) should be richer (lower) than cruise ({})",
            wot_afr,
            cruise_afr
        );
    }

    #[test]
    fn test_generate_cranking_enrichment() {
        let curve = generate_cranking_enrichment(&FuelType::Gasoline);
        assert!(!curve.is_empty());
        // Cold temps should have more enrichment
        assert!(curve[0].1 > curve[curve.len() - 1].1);
    }

    #[test]
    fn test_generate_warmup_enrichment() {
        let curve = generate_warmup_enrichment(&FuelType::Gasoline);
        assert!(!curve.is_empty());
        // Cold should be > 100%, warm should be 100%
        assert!(curve[0].1 > 100.0);
        assert!((curve[curve.len() - 1].1 - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_generate_base_map_complete() {
        let spec = EngineSpec::default();
        let bm = generate_base_map(&spec);
        assert_eq!(bm.rpm_bins.len(), 16);
        assert_eq!(bm.load_bins.len(), 16);
        assert_eq!(bm.ve_table.len(), 16);
        assert_eq!(bm.ignition_table.len(), 16);
        assert_eq!(bm.afr_table.len(), 16);
        assert!(!bm.cranking_enrichment.is_empty());
        assert!(!bm.warmup_enrichment.is_empty());
        assert!(bm.req_fuel > 0.0);
        assert!(bm.prime_pulse_ms > 0.0);
        assert!(bm.scalars.contains_key("reqFuel"));
        assert!(bm.scalars.contains_key("nCylinders"));
    }

    #[test]
    fn test_generate_base_map_turbo() {
        let spec = EngineSpec {
            aspiration: Aspiration::Turbo,
            boost_target_kpa: Some(200.0),
            ..Default::default()
        };
        let bm = generate_base_map(&spec);
        // Turbo map should have load bins going above 101 kPa
        assert!(bm.load_bins.last().unwrap() > &101.0);
    }

    #[test]
    fn test_generate_base_map_e85() {
        let spec = EngineSpec {
            fuel_type: FuelType::E85,
            ..Default::default()
        };
        let bm = generate_base_map(&spec);
        // E85 stoich is 9.8, WOT target should be around 8.5
        let wot_afr = bm.afr_table[15][8];
        assert!(
            wot_afr < 10.0,
            "E85 WOT AFR should be below 10: {}",
            wot_afr
        );
    }
}
