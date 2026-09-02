//! Per-install bearer token for the local MCP server.
//!
//! The token is the only thing standing between a local process and the
//! tune-reading tools, so it lives in its own file (never in
//! `settings.json`, which is world-readable and gets copied around in bug
//! reports) and is written owner-only on unix.
//!
//! Ported from OpenTune's `ai_settings.rs` token helpers; the file name and
//! the 64-char hex shape are kept identical so an existing OpenTune client
//! config only needs its URL changed.

use std::path::{Path, PathBuf};

use rand::Rng;

/// Token file name inside the app data directory.
pub const MCP_TOKEN_FILE: &str = "mcp-token";

/// Generate a random 32-byte, hex-encoded token (64 chars). Hex is folded by
/// hand — one 32-byte encode does not justify a dependency.
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn token_path(dir: &Path) -> PathBuf {
    dir.join(MCP_TOKEN_FILE)
}

/// Write the token via a temp file + rename so a crash mid-write can never
/// leave a truncated token behind (which would lock the user out of their
/// own server until they regenerated it), then restrict the file to the
/// owner on unix — the default 0644 would let any other local account read
/// the bearer token straight off disk.
fn write_token(dir: &Path, token: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("failed to create config dir: {e}"))?;

    let target = token_path(dir);
    let temp = dir.join(format!(".{MCP_TOKEN_FILE}.{}.tmp", std::process::id()));
    std::fs::write(&temp, token.as_bytes())
        .map_err(|e| format!("failed to write MCP token: {e}"))?;
    std::fs::rename(&temp, &target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("failed to write MCP token: {e}")
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("failed to restrict MCP token permissions: {e}"))?;
    }

    Ok(())
}

/// Read the token from `<dir>/mcp-token`, generating and persisting a fresh
/// one when the file is missing, empty, or whitespace-only.
pub fn load_or_create_token(dir: &Path) -> Result<String, String> {
    match std::fs::read_to_string(token_path(dir)) {
        Ok(text) => {
            let token = text.trim();
            if token.is_empty() {
                let fresh = generate_token();
                write_token(dir, &fresh)?;
                Ok(fresh)
            } else {
                Ok(token.to_owned())
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let fresh = generate_token();
            write_token(dir, &fresh)?;
            Ok(fresh)
        }
        Err(e) => Err(format!("failed to read MCP token: {e}")),
    }
}

/// Always mint and persist a fresh token, invalidating the previous one.
pub fn regenerate_token(dir: &Path) -> Result<String, String> {
    let token = generate_token();
    write_token(dir, &token)?;
    Ok(token)
}
