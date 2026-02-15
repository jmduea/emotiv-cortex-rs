use tokio::time::Instant;

use crate::error::CortexResult;

use super::{ResilientClient, TOKEN_REFRESH_INTERVAL};

impl ResilientClient {
    /// Returns the current Cortex token (for advanced use cases).
    pub async fn cortex_token(&self) -> String {
        self.state.read().await.cortex_token.clone()
    }

    /// Check if the token should be refreshed and do so if needed.
    pub(super) async fn maybe_refresh_token(&self) -> CortexResult<()> {
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

    /// Generate a new cortex token (or refresh an existing one).
    ///
    /// On success, also updates the internal token and refresh timestamp.
    ///
    /// # Errors
    /// Returns any error produced by the underlying Cortex API call,
    /// including connection, authentication, protocol, timeout, and configuration errors.
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
        state.cortex_token.clone_from(&new_token);
        state.token_obtained_at = Instant::now();

        Ok(new_token)
    }
}
