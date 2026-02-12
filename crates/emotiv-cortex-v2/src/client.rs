//! # Cortex WebSocket JSON-RPC Client
//!
//! Low-level transport for communicating with the Emotiv Cortex API.
//! Handles WebSocket connection, TLS (self-signed cert for localhost),
//! JSON-RPC request/response correlation, and the authentication flow.
//!
//! ## Architecture
//!
//! The WebSocket connection is split into reader/writer halves using
//! `tokio-tungstenite`'s `StreamExt::split()`. This allows concurrent
//! API calls and data streaming on the same WebSocket:
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │                 CortexClient                     │
//! │                                                  │
//! │  writer: Arc<Mutex<SplitSink>>  ◄── call()       │
//! │                                  ◄── subscribe() │
//! │                                                  │
//! │  reader_loop (spawned task):                     │
//! │    SplitStream ─┬─► RPC response → oneshot tx    │
//! │                 ├─► eeg event    → eeg_tx        │
//! │                 ├─► dev event    → dev_tx        │
//! │                 ├─► mot event    → mot_tx        │
//! │                 └─► pow event    → pow_tx        │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! ## TLS Note
//!
//! The Emotiv Cortex service runs at `wss://localhost:6868` with a
//! self-signed TLS certificate. We configure `native-tls` to accept
//! this certificate for localhost connections only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
use native_tls::TlsConnector as NativeTlsConnector;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{http, Message},
    Connector, MaybeTlsStream, WebSocketStream,
};

use crate::config::CortexConfig;
use crate::error::{CortexError, CortexResult};
use crate::protocol::{
    CortexRequest, CortexResponse, DemographicAttribute, DetectionInfo, DetectionType,
    ExportFormat, HeadsetInfo, MarkerInfo, Methods, ProfileAction, ProfileInfo, RecordInfo,
    SessionInfo, Streams, SubjectInfo, TrainedSignatureActions, TrainingStatus, TrainingTime,
    UserLoginInfo,
};

/// Connection timeout for the initial WebSocket handshake.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Channel buffer size for data stream events.
const STREAM_CHANNEL_BUFFER: usize = 1024;

/// Type alias for the write half of the WebSocket connection.
type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// Type alias for the read half of the WebSocket connection.
type WsReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// A pending RPC response awaiting its matching JSON-RPC response by `id`.
type PendingResponse = oneshot::Sender<CortexResult<serde_json::Value>>;

/// Senders for dispatching stream data events to consumers.
pub type StreamSenders = HashMap<&'static str, mpsc::Sender<serde_json::Value>>;

/// Receivers for consuming stream data events.
pub type StreamReceivers = HashMap<&'static str, mpsc::Receiver<serde_json::Value>>;

/// WebSocket JSON-RPC client for the Emotiv Cortex API.
///
/// This client manages a single WebSocket connection, split into reader
/// and writer halves. The writer is shared (behind `Arc<Mutex>`) so that
/// API calls can be made concurrently with data streaming. The reader
/// runs in a background task that dispatches:
///
/// - **RPC responses** → matched by `id` to pending `oneshot` channels
/// - **Data events** → routed by stream type to `mpsc` channels
pub struct CortexClient {
    /// Shared write half of the WebSocket.
    writer: Arc<Mutex<WsWriter>>,

    /// Map of pending RPC requests awaiting responses, keyed by request ID.
    pending_responses: Arc<Mutex<HashMap<u64, PendingResponse>>>,

    /// Auto-incrementing request ID counter.
    next_id: AtomicU64,

    /// Handle to the background reader loop task.
    reader_handle: Option<JoinHandle<()>>,

    /// Whether the reader loop is currently running.
    reader_running: Arc<AtomicBool>,

    /// Shared stream senders, dynamically updatable without restarting
    /// the reader loop. The reader holds a clone of this Arc and checks
    /// it on each data message.
    stream_senders: Arc<std::sync::Mutex<Option<StreamSenders>>>,

    /// RPC call timeout (from config).
    rpc_timeout: Duration,
}

impl CortexClient {
    /// Connect to the Cortex API WebSocket service.
    ///
    /// The Cortex service must be running on the local machine.
    /// TLS is configured based on the [`CortexConfig`] settings.
    pub async fn connect(config: &CortexConfig) -> CortexResult<Self> {
        let url = &config.cortex_url;
        let accept_invalid_certs = config.should_accept_invalid_certs();
        let rpc_timeout = Duration::from_secs(config.timeouts.rpc_timeout_secs);

        // Configure TLS
        let tls_connector = NativeTlsConnector::builder()
            .danger_accept_invalid_certs(accept_invalid_certs)
            .build()
            .map_err(|e| CortexError::ConnectionFailed {
                url: url.clone(),
                reason: format!("TLS configuration failed: {}", e),
            })?;

        let connector = Connector::NativeTls(tls_connector);

        // Parse the WebSocket URL as a URI for the connection.
        let uri: http::Uri =
            url.parse()
                .map_err(|e: http::uri::InvalidUri| CortexError::ConnectionFailed {
                    url: url.clone(),
                    reason: format!("Invalid URL: {}", e),
                })?;

        let connect_fut = connect_async_tls_with_config(
            uri,
            None, // WebSocket config
            true, // disable_nagle
            Some(connector),
        );

        let (ws, response) = tokio::time::timeout(CONNECT_TIMEOUT, connect_fut)
            .await
            .map_err(|_| CortexError::Timeout { seconds: 5 })?
            .map_err(|e| CortexError::ConnectionFailed {
                url: url.clone(),
                reason: format!("WebSocket connection failed: {}", e),
            })?;

        tracing::info!(url, status = %response.status(), "Connected to Cortex API");

        // Split the WebSocket into reader and writer halves.
        let (writer, reader) = ws.split();

        let pending_responses: Arc<Mutex<HashMap<u64, PendingResponse>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reader_running = Arc::new(AtomicBool::new(true));
        let stream_senders: Arc<std::sync::Mutex<Option<StreamSenders>>> =
            Arc::new(std::sync::Mutex::new(None));

        // Start the reader loop immediately — it needs to be running before
        // any API calls so that responses can be dispatched.
        let reader_handle = Self::spawn_reader_loop(
            reader,
            Arc::clone(&pending_responses),
            Arc::clone(&reader_running),
            Arc::clone(&stream_senders),
        );

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            pending_responses,
            next_id: AtomicU64::new(1),
            reader_handle: Some(reader_handle),
            reader_running,
            stream_senders,
            rpc_timeout,
        })
    }

    /// Connect to the Cortex API using just a URL (convenience for simple use cases).
    ///
    /// Uses default timeouts and localhost TLS settings.
    pub async fn connect_url(url: &str) -> CortexResult<Self> {
        let config = CortexConfig {
            client_id: String::new(),
            client_secret: String::new(),
            cortex_url: url.to_string(),
            ..CortexConfig::new("", "")
        };
        Self::connect(&config).await
    }

    /// Spawn the background reader loop that dispatches WebSocket messages.
    fn spawn_reader_loop(
        mut reader: WsReader,
        pending_responses: Arc<Mutex<HashMap<u64, PendingResponse>>>,
        running: Arc<AtomicBool>,
        stream_senders: Arc<std::sync::Mutex<Option<StreamSenders>>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                let msg = tokio::select! {
                    msg = reader.next() => msg,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
                };

                match msg {
                    Some(Ok(Message::Text(text))) => {
                        tracing::debug!(raw = %text, "Reader loop received message");

                        let value: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!("Failed to parse WebSocket message as JSON: {}", e);
                                continue;
                            }
                        };

                        // Check if this is an RPC response (has an `id` field)
                        if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                            let response: std::result::Result<CortexResponse, _> =
                                serde_json::from_value(value);

                            let mut pending = pending_responses.lock().await;
                            if let Some(tx) = pending.remove(&id) {
                                match response {
                                    Ok(resp) => {
                                        let result = if let Some(error) = resp.error {
                                            tracing::error!(
                                                id,
                                                code = error.code,
                                                message = %error.message,
                                                "Cortex API error in RPC response",
                                            );
                                            Err(CortexError::from_api_error(
                                                error.code,
                                                error.message,
                                            ))
                                        } else {
                                            resp.result.ok_or_else(|| CortexError::ProtocolError {
                                                reason: "Response has no result or error".into(),
                                            })
                                        };
                                        let _ = tx.send(result);
                                    }
                                    Err(e) => {
                                        let _ = tx.send(Err(CortexError::ProtocolError {
                                            reason: format!("Failed to parse RPC response: {}", e),
                                        }));
                                    }
                                }
                            } else {
                                tracing::debug!(id, "Received response for unknown request ID");
                            }
                            continue;
                        }

                        // Not an RPC response — route as a stream data event.
                        if let Ok(guard) = stream_senders.lock() {
                            if let Some(ref senders) = *guard {
                                for (key, tx) in senders.iter() {
                                    if value.get(*key).is_some() {
                                        let _ = tx.try_send(value);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        tracing::info!("Cortex WebSocket closed by server");
                        let mut pending = pending_responses.lock().await;
                        for (_, tx) in pending.drain() {
                            let _ = tx.send(Err(CortexError::ConnectionLost {
                                reason: "Cortex WebSocket closed".into(),
                            }));
                        }
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("WebSocket read error: {}", e);
                        let mut pending = pending_responses.lock().await;
                        for (_, tx) in pending.drain() {
                            let _ = tx.send(Err(CortexError::WebSocket(format!(
                                "WebSocket error: {}",
                                e
                            ))));
                        }
                        break;
                    }
                    None => {
                        tracing::info!("Cortex WebSocket stream ended");
                        break;
                    }
                    _ => {
                        // Binary messages, pings, pongs — skip
                    }
                }
            }

            tracing::debug!("Reader loop exiting");
            running.store(false, Ordering::SeqCst);
        })
    }

    // ─── Core RPC ───────────────────────────────────────────────────────

    /// Send a JSON-RPC request and wait for the matching response.
    async fn call(
        &self,
        method: &'static str,
        params: serde_json::Value,
    ) -> CortexResult<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = CortexRequest::new(id, method, params);

        let json = serde_json::to_string(&request).map_err(|e| CortexError::ProtocolError {
            reason: format!("serialize error: {}", e),
        })?;

        tracing::debug!(method, id, json = %json, "Sending Cortex request");

        // Register the pending response before sending
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_responses.lock().await;
            pending.insert(id, tx);
        }

        // Send the request via the shared writer
        {
            let mut writer = self.writer.lock().await;
            writer
                .send(Message::Text(json.into()))
                .await
                .map_err(|e| CortexError::WebSocket(format!("Send error: {}", e)))?;
        }

        // Wait for the reader loop to deliver the response
        let timeout_secs = self.rpc_timeout.as_secs();
        let result = tokio::time::timeout(self.rpc_timeout, rx)
            .await
            .map_err(|_| {
                // Clean up the pending entry on timeout
                let pending = self.pending_responses.clone();
                tokio::spawn(async move {
                    pending.lock().await.remove(&id);
                });
                CortexError::Timeout {
                    seconds: timeout_secs,
                }
            })?
            .map_err(|_| CortexError::ConnectionLost {
                reason: "Response channel dropped (reader loop died)".into(),
            })??;

        tracing::debug!(method, id, "Cortex RPC succeeded");
        Ok(result)
    }

    // ─── Streaming ──────────────────────────────────────────────────────

    /// Stream name validation and mapping to static keys.
    fn stream_key(name: &str) -> &'static str {
        match name {
            Streams::EEG => "eeg",
            Streams::DEV => "dev",
            Streams::MOT => "mot",
            Streams::EQ => "eq",
            Streams::POW => "pow",
            Streams::MET => "met",
            Streams::COM => "com",
            Streams::FAC => "fac",
            Streams::SYS => "sys",
            other => {
                tracing::warn!(stream = other, "Unknown stream type");
                "unknown"
            }
        }
    }

    /// Create data stream channels for the specified streams.
    ///
    /// This replaces ALL existing stream channels. Call before
    /// [`subscribe_streams`](Self::subscribe_streams).
    pub fn create_stream_channels(&self, streams: &[&str]) -> StreamReceivers {
        let mut senders = StreamSenders::new();
        let mut receivers = StreamReceivers::new();

        for &stream in streams {
            let (tx, rx) = mpsc::channel(STREAM_CHANNEL_BUFFER);
            senders.insert(Self::stream_key(stream), tx);
            receivers.insert(Self::stream_key(stream), rx);
        }

        if let Ok(mut guard) = self.stream_senders.lock() {
            *guard = Some(senders);
        }

        receivers
    }

    /// Add a single stream channel without disturbing existing ones.
    ///
    /// Returns a receiver for the new channel.
    pub fn add_stream_channel(&self, stream: &str) -> Option<mpsc::Receiver<serde_json::Value>> {
        let (tx, rx) = mpsc::channel(STREAM_CHANNEL_BUFFER);
        if let Ok(mut guard) = self.stream_senders.lock() {
            let senders = guard.get_or_insert_with(StreamSenders::new);
            senders.insert(Self::stream_key(stream), tx);
            Some(rx)
        } else {
            None
        }
    }

    /// Remove a single stream channel sender.
    pub fn remove_stream_channel(&self, stream: &str) {
        if let Ok(mut guard) = self.stream_senders.lock() {
            if let Some(ref mut senders) = *guard {
                senders.remove(stream);
            }
        }
    }

    /// Clear all stream senders.
    pub fn clear_stream_channels(&self) {
        if let Ok(mut guard) = self.stream_senders.lock() {
            *guard = None;
        }
    }

    // ─── Authentication ─────────────────────────────────────────────────

    /// Query Cortex service version and build info.
    ///
    /// No authentication required. Useful as a health check.
    pub async fn get_cortex_info(&self) -> CortexResult<serde_json::Value> {
        self.call(Methods::GET_CORTEX_INFO, serde_json::json!({}))
            .await
    }

    /// Check if the application has been granted access rights.
    pub async fn has_access_right(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> CortexResult<bool> {
        let result = self
            .call(
                Methods::HAS_ACCESS_RIGHT,
                serde_json::json!({
                    "clientId": client_id,
                    "clientSecret": client_secret,
                }),
            )
            .await?;

        Ok(result
            .get("accessGranted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get the currently logged-in Emotiv user.
    pub async fn get_user_login(&self) -> CortexResult<Vec<UserLoginInfo>> {
        let result = self
            .call(Methods::GET_USER_LOGIN, serde_json::json!({}))
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse user login info: {}", e),
        })
    }

    /// Authenticate with the Cortex API.
    ///
    /// Performs: `getCortexInfo` → `requestAccess` → `authorize`.
    ///
    /// Returns the cortex token needed for all subsequent operations.
    pub async fn authenticate(&self, client_id: &str, client_secret: &str) -> CortexResult<String> {
        // Step 0: getCortexInfo — verify API is alive
        let cortex_info_ok = match self.get_cortex_info().await {
            Ok(info) => {
                tracing::info!("Cortex API info: {}", info);
                true
            }
            Err(e) => {
                tracing::warn!("getCortexInfo failed (continuing anyway): {}", e);
                false
            }
        };

        // Step 1: requestAccess — gracefully skip if method doesn't exist
        match self
            .call(
                Methods::REQUEST_ACCESS,
                serde_json::json!({
                    "clientId": client_id,
                    "clientSecret": client_secret,
                }),
            )
            .await
        {
            Ok(_) => tracing::debug!("Cortex access requested"),
            Err(CortexError::MethodNotFound { .. }) => {
                tracing::info!(
                    "requestAccess not available on this Cortex version \
                     (Launcher handles app approval directly)"
                );
            }
            Err(e) => return Err(e),
        }

        // Step 2: authorize and get a cortex token
        let auth_result = match self
            .call(
                Methods::AUTHORIZE,
                serde_json::json!({
                    "clientId": client_id,
                    "clientSecret": client_secret,
                }),
            )
            .await
        {
            Ok(result) => result,
            Err(CortexError::MethodNotFound { .. }) => {
                if !cortex_info_ok {
                    tracing::error!(
                        "Both getCortexInfo and authorize returned 'Method not found'. \
                         The service may not be the Emotiv Cortex API, or may be incompatible."
                    );
                }
                return Err(CortexError::AuthenticationFailed {
                    reason: "Cortex API 'authorize' method not found (-32601). \
                             Check that the EMOTIV Launcher is running and you are logged in."
                        .into(),
                });
            }
            Err(e) => return Err(e),
        };

        let cortex_token = auth_result
            .get("cortexToken")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CortexError::ProtocolError {
                reason: "authorize response missing cortexToken".into(),
            })?
            .to_string();

        tracing::info!("Cortex authentication successful");

        Ok(cortex_token)
    }

    /// Generate a new cortex token (or refresh an existing one).
    ///
    /// Can be used to obtain a fresh token without the full `requestAccess` → `authorize` flow.
    pub async fn generate_new_token(
        &self,
        cortex_token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> CortexResult<String> {
        let result = self
            .call(
                Methods::GENERATE_NEW_TOKEN,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "clientId": client_id,
                    "clientSecret": client_secret,
                }),
            )
            .await?;

        result
            .get("cortexToken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| CortexError::ProtocolError {
                reason: "generateNewToken response missing cortexToken".into(),
            })
    }

    /// Get information about the current user.
    pub async fn get_user_info(
        &self,
        cortex_token: &str,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::GET_USER_INFO,
            serde_json::json!({
                "cortexToken": cortex_token,
            }),
        )
        .await
    }

    /// Get information about the license used by the application.
    pub async fn get_license_info(
        &self,
        cortex_token: &str,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::GET_LICENSE_INFO,
            serde_json::json!({
                "cortexToken": cortex_token,
            }),
        )
        .await
    }

    // ─── Headset Management ─────────────────────────────────────────────

    /// Query available headsets.
    pub async fn query_headsets(&self) -> CortexResult<Vec<HeadsetInfo>> {
        let result = self
            .call(Methods::QUERY_HEADSETS, serde_json::json!({}))
            .await?;

        let headsets: Vec<HeadsetInfo> =
            serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse headset list: {}", e),
            })?;

        tracing::info!(count = headsets.len(), "Queried headsets");

        Ok(headsets)
    }

    /// Connect to a specific headset via the Cortex service.
    pub async fn connect_headset(&self, headset_id: &str) -> CortexResult<()> {
        self.call(
            Methods::CONTROL_DEVICE,
            serde_json::json!({
                "command": "connect",
                "headset": headset_id,
            }),
        )
        .await?;

        tracing::info!(headset = headset_id, "Headset connection initiated");
        Ok(())
    }

    /// Disconnect a headset from the Cortex service.
    pub async fn disconnect_headset(&self, headset_id: &str) -> CortexResult<()> {
        self.call(
            Methods::CONTROL_DEVICE,
            serde_json::json!({
                "command": "disconnect",
                "headset": headset_id,
            }),
        )
        .await?;

        tracing::info!(headset = headset_id, "Headset disconnection initiated");
        Ok(())
    }

    /// Trigger headset scanning / refresh.
    pub async fn refresh_headsets(&self) -> CortexResult<()> {
        self.call(
            Methods::CONTROL_DEVICE,
            serde_json::json!({
                "command": "refresh",
            }),
        )
        .await?;

        tracing::debug!("Headset refresh/scan triggered");
        Ok(())
    }

    /// Synchronize the system clock with the headset clock.
    pub async fn sync_with_headset_clock(
        &self,
        cortex_token: &str,
        headset_id: &str,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::SYNC_WITH_HEADSET_CLOCK,
            serde_json::json!({
                "cortexToken": cortex_token,
                "headset": headset_id,
            }),
        )
        .await
    }

    /// Manage EEG channel mapping configurations for an EPOC Flex headset.
    ///
    /// The `status` parameter controls the operation: `"create"`, `"get"`, `"read"`,
    /// `"update"`, or `"delete"`. Additional parameters depend on the operation.
    pub async fn config_mapping(
        &self,
        cortex_token: &str,
        headset_id: &str,
        status: &str,
        mapping_name: Option<&str>,
        mapping_uuid: Option<&str>,
        mappings: Option<&serde_json::Value>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "headset": headset_id,
            "status": status,
        });

        if let Some(name) = mapping_name {
            params["name"] = serde_json::json!(name);
        }
        if let Some(uuid) = mapping_uuid {
            params["uuid"] = serde_json::json!(uuid);
        }
        if let Some(m) = mappings {
            params["mappings"] = m.clone();
        }

        self.call(Methods::CONFIG_MAPPING, params).await
    }

    /// Update settings of an EPOC+ or EPOC X headset.
    ///
    /// The `setting` value is a JSON object with device-specific keys
    /// (e.g., `{"mode": "EPOC", "eegRate": 256, "memsRate": 64}`).
    pub async fn update_headset(
        &self,
        cortex_token: &str,
        headset_id: &str,
        setting: serde_json::Value,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::UPDATE_HEADSET,
            serde_json::json!({
                "cortexToken": cortex_token,
                "headset": headset_id,
                "setting": setting,
            }),
        )
        .await
    }

    /// Update the headband position or custom name of an EPOC X headset.
    pub async fn update_headset_custom_info(
        &self,
        cortex_token: &str,
        headset_id: &str,
        headband_position: Option<&str>,
        custom_name: Option<&str>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "headset": headset_id,
        });

        if let Some(pos) = headband_position {
            params["headbandPosition"] = serde_json::json!(pos);
        }
        if let Some(name) = custom_name {
            params["customName"] = serde_json::json!(name);
        }

        self.call(Methods::UPDATE_HEADSET_CUSTOM_INFO, params).await
    }

    // ─── Session Management ─────────────────────────────────────────────

    /// Create a session for a headset.
    pub async fn create_session(
        &self,
        cortex_token: &str,
        headset_id: &str,
    ) -> CortexResult<SessionInfo> {
        let result = self
            .call(
                Methods::CREATE_SESSION,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "headset": headset_id,
                    "status": "active",
                }),
            )
            .await?;

        let session: SessionInfo =
            serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse session info: {}", e),
            })?;

        tracing::info!(session_id = %session.id, "Session created");
        Ok(session)
    }

    /// Query existing sessions.
    pub async fn query_sessions(&self, cortex_token: &str) -> CortexResult<Vec<SessionInfo>> {
        let result = self
            .call(
                Methods::QUERY_SESSIONS,
                serde_json::json!({
                    "cortexToken": cortex_token,
                }),
            )
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse sessions: {}", e),
        })
    }

    /// Close a session.
    pub async fn close_session(&self, cortex_token: &str, session_id: &str) -> CortexResult<()> {
        let _ = self
            .call(
                Methods::UPDATE_SESSION,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "session": session_id,
                    "status": "close",
                }),
            )
            .await;

        tracing::info!(session_id, "Session closed");
        Ok(())
    }

    // ─── Data Streams ───────────────────────────────────────────────────

    /// Subscribe to one or more data streams.
    pub async fn subscribe_streams(
        &self,
        cortex_token: &str,
        session_id: &str,
        streams: &[&str],
    ) -> CortexResult<()> {
        self.call(
            Methods::SUBSCRIBE,
            serde_json::json!({
                "cortexToken": cortex_token,
                "session": session_id,
                "streams": streams,
            }),
        )
        .await?;

        tracing::info!(session_id, ?streams, "Subscribed to data streams");
        Ok(())
    }

    /// Unsubscribe from one or more data streams.
    pub async fn unsubscribe_streams(
        &self,
        cortex_token: &str,
        session_id: &str,
        streams: &[&str],
    ) -> CortexResult<()> {
        self.call(
            Methods::UNSUBSCRIBE,
            serde_json::json!({
                "cortexToken": cortex_token,
                "session": session_id,
                "streams": streams,
            }),
        )
        .await?;

        tracing::info!(session_id, ?streams, "Unsubscribed from data streams");
        Ok(())
    }

    // ─── Records ────────────────────────────────────────────────────────

    /// Start a new recording.
    pub async fn create_record(
        &self,
        cortex_token: &str,
        session_id: &str,
        title: &str,
    ) -> CortexResult<RecordInfo> {
        let result = self
            .call(
                Methods::CREATE_RECORD,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "session": session_id,
                    "title": title,
                }),
            )
            .await?;

        let record_value =
            result
                .get("record")
                .cloned()
                .ok_or_else(|| CortexError::ProtocolError {
                    reason: "createRecord response missing 'record' field".into(),
                })?;

        let record: RecordInfo =
            serde_json::from_value(record_value).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse record info: {}", e),
            })?;

        tracing::info!(record_id = %record.uuid, "Recording started");
        Ok(record)
    }

    /// Stop an active recording.
    pub async fn stop_record(
        &self,
        cortex_token: &str,
        session_id: &str,
    ) -> CortexResult<RecordInfo> {
        let result = self
            .call(
                Methods::STOP_RECORD,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "session": session_id,
                }),
            )
            .await?;

        let record_value =
            result
                .get("record")
                .cloned()
                .ok_or_else(|| CortexError::ProtocolError {
                    reason: "stopRecord response missing 'record' field".into(),
                })?;

        let record: RecordInfo =
            serde_json::from_value(record_value).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse record info: {}", e),
            })?;

        tracing::info!(record_id = %record.uuid, "Recording stopped");
        Ok(record)
    }

    /// Query recorded sessions.
    pub async fn query_records(
        &self,
        cortex_token: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> CortexResult<Vec<RecordInfo>> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "query": {},
            "orderBy": [{ "startDatetime": "DESC" }],
        });

        if let Some(limit) = limit {
            params["limit"] = serde_json::json!(limit);
        }
        if let Some(offset) = offset {
            params["offset"] = serde_json::json!(offset);
        }

        let result = self.call(Methods::QUERY_RECORDS, params).await?;

        let records = result
            .get("records")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![]));

        serde_json::from_value(records).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse records: {}", e),
        })
    }

    /// Export a recording to CSV or EDF format.
    pub async fn export_record(
        &self,
        cortex_token: &str,
        record_ids: &[String],
        folder: &str,
        format: ExportFormat,
    ) -> CortexResult<()> {
        self.call(
            Methods::EXPORT_RECORD,
            serde_json::json!({
                "cortexToken": cortex_token,
                "recordIds": record_ids,
                "folder": folder,
                "format": format.as_str(),
            }),
        )
        .await?;

        tracing::info!(
            ?record_ids,
            folder,
            format = format.as_str(),
            "Export initiated"
        );
        Ok(())
    }

    /// Update a recording's metadata (title, description, tags).
    pub async fn update_record(
        &self,
        cortex_token: &str,
        record_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        tags: Option<&[String]>,
    ) -> CortexResult<RecordInfo> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "record": record_id,
        });

        if let Some(t) = title {
            params["title"] = serde_json::json!(t);
        }
        if let Some(d) = description {
            params["description"] = serde_json::json!(d);
        }
        if let Some(t) = tags {
            params["tags"] = serde_json::json!(t);
        }

        let result = self.call(Methods::UPDATE_RECORD, params).await?;

        let record_value =
            result
                .get("record")
                .cloned()
                .ok_or_else(|| CortexError::ProtocolError {
                    reason: "updateRecord response missing 'record' field".into(),
                })?;

        serde_json::from_value(record_value).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse record info: {}", e),
        })
    }

    /// Delete one or more recordings.
    pub async fn delete_record(
        &self,
        cortex_token: &str,
        record_ids: &[String],
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::DELETE_RECORD,
            serde_json::json!({
                "cortexToken": cortex_token,
                "records": record_ids,
            }),
        )
        .await
    }

    /// Get detailed information for specific records by their IDs.
    pub async fn get_record_infos(
        &self,
        cortex_token: &str,
        record_ids: &[String],
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::GET_RECORD_INFOS,
            serde_json::json!({
                "cortexToken": cortex_token,
                "recordIds": record_ids,
            }),
        )
        .await
    }

    /// Configure the opt-out setting for data sharing.
    ///
    /// Use `status: "get"` to query, `status: "set"` with `new_opt_out` to change.
    pub async fn config_opt_out(
        &self,
        cortex_token: &str,
        status: &str,
        new_opt_out: Option<bool>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "status": status,
        });

        if let Some(opt) = new_opt_out {
            params["newOptOut"] = serde_json::json!(opt);
        }

        self.call(Methods::CONFIG_OPT_OUT, params).await
    }

    /// Request to download recorded data from the Emotiv cloud.
    pub async fn download_record(
        &self,
        cortex_token: &str,
        record_ids: &[String],
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::DOWNLOAD_RECORD,
            serde_json::json!({
                "cortexToken": cortex_token,
                "recordIds": record_ids,
            }),
        )
        .await
    }

    // ─── Markers ────────────────────────────────────────────────────────

    /// Inject a time-stamped marker during an active recording.
    pub async fn inject_marker(
        &self,
        cortex_token: &str,
        session_id: &str,
        label: &str,
        value: i32,
        port: &str,
        time: Option<f64>,
    ) -> CortexResult<MarkerInfo> {
        let epoch_ms = time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX epoch")
                .as_millis() as f64
        });

        let params = serde_json::json!({
            "cortexToken": cortex_token,
            "session": session_id,
            "label": label,
            "value": value,
            "port": port,
            "time": epoch_ms,
        });

        let result = self.call(Methods::INJECT_MARKER, params).await?;

        let marker_value =
            result
                .get("marker")
                .cloned()
                .ok_or_else(|| CortexError::ProtocolError {
                    reason: "injectMarker response missing 'marker' field".into(),
                })?;

        let marker: MarkerInfo =
            serde_json::from_value(marker_value).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse marker info: {}", e),
            })?;

        tracing::debug!(marker_id = %marker.uuid, label, "Marker injected");
        Ok(marker)
    }

    /// Update a marker to convert it from an instance marker to an interval marker.
    pub async fn update_marker(
        &self,
        cortex_token: &str,
        session_id: &str,
        marker_id: &str,
        time: Option<f64>,
    ) -> CortexResult<()> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "session": session_id,
            "markerId": marker_id,
        });

        if let Some(t) = time {
            params["time"] = serde_json::json!(t);
        }

        self.call(Methods::UPDATE_MARKER, params).await?;
        tracing::debug!(marker_id, "Marker updated");
        Ok(())
    }

    // ─── Subjects ────────────────────────────────────────────────────────

    /// Create a new subject.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_subject(
        &self,
        cortex_token: &str,
        subject_name: &str,
        date_of_birth: Option<&str>,
        sex: Option<&str>,
        country_code: Option<&str>,
        state: Option<&str>,
        city: Option<&str>,
        attributes: Option<&[serde_json::Value]>,
    ) -> CortexResult<SubjectInfo> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "subjectName": subject_name,
        });

        if let Some(dob) = date_of_birth {
            params["dateOfBirth"] = serde_json::json!(dob);
        }
        if let Some(s) = sex {
            params["sex"] = serde_json::json!(s);
        }
        if let Some(cc) = country_code {
            params["countryCode"] = serde_json::json!(cc);
        }
        if let Some(st) = state {
            params["state"] = serde_json::json!(st);
        }
        if let Some(c) = city {
            params["city"] = serde_json::json!(c);
        }
        if let Some(attrs) = attributes {
            params["attributes"] = serde_json::json!(attrs);
        }

        let result = self.call(Methods::CREATE_SUBJECT, params).await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse subject info: {}", e),
        })
    }

    /// Update an existing subject's information.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_subject(
        &self,
        cortex_token: &str,
        subject_name: &str,
        date_of_birth: Option<&str>,
        sex: Option<&str>,
        country_code: Option<&str>,
        state: Option<&str>,
        city: Option<&str>,
        attributes: Option<&[serde_json::Value]>,
    ) -> CortexResult<SubjectInfo> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "subjectName": subject_name,
        });

        if let Some(dob) = date_of_birth {
            params["dateOfBirth"] = serde_json::json!(dob);
        }
        if let Some(s) = sex {
            params["sex"] = serde_json::json!(s);
        }
        if let Some(cc) = country_code {
            params["countryCode"] = serde_json::json!(cc);
        }
        if let Some(st) = state {
            params["state"] = serde_json::json!(st);
        }
        if let Some(c) = city {
            params["city"] = serde_json::json!(c);
        }
        if let Some(attrs) = attributes {
            params["attributes"] = serde_json::json!(attrs);
        }

        let result = self.call(Methods::UPDATE_SUBJECT, params).await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse subject info: {}", e),
        })
    }

    /// Delete one or more subjects.
    pub async fn delete_subjects(
        &self,
        cortex_token: &str,
        subject_names: &[String],
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::DELETE_SUBJECTS,
            serde_json::json!({
                "cortexToken": cortex_token,
                "subjects": subject_names,
            }),
        )
        .await
    }

    /// Query subjects with filtering, sorting, and pagination.
    ///
    /// Returns a tuple of (subjects, total_count).
    pub async fn query_subjects(
        &self,
        cortex_token: &str,
        query: serde_json::Value,
        order_by: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> CortexResult<(Vec<SubjectInfo>, u32)> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "query": query,
            "orderBy": order_by,
        });

        if let Some(l) = limit {
            params["limit"] = serde_json::json!(l);
        }
        if let Some(o) = offset {
            params["offset"] = serde_json::json!(o);
        }

        let result = self.call(Methods::QUERY_SUBJECTS, params).await?;

        let count = result
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let subjects_value = result
            .get("subjects")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![]));

        let subjects: Vec<SubjectInfo> =
            serde_json::from_value(subjects_value).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse subjects: {}", e),
            })?;

        Ok((subjects, count))
    }

    /// Get the list of valid demographic attributes.
    pub async fn get_demographic_attributes(
        &self,
        cortex_token: &str,
    ) -> CortexResult<Vec<DemographicAttribute>> {
        let result = self
            .call(
                Methods::GET_DEMOGRAPHIC_ATTRIBUTES,
                serde_json::json!({
                    "cortexToken": cortex_token,
                }),
            )
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse demographic attributes: {}", e),
        })
    }

    // ─── Profiles ───────────────────────────────────────────────────────

    /// List all profiles for the current user.
    pub async fn query_profiles(&self, cortex_token: &str) -> CortexResult<Vec<ProfileInfo>> {
        let result = self
            .call(
                Methods::QUERY_PROFILE,
                serde_json::json!({
                    "cortexToken": cortex_token,
                }),
            )
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse profiles: {}", e),
        })
    }

    /// Get the profile currently loaded for a headset.
    pub async fn get_current_profile(
        &self,
        cortex_token: &str,
        headset_id: &str,
    ) -> CortexResult<Option<ProfileInfo>> {
        let result = self
            .call(
                Methods::GET_CURRENT_PROFILE,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "headset": headset_id,
                }),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let profile: ProfileInfo =
            serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
                reason: format!("Failed to parse profile info: {}", e),
            })?;

        Ok(Some(profile))
    }

    /// Manage a profile (create, load, unload, save, rename, delete).
    pub async fn setup_profile(
        &self,
        cortex_token: &str,
        headset_id: &str,
        profile_name: &str,
        action: ProfileAction,
    ) -> CortexResult<()> {
        self.call(
            Methods::SETUP_PROFILE,
            serde_json::json!({
                "cortexToken": cortex_token,
                "headset": headset_id,
                "profile": profile_name,
                "status": action.as_str(),
            }),
        )
        .await?;

        tracing::info!(
            profile = profile_name,
            action = action.as_str(),
            "Profile action completed"
        );
        Ok(())
    }

    /// Load an empty guest profile for a headset.
    ///
    /// This unloads any currently loaded profile and loads a blank guest profile,
    /// useful for starting fresh without trained data.
    pub async fn load_guest_profile(
        &self,
        cortex_token: &str,
        headset_id: &str,
    ) -> CortexResult<()> {
        self.call(
            Methods::LOAD_GUEST_PROFILE,
            serde_json::json!({
                "cortexToken": cortex_token,
                "headset": headset_id,
            }),
        )
        .await?;

        tracing::info!(headset = headset_id, "Guest profile loaded");
        Ok(())
    }

    // ─── BCI / Training ─────────────────────────────────────────────────

    /// Get detection info for a specific detection type.
    pub async fn get_detection_info(
        &self,
        detection: DetectionType,
    ) -> CortexResult<DetectionInfo> {
        let result = self
            .call(
                Methods::GET_DETECTION_INFO,
                serde_json::json!({
                    "detection": detection.as_str(),
                }),
            )
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse detection info: {}", e),
        })
    }

    /// Control the training lifecycle for mental commands or facial expressions.
    pub async fn training(
        &self,
        cortex_token: &str,
        session_id: &str,
        detection: DetectionType,
        status: TrainingStatus,
        action: &str,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::TRAINING,
            serde_json::json!({
                "cortexToken": cortex_token,
                "session": session_id,
                "detection": detection.as_str(),
                "status": status.as_str(),
                "action": action,
            }),
        )
        .await
    }

    /// Get or set the active mental command actions.
    pub async fn mental_command_active_action(
        &self,
        cortex_token: &str,
        session_id: &str,
        actions: Option<&[&str]>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "session": session_id,
            "status": if actions.is_some() { "set" } else { "get" },
        });

        if let Some(actions) = actions {
            params["actions"] = serde_json::json!(actions);
        }

        self.call(Methods::MENTAL_COMMAND_ACTIVE_ACTION, params)
            .await
    }

    /// Get or set the mental command action sensitivity.
    pub async fn mental_command_action_sensitivity(
        &self,
        cortex_token: &str,
        session_id: &str,
        values: Option<&[i32]>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "session": session_id,
            "status": if values.is_some() { "set" } else { "get" },
        });

        if let Some(values) = values {
            params["values"] = serde_json::json!(values);
        }

        self.call(Methods::MENTAL_COMMAND_ACTION_SENSITIVITY, params)
            .await
    }

    /// Get the mental command brain map.
    pub async fn mental_command_brain_map(
        &self,
        cortex_token: &str,
        session_id: &str,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::MENTAL_COMMAND_BRAIN_MAP,
            serde_json::json!({
                "cortexToken": cortex_token,
                "session": session_id,
            }),
        )
        .await
    }

    /// Get or set the mental command training threshold.
    pub async fn mental_command_training_threshold(
        &self,
        cortex_token: &str,
        session_id: &str,
    ) -> CortexResult<serde_json::Value> {
        self.call(
            Methods::MENTAL_COMMAND_TRAINING_THRESHOLD,
            serde_json::json!({
                "cortexToken": cortex_token,
                "session": session_id,
            }),
        )
        .await
    }

    /// Get a list of trained actions for a profile's detection type.
    ///
    /// Specify either `profile` (by name) or `session` (by ID), not both.
    pub async fn get_trained_signature_actions(
        &self,
        cortex_token: &str,
        detection: DetectionType,
        profile: Option<&str>,
        session: Option<&str>,
    ) -> CortexResult<TrainedSignatureActions> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "detection": detection.as_str(),
        });

        if let Some(p) = profile {
            params["profile"] = serde_json::json!(p);
        }
        if let Some(s) = session {
            params["session"] = serde_json::json!(s);
        }

        let result = self
            .call(Methods::GET_TRAINED_SIGNATURE_ACTIONS, params)
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse trained signature actions: {}", e),
        })
    }

    /// Get the duration of a training session.
    pub async fn get_training_time(
        &self,
        cortex_token: &str,
        detection: DetectionType,
        session_id: &str,
    ) -> CortexResult<TrainingTime> {
        let result = self
            .call(
                Methods::GET_TRAINING_TIME,
                serde_json::json!({
                    "cortexToken": cortex_token,
                    "detection": detection.as_str(),
                    "session": session_id,
                }),
            )
            .await?;

        serde_json::from_value(result).map_err(|e| CortexError::ProtocolError {
            reason: format!("Failed to parse training time: {}", e),
        })
    }

    /// Get or set the facial expression signature type.
    ///
    /// Use `status: "get"` to query, `status: "set"` with `signature` to change.
    /// Specify either `profile` or `session`, not both.
    pub async fn facial_expression_signature_type(
        &self,
        cortex_token: &str,
        status: &str,
        profile: Option<&str>,
        session: Option<&str>,
        signature: Option<&str>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "status": status,
        });

        if let Some(p) = profile {
            params["profile"] = serde_json::json!(p);
        }
        if let Some(s) = session {
            params["session"] = serde_json::json!(s);
        }
        if let Some(sig) = signature {
            params["signature"] = serde_json::json!(sig);
        }

        self.call(Methods::FACIAL_EXPRESSION_SIGNATURE_TYPE, params)
            .await
    }

    /// Get or set the threshold of a facial expression action.
    ///
    /// Use `status: "get"` to query, `status: "set"` with `value` to change.
    /// Specify either `profile` or `session`, not both.
    /// The `value` range is 0–1000.
    pub async fn facial_expression_threshold(
        &self,
        cortex_token: &str,
        status: &str,
        action: &str,
        profile: Option<&str>,
        session: Option<&str>,
        value: Option<u32>,
    ) -> CortexResult<serde_json::Value> {
        let mut params = serde_json::json!({
            "cortexToken": cortex_token,
            "status": status,
            "action": action,
        });

        if let Some(p) = profile {
            params["profile"] = serde_json::json!(p);
        }
        if let Some(s) = session {
            params["session"] = serde_json::json!(s);
        }
        if let Some(v) = value {
            params["value"] = serde_json::json!(v);
        }

        self.call(Methods::FACIAL_EXPRESSION_THRESHOLD, params)
            .await
    }

    // ─── Connection Management ──────────────────────────────────────────

    /// Returns whether the reader loop is still running.
    pub fn is_connected(&self) -> bool {
        self.reader_running.load(Ordering::SeqCst)
    }

    /// Stop the reader loop.
    pub async fn stop_reader(&mut self) {
        self.reader_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.reader_handle.take() {
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }
    }

    /// Close the WebSocket connection.
    pub async fn disconnect(&mut self) -> CortexResult<()> {
        self.stop_reader().await;

        let mut writer = self.writer.lock().await;
        let _ = writer.close().await;

        Ok(())
    }
}
