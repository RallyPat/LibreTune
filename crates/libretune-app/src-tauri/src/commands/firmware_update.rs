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
    pub objcopy: Option<String>,
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

fn path_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(path) = std::env::var("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    #[cfg(windows)]
    if let Some(user_path) = windows_user_path_from_registry() {
        dirs.extend(std::env::split_paths(&user_path));
    }
    dirs
}

#[cfg(windows)]
fn windows_user_path_from_registry() -> Option<std::ffi::OsString> {
    let output = Command::new("reg")
        .args(["query", r"HKCU\Environment", "/v", "Path"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let value = if let Some((_, rest)) = line.split_once("REG_EXPAND_SZ") {
            expand_windows_env_path(rest.trim())
        } else if let Some((_, rest)) = line.split_once("REG_SZ") {
            rest.trim().to_string()
        } else {
            continue;
        };
        if !value.is_empty() {
            return Some(std::ffi::OsString::from(value));
        }
    }
    None
}

#[cfg(windows)]
fn expand_windows_env_path(path: &str) -> String {
    let mut out = path.to_string();
    if let Ok(profile) = std::env::var("USERPROFILE") {
        out = out.replace("%USERPROFILE%", &profile);
    }
    if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        out = out.replace("%LOCALAPPDATA%", &appdata);
    }
    out
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    path_search_dirs().into_iter().find_map(|dir| {
        let candidate = dir.join(name);
        candidate.is_file().then_some(candidate)
    })
}

#[cfg(windows)]
fn find_via_where(name: &str) -> Option<PathBuf> {
    let output = Command::new("where.exe")
        .arg(name)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    let path = PathBuf::from(line);
    path.is_file().then_some(path)
}

#[cfg(not(windows))]
fn find_via_where(_name: &str) -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn user_downloads_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Downloads"))
}

#[cfg(not(windows))]
fn user_downloads_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn collect_openblt_host_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(windows)]
    {
        dirs.push(PathBuf::from(r"C:\OpenBLT\Host\Binaries"));
        dirs.push(PathBuf::from(r"C:\OpenBLT\Host"));
        dirs.push(PathBuf::from(r"C:\Program Files\OpenBLT\Host\Binaries"));
        dirs.push(PathBuf::from(r"C:\Program Files\OpenBLT\Host"));
    }
    #[cfg(not(windows))]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/opt/openblt/bin"));
    }

    if let Some(downloads) = user_downloads_dir() {
        if let Ok(entries) = std::fs::read_dir(downloads) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !name.contains("openblt") {
                    continue;
                }
                dirs.push(path.join("Host"));
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub in sub_entries.flatten() {
                        let sub_path = sub.path();
                        if sub_path.is_dir() {
                            dirs.push(sub_path.join("Host"));
                        }
                    }
                }
            }
        }
    }

    dirs
}

/// Resolve a flash tool: PATH first, then known install dirs, then `tools/` next to the app.
fn resolve_tool(candidates: &[&str], extra_dirs: &[PathBuf]) -> Option<PathBuf> {
    for name in candidates {
        if let Some(found) = find_on_path(name) {
            return Some(found);
        }
    }

    for dir in extra_dirs {
        for name in candidates {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for sub in ["tools", "bin", "resources/tools"] {
                let dir = exe_dir.join(sub);
                for name in candidates {
                    let candidate = dir.join(name);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    for name in candidates {
        if let Some(found) = find_via_where(name) {
            return Some(found);
        }
    }

    None
}

#[cfg(windows)]
fn stm32_programmer_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from(r"C:\Program Files\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin"),
        PathBuf::from(
            r"C:\Program Files (x86)\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin",
        ),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(local)
                .join("Programs")
                .join("STMicroelectronics")
                .join("STM32Cube")
                .join("STM32CubeProgrammer")
                .join("bin"),
        );
    }
    dirs
}

#[cfg(not(windows))]
fn stm32_programmer_search_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/local/STMicroelectronics/STM32Cube/STM32CubeProgrammer/bin"),
        PathBuf::from("/opt/stm32cubeprog/bin"),
    ]
}

#[cfg(windows)]
fn dfu_util_search_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from(r"C:\Program Files\dfu-util"),
        PathBuf::from(r"C:\Program Files (x86)\dfu-util"),
    ]
}

#[cfg(not(windows))]
fn dfu_util_search_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
    ]
}

fn openblt_search_dirs() -> Vec<PathBuf> {
    collect_openblt_host_dirs()
}

#[cfg(windows)]
fn find_stm32_programmer_cli() -> Option<PathBuf> {
    resolve_tool(
        &["STM32_Programmer_CLI.exe", "STM32_Programmer_CLI"],
        &stm32_programmer_search_dirs(),
    )
}

#[cfg(not(windows))]
fn find_stm32_programmer_cli() -> Option<PathBuf> {
    resolve_tool(&["STM32_Programmer_CLI"], &stm32_programmer_search_dirs())
}

fn find_dfu_util() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        resolve_tool(&["dfu-util.exe", "dfu-util"], &dfu_util_search_dirs())
    }
    #[cfg(not(windows))]
    {
        resolve_tool(&["dfu-util"], &dfu_util_search_dirs())
    }
}

fn find_bootcommander() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        resolve_tool(
            &["BootCommander.exe", "BootCommander"],
            &openblt_search_dirs(),
        )
    }
    #[cfg(not(windows))]
    {
        resolve_tool(&["BootCommander"], &openblt_search_dirs())
    }
}

fn find_objcopy() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        resolve_tool(&["arm-none-eabi-objcopy.exe", "arm-none-eabi-objcopy"], &[])
    }
    #[cfg(not(windows))]
    {
        resolve_tool(&["arm-none-eabi-objcopy"], &[])
    }
}

/// BootCommander expects S-records; convert a raw `.bin` (same as rusEFI Console uses).
fn convert_bin_to_srec(bin_path: &Path, load_address: u32) -> Result<PathBuf, String> {
    let objcopy = find_objcopy().ok_or(
        "arm-none-eabi-objcopy not found on PATH. Install the ARM GCC toolchain (same as the \
         rusEFI build) or use DFU mode with rusefi.hex instead.",
    )?;
    let temp_dir = std::env::temp_dir();
    let stem = bin_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("firmware");
    let srec_path = temp_dir.join(format!("libretune_{}_{:x}.srec", stem, load_address));
    let bin_str = bin_path
        .to_str()
        .ok_or_else(|| "Invalid firmware path".to_string())?;
    let srec_str = srec_path
        .to_str()
        .ok_or_else(|| "Invalid temporary path".to_string())?;
    let address = format!("0x{:08X}", load_address);
    let (ok, output) = run_command_capture(
        &objcopy,
        &[
            "-I",
            "binary",
            "-O",
            "srec",
            &format!("--change-addresses={}", address),
            bin_str,
            srec_str,
        ],
    )?;
    if ok && srec_path.is_file() {
        Ok(srec_path)
    } else {
        Err(format!(
            "Failed to convert .bin to .srec for BootCommander:\n{}",
            output
        ))
    }
}

/// Detect available external flash tools on the system.
#[tauri::command]
pub async fn get_firmware_flasher_info() -> Result<FirmwareFlasherInfo, String> {
    Ok(FirmwareFlasherInfo {
        stm32_programmer_cli: find_stm32_programmer_cli().map(|p| p.display().to_string()),
        dfu_util: find_dfu_util().map(|p| p.display().to_string()),
        bootcommander: find_bootcommander().map(|p| p.display().to_string()),
        objcopy: find_objcopy().map(|p| p.display().to_string()),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareCompanionSuggestion {
    pub companion_path: Option<String>,
    pub companion_kind: String,
    pub message: String,
}

/// Optional hint when the user picks a firmware file.
#[tauri::command]
pub async fn suggest_firmware_companion(
    firmware_path: String,
) -> Result<FirmwareCompanionSuggestion, String> {
    let path = PathBuf::from(&firmware_path);
    if !path.is_file() {
        return Err("Firmware file not found".to_string());
    }

    let ext = firmware_extension(&path);
    let message = match ext.as_str() {
        "bin" => {
            "rusefi.bin is the correct file for a normal serial update (same as rusEFI Console \
             and epicEFI). LibreTune converts it automatically for BootCommander."
                .into()
        }
        "hex" => {
            "rusefi.hex includes flash addresses — best for DFU recovery with STM32CubeProgrammer. \
             For a normal in-car update, prefer rusefi.bin with the serial (OpenBLT) method."
                .into()
        }
        "dfu" => {
            "Pre-packaged DFU image — use with DFU mode and STM32CubeProgrammer or dfu-util.".into()
        }
        _ => String::new(),
    };

    Ok(FirmwareCompanionSuggestion {
        companion_path: None,
        companion_kind: ext,
        message,
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

/// BootCommander can keep COM ports open after a failed serial flash attempt.
fn kill_stale_bootcommander_processes() {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/IM", "BootCommander.exe", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Free a blocked COM port after a failed firmware flash (e.g. stuck BootCommander).
#[tauri::command]
pub async fn release_serial_port_blockers(state: tauri::State<'_, AppState>) -> Result<(), String> {
    kill_stale_bootcommander_processes();
    stop_metrics_task(state.clone()).await;
    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }
    {
        let mut conn_guard = state.connection.lock().await;
        if let Some(conn) = conn_guard.as_mut() {
            conn.disconnect();
        }
        *conn_guard = None;
    }
    Ok(())
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
        ("openblt", "raw_bin") => {
            risk_level = "low";
        }
        ("openblt", "hex") => {
            risk_level = "medium";
            warnings.push(
                "For serial updates, rusefi.bin from firmware/build/ is preferred (same as rusEFI Console). \
                 .hex can work if BootCommander accepts it."
                    .into(),
            );
        }
        ("openblt", "update_srec" | "srec") => {
            risk_level = "low";
        }
        ("openblt", "dfu_package" | "unknown") => {
            risk_level = "high";
            requires_risk_acknowledgement = true;
            warnings.push("Use firmware/build/rusefi.bin for serial updates.".into());
        }
        ("dfu", "hex") => {
            risk_level = "low";
        }
        ("dfu", "dfu_package") => {
            risk_level = "low";
        }
        ("dfu", "raw_bin") => {
            risk_level = "medium";
            warnings.push(
                "Raw .bin via DFU writes the application at 0x08008000. Prefer rusefi.hex — \
                 STM32CubeProgrammer reads addresses from the file automatically."
                    .into(),
            );
        }
        ("dfu", "srec" | "update_srec") => {
            risk_level = "medium";
            warnings.push(
                "DFU mode works best with rusefi.hex or a .dfu package. Use serial update for routine flashes."
                    .into(),
            );
        }
        _ => {}
    }

    let suggested_file_hint = if method == "openblt" {
        "firmware/build/rusefi.bin".into()
    } else {
        "firmware/build/rusefi.hex (or deliver/*.dfu)".into()
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
                "firmware/build/rusefi.bin".into()
            } else {
                "firmware/build/rusefi.hex".into()
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
            "This ECU supports {} — recommended for routine updates.",
            if guidance.recommended_method == "openblt" {
                "serial update (OpenBLT) with rusefi.bin"
            } else {
                "DFU with rusefi.hex"
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
        "openblt" if !matches!(ext.as_str(), "srec" | "s19" | "hex" | "bin") => {
            return Err(
                "Serial update expects firmware/build/rusefi.bin (or .hex / .srec if you have one)"
                    .to_string(),
            );
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
    if method == "openblt" {
        return Err(
            "Serial (OpenBLT) firmware update is disabled in LibreTune — it can brick your ECU. \
             Use rusEFI Console with firmware/build/rusefi.bin for routine updates."
                .to_string(),
        );
    }

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

            let flash_path = if ext == "bin" {
                let address = resolved_bin_address.unwrap_or_else(default_bin_flash_address);
                push_log(
                    &app,
                    &mut log,
                    format!(
                        "Converting rusefi.bin to .srec for BootCommander (load address 0x{:08X})…",
                        address
                    ),
                );
                convert_bin_to_srec(&path, address)?
            } else {
                path.clone()
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
            flash_with_bootcommander(&tool, &flash_path, &serial_port, baud_rate)?
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
