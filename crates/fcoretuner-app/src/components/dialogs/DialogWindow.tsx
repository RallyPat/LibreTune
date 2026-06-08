//! Dialog Window Component
//!
//! Reusable dialog window wrapper following standard ECU tuning patterns.
//! Provides consistent dialog structure with title, content area, toolbar, and footer.

export interface DialogWindow {
  title: string;
  children: DialogChildren;
  width: number;
  height: number;
  can_close: boolean;
  toolbar_buttons: DialogFooterButton[];
}

export function createDialogWindow(): DialogWindow {
  return {
    title: "",
    children: { type: "SinglePanel", children: [] },
    width: 600,
    height: 400,
    can_close: true,
    toolbar_buttons: [],
  };
}

export type DialogChildren = 
  | { type: "SinglePanel", children: DialogComponent[] };

export type DialogComponent = 
  | { type: "Label", text: string }
  | { type: "Field", label: string, field_type: FieldType, value: string }
  | { type: "Checkbox", label: string, checked: boolean, condition?: string }
  | { type: "Select", label: string, options: SelectOption[], selected: number }
  | { type: "Button", label: string, is_primary: boolean, is_disabled: boolean }
  | { type: "Conditional", condition: string, children: DialogComponent[] };

export type FieldType = "text" | "number" | "expression";

export interface SelectOption {
  value: string;
  label: string;
}

export interface DialogFooterButton {
  label: string;
  is_primary: boolean;
  action: () => void;
}
