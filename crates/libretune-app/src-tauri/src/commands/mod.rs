//! Tauri command handlers, organized by domain.
//!
//! Each submodule exports a set of `#[tauri::command]` functions that are
//! registered in `lib.rs::run()`. Commands access shared state via
//! `tauri::State<crate::state::AppState>`.

pub mod adaptive_timing;
pub mod cache_status;
pub mod connection;
pub mod dash_files;
pub mod dash_layout;
pub mod autotune_misc;
pub mod debug_realtime;
pub mod realtime_get;
pub mod annotations;
pub mod base_map;
pub mod channels;
pub mod console;
pub mod constants_read;
pub mod csv_io;
pub mod data_logging;
pub mod diagnostic_loggers;
pub mod dyno;
pub mod git;
pub mod hotkeys;
pub mod ini_dialogs;
pub mod ini_meta;
pub mod ini_metadata;
pub mod ini_repository;
pub mod lua;
pub mod math_channels;
pub mod menu;
pub mod metrics;
pub mod online_ini;
pub mod project_listing;
pub mod project_mgmt;
pub mod project_misc;
pub mod project_tune_sync;
pub mod restore_points;
pub mod settings;
pub mod system;
pub mod table_compare;
pub mod table_ops;
pub mod ts_import;
pub mod tune_compare;
pub mod tune_health;
pub mod tune_info;
pub mod tune_io;
pub mod tune_migration;
pub mod tune_misc;
pub mod wasm_plugin;
