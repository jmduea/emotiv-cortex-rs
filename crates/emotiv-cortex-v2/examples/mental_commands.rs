//! Subscribe to mental commands (requires a trained profile).
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example mental_commands
//! ```

use futures::StreamExt;

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::{DetectionType, ProfileAction, QueryHeadsetsOptions};
use emotiv_cortex_v2::{streams, CortexClient, CortexConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None)?;
    let mut client = CortexClient::connect(&config).await?;
    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await?;

    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await?;
    let headset = headsets.first().ok_or("No headset found")?;
    let model = HeadsetModel::from_headset_info(headset);
    println!("Using headset: {} ({})", headset.id, model);

    // List available mental command actions
    let det_info = client
        .get_detection_info(DetectionType::MentalCommand)
        .await?;
    println!("Detection info: {:?}", det_info);

    // List and load a profile
    let profiles = client.query_profiles(&token).await?;
    println!(
        "Available profiles: {:?}",
        profiles.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    if let Some(profile) = profiles.first() {
        println!("Loading profile: {}", profile.name);
        client
            .setup_profile(&token, &headset.id, &profile.name, ProfileAction::Load)
            .await?;
    } else {
        println!("No profiles found. Create one in the EMOTIV app first.");
        return Ok(());
    }

    let session = client.create_session(&token, &headset.id).await?;

    let mut com_stream = streams::subscribe_mental_commands(&client, &token, &session.id).await?;

    println!("Streaming mental commands. Press Ctrl+C to stop.\n");

    while let Some(cmd) = com_stream.next().await {
        if cmd.power > 0.0 {
            println!("Action: {:<15} Power: {:.3}", cmd.action, cmd.power);
        }
    }

    client.close_session(&token, &session.id).await?;
    client.disconnect().await?;
    Ok(())
}
