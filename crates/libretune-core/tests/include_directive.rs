//! Tests for #include directive support

use libretune_core::ini::EcuDefinition;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a test INI file and return its path
fn create_temp_ini(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_include_basic() {
    let dir = TempDir::new().unwrap();

    // Create the included file
    create_temp_ini(
        &dir,
        "included.ini",
        r#"
[Constants]
    page = 1
    includedConst = scalar, U08, 10, "", 1, 0, 0, 255, 0
"#,
    );

    // Create the main file that includes the other
    let main_path = create_temp_ini(
        &dir,
        "main.ini",
        r#"
[MegaTune]
    signature = "TestInclude"
    queryCommand = "Q"

#include "included.ini"

[Constants]
    page = 1
    mainConst = scalar, U08, 0, "", 1, 0, 0, 255, 0
"#,
    );

    let def = EcuDefinition::from_file(&main_path).expect("Should parse successfully");

    // Check that constants from both files are present
    assert!(
        def.constants.contains_key("mainConst"),
        "Should have mainConst from main.ini"
    );
    assert!(
        def.constants.contains_key("includedConst"),
        "Should have includedConst from included.ini"
    );
    assert_eq!(def.signature, "TestInclude");
}

#[test]
fn test_include_nested() {
    let dir = TempDir::new().unwrap();

    // Create nested includes: main -> level1 -> level2
    create_temp_ini(
        &dir,
        "level2.ini",
        r#"
[Constants]
    page = 1
    level2Const = scalar, U08, 20, "", 1, 0, 0, 255, 0
"#,
    );

    create_temp_ini(
        &dir,
        "level1.ini",
        r#"
#include "level2.ini"

[Constants]
    page = 1
    level1Const = scalar, U08, 10, "", 1, 0, 0, 255, 0
"#,
    );

    let main_path = create_temp_ini(
        &dir,
        "main.ini",
        r#"
[MegaTune]
    signature = "NestedInclude"
    queryCommand = "Q"

#include "level1.ini"

[Constants]
    page = 1
    mainConst = scalar, U08, 0, "", 1, 0, 0, 255, 0
"#,
    );

    let def = EcuDefinition::from_file(&main_path).expect("Should parse successfully");

    // Check that constants from all three files are present
    assert!(def.constants.contains_key("mainConst"));
    assert!(def.constants.contains_key("level1Const"));
    assert!(def.constants.contains_key("level2Const"));
}

#[test]
fn test_include_circular_detection() {
    let dir = TempDir::new().unwrap();

    // Create circular includes: a -> b -> a
    create_temp_ini(
        &dir,
        "a.ini",
        r#"
[Constants]
    page = 1
#include "b.ini"
"#,
    );

    create_temp_ini(
        &dir,
        "b.ini",
        r#"
[Constants]
    page = 1
#include "a.ini"
"#,
    );

    let main_path = dir.path().join("a.ini");
    let result = EcuDefinition::from_file(&main_path);

    assert!(result.is_err(), "Should detect circular include");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("circular"),
        "Error should mention circular reference"
    );
}

#[test]
fn test_include_conditional() {
    let dir = TempDir::new().unwrap();

    // Create included file
    create_temp_ini(
        &dir,
        "extra.ini",
        r#"
[Constants]
    page = 1
    extraConst = scalar, U08, 10, "", 1, 0, 0, 255, 0
"#,
    );

    // Create main file with conditional include
    let main_path = create_temp_ini(
        &dir,
        "main.ini",
        r#"
[MegaTune]
    signature = "ConditionalInclude"
    queryCommand = "Q"

#set INCLUDE_EXTRA
#if INCLUDE_EXTRA
#include "extra.ini"
#endif

[Constants]
    page = 1
    mainConst = scalar, U08, 0, "", 1, 0, 0, 255, 0
"#,
    );

    let def = EcuDefinition::from_file(&main_path).expect("Should parse successfully");

    // extraConst should be present because INCLUDE_EXTRA was set
    assert!(def.constants.contains_key("mainConst"));
    assert!(
        def.constants.contains_key("extraConst"),
        "extraConst should be included"
    );
}

#[test]
fn test_include_conditional_not_set() {
    let dir = TempDir::new().unwrap();

    // Create included file
    create_temp_ini(
        &dir,
        "extra.ini",
        r#"
[Constants]
    page = 1
    extraConst = scalar, U08, 10, "", 1, 0, 0, 255, 0
"#,
    );

    // Create main file with conditional include (but condition is false)
    let main_path = create_temp_ini(
        &dir,
        "main.ini",
        r#"
[MegaTune]
    signature = "ConditionalInclude"
    queryCommand = "Q"

; Note: INCLUDE_EXTRA is NOT set
#if INCLUDE_EXTRA
#include "extra.ini"
#endif

[Constants]
    page = 1
    mainConst = scalar, U08, 0, "", 1, 0, 0, 255, 0
"#,
    );

    let def = EcuDefinition::from_file(&main_path).expect("Should parse successfully");

    // extraConst should NOT be present because INCLUDE_EXTRA was not set
    assert!(def.constants.contains_key("mainConst"));
    assert!(
        !def.constants.contains_key("extraConst"),
        "extraConst should NOT be included"
    );
}

#[test]
fn test_include_file_not_found() {
    let dir = TempDir::new().unwrap();

    let main_path = create_temp_ini(
        &dir,
        "main.ini",
        r#"
[MegaTune]
    signature = "IncludeNotFound"
    queryCommand = "Q"

#include "nonexistent.ini"
"#,
    );

    let result = EcuDefinition::from_file(&main_path);

    assert!(result.is_err(), "Should fail when included file not found");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not found") || err.to_string().contains("nonexistent"),
        "Error should indicate file not found: {}",
        err
    );
}

#[test]
fn test_include_subdirectory() {
    let dir = TempDir::new().unwrap();

    // Create a subdirectory with an include file
    let subdir = dir.path().join("includes");
    fs::create_dir(&subdir).unwrap();
    fs::write(
        subdir.join("sub.ini"),
        r#"
[Constants]
    page = 1
    subConst = scalar, U08, 10, "", 1, 0, 0, 255, 0
"#,
    )
    .unwrap();

    // Create main file that includes from subdirectory
    let main_path = create_temp_ini(
        &dir,
        "main.ini",
        r#"
[MegaTune]
    signature = "SubdirInclude"
    queryCommand = "Q"

#include "includes/sub.ini"

[Constants]
    page = 1
    mainConst = scalar, U08, 0, "", 1, 0, 0, 255, 0
"#,
    );

    let def = EcuDefinition::from_file(&main_path).expect("Should parse successfully");

    assert!(def.constants.contains_key("mainConst"));
    assert!(
        def.constants.contains_key("subConst"),
        "Should include from subdirectory"
    );
}
