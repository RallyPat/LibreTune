//! Sensor calibration curves: turning an INI `referenceTable` solution into
//! the engineering values the ECU should store.
//!
//! This is the layer above [`crate::protocol::calibration`], which owns the
//! wire encoding. Here we answer "what AFR does ADC count 512 mean?"; there we
//! answer "what bytes does that become".
//!
//! Three ways to produce a curve, matching the INI's own vocabulary:
//!
//! * **`solution` expression** — a closed-form formula over `adcValue`
//!   ([`evaluate_solution`]), e.g. the `14Point7` preset.
//! * **`linearGenerator`** — two (volts, AFR) points ([`LinearWideband`]),
//!   which is also what [`auto_calibrate_from_plateaus`] solves for.
//! * **`thermGenerator`** — a three-point Steinhart–Hart thermistor fit
//!   ([`ThermistorFit`]).

use std::collections::HashMap;

use crate::ini::expression::{evaluate, Parser, StringContext};
use crate::ini::{CalibrationSolution, IncTableCache, ReferenceTable, ThermistorOption};

pub mod plateau;

pub use plateau::{
    auto_calibrate_from_plateaus, AfrSample, PlateauCalibration, PlateauDetectorConfig,
};

/// ADC counts spanned by the Speeduino's 10-bit sensor inputs.
pub const ADC_COUNTS: usize = 1024;
/// Full-scale reference voltage of those inputs.
pub const ADC_REFERENCE_VOLTS: f64 = 5.0;

/// Volts at a 10-bit ADC count (0 → 0 V, 1023 → 5 V).
pub fn adc_to_volts(adc: u16) -> f64 {
    adc as f64 * ADC_REFERENCE_VOLTS / (ADC_COUNTS as f64 - 1.0)
}

/// Errors from building a calibration curve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalibrationError {
    /// The solution's expression could not be parsed.
    ParseFailed(String),
    /// The expression parsed but could not be evaluated at some ADC count.
    EvalFailed(String),
    /// The solution defers to an interactive generator and has no formula.
    NotAnExpression(String),
    /// A `table(...)` lookup referenced an `.inc` file we have no content for.
    MissingIncludeFile(String),
    /// Inputs that cannot describe a line (two identical voltages).
    DegenerateInputs(String),
}

impl std::fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalibrationError::ParseFailed(m) => write!(f, "calibration formula parse failed: {m}"),
            CalibrationError::EvalFailed(m) => write!(f, "calibration formula failed: {m}"),
            CalibrationError::NotAnExpression(g) => write!(
                f,
                "this preset uses the '{g}' generator and has no formula to evaluate"
            ),
            CalibrationError::MissingIncludeFile(name) => write!(
                f,
                "preset needs the lookup file '{name}', which is not available"
            ),
            CalibrationError::DegenerateInputs(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for CalibrationError {}

/// Evaluate a `solution` expression across every ADC count, yielding the
/// engineering value (AFR) per count.
///
/// The INI expressions are plain arithmetic over `adcValue` save for the
/// non-linear presets, which call `table(volts, "sensor.inc")`. Those `.inc`
/// files ship with TunerStudio, not with the INI, so a preset that needs one
/// is reported as [`CalibrationError::MissingIncludeFile`] rather than
/// silently evaluating to zero — an all-zero AFR table written to an ECU
/// would make the wideband read 0.0 AFR everywhere and, if anything trusted
/// it for closed-loop, command maximum enrichment.
pub fn evaluate_solution(expression: &str) -> Result<Vec<f64>, CalibrationError> {
    evaluate_solution_with_tables(expression, &mut IncTableCache::new(Vec::new()))
}

/// As [`evaluate_solution`], but able to resolve the `table(volts, "x.inc")`
/// lookups the non-linear presets use, from `cache`'s search paths.
///
/// The expression evaluator's own `table()` returns `0.0` when a lookup
/// misses, which for a calibration curve is the worst possible failure: it
/// produces a table reading 0.0 AFR at every ADC count, writes cleanly, and
/// verifies its CRC. So the file is resolved up-front here and a miss is an
/// error, never a zero.
pub fn evaluate_solution_with_tables(
    expression: &str,
    cache: &mut IncTableCache,
) -> Result<Vec<f64>, CalibrationError> {
    let inc_table = match referenced_include_file(expression) {
        Some(name) => Some(
            cache
                .get_or_load(&name)
                .ok_or(CalibrationError::MissingIncludeFile(name))?
                .clone(),
        ),
        None => None,
    };

    let mut parser = Parser::new(expression);
    let expr = parser
        .parse()
        .map_err(|e| CalibrationError::ParseFailed(format!("{e} (in `{expression}`)")))?;

    let string_context = inc_table.map(|table| StringContext {
        table_lookup: Some(Box::new(move |_filename: &str, value: f64| {
            table.lookup(value)
        })),
        ..Default::default()
    });

    let mut context: HashMap<String, f64> = HashMap::new();
    let mut out = Vec::with_capacity(ADC_COUNTS);
    for adc in 0..ADC_COUNTS {
        context.insert("adcValue".to_string(), adc as f64);
        let value = evaluate(&expr, &context, string_context.as_ref())
            .map_err(|e| CalibrationError::EvalFailed(format!("{e} (at adcValue={adc})")))?;
        out.push(value.as_f64());
    }
    Ok(out)
}

/// Name of the `.inc` lookup file a `table(...)` call references, if any.
fn referenced_include_file(expression: &str) -> Option<String> {
    let idx = expression.find("table(")?;
    let rest = &expression[idx..];
    let open = rest.find('"')?;
    let after = &rest[open + 1..];
    let close = after.find('"')?;
    Some(after[..close].to_string())
}

/// Build the AFR curve for a named solution of a `referenceTable` block.
pub fn solution_curve(
    table: &ReferenceTable,
    label: &str,
    cache: &mut IncTableCache,
) -> Option<Result<Vec<f64>, CalibrationError>> {
    let (_, solution) = table
        .solutions
        .iter()
        .find(|(l, _)| l.eq_ignore_ascii_case(label))?;
    Some(match solution {
        CalibrationSolution::Expression { expression } => {
            evaluate_solution_with_tables(expression, cache)
        }
        CalibrationSolution::Generator { generator } => {
            Err(CalibrationError::NotAnExpression(generator.clone()))
        }
    })
}

/// A two-point linear wideband transfer function — the `linearGenerator`
/// case, and the form the plateau auto-cal solves for.
///
/// Stored as the two measured points rather than slope/intercept so the UI
/// can show exactly what was entered, and so [`describe`](Self::describe)
/// can report the transfer function in the same terms the sensor's datasheet
/// uses.
/// A reference correction applied over an existing AFR curve.
///
/// The workflow this models: the ECU is displaying an AFR through its current
/// calibration, and a trusted reference - the wideband controller's own gauge,
/// a second meter, span gas - says what the mixture actually is. Each point is
/// `(measured, expected)`: what the ECU showed, and what it should have shown.
///
/// One point corrects offset only (`afr' = afr + (expected - measured)`),
/// which is the right tool when the whole scale reads uniformly rich or lean -
/// a ground-offset error does exactly that. Two points solve gain and offset,
/// for a sensor whose error grows across the range - this NA6's Spartan reads
/// about 0.3 AFR low near stoich and nearly 6% low at the lean end, which an
/// offset alone cannot express.
#[derive(Debug, Clone, PartialEq)]
pub struct AfrCorrection {
    /// Multiplier applied to the curve's AFR.
    pub gain: f64,
    /// Offset added after the gain.
    pub offset: f64,
    points: Vec<(f64, f64)>,
}

impl AfrCorrection {
    /// Build from one or two `(measured, expected)` pairs.
    pub fn from_points(points: &[(f64, f64)]) -> Result<Self, CalibrationError> {
        match points {
            [(m, e)] => Ok(Self {
                gain: 1.0,
                offset: e - m,
                points: points.to_vec(),
            }),
            [(m1, e1), (m2, e2)] => {
                if (m1 - m2).abs() < 1e-6 {
                    return Err(CalibrationError::DegenerateInputs(format!(
                        "the two measured readings must differ (both were                          {m1:.2} AFR) - a gain cannot be solved from one point",
                    )));
                }
                let gain = (e2 - e1) / (m2 - m1);
                // A non-positive gain flattens or reverses the curve: richer
                // mixtures would read leaner. No physical sensor error does
                // that; it means the two points were swapped or mistyped.
                if gain <= 0.0 {
                    return Err(CalibrationError::DegenerateInputs(format!(
                        "these points give a gain of {gain:.3}, which would                          reverse the curve - check the expected values are                          paired with the right measured readings",
                    )));
                }
                Ok(Self {
                    gain,
                    offset: e1 - gain * m1,
                    points: points.to_vec(),
                })
            }
            _ => Err(CalibrationError::DegenerateInputs(format!(
                "a correction takes one or two points, got {}",
                points.len()
            ))),
        }
    }

    /// Apply to a curve in place.
    pub fn apply(&self, afr: &mut [f64]) {
        for v in afr.iter_mut() {
            *v = self.gain * *v + self.offset;
        }
    }

    /// Human-readable suffix for the stored transfer function, so a log reads
    /// as corrected rather than silently differing from its preset.
    pub fn describe(&self) -> String {
        match self.points.as_slice() {
            [(m, e)] => format!(
                "corrected {:+.2} AFR against a reference ({m:.2} read as {e:.2})",
                self.offset
            ),
            _ => format!(
                "corrected against a reference (gain {:.4}, offset {:+.2})",
                self.gain, self.offset
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LinearWideband {
    /// First calibration point, `(volts, afr)`.
    pub point1: (f64, f64),
    /// Second calibration point, `(volts, afr)`.
    pub point2: (f64, f64),
}

impl LinearWideband {
    /// Construct from two (volts, AFR) points.
    pub fn new(point1: (f64, f64), point2: (f64, f64)) -> Result<Self, CalibrationError> {
        if (point1.0 - point2.0).abs() < 1e-9 {
            return Err(CalibrationError::DegenerateInputs(format!(
                "the two calibration points must be at different voltages \
                 (both were {:.3} V)",
                point1.0
            )));
        }
        Ok(Self { point1, point2 })
    }

    /// AFR per volt.
    pub fn slope(&self) -> f64 {
        (self.point2.1 - self.point1.1) / (self.point2.0 - self.point1.0)
    }

    /// AFR at 0 V.
    pub fn intercept(&self) -> f64 {
        self.point1.1 - self.slope() * self.point1.0
    }

    /// AFR at a given voltage.
    pub fn afr_at_volts(&self, volts: f64) -> f64 {
        self.intercept() + self.slope() * volts
    }

    /// The full 1024-entry AFR curve this line produces.
    pub fn curve(&self) -> Vec<f64> {
        (0..ADC_COUNTS)
            .map(|adc| self.afr_at_volts(adc_to_volts(adc as u16)))
            .collect()
    }

    /// Human-readable transfer function, stored with the tune so logs are
    /// self-describing: `"9.69–19.80 AFR over 0–5 V (3.847 V, ground-offset
    /// compensated)"`.
    pub fn describe(&self) -> String {
        let at_zero = self.afr_at_volts(0.0);
        let at_full = self.afr_at_volts(ADC_REFERENCE_VOLTS);
        format!(
            "{:.2}–{:.2} AFR over 0–{:.0} V ({:.4} AFR/V)",
            at_zero,
            at_full,
            ADC_REFERENCE_VOLTS,
            self.slope()
        )
    }
}

/// A three-point Steinhart–Hart thermistor fit (`thermGenerator`).
///
/// `1/T = A + B·ln(R) + C·ln(R)³`, with T in kelvin. The INI's `thermOption`
/// presets supply the three (temperature, resistance) points and the bias
/// resistor that forms the divider against the 5 V rail.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThermistorFit {
    /// Steinhart–Hart A coefficient.
    pub a: f64,
    /// Steinhart–Hart B coefficient.
    pub b: f64,
    /// Steinhart–Hart C coefficient.
    pub c: f64,
    /// Bias resistor in the divider, ohms.
    pub bias_resistor: f64,
}

impl ThermistorFit {
    /// Solve the coefficients from three (temperature °C, resistance Ω)
    /// points plus the bias resistor.
    pub fn from_points(
        bias_resistor: f64,
        points: [(f64, f64); 3],
    ) -> Result<Self, CalibrationError> {
        let mut l = [0.0f64; 3];
        let mut y = [0.0f64; 3];
        for (i, (temp_c, res)) in points.iter().enumerate() {
            if *res <= 0.0 {
                return Err(CalibrationError::DegenerateInputs(format!(
                    "thermistor resistance must be positive (point {} was {res} Ω)",
                    i + 1
                )));
            }
            l[i] = res.ln();
            y[i] = 1.0 / (temp_c + 273.15);
        }

        // Standard three-point Steinhart-Hart solution.
        let g2 = (y[1] - y[0]) / (l[1] - l[0]);
        let g3 = (y[2] - y[0]) / (l[2] - l[0]);
        let denom = (l[2] - l[1]) * (l[0] + l[1] + l[2]);
        if !denom.is_finite() || denom.abs() < 1e-12 {
            return Err(CalibrationError::DegenerateInputs(
                "the three thermistor points are collinear in log-resistance \
                 and cannot be fitted"
                    .to_string(),
            ));
        }
        let c = (g3 - g2) / denom;
        let b = g2 - c * (l[0] * l[0] + l[0] * l[1] + l[1] * l[1]);
        let a = y[0] - (b + l[0] * l[0] * c) * l[0];

        Ok(Self {
            a,
            b,
            c,
            bias_resistor,
        })
    }

    /// Build a fit from an INI `thermOption` preset.
    pub fn from_option(option: &ThermistorOption) -> Result<Self, CalibrationError> {
        Self::from_points(option.bias_resistor, option.points)
    }

    /// Temperature (°C) for a given thermistor resistance.
    pub fn temperature_at_resistance(&self, resistance: f64) -> f64 {
        let ln_r = resistance.max(1e-6).ln();
        let inv_t = self.a + self.b * ln_r + self.c * ln_r * ln_r * ln_r;
        1.0 / inv_t - 273.15
    }

    /// Thermistor resistance implied by an ADC count, through the divider
    /// `Vout = 5 · Rtherm / (Rbias + Rtherm)`.
    pub fn resistance_at_adc(&self, adc: u16) -> f64 {
        let volts = adc_to_volts(adc);
        // At the rail the divider is singular; clamp just short of it so the
        // curve stays monotone instead of going infinite or negative.
        let ratio = (volts / ADC_REFERENCE_VOLTS).clamp(1e-6, 1.0 - 1e-6);
        self.bias_resistor * ratio / (1.0 - ratio)
    }

    /// Temperature (°C) the sensor is reading at an ADC count.
    pub fn temperature_at_adc(&self, adc: u16) -> f64 {
        self.temperature_at_resistance(self.resistance_at_adc(adc))
    }

    /// Sample this fit at the ADC bins the firmware assigns, producing the 32
    /// temperatures a temperature calibration write expects.
    pub fn sample_at_bins(&self, bins: &[u16; 32]) -> [f64; 32] {
        let mut out = [0.0f64; 32];
        for (o, bin) in out.iter_mut().zip(bins.iter()) {
            *o = self.temperature_at_adc(*bin);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourteen_point_seven_preset_matches_the_datasheet() {
        // The INI's 14Point7 formula: 10.0001 + adcValue * 0.0097752.
        let curve = evaluate_solution("10.0001 + ( adcValue * 0.0097752 )").unwrap();
        assert_eq!(curve.len(), 1024);
        assert!((curve[0] - 10.0001).abs() < 1e-6);
        // Brief's acceptance check: AFR x 10 at ADC 512 is ~150.
        let at_512 = curve[512] * 10.0;
        assert!(
            (at_512 - 150.0).abs() < 1.0,
            "AFR x 10 at ADC 512 = {at_512}"
        );
        // ... and the byte table over that span is monotone in 100..200.
        let bytes: Vec<u8> = curve.iter().map(|a| (a * 10.0).round() as u8).collect();
        assert!(bytes.windows(2).all(|w| w[1] >= w[0]));
        assert!(bytes[0] >= 100 && bytes[1023] <= 200);
    }

    #[test]
    fn table_lookup_presets_are_refused_not_zeroed() {
        // Writing an all-zero AFR table to an ECU is actively dangerous, so a
        // preset whose .inc file we do not have must fail loudly.
        let err = evaluate_solution(r#"table(adcValue*5/1023 , "nb.inc")"#).unwrap_err();
        assert_eq!(err, CalibrationError::MissingIncludeFile("nb.inc".into()));
    }

    #[test]
    fn linear_wideband_solves_the_measured_na6_case() {
        // Real car: plateaus read 13.60 and 16.90 where the sensor was
        // actually outputting 13.328 (1.666 V) and 16.666 (3.333 V).
        let cal = LinearWideband::new((0.0, 9.69), (5.0, 19.80)).unwrap();
        assert!((cal.afr_at_volts(0.0) - 9.69).abs() < 1e-9);
        assert!((cal.afr_at_volts(5.0) - 19.80).abs() < 1e-9);
        let curve = cal.curve();
        assert_eq!(curve.len(), 1024);
        assert!((curve[0] - 9.69).abs() < 1e-9);
        assert!((curve[1023] - 19.80).abs() < 1e-9);
    }

    #[test]
    fn linear_wideband_rejects_a_single_point() {
        let err = LinearWideband::new((2.5, 14.7), (2.5, 15.0)).unwrap_err();
        assert!(matches!(err, CalibrationError::DegenerateInputs(_)));
    }

    #[test]
    fn thermistor_fit_reproduces_its_own_calibration_points() {
        // The INI's Mazda preset — the NA6's own sensor.
        let fit = ThermistorFit::from_points(
            50000.0,
            [(-40.0, 2022088.0), (21.0, 68273.0), (99.0, 3715.0)],
        )
        .unwrap();
        for (temp_c, res) in [(-40.0, 2022088.0), (21.0, 68273.0), (99.0, 3715.0)] {
            let got = fit.temperature_at_resistance(res);
            assert!(
                (got - temp_c).abs() < 0.01,
                "fit returned {got} °C for {res} Ω, expected {temp_c} °C"
            );
        }
    }

    #[test]
    fn thermistor_curve_falls_as_adc_rises() {
        let fit =
            ThermistorFit::from_points(2490.0, [(-40.0, 100700.0), (30.0, 2238.0), (99.0, 177.0)])
                .unwrap();
        // Higher ADC = higher voltage = higher resistance = colder.
        let cold = fit.temperature_at_adc(900);
        let hot = fit.temperature_at_adc(100);
        assert!(hot > cold, "ADC 100 gave {hot} °C, ADC 900 gave {cold} °C");
    }

    #[test]
    fn adc_endpoints_span_the_reference_rail() {
        assert!((adc_to_volts(0) - 0.0).abs() < 1e-12);
        assert!((adc_to_volts(1023) - 5.0).abs() < 1e-12);
    }
}

#[cfg(test)]
mod afr_correction_tests {
    use super::*;

    #[test]
    fn a_single_point_shifts_the_whole_curve_by_the_error() {
        // ECU showed 14.2 where the reference gauge read 14.7: half an AFR lean.
        let c = AfrCorrection::from_points(&[(14.2, 14.7)]).unwrap();
        assert_eq!(c.gain, 1.0);
        assert!((c.offset - 0.5).abs() < 1e-9);

        let mut curve = vec![10.0, 14.2, 19.5];
        c.apply(&mut curve);
        assert!((curve[0] - 10.5).abs() < 1e-9);
        assert!(
            (curve[1] - 14.7).abs() < 1e-9,
            "the entered point maps exactly"
        );
        assert!((curve[2] - 20.0).abs() < 1e-9);
    }

    /// The NA6's Spartan case: reads low, and by more at the lean end - an
    /// error that grows across the range, which one offset cannot express.
    #[test]
    fn two_points_solve_gain_and_offset_and_both_map_exactly() {
        // ECU showed 12.5 where the reference said 12.08, and 19.0 where it
        // said 18.25 (the measured-twice bench figures for this sensor).
        let c = AfrCorrection::from_points(&[(12.5, 12.08), (19.0, 18.25)]).unwrap();

        let mut probe = vec![12.5, 19.0, 14.7];
        c.apply(&mut probe);
        assert!((probe[0] - 12.08).abs() < 1e-9, "first point maps exactly");
        assert!((probe[1] - 18.25).abs() < 1e-9, "second point maps exactly");
        // Between the points the correction interpolates: stoich lands between
        // the two entered errors, not at either one.
        assert!(probe[2] < 14.7 && probe[2] > 14.0, "got {}", probe[2]);
    }

    #[test]
    fn identical_measured_readings_cannot_solve_a_gain() {
        let e = AfrCorrection::from_points(&[(14.7, 14.5), (14.7, 15.0)]).unwrap_err();
        assert!(e.to_string().contains("must differ"), "{e}");
    }

    /// Swapped or mistyped pairs produce a negative gain - richer reading as
    /// leaner. Refused with an error that says what to check, because writing
    /// a reversed AFR curve to a car is how a lean condition gets fueled
    /// leaner.
    #[test]
    fn a_reversing_correction_is_refused() {
        let e = AfrCorrection::from_points(&[(12.0, 18.0), (18.0, 12.0)]).unwrap_err();
        assert!(e.to_string().contains("reverse"), "{e}");
    }

    #[test]
    fn zero_and_three_points_are_refused() {
        assert!(AfrCorrection::from_points(&[]).is_err());
        assert!(AfrCorrection::from_points(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]).is_err());
    }
}
