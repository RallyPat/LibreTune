//! Curve get/update commands.

use crate::AppState;
use libretune_core::ini::Constant;
use libretune_core::protocol::Connection;
use libretune_core::tune::TuneFile;
use serde::Serialize;

#[derive(Serialize)]
pub struct CurveData {
    pub name: String,
    pub title: String,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub x_label: String,
    pub y_label: String,
    /// X-axis range: (min, max, step)
    pub x_axis: Option<(f32, f32, f32)>,
    /// Y-axis range: (min, max, step)
    pub y_axis: Option<(f32, f32, f32)>,
    /// Output channel name for live cursor (e.g., "coolant")
    pub x_output_channel: Option<String>,
    /// Gauge name for live display
    pub gauge: Option<String>,
}
/// # Arguments
/// * `curve_name` - Curve name from INI definition
///
/// Returns: CurveData with x/y values and metadata
/// Read one constant's values from the loaded tune (offline) or the ECU.
///
/// Every failure returns an error rather than a zero. This used to
/// substitute `0.0` for an element that would not decode and return
/// `vec![0.0; element_count]` for a short page, a missing page, a constant
/// found nowhere, a zero-length read, and the no-tune-no-connection case -
/// five ways to hand `get_curve_data` a flat zero curve that looks like a
/// curve someone flattened. CurveEditor then persists whatever it was
/// shown, so a read that failed becomes a write. A zeroed cranking,
/// warm-up or dwell curve is the shape most likely to hurt if believed.
pub(crate) fn read_const_from_source(
    constant: &Constant,
    tune: Option<&TuneFile>,
    conn: &mut Option<&mut Connection>,
    endianness: libretune_core::ini::Endianness,
) -> Result<Vec<f64>, String> {
    let element_count = constant.shape.element_count();
    let element_size = constant.data_type.size_bytes();
    let length = constant.size_bytes() as u16;

    /// Decode `element_count` elements or say which one stopped us.
    fn decode_all(
        constant: &Constant,
        bytes: &[u8],
        base: usize,
        element_count: usize,
        element_size: usize,
        endianness: libretune_core::ini::Endianness,
        source: &str,
    ) -> Result<Vec<f64>, String> {
        let mut values = Vec::with_capacity(element_count);
        for i in 0..element_count {
            let elem_offset = base + i * element_size;
            let raw_val = constant
                .data_type
                .read_from_bytes(bytes, elem_offset, endianness)
                .ok_or_else(|| {
                    format!(
                        "Element {i} of {element_count} in '{}' could not be decoded from                              {source}. Re-sync and try again.",
                        constant.name
                    )
                })?;
            values.push(constant.raw_to_display(raw_val));
        }
        Ok(values)
    }

    // Offline: read from the tune file.
    if conn.is_none() {
        let tune_file = tune.ok_or_else(|| {
            format!(
                "No tune is loaded and no ECU is connected, so '{}' has no values to show.",
                constant.name
            )
        })?;

        // Named constants parsed from the msq <constant> tags win: they are
        // values that really are in the tune.
        if let Some(tune_value) = tune_file.constants.get(&constant.name) {
            use libretune_core::tune::TuneValue;
            match tune_value {
                TuneValue::Array(arr) => return Ok(arr.clone()),
                TuneValue::Scalar(v) => return Ok(vec![*v]),
                _ => {}
            }
        }

        // Otherwise fall back to raw page data at the INI's offset, for a
        // constant the msq did not name explicitly.
        let page_data = tune_file.pages.get(&constant.page).ok_or_else(|| {
            format!(
                "'{}' is not named in the tune and lives on page {}, which the tune does                      not contain.",
                constant.name, constant.page
            )
        })?;

        let offset = constant.offset as usize;
        let total_bytes = element_count * element_size;
        if offset + total_bytes > page_data.len() {
            return Err(format!(
                "'{}' needs {total_bytes} bytes at offset {offset} of page {}, which holds                      only {}. Re-sync the tune and try again.",
                constant.name,
                constant.page,
                page_data.len()
            ));
        }

        return decode_all(
            constant,
            page_data,
            offset,
            element_count,
            element_size,
            endianness,
            "the tune's page data",
        );
    }

    if length == 0 {
        return Err(format!(
            "'{}' has a zero-byte footprint, so there is nothing to read from the ECU.",
            constant.name
        ));
    }

    let conn_ptr = conn.as_mut().ok_or_else(|| {
        format!(
            "No ECU connection is available to read '{}'.",
            constant.name
        )
    })?;

    let params = libretune_core::protocol::commands::ReadMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        length,
    };
    let raw_data = conn_ptr.read_memory(params).map_err(|e| e.to_string())?;

    // A short reply is the realistic way to get a partial curve off real
    // hardware, and it used to become zeros from the truncation point on.
    if raw_data.len() < element_count * element_size {
        return Err(format!(
            "The ECU returned {} bytes for '{}', which needs {}. Re-sync and try again.",
            raw_data.len(),
            constant.name,
            element_count * element_size
        ));
    }

    decode_all(
        constant,
        &raw_data,
        0,
        element_count,
        element_size,
        endianness,
        "the ECU reply",
    )
}

#[tauri::command]
pub async fn get_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String,
) -> Result<CurveData, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        eprintln!(
            "[WARN] get_curve_data: Definition not loaded when looking for '{}'",
            curve_name
        );
        "Definition not loaded".to_string()
    })?;
    let endianness = def.endianness;

    // Diagnostic logging
    eprintln!(
        "[DEBUG] get_curve_data: Looking for '{}' in {} curves ({} map entries)",
        curve_name,
        def.curves.len(),
        def.curve_map_to_name.len()
    );

    let curve = def.get_curve_by_name_or_map(&curve_name).ok_or_else(|| {
        // Log available curves for debugging
        let available: Vec<_> = def.curves.keys().take(10).cloned().collect();
        eprintln!(
            "[WARN] get_curve_data: Curve '{}' not found. Available curves (first 10): {:?}",
            curve_name, available
        );
        format!(
            "Curve '{}' not found (checked {} curves, {} map entries)",
            curve_name,
            def.curves.len(),
            def.curve_map_to_name.len()
        )
    })?;

    eprintln!(
        "[DEBUG] get_curve_data: Found curve '{}' (title: {})",
        curve.name, curve.title
    );

    // Clone the constant info we need
    let x_const = def
        .constants
        .get(&curve.x_bins)
        .ok_or_else(|| format!("Constant {} not found", curve.x_bins))?
        .clone();
    let y_const = def
        .constants
        .get(&curve.y_bins)
        .ok_or_else(|| format!("Constant {} not found", curve.y_bins))?
        .clone();

    // Clone curve metadata
    let curve_name_out = curve.name.clone();
    let curve_title = curve.title.clone();
    let x_label = curve.column_labels.0.clone();
    let y_label = curve.column_labels.1.clone();
    let x_axis = curve.x_axis;
    let y_axis = curve.y_axis;
    let x_output_channel = curve.x_output_channel.clone();
    let gauge = curve.gauge.clone();

    drop(def_guard);

    // Get tune and connection
    // Lock order: connection before current_tune, matching the convention used
    // by every write path (get_constant_value, update_constant, etc.) — the
    // reverse order deadlocks against those.
    let mut conn_guard = state.connection.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let mut conn = conn_guard.as_mut();

    let x_bins = read_const_from_source(&x_const, tune_guard.as_ref(), &mut conn, endianness)?;
    let y_bins = read_const_from_source(&y_const, tune_guard.as_ref(), &mut conn, endianness)?;

    Ok(CurveData {
        name: curve_name_out,
        title: curve_title,
        x_bins,
        y_bins,
        x_label,
        y_label,
        x_axis,
        y_axis,
        x_output_channel,
        gauge,
    })
}

/// Snapshot of the definition-derived facts `write_constant_array_values`
/// needs, taken before `state.definition`'s lock is dropped (see
/// `update_curve_data`). Bundled into one struct rather than passed as two
/// separate params to stay under clippy's too-many-arguments threshold.
struct WriteContext {
    endianness: libretune_core::ini::Endianness,
    default_page_bytes: usize,
}

fn write_constant_array_values(
    ctx: &WriteContext,
    constant: &libretune_core::ini::Constant,
    values: &[f64],
    cache: &mut libretune_core::tune::TuneCache,
    tune: &mut Option<libretune_core::tune::TuneFile>,
    tune_modified: &mut bool,
    conn: &mut Option<&mut Connection>,
) -> Result<(), String> {
    if values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size for {}: expected {}, got {}",
            constant.name,
            constant.shape.element_count(),
            values.len()
        ));
    }

    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, ctx.endianness);
    }

    if cache.write_bytes(constant.page, constant.offset, &raw_data) {
        if let Some(tune) = tune.as_mut() {
            tune.constants.insert(
                constant.name.clone(),
                libretune_core::tune::TuneValue::Array(values.to_vec()),
            );

            let page_data = tune
                .pages
                .entry(constant.page)
                .or_insert_with(|| vec![0u8; ctx.default_page_bytes]);

            let start = constant.offset as usize;
            let end = start + raw_data.len();
            if end <= page_data.len() {
                page_data[start..end].copy_from_slice(&raw_data);
            }
        }

        *tune_modified = true;
    }

    if let Some(conn) = conn.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data,
        };

        if let Err(e) = conn.write_memory(params) {
            eprintln!(
                "[WARN] Failed to write constant '{}' to ECU: {}",
                constant.name, e
            );
        }
    }

    Ok(())
}

/// Updates curve X and/or Y bin values in the tune cache and optionally writes to ECU.
#[tauri::command]
pub async fn update_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String,
    y_values: Option<Vec<f64>>,
    x_values: Option<Vec<f64>>,
) -> Result<(), String> {
    if y_values.is_none() && x_values.is_none() {
        return Err("No curve values provided".to_string());
    }

    // Snapshot only what we need from the definition, then drop the lock
    // before doing any ECU I/O below — holding it across a blocking
    // conn.write_memory() call starves every other command that needs the
    // definition (e.g. load_tune, table/curve reads). Matches the
    // established pattern in update_constant/update_constant_array_internal.
    let (x_ctx, x_const, y_ctx, y_const) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;

        let curve = def
            .get_curve_by_name_or_map(&curve_name)
            .ok_or_else(|| format!("Curve {} not found", curve_name))?;

        let x_const_name = curve.x_bins.clone();
        let y_const_name = curve.y_bins.clone();
        let x_const = def
            .constants
            .get(&x_const_name)
            .ok_or_else(|| {
                format!(
                    "Constant {} not found for curve {}",
                    x_const_name, curve_name
                )
            })?
            .clone();
        let y_const = def
            .constants
            .get(&y_const_name)
            .ok_or_else(|| {
                format!(
                    "Constant {} not found for curve {}",
                    y_const_name, curve_name
                )
            })?
            .clone();

        let x_ctx = WriteContext {
            endianness: def.endianness,
            default_page_bytes: def
                .page_sizes
                .get(x_const.page as usize)
                .copied()
                .unwrap_or(256) as usize,
        };
        let y_ctx = WriteContext {
            endianness: def.endianness,
            default_page_bytes: def
                .page_sizes
                .get(y_const.page as usize)
                .copied()
                .unwrap_or(256) as usize,
        };

        (x_ctx, x_const, y_ctx, y_const)
    };

    let mut conn_guard = state.connection.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;
    let mut tune_guard = state.current_tune.lock().await;
    let mut modified_guard = state.tune_modified.lock().await;

    let cache = cache_guard
        .as_mut()
        .ok_or("Tune cache not initialized — open or create a project first")?;
    let mut conn = conn_guard.as_mut();

    if let Some(values) = x_values {
        write_constant_array_values(
            &x_ctx,
            &x_const,
            &values,
            cache,
            &mut tune_guard,
            &mut modified_guard,
            &mut conn,
        )?;
    }

    if let Some(values) = y_values {
        write_constant_array_values(
            &y_ctx,
            &y_const,
            &values,
            cache,
            &mut tune_guard,
            &mut modified_guard,
            &mut conn,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod read_const_from_source_tests {
    use super::*;
    use libretune_core::ini::{DataType, Endianness, Shape};
    use libretune_core::tune::{TuneFile, TuneValue};

    /// A 4-element U08 curve at offset 2 of page 3.
    fn constant() -> Constant {
        Constant {
            name: "wueCurve".to_string(),
            page: 3,
            offset: 2,
            data_type: DataType::U08,
            scale: 1.0,
            translate: 0.0,
            shape: Shape::Array1D(4),
            ..Default::default()
        }
    }

    fn tune_with_page(bytes: Vec<u8>) -> TuneFile {
        let mut t = TuneFile::default();
        t.pages.insert(3, bytes);
        t
    }

    /// Offline reads take no connection, which is where four of the five
    /// fabrication paths lived.
    fn offline(tune: Option<&TuneFile>) -> Result<Vec<f64>, String> {
        let mut none: Option<&mut Connection> = None;
        read_const_from_source(&constant(), tune, &mut none, Endianness::Big)
    }

    #[test]
    fn a_complete_page_reads_the_real_values() {
        let t = tune_with_page(vec![0, 0, 10, 20, 30, 40]);
        assert_eq!(offline(Some(&t)).unwrap(), vec![10.0, 20.0, 30.0, 40.0]);
    }

    /// The bug: each of these returned a flat zero curve that CurveEditor
    /// showed as real and would persist on the next save.
    #[test]
    fn every_unreadable_case_refuses_instead_of_returning_zeros() {
        // Page present but too short for the curve.
        let short = tune_with_page(vec![0, 0, 10, 20]);
        let e = offline(Some(&short)).expect_err("short page must not answer");
        assert!(e.contains("wueCurve") && e.contains("page 3"), "{e}");

        // Tune loaded, but not the page this curve lives on.
        let mut elsewhere = TuneFile::default();
        elsewhere.pages.insert(9, vec![0; 64]);
        let e = offline(Some(&elsewhere)).expect_err("missing page must not answer");
        assert!(e.contains("page 3"), "{e}");

        // Neither a tune nor a connection.
        let e = offline(None).expect_err("nothing loaded must not answer");
        assert!(e.contains("wueCurve"), "{e}");
    }

    /// Values that really are in the tune still come from there - that path
    /// never fabricated anything and is deliberately unchanged.
    #[test]
    fn a_named_constant_is_served_from_the_tune() {
        let mut t = tune_with_page(vec![0, 0, 10, 20, 30, 40]);
        t.constants.insert(
            "wueCurve".to_string(),
            TuneValue::Array(vec![1.0, 2.0, 3.0, 4.0]),
        );
        assert_eq!(offline(Some(&t)).unwrap(), vec![1.0, 2.0, 3.0, 4.0]);
    }
}
