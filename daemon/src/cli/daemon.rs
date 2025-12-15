//! Background daemon.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};

use crate::config::{Config, ProjectsRegistry};
use crate::db::Database;
use crate::error::Error;
use crate::ipc::{send_ingest_chunk, MEMORY_SERVICE_SOCKET};
use crate::watcher::{commit, EventBuffer, LogWatcher, MemoryOp};

/// Buffer processing interval in seconds.
const BUFFER_PROCESS_INTERVAL_SECS: u64 = 10;

/// Run background daemon.
pub async fn run() -> Result<(), Error> {
    let config = Config::load()?;
    tracing::info!("Starting Squirrel daemon...");
    tracing::info!("Socket: {}", config.daemon.socket_path);

    // Open database
    let db_path = Config::global_db_path();
    tracing::info!("Database: {}", db_path.display());
    let db = Database::open(&db_path)?;
    let db = Arc::new(Mutex::new(db));
    tracing::info!("Database initialized");

    // Load registered projects
    let registry = ProjectsRegistry::load()?;
    tracing::info!("Watching {} projects", registry.projects.len());

    // Get owner info (default to current user)
    let owner_type = "user".to_string();
    let owner_id = whoami::username();
    tracing::info!("Owner: {}:{}", owner_type, owner_id);

    // Create channel for memory operations
    let (ops_tx, mut ops_rx) = mpsc::channel::<Vec<MemoryOp>>(100);

    // Create watcher with buffer
    let watcher = LogWatcher::new(&config, ops_tx, owner_type, owner_id)?;

    // Get buffer reference before moving watcher
    let buffer = watcher.buffer();

    // Add project paths to watch
    let mut watcher = watcher;
    for project in &registry.projects {
        if let Err(e) = watcher.watch_project(&project.root_path) {
            tracing::warn!("Failed to watch {}: {}", project.root_path.display(), e);
        }
    }

    // Create flush trigger channel
    let (flush_tx, flush_rx) = mpsc::channel::<()>(10);

    // Start IPC server with flush trigger
    let socket_path = config.daemon.socket_path.clone();
    let ipc_handle = tokio::spawn(async move {
        if let Err(e) = crate::ipc::run_server_with_flush(&socket_path, Some(flush_tx)).await {
            tracing::error!("IPC server error: {}", e);
        }
    });

    // Handle memory operations and commit to database
    let db_clone = db.clone();
    let ops_handle = tokio::spawn(async move {
        while let Some(ops) = ops_rx.recv().await {
            if ops.is_empty() {
                continue;
            }

            tracing::info!("Processing {} memory operations", ops.len());

            // Lock database and commit
            let db = db_clone.lock().await;
            let result = commit::commit_ops(&db, &ops, None);

            tracing::info!(
                "Commit result: {} added, {} updated, {} deprecated",
                result.added,
                result.updated,
                result.deprecated
            );

            for error in &result.errors {
                tracing::error!("Commit error: {}", error);
            }
        }
    });

    // Buffer processing loop - sends events to Memory Service
    let buffer_handle = tokio::spawn(process_buffer_loop(buffer, flush_rx));

    // Run watcher loop
    let watcher_handle = tokio::spawn(async move {
        watcher.run().await;
    });

    tracing::info!("Daemon running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down...");
        }
        _ = ipc_handle => {
            tracing::error!("IPC server stopped unexpectedly");
        }
        _ = watcher_handle => {
            tracing::error!("Watcher stopped unexpectedly");
        }
        _ = ops_handle => {
            tracing::error!("Memory ops handler stopped unexpectedly");
        }
        _ = buffer_handle => {
            tracing::error!("Buffer processor stopped unexpectedly");
        }
    }

    Ok(())
}

/// Process buffered events and send to Memory Service.
async fn process_buffer_loop(buffer: Arc<Mutex<EventBuffer>>, mut flush_rx: mpsc::Receiver<()>) {
    let mut interval = tokio::time::interval(Duration::from_secs(BUFFER_PROCESS_INTERVAL_SECS));

    loop {
        // Wait for either timer tick or flush signal
        tokio::select! {
            _ = interval.tick() => {}
            Some(()) = flush_rx.recv() => {
                tracing::info!("Flush signal received, processing immediately");
            }
        }

        // Get pending sessions
        let sessions: Vec<String> = {
            let buffer = buffer.lock().await;
            buffer.pending_sessions()
        };

        if sessions.is_empty() {
            continue;
        }

        tracing::debug!("Processing {} pending sessions", sessions.len());

        for session_id in sessions {
            // Build chunk request
            let request = {
                let mut buffer = buffer.lock().await;
                buffer.build_chunk_request(&session_id, None)
            };

            let Some(request) = request else {
                continue;
            };

            tracing::info!(
                "Sending chunk {} for session {} ({} events)",
                request.chunk_index,
                session_id,
                request.events.len()
            );

            // Send to Memory Service
            match send_ingest_chunk(MEMORY_SERVICE_SOCKET, &request).await {
                Ok(response) => {
                    // Process response through buffer
                    let mut buffer = buffer.lock().await;
                    if let Err(e) = buffer.process_response(&session_id, response).await {
                        tracing::error!("Failed to process response: {}", e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Memory Service unavailable, will retry: {}", e);
                    // Events remain in buffer for retry
                }
            }
        }
    }
}
