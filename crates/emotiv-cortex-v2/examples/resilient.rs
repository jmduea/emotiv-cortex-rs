//! Demonstrates the `ResilientClient` with auto-reconnect and event monitoring.
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example resilient
//! ```

use emotiv_cortex_v2::CortexConfig;
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::reconnect::{ConnectionEvent, ResilientClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None)?;
    println!("Connecting with ResilientClient...");

    let client = ResilientClient::connect(config).await?;

    // Monitor connection events in background
    let mut events = client.event_receiver();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                ConnectionEvent::Connected => println!("[event] Connected"),
                ConnectionEvent::Disconnected { reason } => {
                    println!("[event] Disconnected: {reason}");
                }
                ConnectionEvent::Reconnecting { attempt } => {
                    println!("[event] Reconnecting (attempt {attempt})");
                }
                ConnectionEvent::Reconnected => println!("[event] Reconnected!"),
                ConnectionEvent::ReconnectFailed {
                    attempts,
                    last_error,
                } => println!("[event] Reconnect failed after {attempts} attempts: {last_error}"),
            }
        }
    });

    // Use the client
    let info = client.get_cortex_info().await?;
    println!("Cortex info: {info}");

    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await?;
    println!("Found {} headset(s)", headsets.len());

    for h in &headsets {
        println!("  {} ({})", h.id, h.status);
    }

    // Periodically poll the API so that disconnections are detected
    // and auto-reconnect can trigger. Try restarting the EMOTIV Launcher
    // while this is running to see reconnection in action.
    println!("\nPolling Cortex every 5s. Press Ctrl+C to exit.");
    println!("Try restarting the EMOTIV Launcher to see auto-reconnect in action.\n");

    let poll_client = &client;
    let poll_loop = async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            match poll_client.get_cortex_info().await {
                Ok(_) => println!("[poll] Cortex is reachable"),
                Err(e) => println!("[poll] Error (reconnect may follow): {e}"),
            }
        }
    };

    tokio::select! {
        _ = poll_loop => {}
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down...");
        }
    }

    client.disconnect().await?;
    println!("Disconnected.");
    Ok(())
}
