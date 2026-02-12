//! Create a recording, inject markers, stop, and export.
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example records
//! ```

use emotiv_cortex_v2::protocol::{ExportFormat, QueryHeadsetsOptions};
use emotiv_cortex_v2::{CortexClient, CortexConfig};

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
    println!("Using headset: {}", headset.id);

    let session = client.create_session(&token, &headset.id).await?;
    println!("Session: {}", session.id);

    // Start recording
    let record = client
        .create_record(&token, &session.id, "emotiv-cortex-v2 example recording")
        .await?;
    println!("Recording started: {}", record.uuid);

    // Inject some markers
    for i in 1..=3 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let marker = client
            .inject_marker(
                &token,
                &session.id,
                &format!("event_{}", i),
                i,
                "emotiv-cortex-v2-example",
                None,
            )
            .await?;
        println!("Marker injected: {} (label: event_{})", marker.uuid, i);
    }

    // Stop recording
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let stopped = client.stop_record(&token, &session.id).await?;
    println!("Recording stopped: {}", stopped.uuid);

    // Query all records
    let records = client.query_records(&token, Some(5), None).await?;
    println!("\nRecent recordings ({}):", records.len());
    for r in &records {
        println!(
            "  {} — {}",
            r.uuid,
            r.title.as_deref().unwrap_or("(untitled)")
        );
    }

    // Export to CSV (uncomment to actually export)
    // let export_dir = std::env::temp_dir().to_string_lossy().to_string();
    // client.export_record(&token, &[record.uuid.clone()], &export_dir, ExportFormat::Csv).await?;
    // println!("Exported to {}", export_dir);
    let _ = ExportFormat::Csv; // suppress unused import warning

    client.close_session(&token, &session.id).await?;
    client.disconnect().await?;
    Ok(())
}
