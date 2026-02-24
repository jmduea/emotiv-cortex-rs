//! Async bridge — spawns tasks that consume Cortex streams and forward
//! data as [`AppEvent`] variants into the TUI event loop channel.
//!
//! The bridge handles a two-phase startup:
//! 1. **Authenticate & discover** — authenticate → query headsets → send list.
//! 2. **Connect** (user-initiated) — connect headset → create session → subscribe.

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::streams;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::app::StreamType;
use crate::event::{AppEvent, LogEntry};

// ─── Phase 1: Authenticate & Discover ────────────────────────────────────

/// Result of a successful authenticate-and-discover sequence.
pub struct AuthResult {
    pub token: String,
}

/// Authenticate and query available headsets, sending progress events
/// to the TUI.  Does **not** connect to any headset.
pub async fn authenticate_and_discover(
    client: &emotiv_cortex_v2::CortexClient,
    config: &emotiv_cortex_v2::CortexConfig,
    tx: &mpsc::UnboundedSender<AppEvent>,
) -> Result<AuthResult, Box<dyn std::error::Error + Send + Sync>> {
    // 1. Authenticate
    tx.send(AppEvent::Log(LogEntry::info("Authenticating…")))?;

    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await?;

    tx.send(AppEvent::Log(LogEntry::info(format!(
        "Authenticated (token: {}…)",
        &token[..20.min(token.len())]
    ))))?;

    // 2. Query headsets
    tx.send(AppEvent::Log(LogEntry::info("Querying headsets…")))?;

    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await?;

    if headsets.is_empty() {
        tx.send(AppEvent::Log(LogEntry::warn(
            "No headsets found. Make sure your headset is turned on.",
        )))?;
    } else {
        tx.send(AppEvent::Log(LogEntry::info(format!(
            "Found {} headset(s)",
            headsets.len()
        ))))?;
    }

    tx.send(AppEvent::HeadsetUpdate(headsets))?;

    Ok(AuthResult { token })
}

// ─── Phase 2: Connect (user-initiated) ──────────────────────────────────

/// Result of a successful headset connection + session creation.
pub struct ConnectResult {
    pub session_id: String,
    pub headset_id: String,
    pub model: HeadsetModel,
}

/// Connect to a specific headset and create a session.
///
/// Called when the user selects a headset in the Device tab and presses
/// Enter.
pub async fn connect_headset_and_create_session(
    client: &emotiv_cortex_v2::CortexClient,
    token: &str,
    headset: &emotiv_cortex_v2::protocol::headset::HeadsetInfo,
    tx: &mpsc::UnboundedSender<AppEvent>,
) -> Result<ConnectResult, Box<dyn std::error::Error + Send + Sync>> {
    let headset_id = headset.id.clone();
    let model = HeadsetModel::from_headset_info(headset);

    // 1. Connect headset (if not already connected)
    if headset.status == "connected" {
        tx.send(AppEvent::Log(LogEntry::info(format!(
            "{headset_id} already connected"
        ))))?;
    } else {
        tx.send(AppEvent::Log(LogEntry::info(format!(
            "Connecting to {headset_id}\u{2026}"
        ))))?;
        client.connect_headset(&headset_id).await?;

        // Give the connection time to stabilize
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        tx.send(AppEvent::Log(LogEntry::info("Headset connected")))?;
    }

    // 2. Create session
    tx.send(AppEvent::Log(LogEntry::info("Creating session…")))?;

    let session = client.create_session(token, &headset_id).await?;

    tx.send(AppEvent::Log(LogEntry::info(format!(
        "Session created: {}",
        &session.id[..16.min(session.id.len())]
    ))))?;

    Ok(ConnectResult {
        session_id: session.id,
        headset_id,
        model,
    })
}

// ─── Refresh ─────────────────────────────────────────────────────────────

/// Re-query headsets and send an update event.
pub async fn refresh_headsets(
    client: &emotiv_cortex_v2::CortexClient,
    tx: &mpsc::UnboundedSender<AppEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tx.send(AppEvent::Log(LogEntry::info("Refreshing headsets…")))?;
    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await?;
    tx.send(AppEvent::Log(LogEntry::info(format!(
        "Found {} headset(s)",
        headsets.len()
    ))))?;
    tx.send(AppEvent::HeadsetUpdate(headsets))?;
    Ok(())
}

/// Subscribe to default streams (dev + metrics) and spawn forwarding tasks.
///
/// Each task reads from a `Pin<Box<dyn Stream>>` and sends parsed data
/// through the event channel.
pub async fn subscribe_default_streams(
    client: &emotiv_cortex_v2::CortexClient,
    token: &str,
    session_id: &str,
    model: &HeadsetModel,
    tx: mpsc::UnboundedSender<AppEvent>,
    shutdown: tokio::sync::broadcast::Sender<()>,
) -> Result<Vec<StreamType>, Box<dyn std::error::Error + Send + Sync>> {
    let mut subscribed = Vec::new();

    // Subscribe to device quality (always — for status bar battery/signal)
    {
        let num_ch = model.num_channels();
        let mut stream = streams::subscribe_dev(client, token, session_id, num_ch).await?;
        let tx = tx.clone();
        let mut shutdown_rx = shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    item = stream.next() => {
                        let Some(data) = item else { break };
                        if tx.send(AppEvent::DeviceQuality(data)).is_err() { break; }
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        });
        subscribed.push(StreamType::Dev);
    }

    // Subscribe to performance metrics
    {
        let mut stream = streams::subscribe_metrics(client, token, session_id).await?;
        let tx = tx.clone();
        let mut shutdown_rx = shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    item = stream.next() => {
                        let Some(data) = item else { break };
                        if tx.send(AppEvent::Metrics(data)).is_err() { break; }
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        });
        subscribed.push(StreamType::Metrics);
    }

    // Subscribe to EEG
    {
        let num_ch = model.num_channels();
        let mut stream = streams::subscribe_eeg(client, token, session_id, num_ch).await?;
        let tx = tx.clone();
        let mut shutdown_rx = shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    item = stream.next() => {
                        let Some(data) = item else { break };
                        if tx.send(AppEvent::Eeg(data)).is_err() { break; }
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        });
        subscribed.push(StreamType::Eeg);
    }

    // Subscribe to motion
    {
        let mut stream = streams::subscribe_motion(client, token, session_id).await?;
        let tx = tx.clone();
        let mut shutdown_rx = shutdown.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    item = stream.next() => {
                        let Some(data) = item else { break };
                        if tx.send(AppEvent::Motion(data)).is_err() { break; }
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        });
        subscribed.push(StreamType::Motion);
    }

    tx.send(AppEvent::Log(LogEntry::info(format!(
        "Subscribed to {} streams",
        subscribed.len()
    ))))?;

    Ok(subscribed)
}
