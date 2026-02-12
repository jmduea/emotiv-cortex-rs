//! # emotiv-cortex-v2
//!
//! A Rust client for the [Emotiv Cortex v2 WebSocket API](https://emotiv.gitbook.io/cortex-api/).
//!
//! This crate provides a complete, typed interface to the Emotiv Cortex service
//! for interacting with Emotiv EEG headsets (Insight, EPOC+, EPOC X, EPOC Flex).
//!
//! ## Quick Start
//!
//! ```ignore
//! use emotiv_cortex_v2::{CortexClient, CortexConfig};
//!
//! #[tokio::main]
//! async fn main() -> emotiv_cortex_v2::CortexResult<()> {
//!     // Load config from environment or cortex.toml
//!     let config = CortexConfig::discover(None)?;
//!
//!     // Connect to the Cortex service
//!     let mut client = CortexClient::connect(&config).await?;
//!
//!     // Check service info
//!     let info = client.get_cortex_info().await?;
//!     println!("Cortex version: {:?}", info);
//!
//!     // Authenticate
//!     let token = client.authenticate(&config.client_id, &config.client_secret).await?;
//!
//!     // Discover headsets
//!     let headsets = client.query_headsets().await?;
//!     for h in &headsets {
//!         println!("Found: {} ({})", h.id, h.status);
//!     }
//!
//!     client.disconnect().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Two-Layer API
//!
//! | Layer | Type | Token mgmt | Reconnect | Retry | Best for |
//! |-------|------|-----------|-----------|-------|----------|
//! | Low-level | [`CortexClient`] | Manual | No | No | Examples, testing, full control |
//! | High-level | `ResilientClient` | Automatic | Yes | Yes | Production applications |
//!
//! ## Configuration
//!
//! See [`CortexConfig`] for the full configuration reference.
//! The simplest setup uses environment variables:
//!
//! ```bash
//! export EMOTIV_CLIENT_ID="your-client-id"
//! export EMOTIV_CLIENT_SECRET="your-client-secret"
//! ```
//!
//! Or a `cortex.toml` file:
//!
//! ```toml
//! client_id = "your-client-id"
//! client_secret = "your-client-secret"
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod headset;
pub mod health;
pub mod protocol;
pub mod reconnect;
pub mod retry;
pub mod streams;

// ─── Public re-exports ──────────────────────────────────────────────────

pub use client::CortexClient;
pub use config::CortexConfig;
pub use error::{CortexError, CortexResult};
pub use headset::HeadsetModel;
pub use reconnect::ResilientClient;
pub use streams::TypedStream;
