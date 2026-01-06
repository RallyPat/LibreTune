use libretune_core::ini::{EcuDefinition, Menu, DialogDefinition, Constant, MenuItem, HelpTopic, DataType, AdaptiveTimingConfig};
use libretune_core::protocol::serial::list_ports;
use libretune_core::protocol::{Connection, ConnectionConfig, ConnectionState};
use libretune_core::autotune::{AutoTuneState, AutoTuneSettings, AutoTuneFilters, AutoTuneAuthorityLimits, AutoTuneRecommendation};
use libretune_core::dashboard::{DashboardLayout, GaugeConfig, get_dashboard_file, get_dashboard_file_path};
use libretune_core::dash::{self, DashFile, DashComponent, GaugePainter, TsColor, Bibliography, VersionInfo};
use libretune_core::datalog::{DataLogger, LogEntry};
use libretune_core::demo::DemoSimulator;
use libretune_core::table_ops;
use libretune_core::tune::{TuneFile, TuneValue, TuneCache, PageState};
use libretune_core::project::{Project, ProjectConfig, IniRepository, IniEntry, ProjectInfo, ConnectionSettings, ProjectSettings, OnlineIniRepository, OnlineIniEntry, IniSource};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use std::fs;
use std::io::Write;
use tauri::Emitter;
use tauri::Manager;

/// Get the LibreTune app data directory (cross-platform)
fn get_app_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir()
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("LibreTune")
        })
}

/// Get the projects directory (cross-platform)
fn get_projects_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("projects")
}

/// Get the definitions directory (cross-platform)
fn get_definitions_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("definitions")
}

/// Get the settings file path (cross-platform)
fn get_settings_path(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("settings.json")
}

/// Get the dashboards directory (cross-platform)
fn get_dashboards_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("dashboards")
}

struct AppState {
    connection: Mutex<Option<Connection>>,
    definition: Mutex<Option<EcuDefinition>>,
    autotune_state: Mutex<AutoTuneState>,
    streaming_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    // Background task for AutoTune auto-send
    autotune_send_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    current_tune: Mutex<Option<TuneFile>>,
    current_tune_path: Mutex<Option<PathBuf>>,
    tune_modified: Mutex<bool>,
    data_logger: Mutex<DataLogger>,
    current_project: Mutex<Option<Project>>,
    ini_repository: Mutex<Option<IniRepository>>,
    // Online INI repository for downloading INIs from GitHub
    online_ini_repository: Mutex<OnlineIniRepository>,
    // Local cache of ECU page data for offline editing
    tune_cache: Mutex<Option<TuneCache>>,
    // Demo mode - simulates a running vehicle for UI testing
    demo_mode: Mutex<bool>,
}

#[derive(Serialize)]
struct ConnectionStatus {
    state: ConnectionState,
    signature: Option<String>,
    has_definition: bool,
    ini_name: Option<String>,
    demo_mode: bool,
}

/// Signature match type for comparing ECU and INI signatures
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum SignatureMatchType {
    /// Signatures match exactly
    Exact,
    /// Signatures match partially (one contains the other, version diff)
    Partial,
    /// Signatures do not match
    Mismatch,
}

/// Information about a signature mismatch for the frontend
#[derive(Serialize, Clone)]
struct SignatureMismatchInfo {
    /// The signature reported by the ECU
    ecu_signature: String,
    /// The signature expected by the loaded INI file
    ini_signature: String,
    /// How closely the signatures match
    match_type: SignatureMatchType,
    /// Path to the currently loaded INI
    current_ini_path: Option<String>,
    /// List of INIs that might match the ECU signature
    matching_inis: Vec<MatchingIniInfo>,
}

/// Information about an INI that matches the ECU signature
#[derive(Serialize, Clone)]
struct MatchingIniInfo {
    /// Path to the INI file
    path: String,
    /// Display name of the INI
    name: String,
    /// Signature from this INI
    signature: String,
    /// How well it matches (exact or partial)
    match_type: SignatureMatchType,
}

/// Result of ECU connection attempt
#[derive(Serialize)]
struct ConnectResult {
    /// The signature reported by the ECU
    signature: String,
    /// Mismatch info if signatures don't match exactly
    mismatch_info: Option<SignatureMismatchInfo>,
}

/// Result of ECU sync operation
#[derive(Serialize)]
struct SyncResult {
    /// Whether all pages synced successfully
    success: bool,
    /// Number of pages successfully synced
    pages_synced: u8,
    /// Number of pages that failed to sync
    pages_failed: u8,
    /// Total number of pages attempted
    total_pages: u8,
    /// Error messages for failed pages (for logging)
    errors: Vec<String>,
}

/// Extended constant info for frontend with value_type field
#[derive(Serialize)]
struct ConstantInfo {
    name: String,
    label: Option<String>,
    units: String,
    digits: u8,
    min: f64,
    max: f64,
    value_type: String,  // "scalar", "string", "bits", "array"
    bit_options: Vec<String>,
    help: Option<String>,
    visibility_condition: Option<String>,  // Expression for when field should be visible
}

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    last_ini_path: Option<String>,
    units_system: String,        // "metric" or "imperial"
    auto_burn_on_close: bool,   // Auto-burn toggle
    gauge_snap_to_grid: bool,    // Dashboard gauge snap to grid
    gauge_free_move: bool,       // Dashboard gauge free move
    gauge_lock: bool,           // Dashboard gauge lock in place
    indicator_column_count: String,  // "auto" or number like "12"
    indicator_fill_empty: bool,      // Fill empty cells in last row
    indicator_text_fit: String,      // "scale" or "wrap"
}

fn save_settings(app: &tauri::AppHandle, settings: &Settings) {
    let settings_path = get_settings_path(app);
    // Ensure parent directory exists
    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&settings_path, json);
    }
}

fn load_settings(app: &tauri::AppHandle) -> Settings {
    let settings_path = get_settings_path(app);
    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        if let Ok(settings) = serde_json::from_str(&content) {
            return settings;
        }
    }
    Settings::default()
}

// =============================================================================
// Dashboard Format Conversion Helpers
// =============================================================================

/// Convert legacy DashboardLayout to TunerStudio DashFile format
fn convert_layout_to_dashfile(layout: &DashboardLayout) -> DashFile {
    use libretune_core::dashboard::GaugeType;
    use libretune_core::dash::{GaugeCluster, BackgroundStyle};
    
    let mut dash = DashFile {
        bibliography: Bibliography {
            author: "LibreTune".to_string(),
            company: "LibreTune Project".to_string(),
            write_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
        version_info: VersionInfo {
            file_format: "3.0".to_string(),
            firmware_signature: None,
        },
        gauge_cluster: GaugeCluster {
            anti_aliasing: true,
            cluster_background_color: TsColor { alpha: 255, red: 30, green: 30, blue: 30 },
            background_dither_color: None,
            cluster_background_image_file_name: layout.background_image.clone(),
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };
    
    for gauge in &layout.gauges {
        let painter = match gauge.gauge_type {
            GaugeType::AnalogDial => GaugePainter::AnalogGauge,
            GaugeType::DigitalReadout => GaugePainter::BasicReadout,
            GaugeType::BarGauge => GaugePainter::HorizontalBarGauge,
            GaugeType::SweepGauge => GaugePainter::AsymmetricSweepGauge,
            GaugeType::LEDIndicator | GaugeType::WarningLight => GaugePainter::BasicReadout,
        };
        
        let ts_gauge = dash::GaugeConfig {
            id: gauge.id.clone(),
            title: gauge.label.clone(),
            units: gauge.units.clone(),
            output_channel: gauge.channel.clone(),
            min: gauge.min_value,
            max: gauge.max_value,
            low_warning: gauge.low_warning,
            high_warning: gauge.high_warning,
            high_critical: gauge.high_critical,
            value_digits: gauge.decimals as i32,
            relative_x: gauge.x,
            relative_y: gauge.y,
            relative_width: gauge.width,
            relative_height: gauge.height,
            gauge_painter: painter,
            font_color: parse_hex_color(&gauge.font_color),
            needle_color: parse_hex_color(&gauge.needle_color),
            trim_color: parse_hex_color(&gauge.trim_color),
            show_history: gauge.show_history,
            ..Default::default()
        };
        
        dash.gauge_cluster.components.push(DashComponent::Gauge(ts_gauge));
    }
    
    dash
}

/// Convert TunerStudio DashFile to legacy DashboardLayout format
fn convert_dashfile_to_layout(dash: &DashFile, name: &str) -> DashboardLayout {
    use libretune_core::dashboard::GaugeType;
    
    let mut layout = DashboardLayout {
        name: name.to_string(),
        gauges: Vec::new(),
        is_fullscreen: false,
        background_image: dash.gauge_cluster.cluster_background_image_file_name.clone(),
    };
    
    for (idx, component) in dash.gauge_cluster.components.iter().enumerate() {
        if let DashComponent::Gauge(g) = component {
            let gauge_type = match g.gauge_painter {
                GaugePainter::AnalogGauge | GaugePainter::BasicAnalogGauge | 
                GaugePainter::CircleAnalogGauge => GaugeType::AnalogDial,
                GaugePainter::BasicReadout => GaugeType::DigitalReadout,
                GaugePainter::HorizontalBarGauge | GaugePainter::VerticalBarGauge |
                GaugePainter::HorizontalLineGauge | GaugePainter::VerticalDashedBar |
                GaugePainter::AnalogBarGauge | GaugePainter::AnalogMovingBarGauge |
                GaugePainter::Histogram => GaugeType::BarGauge,
                GaugePainter::AsymmetricSweepGauge => GaugeType::SweepGauge,
                GaugePainter::LineGraph => GaugeType::DigitalReadout, // Deferred
            };
            
            let config = GaugeConfig {
                id: if g.id.is_empty() { format!("gauge_{}", idx) } else { g.id.clone() },
                gauge_type,
                channel: g.output_channel.clone(),
                label: g.title.clone(),
                x: g.relative_x,
                y: g.relative_y,
                width: g.relative_width,
                height: g.relative_height,
                z_index: idx as u32,
                min_value: g.min,
                max_value: g.max,
                low_warning: g.low_warning,
                high_warning: g.high_warning,
                high_critical: g.high_critical,
                decimals: g.value_digits.max(0) as u32,
                units: g.units.clone(),
                font_color: g.font_color.to_css_hex(),
                needle_color: g.needle_color.to_css_hex(),
                trim_color: g.trim_color.to_css_hex(),
                show_history: g.show_history,
                show_min_max: false,
                on_condition: None,
                on_color: None,
                off_color: None,
                blink: None,
            };
            
            layout.gauges.push(config);
        }
    }
    
    layout
}

/// Parse a CSS hex color string to TsColor
fn parse_hex_color(hex: &str) -> TsColor {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        TsColor { alpha: 255, red: r, green: g, blue: b }
    } else {
        TsColor::default()
    }
}

// =============================================================================
// Signature Comparison Helpers
// =============================================================================

/// Compare two signatures and determine match type
fn compare_signatures(ecu_sig: &str, ini_sig: &str) -> SignatureMatchType {
    let ecu_normalized = ecu_sig.trim().to_lowercase();
    let ini_normalized = ini_sig.trim().to_lowercase();
    
    if ecu_normalized == ini_normalized {
        SignatureMatchType::Exact
    } else if ecu_normalized.contains(&ini_normalized) || ini_normalized.contains(&ecu_normalized) {
        // One contains the other - partial match (version differences)
        SignatureMatchType::Partial
    } else {
        // Check for common base signature (e.g., "speeduino 202305" vs "speeduino 202307")
        // Split on spaces and compare first word(s)
        let ecu_parts: Vec<&str> = ecu_normalized.split_whitespace().collect();
        let ini_parts: Vec<&str> = ini_normalized.split_whitespace().collect();
        
        if !ecu_parts.is_empty() && !ini_parts.is_empty() && ecu_parts[0] == ini_parts[0] {
            // Same base ECU type, different version
            SignatureMatchType::Partial
        } else {
            // Check for common firmware family keywords (e.g., "uaefi", "speeduino", etc.)
            // This helps recognize similar projects like "rusEFI ... uaefi ..." variants
            let common_keywords = ["uaefi", "speeduino", "rusefi", "epicefi", "megasquirt"];
            let ecu_has_keyword = common_keywords.iter().any(|kw| ecu_normalized.contains(kw));
            let ini_has_keyword = common_keywords.iter().any(|kw| ini_normalized.contains(kw));
            
            if ecu_has_keyword && ini_has_keyword {
                // Both have common firmware keywords - check if they share at least one
                let ecu_keywords: Vec<&str> = common_keywords.iter()
                    .filter(|kw| ecu_normalized.contains(**kw))
                    .copied()
                    .collect();
                let ini_keywords: Vec<&str> = common_keywords.iter()
                    .filter(|kw| ini_normalized.contains(**kw))
                    .copied()
                    .collect();
                
                // If they share a keyword, it's a partial match (same firmware family)
                if ecu_keywords.iter().any(|kw| ini_keywords.contains(kw)) {
                    SignatureMatchType::Partial
                } else {
                    SignatureMatchType::Mismatch
                }
            } else {
                SignatureMatchType::Mismatch
            }
        }
    }
}

/// Find INI files that match the given ECU signature
async fn find_matching_inis_internal(
    state: &tauri::State<'_, AppState>,
    ecu_signature: &str,
) -> Vec<MatchingIniInfo> {
    let mut matches = Vec::new();
    
    // Check INI repository if loaded
    let repo_guard = state.ini_repository.lock().await;
    if let Some(ref repo) = *repo_guard {
        for entry in repo.list() {
            let match_type = compare_signatures(ecu_signature, &entry.signature);
            if match_type != SignatureMatchType::Mismatch {
                matches.push(MatchingIniInfo {
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                    signature: entry.signature.clone(),
                    match_type,
                });
            }
        }
    }
    
    // Sort by match type (exact first, then partial)
    matches.sort_by(|a, b| {
        match (&a.match_type, &b.match_type) {
            (SignatureMatchType::Exact, SignatureMatchType::Partial) => std::cmp::Ordering::Less,
            (SignatureMatchType::Partial, SignatureMatchType::Exact) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });
    
    matches
}

#[tauri::command]
async fn get_serial_ports() -> Result<Vec<String>, String> {
    Ok(list_ports().into_iter().map(|p| p.name).collect())
}

#[tauri::command]
async fn get_available_inis(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let mut inis = Vec::new();
    let definitions_dir = get_definitions_dir(&app);
    println!("Scanning for INIs in: {:?}", definitions_dir);
    
    // Ensure definitions directory exists
    if !definitions_dir.exists() {
        let _ = std::fs::create_dir_all(&definitions_dir);
        println!("Created definitions directory: {:?}", definitions_dir);
        return Ok(inis); // Return empty list for new install
    }
    
    match std::fs::read_dir(&definitions_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext.to_string_lossy().to_lowercase() == "ini" {
                        if let Some(name) = entry.file_name().to_str() {
                            inis.push(name.to_string());
                        }
                    }
                }
            }
            println!("Found {} INI files", inis.len());
        },
        Err(e) => {
            println!("Failed to read definitions directory: {}", e);
            return Err(format!("Failed to read definitions directory: {}", e));
        }
    }
    inis.sort();
    Ok(inis)
}

#[tauri::command]
async fn load_ini(app: tauri::AppHandle, state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    // Resolve path: absolute paths stay as-is, relative paths are resolved from definitions dir
    let full_path = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        get_definitions_dir(&app).join(&path)
    };

    println!("Loading INI from: {:?}", full_path);
    match EcuDefinition::from_file(full_path.to_string_lossy().as_ref()) {
        Ok(def) => {
            println!("Successfully loaded INI: {} ({} tables, {} pages)", 
                def.signature, def.tables.len(), def.n_pages);
            
            // Get current tune before updating definition (if any)
            let current_tune = {
                let tune_guard = state.current_tune.lock().await;
                tune_guard.as_ref().cloned()
            };
            
            // Update definition
            let def_clone = def.clone();
            let mut guard = state.definition.lock().await;
            *guard = Some(def);
            drop(guard);
            
            // Initialize TuneCache from new definition
            let cache = TuneCache::from_definition(&def_clone);
            let mut cache_guard = state.tune_cache.lock().await;
            *cache_guard = Some(cache);
            
            // Re-apply current tune to new cache if we have one
            if let Some(tune) = current_tune {
                eprintln!("[DEBUG] load_ini: Re-applying tune data to new INI definition");
                use libretune_core::tune::TuneValue;
                
                let mut applied_count = 0;
                let mut skipped_count = 0;
                
                for (name, tune_value) in &tune.constants {
                    if let Some(constant) = def_clone.constants.get(name) {
                        // PC variables
                        if constant.is_pc_variable {
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    cache_guard.as_mut().unwrap().local_values.insert(name.clone(), *v);
                                    applied_count += 1;
                                }
                                TuneValue::Array(arr) if !arr.is_empty() => {
                                    cache_guard.as_mut().unwrap().local_values.insert(name.clone(), arr[0]);
                                    applied_count += 1;
                                }
                                _ => {
                                    skipped_count += 1;
                                }
                            }
                            continue;
                        }
                        
                        // Handle bits constants specially (they're packed, size_bytes() == 0)
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            let cache = cache_guard.as_mut().unwrap();
                            // Bits constants: read current byte(s), modify the bits, write back
                            let bit_pos = constant.bit_position.unwrap_or(0);
                            let bit_size = constant.bit_size.unwrap_or(1);
                            
                            // Calculate which byte(s) contain the bits
                            let byte_offset = (bit_pos / 8) as u16;
                            let bit_in_byte = bit_pos % 8;
                            
                            // Calculate how many bytes we need
                            let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
                            let bytes_needed = if bits_remaining_after_first_byte > 0 {
                                1 + ((bits_remaining_after_first_byte + 7) / 8)
                            } else {
                                1
                            };
                            let bytes_needed_usize = bytes_needed as usize;
                            
                            // Read current byte(s) value (or 0 if not present)
                            let read_offset = constant.offset + byte_offset;
                            let mut current_bytes: Vec<u8> = cache.read_bytes(constant.page, read_offset, bytes_needed as u16)
                                .map(|s| s.to_vec())
                                .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);
                            
                            // Ensure we have enough bytes
                            while current_bytes.len() < bytes_needed_usize {
                                current_bytes.push(0u8);
                            }
                            
                            // Get the bit value from MSQ (index into bit_options)
                            // MSQ can store bits constants as numeric indices, option strings, or booleans
                            let bit_value = match tune_value {
                                TuneValue::Scalar(v) => *v as u32,
                                TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                                TuneValue::Bool(b) => {
                                    // Boolean values: true = 1, false = 0
                                    // For bits constants with 2 options (like ["false", "true"]), 
                                    // boolean true maps to index 1, false to index 0
                                    if *b { 1 } else { 0 }
                                }
                                TuneValue::String(s) => {
                                    // Look up the string in bit_options to find its index
                                    if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                                        index as u32
                                    } else {
                                        // Try case-insensitive match
                                        if let Some(index) = constant.bit_options.iter().position(|opt| opt.eq_ignore_ascii_case(s)) {
                                            index as u32
                                        } else {
                                            skipped_count += 1;
                                            continue;
                                        }
                                    }
                                }
                                _ => {
                                    skipped_count += 1;
                                    continue;
                                }
                            };
                            
                            // Modify the first byte
                            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                            let mask_first = if bits_in_first_byte >= 8 {
                                0xFF
                            } else {
                                (1u8 << bits_in_first_byte) - 1
                            };
                            let value_first = (bit_value & mask_first as u32) as u8;
                            current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte)) | (value_first << bit_in_byte);
                            
                            // If bits span multiple bytes, modify additional bytes
                            if bits_remaining_after_first_byte > 0 {
                                let mut bits_collected = bits_in_first_byte;
                                for i in 1..bytes_needed_usize.min(current_bytes.len()) {
                                    let remaining_bits = bit_size - bits_collected;
                                    if remaining_bits == 0 {
                                        break;
                                    }
                                    let bits_from_this_byte = remaining_bits.min(8);
                                    let mask = if bits_from_this_byte >= 8 {
                                        0xFF
                                    } else {
                                        (1u8 << bits_from_this_byte) - 1
                                    };
                                    let value_from_bit = ((bit_value >> bits_collected) & mask as u32) as u8;
                                    current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                    bits_collected += bits_from_this_byte;
                                }
                            }
                            
                            // Write the modified byte(s) back
                            if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                                applied_count += 1;
                            } else {
                                skipped_count += 1;
                            }
                            continue;
                        }
                        
                        // Skip zero-size constants (shouldn't happen for non-bits)
                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            skipped_count += 1;
                            continue;
                        }
                        
                        // Convert and write to cache
                        let element_size = constant.data_type.size_bytes();
                        let element_count = constant.shape.element_count();
                        let mut raw_data = vec![0u8; length as usize];
                        
                        match tune_value {
                            TuneValue::Scalar(v) => {
                                let raw_val = constant.display_to_raw(*v);
                                constant.data_type.write_to_bytes(&mut raw_data, 0, raw_val, def_clone.endianness);
                                if cache_guard.as_mut().unwrap().write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                } else {
                                    skipped_count += 1;
                                }
                            }
                            TuneValue::Array(arr) => {
                                let write_count = arr.len().min(element_count);
                                let last_value = arr.last().copied().unwrap_or(0.0);
                                
                                for i in 0..element_count {
                                    let val = if i < arr.len() { arr[i] } else { last_value };
                                    let raw_val = constant.display_to_raw(val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(&mut raw_data, offset, raw_val, def_clone.endianness);
                                }
                                
                                if cache_guard.as_mut().unwrap().write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                } else {
                                    skipped_count += 1;
                                }
                            }
                            TuneValue::String(_) | TuneValue::Bool(_) => {
                                skipped_count += 1;
                            }
                        }
                    } else {
                        skipped_count += 1;
                    }
                }
                
                eprintln!("[DEBUG] load_ini: Re-applied tune constants - applied: {}, skipped: {}, total: {}", 
                    applied_count, skipped_count, tune.constants.len());
                
                // Emit event to notify UI that tune was re-applied
                let _ = app.emit("tune:loaded", "ini_changed");
            }
            drop(cache_guard);
            
            // Save as last INI
            let mut settings = load_settings(&app);
            settings.last_ini_path = Some(full_path.to_string_lossy().to_string());
            save_settings(&app, &settings);
            
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to parse INI {:?}: {}", full_path, e);
            eprintln!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn connect_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    port_name: String,
    baud_rate: u32,
    timeout_ms: Option<u64>,
) -> Result<ConnectResult, String> {
    let mut config = ConnectionConfig::default();
    config.port_name = port_name.clone();

    // Validate baud rate passed from UI: guard against 0.
    if baud_rate == 0 {
        eprintln!("[WARN] connect_to_ecu: received baud_rate 0, defaulting to {}", libretune_core::protocol::DEFAULT_BAUD_RATE);
        config.baud_rate = libretune_core::protocol::DEFAULT_BAUD_RATE;
    } else {
        config.baud_rate = baud_rate;
    }

    // Log resolved configuration for diagnostics
    eprintln!("[INFO] connect_to_ecu: port='{}' baud={} timeout_ms={}", config.port_name, config.baud_rate, config.timeout_ms);
    
    // Get protocol settings from loaded definition if available
    let def_guard = state.definition.lock().await;
    let protocol_settings = def_guard.as_ref().map(|d| d.protocol.clone());
    let endianness = def_guard.as_ref().map(|d| d.endianness).unwrap_or_default();
    let expected_signature = def_guard.as_ref().map(|d| d.signature.clone());
    drop(def_guard);

    // If a timeout was provided by the UI, apply it
    if let Some(t) = timeout_ms {
        eprintln!("[INFO] connect_to_ecu: using timeout_ms={} from UI", t);
        config.timeout_ms = t;
    }

    
    // Create connection in a dedicated OS thread (not Tokio's spawn_blocking)
    // Use catch_unwind to capture panics and send them back as errors.
    // Capture a small copy of the connection parameters for post-mortem logging
    let log_port = config.port_name.clone();
    let log_baud = config.baud_rate;
    let log_timeout = config.timeout_ms;

    let (tx, rx) = std::sync::mpsc::channel();
    
    std::thread::spawn(move || {
        let send_err = |s: String| {
            let _ = tx.send(Err(s));
        };

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut conn = if let Some(protocol) = protocol_settings {
                Connection::with_protocol(config, protocol, endianness)
            } else {
                Connection::new(config)
            };

            match conn.connect() {
                Ok(_) => Ok(conn),
                Err(e) => Err(e.to_string()),
            }
        }));

        match res {
            Ok(Ok(conn)) => {
                let _ = tx.send(Ok(conn));
            }
            Ok(Err(e)) => send_err(e),
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                send_err(format!("Connection thread panicked: {}", panic_msg));
            }
        }
    });
    
    // Wait for result with a longer timeout to account for USB latency
    let result = match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err("Connection timed out after 15 seconds".to_string())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err("Connection thread crashed or disconnected".to_string())
        }
    };

    match result {
        Ok(conn) => {
            let signature = conn.signature().unwrap_or("Unknown").to_string();
            
            // Check signature match and build mismatch info if needed
            let mismatch_info = if let Some(ref expected) = expected_signature {
                let match_type = compare_signatures(&signature, expected);
                
                if match_type != SignatureMatchType::Exact {
                    // Log the mismatch
                    eprintln!("Warning: ECU signature '{}' {} INI signature '{}'", 
                        signature, 
                        if match_type == SignatureMatchType::Partial { "partially matches" } else { "does not match" },
                        expected);
                    
                    // Find matching INIs from repository
                    let matching_inis = find_matching_inis_internal(&state, &signature).await;
                    
                    // Get current INI path from settings
                    let current_ini_path = {
                        let settings = load_settings(&app);
                        settings.last_ini_path.clone()
                    };
                    
                    let info = SignatureMismatchInfo {
                        ecu_signature: signature.clone(),
                        ini_signature: expected.clone(),
                        match_type,
                        current_ini_path,
                        matching_inis,
                    };
                    
                    // Also emit event for backward compatibility
                    let _ = app.emit("signature:mismatch", &info);
                    
                    Some(info)
                } else {
                    None
                }
            } else {
                None
            };
            
            let mut guard = state.connection.lock().await;
            *guard = Some(conn);
            
            Ok(ConnectResult {
                signature,
                mismatch_info,
            })
        }
        Err(e) => {
            eprintln!("[ERROR] connect_to_ecu failed: {} (port='{}' baud={} timeout_ms={})", e, log_port, log_baud, log_timeout);
            Err(e)
        },
    }
}

/// Sync response with progress information
#[derive(Serialize)]
struct SyncProgress {
    current_page: u8,
    total_pages: u8,
    bytes_read: usize,
    total_bytes: usize,
    complete: bool,
    /// Optional: page that just failed (for partial sync indication)
    failed_page: Option<u8>,
}

/// Read all ECU pages and store in TuneFile
/// Returns SyncResult indicating success/partial/failure
#[tauri::command]
async fn sync_ecu_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, String> {
    // Get definition to know page sizes
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    let signature = def.signature.clone();
    let n_pages = def.n_pages;
    let page_sizes: Vec<u32> = def.protocol.page_sizes.clone();
    let total_bytes: usize = page_sizes.iter().map(|&s| s as usize).sum();
    drop(def_guard);
    
    // Create new tune file
    let mut tune = TuneFile::new(&signature);
    let mut bytes_read: usize = 0;
    let mut pages_synced: u8 = 0;
    let mut pages_failed: u8 = 0;
    let mut errors: Vec<String> = Vec::new();
    
    for page in 0..n_pages {
        let page_size = page_sizes.get(page as usize).copied().unwrap_or(0);
        
        // Emit progress
        let progress = SyncProgress {
            current_page: page,
            total_pages: n_pages,
            bytes_read,
            total_bytes,
            complete: false,
            failed_page: None,
        };
        let _ = app.emit("sync:progress", &progress);
        
        if page_size == 0 {
            // Empty page, skip but count as success
            pages_synced += 1;
            continue;
        }
        
        // Read page data - wrapped in error handling for resilience
        let page_num = page;
        let mut conn_guard = state.connection.lock().await;
        let conn = match conn_guard.as_mut() {
            Some(c) => c,
            None => {
                errors.push(format!("Page {}: Not connected", page_num));
                pages_failed += 1;
                continue;
            }
        };
        
        // Try to read page - continue on failure
        match conn.read_page(page_num) {
            Ok(page_data) => {
                bytes_read += page_data.len();
                pages_synced += 1;
                
                // Store in TuneFile
                tune.pages.insert(page_num, page_data.clone());
                
                // Also populate TuneCache
                {
                    let mut cache_guard = state.tune_cache.lock().await;
                    if let Some(cache) = cache_guard.as_mut() {
                        cache.load_page(page_num, page_data);
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Page {}: {}", page_num, e);
                eprintln!("[WARN] sync_ecu_data: {}", error_msg);
                errors.push(error_msg);
                pages_failed += 1;
                
                // Emit progress with failed page indicator
                let progress = SyncProgress {
                    current_page: page,
                    total_pages: n_pages,
                    bytes_read,
                    total_bytes,
                    complete: false,
                    failed_page: Some(page_num),
                };
                let _ = app.emit("sync:progress", &progress);
            }
        }
        
        drop(conn_guard);
    }
    
    // Store tune file in state (even if partial)
    let mut tune_guard = state.current_tune.lock().await;
    let project_tune = tune_guard.clone(); // Keep copy for comparison
    let ecu_tune = tune.clone(); // Keep copy for comparison
    *tune_guard = Some(tune);
    
    // Mark as not modified (freshly synced from ECU)
    let mut modified_guard = state.tune_modified.lock().await;
    *modified_guard = false;
    drop(modified_guard);
    drop(tune_guard);
    
    // Emit complete
    let progress = SyncProgress {
        current_page: n_pages,
        total_pages: n_pages,
        bytes_read,
        total_bytes,
        complete: true,
        failed_page: None,
    };
    let _ = app.emit("sync:progress", &progress);
    
    // Check if project tune exists and differs from ECU tune
    if let Some(ref project) = project_tune {
        if project.signature == ecu_tune.signature {
            // Compare page data
            let mut has_differences = false;
            let mut diff_pages: Vec<u8> = Vec::new();
            
            // Check all pages that exist in either tune
            let all_pages: std::collections::HashSet<u8> = project.pages.keys()
                .chain(ecu_tune.pages.keys())
                .copied()
                .collect();
            
            for page_num in all_pages {
                let project_page = project.pages.get(&page_num);
                let ecu_page = ecu_tune.pages.get(&page_num);
                
                match (project_page, ecu_page) {
                    (Some(p), Some(e)) if p != e => {
                        has_differences = true;
                        diff_pages.push(page_num);
                    }
                    (Some(_), None) | (None, Some(_)) => {
                        has_differences = true;
                        diff_pages.push(page_num);
                    }
                    _ => {}
                }
            }
            
            if has_differences {
                // Emit event for frontend to show dialog
                let ecu_page_nums: Vec<u8> = ecu_tune.pages.keys().copied().collect();
                let project_page_nums: Vec<u8> = project.pages.keys().copied().collect();
                let _ = app.emit("tune:mismatch", &serde_json::json!({
                    "ecu_pages": ecu_page_nums,
                    "project_pages": project_page_nums,
                    "diff_pages": diff_pages,
                }));
            }
        }
    }
    
    // Log detailed errors for debugging
    if !errors.is_empty() {
        eprintln!("[WARN] sync_ecu_data completed with {} errors:", errors.len());
        for err in &errors {
            eprintln!("  - {}", err);
        }
    }
    
    Ok(SyncResult {
        success: pages_failed == 0,
        pages_synced,
        pages_failed,
        total_pages: n_pages,
        errors,
    })
}

#[tauri::command]
async fn disconnect_ecu(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.connection.lock().await;
    *guard = None;
    Ok(())
}

/// Response for adaptive timing stats
#[derive(Serialize)]
struct AdaptiveTimingStats {
    enabled: bool,
    avg_response_ms: Option<f64>,
    sample_count: usize,
    current_timeout_ms: Option<u64>,
}

/// Enable adaptive timing (experimental feature that dynamically adjusts communication speed)
#[tauri::command]
async fn enable_adaptive_timing(
    state: tauri::State<'_, AppState>,
    multiplier: Option<f32>,
    min_timeout_ms: Option<u32>,
    max_timeout_ms: Option<u32>,
) -> Result<AdaptiveTimingStats, String> {
    let mut guard = state.connection.lock().await;
    let conn = guard.as_mut().ok_or("Not connected to ECU")?;
    
    let config = AdaptiveTimingConfig {
        enabled: true,
        multiplier: multiplier.unwrap_or(2.5),
        min_timeout_ms: min_timeout_ms.unwrap_or(10),
        max_timeout_ms: max_timeout_ms.unwrap_or(500),
        sample_count: 20,
    };
    
    conn.enable_adaptive_timing(Some(config));
    
    Ok(AdaptiveTimingStats {
        enabled: true,
        avg_response_ms: None,
        sample_count: 0,
        current_timeout_ms: None,
    })
}

/// Disable adaptive timing (return to INI-specified timing)
#[tauri::command]
async fn disable_adaptive_timing(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = state.connection.lock().await;
    let conn = guard.as_mut().ok_or("Not connected to ECU")?;
    
    conn.disable_adaptive_timing();
    Ok(())
}

/// Get adaptive timing statistics
#[tauri::command]
async fn get_adaptive_timing_stats(
    state: tauri::State<'_, AppState>,
) -> Result<AdaptiveTimingStats, String> {
    let guard = state.connection.lock().await;
    let conn = guard.as_ref().ok_or("Not connected to ECU")?;
    
    let enabled = conn.is_adaptive_timing_enabled();
    let stats = conn.adaptive_timing_stats();
    
    Ok(AdaptiveTimingStats {
        enabled,
        avg_response_ms: stats.as_ref().map(|(avg, _)| avg.as_micros() as f64 / 1000.0),
        sample_count: stats.as_ref().map(|(_, count)| *count).unwrap_or(0),
        current_timeout_ms: None, // Could add this if needed
    })
}

#[tauri::command]
async fn get_connection_status(state: tauri::State<'_, AppState>) -> Result<ConnectionStatus, String> {
    let conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let demo_mode = *state.demo_mode.lock().await;

    let (state_val, signature) = if demo_mode {
        (ConnectionState::Connected, Some("DEMO - Simulated EpicEFI".to_string()))
    } else {
        match &*conn_guard {
            Some(conn) => (conn.state(), conn.signature().map(|s| s.to_string())),
            None => (ConnectionState::Disconnected, None),
        }
    };

    Ok(ConnectionStatus {
        state: state_val,
        signature,
        has_definition: def_guard.is_some(),
        ini_name: def_guard.as_ref().map(|d| d.signature.clone()),
        demo_mode,
    })
}

#[tauri::command]
async fn auto_load_last_ini(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let settings = load_settings(&app);
    if let Some(path) = settings.last_ini_path {
        if Path::new(&path).exists() {
             return Ok(Some(path));
        }
    }
    Ok(None)
}

#[derive(Serialize)]
struct TableData {
    name: String,
    title: String,
    x_bins: Vec<f64>,
    y_bins: Vec<f64>,
    z_values: Vec<Vec<f64>>,
    x_axis_name: String,
    y_axis_name: String,
    /// Output channel name for X-axis (used for live cell highlighting)
    x_output_channel: Option<String>,
    /// Output channel name for Y-axis (used for live cell highlighting)
    y_output_channel: Option<String>,
}

#[derive(Serialize)]
struct CurveData {
    name: String,
    title: String,
    x_bins: Vec<f64>,
    y_bins: Vec<f64>,
    x_label: String,
    y_label: String,
}

/// Clean up INI expression labels for display
/// Converts expressions like `{bitStringValue(pwmAxisLabels, gppwm1_loadAxis)}` 
/// to a readable fallback like `gppwm1_loadAxis`
fn clean_axis_label(label: &str) -> String {
    let trimmed = label.trim();
    
    // If it's an expression (starts with {), try to extract meaningful part
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        // Extract content inside braces
        let inner = &trimmed[1..trimmed.len()-1];
        
        // Check for bitStringValue(list, index) pattern
        if inner.starts_with("bitStringValue(") {
            // Extract the second parameter (the index variable name)
            if let Some(comma_pos) = inner.find(',') {
                let second_part = inner[comma_pos+1..].trim();
                // Remove trailing ) if present
                let name = second_part.trim_end_matches(')').trim();
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
        
        // Fallback: just return the inner content without braces
        return inner.to_string();
    }
    
    // Not an expression, return as-is
    trimmed.to_string()
}

#[tauri::command]
async fn get_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String
) -> Result<TableData, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let endianness = def.endianness;

    let table = def.get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;
    
    // Clone the table info we need
    let x_bins_name = table.x_bins.clone();
    let y_bins_name = table.y_bins.clone();
    let map_name = table.map.clone();
    let is_3d = table.is_3d();
    let table_name_out = table.name.clone();
    let table_title = table.title.clone();
    let x_label = table.x_label.clone().unwrap_or_else(|| table.x_bins.clone());
    let y_label = table.y_label.clone().unwrap_or_else(|| table.y_bins.clone().unwrap_or_default());
    let x_output_channel = table.x_output_channel.clone();
    let y_output_channel = table.y_output_channel.clone();
    
    // Collect constant info we need
    let x_const = def.constants.get(&x_bins_name)
        .ok_or_else(|| format!("Constant {} not found", x_bins_name))?.clone();
    let y_const = y_bins_name.as_ref()
        .and_then(|name| def.constants.get(name).cloned());
    let z_const = def.constants.get(&map_name)
        .ok_or_else(|| format!("Constant {} not found", map_name))?.clone();
    
    drop(def_guard);

    // Helper to read constant data from TuneFile (offline) or ECU (online)
    fn read_const_from_source(
        constant: &Constant,
        tune: Option<&TuneFile>,
        cache: Option<&TuneCache>,
        conn: &mut Option<&mut Connection>,
        endianness: libretune_core::ini::Endianness,
    ) -> Result<Vec<f64>, String> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        if length == 0 {
            return Ok(vec![0.0; element_count]);
        }
        
        // If offline, always read from TuneFile (MSQ file) - no cache fallback
        if conn.is_none() {
            if let Some(tune_file) = tune {
                if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                    use libretune_core::tune::TuneValue;
                    match tune_value {
                        TuneValue::Array(arr) => {
                            eprintln!("[DEBUG] read_const_from_source: CACHE HIT for '{}' (page={}, offset={}, len={}, offline mode)", 
                                constant.name, constant.page, constant.offset, length);
                            return Ok(arr.clone());
                        }
                        TuneValue::Scalar(v) => {
                            eprintln!("[DEBUG] read_const_from_source: Found '{}' in TuneFile as Scalar({}), returning as single-element array", 
                                constant.name, v);
                            return Ok(vec![*v]);
                        }
                        _ => {
                            eprintln!("[DEBUG] read_const_from_source: Found '{}' in TuneFile but wrong type, falling through", constant.name);
                        }
                    }
                } else {
                    eprintln!("[DEBUG] read_const_from_source: Constant '{}' not found in TuneFile, returning zeros", constant.name);
                    return Ok(vec![0.0; element_count]);
                }
            } else {
                eprintln!("[DEBUG] read_const_from_source: No TuneFile loaded, returning zeros");
                return Ok(vec![0.0; element_count]);
            }
        }
        
        // If connected to ECU, always read from ECU (live data)
        if let Some(ref mut conn_ptr) = conn {
            eprintln!("[DEBUG] read_const_from_source: reading '{}' from ECU (online mode)", constant.name);
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                length,
            };

            let raw_data = conn_ptr.read_memory(params).map_err(|e| e.to_string())?;
            
            let mut values = Vec::new();
            for i in 0..element_count {
                let offset = i * element_size;
                if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, offset, endianness) {
                    values.push(constant.raw_to_display(raw_val));
                } else {
                    values.push(0.0);
                }
            }
            return Ok(values);
        }
        
        // If offline and not in TuneFile, return zeros (should always be in TuneFile)
        eprintln!("[DEBUG] read_const_from_source: Constant '{}' not found in TuneFile, returning zeros", constant.name);
        Ok(vec![0.0; element_count])
    }

    // Get tune, cache and connection
    let tune_guard = state.current_tune.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let mut conn_guard = state.connection.lock().await;
    let mut conn = conn_guard.as_mut();

    let x_bins = read_const_from_source(&x_const, tune_guard.as_ref(), cache_guard.as_ref(), &mut conn, endianness)?;
    let y_bins = if let Some(ref y) = y_const {
        read_const_from_source(y, tune_guard.as_ref(), cache_guard.as_ref(), &mut conn, endianness)?
    } else {
        vec![0.0]
    };
    let z_flat = read_const_from_source(&z_const, tune_guard.as_ref(), cache_guard.as_ref(), &mut conn, endianness)?;
    
    drop(cache_guard);
    drop(conn_guard);
    
    // Reshape Z values into 2D array [y][x]
    let x_size = x_bins.len();
    let y_size = if is_3d { y_bins.len() } else { 1 };
    
    let mut z_values = Vec::with_capacity(y_size);
    for y in 0..y_size {
        let mut row = Vec::with_capacity(x_size);
        for x in 0..x_size {
            let idx = y * x_size + x;
            row.push(*z_flat.get(idx).unwrap_or(&0.0));
        }
        z_values.push(row);
    }

    Ok(TableData {
        name: table_name_out,
        title: table_title,
        x_bins,
        y_bins,
        z_values,
        x_axis_name: clean_axis_label(&x_label),
        y_axis_name: clean_axis_label(&y_label),
        x_output_channel,
        y_output_channel,
    })
}

/// Lightweight command to check if a table exists in the definition
/// This is used by the frontend to determine if a panel should render as a table button
#[tauri::command]
async fn get_table_info(
    state: tauri::State<'_, AppState>,
    table_name: String
) -> Result<TableInfo, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    if let Some(table) = def.get_table_by_name_or_map(&table_name) {
        Ok(TableInfo {
            name: table.name.clone(),
            title: table.title.clone(),
        })
    } else {
        Err(format!("Table {} not found", table_name))
    }
}

#[derive(Serialize)]
struct ProtocolDefaults {
    default_baud_rate: u32,
    inter_write_delay: u32,
    delay_after_port_open: u32,
    message_envelope_format: Option<String>,
    page_activation_delay: u32,
    // Suggested read timeout for UI (ms)
    timeout_ms: u32,
}

#[tauri::command]
async fn get_protocol_defaults(state: tauri::State<'_, AppState>) -> Result<ProtocolDefaults, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let proto = def.protocol.clone();
    Ok(ProtocolDefaults {
        default_baud_rate: proto.default_baud_rate,
        inter_write_delay: proto.inter_write_delay,
        delay_after_port_open: proto.delay_after_port_open,
        message_envelope_format: proto.message_envelope_format.clone(),
        page_activation_delay: proto.page_activation_delay,
        timeout_ms: proto.block_read_timeout,
    })
}

/// Status of the tune cache for UI display
#[derive(Serialize)]
struct TuneCacheStatus {
    /// Total number of pages
    total_pages: u8,
    /// Number of pages loaded
    loaded_pages: u8,
    /// Whether all pages are loaded
    fully_loaded: bool,
    /// Whether currently loading
    is_loading: bool,
    /// Whether there are unsaved changes
    has_dirty_data: bool,
    /// Whether there are pending burns
    has_pending_burn: bool,
    /// Count of dirty bytes
    dirty_byte_count: usize,
    /// Pages with dirty data
    dirty_pages: Vec<u8>,
}

#[tauri::command]
async fn get_tune_cache_status(state: tauri::State<'_, AppState>) -> Result<TuneCacheStatus, String> {
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref().ok_or("TuneCache not initialized")?;
    
    let total_pages = cache.page_count();
    let mut loaded_pages = 0u8;
    for page in 0..total_pages {
        match cache.page_state(page) {
            PageState::Clean | PageState::Dirty | PageState::Pending => loaded_pages += 1,
            _ => {}
        }
    }
    
    Ok(TuneCacheStatus {
        total_pages,
        loaded_pages,
        fully_loaded: cache.is_fully_loaded(),
        is_loading: cache.is_loading(),
        has_dirty_data: cache.has_dirty_data(),
        has_pending_burn: cache.has_pending_burn(),
        dirty_byte_count: cache.dirty_byte_count(),
        dirty_pages: cache.dirty_pages(),
    })
}

/// Load all ECU pages into the cache (background operation)
#[tauri::command]
async fn load_all_pages(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Get pages to load and their sizes
    let pages_to_load: Vec<(u8, u16)>;
    {
        let cache_guard = state.tune_cache.lock().await;
        let def_guard = state.definition.lock().await;
        
        let cache = cache_guard.as_ref().ok_or("TuneCache not initialized")?;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        
        pages_to_load = cache.pages_to_load()
            .into_iter()
            .filter_map(|p| {
                def.page_sizes.get(p as usize).map(|size| (p, *size))
            })
            .collect();
    }
    
    if pages_to_load.is_empty() {
        return Ok(());
    }
    
    // Mark pages as loading
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page, _) in &pages_to_load {
                cache.mark_loading(*page);
            }
        }
    }
    
    // Emit loading started event
    let _ = app.emit("cache:loading", serde_json::json!({
        "pages": pages_to_load.len(),
        "status": "started"
    }));
    
    // Load pages one at a time to avoid blocking
    for (page, size) in pages_to_load {
        // Read page from ECU
        let page_data: Result<Vec<u8>, String> = {
            let mut conn_guard = state.connection.lock().await;
            if let Some(conn) = conn_guard.as_mut() {
                let params = libretune_core::protocol::commands::ReadMemoryParams {
                    can_id: 0,
                    page,
                    offset: 0,
                    length: size,
                };
                conn.read_memory(params).map_err(|e| e.to_string())
            } else {
                Err("Not connected".to_string())
            }
        };
        
        // Update cache with result
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                match page_data {
                    Ok(data) => {
                        cache.load_page(page, data);
                        let _ = app.emit("cache:page_loaded", serde_json::json!({
                            "page": page,
                            "success": true
                        }));
                    }
                    Err(e) => {
                        cache.mark_error(page);
                        let _ = app.emit("cache:page_loaded", serde_json::json!({
                            "page": page,
                            "success": false,
                            "error": e
                        }));
                    }
                }
            }
        }
        
        // Small delay between pages to avoid overwhelming the ECU
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Emit loading complete event
    let _ = app.emit("cache:loading", serde_json::json!({
        "status": "complete"
    }));
    
    Ok(())
} 

#[tauri::command]
async fn get_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String
) -> Result<CurveData, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let mut conn = conn_guard.as_mut();

    let curve = def.curves.get(&curve_name)
        .ok_or_else(|| format!("Curve {} not found", curve_name))?;

    // Helper function to read a constant's values
    let mut read_const_values = |const_name: &str| -> Result<Vec<f64>, String> {
        let constant = def.constants.get(const_name)
            .ok_or_else(|| format!("Constant {} not found", const_name))?;
        
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        if length == 0 || conn.is_none() {
            return Ok(vec![0.0; element_count]);
        }
        
        let conn_ptr = conn.as_mut().unwrap();

        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn_ptr.read_memory(params).map_err(|e| e.to_string())?;
        
        let mut values = Vec::new();
        for i in 0..element_count {
            let offset = i * element_size;
            if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, offset, def.endianness) {
                values.push(constant.raw_to_display(raw_val));
            } else {
                values.push(0.0);
            }
        }
        
        Ok(values)
    };

    let x_bins = read_const_values(&curve.x_bins)?;
    let y_bins = read_const_values(&curve.y_bins)?;

    Ok(CurveData {
        name: curve.name.clone(),
        title: curve.title.clone(),
        x_bins,
        y_bins,
        x_label: curve.column_labels.0.clone(),
        y_label: curve.column_labels.1.clone(),
    })
}

#[tauri::command]
async fn update_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
    z_values: Vec<Vec<f64>>
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let table = def.get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def.constants.get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    // Flatten z_values
    let flat_values: Vec<f64> = z_values.into_iter().flatten().collect();
    
    if flat_values.len() != constant.shape.element_count() {
        return Err(format!("Invalid data size: expected {}, got {}", constant.shape.element_count(), flat_values.len()));
    }

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes() as usize];

    for (i, val) in flat_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant.data_type.write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    // Always write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Get or create page data
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    // Create empty page if it doesn't exist
                    vec![0u8; def.page_sizes.get(constant.page as usize).copied().unwrap_or(256) as usize]
                });
                
                // Update the page data
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
            }
            
            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected (optional - offline mode works without this)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };
        
        // Don't fail if ECU write fails - offline mode should still work
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
async fn get_realtime_data(state: tauri::State<'_, AppState>) -> Result<HashMap<String, f64>, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;

    let (conn, def) = match (&mut *conn_guard, &*def_guard) {
        (Some(c), Some(d)) => (c, d),
        _ => return Err("Connection or definition missing".to_string()),
    };

    // Get raw data from ECU
    let raw_data = conn.get_realtime_data().map_err(|e| e.to_string())?;

    // Parse data using output channels
    let mut data = HashMap::new();
    for (name, channel) in &def.output_channels {
        if let Some(val) = channel.parse(&raw_data, def.endianness) {
            data.insert(name.clone(), val);
        }
    }

    Ok(data)
}

#[tauri::command]
async fn start_realtime_stream(app: tauri::AppHandle, state: tauri::State<'_, AppState>, interval_ms: Option<u64>) -> Result<(), String> {
    let interval = interval_ms.unwrap_or(100);
    let is_demo = *state.demo_mode.lock().await;

    // In demo mode, we only need the definition
    // In real mode, we need both connection and definition
    if !is_demo {
        let conn_guard = state.connection.lock().await;
        let def_guard = state.definition.lock().await;
        if conn_guard.is_none() || def_guard.is_none() {
            return Err("Connection or definition missing".to_string());
        }
    } else {
        let def_guard = state.definition.lock().await;
        if def_guard.is_none() {
            return Err("Definition not loaded for demo mode".to_string());
        }
    }

    // Check if already running
    let mut task_guard = state.streaming_task.lock().await;
    if task_guard.is_some() {
        return Ok(());
    }

    let app_handle = app.clone();

    let handle = tokio::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(interval));
        
        // For demo mode, create a simulator
        let mut demo_simulator: Option<DemoSimulator> = None;
        let start_time = std::time::Instant::now();
        
        loop {
            ticker.tick().await;

            let is_demo = *app_state.demo_mode.lock().await;
            
            if is_demo {
                // Demo mode: generate simulated data
                if demo_simulator.is_none() {
                    demo_simulator = Some(DemoSimulator::new());
                }
                
                if let Some(ref mut sim) = demo_simulator {
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let data = sim.update(elapsed_ms);
                    let _ = app_handle.emit("realtime:update", &data);
                }
            } else {
                // Real ECU mode: read from connection
                demo_simulator = None; // Clear simulator if we switch modes
                
                let mut conn_guard = app_state.connection.lock().await;
                let def_guard = app_state.definition.lock().await;

                if let (Some(conn), Some(def)) = (conn_guard.as_mut(), def_guard.as_ref()) {
                    match conn.get_realtime_data() {
                        Ok(raw) => {
                            let mut data: HashMap<String, f64> = HashMap::new();
                            for (name, channel) in &def.output_channels {
                                if let Some(val) = channel.parse(&raw, def.endianness) {
                                    data.insert(name.clone(), val);
                                }
                            }
                            let _ = app_handle.emit("realtime:update", &data);
                        }
                        Err(e) => {
                            let _ = app_handle.emit("realtime:error", &e.to_string());
                        }
                    }
                }
            }
        }
    });

    *task_guard = Some(handle);
    Ok(())
}

#[tauri::command]
async fn stop_realtime_stream(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(handle) = task_guard.take() {
        handle.abort();
    }
    Ok(())
}

#[derive(Serialize)]
struct TableInfo {
    name: String,
    title: String,
}

#[tauri::command]
async fn get_tables(state: tauri::State<'_, AppState>) -> Result<Vec<TableInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    let mut tables: Vec<TableInfo> = def.tables.values()
        .map(|t| TableInfo { name: t.name.clone(), title: t.title.clone() })
        .collect();
    tables.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(tables)
}

/// Gauge configuration info returned to frontend
#[derive(Serialize)]
struct GaugeInfo {
    name: String,
    channel: String,
    title: String,
    units: String,
    lo: f64,
    hi: f64,
    low_warning: f64,
    high_warning: f64,
    low_danger: f64,
    high_danger: f64,
    digits: u8,
}

/// FrontPage indicator info returned to frontend
#[derive(Serialize)]
struct FrontPageIndicatorInfo {
    expression: String,
    label_off: String,
    label_on: String,
    bg_off: String,
    fg_off: String,
    bg_on: String,
    fg_on: String,
}

/// FrontPage configuration info returned to frontend
#[derive(Serialize)]
struct FrontPageInfo {
    /// Gauge names for gauge1-gauge8 (references to [GaugeConfigurations])
    gauges: Vec<String>,
    /// Status indicators
    indicators: Vec<FrontPageIndicatorInfo>,
}

#[tauri::command]
async fn get_frontpage(state: tauri::State<'_, AppState>) -> Result<Option<FrontPageInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    Ok(def.frontpage.as_ref().map(|fp| FrontPageInfo {
        gauges: fp.gauges.clone(),
        indicators: fp.indicators.iter().map(|ind| FrontPageIndicatorInfo {
            expression: ind.expression.clone(),
            label_off: ind.label_off.clone(),
            label_on: ind.label_on.clone(),
            bg_off: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.bg_off),
            fg_off: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.fg_off),
            bg_on: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.bg_on),
            fg_on: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.fg_on),
        }).collect(),
    }))
}

#[tauri::command]
async fn get_gauge_configs(state: tauri::State<'_, AppState>) -> Result<Vec<GaugeInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    let gauges: Vec<GaugeInfo> = def.gauges.values()
        .map(|g| GaugeInfo {
            name: g.name.clone(),
            channel: g.channel.clone(),
            title: g.title.clone(),
            units: g.units.clone(),
            lo: g.lo,
            hi: g.hi,
            low_warning: g.low_warning,
            high_warning: g.high_warning,
            low_danger: g.low_danger,
            high_danger: g.high_danger,
            digits: g.digits,
        })
        .collect();
    Ok(gauges)
}

/// Output channel info returned to frontend
#[derive(Serialize, Clone)]
struct ChannelInfo {
    /// Channel name/identifier
    name: String,
    /// Human-readable label (if available)
    label: Option<String>,
    /// Unit of measurement
    units: String,
    /// Scale factor for display
    scale: f64,
    /// Translate offset for display  
    translate: f64,
}

/// Get all available output channels from the INI definition
#[tauri::command]
async fn get_available_channels(state: tauri::State<'_, AppState>) -> Result<Vec<ChannelInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    let mut channels: Vec<ChannelInfo> = def.output_channels.values()
        .map(|ch| ChannelInfo {
            name: ch.name.clone(),
            label: ch.label.clone(),
            units: ch.units.clone(),
            scale: ch.scale,
            translate: ch.translate,
        })
        .collect();
    
    // Sort by name for consistent ordering
    channels.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(channels)
}

/// Get suggested status bar channels based on FrontPage or common defaults
#[tauri::command]
async fn get_status_bar_defaults(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    // Try to get channels from FrontPage gauges first
    if let Some(fp) = &def.frontpage {
        if !fp.gauges.is_empty() {
            // Get the channel names for the first few gauges
            let mut channels = Vec::new();
            for gauge_name in fp.gauges.iter().take(4) {
                if let Some(gauge) = def.gauges.get(gauge_name) {
                    channels.push(gauge.channel.clone());
                }
            }
            if !channels.is_empty() {
                return Ok(channels);
            }
        }
    }
    
    // Fall back to common channel names if they exist
    let common_channels = ["RPM", "rpm", "AFR", "afr", "lambda", "MAP", "map", "TPS", "tps", "coolant", "CLT", "IAT"];
    let mut defaults = Vec::new();
    for name in common_channels.iter() {
        if def.output_channels.contains_key(*name) && !defaults.contains(&name.to_string()) {
            defaults.push(name.to_string());
            if defaults.len() >= 4 {
                break;
            }
        }
    }
    
    // If still empty, just take first 4 channels
    if defaults.is_empty() {
        defaults = def.output_channels.keys()
            .take(4)
            .cloned()
            .collect();
    }
    
    Ok(defaults)
}

#[tauri::command]
async fn get_menu_tree(
    state: tauri::State<'_, AppState>,
    filter_context: Option<HashMap<String, f64>>
) -> Result<Vec<Menu>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    if let Some(context) = filter_context {
        let mut filtered_menus = Vec::new();
        for menu in &def.menus {
            let filtered_items = filter_menu_items(&menu.items, &context);
            if !filtered_items.is_empty() {
                filtered_menus.push(Menu {
                    name: menu.name.clone(),
                    title: menu.title.clone(),
                    items: filtered_items,
                });
            }
        }
        Ok(filtered_menus)
    } else {
        Ok(def.menus.clone())
    }
}

fn filter_menu_items(items: &[MenuItem], context: &HashMap<String, f64>) -> Vec<MenuItem> {
    let mut filtered = Vec::new();
    for item in items {
        if should_show_item(item, context) {
            // If it's a SubMenu, recursively filter its children
            let filtered_item = match item {
                MenuItem::SubMenu { label, items: sub_items, visibility_condition, enabled_condition } => {
                    let filtered_children = filter_menu_items(sub_items, context);
                    if !filtered_children.is_empty() {
                        MenuItem::SubMenu {
                            label: label.clone(),
                            items: filtered_children,
                            visibility_condition: visibility_condition.clone(),
                            enabled_condition: enabled_condition.clone(),
                        }
                    } else {
                        continue; // Skip submenu with no visible children
                    }
                }
                _ => item.clone(),
            };
            filtered.push(filtered_item);
        }
    }
    filtered
}

fn should_show_item(item: &MenuItem, context: &HashMap<String, f64>) -> bool {
    match item {
        MenuItem::Dialog { visibility_condition, enabled_condition, .. } 
        | MenuItem::Table { visibility_condition, enabled_condition, .. } 
        | MenuItem::SubMenu { visibility_condition, enabled_condition, .. }
        | MenuItem::Std { visibility_condition, enabled_condition, .. }
        | MenuItem::Help { visibility_condition, enabled_condition, .. } => {
            // Evaluate visibility condition first (if present)
            if let Some(vis_cond) = visibility_condition {
                let mut parser = libretune_core::ini::expression::Parser::new(vis_cond);
                if let Ok(expr) = parser.parse() {
                    if let Ok(val) = libretune_core::ini::expression::evaluate_simple(&expr, context) {
                        if !val.as_bool() {
                            return false; // Not visible
                        }
                    } else {
                        return true; // Show on error
                    }
                } else {
                    return true; // Show on parse error
                }
            }
            
            // If no visibility condition or visibility is true, check enabled condition
            // For now, we use enabled_condition as a fallback visibility check
            // (items that are disabled but visible can be shown grayed out later)
            if let Some(en_cond) = enabled_condition {
                let mut parser = libretune_core::ini::expression::Parser::new(en_cond);
                if let Ok(expr) = parser.parse() {
                    if let Ok(val) = libretune_core::ini::expression::evaluate_simple(&expr, context) {
                        return val.as_bool();
                    }
                }
                return true; // Show on error
            }
            
            true // No conditions, show by default
        }
        MenuItem::Separator => true,
    }
}

#[tauri::command]
async fn evaluate_expression(
    state: tauri::State<'_, AppState>,
    expression: String,
    context: HashMap<String, f64>
) -> Result<bool, String> {
    let mut parser = libretune_core::ini::expression::Parser::new(&expression);
    let expr = parser.parse().map_err(|e| e)?;
    let val = libretune_core::ini::expression::evaluate_simple(&expr, &context).map_err(|e| e)?;
    Ok(val.as_bool())
}

#[tauri::command]
async fn get_dialog_definition(
    state: tauri::State<'_, AppState>,
    name: String
) -> Result<DialogDefinition, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.dialogs.get(&name)
        .cloned()
        .ok_or_else(|| format!("Dialog {} not found", name))
}

#[tauri::command]
async fn get_help_topic(
    state: tauri::State<'_, AppState>,
    name: String
) -> Result<HelpTopic, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.help_topics.get(&name)
        .cloned()
        .ok_or_else(|| format!("Help topic {} not found", name))
}

#[tauri::command]
async fn get_constant(state: tauri::State<'_, AppState>, name: String) -> Result<ConstantInfo, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let constant = def.constants.get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;
    
    // Determine value_type from DataType
    let value_type = match constant.data_type {
        DataType::String => "string".to_string(),
        DataType::Bits => "bits".to_string(),
        _ => {
            // Check if it's an array
            match &constant.shape {
                libretune_core::ini::Shape::Scalar => "scalar".to_string(),
                _ => "array".to_string(),
            }
        }
    };
    
    eprintln!("[DEBUG] get_constant '{}': bit_options.len()={}, value_type={}", 
        name, constant.bit_options.len(), value_type);
    if constant.bit_options.len() > 0 && constant.bit_options.len() <= 10 {
        eprintln!("[DEBUG] get_constant '{}': bit_options={:?}", name, constant.bit_options);
    }
    
    Ok(ConstantInfo {
        name: constant.name.clone(),
        label: constant.label.clone(),
        units: constant.units.clone(),
        digits: constant.digits,
        min: constant.min,
        max: constant.max,
        value_type,
        bit_options: constant.bit_options.clone(),
        help: constant.help.clone(),
        visibility_condition: constant.visibility_condition.clone(),
    })
}

#[tauri::command]
async fn get_constant_string_value(state: tauri::State<'_, AppState>, name: String) -> Result<String, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut();

    let constant = def.constants.get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;
    
    // For string type, read the raw bytes and convert to UTF-8 string
    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }
    
    // When offline, try reading directly from TuneFile first (simpler and more reliable)
    if conn.is_none() {
        if let Some(tune) = tune_guard.as_ref() {
            if let Some(tune_value) = tune.constants.get(&name) {
                use libretune_core::tune::TuneValue;
                if let TuneValue::String(s) = tune_value {
                    return Ok(s.clone());
                }
            }
        }
    }
    
    // Get string length from shape (e.g., Array1D(32) means 32 chars)
    let length = constant.shape.element_count() as u16;
    if length == 0 {
        return Ok(String::new());
    }
    
    // If connected to ECU, always read from ECU (live data)
    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        // Convert to string, stopping at first null byte
        let s = String::from_utf8_lossy(&raw_data);
        let s = s.trim_end_matches('\0').to_string();
        return Ok(s);
    }
    
    // If offline and not in TuneFile, return empty string (should always be in TuneFile)
    Ok(String::new())
}

#[tauri::command]
async fn get_constant_value(state: tauri::State<'_, AppState>, name: String) -> Result<f64, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut();

    let constant = def.constants.get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;
    
    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        // Check local cache first
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(&val) = cache.local_values.get(&name) {
                return Ok(val);
            }
        }
        // Fall back to default value from INI
        if let Some(&default_val) = def.default_values.get(&name) {
            return Ok(default_val);
        }
        // Last resort: use min value or 0
        return Ok(constant.min);
    }
    
    // When offline, ALWAYS read from TuneFile (MSQ file) - no cache fallback
    if conn.is_none() {
        if let Some(tune) = tune_guard.as_ref() {
            if let Some(tune_value) = tune.constants.get(&name) {
                use libretune_core::tune::TuneValue;
                match tune_value {
                    TuneValue::Scalar(v) => {
                        // For bits constants, the value might be a string - need to look it up
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            // If it's already a number, return it (even if it maps to "INVALID" - that's what's in the MSQ)
                            let index = *v as usize;
                            if index < constant.bit_options.len() {
                                let option_str = &constant.bit_options[index];
                                eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), returning as bits index (maps to '{}')", 
                                    name, v, option_str);
                            } else {
                                eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), but out of range (bit_options len={}), returning anyway", 
                                    name, v, constant.bit_options.len());
                            }
                            return Ok(*v);
                        } else {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), returning directly", name, v);
                            return Ok(*v);
                        }
                    }
                    TuneValue::String(s) if constant.data_type == libretune_core::ini::DataType::Bits => {
                        // Look up string in bit_options
                        if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), matched at index {}", name, s, index);
                            return Ok(index as f64);
                        }
                        // Try case-insensitive
                        if let Some(index) = constant.bit_options.iter().position(|opt| opt.eq_ignore_ascii_case(s)) {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), case-insensitive match at index {}", name, s, index);
                            return Ok(index as f64);
                        }
                        eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), but not found in bit_options, returning 0", 
                            name, s);
                        return Ok(0.0);
                    }
                    TuneValue::String(_s) => {
                        // Non-bits string constants - should use get_constant_string_value
                        eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String, but constant is not Bits type, returning 0", name);
                        return Ok(0.0);
                    }
                    TuneValue::Array(arr) => {
                        // For arrays, return first element or 0
                        if !arr.is_empty() {
                            return Ok(arr[0]);
                        }
                        return Ok(0.0);
                    }
                    TuneValue::Bool(b) => {
                        return Ok(if *b { 1.0 } else { 0.0 });
                    }
                }
            } else {
                // Constant not in TuneFile - return 0 (or default)
                eprintln!("[DEBUG] get_constant_value: Constant '{}' not found in TuneFile, returning 0", name);
                return Ok(0.0);
            }
        } else {
            // No tune file loaded - return 0
            eprintln!("[DEBUG] get_constant_value: No TuneFile loaded, returning 0");
            return Ok(0.0);
        }
    }
    
    // When online, read from ECU
    // Handle bits constants specially (they're packed, size_bytes() == 0)
    if constant.data_type == libretune_core::ini::DataType::Bits {
        let bit_pos = constant.bit_position.unwrap_or(0);
        let bit_size = constant.bit_size.unwrap_or(1);
        
        // Calculate which byte contains the bits and the bit position within that byte
        let byte_offset = (bit_pos / 8) as u16;
        let bit_in_byte = bit_pos % 8;
        
        // Calculate how many bytes we need to read (may span multiple bytes)
        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
        let bytes_needed = if bits_remaining_after_first_byte > 0 {
            // Need multiple bytes: first byte + additional bytes
            1 + ((bits_remaining_after_first_byte + 7) / 8)
        } else {
            // All bits fit in one byte
            1
        };
        
        // Read the byte(s) containing the bits from ECU
        let read_offset = constant.offset + byte_offset;
        if let Some(conn) = conn {
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                length: bytes_needed as u16,
            };
            if let Ok(bytes) = conn.read_memory(params) {
                if bytes.is_empty() {
                    return Ok(0.0);
                }
                
                // Extract bits from the first byte
                let first_byte = bytes[0];
                let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                let mask_first = if bits_in_first_byte >= 8 {
                    0xFF
                } else {
                    (1u8 << bits_in_first_byte) - 1
                };
                let mut bit_val = ((first_byte >> bit_in_byte) & mask_first) as u32;
                
                // If bits span multiple bytes, extract from additional bytes
                if bits_remaining_after_first_byte > 0 && bytes.len() > 1 {
                    let mut bits_collected = bits_in_first_byte;
                    for i in 1..bytes.len() {
                        let remaining_bits = bit_size - bits_collected;
                        if remaining_bits == 0 {
                            break;
                        }
                        let bits_from_this_byte = remaining_bits.min(8);
                        let mask = if bits_from_this_byte >= 8 {
                            0xFF
                        } else {
                            (1u8 << bits_from_this_byte) - 1
                        };
                        let val_from_byte = (bytes[i] & mask) as u32;
                        bit_val |= val_from_byte << bits_collected;
                        bits_collected += bits_from_this_byte;
                    }
                }
                
                // Return the raw bit value (index into bit_options array)
                eprintln!("[DEBUG] get_constant_value: Read bits constant '{}' from ECU: bit_val={}, bit_options len={}", 
                    name, bit_val, constant.bit_options.len());
                return Ok(bit_val as f64);
            }
        }
        
        eprintln!("[DEBUG] get_constant_value: Could not read bits constant '{}' from ECU, returning 0", name);
        return Ok(0.0);
    }
    
    let length = constant.size_bytes() as u16;
    if length == 0 { return Ok(0.0); } // Zero-size constants (shouldn't happen for non-bits)
    
    // If connected to ECU, always read from ECU (live data)
    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, 0, def.endianness) {
            return Ok(constant.raw_to_display(raw_val));
        }
        return Ok(0.0);
    }
    
    // If offline, read from cache (MSQ data)
    if let Some(cache) = cache_guard.as_ref() {
        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
            if let Some(raw_val) = constant.data_type.read_from_bytes(raw_data, 0, def.endianness) {
                return Ok(constant.raw_to_display(raw_val));
            }
        }
    }
    
    // No cache and not connected - return 0
    Ok(0.0)
}

#[tauri::command]
async fn update_constant(
    state: tauri::State<'_, AppState>,
    name: String,
    value: f64
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def.constants.get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        if let Some(cache) = cache_guard.as_mut() {
            cache.local_values.insert(name, value);
        }
        return Ok(());
    }
    
    // Convert display value to raw bytes
    let raw_val = constant.display_to_raw(value);
    let mut raw_data = vec![0u8; constant.size_bytes() as usize];
    constant.data_type.write_to_bytes(&mut raw_data, 0, raw_val, def.endianness);

    // Always write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Get or create page data
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    // Create empty page if it doesn't exist
                    vec![0u8; def.page_sizes.get(constant.page as usize).copied().unwrap_or(256) as usize]
                });
                
                // Update the page data
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
            }
            
            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected (optional - offline mode works without this)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };
        
        // Don't fail if ECU write fails - offline mode should still work
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
async fn get_all_constant_values(state: tauri::State<'_, AppState>) -> Result<HashMap<String, f64>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    let mut conn_guard = state.connection.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    
    let mut values = HashMap::new();
    for (name, constant) in &def.constants {
        // Skip array constants (only need scalars for visibility conditions)
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            continue;
        }
        
        // Try to get the value - prioritize ECU if connected, otherwise tune file or cache
        let value = if let Some(ref mut conn_ptr) = conn_guard.as_mut() {
            // Online: read from ECU
            let length = constant.size_bytes() as u16;
            if length > 0 {
                let params = libretune_core::protocol::commands::ReadMemoryParams {
                    can_id: 0,
                    page: constant.page,
                    offset: constant.offset,
                    length,
                };
                if let Ok(raw_data) = conn_ptr.read_memory(params) {
                    if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, 0, def.endianness) {
                        constant.raw_to_display(raw_val)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else if constant.data_type == DataType::Bits {
                // Bits constant - read from byte and extract bits
                let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
                let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
                let bytes_needed = ((bit_in_byte + constant.bit_size.unwrap_or(0) + 7) / 8) as u16;
                let params = libretune_core::protocol::commands::ReadMemoryParams {
                    can_id: 0,
                    page: constant.page,
                    offset: constant.offset + byte_offset,
                    length: bytes_needed.max(1),
                };
                if let Ok(raw_data) = conn_ptr.read_memory(params) {
                    let mut bit_value = 0u64;
                    for (i, &byte) in raw_data.iter().enumerate() {
                        let bit_start = if i == 0 { bit_in_byte } else { 0 };
                        let bit_end = if i == bytes_needed as usize - 1 {
                            bit_in_byte + constant.bit_size.unwrap_or(0)
                        } else {
                            8
                        };
                        let bits = ((byte >> bit_start) & ((1u8 << (bit_end - bit_start)) - 1)) as u64;
                        bit_value |= bits << (i * 8);
                    }
                    bit_value as f64
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            // Offline: read from TuneFile first, then cache
            if let Some(tune) = tune_guard.as_ref() {
                if let Some(tune_value) = tune.constants.get(name) {
                    use libretune_core::tune::TuneValue;
                    match tune_value {
                        TuneValue::Scalar(v) => *v,
                        TuneValue::Bool(b) if constant.data_type == DataType::Bits => {
                            // Convert boolean to index (false = 0, true = 1)
                            // This matches the typical bit_options pattern: ["false", "true"]
                            if *b { 1.0 } else { 0.0 }
                        }
                        TuneValue::String(s) if constant.data_type == DataType::Bits => {
                            // Look up string in bit_options
                            if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                                index as f64
                            } else if let Some(index) = constant.bit_options.iter().position(|opt| opt.eq_ignore_ascii_case(s)) {
                                index as f64
                            } else {
                                0.0
                            }
                        }
                        _ => 0.0,
                    }
                } else if let Some(cache) = cache_guard.as_ref() {
                    // Fall back to cache
                    let length = constant.size_bytes() as u16;
                    if length > 0 {
                        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
                            if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, 0, def.endianness) {
                                constant.raw_to_display(raw_val)
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        }
                    } else if constant.data_type == DataType::Bits {
                        // Bits constant from cache
                        let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
                        let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
                        let bytes_needed = ((bit_in_byte + constant.bit_size.unwrap_or(0) + 7) / 8) as u16;
                        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset + byte_offset, bytes_needed.max(1)) {
                            let mut bit_value = 0u64;
                            for (i, &byte) in raw_data.iter().enumerate() {
                                let bit_start = if i == 0 { bit_in_byte } else { 0 };
                                let bit_end = if i == bytes_needed as usize - 1 {
                                    bit_in_byte + constant.bit_size.unwrap_or(0)
                                } else {
                                    8
                                };
                                let bits = ((byte >> bit_start) & ((1u8 << (bit_end - bit_start)) - 1)) as u64;
                                bit_value |= bits << (i * 8);
                            }
                            bit_value as f64
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                } else {
                    // Not in TuneFile, try cache
                    if let Some(cache) = cache_guard.as_ref() {
                        let length = constant.size_bytes() as u16;
                        if length > 0 {
                            if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
                                if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, 0, def.endianness) {
                                    constant.raw_to_display(raw_val)
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            }
                        } else if constant.data_type == DataType::Bits {
                            // Bits constant from cache
                            let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
                            let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
                            let bytes_needed = ((bit_in_byte + constant.bit_size.unwrap_or(0) + 7) / 8) as u16;
                            if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset + byte_offset, bytes_needed.max(1)) {
                                let mut bit_value = 0u64;
                                for (i, &byte) in raw_data.iter().enumerate() {
                                    let bit_start = if i == 0 { bit_in_byte } else { 0 };
                                    let bit_end = if i == bytes_needed as usize - 1 {
                                        bit_in_byte + constant.bit_size.unwrap_or(0)
                                    } else {
                                        8
                                    };
                                    let bits = ((byte >> bit_start) & ((1u8 << (bit_end - bit_start)) - 1)) as u64;
                                    bit_value |= bits << (i * 8);
                                }
                                bit_value as f64
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                }
            } else if let Some(cache) = cache_guard.as_ref() {
                // No tune file, try cache
                let length = constant.size_bytes() as u16;
                if length > 0 {
                    if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
                        if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, 0, def.endianness) {
                            constant.raw_to_display(raw_val)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                } else if constant.data_type == DataType::Bits {
                    // Bits constant from cache
                    let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
                    let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
                    let bytes_needed = ((bit_in_byte + constant.bit_size.unwrap_or(0) + 7) / 8) as u16;
                    if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset + byte_offset, bytes_needed.max(1)) {
                        let mut bit_value = 0u64;
                        for (i, &byte) in raw_data.iter().enumerate() {
                            let bit_start = if i == 0 { bit_in_byte } else { 0 };
                            let bit_end = if i == bytes_needed as usize - 1 {
                                bit_in_byte + constant.bit_size.unwrap_or(0)
                            } else {
                                8
                            };
                            let bits = ((byte >> bit_start) & ((1u8 << (bit_end - bit_start)) - 1)) as u64;
                            bit_value |= bits << (i * 8);
                        }
                        bit_value as f64
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };
        
        values.insert(name.clone(), value);
    }
    
    Ok(values)
}

#[tauri::command]
async fn start_autotune(
    state: tauri::State<'_, AppState>,
    _table_name: String,
    settings: AutoTuneSettings,
    filters: AutoTuneFilters,
    authority_limits: AutoTuneAuthorityLimits,
) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    guard.start();
    Ok(())
}

#[tauri::command]
async fn stop_autotune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    guard.stop();
    Ok(())
}

#[derive(Serialize)]
struct AutoTuneHeatEntry {
    cell_x: usize,
    cell_y: usize,
    hit_weighting: f64,
    change_magnitude: f64,
    beginning_value: f64,
    recommended_value: f64,
    hit_count: u32,
}

#[tauri::command]
async fn get_autotune_recommendations(state: tauri::State<'_, AppState>) -> Result<Vec<AutoTuneRecommendation>, String> {
    let guard = state.autotune_state.lock().await;
    Ok(guard.get_recommendations())
}

#[tauri::command]
async fn get_autotune_heatmap(state: tauri::State<'_, AppState>) -> Result<Vec<AutoTuneHeatEntry>, String> {
    let guard = state.autotune_state.lock().await;
    let recs = guard.get_recommendations();

    let mut entries: Vec<AutoTuneHeatEntry> = Vec::new();
    for r in recs.iter() {
        let change = (r.recommended_value - r.beginning_value).abs();
        entries.push(AutoTuneHeatEntry {
            cell_x: r.cell_x,
            cell_y: r.cell_y,
            hit_weighting: r.hit_weighting,
            change_magnitude: change,
            beginning_value: r.beginning_value,
            recommended_value: r.recommended_value,
            hit_count: r.hit_count,
        });
    }

    Ok(entries)
}

#[tauri::command]
async fn send_autotune_recommendations(state: tauri::State<'_, AppState>, table_name: String) -> Result<(), String> {
    // Collect recommendations
    let guard = state.autotune_state.lock().await;
    let recs = guard.get_recommendations();
    if recs.is_empty() {
        return Err("No recommendations to send".to_string());
    }

    // Ensure connection and definition exist
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Find target table
    let table = def.get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    // Read current table map values
    let constant = def.constants.get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let element_count = constant.shape.element_count();
    let element_size = constant.data_type.size_bytes();
    let length = constant.size_bytes() as u16;

    if length == 0 {
        return Err("Table has zero length".to_string());
    }

    let params = libretune_core::protocol::commands::ReadMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        length,
    };

    let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;

    // Convert to display values
    let mut values: Vec<f64> = Vec::with_capacity(element_count);
    for i in 0..element_count {
        let offset = i * element_size;
        if let Some(raw_val) = constant.data_type.read_from_bytes(&raw_data, offset, def.endianness) {
            values.push(constant.raw_to_display(raw_val));
        } else {
            values.push(0.0);
        }
    }

    // Determine table dimensions
    let x_size = table.x_size;
    let y_size = table.y_size;

    // Apply recommendations
    for r in recs.iter() {
        if r.cell_x >= x_size || r.cell_y >= y_size {
            eprintln!("[WARN] send_autotune_recommendations: recommendation out of bounds: {}x{}", r.cell_x, r.cell_y);
            continue;
        }
        let idx = r.cell_y * x_size + r.cell_x;
        values[idx] = r.recommended_value;
    }

    // Convert back to raw bytes
    let mut raw_out = vec![0u8; constant.size_bytes() as usize];
    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant.data_type.write_to_bytes(&mut raw_out, offset, raw_val, def.endianness);
    }

    // Write back to ECU
    let write_params = libretune_core::protocol::commands::WriteMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        data: raw_out,
    };

    conn.write_memory(write_params).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn burn_autotune_recommendations(state: tauri::State<'_, AppState>, table_name: String) -> Result<(), String> {
    // Ensure connection and definition exist
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Find target table constant page
    let table = def.get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def.constants.get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let params = libretune_core::protocol::commands::BurnParams {
        can_id: 0,
        page: constant.page,
    };

    conn.burn(params).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn lock_autotune_cells(state: tauri::State<'_, AppState>, cells: Vec<(usize, usize)>) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    guard.lock_cells(cells);
    Ok(())
}

#[tauri::command]
async fn start_autotune_autosend(app: tauri::AppHandle, state: tauri::State<'_, AppState>, table_name: String, interval_ms: Option<u64>) -> Result<(), String> {
    let interval = interval_ms.unwrap_or(15000);

    // Ensure connection and definition exist
    {
        let conn_guard = state.connection.lock().await;
        let def_guard = state.definition.lock().await;
        if conn_guard.is_none() || def_guard.is_none() {
            return Err("Connection or definition missing".to_string());
        }
    }

    let mut task_guard = state.autotune_send_task.lock().await;
    if task_guard.is_some() {
        // Already running
        return Ok(());
    }

    let app_handle = app.clone();
    let table = table_name.clone();

    let handle = tokio::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(interval));
        loop {
            ticker.tick().await;

            // Run send_autotune_recommendations logic
            let recs = {
                let guard = app_state.autotune_state.lock().await;
                guard.get_recommendations()
            };

            if recs.is_empty() {
                continue;
            }

            // Acquire connection and definition
            let mut conn_guard = app_state.connection.lock().await;
            let def_guard = app_state.definition.lock().await;
            let def = match def_guard.as_ref() {
                Some(d) => d.clone(),
                None => continue,
            };
            let conn = match conn_guard.as_mut() {
                Some(c) => c,
                None => continue,
            };

            // Find table constant
            let table_def = match def.get_table_by_name_or_map(&table) {
                Some(t) => t.clone(),
                None => continue,
            };

            let constant = match def.constants.get(&table_def.map) {
                Some(cnst) => cnst.clone(),
                None => continue,
            };

            // Read current data
            let params = libretune_core::protocol::commands::ReadMemoryParams { can_id: 0, page: constant.page, offset: constant.offset, length: constant.size_bytes() as u16 };
            let raw_data = match conn.read_memory(params) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let element_count = constant.shape.element_count();
            let element_size = constant.data_type.size_bytes();
            let mut values: Vec<f64> = Vec::with_capacity(element_count);
            for i in 0..element_count {
                let off = i * element_size;
                if let Some(rv) = constant.data_type.read_from_bytes(&raw_data, off, def.endianness) {
                    values.push(constant.raw_to_display(rv));
                } else {
                    values.push(0.0);
                }
            }

            let x_size = table_def.x_size;
            let y_size = table_def.y_size;

            // Apply recommendations
            for r in recs.iter() {
                if r.cell_x >= x_size || r.cell_y >= y_size { continue; }
                let idx = r.cell_y * x_size + r.cell_x;
                values[idx] = r.recommended_value;
            }

            // Convert back to bytes
            let mut raw_out = vec![0u8; constant.size_bytes() as usize];
            for (i, v) in values.iter().enumerate() {
                let rv = constant.display_to_raw(*v);
                let offset = i * element_size;
                constant.data_type.write_to_bytes(&mut raw_out, offset, rv, def.endianness);
            }

            let write_params = libretune_core::protocol::commands::WriteMemoryParams { can_id: 0, page: constant.page, offset: constant.offset, data: raw_out };
            let _ = conn.write_memory(write_params);
        }
    });

    *task_guard = Some(handle);

    Ok(())
}

#[tauri::command]
async fn stop_autotune_autosend(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut task_guard = state.autotune_send_task.lock().await;
    if let Some(h) = task_guard.take() {
        h.abort();
    }
    Ok(())
}

#[tauri::command]
async fn unlock_autotune_cells(state: tauri::State<'_, AppState>, cells: Vec<(usize, usize)>) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    guard.unlock_cells(cells);
    Ok(())
}

#[tauri::command]
async fn rebin_table(
    _state: tauri::State<'_, AppState>,
    _table_name: String,
    _new_x_bins: Vec<f64>,
    _new_y_bins: Vec<f64>,
    _interpolate_z: bool,
) -> Result<TableData, String> {
    // TODO: Requires proper ECU connection testing
    Err("Rebin table operation requires ECU connection".to_string())
}

#[tauri::command]
async fn smooth_table(
    _state: tauri::State<'_, AppState>,
    _table_name: String,
    _factor: f64,
    _selected_cells: Vec<(usize, usize)>,
) -> Result<TableData, String> {
    // TODO: Requires proper ECU connection testing
    Err("Smooth table operation requires ECU connection".to_string())
}

#[tauri::command]
async fn interpolate_cells(
    _state: tauri::State<'_, AppState>,
    _table_name: String,
    _selected_cells: Vec<(usize, usize)>,
) -> Result<TableData, String> {
    // TODO: Requires proper ECU connection testing
    Err("Interpolate cells operation requires ECU connection".to_string())
}

#[tauri::command]
async fn scale_cells(
    _state: tauri::State<'_, AppState>,
    _table_name: String,
    _selected_cells: Vec<(usize, usize)>,
    _scale_factor: f64,
) -> Result<TableData, String> {
    // TODO: Requires proper ECU connection testing
    Err("Scale cells operation requires ECU connection".to_string())
}

#[tauri::command]
async fn set_cells_equal(
    _state: tauri::State<'_, AppState>,
    _table_name: String,
    _selected_cells: Vec<(usize, usize)>,
    _value: f64,
) -> Result<TableData, String> {
    // TODO: Requires proper ECU connection testing
    Err("Set cells equal operation requires ECU connection".to_string())
}

#[tauri::command]
async fn save_dashboard_layout(
    state: tauri::State<'_, AppState>,
    project_name: String,
    layout: DashboardLayout,
) -> Result<(), String> {
    let path = get_dashboard_file_path(&project_name);
    
    // Convert DashboardLayout to TunerStudio DashFile format
    let dash_file = convert_layout_to_dashfile(&layout);
    
    // Write as TunerStudio XML format
    dash::save_dash_file(&dash_file, &path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;
    
    Ok(())
}

#[tauri::command]
async fn load_dashboard_layout(
    state: tauri::State<'_, AppState>,
    project_name: String,
) -> Result<DashboardLayout, String> {
    let path = get_dashboard_file_path(&project_name);
    
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read dashboard file: {}", e))?;
    
    // Try TunerStudio XML format first
    if content.trim().starts_with("<?xml") || content.trim().starts_with("<dsh") {
        let dash_file = dash::parse_dash_file(&content)
            .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?;
        return Ok(convert_dashfile_to_layout(&dash_file, &project_name));
    }
    
    // Fall back to legacy JSON format for backward compatibility
    let layout: DashboardLayout = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse dashboard file: {}", e))?;
    
    Ok(layout)
}

#[tauri::command]
async fn list_dashboard_layouts(
    state: tauri::State<'_, AppState>,
    project_name: String,
) -> Result<Vec<String>, String> {
    let projects_dir = libretune_core::project::Project::projects_dir()
        .map_err(|e| format!("Failed to get projects directory: {}", e))?;
    
    let mut dashboards = Vec::new();
    
    // Ensure projects directory exists
    if !projects_dir.exists() {
        let _ = std::fs::create_dir_all(&projects_dir);
        return Ok(dashboards); // Return empty list
    }
    
    let entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;
    
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".dash") {
                let dash_name = name.replace(".dash", "");
                dashboards.push(dash_name);
            }
        }
    }
    
    dashboards.sort();
    Ok(dashboards)
}

/// Create a LibreTune default dashboard
#[tauri::command]
async fn create_default_dashboard(
    state: tauri::State<'_, AppState>,
    project_name: String,
    template: String,
) -> Result<DashboardLayout, String> {
    use libretune_core::dash::{GaugeCluster, BackgroundStyle};
    
    println!("[create_default_dashboard] Creating template: {} for project: {}", template, project_name);
    
    let dash_file = match template.as_str() {
        "basic" => create_basic_dashboard(),
        "racing" => create_racing_dashboard(),
        "tuning" => create_tuning_dashboard(),
        _ => create_basic_dashboard(),
    };
    
    println!("[create_default_dashboard] Dashboard has {} components", dash_file.gauge_cluster.components.len());
    
    // Save it
    let path = get_dashboard_file_path(&project_name);
    println!("[create_default_dashboard] Saving to: {:?}", path);
    dash::save_dash_file(&dash_file, &path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;
    
    // Return as layout
    let layout = convert_dashfile_to_layout(&dash_file, &project_name);
    println!("[create_default_dashboard] Returning layout with {} gauges", layout.gauges.len());
    Ok(layout)
}

/// Load a TunerStudio .dash file directly from a path (for testing)
#[tauri::command]
async fn load_tunerstudio_dash(
    path: String,
) -> Result<DashboardLayout, String> {
    println!("[load_tunerstudio_dash] Loading from: {}", path);
    
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read dashboard file: {}", e))?;
    
    let dash_file = dash::parse_dash_file(&content)
        .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?;
    
    let layout = convert_dashfile_to_layout(&dash_file, "TunerStudio Dashboard");
    println!("[load_tunerstudio_dash] Loaded {} gauges", layout.gauges.len());
    Ok(layout)
}

/// Load a TunerStudio .dash file and return the full DashFile structure
#[tauri::command]
async fn get_dash_file(
    path: String,
) -> Result<DashFile, String> {
    println!("[get_dash_file] Loading from: {}", path);
    
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read dashboard file: {}", e))?;
    
    let dash_file = dash::parse_dash_file(&content)
        .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?;
    
    println!("[get_dash_file] Loaded {} components, {} embedded images", 
        dash_file.gauge_cluster.components.len(),
        dash_file.gauge_cluster.embedded_images.len());
    Ok(dash_file)
}

/// Info about an available dashboard file
#[derive(Serialize)]
struct DashFileInfo {
    name: String,
    path: String,
}

/// List all available dashboard files (.ltdash.xml and .dash for import)
/// Scans app data dashboards directory and creates defaults if empty
#[tauri::command]
async fn list_available_dashes(
    app: tauri::AppHandle,
) -> Result<Vec<DashFileInfo>, String> {
    let dash_dir = get_dashboards_dir(&app);
    
    // Create directory if it doesn't exist
    if !dash_dir.exists() {
        std::fs::create_dir_all(&dash_dir)
            .map_err(|e| format!("Failed to create dashboards directory: {}", e))?;
    }
    
    // Check if directory is empty and create defaults if so
    let is_empty = std::fs::read_dir(&dash_dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);
    
    if is_empty {
        println!("[list_available_dashes] Creating default dashboards in {:?}", dash_dir);
        create_default_dashboard_files(&dash_dir)?;
    }
    
    let mut dashes = Vec::new();
    
    match std::fs::read_dir(&dash_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                
                // Accept .ltdash.xml (native) and .dash (TunerStudio import)
                if file_name.ends_with(".ltdash.xml") || file_name.ends_with(".dash") {
                    if let Some(name) = path.file_name() {
                        dashes.push(DashFileInfo {
                            name: name.to_string_lossy().to_string(),
                            path: path.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to read dash directory: {}", e));
        }
    }
    
    dashes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(dashes)
}

/// Create default dashboard XML files in the given directory
fn create_default_dashboard_files(dir: &Path) -> Result<(), String> {
    // Basic Dashboard
    let basic = create_basic_dashboard();
    let basic_xml = dash::write_dash_file(&basic)
        .map_err(|e| format!("Failed to serialize basic dashboard: {}", e))?;
    std::fs::write(dir.join("Basic.ltdash.xml"), basic_xml)
        .map_err(|e| format!("Failed to write Basic.ltdash.xml: {}", e))?;
    
    // Tuning Dashboard  
    let tuning = create_tuning_dashboard();
    let tuning_xml = dash::write_dash_file(&tuning)
        .map_err(|e| format!("Failed to serialize tuning dashboard: {}", e))?;
    std::fs::write(dir.join("Tuning.ltdash.xml"), tuning_xml)
        .map_err(|e| format!("Failed to write Tuning.ltdash.xml: {}", e))?;
    
    // Racing Dashboard
    let racing = create_racing_dashboard();
    let racing_xml = dash::write_dash_file(&racing)
        .map_err(|e| format!("Failed to serialize racing dashboard: {}", e))?;
    std::fs::write(dir.join("Racing.ltdash.xml"), racing_xml)
        .map_err(|e| format!("Failed to write Racing.ltdash.xml: {}", e))?;
    
    println!("[create_default_dashboard_files] Created 3 default dashboards");
    Ok(())
}

/// Get list of available dashboard templates
#[tauri::command]
async fn get_dashboard_templates() -> Result<Vec<DashboardTemplateInfo>, String> {
    Ok(vec![
        DashboardTemplateInfo {
            id: "basic".to_string(),
            name: "Basic Dashboard".to_string(),
            description: "Essential gauges: RPM, AFR, Coolant, Throttle".to_string(),
        },
        DashboardTemplateInfo {
            id: "racing".to_string(),
            name: "Racing Dashboard".to_string(),
            description: "Large RPM with shift lights, oil pressure, water temp".to_string(),
        },
        DashboardTemplateInfo {
            id: "tuning".to_string(),
            name: "Tuning Dashboard".to_string(),
            description: "AFR, VE, Spark advance, and correction factors".to_string(),
        },
    ])
}

#[derive(Serialize)]
struct DashboardTemplateInfo {
    id: String,
    name: String,
    description: String,
}

/// Create a basic dashboard layout - LibreTune default
fn create_basic_dashboard() -> DashFile {
    use libretune_core::dash::{GaugeCluster, BackgroundStyle};
    
    let mut dash = DashFile {
        bibliography: Bibliography {
            author: "LibreTune".to_string(),
            company: "LibreTune Project".to_string(),
            write_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
        version_info: VersionInfo {
            file_format: "3.0".to_string(),
            firmware_signature: None,
        },
        gauge_cluster: GaugeCluster {
            anti_aliasing: true,
            cluster_background_color: TsColor { alpha: 255, red: 25, green: 25, blue: 30 },
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };
    
    // Row 1: Large RPM gauge
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "rpm".to_string(),
        title: "RPM".to_string(),
        units: "".to_string(),
        output_channel: "rpm".to_string(),
        min: 0.0,
        max: 8000.0,
        high_warning: Some(6500.0),
        high_critical: Some(7200.0),
        gauge_painter: GaugePainter::AnalogGauge,
        relative_x: 0.02,
        relative_y: 0.02,
        relative_width: 0.46,
        relative_height: 0.48,
        back_color: TsColor { alpha: 255, red: 40, green: 40, blue: 45 },
        font_color: TsColor { alpha: 255, red: 255, green: 255, blue: 255 },
        needle_color: TsColor { alpha: 255, red: 255, green: 80, blue: 0 },
        trim_color: TsColor { alpha: 255, red: 100, green: 100, blue: 110 },
        warn_color: TsColor { alpha: 255, red: 255, green: 200, blue: 0 },
        critical_color: TsColor { alpha: 255, red: 255, green: 50, blue: 50 },
        ..Default::default()
    }));
    
    // Row 1: AFR gauge (digital readout)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "afr".to_string(),
        title: "AFR".to_string(),
        units: "".to_string(),
        output_channel: "afr".to_string(),
        min: 10.0,
        max: 20.0,
        low_warning: Some(11.5),
        high_warning: Some(15.0),
        low_critical: Some(10.5),
        high_critical: Some(16.5),
        value_digits: 2,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.52,
        relative_y: 0.02,
        relative_width: 0.22,
        relative_height: 0.23,
        back_color: TsColor { alpha: 255, red: 35, green: 35, blue: 40 },
        font_color: TsColor { alpha: 255, red: 0, green: 255, blue: 128 },
        ..Default::default()
    }));
    
    // Row 1: Coolant temp (bar gauge)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "coolant".to_string(),
        title: "COOLANT".to_string(),
        units: "°C".to_string(),
        output_channel: "coolant".to_string(),
        min: -40.0,
        max: 120.0,
        high_warning: Some(100.0),
        high_critical: Some(110.0),
        value_digits: 0,
        gauge_painter: GaugePainter::VerticalBarGauge,
        relative_x: 0.76,
        relative_y: 0.02,
        relative_width: 0.10,
        relative_height: 0.48,
        back_color: TsColor { alpha: 255, red: 30, green: 30, blue: 35 },
        font_color: TsColor { alpha: 255, red: 100, green: 200, blue: 255 },
        trim_color: TsColor { alpha: 255, red: 0, green: 150, blue: 255 },
        warn_color: TsColor { alpha: 255, red: 255, green: 200, blue: 0 },
        critical_color: TsColor { alpha: 255, red: 255, green: 50, blue: 50 },
        ..Default::default()
    }));
    
    // Row 1: IAT temp (bar gauge)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "iat".to_string(),
        title: "IAT".to_string(),
        units: "°C".to_string(),
        output_channel: "iat".to_string(),
        min: -40.0,
        max: 80.0,
        high_warning: Some(50.0),
        high_critical: Some(65.0),
        value_digits: 0,
        gauge_painter: GaugePainter::VerticalBarGauge,
        relative_x: 0.88,
        relative_y: 0.02,
        relative_width: 0.10,
        relative_height: 0.48,
        back_color: TsColor { alpha: 255, red: 30, green: 30, blue: 35 },
        font_color: TsColor { alpha: 255, red: 255, green: 200, blue: 100 },
        trim_color: TsColor { alpha: 255, red: 255, green: 180, blue: 0 },
        warn_color: TsColor { alpha: 255, red: 255, green: 200, blue: 0 },
        critical_color: TsColor { alpha: 255, red: 255, green: 50, blue: 50 },
        ..Default::default()
    }));
    
    // Row 1: TPS (digital, below AFR)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "tps".to_string(),
        title: "TPS".to_string(),
        units: "%".to_string(),
        output_channel: "tps".to_string(),
        min: 0.0,
        max: 100.0,
        value_digits: 1,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.52,
        relative_y: 0.27,
        relative_width: 0.22,
        relative_height: 0.23,
        back_color: TsColor { alpha: 255, red: 35, green: 35, blue: 40 },
        font_color: TsColor { alpha: 255, red: 255, green: 255, blue: 255 },
        ..Default::default()
    }));
    
    // Row 2: MAP gauge
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "map".to_string(),
        title: "MAP".to_string(),
        units: "kPa".to_string(),
        output_channel: "map".to_string(),
        min: 0.0,
        max: 250.0,
        value_digits: 0,
        gauge_painter: GaugePainter::HorizontalBarGauge,
        relative_x: 0.02,
        relative_y: 0.52,
        relative_width: 0.30,
        relative_height: 0.12,
        back_color: TsColor { alpha: 255, red: 30, green: 30, blue: 35 },
        font_color: TsColor { alpha: 255, red: 200, green: 200, blue: 255 },
        trim_color: TsColor { alpha: 255, red: 100, green: 100, blue: 200 },
        ..Default::default()
    }));
    
    // Row 2: Battery Voltage
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "battery".to_string(),
        title: "BATTERY".to_string(),
        units: "V".to_string(),
        output_channel: "battery".to_string(),
        min: 10.0,
        max: 16.0,
        low_warning: Some(11.5),
        high_warning: Some(15.0),
        low_critical: Some(10.5),
        value_digits: 1,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.34,
        relative_y: 0.52,
        relative_width: 0.15,
        relative_height: 0.12,
        back_color: TsColor { alpha: 255, red: 35, green: 35, blue: 40 },
        font_color: TsColor { alpha: 255, red: 255, green: 220, blue: 100 },
        ..Default::default()
    }));
    
    // Row 2: Advance
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "advance".to_string(),
        title: "ADVANCE".to_string(),
        units: "°".to_string(),
        output_channel: "advance".to_string(),
        min: -10.0,
        max: 50.0,
        value_digits: 1,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.51,
        relative_y: 0.52,
        relative_width: 0.15,
        relative_height: 0.12,
        back_color: TsColor { alpha: 255, red: 35, green: 35, blue: 40 },
        font_color: TsColor { alpha: 255, red: 255, green: 150, blue: 50 },
        ..Default::default()
    }));
    
    // Row 2: VE
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "ve".to_string(),
        title: "VE".to_string(),
        units: "%".to_string(),
        output_channel: "ve".to_string(),
        min: 0.0,
        max: 150.0,
        value_digits: 1,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.68,
        relative_y: 0.52,
        relative_width: 0.15,
        relative_height: 0.12,
        back_color: TsColor { alpha: 255, red: 35, green: 35, blue: 40 },
        font_color: TsColor { alpha: 255, red: 100, green: 255, blue: 100 },
        ..Default::default()
    }));
    
    // Row 2: PW
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "pw".to_string(),
        title: "PW".to_string(),
        units: "ms".to_string(),
        output_channel: "pulseWidth1".to_string(),
        min: 0.0,
        max: 20.0,
        value_digits: 2,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.85,
        relative_y: 0.52,
        relative_width: 0.13,
        relative_height: 0.12,
        back_color: TsColor { alpha: 255, red: 35, green: 35, blue: 40 },
        font_color: TsColor { alpha: 255, red: 200, green: 200, blue: 200 },
        ..Default::default()
    }));
    
    dash
}

/// Create a racing-focused dashboard
fn create_racing_dashboard() -> DashFile {
    use libretune_core::dash::{GaugeCluster, BackgroundStyle};
    
    let mut dash = DashFile {
        bibliography: Bibliography {
            author: "LibreTune".to_string(),
            company: "LibreTune Project".to_string(),
            write_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
        version_info: VersionInfo {
            file_format: "3.0".to_string(),
            firmware_signature: None,
        },
        gauge_cluster: GaugeCluster {
            anti_aliasing: true,
            cluster_background_color: TsColor { alpha: 255, red: 15, green: 15, blue: 20 },
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };
    
    // Giant center RPM
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "rpm".to_string(),
        title: "RPM".to_string(),
        units: "".to_string(),
        output_channel: "rpm".to_string(),
        min: 0.0,
        max: 10000.0,
        high_warning: Some(8000.0),
        high_critical: Some(9000.0),
        gauge_painter: GaugePainter::AnalogGauge,
        relative_x: 0.15,
        relative_y: 0.05,
        relative_width: 0.70,
        relative_height: 0.70,
        back_color: TsColor { alpha: 255, red: 25, green: 25, blue: 30 },
        font_color: TsColor { alpha: 255, red: 255, green: 255, blue: 255 },
        needle_color: TsColor { alpha: 255, red: 255, green: 0, blue: 0 },
        trim_color: TsColor { alpha: 255, red: 80, green: 80, blue: 90 },
        warn_color: TsColor { alpha: 255, red: 255, green: 255, blue: 0 },
        critical_color: TsColor { alpha: 255, red: 255, green: 0, blue: 0 },
        ..Default::default()
    }));
    
    // Oil pressure (left)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "oilpres".to_string(),
        title: "OIL".to_string(),
        units: "psi".to_string(),
        output_channel: "oilPressure".to_string(),
        min: 0.0,
        max: 100.0,
        low_warning: Some(20.0),
        low_critical: Some(10.0),
        value_digits: 0,
        gauge_painter: GaugePainter::VerticalBarGauge,
        relative_x: 0.02,
        relative_y: 0.05,
        relative_width: 0.10,
        relative_height: 0.55,
        back_color: TsColor { alpha: 255, red: 30, green: 30, blue: 35 },
        font_color: TsColor { alpha: 255, red: 255, green: 200, blue: 100 },
        ..Default::default()
    }));
    
    // Water temp (right)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "coolant".to_string(),
        title: "H2O".to_string(),
        units: "°C".to_string(),
        output_channel: "coolant".to_string(),
        min: 0.0,
        max: 130.0,
        high_warning: Some(105.0),
        high_critical: Some(115.0),
        value_digits: 0,
        gauge_painter: GaugePainter::VerticalBarGauge,
        relative_x: 0.88,
        relative_y: 0.05,
        relative_width: 0.10,
        relative_height: 0.55,
        back_color: TsColor { alpha: 255, red: 30, green: 30, blue: 35 },
        font_color: TsColor { alpha: 255, red: 100, green: 200, blue: 255 },
        ..Default::default()
    }));
    
    // Speed (bottom left)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "speed".to_string(),
        title: "SPEED".to_string(),
        units: "km/h".to_string(),
        output_channel: "speed".to_string(),
        min: 0.0,
        max: 300.0,
        value_digits: 0,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.02,
        relative_y: 0.78,
        relative_width: 0.23,
        relative_height: 0.20,
        back_color: TsColor { alpha: 255, red: 25, green: 25, blue: 30 },
        font_color: TsColor { alpha: 255, red: 255, green: 255, blue: 255 },
        font_size_adjustment: 4,
        ..Default::default()
    }));
    
    // AFR (bottom center-left)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "afr".to_string(),
        title: "AFR".to_string(),
        units: "".to_string(),
        output_channel: "afr".to_string(),
        min: 10.0,
        max: 18.0,
        low_warning: Some(11.0),
        high_warning: Some(15.0),
        value_digits: 1,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.27,
        relative_y: 0.78,
        relative_width: 0.22,
        relative_height: 0.20,
        back_color: TsColor { alpha: 255, red: 25, green: 25, blue: 30 },
        font_color: TsColor { alpha: 255, red: 0, green: 255, blue: 128 },
        font_size_adjustment: 4,
        ..Default::default()
    }));
    
    // Boost (bottom center-right)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "boost".to_string(),
        title: "BOOST".to_string(),
        units: "psi".to_string(),
        output_channel: "boost".to_string(),
        min: -15.0,
        max: 30.0,
        high_warning: Some(22.0),
        high_critical: Some(26.0),
        value_digits: 1,
        gauge_painter: GaugePainter::BasicReadout,
        relative_x: 0.51,
        relative_y: 0.78,
        relative_width: 0.22,
        relative_height: 0.20,
        back_color: TsColor { alpha: 255, red: 25, green: 25, blue: 30 },
        font_color: TsColor { alpha: 255, red: 100, green: 200, blue: 255 },
        font_size_adjustment: 4,
        ..Default::default()
    }));
    
    // Fuel level (bottom right)
    dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
        id: "fuel".to_string(),
        title: "FUEL".to_string(),
        units: "%".to_string(),
        output_channel: "fuelLevel".to_string(),
        min: 0.0,
        max: 100.0,
        low_warning: Some(20.0),
        low_critical: Some(10.0),
        value_digits: 0,
        gauge_painter: GaugePainter::HorizontalBarGauge,
        relative_x: 0.75,
        relative_y: 0.78,
        relative_width: 0.23,
        relative_height: 0.20,
        back_color: TsColor { alpha: 255, red: 25, green: 25, blue: 30 },
        font_color: TsColor { alpha: 255, red: 255, green: 200, blue: 0 },
        ..Default::default()
    }));
    
    dash
}

/// Create a tuning-focused dashboard
fn create_tuning_dashboard() -> DashFile {
    use libretune_core::dash::{GaugeCluster, BackgroundStyle};
    
    let mut dash = DashFile {
        bibliography: Bibliography {
            author: "LibreTune".to_string(),
            company: "LibreTune Project".to_string(),
            write_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
        version_info: VersionInfo {
            file_format: "3.0".to_string(),
            firmware_signature: None,
        },
        gauge_cluster: GaugeCluster {
            anti_aliasing: true,
            cluster_background_color: TsColor { alpha: 255, red: 20, green: 25, blue: 30 },
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };
    
    // 3x3 grid of tuning-relevant gauges
    // (id, title, units, channel, min, max, low_crit, low_warn, high_warn, high_crit, digits)
    let gauges: Vec<(&str, &str, &str, &str, f64, f64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, i32)> = vec![
        ("rpm", "RPM", "", "rpm", 0.0, 8000.0, None, None, Some(6500.0), Some(7200.0), 0),
        ("afr", "AFR", "", "afr", 10.0, 20.0, Some(10.5), Some(11.5), Some(15.5), Some(16.5), 2),
        ("map", "MAP", "kPa", "map", 0.0, 250.0, None, None, None, None, 0),
        ("tps", "TPS", "%", "tps", 0.0, 100.0, None, None, None, None, 1),
        ("ve", "VE", "%", "ve", 0.0, 150.0, None, None, None, None, 1),
        ("advance", "ADV", "°", "advance", -10.0, 50.0, None, None, None, None, 1),
        ("egt", "EGT", "°C", "egt", 0.0, 1000.0, None, None, Some(800.0), Some(900.0), 0),
        ("pw", "PW", "ms", "pulseWidth1", 0.0, 25.0, None, None, None, None, 2),
        ("duty", "DUTY", "%", "injDuty", 0.0, 100.0, None, None, Some(85.0), Some(95.0), 1),
    ];
    
    for (i, (id, title, units, channel, min, max, low_crit, low_warn, high_warn, high_crit, digits)) in gauges.iter().enumerate() {
        let row = i / 3;
        let col = i % 3;
        let x = 0.02 + (col as f64) * 0.33;
        let y = 0.02 + (row as f64) * 0.33;
        
        dash.gauge_cluster.components.push(DashComponent::Gauge(dash::GaugeConfig {
            id: id.to_string(),
            title: title.to_string(),
            units: units.to_string(),
            output_channel: channel.to_string(),
            min: *min,
            max: *max,
            low_critical: *low_crit,
            low_warning: *low_warn,
            high_warning: *high_warn,
            high_critical: *high_crit,
            value_digits: *digits,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: x,
            relative_y: y,
            relative_width: 0.31,
            relative_height: 0.31,
            back_color: TsColor { alpha: 255, red: 30, green: 35, blue: 40 },
            font_color: TsColor { alpha: 255, red: 200, green: 255, blue: 200 },
            warn_color: TsColor { alpha: 255, red: 255, green: 200, blue: 0 },
            critical_color: TsColor { alpha: 255, red: 255, green: 50, blue: 50 },
            ..Default::default()
        }));
    }
    
    dash
}

// =============================================================================
// Tune File Save/Load/Burn Commands
// =============================================================================

#[derive(Serialize)]
struct TuneInfo {
    path: Option<String>,
    signature: String,
    modified: bool,
    has_tune: bool,
}

#[tauri::command]
async fn get_tune_info(state: tauri::State<'_, AppState>) -> Result<TuneInfo, String> {
    let tune_guard = state.current_tune.lock().await;
    let path_guard = state.current_tune_path.lock().await;
    let modified = *state.tune_modified.lock().await;
    
    match &*tune_guard {
        Some(tune) => Ok(TuneInfo {
            path: path_guard.as_ref().map(|p| p.to_string_lossy().to_string()),
            signature: tune.signature.clone(),
            modified,
            has_tune: true,
        }),
        None => Ok(TuneInfo {
            path: None,
            signature: String::new(),
            modified: false,
            has_tune: false,
        }),
    }
}

#[tauri::command]
async fn new_tune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let signature = def_guard.as_ref()
        .map(|d| d.signature.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    
    let tune = TuneFile::new(&signature);
    
    *state.current_tune.lock().await = Some(tune);
    *state.current_tune_path.lock().await = None;
    *state.tune_modified.lock().await = false;
    
    Ok(())
}

#[tauri::command]
async fn save_tune(state: tauri::State<'_, AppState>, path: Option<String>) -> Result<String, String> {
    let mut tune_guard = state.current_tune.lock().await;
    let path_guard = state.current_tune_path.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let def_guard = state.definition.lock().await;
    
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    // Write TuneCache data to TuneFile before saving (ensures offline changes are saved)
    if let Some(cache) = cache_guard.as_ref() {
        // Copy all pages from cache to tune file
        for page_num in 0..def.n_pages {
            if let Some(page_data) = cache.get_page(page_num) {
                tune.pages.insert(page_num, page_data.to_vec());
            }
        }
        
        // Read constants from cache and add to tune file
        use libretune_core::tune::TuneValue;
        let mut constants_saved = 0;
        
        for (name, constant) in &def.constants {
            // Skip PC variables - they're stored separately
            if constant.is_pc_variable {
                // Get PC variable from local_values
                if let Some(value) = cache.local_values.get(name) {
                    tune.constants.insert(name.clone(), TuneValue::Scalar(*value));
                    constants_saved += 1;
                }
                continue;
            }
            
            // Handle bits constants specially - they have zero size_bytes() but we need to read them
            if constant.data_type == libretune_core::ini::DataType::Bits {
                // Read the byte(s) containing the bits
                let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
                let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
                let bit_size = constant.bit_size.unwrap_or(0);
                let bytes_needed = ((bit_in_byte + bit_size + 7) / 8).max(1) as u16;
                
                if let Some(bytes) = cache.read_bytes(constant.page, constant.offset + byte_offset, bytes_needed) {
                    // Extract the bit value
                    let mut bit_val: u32 = 0;
                    let mut bits_remaining = bit_size;
                    let mut current_bit = bit_in_byte;
                    
                    for byte in bytes.iter().take(bytes_needed as usize) {
                        let bits_in_this_byte = bits_remaining.min(8 - current_bit);
                        // Safe shift: ensure we don't shift by 8 or more
                        let mask = if bits_in_this_byte == 0 {
                            0
                        } else if bits_in_this_byte == 8 && current_bit == 0 {
                            // All bits in this byte
                            0xFFu8
                        } else {
                            // bits_in_this_byte is guaranteed to be < 8 here
                            let base_mask = (1u8 << bits_in_this_byte.min(7)) - 1;
                            base_mask << current_bit
                        };
                        let extracted = ((*byte & mask) >> current_bit) as u32;
                        bit_val |= extracted << (bit_size - bits_remaining);
                        
                        bits_remaining = bits_remaining.saturating_sub(bits_in_this_byte);
                        if bits_remaining == 0 {
                            break;
                        }
                        current_bit = 0;
                    }
                    
                    // Convert bit index to string from bit_options
                    let bit_index = bit_val as usize;
                    if bit_index < constant.bit_options.len() {
                        let option_string = constant.bit_options[bit_index].clone();
                        tune.constants.insert(name.clone(), TuneValue::String(option_string));
                        constants_saved += 1;
                    } else {
                        // Out of range - save as numeric index (fallback)
                        tune.constants.insert(name.clone(), TuneValue::Scalar(bit_val as f64));
                        constants_saved += 1;
                    }
                }
                continue;
            }
            
            // Skip constants with zero size
            let length = constant.size_bytes() as u16;
            if length == 0 {
                continue;
            }
            
            // Read constant from cache
            let page_state = cache.page_state(constant.page);
            let page_size = cache.page_size(constant.page);
            let page_data_opt = cache.get_page(constant.page);
            let page_data_len = page_data_opt.map(|p| p.len()).unwrap_or(0);
            
            if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                eprintln!("[DEBUG] save_tune: Attempting to save '{}' - page={}, offset={}, len={}, page_state={:?}, page_size={:?}, page_data_len={}", 
                    name, constant.page, constant.offset, length, page_state, page_size, page_data_len);
            }
            
            if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
                let element_count = constant.shape.element_count();
                let element_size = constant.data_type.size_bytes();
                let mut values = Vec::new();
                
                for i in 0..element_count {
                    let offset = i * element_size;
                    if let Some(raw_val) = constant.data_type.read_from_bytes(raw_data, offset, def.endianness) {
                        values.push(constant.raw_to_display(raw_val));
                    } else {
                        values.push(0.0);
                    }
                }
                
                // Convert to TuneValue format
                let tune_value = if element_count == 1 {
                    TuneValue::Scalar(values[0])
                } else {
                    TuneValue::Array(values)
                };
                
                tune.constants.insert(name.clone(), tune_value);
                constants_saved += 1;
                
                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                    eprintln!("[DEBUG] save_tune: ✓ Saved '{}' - {} elements", name, element_count);
                }
            } else {
                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                    eprintln!("[DEBUG] save_tune: ✗ Failed to read '{}' from cache - page_state={:?}, page_size={:?}, page_data_len={}, required_offset={}", 
                        name, page_state, page_size, page_data_len, constant.offset as usize + length as usize);
                }
            }
        }
        
        eprintln!("[DEBUG] save_tune: Saved {} constants from cache to tune file", constants_saved);
    }
    
    // Update modified timestamp
    tune.touch();
    
    // Use provided path, or current path, or generate default
    let save_path = if let Some(p) = path {
        PathBuf::from(p)
    } else if let Some(p) = path_guard.as_ref() {
        p.clone()
    } else {
        // Generate default path in projects directory
        let filename = format!("{}.msq", tune.signature.replace(' ', "_"));
        libretune_core::project::Project::projects_dir()
            .map_err(|e| format!("Failed to get projects directory: {}", e))?
            .join(filename)
    };
    
    // Ensure projects directory exists
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    
    tune.save(&save_path)
        .map_err(|e| format!("Failed to save tune: {}", e))?;
    
    drop(tune_guard);
    drop(path_guard);
    drop(cache_guard);
    drop(def_guard);
    
    *state.current_tune_path.lock().await = Some(save_path.clone());
    *state.tune_modified.lock().await = false;
    
    Ok(save_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn save_tune_as(state: tauri::State<'_, AppState>, path: String) -> Result<String, String> {
    save_tune(state, Some(path)).await
}

#[tauri::command]
async fn load_tune(state: tauri::State<'_, AppState>, app: tauri::AppHandle, path: String) -> Result<TuneInfo, String> {
    eprintln!("\n[INFO] ========================================");
    eprintln!("[INFO] LOADING TUNE FILE: {}", path);
    eprintln!("[INFO] ========================================");
    
    let tune = TuneFile::load(&path)
        .map_err(|e| format!("Failed to load tune: {}", e))?;
    
    eprintln!("[INFO] ✓ Tune file loaded successfully");
    eprintln!("[INFO]   Signature: '{}'", tune.signature);
    eprintln!("[INFO]   Constants: {}", tune.constants.len());
    eprintln!("[INFO]   Pages: {}", tune.pages.len());
    
    // Debug: List first 20 constant names to see what we parsed
    let constant_names: Vec<String> = tune.constants.keys().take(20).cloned().collect();
    eprintln!("[DEBUG] load_tune: Sample constants from MSQ: {:?}", constant_names);
    
    // Debug: Check VE table constants specifically
    let ve_table_in_tune = tune.constants.contains_key("veTable");
    let ve_rpm_bins_in_tune = tune.constants.contains_key("veRpmBins");
    let ve_load_bins_in_tune = tune.constants.contains_key("veLoadBins");
    eprintln!("[DEBUG] load_tune: VE constants in tune - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
        ve_table_in_tune, ve_rpm_bins_in_tune, ve_load_bins_in_tune);
    
    // Check if MSQ signature matches current INI definition (informational only)
    // We'll still apply constants by name match regardless of signature match
    let def_guard = state.definition.lock().await;
    let current_ini_signature = def_guard.as_ref().map(|d| d.signature.clone());
    drop(def_guard);
    
    if let Some(ref ini_sig) = current_ini_signature {
        let match_type = compare_signatures(&tune.signature, ini_sig);
        if match_type != SignatureMatchType::Exact {
            eprintln!("[INFO] load_tune: MSQ signature '{}' {} current INI signature '{}' - will apply constants by name match", 
                tune.signature,
                if match_type == SignatureMatchType::Partial { "partially matches" } else { "does not match" },
                ini_sig);
            eprintln!("[INFO] load_tune: This is normal - many constants (like VE table, ignition tables) will still work across different INI versions");
            
            // Only show dialog for complete mismatches, and only if we find better matching INIs
            if match_type == SignatureMatchType::Mismatch {
                let matching_inis = find_matching_inis_internal(&state, &tune.signature).await;
                let matching_count = matching_inis.len();
                
                // Only show dialog if we found better matching INIs
                if matching_count > 0 {
                    let current_ini_path = {
                        let settings = load_settings(&app);
                        settings.last_ini_path.clone()
                    };
                    
                    let mismatch_info = SignatureMismatchInfo {
                        ecu_signature: tune.signature.clone(),
                        ini_signature: ini_sig.clone(),
                        match_type,
                        current_ini_path,
                        matching_inis,
                    };
                    
                    let _ = app.emit("signature:mismatch", &mismatch_info);
                    eprintln!("[INFO] load_tune: Found {} better matching INI file(s). You can switch in the dialog, or continue with current INI.", matching_count);
                }
            }
        } else {
            eprintln!("[INFO] load_tune: MSQ signature matches current INI definition");
        }
    } else {
        eprintln!("[WARN] load_tune: No INI definition loaded - will apply constants by name match if definition is loaded later");
    }
    
    let info = TuneInfo {
        path: Some(path.clone()),
        signature: tune.signature.clone(),
        modified: false,
        has_tune: true,
    };
    
    // Populate TuneCache from loaded tune data
    // This allows table operations to use cached data instead of reading from ECU
    {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref();
        let mut cache_guard = state.tune_cache.lock().await;
        
        // Initialize cache if it doesn't exist, or reinitialize if it was reset
        if cache_guard.is_none() {
            if let Some(def) = def {
                eprintln!("[DEBUG] load_tune: Initializing cache from definition");
                *cache_guard = Some(TuneCache::from_definition(def));
            } else {
                eprintln!("[WARN] load_tune: No definition loaded, cannot initialize cache");
                return Err("No ECU definition loaded. Please open a project first.".to_string());
            }
        }
        
        // Ensure cache is initialized even if it exists but is empty
        if let Some(cache) = cache_guard.as_mut() {
            if cache.page_count() == 0 {
                if let Some(def) = def {
                    eprintln!("[DEBUG] load_tune: Cache exists but is empty, reinitializing from definition");
                    *cache_guard = Some(TuneCache::from_definition(def));
                }
            }
        }
        
        if let Some(cache) = cache_guard.as_mut() {
            // First, load any raw page data
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
                eprintln!("[DEBUG] load_tune: populated cache page {} with {} bytes", page_num, page_data.len());
            }
            
            // Then, apply constants from tune file to cache
            if let Some(def) = def {
                eprintln!("[DEBUG] load_tune: Definition loaded - {} constants in definition", def.constants.len());
                
                // Debug: Check if VE table constants are in the definition
                let ve_table_in_def = def.constants.contains_key("veTable");
                let ve_rpm_bins_in_def = def.constants.contains_key("veRpmBins");
                let ve_load_bins_in_def = def.constants.contains_key("veLoadBins");
                eprintln!("[DEBUG] load_tune: VE constants in definition - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                    ve_table_in_def, ve_rpm_bins_in_def, ve_load_bins_in_def);
                
                // Debug: Show what veTable constant looks like if it exists
                if let Some(ve_const) = def.constants.get("veTable") {
                    eprintln!("[DEBUG] load_tune: veTable constant - page={}, offset={}, size={}, shape={:?}", 
                        ve_const.page, ve_const.offset, ve_const.size_bytes(), ve_const.shape);
                }
                
                use libretune_core::tune::TuneValue;
                
                let mut applied_count = 0;
                let mut skipped_count = 0;
                let mut failed_count = 0;
                let mut pcvar_count = 0;
                let mut zero_size_count = 0;
                let mut string_bool_count = 0;
                
                for (name, tune_value) in &tune.constants {
                    // Debug VE table constants
                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                        eprintln!("[DEBUG] load_tune: Found VE constant '{}' in MSQ file", name);
                    }
                    
                    // Look up constant in definition
                    if let Some(constant) = def.constants.get(name) {
                        // PC variables are stored locally, not in page data
                        if constant.is_pc_variable {
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    cache.local_values.insert(name.clone(), *v);
                                    pcvar_count += 1;
                                    eprintln!("[DEBUG] load_tune: set PC variable '{}' = {}", name, v);
                                }
                                TuneValue::Array(arr) if !arr.is_empty() => {
                                    // For arrays, store first value (or handle differently if needed)
                                    cache.local_values.insert(name.clone(), arr[0]);
                                    pcvar_count += 1;
                                    eprintln!("[DEBUG] load_tune: set PC variable '{}' = {} (from array)", name, arr[0]);
                                }
                                _ => {
                                    skipped_count += 1;
                                    eprintln!("[DEBUG] load_tune: skipping PC variable '{}' (unsupported value type)", name);
                                }
                            }
                            continue;
                        }
                        
                        // Handle bits constants specially (they're packed, size_bytes() == 0)
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            // Bits constants: read current byte(s), modify the bits, write back
                            let bit_pos = constant.bit_position.unwrap_or(0);
                            let bit_size = constant.bit_size.unwrap_or(1);
                            
                            // Calculate which byte(s) contain the bits
                            let byte_offset = (bit_pos / 8) as u16;
                            let bit_in_byte = bit_pos % 8;
                            
                            // Calculate how many bytes we need
                            let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
                            let bytes_needed = if bits_remaining_after_first_byte > 0 {
                                1 + ((bits_remaining_after_first_byte + 7) / 8)
                            } else {
                                1
                            };
                            let bytes_needed_usize = bytes_needed as usize;
                            
                            // Read current byte(s) value (or 0 if not present)
                            let read_offset = constant.offset + byte_offset;
                            let mut current_bytes: Vec<u8> = cache.read_bytes(constant.page, read_offset, bytes_needed as u16)
                                .map(|s| s.to_vec())
                                .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);
                            
                            // Ensure we have enough bytes
                            while current_bytes.len() < bytes_needed_usize {
                                current_bytes.push(0u8);
                            }
                            
                            // Get the bit value from MSQ (index into bit_options)
                            // MSQ can store bits constants as numeric indices, option strings, or booleans
                            let bit_value = match tune_value {
                                TuneValue::Scalar(v) => *v as u32,
                                TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                                TuneValue::Bool(b) => {
                                    // Boolean values: true = 1, false = 0
                                    // For bits constants with 2 options (like ["false", "true"]), 
                                    // boolean true maps to index 1, false to index 0
                                    if *b { 1 } else { 0 }
                                }
                                TuneValue::String(s) => {
                                    // Look up the string in bit_options to find its index
                                    if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                                        index as u32
                                    } else {
                                        // Try case-insensitive match
                                        if let Some(index) = constant.bit_options.iter().position(|opt| opt.eq_ignore_ascii_case(s)) {
                                            index as u32
                                        } else {
                                            skipped_count += 1;
                                            eprintln!("[DEBUG] load_tune: skipping bits constant '{}' (string '{}' not found in bit_options: {:?})", name, s, constant.bit_options);
                                            continue;
                                        }
                                    }
                                }
                                _ => {
                                    skipped_count += 1;
                                    eprintln!("[DEBUG] load_tune: skipping bits constant '{}' (unsupported value type)", name);
                                    continue;
                                }
                            };
                            
                            // Modify the first byte
                            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                            let mask_first = if bits_in_first_byte >= 8 {
                                0xFF
                            } else {
                                (1u8 << bits_in_first_byte) - 1
                            };
                            let value_first = (bit_value & mask_first as u32) as u8;
                            current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte)) | (value_first << bit_in_byte);
                            
                            // If bits span multiple bytes, modify additional bytes
                            if bits_remaining_after_first_byte > 0 {
                                let mut bits_collected = bits_in_first_byte;
                                for i in 1..bytes_needed_usize.min(current_bytes.len()) {
                                    let remaining_bits = bit_size - bits_collected;
                                    if remaining_bits == 0 {
                                        break;
                                    }
                                    let bits_from_this_byte = remaining_bits.min(8);
                                    let mask = if bits_from_this_byte >= 8 {
                                        0xFF
                                    } else {
                                        (1u8 << bits_from_this_byte) - 1
                                    };
                                    let value_from_bit = ((bit_value >> bits_collected) & mask as u32) as u8;
                                    current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                    bits_collected += bits_from_this_byte;
                                }
                            }
                            
                            // Write the modified byte(s) back
                            if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                                applied_count += 1;
                                eprintln!("[DEBUG] load_tune: ✓ Applied bits constant '{}' = {} (bit_pos={}, bit_size={}, bytes={})", 
                                    name, bit_value, bit_pos, bit_size, bytes_needed);
                            } else {
                                failed_count += 1;
                                eprintln!("[DEBUG] load_tune: ✗ Failed to write bits constant '{}'", name);
                            }
                            continue;
                        }
                        
                        // Skip if constant has no size (shouldn't happen for non-bits)
                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            zero_size_count += 1;
                            skipped_count += 1;
                            eprintln!("[DEBUG] load_tune: skipping constant '{}' (zero size)", name);
                            continue;
                        }
                        
                        // Convert tune value to raw bytes
                        let element_size = constant.data_type.size_bytes();
                        let element_count = constant.shape.element_count();
                        let mut raw_data = vec![0u8; length as usize];
                        
                        match tune_value {
                            TuneValue::Scalar(v) => {
                                let raw_val = constant.display_to_raw(*v);
                                constant.data_type.write_to_bytes(&mut raw_data, 0, raw_val, def.endianness);
                                // Check if page exists before writing
                                let page_exists = cache.page_size(constant.page).is_some();
                                let page_state_before = cache.page_state(constant.page);
                                
                                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                    eprintln!("[DEBUG] load_tune: About to write '{}' - page={}, page_exists={}, page_state={:?}, offset={}, len={}", 
                                        name, constant.page, page_exists, page_state_before, constant.offset, length);
                                }
                                
                                if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                    let page_state_after = cache.page_state(constant.page);
                                    
                                    // Verify the data was actually written by reading it back
                                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                        let verify_read = cache.read_bytes(constant.page, constant.offset, length);
                                        eprintln!("[DEBUG] load_tune: ✓ Applied constant '{}' = {} (scalar, page={}, offset={}, state={:?}, verify_read={})", 
                                            name, v, constant.page, constant.offset, page_state_after, verify_read.is_some());
                                    }
                                } else {
                                    failed_count += 1;
                                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                        eprintln!("[DEBUG] load_tune: ✗ Failed to write constant '{}' (scalar, page={}, offset={}, len={}, page_size={:?}, page_exists={})", 
                                            name, constant.page, constant.offset, length, cache.page_size(constant.page), page_exists);
                                    }
                                }
                            }
                            TuneValue::Array(arr) => {
                                // Handle size mismatches: write what we have, pad or truncate as needed
                                let write_count = arr.len().min(element_count);
                                let last_value = arr.last().copied().unwrap_or(0.0);
                                
                                for i in 0..element_count {
                                    let val = if i < arr.len() {
                                        arr[i]
                                    } else {
                                        // Pad with last value if array is smaller
                                        last_value
                                    };
                                    let raw_val = constant.display_to_raw(val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
                                }
                                
                                // Check if page exists before writing
                                let page_exists = cache.page_size(constant.page).is_some();
                                let page_state_before = cache.page_state(constant.page);
                                
                                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                    if arr.len() != element_count {
                                        eprintln!("[DEBUG] load_tune: array size mismatch for '{}': expected {}, got {} (will pad/truncate)", 
                                            name, element_count, arr.len());
                                    }
                                    eprintln!("[DEBUG] load_tune: About to write '{}' - page={}, page_exists={}, page_state={:?}, offset={}, len={}", 
                                        name, constant.page, page_exists, page_state_before, constant.offset, length);
                                }
                                
                                if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                    let page_state_after = cache.page_state(constant.page);
                                    
                                    // Verify the data was actually written by reading it back
                                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                        let verify_read = cache.read_bytes(constant.page, constant.offset, length);
                                        eprintln!("[DEBUG] load_tune: ✓ Applied constant '{}' (array, {} elements written, {} expected, page={}, offset={}, state={:?}, verify_read={})", 
                                            name, write_count, element_count, constant.page, constant.offset, page_state_after, verify_read.is_some());
                                    }
                                } else {
                                    failed_count += 1;
                                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                        eprintln!("[DEBUG] load_tune: ✗ Failed to write constant '{}' (array, page={}, offset={}, len={}, page_size={:?}, page_exists={})", 
                                            name, constant.page, constant.offset, length, cache.page_size(constant.page), page_exists);
                                    }
                                }
                            }
                            TuneValue::String(_) | TuneValue::Bool(_) => {
                                string_bool_count += 1;
                                skipped_count += 1;
                                eprintln!("[DEBUG] load_tune: skipping constant '{}' (string/bool not supported for page data)", name);
                            }
                        }
                    } else {
                        skipped_count += 1;
                        if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                            eprintln!("[DEBUG] load_tune: constant '{}' not found in definition", name);
                        }
                    }
                }
                
                // Print prominent summary
                let total_accounted = applied_count + pcvar_count + skipped_count + failed_count;
                eprintln!("\n[INFO] ========================================");
                eprintln!("[INFO] Tune Load Summary:");
                eprintln!("[INFO]   Total constants in MSQ: {}", tune.constants.len());
                eprintln!("[INFO]   Successfully applied (page data): {}", applied_count);
                eprintln!("[INFO]   PC variables applied: {}", pcvar_count);
                eprintln!("[INFO]   Failed to apply: {}", failed_count);
                eprintln!("[INFO]   Skipped:");
                eprintln!("[INFO]     - Not in definition: {}", skipped_count - zero_size_count - string_bool_count);
                eprintln!("[INFO]     - Zero size (packed bits): {}", zero_size_count);
                eprintln!("[INFO]     - String/Bool (unsupported): {}", string_bool_count);
                eprintln!("[INFO]   Total skipped: {}", skipped_count);
                if total_accounted != tune.constants.len() {
                    eprintln!("[WARN]   ⚠ Accounting mismatch: {} constants unaccounted for!", 
                        tune.constants.len() - total_accounted);
                }
                eprintln!("[INFO] ========================================\n");
                
                // Debug: Check page states after loading and show actual data sizes
                eprintln!("[DEBUG] load_tune: Page states after loading:");
                for page in 0..cache.page_count() {
                    let state = cache.page_state(page);
                    let def_size = cache.page_size(page);
                    let actual_size = cache.get_page(page).map(|p| p.len()).unwrap_or(0);
                    if state != PageState::NotLoaded || def_size.is_some() || actual_size > 0 {
                        eprintln!("[DEBUG] load_tune:   Page {}: state={:?}, def_size={:?}, actual_data_size={} bytes", 
                            page, state, def_size, actual_size);
                    }
                }
                
                if applied_count > 0 {
                    let total_applied = applied_count + pcvar_count;
                    eprintln!("[INFO] ✓ Successfully loaded {} constants into cache ({} page data + {} PC variables).", 
                        total_applied, applied_count, pcvar_count);
                    eprintln!("[INFO]   Important tables like VE, ignition, and fuel should work even if some constants don't match.");
                    eprintln!("[INFO]   All open tables will refresh automatically.");
                    
                    // Informational note if many constants were skipped (not a warning - this is normal)
                    if skipped_count > applied_count && skipped_count > 100 {
                        let applied_percent = (total_applied as f64 / tune.constants.len() as f64 * 100.0) as u32;
                        eprintln!("[INFO] ℹ Note: {} constants ({}%) were skipped - they're not in the current INI definition.", skipped_count, 100 - applied_percent);
                        eprintln!("[INFO]   This is normal when INI versions differ. Core tuning tables should still work.");
                        eprintln!("[INFO]   If you need those constants, switch to a matching INI file in Settings.");
                    }
                } else {
                    eprintln!("[WARN] ⚠ No constants were applied! This usually means the MSQ file doesn't match the current INI definition.");
                    eprintln!("[WARN]   MSQ signature: '{}'", tune.signature);
                    eprintln!("[WARN]   Check the Signature Mismatch dialog (if shown) or switch to a matching INI file in Settings.");
                }
            } else {
                eprintln!("[DEBUG] load_tune: no definition loaded, skipping constant application");
            }
        }
    }
    
    *state.current_tune.lock().await = Some(tune.clone());
    *state.current_tune_path.lock().await = Some(PathBuf::from(path));
    *state.tune_modified.lock().await = false;
    
    // If a project is open, save the tune to the project's CurrentTune.msq
    // This ensures it will be auto-loaded next time the project is opened
    let proj_guard = state.current_project.lock().await;
    if let Some(ref project) = *proj_guard {
        let project_tune_path = project.path.join("CurrentTune.msq");
        if let Err(e) = tune.save(&project_tune_path) {
            eprintln!("[WARN] Failed to save tune to project folder: {}", e);
        } else {
            eprintln!("[INFO] ✓ Saved tune to project: {:?}", project_tune_path);
            // Update the stored tune path to point to the project's tune file
            *state.current_tune_path.lock().await = Some(project_tune_path);
        }
    }
    drop(proj_guard);
    
    // Emit event to notify UI that tune was loaded
    let _ = app.emit("tune:loaded", "file");
    
    Ok(info)
}

#[tauri::command]
async fn list_tune_files() -> Result<Vec<String>, String> {
    let projects_dir = libretune_core::project::Project::projects_dir()
        .map_err(|e| format!("Failed to get projects directory: {}", e))?;
    
    // Ensure directory exists
    std::fs::create_dir_all(&projects_dir)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;
    
    let mut tunes = Vec::new();
    
    let entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;
    
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".msq") || name.ends_with(".json") {
                tunes.push(entry.path().to_string_lossy().to_string());
            }
        }
    }
    
    tunes.sort();
    Ok(tunes)
}

#[tauri::command]
async fn burn_to_ecu(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    
    // Send burn command to ECU
    // The 'b' command tells the ECU to write RAM to flash
    conn.send_burn_command()
        .map_err(|e| format!("Burn failed: {}", e))?;
    
    Ok(())
}

#[tauri::command]
async fn mark_tune_modified(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.tune_modified.lock().await = true;
    Ok(())
}

/// Compare the current project tune with the tune synced from ECU
/// Returns true if they differ, false if identical
#[tauri::command]
async fn compare_project_and_ecu_tunes(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let tune_guard = state.current_tune.lock().await;
    let project_guard = state.current_project.lock().await;
    
    // Get ECU tune (synced from ECU, currently in current_tune)
    let ecu_tune = match tune_guard.as_ref() {
        Some(t) => t,
        None => return Ok(false), // No ECU tune, can't compare
    };
    
    // Get project tune path and load it
    let project_tune = if let Some(ref project) = *project_guard {
        let tune_path = project.current_tune_path();
        if tune_path.exists() {
            match TuneFile::load(&tune_path) {
                Ok(tune) => Some(tune),
                Err(e) => {
                    eprintln!("[WARN] Failed to load project tune for comparison: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    
    // If no project tune, they're different (ECU has data, project doesn't)
    let project_tune = match project_tune {
        Some(t) => t,
        None => return Ok(true), // Different - project has no tune
    };
    
    // Compare page data
    // Get all unique page numbers
    let mut all_pages: Vec<u8> = project_tune.pages.keys()
        .chain(ecu_tune.pages.keys())
        .copied()
        .collect();
    all_pages.sort();
    all_pages.dedup();
    
    // Compare each page
    for page_num in all_pages {
        let project_page = project_tune.pages.get(&page_num);
        let ecu_page = ecu_tune.pages.get(&page_num);
        
        match (project_page, ecu_page) {
            (None, None) => continue, // Both missing, skip
            (Some(_), None) | (None, Some(_)) => return Ok(true), // One missing, different
            (Some(p), Some(e)) => {
                if p != e {
                    return Ok(true); // Pages differ
                }
            }
        }
    }
    
    // All pages match
    Ok(false)
}

/// Write the project tune to ECU
/// Loads the tune from the project's CurrentTune.msq and writes all pages to ECU
#[tauri::command]
async fn write_project_tune_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let def_guard = state.definition.lock().await;
    
    let project = project_guard.as_ref().ok_or("No project open")?;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    // Load project tune
    let tune_path = project.current_tune_path();
    let tune = TuneFile::load(&tune_path)
        .map_err(|e| format!("Failed to load project tune: {}", e))?;
    
    drop(project_guard);
    drop(def_guard);
    
    // Write all pages to ECU
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    
    // Sort pages for consistent writing
    let mut pages: Vec<(u8, &Vec<u8>)> = tune.pages.iter().map(|(k, v)| (*k, v)).collect();
    pages.sort_by_key(|(p, _)| *p);
    
    for (page_num, page_data) in pages {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: page_num,
            offset: 0,
            data: page_data.clone(),
        };
        conn.write_memory(params).map_err(|e| format!("Failed to write page {}: {}", page_num, e))?;
    }
    
    // Update cache and current_tune with project tune
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }
        }
    }
    
    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = Some(tune);
    
    // Update path to project tune file
    *state.current_tune_path.lock().await = Some(tune_path);
    
    // Mark as not modified (freshly loaded from project)
    *state.tune_modified.lock().await = false;
    
    Ok(())
}

/// Save the current tune to the project's tune file
#[tauri::command]
async fn save_tune_to_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let tune_guard = state.current_tune.lock().await;
    
    let project = project_guard.as_ref().ok_or("No project open")?;
    let tune = tune_guard.as_ref().ok_or("No tune loaded")?.clone();
    
    let tune_path = project.current_tune_path();
    
    drop(project_guard);
    drop(tune_guard);
    
    // Save tune to project path
    tune.save(&tune_path)
        .map_err(|e| format!("Failed to save tune to project: {}", e))?;
    
    // Update path
    *state.current_tune_path.lock().await = Some(tune_path);
    
    // Mark as not modified
    *state.tune_modified.lock().await = false;
    
    Ok(())
}

// =============================================================================
// Data Logging Commands
// =============================================================================

#[derive(Serialize)]
struct LoggingStatus {
    is_recording: bool,
    entry_count: usize,
    duration_ms: u64,
    channels: Vec<String>,
}

#[tauri::command]
async fn start_logging(state: tauri::State<'_, AppState>, sample_rate: Option<f64>) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    
    // Get channel names from output channels
    let channels: Vec<String> = def.output_channels.keys().cloned().collect();
    
    let mut logger = state.data_logger.lock().await;
    *logger = DataLogger::new(channels);
    if let Some(rate) = sample_rate {
        logger.set_sample_rate(rate);
    }
    logger.start();
    
    Ok(())
}

#[tauri::command]
async fn stop_logging(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.stop();
    Ok(())
}

#[tauri::command]
async fn get_logging_status(state: tauri::State<'_, AppState>) -> Result<LoggingStatus, String> {
    let logger = state.data_logger.lock().await;
    Ok(LoggingStatus {
        is_recording: logger.is_recording(),
        entry_count: logger.entry_count(),
        duration_ms: logger.duration().as_millis() as u64,
        channels: logger.channels().to_vec(),
    })
}

#[derive(Serialize)]
struct LogEntryData {
    timestamp_ms: u64,
    values: HashMap<String, f64>,
}

#[tauri::command]
async fn get_log_entries(
    state: tauri::State<'_, AppState>,
    start_index: Option<usize>,
    count: Option<usize>,
) -> Result<Vec<LogEntryData>, String> {
    let logger = state.data_logger.lock().await;
    let channels = logger.channels();
    
    let start = start_index.unwrap_or(0);
    let max_count = count.unwrap_or(1000);
    
    let entries: Vec<LogEntryData> = logger.entries()
        .skip(start)
        .take(max_count)
        .map(|entry| {
            let mut values = HashMap::new();
            for (i, channel) in channels.iter().enumerate() {
                if let Some(&val) = entry.values.get(i) {
                    values.insert(channel.clone(), val);
                }
            }
            LogEntryData {
                timestamp_ms: entry.timestamp.as_millis() as u64,
                values,
            }
        })
        .collect();
    
    Ok(entries)
}

#[tauri::command]
async fn clear_log(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.clear();
    Ok(())
}

#[tauri::command]
async fn save_log(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let logger = state.data_logger.lock().await;
    
    // Create CSV content
    let mut csv = String::new();
    
    // Header row
    csv.push_str("Time (ms)");
    for channel in logger.channels() {
        csv.push(',');
        csv.push_str(channel);
    }
    csv.push('\n');
    
    // Data rows
    for entry in logger.entries() {
        csv.push_str(&format!("{}", entry.timestamp.as_millis()));
        for val in &entry.values {
            csv.push(',');
            csv.push_str(&format!("{:.4}", val));
        }
        csv.push('\n');
    }
    
    std::fs::write(&path, csv)
        .map_err(|e| format!("Failed to save log: {}", e))?;
    
    Ok(())
}

// =====================================================
// Project Management Commands
// =====================================================

#[derive(Serialize)]
struct ProjectInfoResponse {
    name: String,
    path: String,
    signature: String,
    modified: String,
}

#[derive(Serialize)]
struct IniEntryResponse {
    id: String,
    name: String,
    signature: String,
    path: String,
}

#[derive(Serialize)]
struct CurrentProjectInfo {
    name: String,
    path: String,
    signature: String,
    has_tune: bool,
    tune_modified: bool,
    connection: ConnectionSettingsResponse,
}

#[derive(Serialize)]
struct ConnectionSettingsResponse {
    port: Option<String>,
    baud_rate: u32,
}

/// Get the path to the projects directory
#[tauri::command]
async fn get_projects_path() -> Result<String, String> {
    let path = Project::projects_dir()
        .map_err(|e| format!("Failed to get projects directory: {}", e))?;
    
    // Create if doesn't exist
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;
    
    Ok(path.to_string_lossy().to_string())
}

/// List all available projects
#[tauri::command]
async fn list_projects() -> Result<Vec<ProjectInfoResponse>, String> {
    let projects = Project::list_projects()
        .map_err(|e| format!("Failed to list projects: {}", e))?;
    
    Ok(projects.into_iter().map(|p| ProjectInfoResponse {
        name: p.name,
        path: p.path.to_string_lossy().to_string(),
        signature: p.signature,
        modified: p.modified,
    }).collect())
}

/// Create a new project
#[tauri::command]
async fn create_project(
    state: tauri::State<'_, AppState>,
    name: String,
    ini_id: String,
    tune_path: Option<String>,
) -> Result<CurrentProjectInfo, String> {
    // Get INI path from repository
    let mut repo_guard = state.ini_repository.lock().await;
    let repo = repo_guard.as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;
    
    let ini_path = repo.get_path(&ini_id)
        .ok_or_else(|| format!("INI '{}' not found in repository", ini_id))?;
    
    // Get signature from INI
    let def = EcuDefinition::from_file(&ini_path)
        .map_err(|e| format!("Failed to parse INI: {}", e))?;
    let signature = def.signature.clone();
    
    // Create the project with optional imported tune
    let mut project = Project::create(&name, &ini_path, &signature, None)
        .map_err(|e| format!("Failed to create project: {}", e))?;
    
    // Store current project and load its definition first (needed for applying tune)
    let mut def_guard = state.definition.lock().await;
    *def_guard = Some(def.clone());
    drop(def_guard);
    
    // Initialize TuneCache from definition
    let cache = TuneCache::from_definition(&def);
    {
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);
    }
    
    // If a tune path was provided, import it and apply to cache
    if let Some(tune_file) = tune_path {
        let tune_path_ref = std::path::Path::new(&tune_file);
        if tune_path_ref.exists() {
            // TuneFile::load handles both XML and MSQ formats automatically
            let tune = TuneFile::load(tune_path_ref)
                .map_err(|e| format!("Failed to load tune: {}", e))?;
            
            // Apply tune constants to cache (same logic as load_tune)
            {
                let mut cache_guard = state.tune_cache.lock().await;
                if let Some(cache) = cache_guard.as_mut() {
                    // Load any raw page data
                    for (page_num, page_data) in &tune.pages {
                        cache.load_page(*page_num, page_data.clone());
                    }
                    
                    // Apply constants from tune file to cache
                    use libretune_core::tune::TuneValue;
                    
                    for (name, tune_value) in &tune.constants {
                        if let Some(constant) = def.constants.get(name) {
                            // PC variables are stored locally
                            if constant.is_pc_variable {
                                match tune_value {
                                    TuneValue::Scalar(v) => {
                                        cache.local_values.insert(name.clone(), *v);
                                    }
                                    TuneValue::Array(arr) if !arr.is_empty() => {
                                        cache.local_values.insert(name.clone(), arr[0]);
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            
                            let length = constant.size_bytes() as u16;
                            if length == 0 {
                                continue;
                            }
                            
                            let element_size = constant.data_type.size_bytes();
                            let element_count = constant.shape.element_count();
                            let mut raw_data = vec![0u8; length as usize];
                            
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    let raw_val = constant.display_to_raw(*v);
                                    constant.data_type.write_to_bytes(&mut raw_data, 0, raw_val, def.endianness);
                                    let _ = cache.write_bytes(constant.page, constant.offset, &raw_data);
                                }
                                TuneValue::Array(arr) if arr.len() == element_count => {
                                    for (i, val) in arr.iter().enumerate() {
                                        let raw_val = constant.display_to_raw(*val);
                                        let offset = i * element_size;
                                        constant.data_type.write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
                                    }
                                    let _ = cache.write_bytes(constant.page, constant.offset, &raw_data);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            
            // Store tune in project
            project.current_tune = Some(tune);
            project.save_current_tune()
                .map_err(|e| format!("Failed to save imported tune: {}", e))?;
        }
    }
    
    let response = CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified: project.dirty,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
        },
    };
    
    let mut proj_guard = state.current_project.lock().await;
    *proj_guard = Some(project);
    
    Ok(response)
}

/// Open an existing project
#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<CurrentProjectInfo, String> {
    eprintln!("\n[INFO] ========================================");
    eprintln!("[INFO] OPENING PROJECT: {}", path);
    eprintln!("[INFO] ========================================");
    
    let project = Project::open(&path)
        .map_err(|e| format!("Failed to open project: {}", e))?;
    
    eprintln!("[INFO] Project opened: {}", project.config.name);
    eprintln!("[INFO] Project has tune file: {}", project.current_tune.is_some());
    
    if let Some(ref tune) = project.current_tune {
        eprintln!("[INFO] Tune file signature: '{}'", tune.signature);
        eprintln!("[INFO] Tune file has {} constants", tune.constants.len());
        eprintln!("[INFO] Tune file has {} pages", tune.pages.len());
    } else {
        let tune_path = project.current_tune_path();
        eprintln!("[WARN] No tune file loaded. Expected at: {:?}", tune_path);
        eprintln!("[WARN] Tune file exists: {}", tune_path.exists());
    }
    
    // Load the project's INI definition
    let ini_path = project.ini_path();
    eprintln!("[INFO] Loading INI from: {:?}", ini_path);
    let def = EcuDefinition::from_file(&ini_path)
        .map_err(|e| format!("Failed to parse project INI: {}", e))?;
    
    eprintln!("[INFO] INI signature: '{}'", def.signature);
    eprintln!("[INFO] INI has {} constants", def.constants.len());
    
    let response = CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified: project.dirty,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
        },
    };
    
    // Disconnect any existing connection when opening a new project
    // to avoid stale connection state from previous ECU
    let mut conn_guard = state.connection.lock().await;
    *conn_guard = None;
    drop(conn_guard);
    
    // Store current project and definition
    let mut def_guard = state.definition.lock().await;
    let def_clone = def.clone();
    *def_guard = Some(def);
    drop(def_guard);
    
    // Save project path before moving project into mutex
    let project_path = project.path.clone();
    let project_tune = project.current_tune.as_ref().cloned();
    
    // Load project tune if it exists
    let mut proj_guard = state.current_project.lock().await;
    *proj_guard = Some(project);
    drop(proj_guard);
    
    // Always try to load CurrentTune.msq if it exists, even if project.current_tune wasn't set
    let tune_to_load = if let Some(tune) = project_tune {
        Some(tune)
    } else {
        // Try to load tune file directly if it wasn't auto-loaded
        let tune_path = project_path.join("CurrentTune.msq");
        if tune_path.exists() {
            eprintln!("[INFO] Auto-loading tune file: {:?}", tune_path);
            match TuneFile::load(&tune_path) {
                Ok(tune) => {
                    eprintln!("[INFO] ✓ Successfully loaded tune file with {} constants", tune.constants.len());
                    Some(tune)
                }
                Err(e) => {
                    eprintln!("[WARN] Failed to load tune file: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };
    
    // Initialize TuneCache and load project tune
    if let Some(tune) = tune_to_load {
        // Create TuneCache from definition
        let cache = TuneCache::from_definition(&def_clone);
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);
        
        // Populate cache from project tune
        if let Some(cache) = cache_guard.as_mut() {
            // Load any raw page data first
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }
            
            // Apply constants from tune file to cache (same logic as load_tune)
            use libretune_core::tune::TuneValue;
            
            // Debug: Check if VE table constants are in the tune
            let ve_table_in_tune = tune.constants.contains_key("veTable");
            let ve_rpm_bins_in_tune = tune.constants.contains_key("veRpmBins");
            let ve_load_bins_in_tune = tune.constants.contains_key("veLoadBins");
            eprintln!("[DEBUG] open_project: VE constants in tune - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                ve_table_in_tune, ve_rpm_bins_in_tune, ve_load_bins_in_tune);
            
            // Debug: Check if VE table constants are in the definition
            let ve_table_in_def = def_clone.constants.contains_key("veTable");
            let ve_rpm_bins_in_def = def_clone.constants.contains_key("veRpmBins");
            let ve_load_bins_in_def = def_clone.constants.contains_key("veLoadBins");
            eprintln!("[DEBUG] open_project: VE constants in definition - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                ve_table_in_def, ve_rpm_bins_in_def, ve_load_bins_in_def);
            
            // Debug: Show sample constant names from MSQ and definition to see why they're not matching
            let msq_sample: Vec<String> = tune.constants.keys().take(10).cloned().collect();
            let def_sample: Vec<String> = def_clone.constants.keys().take(10).cloned().collect();
            eprintln!("[DEBUG] open_project: Sample MSQ constants: {:?}", msq_sample);
            eprintln!("[DEBUG] open_project: Sample definition constants: {:?}", def_sample);
            eprintln!("[DEBUG] open_project: Total MSQ constants: {}, Total definition constants: {}", 
                tune.constants.len(), def_clone.constants.len());
            
            let mut applied_count = 0;
            let mut skipped_count = 0;
            let mut failed_count = 0;
            
            for (name, tune_value) in &tune.constants {
                // Debug VE table constants specifically
                let is_ve_related = name == "veTable" || name == "veRpmBins" || name == "veLoadBins";
                
                if let Some(constant) = def_clone.constants.get(name) {
                    if is_ve_related {
                        eprintln!("[DEBUG] open_project: Found constant '{}' in definition (page={}, offset={}, size={})", 
                            name, constant.page, constant.offset, constant.size_bytes());
                    }
                    
                    // PC variables are stored locally
                    if constant.is_pc_variable {
                        match tune_value {
                            TuneValue::Scalar(v) => {
                                cache.local_values.insert(name.clone(), *v);
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied PC variable '{}' = {}", name, v);
                                }
                            }
                            TuneValue::Array(arr) if !arr.is_empty() => {
                                cache.local_values.insert(name.clone(), arr[0]);
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied PC variable '{}' = {} (from array)", name, arr[0]);
                                }
                            }
                            _ => {
                                skipped_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Skipped PC variable '{}' (unsupported value type)", name);
                                }
                            }
                        }
                        continue;
                    }
                    
                    // Handle bits constants specially (they're packed, size_bytes() == 0)
                    if constant.data_type == libretune_core::ini::DataType::Bits {
                        // Bits constants: read current byte(s), modify the bits, write back
                        let bit_pos = constant.bit_position.unwrap_or(0);
                        let bit_size = constant.bit_size.unwrap_or(1);
                        
                        // Calculate which byte(s) contain the bits
                        let byte_offset = (bit_pos / 8) as u16;
                        let bit_in_byte = bit_pos % 8;
                        
                        // Calculate how many bytes we need
                        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
                        let bytes_needed = if bits_remaining_after_first_byte > 0 {
                            1 + ((bits_remaining_after_first_byte + 7) / 8)
                        } else {
                            1
                        };
                        let bytes_needed_usize = bytes_needed as usize;
                        
                        // Read current byte(s) value (or 0 if not present)
                        let read_offset = constant.offset + byte_offset;
                        let mut current_bytes: Vec<u8> = cache.read_bytes(constant.page, read_offset, bytes_needed as u16)
                            .map(|s| s.to_vec())
                            .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);
                        
                        // Ensure we have enough bytes
                        while current_bytes.len() < bytes_needed_usize {
                            current_bytes.push(0u8);
                        }
                        
                        // Get the bit value from MSQ (index into bit_options)
                        // MSQ can store bits constants as numeric indices or as option strings
                        let bit_value = match tune_value {
                            TuneValue::Scalar(v) => *v as u32,
                            TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                            TuneValue::String(s) => {
                                // Look up the string in bit_options to find its index
                                if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                                    index as u32
                                } else {
                                    // Try case-insensitive match
                                    if let Some(index) = constant.bit_options.iter().position(|opt| opt.eq_ignore_ascii_case(s)) {
                                        index as u32
                                    } else {
                                        skipped_count += 1;
                                        if is_ve_related {
                                            eprintln!("[DEBUG] open_project: Skipped bits constant '{}' (string '{}' not found in bit_options: {:?})", name, s, constant.bit_options);
                                        }
                                        continue;
                                    }
                                }
                            }
                            _ => {
                                skipped_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Skipped bits constant '{}' (unsupported value type)", name);
                                }
                                continue;
                            }
                        };
                        
                        // Modify the first byte
                        let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                        let mask_first = if bits_in_first_byte >= 8 {
                            0xFF
                        } else {
                            (1u8 << bits_in_first_byte) - 1
                        };
                        let value_first = (bit_value & mask_first as u32) as u8;
                        current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte)) | (value_first << bit_in_byte);
                        
                        // If bits span multiple bytes, modify additional bytes
                        if bits_remaining_after_first_byte > 0 {
                            let mut bits_collected = bits_in_first_byte;
                            for i in 1..bytes_needed_usize.min(current_bytes.len()) {
                                let remaining_bits = bit_size - bits_collected;
                                if remaining_bits == 0 {
                                    break;
                                }
                                let bits_from_this_byte = remaining_bits.min(8);
                                let mask = if bits_from_this_byte >= 8 {
                                    0xFF
                                } else {
                                    (1u8 << bits_from_this_byte) - 1
                                };
                                let value_from_bit = ((bit_value >> bits_collected) & mask as u32) as u8;
                                current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                bits_collected += bits_from_this_byte;
                            }
                        }
                        
                        // Write the modified byte(s) back
                        if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                            applied_count += 1;
                            if is_ve_related {
                                eprintln!("[DEBUG] open_project: Applied bits constant '{}' = {} (bit_pos={}, bit_size={}, bytes={})", 
                                    name, bit_value, bit_pos, bit_size, bytes_needed);
                            }
                        } else {
                            failed_count += 1;
                            if is_ve_related {
                                eprintln!("[DEBUG] open_project: Failed to write bits constant '{}'", name);
                            }
                        }
                        continue;
                    }
                    
                    let length = constant.size_bytes() as u16;
                    if length == 0 {
                        skipped_count += 1;
                        if is_ve_related {
                            eprintln!("[DEBUG] open_project: Skipped constant '{}' (zero size)", name);
                        }
                        continue;
                    }
                    
                    let element_size = constant.data_type.size_bytes();
                    let element_count = constant.shape.element_count();
                    let mut raw_data = vec![0u8; length as usize];
                    
                    match tune_value {
                        TuneValue::Scalar(v) => {
                            let raw_val = constant.display_to_raw(*v);
                            constant.data_type.write_to_bytes(&mut raw_data, 0, raw_val, def_clone.endianness);
                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied constant '{}' = {} (scalar, page={}, offset={})", 
                                        name, v, constant.page, constant.offset);
                                }
                            } else {
                                failed_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Failed to write constant '{}' (page={}, offset={}, len={}, page_size={:?})", 
                                        name, constant.page, constant.offset, length, cache.page_size(constant.page));
                                }
                            }
                        }
                        TuneValue::Array(arr) => {
                            // Handle size mismatches: write what we have, pad or truncate as needed
                            let write_count = arr.len().min(element_count);
                            let last_value = arr.last().copied().unwrap_or(0.0);
                            
                            if arr.len() != element_count && is_ve_related {
                                eprintln!("[DEBUG] open_project: Array size mismatch for '{}': expected {}, got {} (will write {} and pad/truncate)", 
                                    name, element_count, arr.len(), write_count);
                            }
                            
                            for i in 0..element_count {
                                let val = if i < arr.len() {
                                    arr[i]
                                } else {
                                    // Pad with last value if array is smaller
                                    last_value
                                };
                                let raw_val = constant.display_to_raw(val);
                                let offset = i * element_size;
                                constant.data_type.write_to_bytes(&mut raw_data, offset, raw_val, def_clone.endianness);
                            }
                            
                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied constant '{}' (array, {} elements written, page={}, offset={})", 
                                        name, write_count, constant.page, constant.offset);
                                }
                            } else {
                                failed_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Failed to write constant '{}' (array, page={}, offset={}, len={}, page_size={:?})", 
                                        name, constant.page, constant.offset, length, cache.page_size(constant.page));
                                }
                            }
                        }
                        TuneValue::String(_) | TuneValue::Bool(_) => {
                            skipped_count += 1;
                            if is_ve_related {
                                eprintln!("[DEBUG] open_project: Skipped constant '{}' (string/bool not supported for page data)", name);
                            }
                        }
                    }
                } else {
                    skipped_count += 1;
                    // Log first 10 skipped constants to see what's missing
                    if skipped_count <= 10 || is_ve_related {
                        eprintln!("[DEBUG] open_project: Constant '{}' not found in definition (skipped {}/{})", 
                            name, skipped_count, tune.constants.len());
                    }
                }
            }
            
            eprintln!("\n[INFO] ========================================");
            eprintln!("[INFO] TUNE LOAD SUMMARY:");
            eprintln!("[INFO]   Applied: {} constants", applied_count);
            eprintln!("[INFO]   Failed: {} constants", failed_count);
            eprintln!("[INFO]   Skipped: {} constants", skipped_count);
            eprintln!("[INFO]   Total in MSQ: {} constants", tune.constants.len());
            eprintln!("[INFO] ========================================\n");
        }
        drop(cache_guard);
        
        // Store tune in state
        *state.current_tune.lock().await = Some(tune.clone());
        *state.current_tune_path.lock().await = Some(project_path.join("CurrentTune.msq"));
        
        // Emit event to notify UI that tune was loaded
        let _ = app.emit("tune:loaded", "project");
        eprintln!("[INFO] ✓ Project opened successfully with tune file");
    } else {
        // No project tune - create empty cache
        eprintln!("[WARN] ⚠ Project opened but NO TUNE FILE found!");
        eprintln!("[WARN]   Expected tune file at: {:?}", project_path.join("CurrentTune.msq"));
        eprintln!("[WARN]   You can load an MSQ file manually using File > Load Tune");
        let cache = TuneCache::from_definition(&def_clone);
        *state.tune_cache.lock().await = Some(cache);
    }
    
    Ok(response)
}

/// Close the current project
#[tauri::command]
async fn close_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Get and close the project
    let mut proj_guard = state.current_project.lock().await;
    if let Some(project) = proj_guard.take() {
        project.close()
            .map_err(|e| format!("Failed to close project: {}", e))?;
    }
    
    // Clear definition
    let mut def_guard = state.definition.lock().await;
    *def_guard = None;
    
    // Clear tune
    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = None;
    
    Ok(())
}

/// Get current project info (or null if no project open)
#[tauri::command]
async fn get_current_project(state: tauri::State<'_, AppState>) -> Result<Option<CurrentProjectInfo>, String> {
    let proj_guard = state.current_project.lock().await;
    let tune_modified = *state.tune_modified.lock().await;
    
    Ok(proj_guard.as_ref().map(|project| CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
        },
    }))
}

/// Update project connection settings
#[tauri::command]
async fn update_project_connection(
    state: tauri::State<'_, AppState>,
    port: Option<String>,
    baud_rate: u32,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard.as_mut()
        .ok_or_else(|| "No project open".to_string())?;
    
    project.config.connection.port = port;
    project.config.connection.baud_rate = baud_rate;
    project.save_config()
        .map_err(|e| format!("Failed to save project config: {}", e))?;
    
    Ok(())
}

/// Find INI files that match a given ECU signature
#[tauri::command]
async fn find_matching_inis(
    state: tauri::State<'_, AppState>,
    ecu_signature: String,
) -> Result<Vec<MatchingIniInfo>, String> {
    Ok(find_matching_inis_internal(&state, &ecu_signature).await)
}

/// Update the project's INI file and optionally force re-sync
#[tauri::command]
async fn update_project_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ini_path: String,
    force_resync: bool,
) -> Result<(), String> {
    // Load the new INI definition
    let new_def = EcuDefinition::from_file(&ini_path)
        .map_err(|e| format!("Failed to parse INI file: {}", e))?;
    
    // Update the project config if we have a project open
    let mut proj_guard = state.current_project.lock().await;
    if let Some(ref mut project) = *proj_guard {
        // Copy the new INI to the project directory
        let project_ini_path = project.ini_path();
        std::fs::copy(&ini_path, &project_ini_path)
            .map_err(|e| format!("Failed to copy INI to project: {}", e))?;
        
        // Update project signature
        project.config.signature = new_def.signature.clone();
        project.save_config()
            .map_err(|e| format!("Failed to save project config: {}", e))?;
    }
    drop(proj_guard);
    
    // Update the loaded definition
    let mut def_guard = state.definition.lock().await;
    let def_clone = new_def.clone();
    *def_guard = Some(new_def);
    drop(def_guard);
    
    // Update settings with new INI path
    let mut settings = load_settings(&app);
    settings.last_ini_path = Some(ini_path);
    save_settings(&app, &settings);
    
    // Re-initialize cache with new definition and re-apply project tune constants
    let project_tune = {
        let proj_guard = state.current_project.lock().await;
        proj_guard.as_ref().and_then(|p| p.current_tune.as_ref().cloned())
    };
    
    // Create new cache from updated definition
    let cache = TuneCache::from_definition(&def_clone);
    let mut cache_guard = state.tune_cache.lock().await;
    *cache_guard = Some(cache);
    
    // Re-apply project tune constants with new definition
    if let Some(tune) = project_tune {
        if let Some(cache) = cache_guard.as_mut() {
            // Load any raw page data first
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }
            
            // Apply constants from tune file to cache (same logic as open_project)
            use libretune_core::tune::TuneValue;
            
            let mut applied_count = 0;
            let mut skipped_count = 0;
            let mut failed_count = 0;
            
            for (name, tune_value) in &tune.constants {
                if let Some(constant) = def_clone.constants.get(name) {
                    // PC variables are stored locally
                    if constant.is_pc_variable {
                        match tune_value {
                            TuneValue::Scalar(v) => {
                                cache.local_values.insert(name.clone(), *v);
                                applied_count += 1;
                            }
                            TuneValue::Array(arr) if !arr.is_empty() => {
                                cache.local_values.insert(name.clone(), arr[0]);
                                applied_count += 1;
                            }
                            _ => {
                                skipped_count += 1;
                            }
                        }
                        continue;
                    }
                    
                    let length = constant.size_bytes() as u16;
                    if length == 0 {
                        skipped_count += 1;
                        continue;
                    }
                    
                    let element_size = constant.data_type.size_bytes();
                    let element_count = constant.shape.element_count();
                    let mut raw_data = vec![0u8; length as usize];
                    
                    match tune_value {
                        TuneValue::Scalar(v) => {
                            let raw_val = constant.display_to_raw(*v);
                            constant.data_type.write_to_bytes(&mut raw_data, 0, raw_val, def_clone.endianness);
                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                            } else {
                                failed_count += 1;
                            }
                        }
                        TuneValue::Array(arr) => {
                            // Handle size mismatches
                            let write_count = arr.len().min(element_count);
                            let last_value = arr.last().copied().unwrap_or(0.0);
                            
                            for i in 0..element_count {
                                let val = if i < arr.len() {
                                    arr[i]
                                } else {
                                    last_value
                                };
                                let raw_val = constant.display_to_raw(val);
                                let offset = i * element_size;
                                constant.data_type.write_to_bytes(&mut raw_data, offset, raw_val, def_clone.endianness);
                            }
                            
                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                            } else {
                                failed_count += 1;
                            }
                        }
                        TuneValue::String(_) | TuneValue::Bool(_) => {
                            skipped_count += 1;
                        }
                    }
                } else {
                    skipped_count += 1;
                }
            }
            
            eprintln!("[DEBUG] update_project_ini: Re-applied tune constants - applied: {}, failed: {}, skipped: {}, total: {}", 
                applied_count, failed_count, skipped_count, tune.constants.len());
            
            // Emit event to notify UI that tune data was re-applied
            let _ = app.emit("tune:loaded", "ini_updated");
        }
    }
    drop(cache_guard);
    
    // If force_resync is requested and we're connected, trigger re-sync
    if force_resync {
        let conn_guard = state.connection.lock().await;
        if conn_guard.is_some() {
            drop(conn_guard);
            // Emit event to notify frontend to re-sync
            let _ = app.emit("ini:changed", "resync_required");
        }
    }
    
    Ok(())
}

// =====================================================
// INI Repository Commands
// =====================================================

/// Initialize the INI repository
#[tauri::command]
async fn init_ini_repository(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let repo = IniRepository::open(None)
        .map_err(|e| format!("Failed to open INI repository: {}", e))?;
    
    let path = repo.path.to_string_lossy().to_string();
    
    let mut guard = state.ini_repository.lock().await;
    *guard = Some(repo);
    
    Ok(path)
}

/// List INIs in the repository
#[tauri::command]
async fn list_repository_inis(state: tauri::State<'_, AppState>) -> Result<Vec<IniEntryResponse>, String> {
    let guard = state.ini_repository.lock().await;
    let repo = guard.as_ref()
        .ok_or_else(|| "INI repository not initialized".to_string())?;
    
    Ok(repo.list().iter().map(|e| IniEntryResponse {
        id: e.id.clone(),
        name: e.name.clone(),
        signature: e.signature.clone(),
        path: e.path.clone(),
    }).collect())
}

/// Import an INI file into the repository
#[tauri::command]
async fn import_ini(state: tauri::State<'_, AppState>, source_path: String) -> Result<IniEntryResponse, String> {
    let mut guard = state.ini_repository.lock().await;
    let repo = guard.as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;
    
    let id = repo.import(Path::new(&source_path))
        .map_err(|e| format!("Failed to import INI: {}", e))?;
    
    let entry = repo.get(&id)
        .ok_or_else(|| "Failed to get imported INI".to_string())?;
    
    Ok(IniEntryResponse {
        id: entry.id.clone(),
        name: entry.name.clone(),
        signature: entry.signature.clone(),
        path: entry.path.clone(),
    })
}

/// Scan a directory for INI files and import them
#[tauri::command]
async fn scan_for_inis(state: tauri::State<'_, AppState>, directory: String) -> Result<Vec<String>, String> {
    let mut guard = state.ini_repository.lock().await;
    let repo = guard.as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;
    
    repo.scan_directory(Path::new(&directory))
        .map_err(|e| format!("Failed to scan directory: {}", e))
}

/// Remove an INI from the repository
#[tauri::command]
async fn remove_ini(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let mut guard = state.ini_repository.lock().await;
    let repo = guard.as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;
    
    repo.remove(&id)
        .map_err(|e| format!("Failed to remove INI: {}", e))
}

// =============================================================================
// ONLINE INI REPOSITORY COMMANDS
// =============================================================================

/// Serializable version of OnlineIniEntry for the frontend
#[derive(Serialize)]
struct OnlineIniEntryResponse {
    source: String,
    name: String,
    signature: Option<String>,
    download_url: String,
    repo_path: String,
    size: Option<u64>,
}

impl From<OnlineIniEntry> for OnlineIniEntryResponse {
    fn from(entry: OnlineIniEntry) -> Self {
        OnlineIniEntryResponse {
            source: entry.source.display_name().to_string(),
            name: entry.name,
            signature: entry.signature,
            download_url: entry.download_url,
            repo_path: entry.repo_path,
            size: entry.size,
        }
    }
}

/// Check if we have internet connectivity
#[tauri::command]
async fn check_internet_connectivity(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let repo = state.online_ini_repository.lock().await;
    Ok(repo.check_connectivity().await)
}

/// Search for INI files online matching a signature
/// If signature is None, returns all available INIs
#[tauri::command]
async fn search_online_inis(
    state: tauri::State<'_, AppState>,
    signature: Option<String>,
) -> Result<Vec<OnlineIniEntryResponse>, String> {
    let mut repo = state.online_ini_repository.lock().await;
    
    let results = repo.search(signature.as_deref())
        .await
        .map_err(|e| format!("Failed to search online INIs: {}", e))?;
    
    Ok(results.into_iter().map(|e| e.into()).collect())
}

/// Download an INI file from online repository
#[tauri::command]
async fn download_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    download_url: String,
    name: String,
    source: String,
) -> Result<String, String> {
    let repo = state.online_ini_repository.lock().await;
    
    // Create an OnlineIniEntry from the provided info
    let source_enum = match source.to_lowercase().as_str() {
        "speeduino" => IniSource::Speeduino,
        "rusefi" => IniSource::RusEFI,
        _ => IniSource::Custom,
    };
    
    let entry = OnlineIniEntry {
        source: source_enum,
        name: name.clone(),
        signature: None,
        download_url,
        repo_path: name.clone(),
        size: None,
    };
    
    // Download to definitions directory
    let definitions_dir = get_definitions_dir(&app);
    
    let downloaded_path = repo.download(&entry, &definitions_dir)
        .await
        .map_err(|e| format!("Failed to download INI: {}", e))?;
    
    // Also import to local repository
    drop(repo);
    let mut local_repo_guard = state.ini_repository.lock().await;
    if let Some(ref mut local_repo) = *local_repo_guard {
        let _ = local_repo.import(&downloaded_path);
    }
    
    Ok(downloaded_path.to_string_lossy().to_string())
}

// =============================================================================
// DEMO MODE COMMANDS
// =============================================================================

/// Enable or disable demo mode (simulated ECU for UI testing)
/// When enabled, loads a bundled EpicEFI INI and generates simulated sensor data
#[tauri::command]
async fn set_demo_mode(app: tauri::AppHandle, state: tauri::State<'_, AppState>, enabled: bool) -> Result<(), String> {
    // Stop any existing streaming first
    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }
    
    if enabled {
        // Disconnect any existing connection to avoid mismatched definitions
        {
            let mut conn_guard = state.connection.lock().await;
            *conn_guard = None;
        }

        // Close and clear any open project/tune to ensure a clean demo state
        {
            let mut proj_guard = state.current_project.lock().await;
            if let Some(project) = proj_guard.take() {
                let _ = project.close();
            }
        }
        {
            let mut tune_guard = state.current_tune.lock().await;
            *tune_guard = None;
        }
        {
            let mut tune_mod_guard = state.tune_modified.lock().await;
            *tune_mod_guard = false;
        }

        // Load the bundled demo INI
        let resource_path = app.path().resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?
            .join("resources")
            .join("demo.ini");
        
        // Try resource path first, then development path
        let ini_path = if resource_path.exists() {
            resource_path
        } else {
            // Development fallback: look in src-tauri/resources
            let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("demo.ini");
            if dev_path.exists() {
                dev_path
            } else {
                return Err(format!("Demo INI not found at {:?} or {:?}", resource_path, dev_path));
            }
        };
        
        // Load the INI definition
        let def = EcuDefinition::from_file(ini_path.to_string_lossy().as_ref())
            .map_err(|e| format!("Failed to load demo INI: {}", e))?;
        
        // Initialize TuneCache from definition
        let cache = TuneCache::from_definition(&def);

        // Apply the demo state to the AppState (aborts streaming, clears connection/project/tune and stores def/cache)
        apply_demo_enable(&state, def, cache).await?;

        // Notify frontend that definition/demo mode changed
        let _ = app.emit("demo:changed", true);
        let _ = app.emit("definition:changed", ());

        eprintln!("[DEMO] Demo mode enabled - loaded demo INI and cleared open project/connection");
    } else {
        // Disable demo mode
        {
            let mut demo_guard = state.demo_mode.lock().await;
            *demo_guard = false;
        }

        // Notify frontend demo disabled
        let _ = app.emit("demo:changed", false);

        eprintln!("[DEMO] Demo mode disabled");
    }
    
    Ok(())
}

/// Internal helper: apply demo enable with a provided definition and cache
async fn apply_demo_enable(state: &AppState, def: EcuDefinition, cache: TuneCache) -> Result<(), String> {
    // Stop any existing streaming task first
    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    // Disconnect any existing connection
    {
        let mut conn_guard = state.connection.lock().await;
        *conn_guard = None;
    }

    // Close and clear any open project/tune to ensure a clean demo state
    {
        let mut proj_guard = state.current_project.lock().await;
        if let Some(project) = proj_guard.take() {
            let _ = project.close();
        }
    }

    {
        let mut tune_guard = state.current_tune.lock().await;
        *tune_guard = None;
    }

    {
        let mut tune_mod_guard = state.tune_modified.lock().await;
        *tune_mod_guard = false;
    }

    // Store the provided cache and definition
    {
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);
    }

    {
        let mut def_guard = state.definition.lock().await;
        *def_guard = Some(def);
    }

    // Set demo mode flag
    {
        let mut demo_guard = state.demo_mode.lock().await;
        *demo_guard = true;
    }

    Ok(())
}

async fn apply_demo_disable(state: &AppState) -> Result<(), String> {
    {
        let mut demo_guard = state.demo_mode.lock().await;
        *demo_guard = false;
    }
    Ok(())
}

/// Check if demo mode is currently enabled
#[tauri::command]
async fn get_demo_mode(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let demo_guard = state.demo_mode.lock().await;
    Ok(*demo_guard)
}

#[cfg(test)]
mod demo_mode_tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_apply_demo_enable_and_disable() {
        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::new()),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
        };

        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("demo.ini");
        assert!(dev_path.exists(), "Demo INI not found at {:?}", dev_path);
        let def = EcuDefinition::from_file(dev_path.to_string_lossy().as_ref()).expect("Load demo INI");
        let cache = TuneCache::from_definition(&def);

        // initial state
        assert!(!*state.demo_mode.lock().await);
        assert!(state.definition.lock().await.is_none());
        assert!(state.tune_cache.lock().await.is_none());

        apply_demo_enable(&state, def.clone(), cache).await.expect("apply enable");
        assert!(*state.demo_mode.lock().await);
        assert!(state.definition.lock().await.is_some());
        assert!(state.tune_cache.lock().await.is_some());

        apply_demo_disable(&state).await.expect("apply disable");
        assert!(!*state.demo_mode.lock().await);
    }
}

/// Get application settings
#[tauri::command]
async fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    Ok(load_settings(&app))
}

/// Update a single setting
#[tauri::command]
async fn update_setting(
    app: tauri::AppHandle,
    key: String,
    value: String
) -> Result<(), String> {
    let mut settings = load_settings(&app);

    match key.as_str() {
        "units_system" => settings.units_system = value,
        "auto_burn_on_close" => settings.auto_burn_on_close = value.parse().map_err(|_| "Invalid boolean value")?,
        "gauge_snap_to_grid" => settings.gauge_snap_to_grid = value.parse().map_err(|_| "Invalid boolean value")?,
        "gauge_free_move" => settings.gauge_free_move = value.parse().map_err(|_| "Invalid boolean value")?,
        "gauge_lock" => settings.gauge_lock = value.parse().map_err(|_| "Invalid boolean value")?,
        "indicator_column_count" => settings.indicator_column_count = value,
        "indicator_fill_empty" => settings.indicator_fill_empty = value.parse().map_err(|_| "Invalid boolean value")?,
        "indicator_text_fit" => settings.indicator_text_fit = value,
        _ => return Err(format!("Unknown setting: {}", key)),
    }

    save_settings(&app, &settings);
    Ok(())
}

/// Update a string-type constant
#[tauri::command]
async fn update_constant_string(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    name: String,
    value: String
) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def.constants.get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // Validate it's a string type
    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }

    // For now, string constants are just stored locally without ECU write
    // In the future, we might need to handle ECU memory updates
    eprintln!("Updated string constant '{}' to: '{}'", name, value);

    Ok(())
}

/// Use the project tune (discard ECU tune)
#[tauri::command]
async fn use_project_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let project = project_guard.as_ref().ok_or("No project loaded")?;
    
    // Load project tune from disk
    let tune_path = project.current_tune_path();
    if tune_path.exists() {
        let tune = TuneFile::load(&tune_path)
            .map_err(|e| format!("Failed to load project tune: {}", e))?;
        
        // Populate TuneCache from project tune
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                for (page_num, page_data) in &tune.pages {
                    cache.load_page(*page_num, page_data.clone());
                }
            }
        }
        
        // Set as current tune
        *state.current_tune.lock().await = Some(tune);
        *state.current_tune_path.lock().await = Some(tune_path);
        *state.tune_modified.lock().await = false;
        
        // Emit event to trigger re-sync if connected
        let _ = app.emit("tune:loaded", "project");
    } else {
        return Err("Project tune file not found".to_string());
    }
    
    Ok(())
}

/// Use the ECU tune (discard project tune changes)
#[tauri::command]
async fn use_ecu_tune(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // ECU tune is already loaded from sync, just mark as not modified
    *state.tune_modified.lock().await = false;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::new()),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_serial_ports,
            get_available_inis,
            connect_to_ecu,
            sync_ecu_data,
            disconnect_ecu,
            enable_adaptive_timing,
            disable_adaptive_timing,
            get_adaptive_timing_stats,
            get_connection_status,
            load_ini,
            get_realtime_data,
            start_realtime_stream,
            stop_realtime_stream,
            get_table_data,
            get_table_info,
            get_curve_data,
            get_tables,
            get_gauge_configs,
            get_available_channels,
            get_status_bar_defaults,
            get_frontpage,
            update_table_data,
            get_menu_tree,
            get_dialog_definition,
            // INI / protocol defaults
            get_protocol_defaults,
            get_help_topic,
            get_constant,
            get_constant_value,
            get_constant_string_value,
            update_constant,
            auto_load_last_ini,
            evaluate_expression,
            get_all_constant_values,
            start_autotune,
            stop_autotune,
            get_autotune_recommendations,
            get_autotune_heatmap,
            send_autotune_recommendations,
            burn_autotune_recommendations,
            lock_autotune_cells,
            unlock_autotune_cells,
            rebin_table,
            smooth_table,
            interpolate_cells,
            scale_cells,
            set_cells_equal,
            save_dashboard_layout,
            load_dashboard_layout,
            list_dashboard_layouts,
            create_default_dashboard,
            get_dashboard_templates,
            load_tunerstudio_dash,
            get_dash_file,
            list_available_dashes,
            // Tune file commands
            get_tune_info,
            new_tune,
            save_tune,
            save_tune_as,
            load_tune,
            list_tune_files,
            burn_to_ecu,
            use_project_tune,
            use_ecu_tune,
            mark_tune_modified,
            compare_project_and_ecu_tunes,
            write_project_tune_to_ecu,
            save_tune_to_project,
            // Tune cache commands
            get_tune_cache_status,
            load_all_pages,
            // Data logging commands
            start_logging,
            stop_logging,
            get_logging_status,
            get_log_entries,
            clear_log,
            save_log,
            // Project management commands
            get_projects_path,
            list_projects,
            create_project,
            open_project,
            close_project,
            get_current_project,
            update_project_connection,
            // INI signature management commands
            find_matching_inis,
            update_project_ini,
            // INI repository commands
            init_ini_repository,
            list_repository_inis,
            import_ini,
            scan_for_inis,
            remove_ini,
            // Online INI repository commands
            check_internet_connectivity,
            search_online_inis,
            download_ini,
            // Demo mode commands
            set_demo_mode,
            get_demo_mode,
            // Settings commands
            get_settings,
            update_setting,
            update_constant_string
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
