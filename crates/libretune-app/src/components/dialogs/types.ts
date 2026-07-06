// Shared types for the DialogRenderer family.
//
// These mirror the shapes returned by the Tauri commands `get_dialog_definition`,
// `get_indicator_panel`, `get_constant`, `get_table_info`, `get_curve_data`,
// and `get_port_editor`.

export interface DialogComponent {
  type: 'Panel' | 'Field' | 'RuntimeValue' | 'LiveGraph' | 'Table' | 'Label' | 'Indicator' | 'CommandButton';
  name?: string;
  label?: string;
  text?: string;
  title?: string;
  position?: string;
  channels?: string[];
  expression?: string;
  label_off?: string;
  label_on?: string;
  visibility_condition?: string;  // Visibility condition (hides field if false)
  enabled_condition?: string;     // Enable condition (disables field if false)
  condition?: string;             // Legacy: single condition (treated as enabled_condition)
  // CommandButton specific fields
  command?: string;               // Command name from [ControllerCommands]
  on_close_behavior?: 'ClickOnCloseIfEnabled' | 'ClickOnCloseIfDisabled' | 'ClickOnClose';
}

export interface DialogDefinition {
  name: string;
  title: string;
  components: DialogComponent[];
}

export interface Constant {
  name: string;
  label?: string;
  units: string;
  digits: number;
  min: number;
  max: number;
  value_type: 'scalar' | 'string' | 'bits' | 'array';
  bit_options: string[];
  help?: string;
  visibility_condition?: string;  // Expression for when field should be visible
  display_offset?: number;  // For bits type: offset to add to displayed value (e.g., +1 for [4:7+1])
}

export interface TableInfo {
  name: string;
  title: string;
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

export interface CurveData {
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

export interface FieldInfo {
  label: string;
  name: string;
  help?: string;
}

export interface IndicatorPanel {
  name: string;
  columns: number;
  visibility_condition?: string;
  indicators: Array<{
    expression: string;
    label_off: string;
    label_on: string;
    color_off_fg?: string;
    color_off_bg?: string;
    color_on_fg?: string;
    color_on_bg?: string;
  }>;
}

export interface PortEditorConfig {
  name: string;
  label: string;
  enable_condition?: string;
}

/// Returns true if `value` is an in-progress numeric edit (empty, just minus,
/// just decimal point) that should be tolerated by the input field without
/// being parsed as a final number.
export function isIncompleteNumericInput(value: string): boolean {
  return value === '' || value === '-' || value === '.';
}

/// Synthetic placeholder definitions for `std_*` panel names that LibreTune
/// doesn't ship — surfaces a friendly Label component instead of an error.
export function buildStdPlaceholderDefinition(name: string): DialogDefinition | null {
  if (name === 'std_injection') {
    return {
      name,
      title: 'Injection Setup',
      components: [
        {
          type: 'Label',
          text:
            'This dashboard references the standard "std_injection" panel. LibreTune does not bundle that legacy panel, but you can configure injectors via the Engine Constants and Injector Characteristics dialogs.',
        },
      ],
    };
  }

  if (name.startsWith('std_')) {
    return {
      name,
      title: `Standard Panel: ${name.replace(/^std_/, '')}`,
      components: [
        {
          type: 'Label',
          text: `Panel "${name}" is a standard TunerStudio shortcut. LibreTune doesn't ship this panel yet; please open the related dialog from the menu instead.`,
        },
      ],
    };
  }

  return null;
}
