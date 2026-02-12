//! # Connection Health Monitor
//!
//! Background task that periodically pings the Cortex API via `getCortexInfo`
//! to detect connection staleness before it causes user-visible failures.
//!
//! Used internally by [`ResilientClient`](crate::reconnect::ResilientClient)
//! to trigger proactive reconnection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::client::CortexClient;
use crate::config::HealthConfig;

/// Signals emitted by the health monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// The Cortex API responded successfully.
    Healthy,

    /// A health check failed. Contains the consecutive failure count.
    Degraded { consecutive_failures: u32 },

    /// Too many consecutive failures — connection is considered dead.
    Unhealthy { consecutive_failures: u32 },
}

/// Background health monitor that periodically checks the Cortex connection.
///
/// Polls `getCortexInfo()` at a configurable interval and emits
/// [`HealthStatus`] events via an mpsc channel. After
/// `max_consecutive_failures` failures, emits `Unhealthy` to signal
/// that a reconnection is needed.
pub struct HealthMonitor {
    handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl HealthMonitor {
    /// Start the health monitor.
    ///
    /// Returns the monitor handle and a receiver for health status events.
    /// The monitor runs until [`stop()`](Self::stop) is called or the
    /// `CortexClient` Arc is dropped.
    pub fn start(
        client: Arc<CortexClient>,
        config: &HealthConfig,
    ) -> (Self, mpsc::Receiver<HealthStatus>) {
        let interval = Duration::from_secs(config.interval_secs);
        let max_failures = config.max_consecutive_failures;
        let running = Arc::new(AtomicBool::new(true));

        let (tx, rx) = mpsc::channel(16);

        let handle = {
            let running = Arc::clone(&running);
            tokio::spawn(async move {
                let mut consecutive_failures: u32 = 0;

                while running.load(Ordering::SeqCst) {
                    tokio::time::sleep(interval).await;

                    if !running.load(Ordering::SeqCst) {
                        break;
                    }

                    match client.get_cortex_info().await {
                        Ok(_) => {
                            if consecutive_failures > 0 {
                                tracing::info!(
                                    previous_failures = consecutive_failures,
                                    "Health check recovered"
                                );
                            }
                            consecutive_failures = 0;
                            let _ = tx.try_send(HealthStatus::Healthy);
                        }
                        Err(e) => {
                            consecutive_failures += 1;
                            tracing::warn!(
                                consecutive_failures,
                                error = %e,
                                "Health check failed"
                            );

                            if consecutive_failures >= max_failures {
                                let _ = tx.try_send(HealthStatus::Unhealthy {
                                    consecutive_failures,
                                });
                            } else {
                                let _ = tx.try_send(HealthStatus::Degraded {
                                    consecutive_failures,
                                });
                            }
                        }
                    }
                }

                tracing::debug!("Health monitor stopped");
            })
        };

        (
            Self {
                handle: Some(handle),
                running,
            },
            rx,
        )
    }

    /// Stop the health monitor.
    pub async fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }
    }

    /// Returns whether the monitor is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for HealthMonitor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_variants() {
        let healthy = HealthStatus::Healthy;
        let degraded = HealthStatus::Degraded {
            consecutive_failures: 2,
        };
        let unhealthy = HealthStatus::Unhealthy {
            consecutive_failures: 5,
        };

        assert_eq!(healthy, HealthStatus::Healthy);
        assert_eq!(
            degraded,
            HealthStatus::Degraded {
                consecutive_failures: 2
            }
        );
        assert_ne!(healthy, unhealthy);
    }
}
