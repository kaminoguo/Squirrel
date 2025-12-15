//! Squirrel Daemon - Local-first memory system for AI coding tools.

mod cli;
mod config;
mod db;
mod error;
mod ipc;
mod mcp;
mod watcher;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        None => {
            cli::setup::run().await?;
        }
        Some(Commands::Init { skip_history }) => {
            cli::init::run(skip_history).await?;
        }
        Some(Commands::Search { query, kind, tier }) => {
            cli::search::run(&query, kind.as_deref(), tier.as_deref()).await?;
        }
        Some(Commands::Forget { id, query, confirm }) => {
            cli::forget::run(id.as_deref(), query.as_deref(), confirm).await?;
        }
        Some(Commands::Export { kind, project }) => {
            cli::export::run(kind.as_deref(), project).await?;
        }
        Some(Commands::Import { file }) => {
            cli::import::run(&file).await?;
        }
        Some(Commands::Status) => {
            cli::status::run().await?;
        }
        Some(Commands::Config { key, value }) => {
            cli::config::run(key.as_deref(), value.as_deref()).await?;
        }
        Some(Commands::Sync) => {
            cli::sync::run().await?;
        }
        Some(Commands::Flush) => {
            cli::flush::run().await?;
        }
        Some(Commands::Policy { action }) => {
            cli::policy::run(&action).await?;
        }
        Some(Commands::Mcp) => {
            cli::mcp::run().await?;
        }
        Some(Commands::Daemon) => {
            cli::daemon::run().await?;
        }
        Some(Commands::Clean {
            project,
            global,
            all,
            force,
        }) => {
            cli::clean::run(project, global, all, force).await?;
        }
    }

    Ok(())
}
