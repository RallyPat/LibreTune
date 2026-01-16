//! Table and curve lookup tests - verifies tables and curves are parsed and can be looked up

use libretune_core::ini::EcuDefinition;
use std::path::PathBuf;

fn user_project_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // project root
        .join("reference")
        .join("sampleUserProjects")
        .join("BeerMoneyMotorsports1-11-26")
}

#[test]
fn test_user_ini_table_parsing() {
    let ini_path = user_project_dir().join("fome_proteus_f7.ini");
    println!("Testing INI: {:?}", ini_path);
    
    let def = EcuDefinition::from_file(&ini_path)
        .expect("Should parse user's FOME INI");
    
    println!("Signature: {}", def.signature);
    println!("Tables count: {}", def.tables.len());
    println!("table_map_to_name count: {}", def.table_map_to_name.len());
    
    // Print first 10 table names
    println!("\nFirst 10 tables:");
    for (i, name) in def.tables.keys().take(10).enumerate() {
        println!("  {}: {}", i + 1, name);
    }
    
    // Check veTableTbl exists
    assert!(
        def.tables.contains_key("veTableTbl"),
        "veTableTbl should exist in tables. Available keys: {:?}",
        def.tables.keys().take(5).collect::<Vec<_>>()
    );
    
    // Check lookup by name
    let by_name = def.get_table_by_name_or_map("veTableTbl");
    assert!(by_name.is_some(), "Should find veTableTbl by name");
    println!("\nveTableTbl found by name: {:?}", by_name.unwrap().title);
    
    // Check lookup by map name
    let by_map = def.get_table_by_name_or_map("veTableMap");
    assert!(by_map.is_some(), "Should find veTableTbl by map name veTableMap");
    println!("veTableTbl found by map name: {:?}", by_map.unwrap().title);
    
    // Check ignitionTableTbl
    let ign_by_name = def.get_table_by_name_or_map("ignitionTableTbl");
    assert!(ign_by_name.is_some(), "Should find ignitionTableTbl by name");
    println!("ignitionTableTbl found: {:?}", ign_by_name.unwrap().title);
    
    println!("\n✅ All table lookups passed!");
}

#[test]
fn test_table_lookup_by_name_and_map() {
    let ini_path = user_project_dir().join("fome_proteus_f7.ini");
    let def = EcuDefinition::from_file(&ini_path)
        .expect("Should parse INI");
    
    // Both lookup methods should return the same table
    let by_name = def.get_table_by_name_or_map("veTableTbl");
    let by_map = def.get_table_by_name_or_map("veTableMap");
    
    assert!(by_name.is_some(), "Should find by name");
    assert!(by_map.is_some(), "Should find by map");
    assert_eq!(by_name.unwrap().name, by_map.unwrap().name, "Both lookups should return same table");
}

#[test]
fn test_curve_parsing_injectors_dead_time() {
    let ini_path = user_project_dir().join("fome_proteus_f7.ini");
    println!("Testing curve parsing in INI: {:?}", ini_path);
    
    let def = EcuDefinition::from_file(&ini_path)
        .expect("Should parse user's FOME INI");
    
    println!("Curves count: {}", def.curves.len());
    
    // Print all curve names
    println!("\nAll curves:");
    for (i, name) in def.curves.keys().enumerate() {
        println!("  {}: {}", i + 1, name);
    }
    
    // Check injectorsDeadTime curve exists
    assert!(
        def.curves.contains_key("injectorsDeadTime"),
        "injectorsDeadTime curve should exist. Available curves: {:?}",
        def.curves.keys().collect::<Vec<_>>()
    );
    
    // Verify curve properties
    let curve = def.curves.get("injectorsDeadTime").unwrap();
    assert_eq!(curve.name, "injectorsDeadTime");
    assert_eq!(curve.title, "Injector dead time");
    assert_eq!(curve.x_bins, "injector_battLagCorrBins");
    assert_eq!(curve.y_bins, "injector_battLagCorr");
    println!("\n✅ injectorsDeadTime curve parsed correctly!");
    println!("  Title: {}", curve.title);
    println!("  X bins: {}", curve.x_bins);
    println!("  Y bins: {}", curve.y_bins);
    println!("  Column labels: {:?}", curve.column_labels);
}

#[test]
fn test_curve_lookup_by_name_or_map() {
    let ini_path = user_project_dir().join("fome_proteus_f7.ini");
    let def = EcuDefinition::from_file(&ini_path)
        .expect("Should parse INI");
    
    // Direct lookup by name
    let by_name = def.get_curve_by_name_or_map("injectorsDeadTime");
    assert!(by_name.is_some(), "Should find injectorsDeadTime by name");
    assert_eq!(by_name.unwrap().title, "Injector dead time");
    
    println!("✅ Curve lookup by name works!");
}

#[test]
fn test_injector_constants_parsed() {
    let ini_path = user_project_dir().join("fome_proteus_f7.ini");
    let def = EcuDefinition::from_file(&ini_path)
        .expect("Should parse INI");
    
    // Verify the underlying constants for injectorsDeadTime curve exist
    assert!(
        def.constants.contains_key("injector_battLagCorrBins"),
        "injector_battLagCorrBins constant should exist"
    );
    assert!(
        def.constants.contains_key("injector_battLagCorr"),
        "injector_battLagCorr constant should exist"
    );
    
    // Verify isInjectionEnabled constant exists (visibility condition)
    assert!(
        def.constants.contains_key("isInjectionEnabled"),
        "isInjectionEnabled constant should exist for visibility conditions"
    );
    
    println!("✅ Injector-related constants parsed correctly!");
}

#[test]
fn test_injector_config_dialog_has_deadtime_panel() {
    use libretune_core::ini::DialogComponent;
    
    let ini_path = user_project_dir().join("fome_proteus_f7.ini");
    let def = EcuDefinition::from_file(&ini_path)
        .expect("Should parse INI");
    
    // Check injectorConfig dialog exists
    let dialog = def.dialogs.get("injectorConfig")
        .expect("injectorConfig dialog should exist");
    
    println!("injectorConfig dialog has {} components", dialog.components.len());
    
    // Find the injectorsDeadTime panel
    let mut found_panel = false;
    for (i, comp) in dialog.components.iter().enumerate() {
        match comp {
            DialogComponent::Panel { name, visibility_condition } => {
                println!("  Component {}: Panel name='{}', condition={:?}", i, name, visibility_condition);
                if name == "injectorsDeadTime" {
                    found_panel = true;
                    assert_eq!(visibility_condition.as_deref(), Some("isInjectionEnabled"),
                        "Expected visibility_condition to be 'isInjectionEnabled'");
                }
            }
            DialogComponent::Field { name, .. } => {
                println!("  Component {}: Field name='{}'", i, name);
            }
            DialogComponent::Label { text } => {
                println!("  Component {}: Label text='{}'", i, text);
            }
            _ => {
                println!("  Component {}: {:?}", i, comp);
            }
        }
    }
    
    assert!(found_panel, "injectorsDeadTime panel should be in injectorConfig dialog");
    println!("\n✅ injectorConfig dialog contains injectorsDeadTime panel!");
}
