# emotiv-cortex-v2

A Rust client for the [Emotiv Cortex v2 WebSocket API](https://emotiv.gitbook.io/cortex-api/).

Provides a complete, typed interface to the Emotiv Cortex service for interacting with Emotiv EEG headsets (Insight, EPOC+, EPOC X, EPOC Flex).

## Features

- Full Cortex v2 API coverage (authentication, headsets, sessions, all 9 data streams, records, markers, profiles, BCI training)
- Two-layer client: raw `CortexClient` for full control, `ResilientClient` for production use with auto-reconnect, token refresh, and retry
- Typed data streams (EEG, motion, band power, performance metrics, mental commands, facial expressions, device quality)
- Automatic TLS handling (self-signed certs for localhost)
- Configurable via TOML file or environment variables

## Which client should I use?

| Layer | Type | Token mgmt | Reconnect | Best for |
|---|---|---|---|---|
| Low-level | `CortexClient` | Manual | No | tooling, tests, direct protocol control |
| High-level | `ResilientClient` | Automatic | Yes | long-running production applications |

## Prerequisites

- [EMOTIV Launcher](https://www.emotiv.com/emotiv-launcher/) installed and running
- API credentials from the [Emotiv Developer Portal](https://www.emotiv.com/developer/)

## Quick Start

```toml
[dependencies]
emotiv-cortex-v2 = "0.2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use emotiv_cortex_v2::{CortexClient, CortexConfig};
use emotiv_cortex_v2::protocol::QueryHeadsetsOptions;

#[tokio::main]
async fn main() -> emotiv_cortex_v2::CortexResult<()> {
    let config = CortexConfig::discover(None)?;
    let mut client = CortexClient::connect(&config).await?;

    let info = client.get_cortex_info().await?;
    println!("Cortex: {:?}", info);

    let token = client.authenticate(&config.client_id, &config.client_secret).await?;
    let headsets = client.query_headsets(QueryHeadsetsOptions::default()).await?;
    for h in &headsets {
        println!("{} ({})", h.id, h.status);
    }

    client.disconnect().await?;
    Ok(())
}
```

## Configuration

Set environment variables:

```bash
export EMOTIV_CLIENT_ID="your-client-id"
export EMOTIV_CLIENT_SECRET="your-client-secret"
```

Or create a `cortex.toml` (see `cortex.toml.example` for all options):

```toml
client_id = "your-client-id"
client_secret = "your-client-secret"
```

## Examples

See the [`examples/`](examples/) directory for complete working examples covering all API areas.

For endpoint-by-endpoint compatibility tracking against the official API reference,
see [`docs/api-parity.md`](docs/api-parity.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
