//! Tune migration detection and handling
//!
//! Compares tune file constant manifests with current INI definitions
//! to detect version differences and suggest migrations.

use serde::{Deserialize, Serialize};

use super::ConstantManifestEntry;
use crate::ini::EcuDefinition;

/// A report of differences between a tune's saved manifest and current INI
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MigrationReport {
    /// Constants in the new INI that weren't in the tune
    pub missing_in_tune: Vec<String>,
    
    /// Constants in the tune that aren't in the new INI
    pub missing_in_ini: Vec<String>,
    
    /// Constants whose type or shape changed
    pub type_changed: Vec<ConstantChange>,
    
    /// Constants whose scale/translate changed (may affect values)
    pub scale_changed: Vec<ConstantChange>,
    
    /// True if all changes can be auto-migrated safely
    pub can_auto_migrate: bool,
    
    /// True if user should review changes before applying
    pub requires_user_review: bool,
    
    /// Overall severity: "none", "low", "medium", "high"
    pub severity: String,
}

/// Details about a constant that changed between INI versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantChange {
    /// Constant name
    pub name: String,
    
    /// Old value (from tune manifest)
    pub old_type: String,
    pub old_page: u8,
    pub old_offset: u16,
    pub old_scale: f64,
    pub old_translate: f64,
    
    /// New value (from current INI)
    pub new_type: String,
    pub new_page: u8,
    pub new_offset: u16,
    pub new_scale: f64,
    pub new_translate: f64,
    
    /// Description of what changed
    pub change_description: String,
}

impl MigrationReport {
    /// Create an empty report (no changes detected)
    pub fn empty() -> Self {
        Self {
            missing_in_tune: Vec::new(),
            missing_in_ini: Vec::new(),
            type_changed: Vec::new(),
            scale_changed: Vec::new(),
            can_auto_migrate: true,
            requires_user_review: false,
            severity: "none".to_string(),
        }
    }
    
    /// Check if there are any differences
    pub fn has_changes(&self) -> bool {
        !self.missing_in_tune.is_empty()
            || !self.missing_in_ini.is_empty()
            || !self.type_changed.is_empty()
            || !self.scale_changed.is_empty()
    }
    
    /// Get a summary of the changes
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        
        if !self.missing_in_tune.is_empty() {
            parts.push(format!("{} new constants", self.missing_in_tune.len()));
        }
        if !self.missing_in_ini.is_empty() {
            parts.push(format!("{} removed constants", self.missing_in_ini.len()));
        }
        if !self.type_changed.is_empty() {
            parts.push(format!("{} type changes", self.type_changed.len()));
        }
        if !self.scale_changed.is_empty() {
            parts.push(format!("{} scale changes", self.scale_changed.len()));
        }
        
        if parts.is_empty() {
            "No changes detected".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Compare a tune's saved manifest against current INI definition
pub fn compare_manifests(
    saved_manifest: &[ConstantManifestEntry],
    current_ini: &EcuDefinition,
) -> MigrationReport {
    let mut report = MigrationReport::empty();
    
    // Build lookup from manifest entries
    let saved_map: std::collections::HashMap<&str, &ConstantManifestEntry> = 
        saved_manifest.iter().map(|e| (e.name.as_str(), e)).collect();
    
    // Check for constants in INI but not in tune (new constants)
    for (name, constant) in &current_ini.constants {
        if !saved_map.contains_key(name.as_str()) {
            // Skip PC variables (they're local-only)
            if constant.is_pc_variable {
                continue;
            }
            report.missing_in_tune.push(name.clone());
        }
    }
    
    // Check each saved constant against current INI
    for entry in saved_manifest {
        if let Some(current) = current_ini.constants.get(&entry.name) {
            // Constant exists in both - check for changes
            let current_type = format!("{:?}", current.data_type);
            
            // Check if type changed
            if entry.data_type != current_type {
                report.type_changed.push(ConstantChange {
                    name: entry.name.clone(),
                    old_type: entry.data_type.clone(),
                    old_page: entry.page,
                    old_offset: entry.offset,
                    old_scale: entry.scale,
                    old_translate: entry.translate,
                    new_type: current_type.clone(),
                    new_page: current.page,
                    new_offset: current.offset,
                    new_scale: current.scale,
                    new_translate: current.translate,
                    change_description: format!(
                        "Type changed from {} to {}",
                        entry.data_type, current_type
                    ),
                });
            }
            // Check if scale/translate changed (affects display values)
            else if (entry.scale - current.scale).abs() > 1e-9 
                || (entry.translate - current.translate).abs() > 1e-9 
            {
                report.scale_changed.push(ConstantChange {
                    name: entry.name.clone(),
                    old_type: entry.data_type.clone(),
                    old_page: entry.page,
                    old_offset: entry.offset,
                    old_scale: entry.scale,
                    old_translate: entry.translate,
                    new_type: current_type,
                    new_page: current.page,
                    new_offset: current.offset,
                    new_scale: current.scale,
                    new_translate: current.translate,
                    change_description: format!(
                        "Scale changed from {}*x+{} to {}*x+{}",
                        entry.scale, entry.translate,
                        current.scale, current.translate
                    ),
                });
            }
        } else {
            // Constant was in tune but not in current INI
            report.missing_in_ini.push(entry.name.clone());
        }
    }
    
    // Determine severity and migration flags
    if !report.type_changed.is_empty() || !report.missing_in_ini.is_empty() {
        report.requires_user_review = true;
        report.can_auto_migrate = false;
        report.severity = "high".to_string();
    } else if !report.scale_changed.is_empty() {
        report.requires_user_review = true;
        report.can_auto_migrate = true;
        report.severity = "medium".to_string();
    } else if !report.missing_in_tune.is_empty() {
        // Only new constants - can auto-apply defaults
        report.can_auto_migrate = true;
        report.requires_user_review = false;
        report.severity = "low".to_string();
    }
    
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_manifest_comparison() {
        // Empty manifest should report all INI constants as missing
        let manifest = vec![];
        let def = EcuDefinition::default();
        
        let report = compare_manifests(&manifest, &def);
        assert!(!report.has_changes());
        assert_eq!(report.severity, "none");
    }
    
    #[test]
    fn test_migration_report_summary() {
        let mut report = MigrationReport::empty();
        report.missing_in_tune = vec!["const1".to_string(), "const2".to_string()];
        report.missing_in_ini = vec!["old_const".to_string()];
        
        let summary = report.summary();
        assert!(summary.contains("2 new constants"));
        assert!(summary.contains("1 removed constants"));
    }
}
