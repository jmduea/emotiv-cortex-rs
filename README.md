# NeuroClient

[![CI](https://github.com/jmduea/NeuroClient/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/NeuroClient/actions/workflows/ci.yml)

An independent Rust client for the Emotiv Cortex v2 API.

This workspace contains:

- [`neuroclient`](crates/neuroclient) — typed, asynchronous Cortex v2 client library
- [`neuroclient-tui`](crates/neuroclient-tui) — interactive dashboard and optional
  Lab Streaming Layer (LSL) bridge

## Release policy

- `0.4.0` will be the first release under the NeuroClient identity.
- The former `emotiv-cortex-v2` package is frozen and unyanked at `0.3.4`.
- Only `neuroclient` is published to crates.io.
- `neuroclient-tui` exact-pins the matching library version and is distributed as Windows
  binaries through GitHub Releases; other platforms build it from source.
- See the [0.3-to-0.4 migration guide](crates/neuroclient/docs/migration-0.3-to-0.4.md).

## Development

Contributions are welcome. Run the relevant checks before opening a pull request:

```console
cargo fmt --all --check
cargo check --workspace
cargo clippy -p neuroclient --lib --no-default-features \
  --features rustls-tls,config-toml -- -D warnings \
  -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic \
  -D clippy::todo -D clippy::unimplemented

# feature matrix
cargo check -p neuroclient --no-default-features --features rustls-tls,config-toml
cargo check -p neuroclient --no-default-features --features native-tls,config-toml
cargo test -p neuroclient --no-default-features --features rustls-tls,config-toml --tests
```

### Pre-commit and pre-push gates

This repo provides a `pre-commit` configuration with local gates:

- **pre-commit**: `rustfmt` + strict `clippy` for `neuroclient` and `neuroclient-tui`
- **pre-push**:
  - test baseline (`rustls`)
  - doctests for `neuroclient`
  - rustdoc builds for both crates with warnings denied
  - workspace coverage gate using the floor in `ci/quality-thresholds.env`

Install and run checks with **uv** (recommended):

```console
uv sync
cargo install cargo-llvm-cov   # needed for pre-push coverage gate
uv run pre-commit -- run --all-files
uv run pre-commit -- run --all-files --hook-stage pre-push
```

Use the repository hooks so Git runs pre-commit through uv:

```console
git config core.hooksPath .githooks
```

Do not use `--no-verify` with commits or pushes; it bypasses these quality gates.

CI denies all Clippy warnings, including pedantic lints. Coverage and other quality floors
live in `ci/quality-thresholds.env` and are only raised by the scheduled Quality Ratchet.

## Status and disclaimer

**Pre-release.** APIs and behavior may change.

**Hardware validation.** End-to-end physical-hardware testing has only used an Insight
headset. Other supported models follow the documented Cortex protocol but have not been
validated on-device. See the evidence-qualified
[API parity matrix](crates/neuroclient/docs/api-parity.md).

**Not affiliated with Emotiv.** NeuroClient is independent and is not created by, affiliated
with, supported by, sponsored by, or endorsed by Emotiv, Inc. Emotiv and Emotiv Cortex are
trademarks of Emotiv, Inc. NeuroClient interoperates with the Emotiv Cortex API. For official
products and support, visit [emotiv.com](https://www.emotiv.com/).

## License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.
