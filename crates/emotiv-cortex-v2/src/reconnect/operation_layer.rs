use std::sync::Arc;

use crate::client::CortexClient;
use crate::error::CortexResult;

use super::ResilientClient;

impl ResilientClient {
    /// Get a clone of the Arc<CortexClient> and the current token.
    pub(super) async fn client_and_token(&self) -> (Arc<CortexClient>, String) {
        let state = self.state.read().await;
        (Arc::clone(&state.client), state.cortex_token.clone())
    }

    /// Get a clone of the Arc<CortexClient>.
    pub(super) async fn client(&self) -> Arc<CortexClient> {
        Arc::clone(&self.state.read().await.client)
    }

    /// Execute a token-free operation with automatic reconnection.
    pub(super) async fn exec<F, Fut, T>(&self, f: F) -> CortexResult<T>
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
    pub(super) async fn exec_with_token<F, Fut, T>(&self, f: F) -> CortexResult<T>
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
}
