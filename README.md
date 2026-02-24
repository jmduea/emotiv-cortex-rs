# emotiv-cortex-rs

Rust workspace for Emotiv Cortex v2 tooling and integrations.

## Status and disclaimer

**Pre-release.** These crates are under active development. APIs and behavior may change; treat as pre-release software when integrating or depending on them.

**Not affiliated with Emotiv.** This project is independent, community-maintained, and is **not** created by, affiliated with, supported by, sponsored by, or endorsed by Emotiv, Inc. Emotiv and Emotiv Cortex are trademarks of Emotiv, Inc. This repository builds on and interoperates with the Emotiv Cortex API; for official support and products, see [emotiv.com](https://www.emotiv.com/).

[CI](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)

## Crates

- `emotiv-cortex-v2` - typed Rust client for the Emotiv Cortex v2 WebSocket API
- `emotiv-cortex-tui` - interactive TUI for exploring Cortex APIs and streaming
self-documenting LSL outlets

## Install

```bash
cargo install emotiv-cortex-tui
```

From this repository checkout:

```bash
# bash / macOS / WSL
./scripts/install-emotiv-cortex-tui.sh

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-tui.ps1
```

Enable LSL support (Windows and macOS only):

```bash
# cargo directly
cargo install emotiv-cortex-tui --features lsl

# bash / macOS
./scripts/install-emotiv-cortex-tui.sh --lsl

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-tui.ps1 -Lsl
```

## Development

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy -p emotiv-cortex-v2 --lib --no-default-features --features rustls-tls,config-toml -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::unimplemented
cargo clippy -p emotiv-cortex-tui --bin emotiv-cortex-tui --no-default-features -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::unimplemented

# feature matrix
cargo check -p emotiv-cortex-v2 --no-default-features --features rustls-tls,config-toml
cargo check -p emotiv-cortex-v2 --no-default-features --features native-tls,config-toml
cargo check -p emotiv-cortex-tui --no-default-features
cargo test -p emotiv-cortex-v2 --no-default-features --features rustls-tls,config-toml --tests
```

### Pre-commit and pre-push gates

This repo provides a `pre-commit` configuration with local gates:

- **pre-commit**: `rustfmt` + strict `clippy` for `emotiv-cortex-v2` and `emotiv-cortex-tui`
- **pre-push**:
  - test baseline (`rustls`)
  - doctests for `emotiv-cortex-v2`
  - rustdoc builds for both crates with warnings denied
  - workspace coverage gate (line coverage >= 50%)

Install and run checks with **uv** (recommended):

```bash
uv sync
cargo install cargo-llvm-cov   # needed for pre-push coverage gate
uv run pre-commit -- run --all-files
uv run pre-commit -- run --all-files --hook-stage pre-push
```

Use the repo githooks so Git runs pre-commit via uv (required for hooks to work without a global `pre-commit`). **Do not use `--no-verify`** on commit/push or you bypass these gates:

```bash
git config core.hooksPath .githooks
```

Or install pre-commit yourself and use its hooks (requires `pre-commit` on PATH, e.g. `pipx install pre-commit`):

```bash
pipx install pre-commit
pre-commit install --hook-type pre-commit --hook-type pre-push
```

Pedantic linting remains non-blocking for now:

```bash
cargo clippy -p emotiv-cortex-v2 --lib --no-default-features --features rustls-tls,config-toml -- -W clippy::pedantic
cargo clippy -p emotiv-cortex-tui --bin emotiv-cortex-tui --no-default-features -- -W clippy::pedantic
```

