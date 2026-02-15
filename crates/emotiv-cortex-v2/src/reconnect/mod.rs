//! # Resilient Client
//!
//! Production-grade wrapper around [`CortexClient`] that adds:
//!
//! - **Automatic reconnection** with configurable exponential backoff
//! - **Token management** — internal tracking with proactive refresh
//! - **Connection events** — broadcast channel for lifecycle notifications
//! - **Health monitoring** — optional background liveness checks
//!
//! ## Usage
//!
//! ```no_run
//! use emotiv_cortex_v2::{CortexConfig, reconnect::ResilientClient};
//! use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
//!
//! # async fn demo() -> emotiv_cortex_v2::CortexResult<()> {
//! let config = CortexConfig::discover(None)?;
//! let client = ResilientClient::connect(config).await?;
//!
//! // Subscribe to connection events
//! let mut events = client.event_receiver();
//! tokio::spawn(async move {
//!     while let Ok(event) = events.recv().await {
//!         println!("Connection event: {:?}", event);
//!     }
//! });
//!
//! // Use like CortexClient, but without passing tokens
//! let headsets = client.query_headsets(QueryHeadsetsOptions::default()).await?;
//! let session = client.create_session(&headsets[0].id).await?;
//! let _ = session;
//! # Ok(())
//! # }
//! ```
//!
//! ## Reconnection Behavior
//!
//! On connection loss, `ResilientClient`:
//! 1. Emits `ConnectionEvent::Disconnected`
//! 2. Enters a backoff loop (configurable base/max delay, max attempts)
//! 3. Emits `ConnectionEvent::Reconnecting` before each attempt
//! 4. On success: re-authenticates, emits `ConnectionEvent::Reconnected`
//! 5. On exhaustion: emits `ConnectionEvent::ReconnectFailed`
//!
//! **Streams are NOT auto-re-subscribed.** Consumers must listen for
//! `Reconnected` events and re-subscribe, since the session ID changes.
//!
//! ## Method Contract Template
//!
//! Wrapper methods in this module preserve the underlying [`CortexClient`]
//! endpoint semantics, while adding:
//! - token injection/refresh behavior
//! - reconnect behavior on connection-class errors
//! - connection event side effects

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use tokio::sync::{RwLock, broadcast};
use tokio::time::Instant;

use crate::client::CortexClient;
use crate::config::CortexConfig;
use crate::error::CortexResult;
use crate::health::HealthMonitor;

mod endpoints;
mod operation_layer;
mod reconnect_layer;
mod token_layer;

/// Token refresh interval — re-authenticate before the token expires.
const TOKEN_REFRESH_INTERVAL: Duration = Duration::from_secs(55 * 60); // 55 minutes

/// Connection lifecycle events emitted by [`ResilientClient`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// Successfully connected and authenticated.
    Connected,

    /// Connection was lost.
    Disconnected { reason: String },

    /// Attempting to reconnect (includes the attempt number).
    Reconnecting { attempt: u32 },

    /// Successfully reconnected and re-authenticated.
    Reconnected,

    /// All reconnection attempts exhausted.
    ReconnectFailed { attempts: u32, last_error: String },
}

/// Internal state holding the active client and authentication info.
struct ClientState {
    client: Arc<CortexClient>,
    cortex_token: String,
    token_obtained_at: Instant,
}

/// Production-grade Cortex API client with automatic reconnection
/// and token management.
///
/// All API methods internally manage the Cortex token — callers never
/// pass `cortex_token` parameters. On transient connection failures,
/// the client automatically reconnects and re-authenticates.
///
/// See [module docs](self) for usage examples.
pub struct ResilientClient {
    config: CortexConfig,
    state: RwLock<ClientState>,
    event_tx: broadcast::Sender<ConnectionEvent>,
    reconnecting: Arc<AtomicBool>,
    health_monitor: std::sync::Mutex<Option<HealthMonitor>>,
}

impl ResilientClient {
    /// Connect to the Cortex API and authenticate.
    ///
    /// This establishes the WebSocket connection, performs the full
    /// authentication flow, and optionally starts the health monitor.
    ///
    /// # Errors
    /// Returns any error produced by the underlying Cortex API call,
    /// including connection, authentication, protocol, timeout, and configuration errors.
    pub async fn connect(config: CortexConfig) -> CortexResult<Self> {
        let client = CortexClient::connect(&config).await?;
        let cortex_token = client
            .authenticate(&config.client_id, &config.client_secret)
            .await?;

        let (event_tx, _) = broadcast::channel(64);
        let _ = event_tx.send(ConnectionEvent::Connected);

        let state = ClientState {
            client: Arc::new(client),
            cortex_token,
            token_obtained_at: Instant::now(),
        };

        let resilient = Self {
            config,
            state: RwLock::new(state),
            event_tx,
            reconnecting: Arc::new(AtomicBool::new(false)),
            health_monitor: std::sync::Mutex::new(None),
        };

        // Start health monitor if enabled
        if resilient.config.health.enabled {
            resilient.start_health_monitor().await;
        }

        Ok(resilient)
    }

    /// Subscribe to connection lifecycle events.
    pub fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_event_variants() {
        let connected = ConnectionEvent::Connected;
        let disconnected = ConnectionEvent::Disconnected {
            reason: "test".into(),
        };
        let reconnecting = ConnectionEvent::Reconnecting { attempt: 1 };
        let reconnected = ConnectionEvent::Reconnected;
        let failed = ConnectionEvent::ReconnectFailed {
            attempts: 3,
            last_error: "timeout".into(),
        };

        assert_eq!(connected, ConnectionEvent::Connected);
        assert_ne!(connected, reconnected);
        assert_eq!(
            disconnected,
            ConnectionEvent::Disconnected {
                reason: "test".into()
            }
        );
        assert_eq!(reconnecting, ConnectionEvent::Reconnecting { attempt: 1 });
        assert_ne!(reconnecting, ConnectionEvent::Reconnecting { attempt: 2 });
        assert_eq!(
            failed,
            ConnectionEvent::ReconnectFailed {
                attempts: 3,
                last_error: "timeout".into()
            }
        );
    }

    #[test]
    fn test_token_refresh_interval() {
        // 55 minutes
        assert_eq!(TOKEN_REFRESH_INTERVAL, Duration::from_secs(55 * 60));
    }
}
