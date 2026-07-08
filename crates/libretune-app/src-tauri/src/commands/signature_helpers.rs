//! Signature comparison and INI matching helpers.

use crate::{AppState, ConnectResult, MatchingIniInfo, SignatureMatchType, SignatureMismatchInfo};
use libretune_core::protocol::ConnectionConfig;

/// Normalize a signature string for robust comparison:
/// - Lowercase
/// - Replace non-alphanumeric characters with spaces
/// - Collapse multiple spaces
pub(crate) fn normalize_signature(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;

    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if ch.is_whitespace() {
            if !last_was_space && !out.is_empty() {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            // Punctuation or other characters -> treat as separator
            if !last_was_space && !out.is_empty() {
                out.push(' ');
                last_was_space = true;
            }
        }
    }

    out.trim().to_string()
}

/// Compare two signatures and determine match type using normalized strings
pub(crate) fn compare_signatures(ecu_sig: &str, ini_sig: &str) -> SignatureMatchType {
    compare_signatures_with_prefix(ecu_sig, ini_sig, None)
}

/// Compare signatures honoring an optional `signaturePrefix` (msEnvelope_1.0 spec §3.4).
/// When `ini_prefix` is provided and the ECU signature starts with that prefix
/// (after normalization), the match is upgraded to `Exact` even if the trailing
/// build/version differs.
pub(crate) fn compare_signatures_with_prefix(
    ecu_sig: &str,
    ini_sig: &str,
    ini_prefix: Option<&str>,
) -> SignatureMatchType {
    let ecu_normalized = normalize_signature(ecu_sig);
    let ini_normalized = normalize_signature(ini_sig);

    if ecu_normalized == ini_normalized {
        return SignatureMatchType::Exact;
    }

    if let Some(prefix) = ini_prefix {
        let prefix_normalized = normalize_signature(prefix);
        if !prefix_normalized.is_empty() && ecu_normalized.starts_with(&prefix_normalized) {
            return SignatureMatchType::Exact;
        }
    }

    // Check for common suffixes (hashes)
    // RusEFI signatures often end with a hash or unique ID (e.g. "rusEFI master 2024.02.24.simulator.12345678")
    // If both end with the same alphanumeric string > 6 chars, treat as Exact.
    if let (Some(ecu_suffix), Some(ini_suffix)) = (
        ecu_normalized.split('.').next_back(),
        ini_normalized.split('.').next_back(),
    ) {
        if ecu_suffix.len() > 6
            && ecu_suffix.chars().all(|c| c.is_alphanumeric())
            && ecu_suffix == ini_suffix
        {
            return SignatureMatchType::Exact;
        }
    }

    // Also check split by whitespace just in case hash is separated by space
    if let (Some(ecu_suffix), Some(ini_suffix)) = (
        ecu_normalized.split_whitespace().last(),
        ini_normalized.split_whitespace().last(),
    ) {
        if ecu_suffix.len() > 6
            && ecu_suffix.chars().all(|c| c.is_alphanumeric())
            && ecu_suffix == ini_suffix
        {
            return SignatureMatchType::Exact;
        }
    }

    if ecu_normalized.contains(&ini_normalized) || ini_normalized.contains(&ecu_normalized) {
        return SignatureMatchType::Partial;
    }

    // Compare first token (base type) e.g., "speeduino" and "rusEFI"
    let ecu_first = ecu_normalized.split_whitespace().next();
    let ini_first = ini_normalized.split_whitespace().next();
    if let (Some(ecu_first), Some(ini_first)) = (ecu_first, ini_first) {
        if ecu_first == ini_first {
            return SignatureMatchType::Partial;
        }
    }

    // Check for common firmware family keywords
    let common_keywords = ["uaefi", "speeduino", "rusefi", "epicefi", "megasquirt"];
    let ecu_has_keyword = common_keywords.iter().any(|kw| ecu_normalized.contains(kw));
    let ini_has_keyword = common_keywords.iter().any(|kw| ini_normalized.contains(kw));

    if ecu_has_keyword && ini_has_keyword {
        let ecu_keywords: Vec<&str> = common_keywords
            .iter()
            .filter(|kw| ecu_normalized.contains(**kw))
            .copied()
            .collect();
        let ini_keywords: Vec<&str> = common_keywords
            .iter()
            .filter(|kw| ini_normalized.contains(**kw))
            .copied()
            .collect();

        if ecu_keywords.iter().any(|kw| ini_keywords.contains(kw)) {
            return SignatureMatchType::Partial;
        }
    }

    SignatureMatchType::Mismatch
}

/// Build a shallow SignatureMismatchInfo (without resolving matching INIs) for testing
#[allow(dead_code)]
pub(crate) fn build_shallow_mismatch_info(
    ecu_signature: &str,
    ini_signature: &str,
    current_ini_path: Option<String>,
) -> SignatureMismatchInfo {
    let match_type = compare_signatures(ecu_signature, ini_signature);
    SignatureMismatchInfo {
        ecu_signature: ecu_signature.to_string(),
        ini_signature: ini_signature.to_string(),
        match_type,
        current_ini_path,
        matching_inis: Vec::new(),
    }
}

/// Find INI files that match the given ECU signature (uses tauri State wrapper)
pub(crate) async fn find_matching_inis_internal(
    state: &tauri::State<'_, AppState>,
    ecu_signature: &str,
) -> Vec<MatchingIniInfo> {
    find_matching_inis_from_state(state, ecu_signature).await
}

// Test-only helper: simulate the signature handling part of connect_to_ecu
#[cfg(test)]
pub(crate) async fn connect_to_ecu_simulated(state: &AppState, signature: &str) -> ConnectResult {
    // If there's a loaded definition, compare signatures
    let (expected_signature, expected_prefix) = {
        let def_guard = state.definition.lock().await;
        match def_guard.as_ref() {
            Some(d) => (Some(d.signature.clone()), d.signature_prefix.clone()),
            None => (None, None),
        }
    };

    let mismatch_info = if let Some(ref expected) = expected_signature {
        let match_type =
            compare_signatures_with_prefix(signature, expected, expected_prefix.as_deref());
        if match_type != SignatureMatchType::Exact {
            let matching_inis = find_matching_inis_from_state(state, signature).await;
            let current_ini_path = None; // In tests we don't need an app handle to load settings
            Some(SignatureMismatchInfo {
                ecu_signature: signature.to_string(),
                ini_signature: expected.clone(),
                match_type,
                current_ini_path,
                matching_inis,
            })
        } else {
            None
        }
    } else {
        None
    };

    ConnectResult {
        signature: signature.to_string(),
        mismatch_info,
    }
}

// Helper that invokes the optional connection factory and builds a ConnectResult
pub(crate) async fn call_connection_factory_and_build_result(
    state: &AppState,
    config: ConnectionConfig,
) -> Result<ConnectResult, String> {
    // Read protocol settings and expected signature from state
    let def_guard = state.definition.lock().await;
    let protocol_settings = def_guard.as_ref().map(|d| d.protocol.clone());
    let endianness = def_guard.as_ref().map(|d| d.endianness).unwrap_or_default();
    let expected_signature = def_guard.as_ref().map(|d| d.signature.clone());
    let expected_prefix = def_guard.as_ref().and_then(|d| d.signature_prefix.clone());
    drop(def_guard);

    let factory_opt = state.connection_factory.lock().await.clone();
    if let Some(factory) = factory_opt {
        match (factory)(config, protocol_settings, endianness) {
            Ok(signature) => {
                // Build mismatch info if needed
                let mismatch_info = if let Some(ref expected) = expected_signature {
                    let match_type = compare_signatures_with_prefix(
                        &signature,
                        expected,
                        expected_prefix.as_deref(),
                    );
                    if match_type != SignatureMatchType::Exact {
                        let matching_inis = find_matching_inis_from_state(state, &signature).await;
                        let current_ini_path = None; // caller may provide app if needed

                        Some(SignatureMismatchInfo {
                            ecu_signature: signature.clone(),
                            ini_signature: expected.clone(),
                            match_type,
                            current_ini_path,
                            matching_inis,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                Ok(ConnectResult {
                    signature,
                    mismatch_info,
                })
            }
            Err(e) => Err(format!("Factory-based connect failed: {}", e)),
        }
    } else {
        Err("No connection factory installed".to_string())
    }
}

/// Test-friendly variant that operates on an AppState reference directly
pub(crate) async fn find_matching_inis_from_state(
    state: &AppState,
    ecu_signature: &str,
) -> Vec<MatchingIniInfo> {
    let mut matches = Vec::new();

    // Check INI repository if loaded
    let repo_guard = state.ini_repository.lock().await;
    if let Some(ref repo) = *repo_guard {
        for entry in repo.list() {
            let match_type = compare_signatures(ecu_signature, &entry.signature);
            if match_type != SignatureMatchType::Mismatch {
                matches.push(MatchingIniInfo {
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                    signature: entry.signature.clone(),
                    match_type,
                });
            }
        }
    }

    // Sort by match type (exact first, then partial)
    matches.sort_by(|a, b| match (&a.match_type, &b.match_type) {
        (SignatureMatchType::Exact, SignatureMatchType::Partial) => std::cmp::Ordering::Less,
        (SignatureMatchType::Partial, SignatureMatchType::Exact) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    matches
}
