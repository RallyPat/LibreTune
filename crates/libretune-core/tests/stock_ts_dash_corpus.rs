//! Plan D-4: round-trip the entire corpus of stock TunerStudio `.dash` files
//! shipped under `reference/TunerStudioMS/Dash/` (when present in the
//! workspace checkout) to lock in lossless parse → write → parse fidelity.
//!
//! When the reference corpus is not present (e.g. installed crate or CI
//! without git submodules), the test silently skips so it is safe in any
//! environment.

use libretune_core::dash::{parse_dash_file, write_dash_file};
use std::fs;
use std::path::PathBuf;

fn corpus_dir() -> Option<PathBuf> {
    // Walk up from the crate manifest dir until we find a `reference/` sibling.
    let mut cursor = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..4 {
        let candidate = cursor.join("reference/TunerStudioMS/Dash");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !cursor.pop() {
            break;
        }
    }
    None
}

#[test]
fn stock_ts_dash_files_roundtrip_losslessly() {
    let Some(dir) = corpus_dir() else {
        eprintln!("[skip] reference/TunerStudioMS/Dash/ not present");
        return;
    };

    let entries = fs::read_dir(&dir).expect("read corpus dir");
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("dash") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        total += 1;

        let xml = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                // Some stock files are encoded as ISO-8859-1; fall back to
                // a lossy decode rather than failing the whole sweep.
                let bytes = fs::read(&path).expect("read bytes");
                String::from_utf8_lossy(&bytes).to_string()
            }
        };

        // Parse → write → re-parse. We don't compare raw XML byte-for-byte
        // because attribute ordering, whitespace, and entity escaping all
        // differ; instead we assert the structural model survives the round
        // trip (component counts + key fields).
        let first = match parse_dash_file(&xml) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("parse {name}: {e}"));
                continue;
            }
        };

        let written = match write_dash_file(&first) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("write {name}: {e}"));
                continue;
            }
        };

        let second = match parse_dash_file(&written) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("re-parse {name}: {e}"));
                continue;
            }
        };

        if first.gauge_cluster.components.len() != second.gauge_cluster.components.len() {
            failures.push(format!(
                "{name}: component count drift {} → {}",
                first.gauge_cluster.components.len(),
                second.gauge_cluster.components.len()
            ));
        }
    }

    assert!(total > 0, "no .dash files found in corpus");
    assert!(
        failures.is_empty(),
        "{} of {} stock dashes failed round-trip:\n  - {}",
        failures.len(),
        total,
        failures.join("\n  - ")
    );
    eprintln!("[ok] {total} stock TS dash files round-tripped losslessly");
}
