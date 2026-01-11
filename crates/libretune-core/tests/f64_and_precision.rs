//! Tests for F64 data type support, MSQ page tracking, and computed channel evaluation

use libretune_core::ini::{DataType, EcuDefinition};
use libretune_core::tune::{TuneFile, TuneValue};
use std::collections::HashMap;

/// Test F64 data type parsing from INI format
#[test]
fn test_f64_data_type_parsing() {
    let ini_content = r#"
[MegaTune]
signature = "Test ECU"
queryCommand = "Q"

[Constants]
page = 1
   doubleConst = scalar, F64, 0, "units", 1.0, 0.0, -1e308, 1e308, 8
   double2 = scalar, DOUBLE, 8, "ms", 1.0, 0.0, 0, 1000, 8
   floatConst = scalar, F32, 16, "V", 1.0, 0.0, 0, 20, 2
"#;

    let temp_path = std::env::temp_dir().join("test_f64_type.ini");
    std::fs::write(&temp_path, ini_content).expect("Failed to write temp file");

    let result = EcuDefinition::from_file(&temp_path);
    assert!(result.is_ok(), "Failed to parse INI: {:?}", result.err());

    let def = result.unwrap();

    // Check F64 type parsing
    let double_const = def.constants.get("doubleConst");
    assert!(double_const.is_some(), "doubleConst not found");
    let double_const = double_const.unwrap();
    assert_eq!(
        double_const.data_type,
        DataType::F64,
        "Expected F64 data type"
    );

    // Check DOUBLE alias also parses to F64
    let double2 = def.constants.get("double2");
    assert!(double2.is_some(), "double2 not found");
    let double2 = double2.unwrap();
    assert_eq!(
        double2.data_type,
        DataType::F64,
        "DOUBLE should parse as F64"
    );

    // Check F32 still works
    let float_const = def.constants.get("floatConst");
    assert!(float_const.is_some(), "floatConst not found");
    let float_const = float_const.unwrap();
    assert_eq!(
        float_const.data_type,
        DataType::F32,
        "Expected F32 data type"
    );

    // Cleanup
    let _ = std::fs::remove_file(&temp_path);
}

/// Test F64 data type size calculation
#[test]
fn test_f64_size_bytes() {
    assert_eq!(DataType::F64.size_bytes(), 8, "F64 should be 8 bytes");
    assert_eq!(DataType::F32.size_bytes(), 4, "F32 should be 4 bytes");
    assert_eq!(DataType::U08.size_bytes(), 1, "U08 should be 1 byte");
    assert_eq!(DataType::S16.size_bytes(), 2, "S16 should be 2 bytes");
    assert_eq!(DataType::U32.size_bytes(), 4, "U32 should be 4 bytes");
}

/// Test MSQ page tracking with set_constant_with_page
#[test]
fn test_msq_page_tracking() {
    let mut tune = TuneFile::default();

    // Set constants on different pages
    tune.set_constant_with_page("constPage1".to_string(), TuneValue::Scalar(1.0), 1);
    tune.set_constant_with_page("constPage2".to_string(), TuneValue::Scalar(2.0), 2);
    tune.set_constant_with_page("constPage3".to_string(), TuneValue::Scalar(3.0), 3);

    // Verify page assignments are tracked
    assert_eq!(tune.constant_pages.get("constPage1"), Some(&1));
    assert_eq!(tune.constant_pages.get("constPage2"), Some(&2));
    assert_eq!(tune.constant_pages.get("constPage3"), Some(&3));

    // Verify values are stored (using match instead of eq since TuneValue doesn't derive PartialEq)
    match tune.constants.get("constPage1") {
        Some(TuneValue::Scalar(v)) => assert_eq!(*v, 1.0),
        _ => panic!("constPage1 should be Scalar(1.0)"),
    }
    match tune.constants.get("constPage2") {
        Some(TuneValue::Scalar(v)) => assert_eq!(*v, 2.0),
        _ => panic!("constPage2 should be Scalar(2.0)"),
    }
    match tune.constants.get("constPage3") {
        Some(TuneValue::Scalar(v)) => assert_eq!(*v, 3.0),
        _ => panic!("constPage3 should be Scalar(3.0)"),
    }
}

/// Test high precision formatting for F64 values in MSQ output
#[test]
fn test_f64_precision_roundtrip() {
    // Test value that needs high precision (more than F32 can represent exactly)
    let high_precision_value = 1.234567890123456789_f64;

    // Format with our precision approach
    let formatted = format!("{:.17}", high_precision_value);
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');

    // Parse back
    let parsed: f64 = trimmed.parse().expect("Should parse back");

    // Verify precision is preserved (within f64 precision limits)
    assert!(
        (parsed - high_precision_value).abs() < f64::EPSILON * 10.0,
        "Precision lost: original={}, parsed={}",
        high_precision_value,
        parsed
    );
}

/// Test that small values format correctly
#[test]
fn test_small_value_formatting() {
    let small_value = 0.000000001_f64;

    let formatted = format!("{:.17}", small_value);
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');

    let parsed: f64 = trimmed.parse().expect("Should parse back");

    assert!(
        (parsed - small_value).abs() < f64::EPSILON * 10.0,
        "Small value precision lost: original={}, parsed={}",
        small_value,
        parsed
    );
}

/// Test that integer values format cleanly (no unnecessary decimals in output)
#[test]
fn test_integer_value_formatting() {
    let int_value = 100.0_f64;

    let formatted = format!("{:.17}", int_value);
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');

    // Should trim to clean integer representation
    assert_eq!(
        trimmed, "100",
        "Integer value should format cleanly: got {}",
        trimmed
    );
}

/// Test array formatting with high precision
#[test]
fn test_array_precision_formatting() {
    let values = vec![
        1.234567890123456_f64,
        0.000000001_f64,
        100.0_f64,
        3.14159265358979_f64,
    ];

    let formatted: Vec<String> = values
        .iter()
        .map(|v| {
            let formatted = format!("{:.17}", v);
            let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
            if trimmed.is_empty() {
                "0".to_string()
            } else {
                trimmed.to_string()
            }
        })
        .collect();

    // Parse back and verify precision
    for (i, formatted_str) in formatted.iter().enumerate() {
        let parsed: f64 = formatted_str.parse().expect("Should parse back");
        assert!(
            (parsed - values[i]).abs() < f64::EPSILON * 10.0,
            "Array element {} precision lost: original={}, parsed={}",
            i,
            values[i],
            parsed
        );
    }
}

/// Test computed channel expression evaluation with parse_with_context
#[test]
fn test_computed_channel_expression_evaluation() {
    use libretune_core::ini::{Endianness, OutputChannel};

    // Create an output channel with an expression
    let channel = OutputChannel {
        name: "computedValue".to_string(),
        label: Some("Computed".to_string()),
        data_type: DataType::F32,
        offset: 0,
        bit_position: None,
        units: "computed".to_string(),
        scale: 1.0,
        translate: 0.0,
        expression: Some("{rpm} / 1000".to_string()),
    };

    // Create context with referenced channel values
    let mut context: HashMap<String, f64> = HashMap::new();
    context.insert("rpm".to_string(), 3000.0);

    // Empty data buffer (not used for computed channels)
    let data = vec![];
    let endian = Endianness::Little;

    // Parse with context
    let result = channel.parse_with_context(&data, endian, &context);

    // Should compute 3000 / 1000 = 3.0
    assert!(result.is_some(), "Should return Some value");
    let value = result.unwrap();
    assert!(
        (value - 3.0).abs() < f64::EPSILON,
        "Expected 3.0, got {}",
        value
    );
}

/// Test computed channel with complex expression
#[test]
fn test_computed_channel_complex_expression() {
    use libretune_core::ini::{Endianness, OutputChannel};

    let channel = OutputChannel {
        name: "injectorDuty".to_string(),
        label: Some("Injector Duty".to_string()),
        data_type: DataType::F32,
        offset: 0,
        bit_position: None,
        units: "%".to_string(),
        scale: 1.0,
        translate: 0.0,
        expression: Some("{pulseWidth} * {rpm} / 1200".to_string()),
    };

    let mut context: HashMap<String, f64> = HashMap::new();
    context.insert("pulseWidth".to_string(), 2.0);
    context.insert("rpm".to_string(), 3000.0);

    let data = vec![];
    let endian = Endianness::Little;

    let result = channel.parse_with_context(&data, endian, &context);

    // 2.0 * 3000 / 1200 = 5.0
    assert!(result.is_some(), "Should return Some value");
    let value = result.unwrap();
    assert!((value - 5.0).abs() < 0.001, "Expected 5.0, got {}", value);
}

/// Test computed channel evaluates with default 0 for undefined variables
/// (This matches INI convention where undefined variables default to 0)
#[test]
fn test_computed_channel_undefined_variable_defaults_to_zero() {
    use libretune_core::ini::{Endianness, OutputChannel};

    let channel = OutputChannel {
        name: "undefinedExpr".to_string(),
        label: Some("Undefined".to_string()),
        data_type: DataType::F32,
        offset: 0,
        bit_position: None,
        units: "?".to_string(),
        scale: 1.0,
        translate: 0.0,
        expression: Some("{undefined_channel} * 2".to_string()),
    };

    // Context without the required channel
    let context: HashMap<String, f64> = HashMap::new();

    let data = vec![];
    let endian = Endianness::Little;

    // Should return Some(0.0) since undefined variables default to 0 in INI expressions
    let result = channel.parse_with_context(&data, endian, &context);
    assert!(
        result.is_some(),
        "Should return Some value for computed channel"
    );
    let value = result.unwrap();
    assert_eq!(
        value, 0.0,
        "Undefined variable should default to 0, got {}",
        value
    );
}

/// Test non-computed channel ignores context and uses raw data
#[test]
fn test_non_computed_channel_uses_raw_data() {
    use libretune_core::ini::{Endianness, OutputChannel};

    let channel = OutputChannel {
        name: "rawValue".to_string(),
        label: Some("Raw Value".to_string()),
        data_type: DataType::U16,
        offset: 0,
        bit_position: None,
        units: "raw".to_string(),
        scale: 1.0,
        translate: 0.0,
        expression: None,
    };

    // Context with a value that should be ignored
    let mut context: HashMap<String, f64> = HashMap::new();
    context.insert("rawValue".to_string(), 9999.0);

    // Raw data with value 1000 (little endian U16)
    let data = vec![0xE8, 0x03]; // 1000 in little endian
    let endian = Endianness::Little;

    let result = channel.parse_with_context(&data, endian, &context);

    // Should use raw data value, not context value
    assert!(result.is_some(), "Should return Some value");
    let value = result.unwrap();
    assert_eq!(value, 1000.0, "Should parse from raw data, got {}", value);
}
