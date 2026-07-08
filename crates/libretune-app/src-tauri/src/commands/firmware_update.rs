//! ECU firmware update via DFU or OpenBLT bootloaders.

use crate::commands::metrics::stop_metrics_task;
use crate::commands::tune_io::{resolve_controller_command, send_controller_command_bytes};
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareFlasherInfo {
    pub stm32_programmer_cli: Option<String>,
    pub dfu_util: Option<String>,
    pub bootcommander: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareUpdateResult {
    pub success: bool,
    pub log: Vec<String>,
    pub message: String,
    pub should_reconnect: bool,
}

#[derive(Debug, Clone, Serialize)]
struct FirmwareUpdateLogEvent {
    line: String,
}

fn push_log(app: &AppHandle, log: &mut Vec<String>, line: impl Into<String>) {
    let line = line.into();
    let _ = app.emit(
        "firmware-update:log",
        FirmwareUpdateLogEvent { line: line.clone() },
    );
    log.push(line);
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(name);
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

#[cfg(windows)]
fn find_stm32_programmer_cli() -> Option<PathBuf> {
    if let Some(found) = find_on_path("STM32_Programmer_CLI.exe") {
        return Some(found);
    }
    for base in ["C:\\Program Files", "C:\\Program Files (x86)"] {
        let candidate = PathBuf::from(base)
            .join("STMicroelectronics")
            .join("STM32Cube")
            .join("STM32CubeProgrammer")
            .join("bin")
            .join("STM32_Programmer_CLI.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(not(windows))]
fn find_stm32_programmer_cli() -> Option<PathBuf> {
    find_on_path("STM32_Programmer_CLI")
}

fn find_dfu_util() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        find_on_path("dfu-util.exe").or_else(|| find_on_path("dfu-util"))
    }
    #[cfg(not(windows))]
    {
        find_on_path("dfu-util")
    }
}

fn find_bootcommander() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        find_on_path("BootCommander.exe").or_else(|| find_on_path("BootCommander"))
    }
    #[cfg(not(windows))]
    {
        find_on_path("BootCommander")
    }
}

/// Detect available external flash tools on the system.
#[tauri::command]
pub async fn get_firmware_flasher_info() -> Result<FirmwareFlasherInfo, String> {
    Ok(FirmwareFlasherInfo {
        stm32_programmer_cli: find_stm32_programmer_cli().map(|p| p.display().to_string()),
        dfu_util: find_dfu_util().map(|p| p.display().to_string()),
        bootcommander: find_bootcommander().map(|p| p.display().to_string()),
    })
}

fn resolve_bootloader_command(
    def: &libretune_core::ini::EcuDefinition,
    method: &str,
) -> Result<String, String> {
    match method {
        "dfu" => def
            .controller_commands
            .keys()
            .find(|k| k.eq_ignore_ascii_case("cmd_dfu"))
            .cloned()
            .ok_or_else(|| "This INI has no cmd_dfu controller command".to_string()),
        "openblt" => def
            .controller_commands
            .keys()
            .find(|k| k.eq_ignore_ascii_case("cmd_openblt"))
            .cloned()
            .ok_or_else(|| "This INI has no cmd_openblt controller command".to_string()),
        other => Err(format!("Unknown firmware update method: {}", other)),
    }
}

fn run_command_capture(tool: &Path, args: &[&str]) -> Result<(bool, String), String> {
    let output = Command::new(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run {}: {}", tool.display(), e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    Ok((output.status.success(), combined))
}

fn detect_stm32_usb_port(cli: &Path) -> Option<String> {
    let (_, listing) = run_command_capture(cli, &["-l"]).ok()?;
    for line in listing.lines() {
        let trimmed = line.trim();
        if trimmed.contains("USB") && trimmed.contains(':') {
            if let Some(port) = trimmed.split(':').nth(1).map(str::trim) {
                if port.starts_with("USB") {
                    return Some(port.to_string());
                }
            }
        }
        if trimmed.starts_with("USB") && trimmed.len() <= 8 {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn firmware_extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn parse_flash_address(value: Option<&str>) -> Result<Option<u32>, String> {
    let Some(raw) = value.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let normalized = raw.strip_prefix("0x").unwrap_or(raw);
    u32::from_str_radix(normalized, 16)
        .map(Some)
        .map_err(|_| format!("Invalid flash address: {}", raw))
}

fn default_bin_flash_address() -> u32 {
    // rusEFI / epicEFI application region when a bootloader occupies the first 32 KB.
    0x0800_8000
}

const OPENBLT_BOOTLOADER_ADDRESS: u32 = 0x0800_0000;

fn stm32_programmer_port(cli: &Path) -> String {
    detect_stm32_usb_port(cli).unwrap_or_else(|| "USB1".to_string())
}

fn stm32_full_chip_erase(cli: &Path, port: &str) -> Result<String, String> {
    let port_arg = format!("port={}", port);
    let args = ["-c", port_arg.as_str(), "-e", "all"];
    let (ok, output) = run_command_capture(cli, &args)?;
    if ok {
        Ok(output)
    } else {
        Err(format!(
            "Full chip erase failed (port={}):\n{}",
            port, output
        ))
    }
}

fn flash_with_stm32_programmer(
    cli: &Path,
    firmware_path: &Path,
    bin_address: Option<u32>,
) -> Result<String, String> {
    let port = stm32_programmer_port(cli);
    let port_arg = format!("port={}", port);
    let firmware = firmware_path
        .to_str()
        .ok_or_else(|| "Invalid firmware path".to_string())?;

    let ext = firmware_extension(firmware_path);
    let address_arg = if ext == "bin" {
        let address = bin_address.ok_or(
            "Binary (.bin) files require a flash start address (default: 0x08008000 for rusEFI/epicEFI)",
        )?;
        format!("0x{:08X}", address)
    } else {
        String::new()
    };

    let (ok, output) = if ext == "bin" {
        let args = [
            "-c",
            port_arg.as_str(),
            "-w",
            firmware,
            address_arg.as_str(),
            "-v",
            "-s",
        ];
        run_command_capture(cli, &args)?
    } else {
        let args = ["-c", port_arg.as_str(), "-w", firmware, "-v", "-s"];
        run_command_capture(cli, &args)?
    };

    if ok {
        Ok(output)
    } else {
        Err(format!(
            "STM32_Programmer_CLI failed (port={}):\n{}",
            port, output
        ))
    }
}

fn flash_with_dfu_util(
    tool: &Path,
    firmware_path: &Path,
    bin_address: Option<u32>,
) -> Result<String, String> {
    let firmware = firmware_path
        .to_str()
        .ok_or_else(|| "Invalid firmware path".to_string())?;

    let ext = firmware_extension(firmware_path);
    let (ok, output) = if ext == "bin" {
        let address = bin_address.ok_or(
            "Binary (.bin) files require a flash start address (default: 0x08008000 for rusEFI/epicEFI)",
        )?;
        let sector = format!("0x{:X}:leave", address);
        let args = ["-a", "0", "-s", sector.as_str(), "-D", firmware];
        run_command_capture(tool, &args)?
    } else {
        // .dfu / .hex images embed the correct load address.
        let args = ["-a", "0", "-s", ":leave", "-D", firmware];
        run_command_capture(tool, &args)?
    };

    if ok {
        Ok(output)
    } else {
        Err(format!("dfu-util failed:\n{}", output))
    }
}

fn flash_with_bootcommander(
    tool: &Path,
    firmware_path: &Path,
    serial_port: &str,
    baud_rate: u32,
) -> Result<String, String> {
    let firmware = firmware_path
        .to_str()
        .ok_or_else(|| "Invalid firmware path".to_string())?;
    let device_arg = format!("-d={}", serial_port);
    let baud_arg = format!("-b={}", baud_rate);

    let args = vec![
        "-s=xcp",
        "-t=xcp_rs232",
        device_arg.as_str(),
        baud_arg.as_str(),
        firmware,
    ];
    let (ok, output) = run_command_capture(tool, &args)?;
    if ok {
        Ok(output)
    } else {
        Err(format!("BootCommander failed:\n{}", output))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareUpdateGuidance {
    pub recommended_method: String,
    pub file_kind: String,
    pub risk_level: String,
    pub warnings: Vec<String>,
    pub requires_risk_acknowledgement: bool,
    pub suggested_file_hint: String,
    pub openblt_available: bool,
    pub dfu_available: bool,
}

fn ini_has_command(def: &libretune_core::ini::EcuDefinition, name: &str) -> bool {
    def.controller_commands
        .keys()
        .any(|k| k.eq_ignore_ascii_case(name))
}

fn classify_firmware_file(path: &Path, method: &str) -> FirmwareUpdateGuidance {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = firmware_extension(path);

    let file_kind = if file_name.contains("_update") && matches!(ext.as_str(), "srec" | "s19") {
        "update_srec"
    } else if ext == "dfu" {
        "dfu_package"
    } else if ext == "bin" {
        "raw_bin"
    } else if matches!(ext.as_str(), "srec" | "s19") {
        "srec"
    } else if ext == "hex" {
        "hex"
    } else {
        "unknown"
    };

    let mut warnings = Vec::new();
    let mut risk_level = "low";
    let mut requires_risk_acknowledgement = false;

    match (method, file_kind) {
        ("dfu", "raw_bin") => {
            risk_level = "high";
            requires_risk_acknowledgement = true;
            warnings.push(
                "Raw .bin via DFU only writes the application region (typically 0x08008000)."
                    .into(),
            );
            warnings.push(
                "If OpenBLT at 0x08000000 is missing or corrupt, the ECU will not boot.".into(),
            );
            warnings.push(
                "Prefer OpenBLT + *_update.srec, a deliver/ .dfu package, or DFU recovery mode."
                    .into(),
            );
        }
        ("dfu", "srec" | "update_srec") => {
            risk_level = "medium";
            warnings.push(
                "DFU with .srec may not update the OpenBLT bootloader region correctly.".into(),
            );
            warnings.push("Use OpenBLT + *_update.srec for routine updates when available.".into());
        }
        ("dfu", "dfu_package") => {
            risk_level = "low";
        }
        ("openblt", "update_srec") => {
            risk_level = "low";
        }
        ("openblt", "srec") => {
            risk_level = "medium";
            warnings.push(
                "This .srec may not be an OpenBLT update bundle. Prefer rusefi_*_update.srec from deliver/."
                    .into(),
            );
        }
        ("openblt", "hex") => {
            risk_level = "medium";
            warnings.push("Verify this .hex is built for OpenBLT before flashing.".into());
        }
        ("openblt", "raw_bin" | "dfu_package" | "unknown") => {
            risk_level = "high";
            requires_risk_acknowledgement = true;
            warnings.push("This file type is not recommended for OpenBLT updates.".into());
            warnings.push("Use rusefi_*_update.srec from your firmware deliver/ folder.".into());
        }
        _ => {}
    }

    if file_name.contains("rusefi.bin") || file_name.ends_with(".bin") && method == "dfu" {
        if risk_level == "low" {
            risk_level = "medium";
        }
    }

    let suggested_file_hint = if method == "openblt" {
        "deliver/rusefi_*_update.srec".into()
    } else {
        "deliver/*.dfu (or use DFU recovery for OpenBLT + rusefi.bin)".into()
    };

    FirmwareUpdateGuidance {
        recommended_method: "openblt".into(),
        file_kind: file_kind.into(),
        risk_level: risk_level.into(),
        warnings,
        requires_risk_acknowledgement,
        suggested_file_hint,
        openblt_available: false,
        dfu_available: false,
    }
}

/// Analyze a firmware file and return safety guidance for the chosen update method.
#[tauri::command]
pub async fn get_firmware_update_guidance(
    state: tauri::State<'_, AppState>,
    firmware_path: Option<String>,
    method: String,
) -> Result<FirmwareUpdateGuidance, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No INI definition loaded")?;
    let openblt_available = ini_has_command(def, "cmd_openblt");
    let dfu_available = ini_has_command(def, "cmd_dfu");
    drop(def_guard);

    let mut guidance = if let Some(path) = firmware_path.filter(|p| !p.is_empty()) {
        classify_firmware_file(Path::new(&path), &method)
    } else {
        FirmwareUpdateGuidance {
            recommended_method: if openblt_available {
                "openblt".into()
            } else {
                "dfu".into()
            },
            file_kind: "none".into(),
            risk_level: "low".into(),
            warnings: Vec::new(),
            requires_risk_acknowledgement: false,
            suggested_file_hint: if openblt_available {
                "deliver/rusefi_*_update.srec".into()
            } else {
                "deliver/*.dfu".into()
            },
            openblt_available,
            dfu_available,
        }
    };

    guidance.openblt_available = openblt_available;
    guidance.dfu_available = dfu_available;
    if openblt_available {
        guidance.recommended_method = "openblt".into();
    } else if dfu_available {
        guidance.recommended_method = "dfu".into();
    }

    if method != guidance.recommended_method && guidance.risk_level == "low" {
        guidance.warnings.push(format!(
            "This ECU supports {} — that is the recommended update method.",
            if guidance.recommended_method == "openblt" {
                "OpenBLT + *_update.srec"
            } else {
                "DFU"
            }
        ));
        guidance.risk_level = "medium".into();
    }

    Ok(guidance)
}

fn validate_firmware_file(path: &Path, method: &str) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("Firmware file not found: {}", path.display()));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match method {
        "dfu" if !matches!(ext.as_str(), "dfu" | "hex" | "bin" | "s19" | "srec") => {
            return Err(
                "DFU update expects a .dfu, .hex, .bin, or .srec firmware file".to_string(),
            );
        }
        "openblt" if !matches!(ext.as_str(), "srec" | "s19" | "hex") => {
            return Err("OpenBLT update expects a .srec or .hex firmware file".to_string());
        }
        _ => {}
    }
    Ok(())
}

/// Full firmware update workflow: enter bootloader, flash, return tool output.
#[tauri::command]
pub async fn update_ecu_firmware(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    firmware_path: String,
    method: String,
    bin_flash_address: Option<String>,
    acknowledge_risk: bool,
) -> Result<FirmwareUpdateResult, String> {
    let path = PathBuf::from(&firmware_path);
    validate_firmware_file(&path, &method)?;

    let guidance = classify_firmware_file(&path, &method);
    if guidance.requires_risk_acknowledgement && !acknowledge_risk {
        return Err(
            "High-risk firmware update blocked. Read the warnings and confirm you understand the risks before proceeding.".to_string(),
        );
    }

    let ext = firmware_extension(&path);
    let resolved_bin_address = if ext == "bin" {
        Some(
            parse_flash_address(bin_flash_address.as_deref())?
                .unwrap_or_else(default_bin_flash_address),
        )
    } else {
        None
    };

    let command_name = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("No INI definition loaded")?;
        resolve_bootloader_command(def, &method)?
    };

    let mut log = Vec::new();
    push_log(&app, &mut log, "Preparing firmware update…");

    let bytes = resolve_controller_command(&state, &command_name).await?;
    push_log(&app, &mut log, format!("Sending {} to ECU…", command_name));
    send_controller_command_bytes(&state, &bytes).await?;

    stop_metrics_task(state.clone()).await;
    {
        let mut conn_guard = state.connection.lock().await;
        *conn_guard = None;
    }
    push_log(
        &app,
        &mut log,
        "Disconnected — waiting for bootloader USB device…",
    );
    sleep(Duration::from_secs(3)).await;

    let flash_output = match method.as_str() {
        "dfu" => {
            if let Some(cli) = find_stm32_programmer_cli() {
                if ext == "bin" {
                    push_log(
                        &app,
                        &mut log,
                        format!(
                            "Flashing .bin at 0x{:08X} with {}…",
                            resolved_bin_address.unwrap_or(default_bin_flash_address()),
                            cli.display()
                        ),
                    );
                } else {
                    push_log(&app, &mut log, format!("Flashing with {}…", cli.display()));
                }
                flash_with_stm32_programmer(&cli, &path, resolved_bin_address)?
            } else if let Some(tool) = find_dfu_util() {
                push_log(&app, &mut log, format!("Flashing with {}…", tool.display()));
                flash_with_dfu_util(&tool, &path, resolved_bin_address)?
            } else {
                return Err(
                    "No DFU flasher found. Install STM32CubeProgrammer (STM32_Programmer_CLI) or dfu-util and ensure it is on PATH.".to_string(),
                );
            }
        }
        "openblt" => {
            let tool = find_bootcommander().ok_or(
                "BootCommander not found. Install OpenBLT BootCommander and ensure it is on PATH.",
            )?;
            let (serial_port, baud_rate) = {
                let project_guard = state.current_project.lock().await;
                let project = project_guard
                    .as_ref()
                    .ok_or("No project loaded — cannot determine serial port")?;
                let port = project
                    .config
                    .connection
                    .port
                    .clone()
                    .ok_or("Project has no serial port configured")?;
                let baud = project.config.connection.baud_rate;
                (port, baud)
            };
            push_log(
                &app,
                &mut log,
                format!(
                    "Flashing with {} on {} @ {} baud…",
                    tool.display(),
                    serial_port,
                    baud_rate
                ),
            );
            flash_with_bootcommander(&tool, &path, &serial_port, baud_rate)?
        }
        other => return Err(format!("Unknown firmware update method: {}", other)),
    };

    for line in flash_output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        push_log(&app, &mut log, line);
    }

    let message = if method == "dfu" {
        "Firmware update complete. Power-cycle the ECU, then reconnect and verify the new firmware version."
    } else {
        "Firmware update complete. The ECU should reboot automatically — reconnect and verify the new firmware version."
    };

    push_log(&app, &mut log, message);
    Ok(FirmwareUpdateResult {
        success: true,
        log,
        message: message.to_string(),
        should_reconnect: method == "openblt",
    })
}

fn validate_recovery_image(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("{} file not found: {}", label, path.display()));
    }
    let ext = firmware_extension(path);
    if !matches!(ext.as_str(), "bin" | "hex") {
        return Err(format!(
            "{} must be a .bin or .hex file (got .{})",
            label, ext
        ));
    }
    Ok(())
}

fn flash_recovery_with_stm32_programmer(
    cli: &Path,
    bootloader_path: &Path,
    app_path: &Path,
    app_address: u32,
    full_erase: bool,
) -> Result<String, String> {
    let port = stm32_programmer_port(cli);
    let port_arg = format!("port={}", port);
    let bootloader = bootloader_path
        .to_str()
        .ok_or_else(|| "Invalid bootloader path".to_string())?;
    let app_firmware = app_path
        .to_str()
        .ok_or_else(|| "Invalid application firmware path".to_string())?;
    let bootloader_addr = format!("0x{:08X}", OPENBLT_BOOTLOADER_ADDRESS);
    let app_addr = format!("0x{:08X}", app_address);

    let mut args: Vec<&str> = vec!["-c", port_arg.as_str()];
    if full_erase {
        args.extend(["-e", "all"]);
    }

    match (
        firmware_extension(bootloader_path).as_str(),
        firmware_extension(app_path).as_str(),
    ) {
        ("bin", "bin") => {
            args.extend([
                "-w",
                bootloader,
                bootloader_addr.as_str(),
                "-w",
                app_firmware,
                app_addr.as_str(),
                "-v",
                "-s",
            ]);
        }
        ("bin", "hex") => {
            args.extend([
                "-w",
                bootloader,
                bootloader_addr.as_str(),
                "-w",
                app_firmware,
                "-v",
                "-s",
            ]);
        }
        ("hex", "bin") => {
            args.extend([
                "-w",
                bootloader,
                "-w",
                app_firmware,
                app_addr.as_str(),
                "-v",
                "-s",
            ]);
        }
        _ => {
            args.extend(["-w", bootloader, "-w", app_firmware, "-v", "-s"]);
        }
    }

    let (ok, output) = run_command_capture(cli, &args)?;
    if ok {
        Ok(output)
    } else {
        Err(format!(
            "Recovery flash failed (port={}):\n{}",
            port, output
        ))
    }
}

/// DFU recovery for ECUs that no longer boot after an app-only flash.
/// Does not require an ECU connection — put the board in DFU manually first.
#[tauri::command]
pub async fn recover_ecu_firmware_dfu(
    app: AppHandle,
    bootloader_path: String,
    app_firmware_path: String,
    app_flash_address: Option<String>,
    full_erase: bool,
) -> Result<FirmwareUpdateResult, String> {
    let bootloader = PathBuf::from(&bootloader_path);
    let app_firmware = PathBuf::from(&app_firmware_path);
    validate_recovery_image(&bootloader, "Bootloader")?;
    validate_recovery_image(&app_firmware, "Application")?;

    let app_address = parse_flash_address(app_flash_address.as_deref())?
        .unwrap_or_else(default_bin_flash_address);

    let cli = find_stm32_programmer_cli()
        .ok_or("STM32CubeProgrammer (STM32_Programmer_CLI) is required for DFU recovery.")?;

    let mut log = Vec::new();
    push_log(
        &app,
        &mut log,
        "Starting DFU recovery (OpenBLT bootloader + application)…",
    );
    push_log(
        &app,
        &mut log,
        format!(
            "Bootloader: {} @ 0x{:08X}",
            bootloader.display(),
            OPENBLT_BOOTLOADER_ADDRESS
        ),
    );
    push_log(
        &app,
        &mut log,
        format!(
            "Application: {} @ 0x{:08X}",
            app_firmware.display(),
            app_address
        ),
    );

    if full_erase {
        let port = stm32_programmer_port(&cli);
        push_log(
            &app,
            &mut log,
            "Performing full chip erase (required on STM32F7)…",
        );
        let erase_output = stm32_full_chip_erase(&cli, &port)?;
        for line in erase_output
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            push_log(&app, &mut log, line);
        }
    }

    push_log(&app, &mut log, format!("Flashing with {}…", cli.display()));
    let flash_output =
        flash_recovery_with_stm32_programmer(&cli, &bootloader, &app_firmware, app_address, false)?;
    for line in flash_output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        push_log(&app, &mut log, line);
    }

    let message = "Recovery flash complete. Disconnect USB, power-cycle the ECU, then reconnect in normal mode.";
    push_log(&app, &mut log, message);
    Ok(FirmwareUpdateResult {
        success: true,
        log,
        message: message.to_string(),
        should_reconnect: false,
    })
}
