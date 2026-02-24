# emotiv-cortex-rs

Rust workspace for Emotiv Cortex v2 tooling and integrations.

## Acknowledgments and affiliation

- This project builds on and interoperates with the Emotiv Cortex API and ecosystem.
- Emotiv and Emotiv Cortex are trademarks of Emotiv, Inc.
- This repository is an independent, community-maintained project and is **not** created by,
  affiliated with, sponsored by, or endorsed by Emotiv.

[![CI](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)
[![Test Coverage](https://codecov.io/gh/jmduea/emotiv-cortex-rs/graph/badge.svg?branch=main)](https://codecov.io/gh/jmduea/emotiv-cortex-rs)

## Crates

- `emotiv-cortex-v2` - typed Rust client for the Emotiv Cortex v2 WebSocket API
- `emotiv-cortex-cli` - interactive CLI for exploring Cortex APIs and streaming
  self-documenting LSL outlets

## Install

```bash
cargo install emotiv-cortex-cli
```

From this repository checkout:

```bash
# bash / macOS / WSL
./scripts/install-emotiv-cortex-cli.sh

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-cli.ps1
```

Enable LSL support (Windows and macOS only):

```bash
# cargo directly
cargo install emotiv-cortex-cli --features lsl

# bash / macOS / WSL
./scripts/install-emotiv-cortex-cli.sh --lsl

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-cli.ps1 -Lsl
```

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

### Pre-commit and pre-push gates

This repo provides a `pre-commit` configuration with local gates:

- **pre-commit**: `rustfmt` + strict `clippy` for `emotiv-cortex-v2` and `emotiv-cortex-cli`
- **pre-push**:
  - test baseline (`rustls`)
  - doctests for `emotiv-cortex-v2`
  - rustdoc builds for both crates with warnings denied
  - workspace coverage gate (line coverage >= 50%)

Install and enable hooks:

```bash
pipx install pre-commit
cargo install cargo-llvm-cov
pre-commit install --hook-type pre-commit --hook-type pre-push
```

Run all configured checks manually:

```bash
pre-commit run --all-files
pre-commit run --all-files --hook-stage pre-push
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
