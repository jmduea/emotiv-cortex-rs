//! Subscribe to motion/IMU data (accelerometer, magnetometer, quaternion).
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example motion_data
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

    let mut mot_stream = streams::subscribe_motion(&client, &token, &session.id).await?;

    println!("Streaming motion data. Press Ctrl+C to stop.");

    let mut count = 0u64;
    while let Some(motion) = mot_stream.next().await {
        count += 1;
        if count % 32 == 0 {
            println!(
                "Accel: ({:.3}, {:.3}, {:.3})  Mag: ({:.3}, {:.3}, {:.3})",
                motion.accelerometer[0],
                motion.accelerometer[1],
                motion.accelerometer[2],
                motion.magnetometer[0],
                motion.magnetometer[1],
                motion.magnetometer[2],
            );
        }
    }

    client.close_session(&token, &session.id).await?;
    client.disconnect().await?;
    Ok(())
}
