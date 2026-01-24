//! Watch daemon - watches Claude Code logs and sends episodes to Python service.

use std::path::PathBuf;
use std::time::Duration;

use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::error::Error;
use crate::ipc::{IpcClient, ProcessEpisodeRequest};
use crate::watcher::{
    CompletedSession, FileWatcher, LogParser, PositionStore, SessionTracker, WatchEvent,
};

/// Interval for checking idle sessions (seconds).
const IDLE_CHECK_INTERVAL_SECS: u64 = 60;

/// Interval for polling file events (milliseconds).
const POLL_INTERVAL_MS: u64 = 100;

/// Run the watcher daemon (called by system service).
pub async fn run_daemon() -> Result<(), Error> {
    info!("Starting Squirrel watcher daemon");

    // Initialize components
    let mut file_watcher = FileWatcher::new()?;
    let parser = LogParser::new();
    let mut position_store = PositionStore::new(PositionStore::default_path()?)?;
    let mut session_tracker = SessionTracker::new();
    let ipc_client = IpcClient::default();

    // Check if Python service is running
    if !ipc_client.is_service_running().await {
        warn!("Python Memory Service is not running at /tmp/sqrl_agent.sock");
        warn!("Memories won't be extracted until the service starts");
    }

    // Start watching
    file_watcher.start()?;
    info!("Watching for Claude Code log changes");

    // Track last idle check time
    let mut last_idle_check = std::time::Instant::now();

    // Main event loop
    loop {
        // Poll for file events
        while let Some(event) = file_watcher.try_recv() {
            match event {
                WatchEvent::Modified(path) | WatchEvent::Created(path) => {
                    if let Err(e) =
                        process_file(&path, &parser, &mut position_store, &mut session_tracker)
                    {
                        error!(path = %path.display(), error = %e, "Failed to process file");
                    }
                }
            }
        }

        // Check if it's time for idle session check
        if last_idle_check.elapsed() >= Duration::from_secs(IDLE_CHECK_INTERVAL_SECS) {
            last_idle_check = std::time::Instant::now();

            let completed = session_tracker.check_idle_sessions();
            for session in completed {
                send_to_service(&ipc_client, session).await;
            }

            // Periodically save position store
            if let Err(e) = position_store.save() {
                error!(error = %e, "Failed to save position store");
            }
        }

        // Small sleep to avoid busy-waiting
        sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

/// Process a log file.
fn process_file(
    path: &PathBuf,
    parser: &LogParser,
    position_store: &mut PositionStore,
    session_tracker: &mut SessionTracker,
) -> Result<(), Error> {
    let start_pos = position_store.get_start_position(path)?;
    let (entries, end_pos) = parser.parse_from_position(path, start_pos)?;

    if entries.is_empty() {
        return Ok(());
    }

    info!(
        path = %path.display(),
        entries = entries.len(),
        "Processed log entries"
    );

    // Process entries through session tracker
    for entry in entries {
        let _completed = session_tracker.process_entry(entry);
        // Note: We don't send completed sessions immediately here
        // They will be sent on the next idle check
    }

    // Update position
    position_store.set_position(path.clone(), end_pos)?;

    Ok(())
}

/// Send a completed session to the Python service.
async fn send_to_service(client: &IpcClient, session: CompletedSession) {
    if session.events.is_empty() {
        return;
    }

    info!(
        session_id = %session.session_id,
        project_id = %session.project_id,
        event_count = session.events.len(),
        "Sending session to Memory Service"
    );

    let request = ProcessEpisodeRequest {
        project_id: session.project_id,
        project_root: session.project_root,
        events: session.events,
        // TODO: Fetch existing styles and memories from storage
        existing_user_styles: vec![],
        existing_project_memories: vec![],
    };

    match client.process_episode(request).await {
        Ok(response) => {
            if response.skipped {
                info!(
                    reason = response.skip_reason.as_deref().unwrap_or("unknown"),
                    "Episode skipped"
                );
            } else {
                info!(
                    user_styles = response.user_styles.len(),
                    project_memories = response.project_memories.len(),
                    "Episode processed"
                );
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to send episode to Memory Service");
        }
    }
}
