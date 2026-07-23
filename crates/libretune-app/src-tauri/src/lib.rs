//! LibreTune Tauri application entry point.
//!
//! This module wires together the Tauri runtime, application state, and
//! command handlers. All command implementations live in dedicated modules
//! under `commands/`. State definitions live in `state.rs`. This file
//! intentionally contains no business logic — only registration glue.

use libretune_core::autotune::AutoTuneState;
use libretune_core::datalog::DataLogger;
use libretune_core::project::OnlineIniRepository;
use tokio::sync::Mutex;

mod commands;
mod paths;
mod port_editor; // used by commands/ini_dialogs.rs
mod state;

use state::{AppState, AutoTuneLoadSource, RpmState, RpmStateTracker, StreamStats};

// Re-exports for cross-module use within the crate.
pub(crate) use commands::app_settings::{
    get_commit_message_format, load_settings, save_settings, Settings,
};
pub(crate) use commands::signature_helpers::{
    call_connection_factory_and_build_result, find_matching_inis_internal,
};
pub(crate) use commands::table_internals::{
    get_table_data_internal, update_constant_array_internal, update_table_z_values_internal,
    TableData,
};
pub(crate) use commands::types::{
    ConnectResult, ConnectionSettingsResponse, ConnectionStatus, ConstantInfo, CurrentProjectInfo,
    MatchingIniInfo, SignatureMatchType, SignatureMismatchInfo, SyncProgress, SyncResult,
};
pub(crate) use commands::util_helpers::{
    clean_axis_label, get_conn_lock_holder, parse_runtime_packet_mode, read_raw_value,
    set_conn_lock_holder, stream_log,
};

// Tauri command imports — sorted alphabetically by module name.
use commands::adaptive_timing::{
    disable_adaptive_timing, enable_adaptive_timing, get_adaptive_timing_stats,
};
use commands::annotations::{
    delete_annotation, get_all_annotations, get_annotation, get_table_annotations, set_annotation,
};
use commands::apply_base_map::apply_base_map;
use commands::autotune_misc::{
    burn_autotune_recommendations, get_autotune_heatmap, get_autotune_recommendations,
    lock_autotune_cells, send_autotune_recommendations, stop_autotune, unlock_autotune_cells,
};
use commands::available_inis::get_available_inis;
use commands::base_map::generate_base_map;
use commands::cache_status::{get_table_info, get_tune_cache_status};
use commands::channels::{
    get_available_channels, get_output_channel_status, get_status_bar_defaults,
};
use commands::connect_to_ecu::connect_to_ecu;
use commands::connection::{auto_load_last_ini, disconnect_ecu, get_connection_status};
use commands::console::{
    clear_console_history, get_console_history, get_ecu_type, send_console_command,
};
use commands::constant_update::update_constant;
use commands::constant_values::get_all_constant_values;
use commands::constants_read::{get_constant, get_constant_string_value, get_constant_value};
use commands::csv_io::{export_tune_as_csv, import_tune_from_csv, reset_tune_to_defaults};
use commands::curve_ops::{get_curve_data, update_curve_data};
use commands::dash_files::{
    create_new_dashboard, delete_dashboard, duplicate_dashboard, export_dashboard, get_dash_file,
    rename_dashboard, save_dash_file, validate_dashboard,
};
use commands::dash_layout::{
    check_dash_conflict, get_dashboard_templates, import_dash_file, list_available_dashes,
    reset_dashboards_to_defaults,
};
use commands::data_logging::{
    clear_log, get_log_entries, get_logging_status, read_text_file, save_log, start_logging,
    stop_logging, write_text_file,
};
use commands::debug_realtime::debug_single_realtime_read;
use commands::demo::{get_demo_mode, set_demo_mode};
use commands::diagnostic_loggers::{
    start_composite_logger, start_tooth_logger, stop_composite_logger, stop_tooth_logger,
};
use commands::dyno::{compare_dyno_runs, detect_dyno_headers, load_dyno_run};
use commands::find_inis::find_matching_inis;
use commands::firmware_update::{
    get_firmware_flasher_info, get_firmware_update_guidance, recover_ecu_firmware_dfu,
    release_serial_port_blockers, suggest_firmware_companion, update_ecu_firmware,
};
use commands::get_table_data::get_table_data;
use commands::git::{
    git_checkout, git_commit, git_create_branch, git_current_branch, git_diff, git_has_changes,
    git_has_repo, git_history, git_init_project, git_list_branches, git_switch_branch,
};
use commands::hotkeys::{
    get_hotkey_bindings, is_onboarding_completed, mark_onboarding_completed, save_hotkey_bindings,
};
use commands::ini_dialogs::{
    evaluate_expression, get_dialog_definition, get_help_topic, get_indicator_panel,
    get_port_editor, get_port_editor_assignments, get_readout_panel, save_port_editor_assignments,
};
use commands::ini_meta::{
    get_curves, get_frontpage, get_gauge_config, get_gauge_configs, get_tables,
};
use commands::ini_metadata::{
    get_ini_capabilities, get_protocol_capabilities, get_protocol_defaults, get_ve_analyze_config,
};
use commands::ini_repository::{
    import_ini, init_ini_repository, list_repository_inis, remove_ini, scan_for_inis,
};
use commands::load_ini::load_ini;
use commands::load_pages::load_all_pages;
use commands::load_tune::load_tune;
use commands::lua::run_lua_script;
use commands::math_channels::{
    delete_math_channel, get_math_channels, set_math_channel, validate_math_expression,
};
use commands::menu::{get_menu_tree, get_searchable_index};
use commands::metrics::stop_metrics_task;
use commands::online_ini::{check_internet_connectivity, download_ini, search_online_inis};
use commands::project_lifecycle::{create_project, open_project};
use commands::project_listing::{get_projects_path, list_projects};
use commands::project_mgmt::{
    close_project, get_current_project, update_project_auto_connect, update_project_connection,
};
use commands::project_misc::{delete_project, get_msq_info};
use commands::project_tune_sync::{
    compare_project_and_ecu_tunes, mark_tune_modified, save_tune_to_project,
    write_project_tune_to_ecu,
};
use commands::realtime_get::get_realtime_data;
use commands::realtime_stop::stop_realtime_stream;
use commands::realtime_stream::start_realtime_stream;
use commands::restore_points::{
    create_restore_point, delete_restore_point, list_restore_points, load_restore_point,
};
use commands::save_tune::{save_tune, save_tune_as};
use commands::settings::{get_settings, update_heatmap_custom_stops, update_setting};
use commands::start_autotune::start_autotune;
use commands::sync_ecu_data::sync_ecu_data;
use commands::system::{get_build_info, get_serial_ports};
use commands::table_compare::compare_tables;
use commands::table_ops::{
    add_offset, fill_region, interpolate_cells, interpolate_linear, rebin_table, scale_cells,
    set_cells_equal, smooth_table,
};
use commands::table_update::update_table_data;
use commands::ts_import::{import_tunerstudio_project, preview_tunerstudio_import};
use commands::tune_compare::{compare_tune_files, merge_from_tune};
use commands::tune_health::{
    get_dyno_table_overlay, get_predicted_fills, get_tune_anomalies, get_tune_health_report,
};
use commands::tune_info::{get_tune_info, new_tune};
use commands::tune_io::{burn_to_ecu, execute_controller_command, list_tune_files};
use commands::tune_migration::{
    clear_migration_report, get_migration_report, get_tune_constant_manifest, get_tune_ini_metadata,
};
use commands::tune_misc::{update_constant_string, use_ecu_tune, use_project_tune};
use commands::update_project_ini::update_project_ini;
use commands::wasm_plugin::{
    execute_wasm_plugin, get_wasm_plugin_info, list_wasm_plugins, load_wasm_plugin,
    unload_wasm_plugin,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::new()),
            autotune_secondary_state: Mutex::new(AutoTuneState::new()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            // Background task for connection metrics emission
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            tune_mismatch_snapshot: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            connection_factory: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
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
            get_ecu_type,
            send_console_command,
            get_console_history,
            clear_console_history,
            load_ini,
            get_realtime_data,
            debug_single_realtime_read,
            start_realtime_stream,
            stop_realtime_stream,
            get_table_data,
            get_table_info,
            get_curve_data,
            get_tables,
            get_curves,
            get_gauge_configs,
            get_gauge_config,
            get_available_channels,
            get_output_channel_status,
            get_status_bar_defaults,
            get_frontpage,
            update_table_data,
            update_curve_data,
            get_menu_tree,
            get_searchable_index,
            get_dialog_definition,
            get_indicator_panel,
            get_readout_panel,
            get_port_editor,
            get_port_editor_assignments,
            save_port_editor_assignments,
            // Math Channels
            get_math_channels,
            set_math_channel,
            delete_math_channel,
            validate_math_expression,
            // INI / protocol defaults
            get_protocol_defaults,
            get_protocol_capabilities,
            get_ini_capabilities,
            get_ve_analyze_config,
            get_help_topic,
            get_build_info,
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
            get_predicted_fills,
            get_tune_anomalies,
            get_tune_health_report,
            compare_tune_files,
            merge_from_tune,
            set_annotation,
            get_annotation,
            get_table_annotations,
            delete_annotation,
            get_all_annotations,
            load_dyno_run,
            detect_dyno_headers,
            compare_dyno_runs,
            get_dyno_table_overlay,
            rebin_table,
            smooth_table,
            interpolate_cells,
            interpolate_linear,
            add_offset,
            fill_region,
            scale_cells,
            set_cells_equal,
            get_dashboard_templates,
            get_dash_file,
            validate_dashboard,
            save_dash_file,
            list_available_dashes,
            reset_dashboards_to_defaults,
            check_dash_conflict,
            import_dash_file,
            create_new_dashboard,
            rename_dashboard,
            duplicate_dashboard,
            export_dashboard,
            delete_dashboard,
            // Tune file commands
            get_tune_info,
            new_tune,
            save_tune,
            save_tune_as,
            load_tune,
            get_migration_report,
            clear_migration_report,
            get_tune_ini_metadata,
            get_tune_constant_manifest,
            list_tune_files,
            burn_to_ecu,
            execute_controller_command,
            get_firmware_flasher_info,
            get_firmware_update_guidance,
            suggest_firmware_companion,
            update_ecu_firmware,
            recover_ecu_firmware_dfu,
            release_serial_port_blockers,
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
            read_text_file,
            write_text_file,
            // Diagnostic commands (stubs)
            start_tooth_logger,
            stop_tooth_logger,
            start_composite_logger,
            stop_composite_logger,
            compare_tables,
            reset_tune_to_defaults,
            export_tune_as_csv,
            import_tune_from_csv,
            // Project management commands
            get_projects_path,
            list_projects,
            create_project,
            open_project,
            close_project,
            get_current_project,
            update_project_connection,
            update_project_auto_connect,
            // Restore points commands
            create_restore_point,
            list_restore_points,
            load_restore_point,
            delete_restore_point,
            // TunerStudio import
            preview_tunerstudio_import,
            import_tunerstudio_project,
            // Git version control commands
            git_init_project,
            git_has_repo,
            git_commit,
            git_history,
            git_diff,
            git_checkout,
            git_list_branches,
            git_create_branch,
            git_switch_branch,
            git_current_branch,
            git_has_changes,
            // Base map generator commands
            generate_base_map,
            apply_base_map,
            get_msq_info,
            delete_project,
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
            get_hotkey_bindings,
            save_hotkey_bindings,
            mark_onboarding_completed,
            is_onboarding_completed,
            update_heatmap_custom_stops,
            update_constant_string,
            run_lua_script,
            // WASM Plugin commands
            load_wasm_plugin,
            unload_wasm_plugin,
            list_wasm_plugins,
            execute_wasm_plugin,
            get_wasm_plugin_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
