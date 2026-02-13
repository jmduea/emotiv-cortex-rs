# emotiv-cortex-rs

Rust workspace for Emotiv Cortex v2 tooling and integrations.

## Crates

- `emotiv-cortex-v2` - typed Rust client for the Emotiv Cortex v2 WebSocket API
- `emotiv-cortex-cli` - interactive CLI for exploring Cortex APIs and streaming
  self-documenting LSL outlets

## Install

```bash
cargo install emotiv-cortex-cli
```

Enable LSL support:

```bash
cargo install emotiv-cortex-cli --features lsl
```

Note: LSL is currently supported on Windows and macOS only.

## Development

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy -p emotiv-cortex-v2 --lib --no-default-features --features rustls-tls,config-toml -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::unimplemented
cargo clippy -p emotiv-cortex-cli --bin emotiv-cortex-cli --no-default-features -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::unimplemented

# feature matrix
cargo check -p emotiv-cortex-v2 --no-default-features --features rustls-tls,config-toml
cargo check -p emotiv-cortex-v2 --no-default-features --features native-tls,config-toml
cargo check -p emotiv-cortex-cli --no-default-features
cargo test -p emotiv-cortex-v2 --no-default-features --features rustls-tls,config-toml --tests
```

Pedantic linting remains non-blocking for now:

```bash
cargo clippy -p emotiv-cortex-v2 --lib --no-default-features --features rustls-tls,config-toml -- -W clippy::pedantic
cargo clippy -p emotiv-cortex-cli --bin emotiv-cortex-cli --no-default-features -- -W clippy::pedantic
```

Migration guide:

- `crates/emotiv-cortex-v2/docs/migration-0.2-to-0.3.md`

## Release

Releases are tag-driven. Push a `vX.Y.Z` tag and the release workflow publishes:

1. `emotiv-cortex-v2`
2. `emotiv-cortex-cli` (after crates.io index propagation)
