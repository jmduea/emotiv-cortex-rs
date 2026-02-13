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

    /// Attempt to reconnect with exponential backoff.
    pub(super) async fn reconnect(&self) -> CortexResult<()> {
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
