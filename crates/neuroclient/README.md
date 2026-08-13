# NeuroClient

An independent Rust client for the
[Emotiv Cortex v2 WebSocket API](https://emotiv.gitbook.io/cortex-api/).

[![CI](https://github.com/jmduea/NeuroClient/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/NeuroClient/actions/workflows/ci.yml)

Provides a typed interface to the Emotiv Cortex service for interacting with Emotiv EEG headsets (Insight, EPOC+, EPOC X, EPOC Flex). Protocol coverage is tracked endpoint-by-endpoint against the official API reference in [`docs/api-parity.md`](docs/api-parity.md); rows are marked `match` only where deterministic mock contract tests or documented workflow tests exist.

**Hardware validation:** end-to-end testing against physical hardware has only been performed with an Insight headset. Other models (EPOC+, EPOC X, EPOC Flex) are supported per the documented protocol but have not been validated on-device.

## Features

- Evidence-backed endpoint parity tracking in `docs/api-parity.md`, with `match` rows limited to deterministic contract or workflow coverage
- Two-layer client: raw `CortexClient` for full control, `ResilientClient` for use with auto-reconnect, token refresh, and retry
- Typed data streams (EEG, motion, band power, performance metrics, mental commands, facial expressions, device quality)
- Feature-selectable TLS backend (`rustls-tls` default, `native-tls` opt-in)
- TOML config loading can be enabled/disabled via `config-toml`

## Feature Flags

| Feature       | Default | Description                                                          |
| ------------- | ------- | -------------------------------------------------------------------- |
| `rustls-tls`  | yes     | Use rustls TLS backend (`tokio-tungstenite/rustls-tls-webpki-roots`) |
| `native-tls`  | no      | Use native TLS backend (`tokio-tungstenite/native-tls`)              |
| `config-toml` | yes     | Enable TOML parsing for `CortexConfig::from_file`/`discover`         |

Exactly one TLS backend feature must be enabled (`rustls-tls` or `native-tls`).
If `config-toml` is disabled, `CortexConfig::from_file` and file-based `discover` return a `ConfigError` explaining how to re-enable TOML parsing.

## Which client should I use?

| Layer      | Type              | Token mgmt | Reconnect | Best for                                |
| ---------- | ----------------- | ---------- | --------- | --------------------------------------- |
| Low-level  | `CortexClient`    | Manual     | No        | tooling, tests, direct protocol control |
| High-level | `ResilientClient` | Automatic  | Yes       | ease of use                             |

## Prerequisites

- [EMOTIV Launcher](https://www.emotiv.com/emotiv-launcher/) installed and running
- API credentials from the [Emotiv Developer Portal](https://www.emotiv.com/developer/)

## Installation

`0.4.0` is the first release under the `neuroclient` package name:

```toml
[dependencies]
neuroclient = "0.4.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Use native TLS instead of rustls:

```toml
[dependencies]
neuroclient = { version = "0.4.0", default-features = false, features = ["native-tls", "config-toml"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Versions through `0.3.4` were published as `emotiv-cortex-v2`. That package is
frozen and remains unyanked for reproducible builds. See the
[0.3-to-0.4 migration guide](docs/migration-0.3-to-0.4.md).

## Quick Start

```rust
use neuroclient::{CortexClient, CortexConfig};
use neuroclient::protocol::headset::QueryHeadsetsOptions;

#[tokio::main]
async fn main() -> neuroclient::CortexResult<()> {
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

Or create a `cortex.toml` (see `cortex.toml.example` for all options). Discovery
checks `./cortex.toml`, then the preferred user path
`~/.config/neuroclient/cortex.toml`, followed by the legacy
`~/.config/emotiv-cortex/cortex.toml`.

```toml
client_id = "your-client-id"
client_secret = "your-client-secret"
```

## Examples

See the [`examples/`](examples/) directory for working examples across the API
areas listed in the parity matrix.

For endpoint-by-endpoint compatibility tracking against the official API reference,
see [`docs/api-parity.md`](docs/api-parity.md). That matrix only marks `match`
where deterministic mock contract tests or documented workflow tests exist, and
it calls out remaining documentation ambiguity explicitly.

## Testing

Run the full crate test suite (unit tests, deterministic mock integration tests, and live smokes):

```bash
cargo test -p neuroclient
```

## Protocol Modules

Types are grouped by domain:

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

**Pre-release.** This crate is under active development; APIs and behavior may change. Treat as pre-release when depending on it.

**Not affiliated with Emotiv.** NeuroClient is independent and is not created
by, affiliated with, supported by, sponsored by, or endorsed by Emotiv, Inc.
Emotiv and Emotiv Cortex are trademarks of Emotiv, Inc. For official support
and products, see [emotiv.com](https://www.emotiv.com/).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
