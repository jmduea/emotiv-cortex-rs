//! Authenticate with the Cortex API and get a token.
//!
//! Requires `EMOTIV_CLIENT_ID` and `EMOTIV_CLIENT_SECRET` env vars
//! (or a `cortex.toml` file).
//!
//! ```bash
//! EMOTIV_CLIENT_ID=xxx EMOTIV_CLIENT_SECRET=yyy cargo run --example auth
//! ```

use emotiv_cortex_v2::{CortexClient, CortexConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = CortexConfig::discover(None)?;
    println!("Connecting to Cortex at {}...", config.cortex_url);

    let mut client = CortexClient::connect(&config).await?;

    // Check if user is logged in
    let users = client.get_user_login().await?;
    println!("Logged-in users: {users:?}");

    // Authenticate
    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await?;
    println!("Authentication successful!");
    println!("Token: {}...", &token[..20.min(token.len())]);

    client.disconnect().await?;
    Ok(())
}
