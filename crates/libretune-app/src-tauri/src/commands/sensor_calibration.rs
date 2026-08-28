//! Sensor calibration commands (Speeduino calibration space).
//!
//! These drive the dedicated `t` calibration command implemented in
//! `libretune_core::protocol::calibration` — the CLT/IAT thermistor curves
//! and the O2/AFR transfer table live outside the tune pages, so ordinary
//! page writes can never reach them.
//!
//! What the UI can do through here:
//!
//! * list the calibration presets the loaded INI declares, and preview any of
//!   them as a curve ([`list_calibration_presets`], [`preview_afr_calibration`]);
//! * solve a corrected calibration from a wideband's power-on self-test
//!   ([`auto_calibrate_afr`]);
//! * write a calibration and get the CRC read-back result
//!   ([`write_afr_calibration`], [`write_temperature_calibration`]).

use crate::state::AppState;
use libretune_core::calibration::{
    self, adc_to_volts, auto_calibrate_from_plateaus, AfrSample, LinearWideband,
    PlateauDetectorConfig, ThermistorFit,
};
use libretune_core::ini::{CalibrationSolution, IncTableCache};
use libretune_core::protocol::calibration::{
    temperature_calibration_bins, O2_CALIBRATION_WIRE_BYTES, TEMP_CALIBRATION_POINTS,
};
use libretune_core::protocol::CalibrationTable;

/// Firmware table id for the O2/AFR curve.
const O2_TABLE_ID: u8 = 2;

fn parse_temperature_sensor(sensor: &str) -> Result<CalibrationTable, String> {
    match sensor.to_ascii_lowercase().as_str() {
        "clt" => Ok(CalibrationTable::Clt),
        "iat" => Ok(CalibrationTable::Iat),
        other => Err(format!(
            "unknown temperature sensor '{other}' (expected 'clt' or 'iat')"
        )),
    }
}

/// Refuse a calibration write while the engine is turning.
///
/// The firmware's `t` handler flushes serial and blocks in a receive loop,
/// writing EEPROM as it goes — `receiveCalibration()` on the legacy path
/// issues a full EEPROM write *inside* its 32-iteration loop. On a running
/// engine that stalls the main loop long enough to matter, which is why the
/// firmware assumes the engine is stopped.
///
/// Note this gate is on the *write* only. Plateau detection deliberately
/// tolerates a running engine, because the sensor's self-test overlaps
/// cranking and early idle — see the `calibration::plateau` module docs.
async fn require_engine_stopped(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    // No live stream means there is nothing running to protect; a quiet ECU
    // is the normal case for a calibration write.
    let Ok(snapshot) = crate::commands::realtime_get::get_realtime_data(state.clone()).await else {
        return Ok(());
    };
    let rpm = snapshot
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("rpm"))
        .map(|(_, v)| *v);
    match rpm {
        Some(rpm) if rpm > 0.0 => Err(format!(
            "The engine is running ({rpm:.0} rpm). A calibration write blocks \
             the ECU's main loop while it burns EEPROM, so it must be done \
             with the engine stopped (ignition on is fine)."
        )),
        _ => Ok(()),
    }
}

/// Outcome of a calibration write, including the read-back verification that
/// TunerStudio does not surface.
#[derive(serde::Serialize)]
pub struct CalibrationWriteResult {
    /// Which table was written.
    pub table: String,
    /// True when the ECU's stored CRC32 matched the bytes we sent.
    pub verified: bool,
    /// Why verification was not possible, when `verified` is false without an
    /// outright error — the legacy protocol has no calibration read-back.
    pub verify_note: Option<String>,
    /// The effective transfer function, stored so logs are self-describing.
    pub transfer_function: Option<String>,
}

/// The ADC bins the connected ECU will assign to a 32-point temperature
/// calibration, so the frontend can sample its sensor curve at exactly the
/// points the firmware will use.
///
/// The two protocol paths genuinely differ at firmware 202501 — the legacy
/// receiver assigns `x*32` and the CRC one `x*33` — so this is reported per
/// connection rather than assumed. Disconnected callers get the legacy bins
/// for preview purposes.
#[derive(serde::Serialize)]
pub struct CalibrationBinsInfo {
    pub bins: Vec<u16>,
    pub modern_protocol: bool,
    pub connected: bool,
}

#[tauri::command]
pub async fn get_temperature_calibration_bins(
    state: tauri::State<'_, AppState>,
) -> Result<CalibrationBinsInfo, String> {
    let conn_guard = state.connection.lock().await;
    let modern = conn_guard
        .as_ref()
        .map(|c| c.is_modern_protocol())
        .unwrap_or(false);
    Ok(CalibrationBinsInfo {
        bins: temperature_calibration_bins(modern).to_vec(),
        modern_protocol: modern,
        connected: conn_guard.is_some(),
    })
}

/// One selectable wideband preset from the INI.
#[derive(serde::Serialize)]
pub struct AfrPreset {
    pub label: String,
    /// The formula, when the preset has one.
    pub expression: Option<String>,
    /// The generator name, when the preset defers to an interactive editor.
    pub generator: Option<String>,
    /// False when the preset cannot currently be turned into a curve (its
    /// `.inc` lookup file is missing, or it needs a generator). The UI can
    /// still list it, but must not offer a one-click write.
    pub usable: bool,
    /// Why it is unusable, for a tooltip.
    pub note: Option<String>,
}

/// A thermistor preset from the INI, ready for the wizard.
#[derive(serde::Serialize)]
pub struct ThermPreset {
    pub name: String,
    pub bias_resistor: f64,
    pub points: Vec<(f64, f64)>,
}

/// Everything the calibration dialog needs from the INI.
#[derive(serde::Serialize)]
pub struct CalibrationPresets {
    /// Menu label of the AFR block, e.g. "Calibrate AFR Table...".
    pub afr_label: Option<String>,
    pub afr_presets: Vec<AfrPreset>,
    /// Default two points for the custom-linear editor, from the INI's
    /// `linearGenerator` bounds when it declares them.
    pub linear_defaults: Option<(f64, f64, f64, f64)>,
    pub therm_label: Option<String>,
    pub therm_presets: Vec<ThermPreset>,
}

/// Build an `.inc` lookup cache rooted at the current project, so the
/// non-linear presets resolve when their files are present.
async fn inc_table_cache(state: &tauri::State<'_, AppState>) -> IncTableCache {
    let mut paths = Vec::new();
    if let Some(project) = state.current_project.lock().await.as_ref() {
        paths.push(project.path.clone());
        paths.push(project.path.join("projectCfg"));
    }
    IncTableCache::new(paths)
}

/// List the calibration presets the loaded INI declares.
///
/// Read from the INI's `[ReferenceTables]` blocks rather than hardcoded, so a
/// different Speeduino build — or a MegaSquirt INI, which declares different
/// tables entirely — brings its own list.
#[tauri::command]
pub async fn list_calibration_presets(
    state: tauri::State<'_, AppState>,
) -> Result<CalibrationPresets, String> {
    let mut cache = inc_table_cache(&state).await;

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let afr_block = def
        .reference_tables
        .values()
        .find(|t| t.has_identifier(O2_TABLE_ID));
    let therm_block = def
        .reference_tables
        .values()
        .find(|t| t.has_identifier(0) || t.has_identifier(1));

    let afr_presets = afr_block
        .map(|block| {
            block
                .solutions
                .iter()
                .map(|(label, solution)| match solution {
                    CalibrationSolution::Expression { expression } => {
                        // Evaluate now so a preset whose lookup file is
                        // missing shows as unusable, rather than failing at
                        // the moment the user clicks Write.
                        let err =
                            calibration::evaluate_solution_with_tables(expression, &mut cache)
                                .err();
                        AfrPreset {
                            label: label.clone(),
                            expression: Some(expression.clone()),
                            generator: None,
                            usable: err.is_none(),
                            note: err.map(|e| e.to_string()),
                        }
                    }
                    CalibrationSolution::Generator { generator } => AfrPreset {
                        label: label.clone(),
                        expression: None,
                        generator: Some(generator.clone()),
                        usable: false,
                        note: Some(format!("uses the '{generator}' editor")),
                    },
                })
                .collect()
        })
        .unwrap_or_default();

    let linear_defaults = afr_block
        .and_then(|b| b.generators.iter().find(|g| g.kind == "linearGenerator"))
        .and_then(|g| g.bounds);

    let therm_presets = therm_block
        .map(|block| {
            block
                .therm_options
                .iter()
                .map(|o| ThermPreset {
                    name: o.name.clone(),
                    bias_resistor: o.bias_resistor,
                    points: o.points.to_vec(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(CalibrationPresets {
        afr_label: afr_block.map(|b| b.label.clone()),
        afr_presets,
        linear_defaults,
        therm_label: therm_block.map(|b| b.label.clone()),
        therm_presets,
    })
}

/// A calibration curve for preview and for writing.
#[derive(serde::Serialize)]
pub struct AfrCurve {
    /// AFR at each of the 1024 ADC counts.
    pub afr: Vec<f64>,
    /// Sparse `(volts, afr)` points for plotting.
    pub preview: Vec<(f64, f64)>,
    /// Human-readable transfer function.
    pub transfer_function: String,
    /// True when the curve would clip against the 0.0–25.5 AFR the wire
    /// format can carry.
    pub clips: bool,
}

fn describe_curve(afr: &[f64]) -> AfrCurve {
    let preview = (0..=32)
        .map(|i| {
            let adc = (i * 1023 / 32) as u16;
            (adc_to_volts(adc), afr[adc as usize])
        })
        .collect();
    let (lo, hi) = (afr[0], afr[afr.len() - 1]);
    AfrCurve {
        clips: afr.iter().any(|a| *a < 0.0 || *a > 25.5),
        preview,
        transfer_function: format!("{lo:.2}–{hi:.2} AFR over 0–5 V"),
        afr: afr.to_vec(),
    }
}

/// Evaluate an INI preset, or a two-point custom linear calibration, into a
/// previewable curve. Exactly one of `preset` / `linear` should be given.
#[tauri::command]
pub async fn preview_afr_calibration(
    state: tauri::State<'_, AppState>,
    preset: Option<String>,
    linear: Option<((f64, f64), (f64, f64))>,
    correction: Option<Vec<(f64, f64)>>,
) -> Result<AfrCurve, String> {
    // The base curve: the INI preset, or the manual two-point line.
    let (mut afr, mut transfer): (Vec<f64>, Option<String>) = if let Some((p1, p2)) = linear {
        let cal = LinearWideband::new(p1, p2).map_err(|e| e.to_string())?;
        (cal.curve(), Some(cal.describe()))
    } else {
        let label = preset.ok_or("no preset or custom linear points given")?;
        let mut cache = inc_table_cache(&state).await;

        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let block = def
            .reference_tables
            .values()
            .find(|t| t.has_identifier(O2_TABLE_ID))
            .ok_or("this INI declares no AFR calibration table")?;

        let curve = calibration::solution_curve(block, &label, &mut cache)
            .ok_or_else(|| format!("no calibration preset named '{label}'"))?
            .map_err(|e| e.to_string())?;
        (curve, None)
    };

    // An optional reference correction over the base: one (measured, expected)
    // pair shifts offset, two solve gain and offset. Applied HERE rather than
    // in the dialog so the plotted curve and the written curve cannot drift
    // apart - the dialog writes exactly what this returns.
    if let Some(points) = correction.filter(|p| !p.is_empty()) {
        let corr = calibration::AfrCorrection::from_points(&points).map_err(|e| e.to_string())?;
        corr.apply(&mut afr);
        let suffix = corr.describe();
        transfer = Some(match transfer {
            Some(t) => format!("{t}; {suffix}"),
            None => suffix,
        });
    }

    let mut curve = describe_curve(&afr);
    if let Some(t) = transfer {
        curve.transfer_function = t;
    }
    Ok(curve)
}

/// The corrected calibration solved from a captured key-on trace.
#[derive(serde::Serialize)]
pub struct AutoCalResult {
    pub point1: (f64, f64),
    pub point2: (f64, f64),
    pub observed_afr: (f64, f64),
    pub observed_volts: (f64, f64),
    pub ground_offset_volts: f64,
    pub offset_consistency_volts: f64,
    pub windows: ((f64, f64), (f64, f64)),
    pub description: String,
    pub curve: AfrCurve,
}

/// Solve a corrected wideband calibration from a captured key-on trace.
///
/// This is the thing TunerStudio cannot do. A 14Point7 Spartan 2 drives two
/// known voltages at power-on; whatever the ECU read during those windows,
/// versus what the sensor actually sent, is the error in the wiring — mostly
/// ground offset. Feed in the AFR channel across a key-on and the corrected
/// two-point calibration comes back, ready to preview and write.
///
/// `current_curve` is the calibration that was loaded when the trace was
/// captured; without it the observed AFR readings cannot be turned back into
/// voltages. Pass the curve of the preset currently selected in the ECU.
#[tauri::command]
pub async fn auto_calibrate_afr(
    samples: Vec<(f64, f64)>,
    current_curve: Vec<f64>,
) -> Result<AutoCalResult, String> {
    if current_curve.len() != O2_CALIBRATION_WIRE_BYTES {
        return Err(format!(
            "the currently loaded calibration curve must have \
             {O2_CALIBRATION_WIRE_BYTES} points, got {}",
            current_curve.len()
        ));
    }
    let samples: Vec<AfrSample> = samples
        .into_iter()
        .map(|(time_s, afr)| AfrSample { time_s, afr })
        .collect();

    let solved =
        auto_calibrate_from_plateaus(&samples, &current_curve, &PlateauDetectorConfig::default())
            .map_err(|e| e.to_string())?;

    let mut curve = describe_curve(&solved.calibration.curve());
    curve.transfer_function = solved.calibration.describe();

    Ok(AutoCalResult {
        point1: solved.calibration.point1,
        point2: solved.calibration.point2,
        observed_afr: solved.observed_afr,
        observed_volts: solved.observed_volts,
        ground_offset_volts: solved.ground_offset_volts,
        offset_consistency_volts: solved.offset_consistency_volts,
        windows: solved.windows,
        description: solved.describe(),
        curve,
    })
}

/// Write the 1024-entry O2/AFR transfer curve (AFR per 10-bit ADC count).
///
/// The write is followed by a CRC read-back on the CRC protocol, so the
/// returned `verified` flag means the ECU's stored table really matches what
/// was sent — not merely that the frames were acknowledged.
#[tauri::command]
pub async fn write_afr_calibration(
    state: tauri::State<'_, AppState>,
    afr_values: Vec<f64>,
    transfer_function: Option<String>,
) -> Result<CalibrationWriteResult, String> {
    let afr: Box<[f64; O2_CALIBRATION_WIRE_BYTES]> = afr_values
        .into_boxed_slice()
        .try_into()
        .map_err(|v: Box<[f64]>| {
            format!(
                "expected {O2_CALIBRATION_WIRE_BYTES} AFR values, got {}",
                v.len()
            )
        })?;

    require_engine_stopped(&state).await?;

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    let modern = conn.is_modern_protocol();
    conn.write_o2_calibration(&afr).map_err(|e| e.to_string())?;
    tracing::info!("AFR sensor calibration written (1024 points, verified={modern})");

    Ok(CalibrationWriteResult {
        table: "o2".to_string(),
        verified: modern,
        verify_note: (!modern).then(|| {
            "This ECU is on the legacy protocol, which has no calibration \
             read-back, so the write could not be verified. Firmware newer \
             than 202501 ignores legacy calibration writes entirely."
                .to_string()
        }),
        transfer_function,
    })
}

/// Write a 32-point CLT or IAT calibration curve to the ECU.
///
/// `temps_c` are °C at the bins reported by
/// [`get_temperature_calibration_bins`].
#[tauri::command]
pub async fn write_temperature_calibration(
    state: tauri::State<'_, AppState>,
    sensor: String,
    temps_c: Vec<f64>,
) -> Result<CalibrationWriteResult, String> {
    let table = parse_temperature_sensor(&sensor)?;
    let temps: [f64; TEMP_CALIBRATION_POINTS] = temps_c.try_into().map_err(|v: Vec<f64>| {
        format!(
            "expected {TEMP_CALIBRATION_POINTS} temperatures, got {}",
            v.len()
        )
    })?;

    require_engine_stopped(&state).await?;

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    let modern = conn.is_modern_protocol();
    conn.write_temperature_calibration(table, &temps)
        .map_err(|e| e.to_string())?;
    tracing::info!("sensor calibration written: {sensor} (32 points)");

    let lo = temps.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Ok(CalibrationWriteResult {
        table: sensor.to_ascii_lowercase(),
        verified: modern,
        verify_note: (!modern).then(|| {
            "This ECU is on the legacy protocol, which has no calibration \
             read-back, so the write could not be verified."
                .to_string()
        }),
        transfer_function: Some(format!("{lo:.1}–{hi:.1} °C across 32 ADC bins")),
    })
}

/// Build a 32-point temperature curve from three (temperature °C,
/// resistance Ω) points and a bias resistor, sampled at the bins this
/// connection's protocol will use.
///
/// This is the back end for the thermistor wizard: its Steinhart–Hart fit and
/// the firmware's bin assignment have to agree, or the curve is written to
/// the wrong ADC points.
#[tauri::command]
pub async fn build_thermistor_curve(
    state: tauri::State<'_, AppState>,
    bias_resistor: f64,
    points: Vec<(f64, f64)>,
) -> Result<Vec<f64>, String> {
    let points: [(f64, f64); 3] = points
        .try_into()
        .map_err(|v: Vec<(f64, f64)>| format!("expected 3 calibration points, got {}", v.len()))?;
    let fit = ThermistorFit::from_points(bias_resistor, points).map_err(|e| e.to_string())?;

    let modern = state
        .connection
        .lock()
        .await
        .as_ref()
        .map(|c| c.is_modern_protocol())
        .unwrap_or(false);
    let bins = temperature_calibration_bins(modern);
    Ok(fit.sample_at_bins(&bins).to_vec())
}
