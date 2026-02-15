# emotiv-cortex-v2

A Rust client for the [Emotiv Cortex v2 WebSocket API](https://emotiv.gitbook.io/cortex-api/).

[![CI](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)
[Coverage Reports](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)

Provides a complete, typed interface to the Emotiv Cortex service for interacting with Emotiv EEG headsets (Insight, EPOC+, EPOC X, EPOC Flex).

## Features

- Full Cortex v2 API coverage (authentication, headsets, sessions, all 9 data streams, records, markers, profiles, BCI training)
- Two-layer client: raw `CortexClient` for full control, `ResilientClient` for production use with auto-reconnect, token refresh, and retry
- Typed data streams (EEG, motion, band power, performance metrics, mental commands, facial expressions, device quality)
- Feature-selectable TLS backend (`rustls-tls` default, `native-tls` opt-in)
- TOML config loading can be enabled/disabled via `config-toml`

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `rustls-tls` | yes | Use rustls TLS backend (`tokio-tungstenite/rustls-tls-webpki-roots`) |
| `native-tls` | no | Use native TLS backend (`tokio-tungstenite/native-tls`) |
| `config-toml` | yes | Enable TOML parsing for `CortexConfig::from_file`/`discover` |

Exactly one TLS backend feature must be enabled (`rustls-tls` or `native-tls`).
If `config-toml` is disabled, `CortexConfig::from_file` and file-based `discover` return a `ConfigError` explaining how to re-enable TOML parsing.

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
emotiv-cortex-v2 = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Use native TLS instead of rustls:

```toml
[dependencies]
emotiv-cortex-v2 = { version = "0.3", default-features = false, features = ["native-tls", "config-toml"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use emotiv_cortex_v2::{CortexClient, CortexConfig};
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;

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

## Testing

Run the full crate test suite (unit tests, deterministic mock integration tests, and live smokes):

```bash
cargo test -p emotiv-cortex-v2
```

## Protocol Modules

Types are now grouped by domain:

- `protocol::rpc` - JSON-RPC request/response/error
- `protocol::constants` - `Methods`, `ErrorCodes`, `Streams`
- `protocol::headset` - headset and config-mapping types
- `protocol::session` - `SessionInfo`
- `protocol::streams` - stream event payloads and parsed stream structs
- `protocol::records` - record/marker/export types
- `protocol::profiles` - profile types and actions
- `protocol::training` - detection/training + advanced BCI types
- `protocol::auth` - user login types
- `protocol::subjects` - subject and demographic types

## Migration Notes

Legacy flat imports were removed. Update imports like:

```rust
// old
use emotiv_cortex_v2::protocol::{QueryHeadsetsOptions, Methods, Streams, TrainingStatus};

// new
use emotiv_cortex_v2::protocol::constants::{Methods, Streams};
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::protocol::training::TrainingStatus;
```

`0.3.0` also introduces request-DTO APIs for multi-parameter operations
(`update_record_with`, `create_subject_with`, `update_subject_with`,
`query_subjects_with`, and training threshold/signature request variants).
See `docs/migration-0.2-to-0.3.md` for a full old/new mapping table.

Live smoke tests auto-skip when prerequisites are missing, and can be forced off with:

```bash
EMOTIV_SKIP_LIVE_TESTS=1 cargo test -p emotiv-cortex-v2
```

To run live smoke tests against Cortex locally, set:

```bash
export EMOTIV_CLIENT_ID="your-client-id"
export EMOTIV_CLIENT_SECRET="your-client-secret"
# optional:
export EMOTIV_CORTEX_URL="wss://localhost:6868"
export EMOTIV_HEADSET_ID="INSIGHT-XXXXXXXX"
unset EMOTIV_SKIP_LIVE_TESTS
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
