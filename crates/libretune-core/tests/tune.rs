use libretune_core::tune::TuneValue;

#[test]
fn test_tune_value_scalar_creation() {
    let value = TuneValue::Scalar(100.0);
    match value {
        TuneValue::Scalar(v) => assert_eq!(v, 100.0),
        _ => panic!("Expected scalar value"),
    }
}

#[test]
fn test_tune_value_array_creation() {
    let value = TuneValue::Array(vec![1.0, 2.0, 3.0]);
    match value {
        TuneValue::Array(v) => assert_eq!(v.len(), 3),
        _ => panic!("Expected array value"),
    }
}

#[test]
fn test_tune_value_string_creation() {
    let value = TuneValue::String("test".to_string());
    match value {
        TuneValue::String(v) => assert_eq!(v, "test"),
        _ => panic!("Expected string value"),
    }
}

#[test]
fn test_tune_value_bool_creation() {
    let value = TuneValue::Bool(true);
    match value {
        TuneValue::Bool(v) => assert!(v),
        _ => panic!("Expected bool value"),
    }
}

#[test]
fn test_tune_value_negative_scalar() {
    let value = TuneValue::Scalar(-42.5);
    match value {
        TuneValue::Scalar(v) => assert_eq!(v, -42.5),
        _ => panic!("Expected scalar value"),
    }
}

#[test]
fn test_tune_value_zero() {
    let value = TuneValue::Scalar(0.0);
    match value {
        TuneValue::Scalar(v) => assert_eq!(v, 0.0),
        _ => panic!("Expected scalar value"),
    }
}

#[test]
fn test_tune_value_precision() {
    let value = TuneValue::Scalar(3.14159);
    match value {
        TuneValue::Scalar(v) => assert!((v - 3.14159).abs() < 0.00001),
        _ => panic!("Expected scalar value"),
    }
}

#[test]
fn test_tune_value_large_number() {
    let value = TuneValue::Scalar(99999.99);
    match value {
        TuneValue::Scalar(v) => assert_eq!(v, 99999.99),
        _ => panic!("Expected scalar value"),
    }
}

#[test]
fn test_tune_value_equality() {
    let v1 = TuneValue::Scalar(42.0);
    let v2 = TuneValue::Scalar(42.0);
    let v3 = TuneValue::Scalar(43.0);
    
    match (&v1, &v2, &v3) {
        (TuneValue::Scalar(a), TuneValue::Scalar(b), TuneValue::Scalar(c)) => {
            assert_eq!(a, b);
            assert_ne!(b, c);
        }
        _ => panic!("Expected scalar values"),
    }
}

#[test]
fn test_tune_value_array_operations() {
    let arr = TuneValue::Array(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    match arr {
        TuneValue::Array(v) => {
            assert_eq!(v.len(), 5);
            assert_eq!(v[0], 1.0);
            assert_eq!(v[4], 5.0);
        }
        _ => panic!("Expected array value"),
    }
}

#[test]
fn test_tune_value_array_empty() {
    let arr = TuneValue::Array(vec![]);
    match arr {
        TuneValue::Array(v) => assert_eq!(v.len(), 0),
        _ => panic!("Expected array value"),
    }
}

#[test]
fn test_tune_value_array_access() {
    let arr = TuneValue::Array(vec![10.0, 20.0, 30.0]);
    match arr {
        TuneValue::Array(v) => {
            let sum: f64 = v.iter().sum();
            assert_eq!(sum, 60.0);
        }
        _ => panic!("Expected array value"),
    }
}

#[test]
fn test_tune_value_string_empty() {
    let value = TuneValue::String(String::new());
    match value {
        TuneValue::String(v) => assert!(v.is_empty()),
        _ => panic!("Expected string value"),
    }
}

#[test]
fn test_tune_value_string_long() {
    let value = TuneValue::String("a".repeat(1000));
    match value {
        TuneValue::String(v) => assert_eq!(v.len(), 1000),
        _ => panic!("Expected string value"),
    }
}

#[test]
fn test_tune_value_bool_false() {
    let value = TuneValue::Bool(false);
    match value {
        TuneValue::Bool(v) => assert!(!v),
        _ => panic!("Expected bool value"),
    }
}
