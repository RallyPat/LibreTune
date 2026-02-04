//! Action Management System
//!
//! Implements action management with triggers and user-defined actions.
//! Features:
//! - Event triggers (execute actions based on custom conditions)
//! - User actions (create personal actions with custom parameters)
//! - Built-in actions (exit, open project, mark log, play sound, etc.)

export type TargetActionType = 
  | "exit_and_shutdown"
  | "open_project"
  | "close_project"
  | "move_dash_left"
  | "move_dash_right"
  | "start_data_logging"
  | "stop_data_logging"
  | "toggle_data_logging"
  | "user_action";

export interface ActionTrigger {
  id: string;
  name: string;
  description: string;
  condition: string;
  targetAction: TargetActionType;
  actionId?: string;
}

export interface UserAction {
  id: string;
  name: string;
  description: string;
  actionType: UserActionType;
  params?: Record<string, any>;
}

export type UserActionType = 
  | "execute_shell_command"
  | "load_tune_or_partial_tune"
  | "mark_data_log_comment"
  | "open_project"
  | "reset_runtime_value"
  | "send_controller_command"
  | "show_global_warning"
  | "show_passive_message"
  | "show_settings_dialog"
  | "lua_script";

export interface ActionManagementDialog {
  triggers: ActionTrigger[];
  userActions: UserAction[];
}

export function newActionManagement(): ActionManagementDialog {
  return {
    triggers: [],
    userActions: [],
  };
}
