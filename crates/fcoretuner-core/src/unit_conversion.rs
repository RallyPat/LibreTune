//! Unit Conversion Functions
//!
//! Provides conversion functions for ECU tuning applications:
//! - Temperature: °C ↔ °F
//! - Pressure: kPa ↔ PSI
//! - Air-Fuel Ratio: Lambda ↔ AFR (gasoline=14.7, E85=9.8, etc.)
//! - Speed: km/h ↔ mph
//! - Volume: Liters ↔ Gallons (US & Imperial)

/// Convert Celsius to Fahrenheit
pub fn celsius_to_fahrenheit(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}

/// Convert Fahrenheit to Celsius
pub fn fahrenheit_to_celsius(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0
}

/// Convert kPa to PSI
pub fn kpa_to_psi(kpa: f64) -> f64 {
    kpa * 0.14503773773020923
}

/// Convert PSI to kPa
pub fn psi_to_kpa(psi: f64) -> f64 {
    psi / 0.14503773773020923
}

/// Convert bar to PSI
pub fn bar_to_psi(bar: f64) -> f64 {
    bar * 14.503773773020923
}

/// Convert PSI to bar
pub fn psi_to_bar(psi: f64) -> f64 {
    psi / 14.503773773020923
}

/// Convert Lambda to AFR
///
/// # Arguments
/// * `lambda` - Lambda value (1.0 = stoichiometric)
/// * `fuel_type` - Fuel type: "gasoline" (default), "E85", "LPG", "CNG", etc.
///
/// # Returns
/// AFR value for the specified fuel type
pub fn lambda_to_afr(lambda: f64, fuel_type: &str) -> f64 {
    let stoich_afr = match fuel_type.to_lowercase().as_str() {
        "gasoline" | "petrol" => 14.7,
        "e85" => 9.8,
        "methanol" => 6.4,
        "ethanol" => 9.0,
        "lpg" | "propane" => 15.5,
        "cng" | "natural_gas" => 17.2,
        "diesel" => 14.5,
        _ => 14.7, // Default to gasoline
    };
    lambda * stoich_afr
}

/// Convert AFR to Lambda
///
/// # Arguments
/// * `afr` - AFR value
/// * `fuel_type` - Fuel type: "gasoline" (default), "E85", "LPG", "CNG", etc.
///
/// # Returns
/// Lambda value
pub fn afr_to_lambda(afr: f64, fuel_type: &str) -> f64 {
    let stoich_afr = match fuel_type.to_lowercase().as_str() {
        "gasoline" | "petrol" => 14.7,
        "e85" => 9.8,
        "methanol" => 6.4,
        "ethanol" => 9.0,
        "lpg" | "propane" => 15.5,
        "cng" | "natural_gas" => 17.2,
        "diesel" => 14.5,
        _ => 14.7, // Default to gasoline
    };
    afr / stoich_afr
}

/// Convert km/h to mph
pub fn kmh_to_mph(kmh: f64) -> f64 {
    kmh * 0.62137119223733
}

/// Convert mph to km/h
pub fn mph_to_kmh(mph: f64) -> f64 {
    mph / 0.62137119223733
}

/// Convert Liters to US Gallons
pub fn liters_to_gallons_us(liters: f64) -> f64 {
    liters * 0.26417205235815
}

/// Convert US Gallons to Liters
pub fn gallons_us_to_liters(gallons: f64) -> f64 {
    gallons / 0.26417205235815
}

/// Convert Liters to Imperial Gallons
pub fn liters_to_gallons_imperial(liters: f64) -> f64 {
    liters * 0.21996924829909
}

/// Convert Imperial Gallons to Liters
pub fn gallons_imperial_to_liters(gallons: f64) -> f64 {
    gallons / 0.21996924829909
}

/// Convert Liters to Gallons (imperial flag determines which type)
pub fn liters_to_gallons(liters: f64, imperial: bool) -> f64 {
    if imperial {
        liters_to_gallons_imperial(liters)
    } else {
        liters_to_gallons_us(liters)
    }
}

/// Convert Gallons to Liters (imperial flag determines which type)
pub fn gallons_to_liters(gallons: f64, imperial: bool) -> f64 {
    if imperial {
        gallons_imperial_to_liters(gallons)
    } else {
        gallons_us_to_liters(gallons)
    }
}

/// Convert pounds to kilograms
pub fn lbs_to_kg(lbs: f64) -> f64 {
    lbs * 0.45359237
}

/// Convert kilograms to pounds
pub fn kg_to_lbs(kg: f64) -> f64 {
    kg / 0.45359237
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_celsius_fahrenheit_conversion() {
        assert!((celsius_to_fahrenheit(0.0) - 32.0).abs() < 0.01);
        assert!((fahrenheit_to_celsius(32.0) - 0.0).abs() < 0.01);
        assert!((celsius_to_fahrenheit(100.0) - 212.0).abs() < 0.01);
        assert!((fahrenheit_to_celsius(212.0) - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_kpa_psi_conversion() {
        assert!((kpa_to_psi(100.0) - 14.504).abs() < 0.01);
        assert!((psi_to_kpa(14.504) - 100.0).abs() < 0.01);
        assert!((kpa_to_psi(101.325) - 14.696).abs() < 0.01); // Atmospheric pressure
    }

    #[test]
    fn test_lambda_afr_conversion() {
        assert!((lambda_to_afr(1.0, "gasoline") - 14.7).abs() < 0.01);
        assert!((afr_to_lambda(14.7, "gasoline") - 1.0).abs() < 0.01);
        assert!((lambda_to_afr(1.0, "E85") - 9.8).abs() < 0.01);
        assert!((afr_to_lambda(9.8, "E85") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_kmh_mph_conversion() {
        assert!((kmh_to_mph(100.0) - 62.14).abs() < 0.01);
        assert!((mph_to_kmh(62.14) - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_liters_gallons_conversion() {
        assert!((liters_to_gallons_us(3.78541) - 1.0).abs() < 0.01);
        assert!((gallons_us_to_liters(1.0) - 3.78541).abs() < 0.01);
        assert!((liters_to_gallons_imperial(4.54609) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_lbs_kg_conversion() {
        assert!((lbs_to_kg(100.0) - 45.36).abs() < 0.01);
        assert!((kg_to_lbs(45.36) - 100.0).abs() < 0.01);
    }
}
