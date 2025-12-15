//! Global setup and first-run wizard.

use crate::config::Config;
use crate::db::Database;
use crate::error::Error;

/// Run global setup.
pub async fn run() -> Result<(), Error> {
    println!("Squirrel - Local-first memory for AI coding tools");
    println!();

    // Check/create global directory
    let global_dir = Config::global_dir();
    let is_first_run = !global_dir.exists();

    if is_first_run {
        println!("First-time setup...");
        std::fs::create_dir_all(&global_dir)?;
        println!("  Created ~/.sqrl/");
    }

    // Initialize global database
    let db_path = Config::global_db_path();
    if !db_path.exists() {
        let _db = Database::open(&db_path)?;
        println!("  Initialized global database");
    }

    // Load or create config
    let config = Config::load()?;

    // Show status
    println!();
    println!("Configuration (~/.sqrl/config.toml):");
    println!("  strong_model: {}", config.llm.strong_model);
    println!("  fast_model: {}", config.llm.fast_model);
    println!("  embedding_model: {}", config.llm.embedding_model);
    println!();

    // Check API key
    let has_api_key = !config.llm.openrouter_api_key.is_empty()
        || !config.llm.openai_api_key.is_empty()
        || !config.llm.anthropic_api_key.is_empty();

    if has_api_key {
        println!("  API key: configured");
    } else {
        println!("  API key: NOT SET");
        println!();
        println!("To use Squirrel, set your API key:");
        println!("  sqrl config llm.openrouter_api_key <your-key>");
        println!();
        println!("Get an OpenRouter key at: https://openrouter.ai/keys");
    }

    // Show next steps
    println!();
    if has_api_key {
        println!("Ready! Next steps:");
        println!("  1. cd <your-project>");
        println!("  2. sqrl init");
        println!();
        println!("Commands:");
        println!("  sqrl init      Initialize project");
        println!("  sqrl search    Search memories");
        println!("  sqrl config    View/edit settings");
        println!("  sqrl status    Show status");
        println!("  sqrl --help    All commands");
    } else {
        println!("After setting API key:");
        println!("  1. cd <your-project>");
        println!("  2. sqrl init");
    }

    Ok(())
}
