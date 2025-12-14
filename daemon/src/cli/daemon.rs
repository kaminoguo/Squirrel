//! Background daemon.

use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use crate::config::{Config, ProjectsRegistry};
use crate::db::Database;
use crate::error::Error;
use crate::watcher::{commit, LogWatcher, MemoryOp};

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
    let owner_id = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    tracing::info!("Owner: {}:{}", owner_type, owner_id);

    // Create channel for memory operations
    let (ops_tx, mut ops_rx) = mpsc::channel::<Vec<MemoryOp>>(100);

    // Create watcher with buffer
    let mut watcher = LogWatcher::new(&config, ops_tx, owner_type, owner_id)?;

    // Add project paths to watch
    for project in &registry.projects {
        if let Err(e) = watcher.watch_project(&project.root_path) {
            tracing::warn!("Failed to watch {}: {}", project.root_path.display(), e);
        }
    }

    // Start IPC server
    let socket_path = config.daemon.socket_path.clone();
    let ipc_handle = tokio::spawn(async move {
        if let Err(e) = crate::ipc::run_server(&socket_path).await {
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

            // Lock database and commit (using blocking task for SQLite)
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
    }

    Ok(())
}
