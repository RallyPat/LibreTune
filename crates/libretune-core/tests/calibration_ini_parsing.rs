//! `[ReferenceTables]` parsing across real vendor INIs.
//!
//! Fixtures under `fixtures/calibration/` are the verbatim
//! `[ReferenceTables]` sections of three shipping INIs:
//!
//! * `speeduino_202501_na6.ini` — the project's own `definition.ini`
//!   (thermistor + AFR blocks, ~27 AFR presets);
//! * `ms3.ini` — MegaSquirt 3, which puts *three* identifiers on one
//!   `tableIdentifier` line and wraps `tableLimits` in `#if` blocks;
//! * `ms2extra.ini` — MS2-Extra, whose only block is a MAF table (a fourth
//!   table id, `bytesPerAdc = 2`, `scale = 1`).
//!
//! The section is deliberately parsed generically. The earlier
//! implementation read `[ReferenceTables]` as flat `key = "Label", table`
//! records, which turned every sub-property into its own bogus reference
//! table (`adcCount`, `solution`, `thermOption`, …) and captured none of the
//! calibration metadata.

use libretune_core::ini::{CalibrationSolution, EcuDefinition};

fn parse(fixture: &str) -> EcuDefinition {
    EcuDefinition::from_str(fixture).expect("fixture should parse")
}

fn speeduino() -> EcuDefinition {
    parse(include_str!(
        "fixtures/calibration/speeduino_202501_na6.ini"
    ))
}

#[test]
fn speeduino_declares_exactly_two_blocks() {
    let def = speeduino();
    let mut names: Vec<&str> = def.reference_tables.keys().map(|s| s.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["std_ms2geno2", "std_ms2gentherm"]);
}

#[test]
fn thermistor_block_carries_both_temperature_sensors() {
    let def = speeduino();
    let therm = &def.reference_tables["std_ms2gentherm"];

    assert_eq!(therm.label, "Calibrate Thermistor Tables.");
    assert_eq!(therm.adc_count, 32);
    assert_eq!(therm.bytes_per_adc, 2);
    assert_eq!(therm.scale, 10.0);
    // 32 entries x 2 bytes = the single 64-byte chunk the firmware demands.
    assert_eq!(therm.wire_bytes(), 64);

    // One line declares both: `tableIdentifier = 000, "...", 001, "..."`.
    assert_eq!(therm.identifiers.len(), 2);
    assert_eq!(therm.identifiers[0].id, 0);
    assert_eq!(therm.identifiers[0].label, "Coolant Temperature Sensor");
    assert_eq!(therm.identifiers[1].id, 1);
    assert_eq!(therm.identifiers[1].label, "Air Temperature Sensor");

    // tableLimits lines appear *after* the identifiers they annotate.
    assert_eq!(therm.identifiers[0].limits, Some((-40.0, 350.0, 180.0)));
    assert_eq!(therm.identifiers[1].limits, Some((-40.0, 350.0, 70.0)));

    assert!(therm.topic_help.as_deref().unwrap().contains("wiki"));
}

#[test]
fn thermistor_presets_survive_with_their_measurements() {
    let therm = &speeduino().reference_tables["std_ms2gentherm"];
    assert_eq!(therm.therm_options.len(), 12);

    // The NA6's own sensor.
    let mazda = therm
        .therm_options
        .iter()
        .find(|o| o.name == "Mazda")
        .expect("Mazda preset");
    assert_eq!(mazda.bias_resistor, 50000.0);
    assert_eq!(
        mazda.points,
        [(-40.0, 2022088.0), (21.0, 68273.0), (99.0, 3715.0)]
    );

    // A name containing spaces and a fractional temperature, to pin quoting
    // and number parsing together.
    let vw = therm
        .therm_options
        .iter()
        .find(|o| o.name.starts_with("VW L-Jet"))
        .expect("VW preset");
    assert_eq!(vw.bias_resistor, 1100.0);
    assert_eq!(vw.points[0], (-13.888, 11600.0));
}

#[test]
fn afr_block_matches_the_firmware_o2_table_shape() {
    let o2 = &speeduino().reference_tables["std_ms2geno2"];
    assert_eq!(o2.label, "Calibrate AFR Table...");
    assert_eq!(o2.adc_count, 1024);
    assert_eq!(o2.bytes_per_adc, 1);
    assert_eq!(o2.scale, 10.0);
    // One byte per 10-bit ADC count — the 1024 bytes the `t` command ships.
    assert_eq!(o2.wire_bytes(), 1024);

    assert_eq!(o2.identifiers.len(), 1);
    assert_eq!(o2.identifiers[0].id, 2);
    // The AFR table has no tableLimits line; it must not inherit the
    // thermistor block's.
    assert_eq!(o2.identifiers[0].limits, None);

    assert_eq!(o2.solutions_label.as_deref(), Some("EGO Sensor"));
    assert!(o2.has_identifier(2));
    assert!(!o2.has_identifier(0));
}

#[test]
fn afr_solution_expressions_keep_their_commas_and_braces() {
    let o2 = &speeduino().reference_tables["std_ms2geno2"];

    let find = |label: &str| {
        o2.solutions
            .iter()
            .find(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("preset {label} missing"))
            .1
            .clone()
    };

    assert_eq!(
        find("14Point7"),
        CalibrationSolution::Expression {
            expression: "10.0001 + ( adcValue * 0.0097752 )".to_string()
        }
    );

    // A `table(...)` call contains a comma inside the braces *and* a quoted
    // filename — the field splitter has to respect both.
    assert_eq!(
        find("Narrowband"),
        CalibrationSolution::Expression {
            expression: r#"table(adcValue*5/1023 , "nb.inc")"#.to_string()
        }
    );

    // Generator hand-offs are not expressions.
    assert_eq!(
        find("Custom Linear WB"),
        CalibrationSolution::Generator {
            generator: "linearGenerator".to_string()
        }
    );

    // The stock INI's leading `solution = " ", { }` spacer row would build an
    // all-zero AFR table if it were ever selected, so it is dropped at parse.
    assert!(
        !o2.solutions.iter().any(|(l, _)| l.trim().is_empty()),
        "the blank spacer row should not survive parsing"
    );
    assert!(o2.solutions.len() > 20, "expected the full preset list");
}

#[test]
fn linear_generator_keeps_its_axis_defaults() {
    let o2 = &speeduino().reference_tables["std_ms2geno2"];
    let linear = o2
        .generators
        .iter()
        .find(|g| g.kind == "linearGenerator")
        .expect("linearGenerator");
    assert_eq!(linear.label, "Custom Linear WB");
    assert_eq!(linear.x_units.as_deref(), Some("Volts"));
    assert_eq!(linear.y_units.as_deref(), Some("AFR"));
    assert_eq!(linear.bounds, Some((1.0, 4.0, 9.7, 18.7)));

    // A generator with no axis hints must not invent any.
    let browse = o2
        .generators
        .iter()
        .find(|g| g.kind == "fileBrowseGenerator")
        .expect("fileBrowseGenerator");
    assert_eq!(browse.bounds, None);
}

#[test]
fn section_level_properties_describe_the_wire_format() {
    let def = speeduino();
    // This template is the INI's own statement of the `t` layout, and the
    // reason offset/length are big-endian: %2i/%2o/%2c are TunerStudio's
    // big-endian two-byte fields.
    assert_eq!(
        def.table_write_command.as_deref(),
        Some(r"t\$tsCanId%2i%2o%2c%v")
    );
    // 256 is declared, not a client-side constant. (The `#if mcu_stm32` and
    // `#if COMMS_COMPAT` arms that say 64 are both inactive here.)
    assert_eq!(def.table_blocking_factor, Some(256));
    // The O2 table therefore ships as exactly 4 chunks.
    let o2 = &def.reference_tables["std_ms2geno2"];
    assert_eq!(o2.wire_bytes() / def.table_blocking_factor.unwrap(), 4);
}

#[test]
fn ms3_puts_three_identifiers_on_one_line() {
    let def = parse(include_str!("fixtures/calibration/ms3.ini"));
    let therm = &def.reference_tables["std_ms2gentherm"];

    assert_eq!(therm.identifiers.len(), 3);
    assert_eq!(
        therm.identifiers.iter().map(|i| i.id).collect::<Vec<_>>(),
        [0, 1, 3]
    );
    assert_eq!(therm.identifiers[2].label, "Custom#1 Temperature Sensor");

    // Its tableLimits are spread across an #if/#else; with no symbols
    // defined the #else arm wins, and each limit must land on its own id
    // rather than on whichever identifier came last.
    assert_eq!(therm.identifiers[0].limits, Some((-40.0, 350.0, 180.0)));
    assert_eq!(therm.identifiers[1].limits, Some((-40.0, 350.0, 70.0)));
    assert_eq!(therm.identifiers[2].limits, Some((-40.0, 400.0, 70.0)));

    // MS3 sizes the thermistor table differently from Speeduino, which is
    // exactly why the client must not hardcode 32 points.
    assert_eq!(therm.adc_count, 1024);
    assert_eq!(therm.bytes_per_adc, 2);
    assert_eq!(therm.wire_bytes(), 2048);
}

#[test]
fn ms2extra_declares_a_maf_table() {
    let def = parse(include_str!("fixtures/calibration/ms2extra.ini"));
    let maf = &def.reference_tables["mafTableBurner"];

    assert_eq!(maf.label, "Calibrate MAF Table...");
    // A fourth table id, beyond the CLT/IAT/O2 the Speeduino firmware knows.
    assert_eq!(maf.identifiers.len(), 1);
    assert_eq!(maf.identifiers[0].id, 3);
    assert_eq!(maf.adc_count, 1024);
    assert_eq!(maf.bytes_per_adc, 2);
    assert_eq!(maf.scale, 1.0);
    assert_eq!(def.table_blocking_factor, Some(256));

    // Tab-separated `key<TAB>= value` must parse the same as `key = value`.
    assert!(maf.solutions.len() >= 6);
    assert_eq!(
        maf.solutions[0].1,
        CalibrationSolution::Expression {
            expression: r#"table(adcValue, "maffactor.inc")"#.to_string()
        }
    );
}

#[test]
fn no_sub_property_becomes_a_reference_table_of_its_own() {
    // The regression that motivated the rewrite: every indented property was
    // previously inserted as its own reference table.
    for fixture in [
        include_str!("fixtures/calibration/speeduino_202501_na6.ini"),
        include_str!("fixtures/calibration/ms3.ini"),
        include_str!("fixtures/calibration/ms2extra.ini"),
    ] {
        let def = parse(fixture);
        for bogus in [
            "adcCount",
            "bytesPerAdc",
            "scale",
            "solution",
            "thermOption",
            "tableIdentifier",
            "tableGenerator",
            "topicHelp",
            "tableLimits",
            "solutionsLabel",
            "tableWriteCommand",
            "tableBlockingFactor",
        ] {
            assert!(
                !def.reference_tables.contains_key(bogus),
                "sub-property `{bogus}` was parsed as a reference table"
            );
        }
    }
}
