//! Tests for TunerStudio .dash and .gauge file parsing.

use libretune_core::dash::{parse_dash_file, parse_gauge_file, DashComponent, GaugePainter};
use std::fs;
use std::path::PathBuf;

/// Load a fixture file from tests/fixtures/dashboards/
fn load_fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/dashboards");
    path.push(name);

    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

#[test]
fn test_parse_basic_dash() {
    let xml = load_fixture("basic.dash");
    let result = parse_dash_file(&xml);

    assert!(
        result.is_ok(),
        "Failed to parse basic.dash: {:?}",
        result.err()
    );

    let dash = result.unwrap();

    // Check dashboard properties - DashFile has bibliography, version_info, and gauge_cluster
    // The default() provides "LibreTune" as author if not specified in XML
    assert_eq!(dash.bibliography.author, "LibreTune");

    // Check that we have components
    let components = &dash.gauge_cluster.components;
    assert!(!components.is_empty(), "No components found in dashboard");

    // Count gauges and indicators
    let gauge_count = components
        .iter()
        .filter(|c| matches!(c, DashComponent::Gauge(_)))
        .count();
    let indicator_count = components
        .iter()
        .filter(|c| matches!(c, DashComponent::Indicator(_)))
        .count();

    // Verify component counts (should have 4 gauges and 2 indicators)
    assert_eq!(gauge_count, 4, "Expected 4 gauges, got {}", gauge_count);
    assert_eq!(
        indicator_count, 2,
        "Expected 2 indicators, got {}",
        indicator_count
    );

    // Check RPM gauge
    let rpm_gauge = components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Gauge(g) if g.output_channel == "rpm" => Some(g.as_ref()),
            _ => None,
        })
        .next()
        .expect("RPM gauge not found");

    assert_eq!(rpm_gauge.title, "Engine Speed");
    assert_eq!(rpm_gauge.min, 0.0);
    assert_eq!(rpm_gauge.max, 8000.0);
    assert_eq!(rpm_gauge.units, "rpm");
    assert!(matches!(rpm_gauge.gauge_painter, GaugePainter::Tachometer));
    assert_eq!(rpm_gauge.peg_limits, true);

    // Check coolant gauge
    let coolant_gauge = components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Gauge(g) if g.output_channel == "coolant" => Some(g.as_ref()),
            _ => None,
        })
        .next()
        .expect("Coolant gauge not found");

    eprintln!("Coolant gauge painter: {:?}", coolant_gauge.gauge_painter);
    assert_eq!(coolant_gauge.title, "Coolant Temp");
    assert!(
        matches!(coolant_gauge.gauge_painter, GaugePainter::VerticalBarGauge),
        "Expected VerticalBarGauge, got {:?}",
        coolant_gauge.gauge_painter
    );
    assert_eq!(coolant_gauge.high_warning, Some(100.0));
    assert_eq!(coolant_gauge.high_critical, Some(110.0));

    // Check AFR gauge
    let afr_gauge = components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Gauge(g) if g.output_channel == "afr" => Some(g.as_ref()),
            _ => None,
        })
        .next()
        .expect("AFR gauge not found");

    assert_eq!(afr_gauge.title, "Air/Fuel Ratio");
    assert!(matches!(
        afr_gauge.gauge_painter,
        GaugePainter::BasicReadout
    ));
    assert_eq!(afr_gauge.value_digits, 2);

    // Check MAP gauge
    let map_gauge = components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Gauge(g) if g.output_channel == "map" => Some(g.as_ref()),
            _ => None,
        })
        .next()
        .expect("MAP gauge not found");

    assert_eq!(map_gauge.title, "Manifold Pressure");
    assert!(matches!(
        map_gauge.gauge_painter,
        GaugePainter::AsymmetricSweepGauge
    ));

    // Check battery indicator
    let battery_ind = components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Indicator(i) if i.output_channel == "battery" => Some(i),
            _ => None,
        })
        .next()
        .expect("Battery indicator not found");

    assert_eq!(battery_ind.on_text, "Battery OK");

    // Check error indicator
    let error_ind = components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Indicator(i) if i.output_channel == "error_code" => Some(i),
            _ => None,
        })
        .next()
        .expect("Error indicator not found");

    assert_eq!(error_ind.on_text, "Check Engine");
}

#[test]
fn test_color_parsing() {
    let xml = load_fixture("basic.dash");
    let dash = parse_dash_file(&xml).expect("Failed to parse basic.dash");

    let rpm_gauge = dash
        .gauge_cluster
        .components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Gauge(g) if g.output_channel == "rpm" => Some(g.as_ref()),
            _ => None,
        })
        .next()
        .expect("RPM gauge not found");

    // TunerStudio uses ARGB format as signed 32-bit integers
    // -16777216 = 0xFF000000 = black with full alpha
    // -1 = 0xFFFFFFFF = white
    // -65536 = 0xFFFF0000 = red
    // -6710887 = 0xFF999999 = gray

    // We don't need to check exact values here since the parser
    // converts them, just verify they're set (not default)
    // Colors are TsColor structs, not Option<TsColor>
    eprintln!("RPM gauge back_color: {:?}", rpm_gauge.back_color);
    eprintln!("RPM gauge font_color: {:?}", rpm_gauge.font_color);
    eprintln!("RPM gauge needle_color: {:?}", rpm_gauge.needle_color);
    eprintln!("RPM gauge trim_color: {:?}", rpm_gauge.trim_color);

    // Black has all zeros, but we set needle color to red (-65536 = 0xFFFF0000 = red)
    // So needle should have red=255
    assert_ne!(
        rpm_gauge.needle_color.red, 0,
        "Needle color should be red, not black"
    );

    // Font color is white (-1 = 0xFFFFFFFF), so it should have non-zero values
    assert_ne!(
        rpm_gauge.font_color.red, 0,
        "Font color should be white, not black"
    );
}

#[test]
fn test_gauge_painter_types() {
    let xml = load_fixture("basic.dash");
    let dash = parse_dash_file(&xml).expect("Failed to parse basic.dash");

    // Verify different painter types are parsed correctly
    let painters: Vec<_> = dash
        .gauge_cluster
        .components
        .iter()
        .filter_map(|c| match c {
            DashComponent::Gauge(g) => Some(&g.gauge_painter),
            _ => None,
        })
        .collect();

    assert!(painters
        .iter()
        .any(|p| matches!(p, GaugePainter::Tachometer)));
    assert!(painters
        .iter()
        .any(|p| matches!(p, GaugePainter::VerticalBarGauge)));
    assert!(painters
        .iter()
        .any(|p| matches!(p, GaugePainter::BasicReadout)));
    assert!(painters
        .iter()
        .any(|p| matches!(p, GaugePainter::AsymmetricSweepGauge)));
}

#[test]
fn test_invalid_dash_format() {
    // Test truly malformed XML (not just missing elements)
    let invalid_xml = "<dashboard><unclosed";
    let result = parse_dash_file(invalid_xml);
    assert!(result.is_err(), "Should fail to parse truly malformed XML");
}

#[test]
fn test_empty_dashboard() {
    let empty_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<dashboard>
  <gaugeCluster>
  </gaugeCluster>
</dashboard>"#;

    let result = parse_dash_file(empty_xml);
    assert!(result.is_ok(), "Should parse empty dashboard");

    let dash = result.unwrap();
    assert_eq!(dash.gauge_cluster.components.len(), 0);
}

#[test]
fn test_minimal_gauge() {
    let minimal_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<dashboard>
  <gaugeCluster>
    <dashComp type="Gauge">
      <Title>Test</Title>
      <OutputChannel>test_channel</OutputChannel>
      <GaugePainter>Basic Readout</GaugePainter>
      <Min>0</Min>
      <Max>100</Max>
    </dashComp>
  </gaugeCluster>
</dashboard>"#;

    let result = parse_dash_file(minimal_xml);
    assert!(
        result.is_ok(),
        "Should parse minimal gauge: {:?}",
        result.err()
    );

    let dash = result.unwrap();
    assert_eq!(dash.gauge_cluster.components.len(), 1);

    let gauge = match &dash.gauge_cluster.components[0] {
        DashComponent::Gauge(g) => g.as_ref(),
        _ => panic!("Expected a gauge component"),
    };
    assert_eq!(gauge.title, "Test");
    assert_eq!(gauge.output_channel, "test_channel");
    assert!(matches!(gauge.gauge_painter, GaugePainter::BasicReadout));
}

#[test]
fn test_gauge_file_parsing() {
    // Create a simple .gauge file for testing
    let gauge_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gauge>
  <dashComp type="Gauge">
    <Title>RPM</Title>
    <OutputChannel>rpm</OutputChannel>
    <GaugePainter>Round Analog Gauge</GaugePainter>
    <Min>0</Min>
    <Max>8000</Max>
    <Units>rpm</Units>
  </dashComp>
</gauge>"#;

    let result = parse_gauge_file(gauge_xml);
    assert!(
        result.is_ok(),
        "Should parse .gauge file: {:?}",
        result.err()
    );

    let gauge_file = result.unwrap();
    let gauge = &gauge_file.gauge;
    assert_eq!(gauge.title, "RPM");
    assert_eq!(gauge.output_channel, "rpm");
}
