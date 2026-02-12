//! Subscribe to performance metrics (engagement, stress, relaxation, etc.).
//!
//! Requires a license that supports performance metrics.
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example performance_metrics
//! ```

use futures::StreamExt;

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::{streams, CortexClient, CortexConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None)?;
    let mut client = CortexClient::connect(&config).await?;
    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await?;

    let headsets = client.query_headsets().await?;
    let headset = headsets.first().ok_or("No headset found")?;
    let model = HeadsetModel::from_headset_info(headset);
    println!("Using headset: {} ({})", headset.id, model);

    let session = client.create_session(&token, &headset.id).await?;

    let mut met_stream = streams::subscribe_metrics(&client, &token, &session.id).await?;

    println!("Streaming performance metrics. Press Ctrl+C to stop.\n");
    println!(
        "{:<12} {:<12} {:<12} {:<12} {:<12}",
        "Engage", "Stress", "Relax", "Interest", "Focus"
    );
    println!("{}", "-".repeat(60));

    while let Some(met) = met_stream.next().await {
        println!(
            "{:<12.3} {:<12.3} {:<12.3} {:<12.3} {:<12.3}",
            met.engagement.unwrap_or(0.0),
            met.stress.unwrap_or(0.0),
            met.relaxation.unwrap_or(0.0),
            met.interest.unwrap_or(0.0),
            met.focus.unwrap_or(0.0),
        );
    }

    client.close_session(&token, &session.id).await?;
    client.disconnect().await?;
    Ok(())
}
