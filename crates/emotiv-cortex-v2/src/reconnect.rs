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
//! use emotiv_cortex_v2::protocol::QueryHeadsetsOptions;
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, RwLock};
use tokio::time::Instant;

use crate::client::CortexClient;
use crate::config::CortexConfig;
use crate::error::{CortexError, CortexResult};
use crate::health::{HealthMonitor, HealthStatus};
use crate::protocol::{
    ConfigMappingRequest, ConfigMappingResponse, CurrentProfileInfo, DemographicAttribute,
    DetectionInfo, DetectionType, ExportFormat, HeadsetClockSyncResult, HeadsetInfo, MarkerInfo,
    ProfileAction, ProfileInfo, QueryHeadsetsOptions, RecordInfo, SessionInfo, SubjectInfo,
    TrainedSignatureActions, TrainingStatus, TrainingTime, UserLoginInfo,
};

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

    /// Returns the current Cortex token (for advanced use cases).
    pub async fn cortex_token(&self) -> String {
        self.state.read().await.cortex_token.clone()
    }

    // ─── Internal helpers ────────────────────────────────────────────────

    /// Get a clone of the Arc<CortexClient> and the current token.
    async fn client_and_token(&self) -> (Arc<CortexClient>, String) {
        let state = self.state.read().await;
        (Arc::clone(&state.client), state.cortex_token.clone())
    }

    /// Get a clone of the Arc<CortexClient>.
    async fn client(&self) -> Arc<CortexClient> {
        Arc::clone(&self.state.read().await.client)
    }

    /// Check if the token should be refreshed and do so if needed.
    async fn maybe_refresh_token(&self) -> CortexResult<()> {
        let needs_refresh = {
            let state = self.state.read().await;
            state.token_obtained_at.elapsed() > TOKEN_REFRESH_INTERVAL
        };

        if needs_refresh {
            tracing::info!("Proactively refreshing Cortex token");
            let mut state = self.state.write().await;
            // Double-check after acquiring write lock
            if state.token_obtained_at.elapsed() > TOKEN_REFRESH_INTERVAL {
                match state
                    .client
                    .authenticate(&self.config.client_id, &self.config.client_secret)
                    .await
                {
                    Ok(new_token) => {
                        state.cortex_token = new_token;
                        state.token_obtained_at = Instant::now();
                        tracing::info!("Token refreshed successfully");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Token refresh failed, will retry on next call");
                    }
                }
            }
        }

        Ok(())
    }

    /// Start the background health monitor.
    async fn start_health_monitor(&self) {
        let client = self.client().await;
        let (monitor, mut rx) = HealthMonitor::start(client, &self.config.health);

        // Spawn a task to process health events
        let event_tx = self.event_tx.clone();
        let reconnecting = Arc::clone(&self.reconnecting);

        tokio::spawn(async move {
            while let Some(status) = rx.recv().await {
                if let HealthStatus::Unhealthy { .. } = status {
                    if !reconnecting.load(Ordering::SeqCst) {
                        tracing::warn!("Health monitor detected unhealthy connection");
                        let _ = event_tx.send(ConnectionEvent::Disconnected {
                            reason: "Health check failures exceeded threshold".into(),
                        });
                    }
                }
            }
        });

        if let Ok(mut guard) = self.health_monitor.lock() {
            *guard = Some(monitor);
        }
    }

    /// Attempt to reconnect with exponential backoff.
    async fn reconnect(&self) -> CortexResult<()> {
        // Prevent concurrent reconnection attempts
        if self
            .reconnecting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            // Another task is already reconnecting — wait for it
            while self.reconnecting.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Check if the reconnection succeeded
            if self.client().await.is_connected() {
                return Ok(());
            }
            return Err(CortexError::ConnectionLost {
                reason: "Concurrent reconnection failed".into(),
            });
        }

        let _guard = ReconnectGuard(&self.reconnecting);

        let _ = self.event_tx.send(ConnectionEvent::Disconnected {
            reason: "Connection lost, initiating reconnection".into(),
        });

        // Stop health monitor during reconnection
        if let Ok(mut guard) = self.health_monitor.lock() {
            if let Some(mut monitor) = guard.take() {
                tokio::spawn(async move { monitor.stop().await });
            }
        }

        let reconnect = &self.config.reconnect;
        let mut delay = Duration::from_secs(reconnect.base_delay_secs);
        let max_delay = Duration::from_secs(reconnect.max_delay_secs);
        let max_attempts = if reconnect.max_attempts == 0 {
            u32::MAX // unlimited
        } else {
            reconnect.max_attempts
        };

        for attempt in 1..=max_attempts {
            let _ = self
                .event_tx
                .send(ConnectionEvent::Reconnecting { attempt });

            tracing::info!(
                attempt,
                max_attempts = if reconnect.max_attempts == 0 {
                    "unlimited".to_string()
                } else {
                    max_attempts.to_string()
                },
                "Attempting reconnection"
            );

            match CortexClient::connect(&self.config).await {
                Ok(new_client) => {
                    match new_client
                        .authenticate(&self.config.client_id, &self.config.client_secret)
                        .await
                    {
                        Ok(new_token) => {
                            let new_client = Arc::new(new_client);

                            // Update state
                            {
                                let mut state = self.state.write().await;
                                *state = ClientState {
                                    client: Arc::clone(&new_client),
                                    cortex_token: new_token,
                                    token_obtained_at: Instant::now(),
                                };
                            }

                            let _ = self.event_tx.send(ConnectionEvent::Reconnected);
                            tracing::info!(attempt, "Reconnected and re-authenticated");

                            // Restart health monitor
                            if self.config.health.enabled {
                                self.start_health_monitor().await;
                            }

                            return Ok(());
                        }
                        Err(e) => {
                            tracing::warn!(
                                attempt,
                                error = %e,
                                "Connected but authentication failed"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "Reconnection attempt failed");
                }
            }

            if attempt < max_attempts {
                tracing::debug!(
                    delay_ms = delay.as_millis() as u64,
                    "Backing off before retry"
                );
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }

        let _ = self.event_tx.send(ConnectionEvent::ReconnectFailed {
            attempts: max_attempts,
            last_error: "All reconnection attempts exhausted".into(),
        });

        Err(CortexError::RetriesExhausted {
            attempts: max_attempts,
            last_error: Box::new(CortexError::ConnectionLost {
                reason: "All reconnection attempts exhausted".into(),
            }),
        })
    }

    /// Execute a token-free operation with automatic reconnection.
    async fn exec<F, Fut, T>(&self, f: F) -> CortexResult<T>
    where
        F: Fn(Arc<CortexClient>) -> Fut,
        Fut: std::future::Future<Output = CortexResult<T>>,
    {
        let client = self.client().await;
        match f(client).await {
            Ok(result) => Ok(result),
            Err(e) if e.is_connection_error() && self.config.reconnect.enabled => {
                self.reconnect().await?;
                let client = self.client().await;
                f(client).await
            }
            Err(e) => Err(e),
        }
    }

    /// Execute a token-requiring operation with automatic reconnection
    /// and token management.
    async fn exec_with_token<F, Fut, T>(&self, f: F) -> CortexResult<T>
    where
        F: Fn(Arc<CortexClient>, String) -> Fut,
        Fut: std::future::Future<Output = CortexResult<T>>,
    {
        self.maybe_refresh_token().await?;

        let (client, token) = self.client_and_token().await;
        match f(client, token).await {
            Ok(result) => Ok(result),
            Err(e) if e.is_connection_error() && self.config.reconnect.enabled => {
                self.reconnect().await?;
                let (client, token) = self.client_and_token().await;
                f(client, token).await
            }
            Err(e) => Err(e),
        }
    }

    // ─── Authentication ─────────────────────────────────────────────────

    /// Query Cortex service info. No authentication required.
    pub async fn get_cortex_info(&self) -> CortexResult<serde_json::Value> {
        self.exec(|c| async move { c.get_cortex_info().await })
            .await
    }

    /// Check if the application has access rights.
    pub async fn has_access_right(&self) -> CortexResult<bool> {
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        self.exec(move |c| {
            let id = client_id.clone();
            let secret = client_secret.clone();
            async move { c.has_access_right(&id, &secret).await }
        })
        .await
    }

    /// Get the currently logged-in Emotiv user.
    pub async fn get_user_login(&self) -> CortexResult<Vec<UserLoginInfo>> {
        self.exec(|c| async move { c.get_user_login().await }).await
    }

    /// Generate a new cortex token (or refresh an existing one).
    ///
    /// On success, also updates the internal token and refresh timestamp.
    pub async fn generate_new_token(&self) -> CortexResult<String> {
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let new_token = self
            .exec_with_token(move |c, token| {
                let id = client_id.clone();
                let secret = client_secret.clone();
                async move { c.generate_new_token(&token, &id, &secret).await }
            })
            .await?;

        // Update internal token state
        let mut state = self.state.write().await;
        state.cortex_token = new_token.clone();
        state.token_obtained_at = Instant::now();

        Ok(new_token)
    }

    /// Get information about the current user.
    pub async fn get_user_info(&self) -> CortexResult<serde_json::Value> {
        self.exec_with_token(|c, token| async move { c.get_user_info(&token).await })
            .await
    }

    /// Get information about the license used by the application.
    pub async fn get_license_info(&self) -> CortexResult<serde_json::Value> {
        self.exec_with_token(|c, token| async move { c.get_license_info(&token).await })
            .await
    }

    // ─── Headset Management ─────────────────────────────────────────────

    /// Query available headsets.
    pub async fn query_headsets(
        &self,
        options: QueryHeadsetsOptions,
    ) -> CortexResult<Vec<HeadsetInfo>> {
        self.exec(move |c| {
            let options = options.clone();
            async move { c.query_headsets(options).await }
        })
        .await
    }

    /// Connect to a headset.
    pub async fn connect_headset(&self, headset_id: &str) -> CortexResult<()> {
        let id = headset_id.to_string();
        self.exec(move |c| {
            let id = id.clone();
            async move { c.connect_headset(&id).await }
        })
        .await
    }

    /// Disconnect a headset.
    pub async fn disconnect_headset(&self, headset_id: &str) -> CortexResult<()> {
        let id = headset_id.to_string();
        self.exec(move |c| {
            let id = id.clone();
            async move { c.disconnect_headset(&id).await }
        })
        .await
    }

    /// Trigger headset scanning / refresh.
    pub async fn refresh_headsets(&self) -> CortexResult<()> {
        self.exec(|c| async move { c.refresh_headsets().await })
            .await
    }

    /// Synchronize system clock with headset clock.
    pub async fn sync_with_headset_clock(
        &self,
        headset_id: &str,
    ) -> CortexResult<HeadsetClockSyncResult> {
        let id = headset_id.to_string();
        self.exec(move |c| {
            let id = id.clone();
            async move { c.sync_with_headset_clock(&id).await }
        })
        .await
    }

    /// Manage EEG channel mapping configurations for an EPOC Flex headset.
    pub async fn config_mapping(
        &self,
        request: ConfigMappingRequest,
    ) -> CortexResult<ConfigMappingResponse> {
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.config_mapping(&token, request).await }
        })
        .await
    }

    /// Update settings of an EPOC+ or EPOC X headset.
    pub async fn update_headset(
        &self,
        headset_id: &str,
        setting: serde_json::Value,
    ) -> CortexResult<serde_json::Value> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            let setting = setting.clone();
            async move { c.update_headset(&token, &id, setting).await }
        })
        .await
    }

    /// Update the headband position or custom name of an EPOC X headset.
    pub async fn update_headset_custom_info(
        &self,
        headset_id: &str,
        headband_position: Option<&str>,
        custom_name: Option<&str>,
    ) -> CortexResult<serde_json::Value> {
        let id = headset_id.to_string();
        let pos = headband_position.map(|s| s.to_string());
        let name = custom_name.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            let pos = pos.clone();
            let name = name.clone();
            async move {
                c.update_headset_custom_info(&token, &id, pos.as_deref(), name.as_deref())
                    .await
            }
        })
        .await
    }

    // ─── Session Management ─────────────────────────────────────────────

    /// Create a session for a headset.
    pub async fn create_session(&self, headset_id: &str) -> CortexResult<SessionInfo> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.create_session(&token, &id).await }
        })
        .await
    }

    /// Query existing sessions.
    pub async fn query_sessions(&self) -> CortexResult<Vec<SessionInfo>> {
        self.exec_with_token(|c, token| async move { c.query_sessions(&token).await })
            .await
    }

    /// Close a session.
    pub async fn close_session(&self, session_id: &str) -> CortexResult<()> {
        let id = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.close_session(&token, &id).await }
        })
        .await
    }

    // ─── Data Streams ───────────────────────────────────────────────────

    /// Create data stream channels for the specified streams.
    ///
    /// This delegates to the underlying [`CortexClient::create_stream_channels`].
    pub async fn create_stream_channels(&self, streams: &[&str]) -> crate::client::StreamReceivers {
        self.client().await.create_stream_channels(streams)
    }

    /// Subscribe to data streams.
    pub async fn subscribe_streams(&self, session_id: &str, streams: &[&str]) -> CortexResult<()> {
        let sid = session_id.to_string();
        let stream_names: Vec<String> = streams.iter().map(|s| s.to_string()).collect();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let names = stream_names.clone();
            async move {
                let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                c.subscribe_streams(&token, &sid, &refs).await
            }
        })
        .await
    }

    /// Unsubscribe from data streams.
    pub async fn unsubscribe_streams(
        &self,
        session_id: &str,
        streams: &[&str],
    ) -> CortexResult<()> {
        let sid = session_id.to_string();
        let stream_names: Vec<String> = streams.iter().map(|s| s.to_string()).collect();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let names = stream_names.clone();
            async move {
                let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                c.unsubscribe_streams(&token, &sid, &refs).await
            }
        })
        .await
    }

    // ─── Records ────────────────────────────────────────────────────────

    /// Start a new recording.
    pub async fn create_record(&self, session_id: &str, title: &str) -> CortexResult<RecordInfo> {
        let sid = session_id.to_string();
        let t = title.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let t = t.clone();
            async move { c.create_record(&token, &sid, &t).await }
        })
        .await
    }

    /// Stop an active recording.
    pub async fn stop_record(&self, session_id: &str) -> CortexResult<RecordInfo> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.stop_record(&token, &sid).await }
        })
        .await
    }

    /// Query recorded sessions.
    pub async fn query_records(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> CortexResult<Vec<RecordInfo>> {
        self.exec_with_token(
            move |c, token| async move { c.query_records(&token, limit, offset).await },
        )
        .await
    }

    /// Export a recording to CSV or EDF format.
    pub async fn export_record(
        &self,
        record_ids: &[String],
        folder: &str,
        format: ExportFormat,
    ) -> CortexResult<()> {
        let ids = record_ids.to_vec();
        let f = folder.to_string();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            let f = f.clone();
            async move { c.export_record(&token, &ids, &f, format).await }
        })
        .await
    }

    /// Update a recording's metadata (title, description, tags).
    pub async fn update_record(
        &self,
        record_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        tags: Option<&[String]>,
    ) -> CortexResult<RecordInfo> {
        let rid = record_id.to_string();
        let t = title.map(|s| s.to_string());
        let d = description.map(|s| s.to_string());
        let tg = tags.map(|s| s.to_vec());
        self.exec_with_token(move |c, token| {
            let rid = rid.clone();
            let t = t.clone();
            let d = d.clone();
            let tg = tg.clone();
            async move {
                c.update_record(&token, &rid, t.as_deref(), d.as_deref(), tg.as_deref())
                    .await
            }
        })
        .await
    }

    /// Delete one or more recordings.
    pub async fn delete_record(&self, record_ids: &[String]) -> CortexResult<serde_json::Value> {
        let ids = record_ids.to_vec();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            async move { c.delete_record(&token, &ids).await }
        })
        .await
    }

    /// Get detailed information for specific records by their IDs.
    pub async fn get_record_infos(&self, record_ids: &[String]) -> CortexResult<serde_json::Value> {
        let ids = record_ids.to_vec();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            async move { c.get_record_infos(&token, &ids).await }
        })
        .await
    }

    /// Configure the opt-out setting for data sharing.
    pub async fn config_opt_out(
        &self,
        status: &str,
        new_opt_out: Option<bool>,
    ) -> CortexResult<serde_json::Value> {
        let st = status.to_string();
        self.exec_with_token(move |c, token| {
            let st = st.clone();
            async move { c.config_opt_out(&token, &st, new_opt_out).await }
        })
        .await
    }

    /// Request to download recorded data from the Emotiv cloud.
    pub async fn download_record(&self, record_ids: &[String]) -> CortexResult<serde_json::Value> {
        let ids = record_ids.to_vec();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            async move { c.download_record(&token, &ids).await }
        })
        .await
    }

    // ─── Markers ────────────────────────────────────────────────────────

    /// Inject a time-stamped marker.
    pub async fn inject_marker(
        &self,
        session_id: &str,
        label: &str,
        value: i32,
        port: &str,
        time: Option<f64>,
    ) -> CortexResult<MarkerInfo> {
        let sid = session_id.to_string();
        let l = label.to_string();
        let p = port.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let l = l.clone();
            let p = p.clone();
            async move { c.inject_marker(&token, &sid, &l, value, &p, time).await }
        })
        .await
    }

    /// Update a marker (convert instance to interval marker).
    pub async fn update_marker(
        &self,
        session_id: &str,
        marker_id: &str,
        time: Option<f64>,
    ) -> CortexResult<()> {
        let sid = session_id.to_string();
        let mid = marker_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let mid = mid.clone();
            async move { c.update_marker(&token, &sid, &mid, time).await }
        })
        .await
    }

    // ─── Subjects ────────────────────────────────────────────────────────

    /// Create a new subject.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_subject(
        &self,
        subject_name: &str,
        date_of_birth: Option<&str>,
        sex: Option<&str>,
        country_code: Option<&str>,
        state: Option<&str>,
        city: Option<&str>,
        attributes: Option<&[serde_json::Value]>,
    ) -> CortexResult<SubjectInfo> {
        let name = subject_name.to_string();
        let dob = date_of_birth.map(|s| s.to_string());
        let sx = sex.map(|s| s.to_string());
        let cc = country_code.map(|s| s.to_string());
        let st = state.map(|s| s.to_string());
        let ct = city.map(|s| s.to_string());
        let attrs = attributes.map(|a| a.to_vec());
        self.exec_with_token(move |c, token| {
            let name = name.clone();
            let dob = dob.clone();
            let sx = sx.clone();
            let cc = cc.clone();
            let st = st.clone();
            let ct = ct.clone();
            let attrs = attrs.clone();
            async move {
                c.create_subject(
                    &token,
                    &name,
                    dob.as_deref(),
                    sx.as_deref(),
                    cc.as_deref(),
                    st.as_deref(),
                    ct.as_deref(),
                    attrs.as_deref(),
                )
                .await
            }
        })
        .await
    }

    /// Update an existing subject's information.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_subject(
        &self,
        subject_name: &str,
        date_of_birth: Option<&str>,
        sex: Option<&str>,
        country_code: Option<&str>,
        state: Option<&str>,
        city: Option<&str>,
        attributes: Option<&[serde_json::Value]>,
    ) -> CortexResult<SubjectInfo> {
        let name = subject_name.to_string();
        let dob = date_of_birth.map(|s| s.to_string());
        let sx = sex.map(|s| s.to_string());
        let cc = country_code.map(|s| s.to_string());
        let st = state.map(|s| s.to_string());
        let ct = city.map(|s| s.to_string());
        let attrs = attributes.map(|a| a.to_vec());
        self.exec_with_token(move |c, token| {
            let name = name.clone();
            let dob = dob.clone();
            let sx = sx.clone();
            let cc = cc.clone();
            let st = st.clone();
            let ct = ct.clone();
            let attrs = attrs.clone();
            async move {
                c.update_subject(
                    &token,
                    &name,
                    dob.as_deref(),
                    sx.as_deref(),
                    cc.as_deref(),
                    st.as_deref(),
                    ct.as_deref(),
                    attrs.as_deref(),
                )
                .await
            }
        })
        .await
    }

    /// Delete one or more subjects.
    pub async fn delete_subjects(
        &self,
        subject_names: &[String],
    ) -> CortexResult<serde_json::Value> {
        let names = subject_names.to_vec();
        self.exec_with_token(move |c, token| {
            let names = names.clone();
            async move { c.delete_subjects(&token, &names).await }
        })
        .await
    }

    /// Query subjects with filtering, sorting, and pagination.
    pub async fn query_subjects(
        &self,
        query: serde_json::Value,
        order_by: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> CortexResult<(Vec<SubjectInfo>, u32)> {
        self.exec_with_token(move |c, token| {
            let query = query.clone();
            let order_by = order_by.clone();
            async move {
                c.query_subjects(&token, query, order_by, limit, offset)
                    .await
            }
        })
        .await
    }

    /// Get the list of valid demographic attributes.
    pub async fn get_demographic_attributes(&self) -> CortexResult<Vec<DemographicAttribute>> {
        self.exec_with_token(|c, token| async move { c.get_demographic_attributes(&token).await })
            .await
    }

    // ─── Profiles ───────────────────────────────────────────────────────

    /// List all profiles for the current user.
    pub async fn query_profiles(&self) -> CortexResult<Vec<ProfileInfo>> {
        self.exec_with_token(|c, token| async move { c.query_profiles(&token).await })
            .await
    }

    /// Get the profile currently loaded for a headset.
    pub async fn get_current_profile(&self, headset_id: &str) -> CortexResult<CurrentProfileInfo> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.get_current_profile(&token, &id).await }
        })
        .await
    }

    /// Manage a profile (create, load, unload, save, rename, delete).
    pub async fn setup_profile(
        &self,
        headset_id: &str,
        profile_name: &str,
        action: ProfileAction,
    ) -> CortexResult<()> {
        let hid = headset_id.to_string();
        let pname = profile_name.to_string();
        self.exec_with_token(move |c, token| {
            let hid = hid.clone();
            let pname = pname.clone();
            async move { c.setup_profile(&token, &hid, &pname, action).await }
        })
        .await
    }

    /// Load an empty guest profile for a headset.
    pub async fn load_guest_profile(&self, headset_id: &str) -> CortexResult<()> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.load_guest_profile(&token, &id).await }
        })
        .await
    }

    // ─── BCI / Training ─────────────────────────────────────────────────

    /// Get detection info for a detection type.
    pub async fn get_detection_info(
        &self,
        detection: DetectionType,
    ) -> CortexResult<DetectionInfo> {
        self.exec(move |c| async move { c.get_detection_info(detection).await })
            .await
    }

    /// Control the training lifecycle.
    pub async fn training(
        &self,
        session_id: &str,
        detection: DetectionType,
        status: TrainingStatus,
        action: &str,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        let act = action.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let act = act.clone();
            async move { c.training(&token, &sid, detection, status, &act).await }
        })
        .await
    }

    /// Get or set active mental command actions.
    pub async fn mental_command_active_action(
        &self,
        session_id: &str,
        actions: Option<&[&str]>,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        let owned_actions: Option<Vec<String>> =
            actions.map(|a| a.iter().map(|s| s.to_string()).collect());
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let owned_actions = owned_actions.clone();
            async move {
                let refs: Option<Vec<&str>> = owned_actions
                    .as_ref()
                    .map(|v| v.iter().map(|s| s.as_str()).collect());
                c.mental_command_active_action(&token, &sid, refs.as_deref())
                    .await
            }
        })
        .await
    }

    /// Get or set the mental command action sensitivity.
    pub async fn mental_command_action_sensitivity(
        &self,
        session_id: &str,
        values: Option<&[i32]>,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        let owned_values: Option<Vec<i32>> = values.map(|v| v.to_vec());
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let owned_values = owned_values.clone();
            async move {
                c.mental_command_action_sensitivity(&token, &sid, owned_values.as_deref())
                    .await
            }
        })
        .await
    }

    /// Get the mental command brain map.
    pub async fn mental_command_brain_map(
        &self,
        session_id: &str,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.mental_command_brain_map(&token, &sid).await }
        })
        .await
    }

    /// Get or set the mental command training threshold.
    pub async fn mental_command_training_threshold(
        &self,
        session_id: &str,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.mental_command_training_threshold(&token, &sid).await }
        })
        .await
    }

    /// Get or set the mental command training threshold for a profile.
    pub async fn mental_command_training_threshold_for_profile(
        &self,
        profile: &str,
        status: Option<&str>,
        value: Option<f64>,
    ) -> CortexResult<serde_json::Value> {
        let profile_name = profile.to_string();
        let st = status.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let profile_name = profile_name.clone();
            let st = st.clone();
            async move {
                c.mental_command_training_threshold_for_profile(
                    &token,
                    &profile_name,
                    st.as_deref(),
                    value,
                )
                .await
            }
        })
        .await
    }

    /// Get or set the mental command training threshold with explicit
    /// session/profile targeting.
    pub async fn mental_command_training_threshold_with_params(
        &self,
        session_id: Option<&str>,
        profile: Option<&str>,
        status: Option<&str>,
        value: Option<f64>,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.map(|s| s.to_string());
        let profile_name = profile.map(|s| s.to_string());
        let st = status.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let profile_name = profile_name.clone();
            let st = st.clone();
            async move {
                c.mental_command_training_threshold_with_params(
                    &token,
                    sid.as_deref(),
                    profile_name.as_deref(),
                    st.as_deref(),
                    value,
                )
                .await
            }
        })
        .await
    }

    /// Get a list of trained actions for a profile's detection type.
    pub async fn get_trained_signature_actions(
        &self,
        detection: DetectionType,
        profile: Option<&str>,
        session: Option<&str>,
    ) -> CortexResult<TrainedSignatureActions> {
        let p = profile.map(|s| s.to_string());
        let s = session.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let p = p.clone();
            let s = s.clone();
            async move {
                c.get_trained_signature_actions(&token, detection, p.as_deref(), s.as_deref())
                    .await
            }
        })
        .await
    }

    /// Get the duration of a training session.
    pub async fn get_training_time(
        &self,
        detection: DetectionType,
        session_id: &str,
    ) -> CortexResult<TrainingTime> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.get_training_time(&token, detection, &sid).await }
        })
        .await
    }

    /// Get or set the facial expression signature type.
    pub async fn facial_expression_signature_type(
        &self,
        status: &str,
        profile: Option<&str>,
        session: Option<&str>,
        signature: Option<&str>,
    ) -> CortexResult<serde_json::Value> {
        let st = status.to_string();
        let p = profile.map(|s| s.to_string());
        let s = session.map(|s| s.to_string());
        let sig = signature.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let st = st.clone();
            let p = p.clone();
            let s = s.clone();
            let sig = sig.clone();
            async move {
                c.facial_expression_signature_type(
                    &token,
                    &st,
                    p.as_deref(),
                    s.as_deref(),
                    sig.as_deref(),
                )
                .await
            }
        })
        .await
    }

    /// Get or set the threshold of a facial expression action.
    pub async fn facial_expression_threshold(
        &self,
        status: &str,
        action: &str,
        profile: Option<&str>,
        session: Option<&str>,
        value: Option<u32>,
    ) -> CortexResult<serde_json::Value> {
        let st = status.to_string();
        let act = action.to_string();
        let p = profile.map(|s| s.to_string());
        let s = session.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let st = st.clone();
            let act = act.clone();
            let p = p.clone();
            let s = s.clone();
            async move {
                c.facial_expression_threshold(&token, &st, &act, p.as_deref(), s.as_deref(), value)
                    .await
            }
        })
        .await
    }

    // ─── Connection Management ──────────────────────────────────────────

    /// Returns whether the underlying connection is alive.
    pub async fn is_connected(&self) -> bool {
        self.client().await.is_connected()
    }

    /// Get a reference to the underlying `CortexClient` (for advanced use).
    ///
    /// The returned `Arc` keeps the client alive even if a reconnection
    /// replaces the internal client. Use with care.
    pub async fn inner_client(&self) -> Arc<CortexClient> {
        self.client().await
    }

    /// Gracefully disconnect from the Cortex service.
    ///
    /// Stops the health monitor and drops the connection. The
    /// `ResilientClient` cannot be used after this call.
    pub async fn disconnect(self) -> CortexResult<()> {
        // Take the monitor out of the mutex, then drop the guard before awaiting
        let monitor = self
            .health_monitor
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());

        if let Some(mut monitor) = monitor {
            monitor.stop().await;
        }

        let _ = self.event_tx.send(ConnectionEvent::Disconnected {
            reason: "Graceful disconnect".into(),
        });

        // Drop the state — the CortexClient's reader loop will stop
        // when all Arc references are dropped.
        Ok(())
    }
}

/// Guard that resets the reconnecting flag when dropped.
struct ReconnectGuard<'a>(&'a AtomicBool);

impl<'a> Drop for ReconnectGuard<'a> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
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
