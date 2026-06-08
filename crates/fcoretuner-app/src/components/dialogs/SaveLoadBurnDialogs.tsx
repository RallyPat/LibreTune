//! Save Dialog Tune Component
//!
//! Dialog for saving current dialog settings to msqpart file.

export interface SaveDialogTuneSettings {
  tables?: string[];
  settings?: string[];
  save_format: SaveFormat;
  filename: string;
  auto_burn_on_close: boolean;
  auto_burn_on_page_change: boolean;
}

export type SaveFormat = "msq" | "msqpart";

export function createSaveDialogSettings(): SaveDialogTuneSettings {
  return {
    tables: undefined,
    settings: undefined,
    save_format: "msq",
    filename: "untuned",
    auto_burn_on_close: true,
    auto_burn_on_page_change: true,
  };
}

/// Load dialog tune component
export interface LoadDialogTuneSettings {
  filename: string;
  full_tune_only: boolean;
  dialog_tune?: string;
}

export function createLoadDialogSettings(): LoadDialogTuneSettings {
  return {
    filename: "",
    full_tune_only: false,
    dialog_tune: undefined,
  };
}

/// Burn to ECU component
export interface BurnToECUDialog {
  confirm_before_burn: boolean;
  show_burn_progress: boolean;
}

export function createBurnToECUDialog(): BurnToECUDialog {
  return {
    confirm_before_burn: true,
    show_burn_progress: true,
  };
}
