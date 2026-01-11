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
/// Note: This tests rusEFI specifically, NOT FOME or epicEFI (which are separate projects)
#[test]
fn test_rusefi_ini_fields() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!("Corpus directory not found, skipping test");
        return;
    }

    // Find a rusEFI file (exclude FOME and epicECU variants)
    let rusefi_file = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus")
        .filter_map(|e| e.ok())
        .find(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("rusEFI") 
                && !name.contains("FOME") 
                && !name.contains("epicECU")
                && !name.contains("epicEFI")
        })
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

/// Test that FOME INI files parse correctly and have expected fields
/// FOME is a separate project from rusEFI with its own firmware
#[test]
fn test_fome_ini_fields() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!("Corpus directory not found, skipping test");
        return;
    }

    // Find ALL FOME files and test each one
    let fome_files: Vec<_> = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.contains("FOME")
        })
        .map(|e| e.path())
        .collect();

    assert!(
        !fome_files.is_empty(),
        "No FOME INI files found in corpus"
    );

    println!("Testing {} FOME INI files...", fome_files.len());

    for path in &fome_files {
        let filename = path.file_name().unwrap().to_string_lossy();
        let def = EcuDefinition::from_file(path)
            .expect(&format!("Should parse FOME INI: {}", filename));

        // FOME files should have VeAnalyze section
        assert!(
            def.ve_analyze.is_some(),
            "FOME {} should have VeAnalyze section",
            filename
        );

        // Should have output channels
        assert!(
            !def.output_channels.is_empty(),
            "FOME {} should have output channels",
            filename
        );

        // Should have constants
        assert!(
            !def.constants.is_empty(),
            "FOME {} should have constants",
            filename
        );

        // Should have gauges
        assert!(
            !def.gauges.is_empty(),
            "FOME {} should have gauges",
            filename
        );

        // Should have tables
        assert!(
            !def.tables.is_empty(),
            "FOME {} should have tables",
            filename
        );

        println!("FOME INI successfully parsed: {}", filename);
        println!("  Signature: {}", def.signature);
        println!("  Constants: {}", def.constants.len());
        println!("  Output channels: {}", def.output_channels.len());
        println!("  Gauges: {}", def.gauges.len());
        println!("  Tables: {}", def.tables.len());
    }

    println!("\nAll {} FOME INI files passed validation", fome_files.len());
}

/// Test that epicEFI INI files parse correctly and have expected fields
/// epicEFI is a separate project from rusEFI with its own hardware and firmware
#[test]
fn test_epicefi_ini_fields() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!("Corpus directory not found, skipping test");
        return;
    }

    // Find ALL epicEFI/epicECU files and test each one
    let epic_files: Vec<_> = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.contains("epicECU") || name.contains("epicEFI")
        })
        .map(|e| e.path())
        .collect();

    assert!(
        !epic_files.is_empty(),
        "No epicEFI INI files found in corpus"
    );

    println!("Testing {} epicEFI INI files...", epic_files.len());

    // Test a sample of files to avoid very long test times (there are many epicECU files)
    let sample_size = std::cmp::min(10, epic_files.len());
    let sample: Vec<_> = epic_files.iter().take(sample_size).collect();

    for path in &sample {
        let filename = path.file_name().unwrap().to_string_lossy();
        let def = EcuDefinition::from_file(path)
            .expect(&format!("Should parse epicEFI INI: {}", filename));

        // Should have output channels
        assert!(
            !def.output_channels.is_empty(),
            "epicEFI {} should have output channels",
            filename
        );

        // Should have constants
        assert!(
            !def.constants.is_empty(),
            "epicEFI {} should have constants",
            filename
        );

        // Should have gauges
        assert!(
            !def.gauges.is_empty(),
            "epicEFI {} should have gauges",
            filename
        );

        println!("epicEFI INI successfully parsed: {}", filename);
        println!("  Signature: {}", def.signature);
        println!("  Constants: {}", def.constants.len());
        println!("  Output channels: {}", def.output_channels.len());
        println!("  Gauges: {}", def.gauges.len());
    }

    println!(
        "\nSampled {} of {} epicEFI INI files passed validation",
        sample_size,
        epic_files.len()
    );
}

/// Test MegaSquirt INI files (MS2/MS3) with multi-page support
#[test]
fn test_megasquirt_ini_fields() {
    let corpus_path = corpus_dir();
    if !corpus_path.exists() {
        println!("Corpus directory not found, skipping test");
        return;
    }

    // Find MS2 and MS3 INI files
    let ms_files: Vec<_> = fs::read_dir(&corpus_path)
        .expect("Failed to read corpus directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("MS2") || name.starts_with("MS3")
        })
        .map(|e| e.path())
        .collect();

    if ms_files.is_empty() {
        println!("No MegaSquirt INI files found in corpus, skipping test");
        return;
    }

    println!("Testing {} MegaSquirt INI files...", ms_files.len());

    for path in &ms_files {
        let filename = path.file_name().unwrap().to_string_lossy();
        let def = EcuDefinition::from_file(path)
            .expect(&format!("Should parse MegaSquirt INI: {}", filename));

        // MegaSquirt typically has multiple pages
        println!("MegaSquirt INI successfully parsed: {}", filename);
        println!("  Signature: {}", def.signature);
        println!("  nPages: {}", def.n_pages);
        println!("  pageSizes: {:?}", def.page_sizes);
        println!("  Constants: {}", def.constants.len());
        println!("  Output channels: {}", def.output_channels.len());

        // Verify page numbers are 0-based after normalization
        // Find a constant and check its page number
        if let Some((name, constant)) = def.constants.iter().next() {
            println!(
                "  Sample constant '{}' on page {} (0-based)",
                name, constant.page
            );
            // Page should be 0-based (0, 1, etc.) not 1-based
            assert!(
                constant.page < def.n_pages || def.n_pages == 0,
                "Constant '{}' page {} should be less than nPages {}",
                name,
                constant.page,
                def.n_pages
            );
        }
    }

    println!(
        "\nAll {} MegaSquirt INI files passed validation",
        ms_files.len()
    );
}
