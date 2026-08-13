use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::time::Instant;

use crate::client::CortexClient;
use crate::error::{CortexError, CortexResult};
use crate::health::{HealthMonitor, HealthStatus};

use super::{ClientState, ConnectionEvent, ResilientClient};

impl ResilientClient {
    /// Start the background health monitor.
    pub(super) async fn start_health_monitor(&self) {
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

    /// Attempt to reconnect the client that produced a connection error.
    ///
    /// If another operation has already replaced `failed_client`, this is a
    /// stale failure: the active connection is left untouched and the caller
    /// retries on that connection.
    pub(super) async fn reconnect(&self, failed_client: &Arc<CortexClient>) -> CortexResult<()> {
        let Some(_guard) = self.claim_reconnect(failed_client).await? else {
            return Ok(());
        };

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
                    let warning_rx = new_client.warning_receiver();
                    match new_client
                        .authenticate(&self.config.client_id, &self.config.client_secret)
                        .await
                    {
                        Ok(new_token) => {
                            if !self
                                .install_reconnected_client(
                                    failed_client,
                                    new_client,
                                    warning_rx,
                                    new_token,
                                    attempt,
                                )
                                .await
                            {
                                return Ok(());
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            tracing::warn!(
                                attempt,
                                error_category = e.category(),
                                api_code = ?e.api_code(),
                                "Connected but authentication failed"
                            );
                            // Close the failed attempt's connection instead
                            // of leaking its reader task/socket.
                            new_client.shutdown().await;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        error_category = e.category(),
                        api_code = ?e.api_code(),
                        "Reconnection attempt failed"
                    );
                }
            }

            if attempt < max_attempts {
                let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
                tracing::debug!(delay_ms, "Backing off before retry");
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

    /// Claim the reconnect slot only for the currently active failed client.
    async fn claim_reconnect<'a>(
        &'a self,
        failed_client: &Arc<CortexClient>,
    ) -> CortexResult<Option<ReconnectGuard<'a>>> {
        if !Arc::ptr_eq(&self.client().await, failed_client) {
            return Ok(None);
        }

        if self
            .reconnecting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            while self.reconnecting.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            if self.client().await.is_connected() {
                return Ok(None);
            }
            return Err(CortexError::ConnectionLost {
                reason: "Concurrent reconnection failed".into(),
            });
        }

        let guard = ReconnectGuard(&self.reconnecting);
        if !Arc::ptr_eq(&self.client().await, failed_client) {
            return Ok(None);
        }

        Ok(Some(guard))
    }

    /// Swap a freshly authenticated client into the shared state, shut down
    /// the connection it replaces, and restart supporting tasks.
    async fn install_reconnected_client(
        &self,
        failed_client: &Arc<CortexClient>,
        new_client: CortexClient,
        warning_rx: tokio::sync::broadcast::Receiver<crate::protocol::warnings::CortexWarning>,
        new_token: String,
        attempt: u32,
    ) -> bool {
        let new_client = Arc::new(new_client);

        // Install only if the failed generation is still active. This
        // compare-and-swap invariant prevents a late reconnect result from
        // replacing and shutting down a healthy client.
        let old_client = {
            let mut state = self.state.write().await;
            if !Arc::ptr_eq(&state.client, failed_client) {
                drop(state);
                new_client.shutdown().await;
                return false;
            }

            let old_client = Arc::clone(&state.client);
            *state = ClientState {
                client: Arc::clone(&new_client),
                cortex_token: new_token,
                token_obtained_at: Instant::now(),
            };
            old_client
        };

        Self::spawn_warning_relay(&self.warning_tx, warning_rx);

        // Explicitly shut down the replaced connection — dropping the Arc
        // alone would leave its reader task and socket alive until the
        // server closes.
        old_client.shutdown().await;

        let _ = self.event_tx.send(ConnectionEvent::Reconnected);
        tracing::info!(attempt, "Reconnected and re-authenticated");

        if self.config.health.enabled {
            self.start_health_monitor().await;
        }

        true
    }

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
    ///
    /// # Errors
    /// Returns any error produced by shutting down background health monitoring.
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

        // Explicitly shut down the active connection. Dropping the Arc is
        // not sufficient: the reader task owns clones of its state and
        // would keep the socket alive until the server closes it.
        let client = Arc::clone(&self.state.read().await.client);
        client.shutdown().await;

        Ok(())
    }
}

/// Guard that resets the reconnecting flag when dropped.
struct ReconnectGuard<'a>(&'a AtomicBool);

impl Drop for ReconnectGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
