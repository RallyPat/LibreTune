use libretune_core::unit_conversion::{
    afr_to_lambda, bar_to_psi, celsius_to_fahrenheit, fahrenheit_to_celsius, kmh_to_mph,
    kpa_to_psi, lambda_to_afr, mph_to_kmh, psi_to_bar, psi_to_kpa,
};

#[test]
fn test_celsius_to_fahrenheit() {
    let celsius = 0.0;
    let fahrenheit = celsius_to_fahrenheit(celsius);
    assert_eq!(fahrenheit, 32.0);
}

#[test]
fn test_fahrenheit_to_celsius() {
    let fahrenheit = 32.0;
    let celsius = fahrenheit_to_celsius(fahrenheit);
    assert_eq!(celsius, 0.0);
}

#[test]
fn test_celsius_boiling_point() {
    let celsius = 100.0;
    let fahrenheit = celsius_to_fahrenheit(celsius);
    assert_eq!(fahrenheit, 212.0);
}

#[test]
fn test_fahrenheit_boiling_point() {
    let fahrenheit = 212.0;
    let celsius = fahrenheit_to_celsius(fahrenheit);
    assert_eq!(celsius, 100.0);
}

#[test]
fn test_negative_temperature() {
    let celsius = -40.0;
    let fahrenheit = celsius_to_fahrenheit(celsius);
    assert_eq!(fahrenheit, -40.0); // -40C = -40F
}

#[test]
fn test_kpa_to_psi() {
    let kpa = 101.325; // Standard atmospheric pressure
    let psi = kpa_to_psi(kpa);
    assert!((psi - 14.696).abs() < 0.01);
}

#[test]
fn test_psi_to_kpa() {
    let psi = 14.696; // Standard atmospheric pressure
    let kpa = psi_to_kpa(psi);
    assert!((kpa - 101.325).abs() < 0.1);
}

#[test]
fn test_bar_to_psi() {
    // 1 bar ≈ 14.5 PSI
    let bar = 1.0;
    let psi = bar_to_psi(bar);
    assert!((psi - 14.5037).abs() < 0.1);
}

#[test]
fn test_psi_to_bar() {
    let psi = 14.5037;
    let bar = psi_to_bar(psi);
    assert!((bar - 1.0).abs() < 0.01);
}

#[test]
fn test_zero_pressure() {
    let psi = kpa_to_psi(0.0);
    assert_eq!(psi, 0.0);
}

#[test]
fn test_kmh_to_mph() {
    let kmh = 100.0;
    let mph = kmh_to_mph(kmh);
    assert!((mph - 62.137).abs() < 0.1);
}

#[test]
fn test_mph_to_kmh() {
    let mph = 62.137;
    let kmh = mph_to_kmh(mph);
    assert!((kmh - 100.0).abs() < 0.1);
}

#[test]
fn test_speed_zero() {
    let mph = kmh_to_mph(0.0);
    assert_eq!(mph, 0.0);
}

#[test]
fn test_afr_to_lambda() {
    let afr = 14.7_f64; // Stoichiometric for gasoline
    let lambda = afr_to_lambda(afr, "gasoline");
    assert!((lambda - 1.0_f64).abs() < 0.01_f64);
}

#[test]
fn test_lambda_to_afr() {
    let lambda = 1.0_f64;
    let afr = lambda_to_afr(lambda, "gasoline");
    assert!((afr - 14.7_f64).abs() < 0.01_f64);
}

#[test]
fn test_rich_mixture_lambda() {
    let lambda = 0.9_f64; // 10% richer than stoichiometric
    let afr = lambda_to_afr(lambda, "gasoline");
    assert!(afr < 14.7_f64);
}

#[test]
fn test_lean_mixture_lambda() {
    let lambda = 1.1_f64; // 10% leaner than stoichiometric
    let afr = lambda_to_afr(lambda, "gasoline");
    assert!(afr > 14.7_f64);
}

#[test]
fn test_temperature_round_trip() {
    let original = 25.5_f64;
    let fahrenheit = celsius_to_fahrenheit(original);
    let back_to_celsius = fahrenheit_to_celsius(fahrenheit);
    assert!((back_to_celsius - original).abs() < 0.01);
}

#[test]
fn test_pressure_round_trip() {
    let original = 250.0_f64;
    let psi = kpa_to_psi(original);
    let back_to_kpa = psi_to_kpa(psi);
    assert!((back_to_kpa - original).abs() < 0.1);
}

#[test]
fn test_speed_round_trip() {
    let original = 50.0_f64;
    let kmh = mph_to_kmh(original);
    let back_to_mph = kmh_to_mph(kmh);
    assert!((back_to_mph - original).abs() < 0.1);
}

#[test]
fn test_large_temperature() {
    let celsius = 500.0_f64;
    let fahrenheit = celsius_to_fahrenheit(celsius);
    assert!(fahrenheit > celsius); // Fahrenheit scale is larger
}

#[test]
fn test_high_pressure_conversion() {
    let kpa = 5000.0_f64; // Very high pressure
    let psi = kpa_to_psi(kpa);
    assert!(psi > 0.0_f64);
    assert!(psi < kpa); // PSI units are larger
}
