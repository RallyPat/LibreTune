//! Plan S-5: round-trip the stock TunerStudio `.tuneView` corpus shipped
//! under `reference/TunerStudioMS/TuneView/` (when present in the workspace
//! checkout) to lock in lossless parse → write → parse fidelity.
//!
//! When the reference corpus is not present (e.g. installed crate or CI
//! without git submodules), the test silently skips so it is safe in any
//! environment.

use libretune_core::tune_view::{parse_tune_view, write_tune_view};
use std::fs;
use std::path::PathBuf;

fn corpus_dir() -> Option<PathBuf> {
    let mut cursor = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..4 {
        let candidate = cursor.join("reference/TunerStudioMS/TuneView");
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
fn stock_ts_tune_view_files_roundtrip_losslessly() {
    let Some(dir) = corpus_dir() else {
        eprintln!("[skip] reference/TunerStudioMS/TuneView/ not present");
        return;
    };

    let entries = fs::read_dir(&dir).expect("read corpus dir");
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tuneView") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        total += 1;

        let xml = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                let bytes = fs::read(&path).expect("read bytes");
                String::from_utf8_lossy(&bytes).to_string()
            }
        };

        let parsed = match parse_tune_view(&xml) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!("{name}: parse failed: {e}"));
                continue;
            }
        };

        let serialized = write_tune_view(&parsed);
        let reparsed = match parse_tune_view(&serialized) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!("{name}: re-parse failed: {e}"));
                continue;
            }
        };

        if parsed != reparsed {
            failures.push(format!("{name}: round-trip mismatch"));
        }
    }

    assert!(total > 0, "no .tuneView files in corpus dir");
    assert!(
        failures.is_empty(),
        "{} of {} stock tuneView files failed round-trip:\n{}",
        failures.len(),
        total,
        failures.join("\n")
    );
    eprintln!("[ok] round-tripped {total} stock TS tuneView files");
}
