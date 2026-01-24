//! Open configuration UI.

use crate::error::Error;

/// Open the config UI in browser.
pub fn open() -> Result<(), Error> {
    // TODO: Start local web server and open browser
    // For v1, just show a message
    println!("Configuration UI not yet implemented.");
    println!();
    println!("For now, edit .sqrl/config.json directly:");
    println!("  watcher_enabled: true/false");
    println!();
    println!("Web dashboard coming in v2.");

    Ok(())
}
