//! OS keychain storage for the AI assistant's API key.
//!
//! The key grants access to a paid LLM account, so it should not sit in
//! plaintext `settings.json` next to everything else. When a keychain
//! backend is available (Windows Credential Manager, macOS Keychain,
//! Linux Secret Service), the key lives there and the settings file keeps
//! an empty string.
//!
//! Every operation degrades gracefully: on headless Linux, locked-down
//! sessions, or any backend failure, callers fall back to the previous
//! plaintext behavior rather than breaking the assistant.

/// Keychain service/account identifiers.
const SERVICE: &str = "LibreTune";
const ACCOUNT: &str = "ai_api_key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("keychain unavailable: {e}"))
}

/// Store `key` in the OS keychain, replacing any previous value.
pub(crate) fn store(key: &str) -> Result<(), String> {
    let entry = entry()?;
    // set_password overwrites on every mainstream backend, but a few map it
    // onto create-only semantics; delete-then-set is portable and cheap.
    let _ = entry.delete_credential();
    entry
        .set_password(key)
        .map_err(|e| format!("keychain write failed: {e}"))
}

/// Read the key from the OS keychain. `None` when absent or unavailable.
pub(crate) fn load() -> Option<String> {
    let entry = entry().ok()?;
    match entry.get_password() {
        Ok(k) => Some(k),
        Err(keyring::Error::NoEntry) => None,
        Err(_) => None,
    }
}

/// Remove the key from the OS keychain (best-effort).
pub(crate) fn delete() {
    if let Ok(entry) = entry() {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests probe the real OS keychain of the machine running them.
    // Store may legitimately fail on headless CI — only assert round-trip
    // behavior when the backend is actually usable.

    #[test]
    fn store_and_load_round_trip_when_available() {
        let probe = format!("libretune-test-{}", std::process::id());
        if store(&probe).is_err() {
            return; // no keychain backend here; fallback path is exercised
        }
        assert_eq!(load().as_deref(), Some(probe.as_str()));
        delete();
        assert!(load().is_none());
    }
}
