//! File System Watcher (issue #38: async refactor + folder-missing handling +
//! real debouncing via notify-debouncer-full)
//!
//! Why async + tokio:
//! - Old design used std::thread + std::sync::Mutex<JoinHandle>; start() called
//!   stop() (which join()s the thread) while holding the mutex → deadlock risk
//!   when called from a Tauri tokio command. New design uses tokio::sync::Mutex
//!   and tokio::task::JoinHandle so stop() yields rather than blocks.
//! - Old loop block-on'd a tokio runtime handle from inside a std thread to run
//!   the async DB scan. New loop is itself a tokio task — scan_folder() is just
//!   awaited directly.
//!
//! Why notify-debouncer-full:
//! - Old "debounce" was recv_timeout(2s) with an inner drain loop that resets
//!   every 2s. Under continuous churn (cloud sync, git ops), the drain never
//!   ends and a scan never fires; under bursty churn the drain may fire mid-
//!   sequence. The official debouncer guarantees a fixed timeout window after
//!   the last event before delivering a coalesced batch.
//! - DEBOUNCE_INTERVAL = 1500ms catches OneDrive/Dropbox/iCloud bursts (~500ms
//!   to 1s typical) and Word/Excel save cycles without feeling laggy on a
//!   single drag-drop.

use notify::{event::RemoveKind, EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;

use crate::db::DbPool;

use super::ingest::{get_document_folder, scan_folder};

/// Debounce window — see module-level comment for the choice rationale.
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(1500);

/// Bounded wait when stopping a running watcher. The inner task may be in the
/// middle of a `scan_folder` await that doesn't observe `cancel.cancelled()`
/// until the scan finishes. We don't want to hang the UI thread that called
/// `set_document_folder` or `remove_document_folder` if a scan is genuinely
/// stuck — log a warning and move on. The cancel signal still fires when the
/// scan eventually finishes, so the task exits without leaking the debouncer.
const WATCHER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Tauri event name emitted when the watcher detects the watched root has
/// disappeared (or after a rescan attempt finds it missing).
pub const FOLDER_MISSING_EVENT: &str = "documents-folder-missing";

/// Manages the lifecycle of the file-system watcher task.
/// Allows stopping/restarting when the watched folder changes.
pub struct WatcherState {
    inner: TokioMutex<Option<RunningWatcher>>,
    app_handle: AppHandle,
}

struct RunningWatcher {
    handle: tokio::task::JoinHandle<()>,
    cancel: CancellationToken,
}

impl WatcherState {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            inner: TokioMutex::new(None),
            app_handle,
        }
    }

    /// Stop the running watcher (if any). Returns when the task observes the
    /// cancel and exits, or after `WATCHER_SHUTDOWN_TIMEOUT`. Non-blocking on
    /// the tokio runtime.
    pub async fn stop(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(rw) = guard.take() {
            rw.cancel.cancel();
            match tokio::time::timeout(WATCHER_SHUTDOWN_TIMEOUT, rw.handle).await {
                Ok(_) => {}
                Err(_) => {
                    // Hung scan. The cancel signal is still set, so the task
                    // will exit after the in-flight scan returns. We don't
                    // abort here because the JoinHandle was consumed by
                    // tokio::time::timeout — and abort() on a task mid-
                    // sqlx-transaction risks DB pool corruption anyway.
                    log::warn!(
                        "[Documents] Watcher did not exit within {:?}; cancel signal sent, task will exit on next yield",
                        WATCHER_SHUTDOWN_TIMEOUT
                    );
                }
            }
        }
    }

    /// Stop any existing watcher, then start a new one for the active folder.
    pub async fn start(&self, pool: DbPool) {
        self.stop().await;
        let cancel = CancellationToken::new();
        let app_handle = self.app_handle.clone();
        if let Some(handle) = start_watcher_inner(pool, app_handle, cancel.clone()).await {
            *self.inner.lock().await = Some(RunningWatcher { handle, cancel });
        }
    }
}

/// Spawn the watcher task. Returns None if no folder is configured or the
/// debouncer fails to attach to the path.
async fn start_watcher_inner(
    pool: DbPool,
    app_handle: AppHandle,
    cancel: CancellationToken,
) -> Option<tokio::task::JoinHandle<()>> {
    let folder_path_str = get_document_folder(&pool).await.ok().flatten()?.path;

    // Canonicalize so we can later compare notify event paths against the
    // watched root reliably. macOS FSEvents may surface paths through
    // `/private/var/...` even when the user selected `/var/...`.
    let watched_root = match std::fs::canonicalize(&folder_path_str) {
        Ok(p) => p,
        Err(e) => {
            log::warn!(
                "[Documents] Cannot canonicalize {}: {}; folder not watched",
                folder_path_str,
                e
            );
            return None;
        }
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DebounceEventResult>();
    let mut debouncer = match new_debouncer(DEBOUNCE_INTERVAL, None, move |result: DebounceEventResult| {
        // Send is non-blocking on unbounded; ignore errors (rx dropped → task ended).
        let _ = tx.send(result);
    }) {
        Ok(d) => d,
        Err(e) => {
            log::error!("[Documents] Failed to create debouncer: {}", e);
            return None;
        }
    };

    if let Err(e) = debouncer.watch(&watched_root, RecursiveMode::Recursive) {
        log::error!(
            "[Documents] Failed to watch {}: {}",
            watched_root.display(),
            e
        );
        return None;
    }

    log::info!("[Documents] Watching: {}", watched_root.display());

    let handle = tokio::spawn(async move {
        // Keep the debouncer alive for the duration of the task; dropping it
        // would unsubscribe from FSEvents.
        let _debouncer = debouncer;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    log::info!("[Documents] Watcher cancelled");
                    return;
                }
                msg = rx.recv() => {
                    let Some(result) = msg else {
                        // Sender dropped — should only happen on shutdown.
                        return;
                    };
                    match result {
                        Ok(events) => {
                            // 1. Root-removal detection: any Remove(Folder|Any)
                            //    event whose path canonicalizes to the watched
                            //    root means the folder itself is gone.
                            let root_event = events.iter().any(|de| {
                                matches!(
                                    de.event.kind,
                                    EventKind::Remove(RemoveKind::Folder | RemoveKind::Any)
                                ) && de.event.paths.iter().any(|p| {
                                    std::fs::canonicalize(p).ok().as_deref()
                                        == Some(&watched_root)
                                })
                            });

                            // 2. Belt-and-suspenders: re-check exists() AFTER
                            //    the debounce window. Catches cases where the
                            //    Remove event was filtered or missed (e.g.,
                            //    quick unmount/remount), and avoids false
                            //    positives on atomic-replace renames where a
                            //    Create immediately follows the Remove.
                            if root_event && !watched_root.exists() {
                                log::info!(
                                    "[Documents] Watched root no longer accessible: {}",
                                    watched_root.display()
                                );
                                let _ = app_handle.emit(
                                    FOLDER_MISSING_EVENT,
                                    serde_json::json!({
                                        "path": watched_root.to_string_lossy(),
                                    }),
                                );
                                return;
                            }

                            // 3. Even if no Remove event, a missing folder
                            //    means we shouldn't rescan (would just emit a
                            //    failed-scan event). Surface as folder-missing.
                            if !watched_root.exists() {
                                let _ = app_handle.emit(
                                    FOLDER_MISSING_EVENT,
                                    serde_json::json!({
                                        "path": watched_root.to_string_lossy(),
                                    }),
                                );
                                return;
                            }

                            // 4. need_rescan() = the debouncer dropped events
                            //    under load and recommends a full sync. We
                            //    already do a full scan_folder, so this is
                            //    informational; logged for forensics.
                            if events.iter().any(|de| de.event.need_rescan()) {
                                log::info!("[Documents] Debouncer requested rescan (event drop)");
                            }

                            run_rescan(&pool, &app_handle).await;
                        }
                        Err(errors) => {
                            for e in errors {
                                log::error!("[Documents] Watch error: {}", e);
                            }
                        }
                    }
                }
            }
        }
    });

    Some(handle)
}

/// Trigger a full rescan and emit the corresponding lifecycle events.
/// Extracted from the watcher loop for readability + reuse from tests.
async fn run_rescan(pool: &DbPool, app_handle: &AppHandle) {
    log::debug!("[Documents] Rescan triggered");
    let _ = app_handle.emit(
        "documents-scan",
        serde_json::json!({ "status": "started", "source": "watcher" }),
    );
    match scan_folder(pool).await {
        Ok(stats) => {
            let _ = app_handle.emit(
                "documents-scan",
                serde_json::json!({
                    "status": "completed",
                    "source": "watcher",
                    "file_count": stats.file_count,
                    "chunk_count": stats.chunk_count,
                    "last_scanned_at": stats.last_scanned_at,
                }),
            );
        }
        Err(e) => {
            log::error!("[Documents] Re-scan failed: {}", e);
            let _ = app_handle.emit(
                "documents-scan",
                serde_json::json!({
                    "status": "failed",
                    "source": "watcher",
                    "error": e.to_string(),
                }),
            );
        }
    }
}

/// Start watching the active document folder for changes.
/// Returns a WatcherState that can be used to stop/restart the watcher.
pub async fn start_watcher(pool: DbPool, app_handle: AppHandle) -> WatcherState {
    let state = WatcherState::new(app_handle);
    state.start(pool).await;
    state
}
