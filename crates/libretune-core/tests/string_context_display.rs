//! Regression for #128: braced gauge titles via stringValue().

use libretune_core::ini::expression::{evaluate_display_string, StringContext};
use libretune_core::ini::{parse_gauge_line, EcuDefinition};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn demo_ini_gppwm_gauge_title_resolves_with_string_context() {
    let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../libretune-app/src-tauri/resources/demo.ini");
    if !demo.exists() {
        // Workspace layout may differ in some CI checkouts; skip rather than fail.
        eprintln!("demo.ini not found at {:?}, skipping", demo);
        return;
    }

    let def = EcuDefinition::from_file(demo.to_str().unwrap()).expect("parse demo.ini");
    let gauge = def.gauges.get("gppwmGauge1").expect("gppwmGauge1");
    assert!(
        gauge.title.contains("stringValue"),
        "expected braced title, got {}",
        gauge.title
    );

    let mut string_ctx = StringContext::default();
    string_ctx.get_string_value = Some(Box::new(|name| {
        if name == "gpPwmNote1" {
            Some("gpPwmNote 1".to_string())
        } else {
            None
        }
    }));

    let resolved = evaluate_display_string(&gauge.title, &HashMap::new(), Some(&string_ctx));
    assert_eq!(resolved, "gpPwmNote 1");
    assert_eq!(
        evaluate_display_string(&gauge.title, &HashMap::new(), None),
        ""
    );
}

#[test]
fn parse_and_resolve_string_value_title() {
    let gauge = parse_gauge_line(
        "gppwmGauge1",
        "gppwmOutput1, { stringValue(gpPwmNote1) }, \"%\", 0, 100, 0, 0, 100, 100, 1",
    )
    .unwrap();

    let mut string_ctx = StringContext::default();
    string_ctx.get_string_value = Some(Box::new(|_| Some("Radiator Fan".into())));
    assert_eq!(
        evaluate_display_string(&gauge.title, &HashMap::new(), Some(&string_ctx)),
        "Radiator Fan"
    );
}
