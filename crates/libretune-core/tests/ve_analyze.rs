//! Tests for VeAnalyze, WueAnalyze, and GammaE section parsing

use libretune_core::ini::EcuDefinition;
use libretune_core::ini::FilterOperator;

#[test]
fn test_ve_analyze_parsing() {
    let ini_content = r#"
[MegaTune]
    signature = "TestVeAnalyze"
    queryCommand = "Q"

[VeAnalyze]
    veAnalyzeMap = veTableTbl, lambdaTableTbl, lambdaValue, egoCorrectionForVeAnalyze, { 1 }
    lambdaTargetTables = lambdaTableTbl, afrTSCustom
    
    filter = minRPMFilter, "Minimum RPM", RPMValue, <, 500, true
    filter = minCltFilter, "Minimum CLT", coolant, <, 60, true
    filter = deltaTps, "dTPS", deltaTps, >, 50, true
    filter = std_Custom
    
    option = disableLiveUpdates
    option = burnOnSend
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse successfully");
    
    assert!(def.ve_analyze.is_some(), "VeAnalyze config should be present");
    let config = def.ve_analyze.unwrap();
    
    assert_eq!(config.ve_table_name, "veTableTbl");
    assert_eq!(config.target_table_name, "lambdaTableTbl");
    assert_eq!(config.lambda_channel, "lambdaValue");
    assert_eq!(config.ego_correction_channel, "egoCorrectionForVeAnalyze");
    assert_eq!(config.active_condition, "{ 1 }");
    
    assert_eq!(config.lambda_target_tables.len(), 2);
    assert_eq!(config.lambda_target_tables[0], "lambdaTableTbl");
    assert_eq!(config.lambda_target_tables[1], "afrTSCustom");
    
    // Check filters
    assert_eq!(config.filters.len(), 4);
    
    let rpm_filter = &config.filters[0];
    assert_eq!(rpm_filter.name, "minRPMFilter");
    assert_eq!(rpm_filter.display_name, "Minimum RPM");
    assert_eq!(rpm_filter.channel, "RPMValue");
    assert_eq!(rpm_filter.operator, FilterOperator::LessThan);
    assert_eq!(rpm_filter.default_value, 500.0);
    assert!(rpm_filter.user_adjustable);
    
    let delta_tps = &config.filters[2];
    assert_eq!(delta_tps.operator, FilterOperator::GreaterThan);
    assert_eq!(delta_tps.default_value, 50.0);
    
    // std_Custom filter
    let std_custom = &config.filters[3];
    assert_eq!(std_custom.name, "std_Custom");
    
    // Options
    assert_eq!(config.options.len(), 2);
    assert!(config.options.contains(&"disableLiveUpdates".to_string()));
    assert!(config.options.contains(&"burnOnSend".to_string()));
}

#[test]
fn test_wue_analyze_parsing() {
    let ini_content = r#"
[MegaTune]
    signature = "TestWueAnalyze"
    queryCommand = "Q"

[WueAnalyze]
    wueAnalyzeMap = wueCurve, wueAfrOffsetCurve, lambdaTableTbl, lambdaValue, coolant, wueEnrich, egoCorrection
    lambdaTargetTables = lambdaTableTbl, afrTSCustom
    wuePercentOffset = 100
    
    filter = highThrottle, "High Throttle", throttle, >, 15, true
    filter = lowRpm, "Low RPM", rpm, <, 300, false
    
    option = burnOnSend
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse successfully");
    
    assert!(def.wue_analyze.is_some(), "WueAnalyze config should be present");
    let config = def.wue_analyze.unwrap();
    
    assert_eq!(config.wue_curve_name, "wueCurve");
    assert_eq!(config.afr_temp_comp_curve, "wueAfrOffsetCurve");
    assert_eq!(config.target_table_name, "lambdaTableTbl");
    assert_eq!(config.lambda_channel, "lambdaValue");
    assert_eq!(config.coolant_channel, "coolant");
    assert_eq!(config.wue_channel, "wueEnrich");
    assert_eq!(config.ego_correction_channel, "egoCorrection");
    
    assert_eq!(config.wue_percent_offset, 100.0);
    
    assert_eq!(config.filters.len(), 2);
    assert_eq!(config.options.len(), 1);
}

#[test]
fn test_filter_operators() {
    let ini_content = r#"
[MegaTune]
    signature = "TestFilterOperators"
    queryCommand = "Q"

[VeAnalyze]
    veAnalyzeMap = veTable, targetTable, lambda, ego, { 1 }
    
    filter = lessThan, "Less Than", channel1, <, 100, true
    filter = greaterThan, "Greater Than", channel2, >, 50, true
    filter = equals, "Equals", channel3, =, 0, true
    filter = notEquals, "Not Equals", channel4, !=, 1, true
    filter = bitwiseAnd, "Bitwise AND", channel5, &, 4, true
    filter = bitwiseOr, "Bitwise OR", channel6, |, 8, true
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse successfully");
    let config = def.ve_analyze.unwrap();
    
    assert_eq!(config.filters[0].operator, FilterOperator::LessThan);
    assert_eq!(config.filters[1].operator, FilterOperator::GreaterThan);
    assert_eq!(config.filters[2].operator, FilterOperator::Equal);
    assert_eq!(config.filters[3].operator, FilterOperator::NotEqual);
    assert_eq!(config.filters[4].operator, FilterOperator::BitwiseAnd);
    assert_eq!(config.filters[5].operator, FilterOperator::BitwiseOr);
}

#[test]
fn test_real_corpus_ve_analyze() {
    // Test with a realistic VeAnalyze section from rusEFI
    let ini_content = r#"
[MegaTune]
    signature = "rusEFI"
    queryCommand = "S"

[VeAnalyze]
    veAnalyzeMap = veTableTbl, lambdaTableTbl, lambdaValue, egoCorrectionForVeAnalyze, { 1 }
    lambdaTargetTables = lambdaTableTbl, afrTSCustom

    filter = minRPMFilter, "Minimum RPM", RPMValue, <, 500, true
    filter = minCltFilter, "Minimum CLT", coolant, <, 60, true
    filter = deltaTps, "dTPS", deltaTps, >, 50, true
    filter = VBatt, "VBatt", VBatt, <, 12, true
    filter = minTps, "Minimum TPS", TPSValue, <, 1, true
    filter = std_Custom
    filter = minPps, "Minimum PPS", throttlePedalPosition, <, 3, true
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse real corpus VeAnalyze");
    
    let config = def.ve_analyze.expect("VeAnalyze should be parsed");
    assert_eq!(config.ve_table_name, "veTableTbl");
    assert_eq!(config.filters.len(), 7);
}
