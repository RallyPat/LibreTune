//! Compare AutoTune configurations over one or more recorded drives.
//!
//! The command-line face of [`libretune_core::autotune::replay`], the same
//! engine the Log Analyze view runs. Useful for sweeping many settings at once,
//! which the view deliberately does not do.
//!
//! ```text
//! cargo run -p libretune-core --example autotune_replay -- \
//!     --log drive.csv [--log more.csv...] --ve ve.json [--target afr.json]
//! ```
//!
//! Every configuration sees identical samples, so each difference is
//! attributable to the configuration alone - never true when comparing drives.

use libretune_core::autotune::replay::{replay, LogChannels, ReplayConfig};
use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneReferenceTables, AutoTuneSettings,
    HitWeighting,
};
use std::collections::HashMap;

/// Fitted flow-delay model for the reference engine: floor + k/(rpm*MAP).
const DELAY_FLOOR_MS: f64 = 150.0;
const DELAY_K: f64 = 9_971_256.0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let log_paths: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| *a == "--log" && args.len() > i + 1)
        .map(|(i, _)| args[i + 1].clone())
        .collect();
    let Some(ve_path) = get("--ve") else {
        eprintln!(
            "usage: --log <drive.csv> [--log <more.csv>...] --ve <ve.json> [--target <afr.json>]"
        );
        std::process::exit(2);
    };
    if log_paths.is_empty() {
        eprintln!("at least one --log is required");
        std::process::exit(2);
    }

    let ve = read_table(&ve_path).unwrap_or_else(|e| {
        eprintln!("--ve: {e}");
        std::process::exit(2);
    });
    if ve.x.is_empty() || ve.y.is_empty() {
        eprintln!("--ve: that table JSON has no x_bins/y_bins, and axes cannot be guessed:");
        eprintln!("      a table read against the wrong axis is silently wrong.");
        std::process::exit(2);
    }

    // Several drives are concatenated with a gap longer than any delay, so
    // nothing correlates across the seam and each keeps its own clock.
    let mut log = LogChannels::default();
    let mut offset = 0.0f64;
    for p in &log_paths {
        let rows = read_log(p);
        println!("{} rows from {p}", rows.len());
        let last = append(&mut log, &rows, offset);
        offset = last + 60_000.0;
    }
    println!(
        "VE table {}x{}, rpm {}..{}",
        ve.z.len(),
        ve.x.len(),
        ve.x[0],
        ve.x[ve.x.len() - 1]
    );

    // The target has its own axes; indexing it with the VE table's cell numbers
    // is only right if the two agree. The app resamples now, so this does too.
    let target = get("--target").and_then(|p| read_table(&p).ok());
    let target_z = target.as_ref().map(|t| {
        println!(
            "target AFR table {}x{}, axes {}",
            t.z.len(),
            t.z.first().map_or(0, |r| r.len()),
            if t.x == ve.x && t.y == ve.y {
                "match VE"
            } else {
                "DIFFER from VE - resampling"
            }
        );
        resample(t, &ve.x, &ve.y)
    });

    let delay_tbl = flow_delay_table(&ve.x, &ve.y, DELAY_FLOOR_MS, DELAY_K);
    let base = AutoTuneSettings::default();
    // Logs from a Celsius project; the struct default is Fahrenheit and would
    // reject every sample a warm engine can produce.
    let warm = AutoTuneFilters {
        min_clt: 71.0,
        ..Default::default()
    };
    let steady = AutoTuneFilters {
        min_steady_ms: 800,
        ..warm.clone()
    };

    let mut cases: Vec<(String, ReplayConfig, bool)> = Vec::new();
    let mut push = |name: &str, s: AutoTuneSettings, f: AutoTuneFilters, delay: bool| {
        cases.push((
            name.to_string(),
            ReplayConfig {
                settings: s,
                filters: f,
                authority: AutoTuneAuthorityLimits::default(),
                strict_lambda_match: true,
                validate: true,
            },
            delay,
        ));
    };

    push("baseline", base.clone(), warm.clone(), false);
    push("+ delay table", base.clone(), warm.clone(), true);
    for (n, w) in [
        ("  weighting: None", HitWeighting::Uniform),
        ("  weighting: Soft", HitWeighting::CellProximity),
        ("  weighting: Medium", HitWeighting::CellProximitySquared),
        ("  weighting: Hard", HitWeighting::CellCentreOnly),
    ] {
        push(
            n,
            AutoTuneSettings {
                hit_weighting: w,
                ..base.clone()
            },
            warm.clone(),
            false,
        );
    }
    for bw in [0.0, 20.0, 50.0] {
        push(
            &format!("  confidence weight {bw:.0}"),
            AutoTuneSettings {
                base_weight: bw,
                ..base.clone()
            },
            warm.clone(),
            false,
        );
    }
    push("STEADY 800ms", base.clone(), steady.clone(), false);
    push("STEADY + delay", base.clone(), steady.clone(), true);
    push(
        "STEADY + cw50",
        AutoTuneSettings {
            base_weight: 50.0,
            ..base.clone()
        },
        steady.clone(),
        false,
    );
    push(
        "min_clt 160 (F default)",
        base.clone(),
        AutoTuneFilters::default(),
        false,
    );

    println!("\nscored on blocks each configuration never trained on\n");
    println!(
        "{:<26}{:>7}{:>7}{:>9}{:>8}{:>10}{:>8}",
        "config", "used", "changed", "meanAbs", "max-", "AFR gain", "worse"
    );
    println!("{}", "-".repeat(75));

    let mut first_rejects = None;
    for (name, config, delay) in &cases {
        let tables = AutoTuneReferenceTables {
            ve_table: ve.z.clone(),
            target_afr_table: target_z.clone().unwrap_or_default(),
            lambda_delay_table: if *delay {
                delay_tbl.clone()
            } else {
                Vec::new()
            },
        };
        let r = replay(&log, &ve.x, &ve.y, &tables, config);
        let d: Vec<f64> = r
            .cells
            .iter()
            .map(|c| c.delta)
            .filter(|v| v.abs() > 1e-9)
            .collect();
        let mean_abs = if d.is_empty() {
            0.0
        } else {
            d.iter().map(|v| v.abs()).sum::<f64>() / d.len() as f64
        };
        // A config that accepted nothing has nothing to score. Printing NaN%
        // reads as a number; a dash reads as the absence of one.
        let score = r.validation.as_ref().map_or_else(
            || format!("{:>10}{:>8}", "-", "-"),
            |v| format!("{:>9.1}%{:>7.1}%", v.gain_pct, v.worsened_pct),
        );
        println!(
            "{:<26}{:>7}{:>7}{:>9.2}{:>+8.1}{score}",
            name,
            r.total_samples,
            d.len(),
            mean_abs,
            d.iter().copied().fold(0.0, f64::min),
        );
        if first_rejects.is_none() {
            first_rejects = Some(r.rejections.clone());
        }
    }

    if let Some(rj) = first_rejects {
        println!("\nwhy samples were dropped (first config):");
        for (reason, n) in rj.iter().take(8) {
            println!("  {reason:<30}{n:>8}");
        }
    }
    println!(
        "\n'used' is samples that passed the filters. More is not better: a config\n\
         that accepts more may be accepting transients it should have refused."
    );
}

/// Append a parsed log onto `dst`, shifting its clock by `offset` ms.
/// Returns the last timestamp written.
fn append(dst: &mut LogChannels, rows: &[HashMap<String, f64>], offset: f64) -> f64 {
    let pick = |row: &HashMap<String, f64>, names: &[&str]| -> Option<f64> {
        names.iter().find_map(|n| row.get(*n).copied())
    };
    let mut last = offset;
    for row in rows {
        let (Some(rpm), Some(load), Some(afr)) = (
            pick(row, &["rpm", "RPM"]),
            pick(row, &["map", "MAP", "fuelLoad"]),
            pick(row, &["afr", "AFR", "AFR1", "O2"]),
        ) else {
            continue;
        };
        let t = offset + pick(row, &["Time", "time"]).unwrap_or(0.0) * 1000.0;
        dst.time_ms.push(t);
        dst.rpm.push(rpm);
        dst.load.push(load);
        dst.afr.push(afr);
        dst.ve.push(pick(row, &["veCurr", "VE1"]).unwrap_or(0.0));
        dst.clt.push(pick(row, &["coolant", "CLT"]).unwrap_or(90.0));
        dst.tps.push(pick(row, &["tps", "TPS"]).unwrap_or(0.0));
        dst.tps_rate.push(pick(row, &["TPSdot"]).unwrap_or(0.0));
        dst.fuel_cut.push(pick(row, &["DFCOOn"]).unwrap_or(0.0));
        dst.accel_enrich
            .push(pick(row, &["engine"]).map_or(0.0, |v| f64::from((v as i64) & 16 != 0)));
        last = t;
    }
    last
}

fn flow_delay_table(x: &[f64], y: &[f64], floor: f64, k: f64) -> Vec<Vec<f64>> {
    y.iter()
        .map(|&map| {
            x.iter()
                .map(|&rpm| {
                    let flow = rpm * map;
                    if flow <= 0.0 {
                        floor
                    } else {
                        floor + k / flow
                    }
                })
                .collect()
        })
        .collect()
}

struct Table {
    z: Vec<Vec<f64>>,
    x: Vec<f64>,
    y: Vec<f64>,
}

/// Bilinear resample onto the given axes, so a table with its own bins can be
/// indexed by another table's cell numbers without being wrong.
fn resample(t: &Table, x: &[f64], y: &[f64]) -> Vec<Vec<f64>> {
    y.iter()
        .map(|&yy| x.iter().map(|&xx| interp2(t, xx, yy)).collect())
        .collect()
}

fn interp2(t: &Table, x: f64, y: f64) -> f64 {
    let (y0, y1, fy) = span(&t.y, y);
    let r0 = interp1(&t.z[y0], &t.x, x);
    let r1 = interp1(&t.z[y1], &t.x, x);
    r0 + fy * (r1 - r0)
}

fn interp1(row: &[f64], bins: &[f64], v: f64) -> f64 {
    let (i0, i1, f) = span(bins, v);
    row[i0] + f * (row[i1] - row[i0])
}

fn span(bins: &[f64], v: f64) -> (usize, usize, f64) {
    let n = bins.len();
    if v <= bins[0] {
        return (0, 0, 0.0);
    }
    if v >= bins[n - 1] {
        return (n - 1, n - 1, 0.0);
    }
    for i in 0..n - 1 {
        if v <= bins[i + 1] {
            let w = bins[i + 1] - bins[i];
            return (i, i + 1, if w > 0.0 { (v - bins[i]) / w } else { 0.0 });
        }
    }
    (n - 1, n - 1, 0.0)
}

fn read_log(path: &str) -> Vec<HashMap<String, f64>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("read {path}: {e}");
        std::process::exit(2);
    });
    let mut lines = text.lines();
    let Some(h) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<&str> = h.split(',').map(str::trim).collect();
    lines
        .filter_map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            (v.len() == cols.len()).then(|| {
                cols.iter()
                    .zip(v)
                    .filter_map(|(c, x)| x.trim().parse().ok().map(|f: f64| ((*c).to_string(), f)))
                    .collect()
            })
        })
        .collect()
}

fn read_table(path: &str) -> Result<Table, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let one = |k: &str| -> Vec<f64> {
        v.get(k)
            .and_then(|a| a.as_array())
            .map(|c| c.iter().filter_map(serde_json::Value::as_f64).collect())
            .unwrap_or_default()
    };
    let z: Vec<Vec<f64>> = v
        .get("z_values")
        .and_then(|a| a.as_array())
        .map(|r| {
            r.iter()
                .map(|row| {
                    row.as_array()
                        .map(|c| c.iter().filter_map(serde_json::Value::as_f64).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    if z.is_empty() {
        return Err("no z_values".into());
    }
    Ok(Table {
        z,
        x: one("x_bins"),
        y: one("y_bins"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this guards: a table indexed by another table's cell numbers.
    #[test]
    fn resample_puts_values_at_the_right_rpm() {
        let t = Table {
            z: vec![vec![10.0, 20.0], vec![10.0, 20.0]],
            x: vec![1000.0, 2000.0],
            y: vec![0.0, 100.0],
        };
        let out = resample(&t, &[1000.0, 1500.0, 2000.0], &[0.0, 100.0]);
        assert_eq!(
            out[0],
            vec![10.0, 15.0, 20.0],
            "1500 rpm must interpolate, not index"
        );
    }

    #[test]
    fn delay_falls_with_flow() {
        let t = flow_delay_table(&[1000.0, 6000.0], &[20.0, 95.0], DELAY_FLOOR_MS, DELAY_K);
        assert!(t[0][0] > t[1][1], "idle must be slower than high load/rpm");
        assert!(t[1][1] > DELAY_FLOOR_MS, "never below the sensor's floor");
    }

    /// Concatenated drives must not share a clock, or the delay buffer will
    /// match a sample from one against a sample from another.
    #[test]
    fn appending_a_second_log_shifts_its_clock() {
        let rows: Vec<HashMap<String, f64>> = (0..3)
            .map(|i| {
                HashMap::from([
                    ("Time".to_string(), f64::from(i) * 0.1),
                    ("rpm".to_string(), 2000.0),
                    ("map".to_string(), 50.0),
                    ("afr".to_string(), 14.0),
                ])
            })
            .collect();
        let mut log = LogChannels::default();
        let last = append(&mut log, &rows, 0.0);
        append(&mut log, &rows, last + 60_000.0);
        assert_eq!(log.len(), 6);
        assert!(
            log.time_ms[3] - log.time_ms[2] > 50_000.0,
            "the seam must be wider than any transport delay"
        );
    }
}
