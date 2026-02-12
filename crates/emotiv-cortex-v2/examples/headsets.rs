//! Discover and list connected Emotiv headsets.
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example headsets
//! ```

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::{CortexClient, CortexConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None)?;
    let mut client = CortexClient::connect(&config).await?;
    let _token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await?;

    // Trigger a Bluetooth scan
    println!("Scanning for headsets...");
    if let Err(e) = client.refresh_headsets().await {
        println!("  (refresh warning: {})", e);
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Query discovered headsets
    let headsets = client.query_headsets().await?;

    if headsets.is_empty() {
        println!("No headsets found. Make sure your headset is powered on.");
    } else {
        println!("Found {} headset(s):", headsets.len());
        for h in &headsets {
            let model = HeadsetModel::from_headset_info(h);
            let config = model.channel_config();
            println!("  {} ({}):", h.id, h.status);
            println!("    Model:       {}", model);
            println!("    Channels:    {} ({})", model.num_channels(), model.channel_names().join(", "));
            println!("    Sample rate: {} Hz", config.sampling_rate_hz);
            if let Some(fw) = &h.firmware {
                println!("    Firmware:    {}", fw);
            }
        }
    }

    client.disconnect().await?;
    Ok(())
}
