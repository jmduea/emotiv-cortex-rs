//! Check if the Cortex service is running and get its version info.
//!
//! This is the simplest example — no authentication required.
//!
//! ```bash
//! cargo run --example cortex_info
//! ```

use emotiv_cortex_v2::{CortexClient, CortexConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None).unwrap_or_else(|_| {
        println!("No config found, using default localhost URL");
        CortexConfig::new("", "")
    });

    println!("Connecting to Cortex at {}...", config.cortex_url);

    let mut client = CortexClient::connect(&config).await?;

    let info = client.get_cortex_info().await?;
    println!("Cortex service info:");
    println!("{}", serde_json::to_string_pretty(&info)?);

    client.disconnect().await?;
    Ok(())
}
