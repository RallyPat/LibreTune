# Version Control & Tune Migration

LibreTune uses Git for version control of tune files and implements structural fingerprinting for detecting INI version incompatibilities.

## Git Integration

### Repository Structure

Each LibreTune project is a Git repository:

```
MyProject/
├── .git/                    # Git metadata
├── CurrentTune.msq          # Active tune file
├── ProjectSettings.json     # Project configuration
├── pcVariableValues.msq     # PC variables
├── RestorePoints/           # Backup MSQ files
│   ├── 2026-02-01_14-30-00.msq
│   └── 2026-02-02_09-15-45.msq
└── dashboards/              # Custom dashboards
    └── racing.ltdash.xml
```

### Auto-Commit System

**Commit Triggers**:
1. Manual save (Ctrl+S)
2. Burn to ECU
3. Restore point creation
4. Project close (if unsaved changes)

**User Settings**:
```rust
pub enum AutoCommitMode {
    Always,   // Commit every save automatically
    Never,    // Manual commits only (git command palette)
    Ask,      // Prompt user with dialog
}
```

**Commit Message Format**:
```
LibreTune: {commit_type} - {timestamp}

{user_annotation}

Modified tables: {table_names}
Changed constants: {constant_names}
```

**Placeholders**:
- `{date}` → 2026-02-03
- `{time}` → 14:35:22
- `{table}` → veTable1
- `{user}` → System user name
- `{signature}` → INI signature (truncated)

### Change Annotations

**User Annotation Dialog**:
```
┌─────────────────────────────────────┐
│ Commit Message                      │
│                                     │
│ ┌─────────────────────────────────┐│
│ │ Adjusted VE table for E85       ││
│ │ Increased fuel by 10% across    ││
│ │ mid-range RPM                   ││
│ └─────────────────────────────────┘│
│                                     │
│     [Cancel]  [Commit]              │
└─────────────────────────────────────┘
```

**Storage Format**:
```
commit 1a2b3c4d5e6f...
Author: Pat <pat@example.com>
Date:   Sat Feb 3 14:35:22 2026 -0800

    LibreTune: Manual save - 2026-02-03 14:35:22
    
    LT-Note: Adjusted VE table for E85
    LT-Note: Increased fuel by 10% across mid-range RPM
    
    Modified tables: veTable1Tbl
    Changed constants: crankingRpm, crankingDwell
```

**Parsing**:
```rust
pub fn extract_annotation(commit_message: &str) -> Option<String> {
    let lines: Vec<&str> = commit_message.lines()
        .filter(|line| line.starts_with("LT-Note:"))
        .map(|line| line.strip_prefix("LT-Note:").unwrap().trim())
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}
```

### Commit History

**Timeline View**:
```
┌────────────────────────────────────────────┐
│ Commit History                             │
│                                            │
│ ● 2026-02-03 14:35  Manual save           │
│   1a2b3c4d                                 │
│   Adjusted VE table for E85                │
│   [View Diff] [Checkout]                   │
│                                            │
│ ● 2026-02-03 11:20  Burn to ECU           │
│   2b3c4d5e                                 │
│   Baseline tune after dyno session         │
│   [View Diff] [Checkout]                   │
│                                            │
│ ● 2026-02-02 16:45  Manual save           │
│   3c4d5e6f                                 │
│   Initial commit                           │
│   [View Diff] [Checkout]                   │
└────────────────────────────────────────────┘
```

**Diff View**:
```
┌────────────────────────────────────────────┐
│ Commit Diff: 1a2b3c4d vs 2b3c4d5e         │
│                                            │
│ Modified Files:                            │
│   CurrentTune.msq                          │
│                                            │
│ Changed Constants (2):                     │
│   crankingRpm:   500 → 550 (+50)          │
│   crankingDwell: 3.5 → 4.0 (+0.5)         │
│                                            │
│ Changed Tables (1):                        │
│   veTable1Tbl: 256 cells modified          │
│   Average change: +8.3%                    │
│                                            │
│         [Close]  [Checkout This Version]   │
└────────────────────────────────────────────┘
```

### Branch Management

**Branch Creation**:
```rust
pub fn create_branch(&self, branch_name: &str) -> Result<(), String> {
    let repo = Repository::open(&self.project_path)?;
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    repo.branch(branch_name, &commit, false)?;
    Ok(())
}
```

**Use Cases**:
- **Baseline branch**: Stock tune from manufacturer
- **Development branches**: Experimental tunes
- **Customer branches**: Different vehicle configurations

**Example Workflow**:
```
main (stock tune)
├── dev/e85 (E85 conversion)
├── dev/turbo (turbo upgrade)
└── customer/john-civic (customer-specific tune)
```

## Tune Fingerprinting

### Structural Hash

**Purpose**: Detect when INI definition changes in a way that breaks tune compatibility

**Hash Calculation**:
```rust
pub fn compute_structural_hash(def: &EcuDefinition) -> String {
    let mut hasher = Sha256::new();
    
    // Hash all non-PC constants (sorted by name for determinism)
    let mut constants: Vec<_> = def.constants.iter()
        .filter(|(_, c)| !c.is_pc_variable)
        .collect();
    constants.sort_by_key(|(name, _)| *name);
    
    for (name, c) in constants {
        hasher.update(name.as_bytes());
        hasher.update(&[c.data_type as u8]);
        hasher.update(&c.page.to_le_bytes());
        hasher.update(&c.offset.to_le_bytes());
        hasher.update(&c.scale.to_le_bytes());
    }
    
    format!("{:x}", hasher.finalize())
}
```

**What's Hashed**:
- Constant names
- Data types
- Page numbers
- Offsets
- Scale factors

**What's NOT Hashed**:
- PC variables (user-configurable, not ECU data)
- Display properties (units, min/max, digits)
- Documentation (help text, tooltips)

### Constant Manifest

**Structure**:
```rust
pub struct ConstantManifestEntry {
    pub name: String,
    pub data_type: DataType,
    pub page: u8,
    pub offset: u16,
    pub scale: f64,
    pub translate: f64,
}
```

**Stored in Tune File**:
```xml
<msq>
  <bibliography>...</bibliography>
  
  <iniMetadata>
    <signature>rusEFI 2025.08.19</signature>
    <name>rusEFI Proteus F4</name>
    <hash>a1b2c3d4e5f6...</hash>
    <specVersion>1.0</specVersion>
    <savedAt>2026-02-03T14:35:22Z</savedAt>
  </iniMetadata>
  
  <constantManifest>
    <entry name="crankingRpm" type="U16" page="1" offset="0" scale="1.0" translate="0.0"/>
    <entry name="coolantTemp" type="U08" page="1" offset="10" scale="1.0" translate="-40.0"/>
    ...
  </constantManifest>
  
  <page number="0">...</page>
</msq>
```

## Tune Migration

### Migration Detection

**When Loading Tune**:
```rust
pub fn load_tune(&self, path: &str) -> Result<TuneFile, String> {
    let tune = TuneFile::load_from_file(path)?;
    
    if let Some(manifest) = &tune.constant_manifest {
        let current_manifest = self.definition.generate_constant_manifest();
        let report = compare_manifests(manifest, &current_manifest);
        
        if report.severity != "none" {
            emit_event("tune:migration_needed", &report);
        }
    }
    
    Ok(tune)
}
```

**Comparison Logic**:
```rust
pub struct MigrationReport {
    pub missing_in_tune: Vec<String>,      // New constants in INI
    pub missing_in_ini: Vec<String>,       // Removed constants
    pub type_changed: Vec<ConstantChange>, // Data type changes
    pub scale_changed: Vec<ConstantChange>, // Scale/translate changes
    pub can_auto_migrate: bool,
    pub requires_user_review: bool,
    pub severity: String,  // "none", "low", "medium", "high"
}

pub struct ConstantChange {
    pub name: String,
    pub old_type: Option<DataType>,
    pub new_type: Option<DataType>,
    pub old_scale: Option<f64>,
    pub new_scale: Option<f64>,
}
```

**Severity Levels**:
- **None**: Identical manifests, no migration needed
- **Low**: Only new constants added (tune will work as-is)
- **Medium**: Scale factors changed (values need recalculation)
- **High**: Constants removed or types changed (potential data loss)

### Migration Dialog

**User Interface**:
```
┌─────────────────────────────────────────────────┐
│ ⚠ Tune Migration Required                      │
│                                                 │
│ This tune was created with a different INI     │
│ version. Review the changes below:             │
│                                                 │
│ Severity: HIGH                                  │
│                                                 │
│ ❌ Type Changes (2):                            │
│   • crankingRpm: U16 → U32                     │
│   • coolantTemp: U08 → S16                     │
│                                                 │
│ ⚠ Scale Changes (3):                            │
│   • afrTable: scale 0.1 → 0.01                 │
│   • mapSensor: scale 1.0 → 0.1                 │
│   • tpsPin: scale 1.0 → 0.2                    │
│                                                 │
│ ➕ New Constants (5):                           │
│   • fuelPressure, boostTarget, ...             │
│                                                 │
│ ➖ Removed Constants (1):                       │
│   • oldInjectorType                            │
│                                                 │
│ ⚠ High severity changes may cause issues.      │
│ Consider creating a backup before proceeding.   │
│                                                 │
│       [Cancel]  [Continue with Tune]            │
└─────────────────────────────────────────────────┘
```

### Auto-Migration

**Safe Migrations** (automatic):
1. **New constants**: Default to INI default value
2. **Scale changes**: Recalculate display value
   ```
   old_display = (raw_bytes * old_scale) + old_translate
   new_raw_bytes = (old_display - new_translate) / new_scale
   ```

**Manual Migrations** (require user input):
1. **Type changes**: Prompt for new value or use default
2. **Removed constants**: Warn about data loss
3. **Offset changes**: Suggest full re-tune from ECU

### Restore Points

**Auto-Generated Restore Points**:
- Before migration (pre-migration state)
- Before checkout (pre-checkout state)
- On manual save (if setting enabled)

**Pruning Strategy**:
```rust
pub fn prune_restore_points(&self, keep: usize) -> Result<(), String> {
    let mut points = self.list_restore_points()?;
    points.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    for point in points.iter().skip(keep) {
        self.delete_restore_point(&point.filename)?;
    }
    
    Ok(())
}
```

**Default Settings**:
- Keep: 20 most recent
- Auto-prune: On save
- Warn before deletion

## Performance Characteristics

| Operation | Complexity | Typical Time |
|-----------|-----------|--------------|
| Compute hash | O(n) | <1ms (1000 constants) |
| Generate manifest | O(n) | <1ms |
| Compare manifests | O(n + m) | <5ms |
| Git commit | O(p) | <50ms (p=changed pages) |
| Git diff | O(c) | <20ms (c=commits) |

## Source Code Reference

- Git integration: `crates/libretune-core/src/project/version_control.rs`
- Migration detection: `crates/libretune-core/src/tune/migration.rs`
- Fingerprinting: `crates/libretune-core/src/ini/mod.rs` (compute_structural_hash)
- Frontend UI: 
  - `crates/libretune-app/src/components/dialogs/MigrationReportDialog.tsx`
  - `crates/libretune-app/src/components/tuner-ui/TuneHistoryPanel.tsx`
- Tauri commands: `crates/libretune-app/src-tauri/src/lib.rs`

## See Also

- [Version Control](../projects/version-control.md) - User guide for Git features
- [Change Annotations](../projects/change-annotations.md) - Annotating tune changes
- [Restore Points](../projects/restore-points.md) - Using restore points
- [Settings & Preferences](../getting-started/settings.md) - Configuring auto-commit
