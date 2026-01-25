//! Squirrel daemon - local-first memory system for AI coding tools.

use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod cli;
mod config;
mod dashboard;
mod error;
mod ipc;
mod mcp;
mod storage;
mod watcher;

pub use error::Error;

#[derive(Parser)]
#[command(name = "sqrl")]
#[command(about = "Squirrel - local-first memory system for AI coding tools")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Squirrel for this project
    Init {
        /// Skip processing historical logs
        #[arg(long)]
        no_history: bool,
    },

    /// Enable the watcher daemon
    On,

    /// Disable the watcher daemon
    Off,

    /// Remove all Squirrel data from this project
    Goaway {
        /// Skip confirmation prompt
        #[arg(long, short)]
        force: bool,
    },

    /// Open configuration in browser
    Config,

    /// Show Squirrel status
    Status,

    /// Internal: run the watcher daemon (used by system service)
    #[command(hide = true)]
    WatchDaemon,

    /// Internal: run MCP server (used by AI tools)
    #[command(hide = true)]
    Mcp,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("sqrl=info".parse().unwrap()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        None => {
            // Show help when no command provided
            use clap::CommandFactory;
            Cli::command().print_help().unwrap();
            println!();
        }
        Some(Commands::Init { no_history }) => {
            cli::init::run(!no_history).await?;
        }
        Some(Commands::On) => {
            cli::daemon::enable().await?;
        }
        Some(Commands::Off) => {
            cli::daemon::disable().await?;
        }
        Some(Commands::Goaway { force }) => {
            cli::goaway::run(force).await?;
        }
        Some(Commands::Config) => {
            cli::config::open()?;
        }
        Some(Commands::Status) => {
            let exit_code = cli::status::run()?;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Some(Commands::WatchDaemon) => {
            cli::watch::run_daemon().await?;
        }
        Some(Commands::Mcp) => {
            mcp::run()?;
        }
    }

    Ok(())
}
