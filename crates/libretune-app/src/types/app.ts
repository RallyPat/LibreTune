// Shared application/backend types extracted from App.tsx
import type { TableData as TunerTableData } from "../components/tuner-ui";
import type { CurveData, SimpleGaugeInfo } from "../components/curves/CurveEditor";
import type { DialogDefinition as RendererDialogDef } from "../components/dialogs/DialogRenderer";
import type { SignatureMismatchInfo } from "../components/dialogs/SignatureMismatchDialog";

// Backend types
export interface ConnectionStatus {
  state: "Connected" | "Connecting" | "Disconnected" | "Error";
  signature: string | null;
  has_definition: boolean;
  ini_name?: string | null;
}

export interface ConnectResult {
  signature: string;
  mismatch_info: SignatureMismatchInfo | null;
}

export interface SyncResult {
  success: boolean;
  pages_synced: number;
  pages_failed: number;
  total_pages: number;
  errors: string[];
}

export interface SyncStatus {
  pages_synced: number;
  pages_failed: number;
  total_pages: number;
  errors: string[];
}

export interface CurrentProject {
  name: string;
  path: string;
  signature: string;
  has_tune: boolean;
  tune_modified: boolean;
  connection: {
    port: string | null;
    baud_rate: number;
    auto_connect: boolean;
  };
}

export interface IniCapabilities {
  has_constants: boolean;
  has_output_channels: boolean;
  has_tables: boolean;
  has_curves: boolean;
  has_gauges: boolean;
  has_frontpage: boolean;
  has_dialogs: boolean;
  has_help_topics: boolean;
  has_setting_groups: boolean;
  has_pc_variables: boolean;
  has_default_values: boolean;
  has_datalog_entries: boolean;
  has_datalog_views: boolean;
  has_logger_definitions: boolean;
  has_controller_commands: boolean;
  has_port_editors: boolean;
  has_reference_tables: boolean;
  has_key_actions: boolean;
  has_ve_analyze: boolean;
  has_wue_analyze: boolean;
  has_gamma_e: boolean;
  supports_console: boolean;
  dfu_command_name?: string | null;
  lua_script_constant?: string | null;
}

export interface ProjectInfo {
  name: string;
  path: string;
  signature: string;
  modified: string;
}

export interface IniEntry {
  id: string;
  name: string;
  signature: string;
  path: string;
}

export interface BackendTableData {
  name: string;
  title: string;
  x_axis_name?: string;
  y_axis_name?: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_output_channel?: string | null;
  y_output_channel?: string | null;
}

export interface BackendCurveData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  x_label: string;
  y_label: string;
  x_axis?: [number, number, number] | null;
  y_axis?: [number, number, number] | null;
  x_output_channel?: string | null;
  gauge?: string | null;
}

export interface ChannelInfo {
  name: string;
  label?: string | null;
  units: string;
  scale: number;
  translate: number;
}

export const toTunerTableData = (data: BackendTableData): TunerTableData => ({
  name: data.name,
  xAxis: data.x_bins,
  yAxis: data.y_bins,
  zValues: data.z_values,
  xLabel: data.x_axis_name || 'X',
  yLabel: data.y_axis_name || 'Y',
  xOutputChannel: data.x_output_channel ?? undefined,
  yOutputChannel: data.y_output_channel ?? undefined,
});

export const toCurveData = (data: BackendCurveData): CurveData => ({
  name: data.name,
  title: data.title,
  x_bins: data.x_bins,
  y_bins: data.y_bins,
  x_label: data.x_label,
  y_label: data.y_label,
  x_axis: data.x_axis,
  y_axis: data.y_axis,
  x_output_channel: data.x_output_channel,
  gauge: data.gauge,
});

export interface BackendMenu {
  name: string;
  title: string;
  items: BackendMenuItem[];
}

export interface BackendMenuItem {
  type: "SubMenu" | "Table" | "Dialog" | "Separator" | "Std" | "Help";
  label?: string;
  target?: string;
  condition?: string;
  items?: BackendMenuItem[];
  /** Whether item is visible (evaluated from visibility_condition) */
  visible?: boolean;
  /** Whether item is enabled (evaluated from enabled_condition) */
  enabled?: boolean;
  /** Original visibility condition expression for tooltip */
  visibility_condition?: string;
}

// Protocol defaults fetched from loaded INI
export interface ProtocolDefaults {
  default_baud_rate: number;
  inter_write_delay: number;
  delay_after_port_open: number;
  message_envelope_format?: string | null;
  page_activation_delay: number;
  timeout_ms: number;
}

// PortEditor configuration from backend
export interface PortEditorConfig {
  name: string;
  label: string;
  enable_condition?: string;
}

// Tab content types
export interface TabContent {
  type: "dashboard" | "table" | "curve" | "dialog" | "portEditor" | "settings" | "project" | "autotune" | "datalog" | "tooth-logger" | "composite-logger" | "console" | "lua-console" | "och-status";
  data?: TunerTableData | RendererDialogDef | PortEditorConfig | CurveData | string;
  gauge?: SimpleGaugeInfo | null; // For curve tabs with associated gauges
  /** Search term to highlight within the content (e.g., matching field labels in dialogs) */
  highlightTerm?: string;
}
