//! ECU connection lifecycle commands.

use crate::state::AppState;
use crate::{load_settings, set_conn_lock_holder, stop_metrics_task, ConnectionStatus};
use libretune_core::protocol::ConnectionState;
use std::path::Path;
use std::time::{Duration, Instant};

/// Disconnects from the currently connected ECU.
///
/// Closes the serial connection and clears the connection state.
///
/// How long [`disconnect_ecu`] will wait for another connection transition
/// before going ahead without the lock. Deliberately shorter than the
/// connection-lock deadline: a user asking to disconnect wants the link
/// gone, not a spinner.
const TRANSITION_LOCK_BUDGET: Duration = Duration::from_secs(1);

/// Take `lock`, giving up after `budget` rather than waiting for however
/// long its current holder needs.
async fn acquire_within(
    lock: &tokio::sync::Mutex<()>,
    budget: Duration,
) -> Option<tokio::sync::MutexGuard<'_, ()>> {
    let deadline = Instant::now() + budget;
    loop {
        // `try_lock` in a loop rather than `timeout(lock())`: the guard has to
        // outlive this function, and a timed-out `lock()` future can still be
        // holding a place in the queue when it is dropped.
        if let Ok(guard) = lock.try_lock() {
            return Some(guard);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Returns: Nothing on success
#[tauri::command]
pub async fn disconnect_ecu(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Issue #71: raise the connection's cancel flag FIRST. The task we need out
    // of the way is inside a blocking read holding `state.connection`, so any
    // signal that has to acquire that lock arrives too late by construction.
    // The flag is polled by `read_exact_by_deadline`, which aborts the read.
    crate::state::request_ecu_cancel();
    // Every path that replaces `state.connection` takes this first, so a
    // teardown cannot race a demo transition or a connect that is midway
    // through installing its own connection.
    //
    // Bounded, though: `connect_to_ecu` holds it while its worker thread
    // blocks on a channel for up to 15 s, and waiting unconditionally here
    // would put Issue #71's "disconnect does nothing" hang straight back —
    // five times longer than the deadline below was chosen to allow.
    // Disconnect must always work, so past the budget it proceeds without
    // the lock and accepts the race it guards.
    let _transition = acquire_within(&state.connection_transition, TRANSITION_LOCK_BUDGET).await;
    // Even without the lock, a connect still handshaking will see this
    // bump and drop its connection instead of installing it over ours.
    state
        .connection_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if _transition.is_none() {
        eprintln!(
            "[WARN] disconnect_ecu: another connection transition is still running \
             after {}s, disconnecting anyway (Issue #71)",
            TRANSITION_LOCK_BUDGET.as_secs()
        );
    }

    // Stop metrics and realtime streaming before dropping the connection
    stop_metrics_task(state.clone()).await;

    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    // Issue #71: the realtime stream task performs BLOCKING serial I/O inside the
    // connection lock (get_realtime_data -> send_raw_command/send_packet). Tokio's
    // task abort only takes effect at the next `.await`, so a mid-read task can hold
    // the connection mutex for up to one full read timeout. The cancel flag raised
    // above cuts that read short; polling the lock with a deadline covers the cases
    // it cannot interrupt (a burn, a multi-page write) so the UI never hangs.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match state.connection.try_lock() {
            Ok(mut guard) => {
                if let Some(conn) = guard.as_mut() {
                    conn.disconnect();
                }
                *guard = None;
                crate::state::clear_ecu_cancel_handle();
                return Ok(());
            }
            Err(_) => {
                if Instant::now() >= deadline {
                    // Could not acquire the lock within the deadline even though
                    // cancellation was requested up front — the holder is in a
                    // burn or a multi-page write, which the flag does not
                    // interrupt. Fall back to the async lock so the runtime can
                    // schedule other work while that finishes.
                    eprintln!(
                        "[WARN] disconnect_ecu: connection lock busy after 3s, \
                         forcing disconnect (Issue #71)"
                    );
                    // Retry one more time with a full async lock (may still block,
                    // but lets the runtime schedule other work first).
                    let mut guard = state.connection.lock().await;
                    if let Some(conn) = guard.as_mut() {
                        conn.disconnect();
                    }
                    *guard = None;
                    crate::state::clear_ecu_cancel_handle();
                    return Ok(());
                }
                // Yield to the runtime so the (aborted) streaming task can finish its
                // current tick and release the lock.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

// Adaptive timing commands extracted to commands/adaptive_timing.rs
/// Gets the current ECU connection status.
///
/// Returns comprehensive connection information including state, ECU signature,
/// loaded INI info, and demo mode status.
///
/// Returns: ConnectionStatus with connection state and metadata
#[tauri::command]
pub async fn get_connection_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    // IMPORTANT: Acquire each lock independently and release before taking the next.
    // Holding multiple locks simultaneously causes deadlocks with the realtime stream task.
    let demo_mode = *state.demo_mode.lock().await;

    let streaming_active = {
        let task_guard = state.streaming_task.lock().await;
        task_guard.is_some()
    };

    let (state_val, signature) = if demo_mode && streaming_active {
        (
            ConnectionState::Connected,
            Some("DEMO - Simulated epicEFI".to_string()),
        )
    } else {
        set_conn_lock_holder("get_connection_status");
        let conn_guard = state.connection.lock().await;
        let result = match &*conn_guard {
            Some(conn) => (conn.state(), conn.signature().map(|s| s.to_string())),
            None => (ConnectionState::Disconnected, None),
        };
        drop(conn_guard);
        set_conn_lock_holder("(none)");
        result
    };

    let (has_definition, ini_name) = {
        let def_guard = state.definition.lock().await;
        (
            def_guard.is_some(),
            def_guard.as_ref().map(|d| d.signature.clone()),
        )
    };

    Ok(ConnectionStatus {
        state: state_val,
        signature,
        has_definition,
        ini_name,
        demo_mode,
    })
}

/// Retrieves the path to the last-used INI file from settings.
///
/// Used on startup to auto-load the previously used ECU definition.
///
/// Returns: Optional path to last INI file, or None if not set or file missing
#[tauri::command]
pub async fn auto_load_last_ini(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let settings = load_settings(&app);
    if let Some(path) = settings.last_ini_path {
        if Path::new(&path).exists() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod disconnect_tests {
    use super::{acquire_within, TRANSITION_LOCK_BUDGET};
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn a_free_lock_is_taken_immediately() {
        let lock = Mutex::new(());
        let started = Instant::now();

        let guard = acquire_within(&lock, TRANSITION_LOCK_BUDGET).await;

        assert!(guard.is_some());
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "an uncontended lock must not wait"
        );
    }

    #[tokio::test]
    async fn a_held_lock_is_given_up_on_instead_of_waited_out() {
        // `connect_to_ecu` holds the transition lock while its worker thread
        // blocks on a channel for up to 15 s. Waiting that out would put
        // Issue #71's "disconnect does nothing" hang back, only longer.
        let lock = Mutex::new(());
        let held = lock.lock().await;
        let budget = Duration::from_millis(150);
        let started = Instant::now();

        let guard = acquire_within(&lock, budget).await;

        assert!(guard.is_none(), "the budget must expire, not the caller");
        let waited = started.elapsed();
        assert!(
            waited >= budget,
            "it must actually try for the whole budget"
        );
        assert!(
            waited < budget * 4,
            "it must not wait for the holder: gave up after {waited:?}"
        );
        drop(held);
    }
}

#[cfg(test)]
mod tests {
    use crate::state::{clear_ecu_cancel_handle, request_ecu_cancel, set_ecu_cancel_handle};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Issue #71: `disconnect_ecu` raises the flag before it goes near the
    /// connection mutex. That is only possible while the handle lives outside
    /// that mutex, which is what this pins down — the whole mechanism was inert
    /// because nothing ever held a handle to raise.
    #[test]
    fn the_cancel_flag_is_reachable_without_the_connection_lock() {
        let flag = Arc::new(AtomicBool::new(false));
        set_ecu_cancel_handle(Arc::clone(&flag));

        request_ecu_cancel();
        assert!(
            flag.load(Ordering::Relaxed),
            "the in-flight read must see the cancellation"
        );

        // Once the connection is gone its flag must not be touched again.
        clear_ecu_cancel_handle();
        flag.store(false, Ordering::Relaxed);
        request_ecu_cancel();
        assert!(
            !flag.load(Ordering::Relaxed),
            "a later disconnect must not raise a dropped connection's flag"
        );
    }
}
