//! Corpus test suite - parses all INI files in reference/ecuDef/
//! and reports any parsing errors

use libretune_core::ini::EcuDefinition;
use std::fs;
use std::path::PathBuf;

/// Get the path to the corpus directory
fn corpus_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // project root
        .join("reference")
        .join("ecuDef")
}

/// Test that all INI files in the corpus can be parsed without errors
#[test]
fn test_parse_all_corpus_inis() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!(
            "Corpus directory not found at {:?}, skipping test",
            corpus_path
        );
        return;
    }

    let mut files_tested = 0;
    let mut files_passed = 0;
    let mut errors: Vec<(String, String)> = Vec::new();

    for entry in fs::read_dir(&corpus_path).expect("Failed to read corpus directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "ini") {
            files_tested += 1;
            let filename = path.file_name().unwrap().to_string_lossy().to_string();

            match EcuDefinition::from_file(&path) {
                Ok(_) => {
                    files_passed += 1;
                }
                Err(e) => {
                    errors.push((filename.clone(), e.to_string()));
                    eprintln!("FAIL: {} - {}", filename, e);
                }
            }
        }
    }

    println!("\n=== Corpus Test Results ===");
    println!("Files tested: {}", files_tested);
    println!(
        "Files passed: {} ({:.1}%)",
        files_passed,
        (files_passed as f64 / files_tested as f64) * 100.0
    );
    println!("Files failed: {}", errors.len());

    if !errors.is_empty() {
        println!("\n=== Errors ===");
        for (file, error) in &errors {
            println!("  {} - {}", file, error);
        }
    }

    // We want 100% pass rate for spec compliance
    assert!(
        errors.is_empty(),
        "Failed to parse {} out of {} INI files. Errors:\n{}",
        errors.len(),
        files_tested,
        errors
            .iter()
            .map(|(f, e)| format!("  {}: {}", f, e))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Test that rusEFI INI files parse correctly and have expected fields
#[test]
fn test_rusefi_ini_fields() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!("Corpus directory not found, skipping test");
        return;
    }

    // Find a rusEFI file
    let rusefi_file = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus")
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("rusEFI"))
        .map(|e| e.path());

    if let Some(path) = rusefi_file {
        let def = EcuDefinition::from_file(&path).expect("Should parse rusEFI INI");

        // rusEFI files should have VeAnalyze section
        assert!(
            def.ve_analyze.is_some(),
            "rusEFI should have VeAnalyze section"
        );

        // rusEFI files should have ConstantsExtensions (maintainConstantValue entries)
        assert!(
            !def.maintain_constant_values.is_empty(),
            "rusEFI should have maintainConstantValue entries"
        );

        // Should have output channels
        assert!(
            !def.output_channels.is_empty(),
            "Should have output channels"
        );

        // Should have constants
        assert!(!def.constants.is_empty(), "Should have constants");

        // Should have gauges
        assert!(!def.gauges.is_empty(), "Should have gauges");

        println!("rusEFI INI successfully parsed:");
        println!("  Signature: {}", def.signature);
        println!("  Constants: {}", def.constants.len());
        println!("  Output channels: {}", def.output_channels.len());
        println!("  Gauges: {}", def.gauges.len());
        println!("  Tables: {}", def.tables.len());
        println!(
            "  MaintainConstantValue entries: {}",
            def.maintain_constant_values.len()
        );
    }
}

/// Test that Speeduino INI files parse correctly
#[test]
fn test_speeduino_ini_fields() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!("Corpus directory not found, skipping test");
        return;
    }

    // Find a Speeduino file
    let speeduino_file = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus")
        .filter_map(|e| e.ok())
        .find(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.contains("speeduino") || name.contains("202")
        })
        .map(|e| e.path());

    if let Some(path) = speeduino_file {
        let def = EcuDefinition::from_file(&path).expect("Should parse Speeduino INI");

        // Should have output channels
        assert!(
            !def.output_channels.is_empty(),
            "Should have output channels"
        );

        // Should have constants
        assert!(!def.constants.is_empty(), "Should have constants");

        println!("Speeduino INI successfully parsed:");
        println!("  Signature: {}", def.signature);
        println!("  Constants: {}", def.constants.len());
        println!("  Output channels: {}", def.output_channels.len());
    }
}
