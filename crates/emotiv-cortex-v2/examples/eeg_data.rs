//! Subscribe to raw EEG data and print samples.
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example eeg_data
//! ```

use futures_util::StreamExt;

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::{CortexClient, CortexConfig, streams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None)?;
    let mut client = CortexClient::connect(&config).await?;
    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await?;

    // Find and connect to the first headset
    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await?;
    let headset = headsets.first().ok_or("No headset found")?;
    let model = HeadsetModel::from_headset_info(headset);
    println!("Using headset: {} ({})", headset.id, model);

    // Create a session
    let session = client.create_session(&token, &headset.id).await?;
    println!("Session: {}", session.id);

    // Subscribe to EEG data
    let mut eeg_stream =
        streams::subscribe_eeg(&client, &token, &session.id, model.num_channels()).await?;

    println!(
        "Streaming EEG data ({} channels at {} Hz). Press Ctrl+C to stop.",
        model.num_channels(),
        model.sampling_rate_hz()
    );

    let mut count = 0u64;
    while let Some(eeg_data) = eeg_stream.next().await {
        count += 1;
        if count % 128 == 0 {
            // Print every ~1 second at 128 Hz
            let channels: Vec<String> = eeg_data
                .channels
                .iter()
                .map(|v| format!("{:.2}", v))
                .collect();
            println!(
                "[sample {}] EEG: [{}]",
                eeg_data.counter,
                channels.join(", ")
            );
        }
    }

    client.close_session(&token, &session.id).await?;
    client.disconnect().await?;
    Ok(())
}
