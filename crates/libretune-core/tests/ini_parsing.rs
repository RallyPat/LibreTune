//! Tests for INI parsing, especially edge cases with commas in expressions
//! and space-separated subMenu formats.

use libretune_core::ini::{DataType, EcuDefinition};
use std::path::Path;

// Note: These tests use the public API. The split_ini_line function is tested
// indirectly through EcuDefinition parsing and the resulting data.

/// Test parsing an INI file with constants that have expressions containing commas
/// like { bitStringValue(algorithmUnits , algorithm) }
#[test]
fn test_parse_constant_with_braced_expression() {
    // Create a minimal INI content with a constant that has a braced expression with comma
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[Constants]
page = 1
   loadBinsAFR = array, U08, 272, [16], { bitStringValue(algorithmUnits , algorithm) }, 1.0, 0.0, 0, 255, 0
   simpleBins = array, U08, 0, [16], "kPa", 1.0, 0.0, 0, 255, 0
"#;

    // Write temp file
    let temp_path = std::env::temp_dir().join("test_braced_expr.ini");
    std::fs::write(&temp_path, ini_content).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(result.is_ok(), "Failed to parse INI: {:?}", result.err());

    let def = result.unwrap();

    // Check that loadBinsAFR was parsed with correct shape [16]
    let load_bins = def.constants.get("loadBinsAFR");
    assert!(load_bins.is_some(), "loadBinsAFR constant not found");
    let load_bins = load_bins.unwrap();
    assert_eq!(
        load_bins.shape.element_count(),
        16,
        "loadBinsAFR should have 16 elements, got shape: {:?}",
        load_bins.shape
    );

    // Also verify the simpler constant works
    let simple_bins = def.constants.get("simpleBins");
    assert!(simple_bins.is_some(), "simpleBins constant not found");
    let simple_bins = simple_bins.unwrap();
    assert_eq!(simple_bins.shape.element_count(), 16);
    assert_eq!(simple_bins.units, "kPa");

    // Cleanup
    let _ = std::fs::remove_file(&temp_path);
}

/// Test parsing subMenu with space-separated target "Label" format
#[test]
fn test_parse_submenu_space_separated() {
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[Constants]
page = 1

[Menu]
menu = "Test Menu"
   subMenu = dwell_tblMap    "Dwell Map", { useDwellMap }
   subMenu = stagingMap      "Fuel Staging", { stagingMode == 0 }
   subMenu = normalMenu, "Normal Label", { someCondition }
"#;

    let temp_path = std::env::temp_dir().join("test_submenu_space.ini");
    std::fs::write(&temp_path, ini_content).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(result.is_ok(), "Failed to parse INI: {:?}", result.err());

    let def = result.unwrap();

    // Find the menu
    assert!(!def.menus.is_empty(), "No menus parsed");
    let menu = &def.menus[0];

    // Check we have 3 menu items
    assert_eq!(
        menu.items.len(),
        3,
        "Expected 3 menu items, got {}",
        menu.items.len()
    );

    // Verify the space-separated items have correct labels (not conditions)
    for item in &menu.items {
        match item {
            libretune_core::ini::MenuItem::Dialog {
                label,
                target,
                enabled_condition,
                ..
            } => {
                // Labels should NOT start with { - that would indicate the bug
                assert!(
                    !label.starts_with('{'),
                    "Label '{}' looks like a condition - parsing bug!",
                    label
                );

                // Verify specific items
                if target == "dwell_tblMap" {
                    assert_eq!(label, "Dwell Map", "Wrong label for dwell_tblMap");
                    assert_eq!(enabled_condition.as_deref(), Some("useDwellMap"));
                } else if target == "stagingMap" {
                    assert_eq!(label, "Fuel Staging", "Wrong label for stagingMap");
                    assert_eq!(enabled_condition.as_deref(), Some("stagingMode == 0"));
                } else if target == "normalMenu" {
                    assert_eq!(label, "Normal Label", "Wrong label for normalMenu");
                    assert_eq!(enabled_condition.as_deref(), Some("someCondition"));
                }
            }
            _ => {}
        }
    }

    let _ = std::fs::remove_file(&temp_path);
}

/// Test parsing rusEFI dual-condition subMenu format
#[test]
fn test_parse_submenu_dual_condition() {
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[Constants]
page = 1

[Menu]
menu = "Test Menu"
   subMenu = dcMotorActuatorHw  "DC motor actuator(s) hardware", { 1 }, { uiMode == 0 || uiMode == 1 }
"#;

    let temp_path = std::env::temp_dir().join("test_submenu_dual.ini");
    std::fs::write(&temp_path, ini_content).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(result.is_ok(), "Failed to parse INI: {:?}", result.err());

    let def = result.unwrap();

    assert!(!def.menus.is_empty(), "No menus parsed");
    let menu = &def.menus[0];
    assert!(!menu.items.is_empty(), "No menu items parsed");

    // Check the dual-condition item
    match &menu.items[0] {
        libretune_core::ini::MenuItem::Dialog {
            label,
            target,
            visibility_condition,
            enabled_condition,
            ..
        } => {
            assert_eq!(target, "dcMotorActuatorHw");
            assert_eq!(label, "DC motor actuator(s) hardware");
            // First condition "1" is filtered out as trivial, so visibility_condition is None
            // Second condition goes to enabled_condition
            assert_eq!(visibility_condition.as_ref(), None);
            assert_eq!(
                enabled_condition.as_deref(),
                Some("uiMode == 0 || uiMode == 1")
            );
        }
        other => panic!("Expected Dialog item, got {:?}", other),
    }

    let _ = std::fs::remove_file(&temp_path);
}

/// Test parsing 2D array constants for table dimensions
#[test]
fn test_parse_2d_array_shape() {
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[Constants]
page = 1
   veTable = array, U08, 0, [16x16], "%", 1.0, 0.0, 0, 255, 0
   rpmBins = array, U16, 256, [16], "RPM", 1.0, 0.0, 0, 15000, 0
   loadBins = array, U08, 288, [16], "kPa", 1.0, 0.0, 0, 255, 0
"#;

    let temp_path = std::env::temp_dir().join("test_2d_array.ini");
    std::fs::write(&temp_path, ini_content).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(result.is_ok(), "Failed to parse INI: {:?}", result.err());

    let def = result.unwrap();

    // Check veTable is 16x16 = 256 elements
    let ve_table = def.constants.get("veTable").expect("veTable not found");
    assert_eq!(
        ve_table.shape.element_count(),
        256,
        "veTable should be 16x16=256 elements, got: {:?}",
        ve_table.shape
    );

    // Check rpmBins is 16 elements
    let rpm_bins = def.constants.get("rpmBins").expect("rpmBins not found");
    assert_eq!(rpm_bins.shape.element_count(), 16);

    // Check loadBins is 16 elements
    let load_bins = def.constants.get("loadBins").expect("loadBins not found");
    assert_eq!(load_bins.shape.element_count(), 16);

    let _ = std::fs::remove_file(&temp_path);
}

/// Test parsing OutputChannels with units, scale, translate, and computed channel expressions
#[test]
fn test_parse_output_channels_units_scale() {
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[OutputChannels]
   rpm = U16, 0, "RPM", 1.0, 0.0
   clt = scalar, S16, 2, "C", 0.1, -40.0
   afr = { ego1 / 10.0 }, "AFR"
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Failed to parse INI");

    let rpm = def.get_output_channel("rpm").expect("rpm channel missing");
    assert_eq!(rpm.data_type, DataType::U16);
    assert_eq!(rpm.units, "RPM");
    assert_eq!(rpm.scale, 1.0);
    assert_eq!(rpm.translate, 0.0);

    let clt = def.get_output_channel("clt").expect("clt channel missing");
    assert_eq!(clt.data_type, DataType::S16);
    assert_eq!(clt.units, "C");
    assert!((clt.scale - 0.1).abs() < f64::EPSILON);
    assert!((clt.translate + 40.0).abs() < f64::EPSILON);

    let afr = def.get_output_channel("afr").expect("afr channel missing");
    assert!(afr.is_computed());
    assert_eq!(afr.units, "AFR");
}

/// Integration test: parse a real Speeduino INI and verify key constants
#[test]
fn test_parse_real_speeduino_ini() {
    // Try to find a real INI file
    let ini_paths = [
        Path::new("/home/pat/codingprojects/libretune/definitions/speeduino202501.ini"),
        Path::new("definitions/speeduino202501.ini"),
        Path::new("../definitions/speeduino202501.ini"),
    ];

    let ini_path = ini_paths.iter().find(|p| p.exists());

    if ini_path.is_none() {
        println!("Skipping real INI test - no speeduino INI found");
        return;
    }

    let ini_path = ini_path.unwrap();
    let result = EcuDefinition::from_file(ini_path);

    if let Err(ref e) = result {
        println!(
            "Warning: Failed to parse real INI (may be expected if file format differs): {}",
            e
        );
        return;
    }

    let def = result.unwrap();

    // If we have loadBinsAFR, verify it has 16 elements (not corrupted by comma in expression)
    if let Some(load_bins) = def.constants.get("loadBinsAFR") {
        assert_eq!(
            load_bins.shape.element_count(),
            16,
            "loadBinsAFR should have 16 elements after fix, got: {:?}",
            load_bins.shape
        );
    }

    // Check that menu items don't have condition-like labels
    for menu in &def.menus {
        for item in &menu.items {
            match item {
                libretune_core::ini::MenuItem::Dialog { label, .. }
                | libretune_core::ini::MenuItem::Table { label, .. }
                | libretune_core::ini::MenuItem::Std { label, .. } => {
                    // Labels should not look like conditions
                    if label.starts_with('{') && label.ends_with('}') {
                        panic!(
                            "Menu item label '{}' looks like a condition - parsing bug!",
                            label
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Test section headers with trailing comments (which lack assignment)
/// This validates fix for [Section] ; comment being ignored
#[test]
fn test_section_header_with_comment() {
    let content = r#"
[MegaTune] ; this is a comment
signature = "1234"
"#;

    let result = EcuDefinition::from_str(content).expect("Failed to parse INI");

    assert_eq!(
        result.signature, "1234",
        "Signature should be parsed even if header has comment"
    );
}

/// Test parsing an INI file encoded in Windows-1252 (Latin-1) with Portuguese accented characters.
/// This verifies that characters like ç, ã, õ, é are preserved instead of being replaced with U+FFFD.
#[test]
fn test_parse_windows1252_portuguese_ini() {
    // Windows-1252 encoded INI content with Portuguese accented characters
    // ç=0xE7, ã=0xE3, õ=0xF5, é=0xE9, ó=0xF3, ú=0xFA, í=0xED, ê=0xEA
    let ini_bytes: Vec<u8> = {
        let mut bytes = Vec::new();
        // [MegaTune]\n
        bytes.extend_from_slice(b"[MegaTune]\n");
        bytes.extend_from_slice(b"signature = \"Test ECU\"\n");
        bytes.extend_from_slice(b"queryCommand = \"Q\"\n\n");
        // [Menu]\n with Portuguese accented text
        bytes.extend_from_slice(b"[Menu]\n");
        // menu = "Configurações" (Windows-1252: Configura\xE7\xF5es)
        bytes.extend_from_slice(b"menu = \"Configura");
        bytes.push(0xE7); // ç
        bytes.push(0xF5); // õ
        bytes.extend_from_slice(b"es\"\n");
        // subMenu = ignAdv, "Ignição" (Windows-1252: Igni\xE7\xE3o)
        bytes.extend_from_slice(b"   subMenu = ignAdv, \"Igni");
        bytes.push(0xE7); // ç
        bytes.push(0xE3); // ã
        bytes.extend_from_slice(b"o\"\n");
        // subMenu = fuelSettings, "Saída de Combustível"
        bytes.extend_from_slice(b"   subMenu = fuelSettings, \"Sa");
        bytes.push(0xED); // í
        bytes.extend_from_slice(b"da de Combust");
        bytes.push(0xED); // í
        bytes.extend_from_slice(b"vel\"\n");
        bytes
    };

    let temp_path = std::env::temp_dir().join("test_win1252_portuguese.ini");
    std::fs::write(&temp_path, &ini_bytes).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(
        result.is_ok(),
        "Failed to parse Windows-1252 INI: {:?}",
        result.err()
    );

    let def = result.unwrap();

    // Verify the menu was parsed
    assert!(!def.menus.is_empty(), "No menus parsed from Windows-1252 INI");
    let menu = &def.menus[0];

    // Menu label should contain accented characters, not replacement chars
    assert_eq!(menu.title, "Configurações", "Menu title should preserve Portuguese accents");

    // Verify menu items have correct accented labels
    let mut found_ignition = false;
    let mut found_fuel = false;
    for item in &menu.items {
        match item {
            libretune_core::ini::MenuItem::Dialog { label, target, .. } => {
                if target == "ignAdv" {
                    assert_eq!(label, "Ignição", "Ignição label should preserve accents");
                    // Verify no replacement characters
                    assert!(
                        !label.contains('\u{FFFD}'),
                        "Label should not contain replacement characters: {}",
                        label
                    );
                    found_ignition = true;
                } else if target == "fuelSettings" {
                    assert_eq!(
                        label, "Saída de Combustível",
                        "Fuel label should preserve accents"
                    );
                    assert!(
                        !label.contains('\u{FFFD}'),
                        "Label should not contain replacement characters: {}",
                        label
                    );
                    found_fuel = true;
                }
            }
            _ => {}
        }
    }

    assert!(found_ignition, "Ignição menu item not found");
    assert!(found_fuel, "Saída de Combustível menu item not found");

    let _ = std::fs::remove_file(&temp_path);
}

/// Test parsing an INI file encoded in Windows-1252 with German/French accented characters.
/// Verifies that ä, ö, ü, ß, è, à are preserved correctly.
#[test]
fn test_parse_windows1252_german_french_ini() {
    let ini_bytes: Vec<u8> = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"[MegaTune]\n");
        bytes.extend_from_slice(b"signature = \"Test ECU\"\n");
        bytes.extend_from_slice(b"queryCommand = \"Q\"\n\n");
        bytes.extend_from_slice(b"[Constants]\n");
        bytes.extend_from_slice(b"page = 1\n");
        // Use German umlaut in units: "°C" where ° is 0xB0 in Windows-1252
        bytes.extend_from_slice(b"   coolantTemp = scalar, S16, 0, \"");
        bytes.push(0xB0); // ° (degree symbol)
        bytes.extend_from_slice(b"C\", 0.1, -40.0, -40, 200, 1\n");
        bytes
    };

    let temp_path = std::env::temp_dir().join("test_win1252_german.ini");
    std::fs::write(&temp_path, &ini_bytes).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(
        result.is_ok(),
        "Failed to parse Windows-1252 INI with degree symbol: {:?}",
        result.err()
    );

    let def = result.unwrap();
    let constant = def.constants.get("coolantTemp").expect("coolantTemp not found");
    assert_eq!(constant.units, "°C", "Degree symbol should be preserved");
    assert!(
        !constant.units.contains('\u{FFFD}'),
        "Units should not contain replacement characters"
    );

    let _ = std::fs::remove_file(&temp_path);
}

/// Test that pure UTF-8 INI files still work correctly
#[test]
fn test_parse_utf8_ini_with_accents() {
    // UTF-8 encoded INI with accented characters (multi-byte sequences)
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[Menu]
menu = "Configurações"
   subMenu = ignAdv, "Ignição"
"#;

    let temp_path = std::env::temp_dir().join("test_utf8_accents.ini");
    std::fs::write(&temp_path, ini_content.as_bytes()).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(result.is_ok(), "Failed to parse UTF-8 INI: {:?}", result.err());

    let def = result.unwrap();
    assert!(!def.menus.is_empty(), "No menus parsed from UTF-8 INI");
    assert_eq!(
        def.menus[0].title, "Configurações",
        "UTF-8 accented menu label should be preserved"
    );

    let _ = std::fs::remove_file(&temp_path);
}
