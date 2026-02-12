# emotiv-cortex-rs

Rust workspace for Emotiv Cortex v2 tooling and integrations.

## Crates

- `emotiv-cortex-v2` - typed Rust client for the Emotiv Cortex v2 WebSocket API
- `emotiv-cortex-cli` - interactive CLI for exploring Cortex APIs and streaming to LSL

## Install

```bash
cargo install emotiv-cortex-cli
```

Enable LSL support:

```bash
cargo install emotiv-cortex-cli --features lsl
```

## Development

```bash
cargo check --workspace
cargo test -p emotiv-cortex-v2
cargo check -p emotiv-cortex-cli --no-default-features
```

## Release

Releases are tag-driven. Push a `vX.Y.Z` tag and the release workflow publishes:

1. `emotiv-cortex-v2`
2. `emotiv-cortex-cli` (after crates.io index propagation)

