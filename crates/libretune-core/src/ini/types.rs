//! Common types used across INI parsing

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;

/// ECU type detected from INI signature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcuType {
    /// Speeduino open-source Arduino-based ECU
    Speeduino,
    /// rusEFI standard implementation
    RusEFI,
    /// FOME (Fork of Massive Enhancements) - rusEFI variant
    FOME,
    /// epicEFI - rusEFI variant for epicECU boards
    EpicEFI,
    /// MegaSquirt 2
    MS2,
    /// MegaSquirt 3
    MS3,
    /// Unknown or unsupported ECU type
    Unknown,
}

impl EcuType {
    /// Detect ECU type from signature string
    ///
    /// # Arguments
    /// * `signature` - The ECU signature from [MegaTune] section
    /// * `filename` - The INI filename for additional context
    ///
    /// # Returns
    /// The detected ECU type
    pub fn detect(signature: &str, filename: Option<&str>) -> Self {
        let sig_lower = signature.to_lowercase();
        let filename_lower = filename.map(|f| f.to_lowercase());

        // Check for FOME first (it also contains "rusefi")
        if sig_lower.contains("fome") || filename_lower.as_ref().is_some_and(|f| f.contains("fome"))
        {
            return EcuType::FOME;
        }

        // Check for epicEFI (contains "epicECU" or filename suggests it)
        if sig_lower.contains("epicECU")
            || filename_lower
                .as_ref()
                .is_some_and(|f| f.contains("epicECU"))
        {
            return EcuType::EpicEFI;
        }

        // Check for rusEFI (standard)
        if sig_lower.contains("rusefi") {
            return EcuType::RusEFI;
        }

        // Check for Speeduino
        if sig_lower.contains("speeduino") {
            return EcuType::Speeduino;
        }

        // Check for MegaSquirt
        if sig_lower.starts_with("ms3") || sig_lower.starts_with("ms3format") {
            return EcuType::MS3;
        }

        if sig_lower.starts_with("ms2") || sig_lower.contains("ms2extra") {
            return EcuType::MS2;
        }

        EcuType::Unknown
    }

    /// Check if this ECU type supports the rusEFI console
    pub fn supports_console(&self) -> bool {
        matches!(self, EcuType::RusEFI | EcuType::FOME | EcuType::EpicEFI)
    }

    /// Check if this is a FOME variant
    pub fn is_fome(&self) -> bool {
        matches!(self, EcuType::FOME)
    }

    /// Get display name for the ECU type
    pub fn display_name(&self) -> &'static str {
        match self {
            EcuType::Speeduino => "Speeduino",
            EcuType::RusEFI => "rusEFI",
            EcuType::FOME => "FOME",
            EcuType::EpicEFI => "epicEFI",
            EcuType::MS2 => "MegaSquirt 2",
            EcuType::MS3 => "MegaSquirt 3",
            EcuType::Unknown => "Unknown",
        }
    }
}

/// INI-driven feature capabilities derived from the loaded ECU definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IniCapabilities {
    pub has_constants: bool,
    pub has_output_channels: bool,
    pub has_tables: bool,
    pub has_curves: bool,
    pub has_gauges: bool,
    pub has_frontpage: bool,
    pub has_dialogs: bool,
    pub has_help_topics: bool,
    pub has_setting_groups: bool,
    pub has_pc_variables: bool,
    pub has_default_values: bool,
    pub has_datalog_entries: bool,
    pub has_datalog_views: bool,
    pub has_logger_definitions: bool,
    pub has_controller_commands: bool,
    pub has_port_editors: bool,
    pub has_reference_tables: bool,
    pub has_key_actions: bool,
    pub has_ve_analyze: bool,
    pub has_wue_analyze: bool,
    pub has_gamma_e: bool,
    pub supports_console: bool,
}

/// Data types supported by ECU constants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Unsigned 8-bit integer
    U08,
    /// Signed 8-bit integer
    S08,
    /// Unsigned 16-bit integer
    U16,
    /// Signed 16-bit integer
    S16,
    /// Unsigned 32-bit integer
    U32,
    /// Signed 32-bit integer
    S32,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point (double)
    F64,
    /// ASCII string
    String,
    /// Bit field within a byte
    Bits,
}

/// Endianness of ECU data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Endianness {
    #[default]
    Big,
    Little,
}

impl DataType {
    /// Parse a data type from INI format string
    /// Returns the data type and optionally an endianness override
    /// Per-field big-endian types (BU08, BS16, etc.) override global endianness
    pub fn from_ini_str(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "U08" | "UINT8" | "BYTE" | "BU08" => Some(DataType::U08),
            "S08" | "INT8" | "SBYTE" | "BS08" => Some(DataType::S08),
            "U16" | "UINT16" | "WORD" | "BU16" => Some(DataType::U16),
            "S16" | "INT16" | "SWORD" | "BS16" => Some(DataType::S16),
            "U32" | "UINT32" | "DWORD" | "BU32" => Some(DataType::U32),
            "S32" | "INT32" | "SDWORD" | "BS32" => Some(DataType::S32),
            "F32" | "FLOAT" | "BF32" => Some(DataType::F32),
            "F64" | "DOUBLE" | "BF64" => Some(DataType::F64),
            "ASCII" | "STRING" => Some(DataType::String),
            "BITS" => Some(DataType::Bits),
            _ => None,
        }
    }

    /// Parse a data type and extract per-field endianness override
    /// Types starting with 'B' (BU08, BS16, etc.) force big-endian byte order
    /// Returns (DataType, Option<Endianness>) where the endianness is Some(Big) for B* types
    pub fn from_ini_str_with_endianness(s: &str) -> Option<(Self, Option<Endianness>)> {
        let s = s.trim().to_uppercase();
        let override_endian = if s.starts_with('B')
            && s.len() > 1
            && s.chars().nth(1).is_some_and(|c| c.is_ascii_uppercase())
        {
            // Types like BU08, BS16, BF32 force big-endian
            Some(Endianness::Big)
        } else {
            None
        };
        Self::from_ini_str(&s).map(|dt| (dt, override_endian))
    }

    /// Get the size in bytes for this data type
    pub fn size_bytes(&self) -> usize {
        match self {
            DataType::U08 | DataType::S08 | DataType::Bits => 1,
            DataType::U16 | DataType::S16 => 2,
            DataType::U32 | DataType::S32 | DataType::F32 => 4,
            DataType::F64 => 8,
            DataType::String => 0, // Variable size
        }
    }

    /// Read a value from bytes at given offset
    pub fn read_from_bytes(&self, data: &[u8], offset: usize, endian: Endianness) -> Option<f64> {
        use byteorder::{BigEndian, ByteOrder, LittleEndian};

        if offset + self.size_bytes() > data.len() {
            return None;
        }

        let bytes = &data[offset..];
        match (self, endian) {
            (DataType::U08 | DataType::Bits, _) => Some(bytes[0] as f64),
            (DataType::S08, _) => Some(bytes[0] as i8 as f64),
            (DataType::U16, Endianness::Big) => Some(BigEndian::read_u16(bytes) as f64),
            (DataType::U16, Endianness::Little) => Some(LittleEndian::read_u16(bytes) as f64),
            (DataType::S16, Endianness::Big) => Some(BigEndian::read_i16(bytes) as f64),
            (DataType::S16, Endianness::Little) => Some(LittleEndian::read_i16(bytes) as f64),
            (DataType::U32, Endianness::Big) => Some(BigEndian::read_u32(bytes) as f64),
            (DataType::U32, Endianness::Little) => Some(LittleEndian::read_u32(bytes) as f64),
            (DataType::S32, Endianness::Big) => Some(BigEndian::read_i32(bytes) as f64),
            (DataType::S32, Endianness::Little) => Some(LittleEndian::read_i32(bytes) as f64),
            (DataType::F32, Endianness::Big) => Some(BigEndian::read_f32(bytes) as f64),
            (DataType::F32, Endianness::Little) => Some(LittleEndian::read_f32(bytes) as f64),
            (DataType::F64, Endianness::Big) => Some(BigEndian::read_f64(bytes)),
            (DataType::F64, Endianness::Little) => Some(LittleEndian::read_f64(bytes)),
            (DataType::String, _) => None,
        }
    }

    /// Write a value to bytes at given offset
    pub fn write_to_bytes(&self, data: &mut [u8], offset: usize, value: f64, endian: Endianness) {
        use byteorder::{BigEndian, ByteOrder, LittleEndian};

        if offset + self.size_bytes() > data.len() {
            return;
        }

        let bytes = &mut data[offset..];
        match (self, endian) {
            (DataType::U08 | DataType::Bits, _) => bytes[0] = value as u8,
            (DataType::S08, _) => bytes[0] = value as i8 as u8,
            (DataType::U16, Endianness::Big) => BigEndian::write_u16(bytes, value as u16),
            (DataType::U16, Endianness::Little) => LittleEndian::write_u16(bytes, value as u16),
            (DataType::S16, Endianness::Big) => BigEndian::write_i16(bytes, value as i16),
            (DataType::S16, Endianness::Little) => LittleEndian::write_i16(bytes, value as i16),
            (DataType::U32, Endianness::Big) => BigEndian::write_u32(bytes, value as u32),
            (DataType::U32, Endianness::Little) => LittleEndian::write_u32(bytes, value as u32),
            (DataType::S32, Endianness::Big) => BigEndian::write_i32(bytes, value as i32),
            (DataType::S32, Endianness::Little) => LittleEndian::write_i32(bytes, value as i32),
            (DataType::F32, Endianness::Big) => BigEndian::write_f32(bytes, value as f32),
            (DataType::F32, Endianness::Little) => LittleEndian::write_f32(bytes, value as f32),
            (DataType::F64, Endianness::Big) => BigEndian::write_f64(bytes, value),
            (DataType::F64, Endianness::Little) => LittleEndian::write_f64(bytes, value),
            (DataType::String, _) => {}
        }
    }
}

/// Shape of a constant (scalar, 1D array, 2D array)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// Single value
    Scalar,
    /// 1D array with given size
    Array1D(usize),
    /// 2D array with [rows, cols]
    Array2D { rows: usize, cols: usize },
}

impl Shape {
    /// Parse shape from INI format (e.g., "[16]" or "[16x16]")
    pub fn from_ini_str(s: &str) -> Self {
        let s = s.trim();
        if s.is_empty() {
            return Shape::Scalar;
        }

        // Remove brackets if present and trim whitespace
        let inner = s.trim_start_matches('[').trim_end_matches(']').trim();

        if inner.contains('x') || inner.contains('X') {
            // 2D array
            let parts: Vec<&str> = inner.split(['x', 'X']).collect();
            if parts.len() == 2 {
                if let (Ok(rows), Ok(cols)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
                    return Shape::Array2D { rows, cols };
                }
            }
        } else if let Ok(size) = inner.parse() {
            // 1D array
            return Shape::Array1D(size);
        }

        Shape::Scalar
    }

    /// Get total element count
    pub fn element_count(&self) -> usize {
        match self {
            Shape::Scalar => 1,
            Shape::Array1D(size) => *size,
            Shape::Array2D { rows, cols } => rows * cols,
        }
    }

    /// Get the X dimension (columns for 2D, size for 1D, 1 for scalar)
    pub fn x_size(&self) -> usize {
        match self {
            Shape::Scalar => 1,
            Shape::Array1D(size) => *size,
            Shape::Array2D { cols, .. } => *cols,
        }
    }

    /// Get the Y dimension (rows for 2D, 1 otherwise)
    pub fn y_size(&self) -> usize {
        match self {
            Shape::Scalar => 1,
            Shape::Array1D(_) => 1,
            Shape::Array2D { rows, .. } => *rows,
        }
    }
}

/// Setting group for UI organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingGroup {
    /// Internal reference name
    pub name: String,
    /// Display label
    pub label: String,
    /// Available options
    pub options: Vec<SettingOption>,
}

/// An option within a setting group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingOption {
    /// Option value
    pub value: String,
    pub label: String,
}

/// A high-level menu container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Menu {
    /// Internal name or ID
    pub name: String,
    /// Display title for the top-level menu
    pub title: String,
    /// List of items (submenus, dialogs, tables)
    pub items: Vec<MenuItem>,
}

/// A menu item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MenuItem {
    /// Link to a dialog
    Dialog {
        label: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
        /// Whether item is visible (evaluated from visibility_condition)
        #[serde(default = "default_true")]
        visible: bool,
        /// Whether item is enabled (evaluated from enabled_condition)  
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Link to a table editor
    Table {
        label: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
        /// Whether item is visible (evaluated from visibility_condition)
        #[serde(default = "default_true")]
        visible: bool,
        /// Whether item is enabled (evaluated from enabled_condition)
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Submenu (standard or group)
    SubMenu {
        label: String,
        items: Vec<MenuItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
        /// Whether item is visible (evaluated from visibility_condition)
        #[serde(default = "default_true")]
        visible: bool,
        /// Whether item is enabled (evaluated from enabled_condition)
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Built-in standard feature (std_realtime, std_ms2gentherm, etc.)
    Std {
        label: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
        /// Whether item is visible (evaluated from visibility_condition)
        #[serde(default = "default_true")]
        visible: bool,
        /// Whether item is enabled (evaluated from enabled_condition)
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Link to a help topic
    Help {
        label: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
        /// Whether item is visible (evaluated from visibility_condition)
        #[serde(default = "default_true")]
        visible: bool,
        /// Whether item is enabled (evaluated from enabled_condition)
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Separator between menu items
    Separator,
}

/// Helper for serde default that returns true
fn default_true() -> bool {
    true
}

/// A help topic definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpTopic {
    /// Internal name/ID
    pub name: String,
    /// Display title
    pub title: String,
    /// Web help URL (opens in browser)
    pub web_url: Option<String>,
    /// Text content lines (may contain HTML)
    pub text_lines: Vec<String>,
}

/// Dialog definition for settings windows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogDefinition {
    /// Internal name/ID
    pub name: String,
    /// Display title
    pub title: String,
    /// Components within the dialog
    pub components: Vec<DialogComponent>,
}

/// Components that can exist inside a dialog
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DialogComponent {
    /// A simple text label
    Label { text: String },
    /// Reference to an indicator panel (with optional visibility condition)
    Panel {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        position: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
    },
    /// A constant field with label and optional visibility/enable conditions
    Field {
        label: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        visibility_condition: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
    },
    /// A live graph visualization
    LiveGraph {
        name: String,
        title: String,
        position: String,
        channels: Vec<String>,
    },
    /// An embedded table editor
    Table { name: String },
    /// An indicator
    Indicator {
        expression: String,
        label_off: String,
        label_on: String,
    },
    /// A command button that sends commands to the ECU
    CommandButton {
        /// Button label text
        label: String,
        /// Name of the command in [ControllerCommands] section
        command: String,
        /// Optional condition expression for button enable state
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled_condition: Option<String>,
        /// Behavior on dialog close
        #[serde(skip_serializing_if = "Option::is_none")]
        on_close_behavior: Option<CommandButtonCloseAction>,
    },
}

/// Behavior for commandButton on dialog close
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandButtonCloseAction {
    /// Execute command on close if condition is true
    ClickOnCloseIfEnabled,
    /// Execute command on close if condition is false
    ClickOnCloseIfDisabled,
    /// Always execute command on dialog close
    ClickOnClose,
}

/// Datalog entry definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatalogEntry {
    /// Output channel name
    pub channel: String,
    /// Display label
    pub label: String,
    /// Format string
    pub format: String,
    /// Whether enabled by default
    pub enabled: bool,
}

/// FrontPage configuration for default dashboard layout
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrontPageConfig {
    /// Gauge references (gauge1-gauge8 → gauge names)
    pub gauges: Vec<String>,
    /// Status indicators
    pub indicators: Vec<FrontPageIndicator>,
}

/// FrontPage status indicator with color support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontPageIndicator {
    /// Boolean expression (e.g., "running", "tps > 50", "sd_status & 8")
    pub expression: String,
    /// Label when expression is false
    pub label_off: String,
    /// Label when expression is true (can be dynamic expression with { })
    pub label_on: String,
    /// Background color when off (named color or hex)
    pub bg_off: String,
    /// Foreground (text) color when off
    pub fg_off: String,
    /// Background color when on
    pub bg_on: String,
    /// Foreground (text) color when on
    pub fg_on: String,
}

impl FrontPageIndicator {
    /// Convert a named color to CSS hex color
    pub fn color_to_css(color: &str) -> String {
        match color.to_lowercase().as_str() {
            "white" => "#ffffff".to_string(),
            "black" => "#000000".to_string(),
            "red" => "#ff0000".to_string(),
            "green" => "#00ff00".to_string(),
            "blue" => "#0000ff".to_string(),
            "yellow" => "#ffff00".to_string(),
            "orange" => "#ffa500".to_string(),
            "cyan" => "#00ffff".to_string(),
            "magenta" => "#ff00ff".to_string(),
            "gray" | "grey" => "#808080".to_string(),
            "lightgray" | "lightgrey" => "#d3d3d3".to_string(),
            "darkgray" | "darkgrey" => "#a9a9a9".to_string(),
            "lime" => "#00ff00".to_string(),
            "maroon" => "#800000".to_string(),
            "navy" => "#000080".to_string(),
            "olive" => "#808000".to_string(),
            "purple" => "#800080".to_string(),
            "silver" => "#c0c0c0".to_string(),
            "teal" => "#008080".to_string(),
            // If already hex or unknown, return as-is
            other => {
                if other.starts_with('#') {
                    other.to_string()
                } else {
                    // Try to use as CSS color name
                    other.to_string()
                }
            }
        }
    }
}

/// Configuration for adaptive timing
/// When enabled, dynamically adjusts communication delays based on measured ECU response times
#[derive(Debug, Clone)]
pub struct AdaptiveTimingConfig {
    /// Whether adaptive timing is enabled
    pub enabled: bool,
    /// Minimum timeout in milliseconds (floor for adaptive adjustment)
    pub min_timeout_ms: u32,
    /// Maximum timeout in milliseconds (ceiling for adaptive adjustment)
    pub max_timeout_ms: u32,
    /// Number of samples to keep for rolling average
    pub sample_count: usize,
    /// Multiplier applied to average response time (e.g., 2.5 = timeout is 2.5x avg response)
    pub multiplier: f32,
}

impl Default for AdaptiveTimingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_timeout_ms: 10,
            max_timeout_ms: 500,
            sample_count: 20,
            multiplier: 2.5,
        }
    }
}

impl Serialize for AdaptiveTimingConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AdaptiveTimingConfig", 5)?;
        state.serialize_field("enabled", &self.enabled)?;
        state.serialize_field("min_timeout_ms", &self.min_timeout_ms)?;
        state.serialize_field("max_timeout_ms", &self.max_timeout_ms)?;
        state.serialize_field("sample_count", &self.sample_count)?;
        state.serialize_field("multiplier", &self.multiplier)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for AdaptiveTimingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            enabled: bool,
            min_timeout_ms: u32,
            max_timeout_ms: u32,
            sample_count: usize,
            multiplier: f32,
        }
        let helper = Helper::deserialize(deserializer)?;
        Ok(Self {
            enabled: helper.enabled,
            min_timeout_ms: helper.min_timeout_ms,
            max_timeout_ms: helper.max_timeout_ms,
            sample_count: helper.sample_count,
            multiplier: helper.multiplier,
        })
    }
}

/// Runtime state for adaptive timing (not serialized)
#[derive(Debug, Clone)]
pub struct AdaptiveTiming {
    /// Configuration
    pub config: AdaptiveTimingConfig,
    /// Rolling sample buffer of response times
    samples: VecDeque<Duration>,
    /// Current calculated timeout based on samples
    current_timeout: Duration,
    /// Running sum for efficient average calculation
    running_sum_us: u64,
}

impl AdaptiveTiming {
    /// Create new adaptive timing with given config
    pub fn new(config: AdaptiveTimingConfig) -> Self {
        let initial_timeout =
            Duration::from_millis(((config.min_timeout_ms + config.max_timeout_ms) / 2) as u64);
        Self {
            samples: VecDeque::with_capacity(config.sample_count),
            current_timeout: initial_timeout,
            running_sum_us: 0,
            config,
        }
    }

    /// Record a new response time sample and recalculate timeout
    pub fn record_response_time(&mut self, elapsed: Duration) {
        if !self.config.enabled {
            return;
        }

        let elapsed_us = elapsed.as_micros() as u64;

        // Remove oldest sample if at capacity
        if self.samples.len() >= self.config.sample_count {
            if let Some(old) = self.samples.pop_front() {
                self.running_sum_us = self.running_sum_us.saturating_sub(old.as_micros() as u64);
            }
        }

        // Add new sample
        self.samples.push_back(elapsed);
        self.running_sum_us += elapsed_us;

        // Recalculate timeout
        self.recalculate_timeout();
    }

    /// Get current effective timeout
    pub fn get_timeout(&self) -> Duration {
        if self.config.enabled && !self.samples.is_empty() {
            self.current_timeout
        } else {
            Duration::from_millis(self.config.max_timeout_ms as u64)
        }
    }

    /// Get current effective inter-character timeout (1/4 of main timeout, min 5ms)
    pub fn get_inter_char_timeout(&self) -> Duration {
        let main_ms = self.get_timeout().as_millis() as u64;
        Duration::from_millis(std::cmp::max(5, main_ms / 4))
    }

    /// Get current effective minimum wait time for write_and_wait (1/3 of timeout, min 5ms)
    pub fn get_min_wait(&self) -> Duration {
        let main_ms = self.get_timeout().as_millis() as u64;
        Duration::from_millis(std::cmp::max(5, main_ms / 3))
    }

    /// Reset adaptive timing (e.g., after communication error)
    /// Clears samples and backs off to conservative timeout
    pub fn reset_on_error(&mut self) {
        self.samples.clear();
        self.running_sum_us = 0;
        // Back off to 75% of max timeout
        self.current_timeout = Duration::from_millis((self.config.max_timeout_ms as u64 * 3) / 4);
    }

    /// Check if adaptive timing is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enable or disable adaptive timing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.samples.clear();
            self.running_sum_us = 0;
        }
    }

    /// Get average response time (for diagnostics)
    pub fn average_response_time(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            None
        } else {
            let avg_us = self.running_sum_us / self.samples.len() as u64;
            Some(Duration::from_micros(avg_us))
        }
    }

    /// Get number of samples collected
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    fn recalculate_timeout(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        // Calculate average in microseconds
        let avg_us = self.running_sum_us / self.samples.len() as u64;

        // Apply multiplier
        let timeout_us = (avg_us as f32 * self.config.multiplier) as u64;
        let timeout_ms = timeout_us / 1000;

        // Clamp to configured bounds
        let clamped_ms = timeout_ms
            .max(self.config.min_timeout_ms as u64)
            .min(self.config.max_timeout_ms as u64);

        self.current_timeout = Duration::from_millis(clamped_ms);
    }
}

impl Default for AdaptiveTiming {
    fn default() -> Self {
        Self::new(AdaptiveTimingConfig::default())
    }
}

/// Protocol settings parsed from INI file
/// These define how to communicate with the ECU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSettings {
    /// Message envelope format (e.g., "msEnvelope_1.0" for CRC framing)
    pub message_envelope_format: Option<String>,

    /// Query command to get signature (usually "S" or "Q")
    pub query_command: String,

    /// Delay in ms after opening port before sending commands
    pub delay_after_port_open: u32,

    /// Page identifiers for multi-page ECUs (raw byte sequences)
    pub page_identifiers: Vec<Vec<u8>>,

    /// Page sizes in bytes for each page
    pub page_sizes: Vec<u32>,

    /// Page read command format strings (one per page)
    /// Format: "R%2i%2o%2c" where %2i=page, %2o=offset, %2c=count
    pub page_read_commands: Vec<String>,

    /// Page write command format strings (one per page)
    /// Format: "C%2i%2o%2c%v" where %v=value bytes
    pub page_chunk_write_commands: Vec<String>,

    /// Burn command format strings (one per page, empty = no burn for that page)
    pub burn_commands: Vec<String>,

    /// CRC32 check command format strings (one per page)
    pub crc32_check_commands: Vec<String>,

    /// Command to get realtime/output channel data
    pub och_get_command: Option<String>,

    /// Size of output channel block in bytes
    pub och_block_size: u32,

    /// Max unused runtime range threshold (channel-count hint; 0 = disabled)
    pub max_unused_runtime_range: u32,

    /// Burst mode get command (usually "A")
    pub burst_get_command: Option<String>,

    /// Retrieve config error command
    pub retrieve_config_error: Option<String>,

    /// Delay in ms after burn command
    pub page_activation_delay: u32,

    /// Max bytes per read/write chunk
    pub blocking_factor: u32,

    /// Delay in ms between consecutive writes
    pub inter_write_delay: u32,

    /// Read timeout in ms
    pub block_read_timeout: u32,

    /// Whether to use block writes
    pub write_blocks: bool,

    /// Whether ECU uses CAN IDs (enable2ndByteCanID)
    pub enable_can_id: bool,

    /// Default baud rate
    pub default_baud_rate: u32,

    /// Default IP address for TCP connections
    pub default_ip_address: Option<String>,

    /// Default IP port for TCP connections
    pub default_ip_port: u16,
}

impl Default for ProtocolSettings {
    fn default() -> Self {
        Self {
            message_envelope_format: None,
            query_command: "Q".to_string(),
            delay_after_port_open: 0,
            page_identifiers: Vec::new(),
            page_sizes: Vec::new(),
            page_read_commands: Vec::new(),
            page_chunk_write_commands: Vec::new(),
            burn_commands: Vec::new(),
            crc32_check_commands: Vec::new(),
            och_get_command: None,
            och_block_size: 0,
            max_unused_runtime_range: 0,
            burst_get_command: Some("A".to_string()),
            retrieve_config_error: None,
            page_activation_delay: 500,
            blocking_factor: 256,
            inter_write_delay: 0,
            block_read_timeout: 1000,
            write_blocks: true,
            enable_can_id: true,
            default_baud_rate: 115200,
            default_ip_address: None,
            default_ip_port: 29001,
        }
    }
}

impl ProtocolSettings {
    /// Check if this ECU uses modern CRC-framed protocol
    pub fn uses_modern_protocol(&self) -> bool {
        self.message_envelope_format
            .as_ref()
            .map(|f| f.contains("msEnvelope"))
            .unwrap_or(false)
    }

    /// Get the number of pages
    pub fn num_pages(&self) -> usize {
        self.page_sizes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_parsing() {
        assert_eq!(DataType::from_ini_str("U08"), Some(DataType::U08));
        assert_eq!(DataType::from_ini_str("uint16"), Some(DataType::U16));
        assert_eq!(DataType::from_ini_str("BITS"), Some(DataType::Bits));
        assert_eq!(DataType::from_ini_str("invalid"), None);
    }

    #[test]
    fn test_shape_parsing() {
        assert_eq!(Shape::from_ini_str(""), Shape::Scalar);
        assert_eq!(Shape::from_ini_str("[16]"), Shape::Array1D(16));
        assert_eq!(
            Shape::from_ini_str("[16x16]"),
            Shape::Array2D { rows: 16, cols: 16 }
        );
        assert_eq!(
            Shape::from_ini_str("[8X12]"),
            Shape::Array2D { rows: 8, cols: 12 }
        );
    }
}

// =============================================================================
// Missing INI Section Data Structures (per EFI Analytics PDF spec)
// =============================================================================

/// Controller command definition
/// Commands can be raw byte strings (with hex escapes) or references to other commands (chaining)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerCommand {
    /// Command name/ID
    pub name: String,
    /// Display label
    pub label: String,
    /// Command parts - can be raw strings or command references
    /// e.g., ["\\x00\\x01", "cmd_reset", "\\x02\\x03"]
    pub parts: Vec<CommandPart>,
    /// Optional enable condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_condition: Option<String>,
}

/// Part of a command - either raw bytes or reference to another command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandPart {
    /// Raw byte string with potential hex escapes (e.g., "\\x00\\x01")
    Raw(String),
    /// Reference to another command by name (for chaining)
    Reference(String),
}

/// Logger definition for high-speed data logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerDefinition {
    /// Logger name
    pub name: String,
    /// Display label
    pub label: String,
    /// Sample rate in Hz
    pub sample_rate: f64,
    /// Output channels to log
    pub channels: Vec<String>,
    /// Enable condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_condition: Option<String>,
}

/// Port editor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortEditorConfig {
    /// Port name
    pub name: String,
    /// Display label
    pub label: String,
    /// Enable condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_condition: Option<String>,
}

/// Reference table definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceTable {
    /// Table name
    pub name: String,
    /// Display label
    pub label: String,
    /// Referenced table name
    pub table_name: String,
    /// Enable condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_condition: Option<String>,
}

/// FTP browser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FTPBrowserConfig {
    /// Browser name
    pub name: String,
    /// Display label
    pub label: String,
    /// FTP server address
    pub server: String,
    /// FTP port
    pub port: u16,
    /// Enable condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_condition: Option<String>,
}

/// Datalog view definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatalogView {
    /// View name
    pub name: String,
    /// Display label
    pub label: String,
    /// Channels to display in this view
    pub channels: Vec<String>,
}

/// Indicator panel definition (group of boolean indicators)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorPanel {
    /// Panel name/ID
    pub name: String,
    /// Number of columns for layout
    pub columns: u8,
    /// Optional visibility condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility_condition: Option<String>,
    /// Indicators within this panel
    pub indicators: Vec<IndicatorDefinition>,
}

/// Individual indicator within an indicator panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorDefinition {
    /// Expression to evaluate (boolean)
    pub expression: String,
    /// Label when indicator is off
    pub label_off: String,
    /// Label when indicator is on
    pub label_on: String,
    /// Optional foreground color when off (default: white)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_off_fg: Option<String>,
    /// Optional background color when off (default: black)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_off_bg: Option<String>,
    /// Optional foreground color when on (default: red)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_on_fg: Option<String>,
    /// Optional background color when on (default: black)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_on_bg: Option<String>,
}

/// Key action (keyboard shortcut) definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyAction {
    /// Key combination (e.g., "Ctrl+S", "F5")
    pub key: String,
    /// Action to perform
    pub action: String,
    /// Display label
    pub label: String,
    /// Enable condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_condition: Option<String>,
}

// ============================================================================
// VeAnalyze / WueAnalyze / GammaE Section Types
// ============================================================================

/// Filter operator for VeAnalyze/WueAnalyze
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOperator {
    /// Less than (<)
    LessThan,
    /// Greater than (>)
    GreaterThan,
    /// Equal to (=)
    Equal,
    /// Not equal (!=)
    NotEqual,
    /// Bitwise AND (&)
    BitwiseAnd,
    /// Bitwise OR (|)
    BitwiseOr,
}

impl FilterOperator {
    /// Parse a filter operator from INI string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "<" => Some(FilterOperator::LessThan),
            ">" => Some(FilterOperator::GreaterThan),
            "=" => Some(FilterOperator::Equal),
            "!=" => Some(FilterOperator::NotEqual),
            "&" => Some(FilterOperator::BitwiseAnd),
            "|" => Some(FilterOperator::BitwiseOr),
            _ => None,
        }
    }
}

/// VE/WUE/AFR analysis filter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisFilter {
    /// Filter identifier name
    pub name: String,
    /// Human-readable display name
    pub display_name: String,
    /// Output channel to filter on
    pub channel: String,
    /// Comparison operator
    pub operator: FilterOperator,
    /// Default threshold value
    pub default_value: f64,
    /// Whether the user can adjust this filter
    pub user_adjustable: bool,
}

/// VE Analysis configuration (from [VeAnalyze] section)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VeAnalyzeConfig {
    /// VE table to analyze/modify
    pub ve_table_name: String,
    /// Lambda/AFR target table name
    pub target_table_name: String,
    /// Lambda/AFR channel name for live readings
    pub lambda_channel: String,
    /// EGO correction channel name
    pub ego_correction_channel: String,
    /// Active condition expression (e.g., "{ 1 }" = always active)
    pub active_condition: String,
    /// Lambda target tables (primary and optional custom)
    pub lambda_target_tables: Vec<String>,
    /// Analysis filters
    pub filters: Vec<AnalysisFilter>,
    /// Options (e.g., "disableLiveUpdates", "burnOnSend")
    pub options: Vec<String>,
}

/// WUE (Warm-Up Enrichment) Analysis configuration (from [WueAnalyze] section)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WueAnalyzeConfig {
    /// WUE curve name to analyze/modify
    pub wue_curve_name: String,
    /// AFR temperature compensation curve
    pub afr_temp_comp_curve: String,
    /// Lambda/AFR target table name
    pub target_table_name: String,
    /// Lambda/AFR channel name
    pub lambda_channel: String,
    /// Coolant temperature channel
    pub coolant_channel: String,
    /// WUE enrichment channel
    pub wue_channel: String,
    /// EGO correction channel
    pub ego_correction_channel: String,
    /// Lambda target tables
    pub lambda_target_tables: Vec<String>,
    /// Percentage offset (typically 0 or 100)
    pub wue_percent_offset: f64,
    /// Analysis filters
    pub filters: Vec<AnalysisFilter>,
    /// Options
    pub options: Vec<String>,
}

/// Gamma Enrichment Analysis configuration (from [GammaE] section)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GammaEConfig {
    /// Gamma table name
    pub gamma_table_name: String,
    /// Lambda/AFR channel
    pub lambda_channel: String,
    /// Target table name
    pub target_table_name: String,
    /// Analysis filters
    pub filters: Vec<AnalysisFilter>,
    /// Options
    pub options: Vec<String>,
}

/// maintainConstantValue definition - auto-updates constant based on expression
/// Format: maintainConstantValue = constantName, { expression }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainConstantValue {
    /// Name of constant to maintain
    pub constant_name: String,
    /// Expression to evaluate for the new value
    pub expression: String,
}

/// requiresPowerCycle definition - marks constants that need ECU restart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiresPowerCycle {
    /// Name of constant that requires power cycle
    pub constant_name: String,
}
