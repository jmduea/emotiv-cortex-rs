# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-08-13

### Security

- Removed sensitive payloads from all tracing output: raw request/response/frame
  bodies, `clientSecret`, `cortexToken`, license strings, server error text, and
  biosignal samples are no longer logged at any level. `CortexConfig`,
  `CortexRequest`, `CortexResponse`, `RpcError`, `CortexError`,
  `SubscriptionFailure`, and `CortexWarning` now have redacting `Debug`
  implementations; internal logs, examples, and the TUI report non-sensitive
  error categories instead of server messages, and regression tests assert
  sentinel secrets never reach captured tracing output.
- Removed cortex-token prefixes from the TUI log panel and the `auth` example output.
- Updated dependencies so `cargo audit` and `cargo deny check` pass with no exceptions: patched `rand`, `aws-lc-sys`, and `rustls-webpki` transitives, and upgraded ratatui to 0.30 / crossterm to 0.29, which drops the vulnerable `lru 0.12` and unmaintained `paste` crates.
- Added a `Security` workflow (`cargo audit` + `cargo deny check`) that runs on PRs, main, a weekly schedule, and as a required prerequisite of the release pipeline. Tightened `deny.toml` (yanked crates denied; advisory exceptions require an ID, rationale, owner, and expiry).

### Added

- `protocol::warnings` module with `CortexWarning { code, message }` and known
  warning-code constants. Both `CortexClient` and `ResilientClient` expose a
  broadcast `warning_receiver()`; warnings are dispatched before stream
  samples, relay across reconnects (including warnings buffered during
  authentication), and codes 0/1 automatically close only the affected
  session's stream routes.
- Typed subscription outcomes: `subscribe_streams`/`unsubscribe_streams` now return `SubscribeResult` with per-stream `SubscriptionSuccess`/`SubscriptionFailure` entries instead of raw JSON, so partial failures are visible. Typed one-stream helpers roll back their local route and return a structured error when the requested stream is not confirmed.
- TUI LSL: mental-command and facial-expression events now publish paired outlets — an irregular-rate string marker outlet (action labels, including custom actions) plus the numeric power outlet — sharing one LSL timestamp per event and declaring `desc/pairing/{role,partner_stream}` metadata.
- CI MSRV checks: the library is checked on Rust 1.85 (both TLS backends) and the TUI on Rust 1.88.

### Fixed

- `CortexClient::shutdown` is now idempotent and bounded: it signals the reader, performs a timed WebSocket close, joins the reader task (aborting as a fallback), drains pending RPCs, and closes stream routes so callers never hang. `Drop` aborts the reader when async shutdown was skipped.
- Reconnection now explicitly shuts down the replaced connection (and failed
  attempts), `ResilientClient::disconnect` closes the active socket, and stale
  operation failures or token refreshes can no longer replace or overwrite
  state written by a concurrent reconnect. Manual token-generation results are
  also committed only to the client generation that produced them.
- Stream dispatch is session-aware: routes are keyed by
  `(session_id, stream)`, so simultaneous sessions with the same stream types
  stay isolated. Bulk route creation preserves other sessions and rejects
  duplicate batches atomically instead of replacing receivers.
- Stream subscription reservations are cancellation-safe and
  sender-generation-safe. Partial unsubscribe responses now remove only
  confirmed routes, give failure entries precedence over contradictory success
  entries, and cannot remove a newer route while reporting remaining
  per-stream failures.
- TUI `--url` no longer has an implicit clap default, restoring the documented precedence `--url` > `EMOTIV_CORTEX_URL` > config file > library default.
- TUI LSL facial-expression schema: removed the placeholder `reserved` channel.
- Quality Ratchet: coverage is extracted from the `cargo llvm-cov` JSON summary instead of positional text parsing, thresholds moved to `ci/quality-thresholds.env` so the scheduled PR no longer needs workflow-file permissions, and the ratchet can only raise the floor. CI clippy gates now deny all warnings directly instead of ratcheting an already-zero budget.
- README corrections: fixed repository and CI links, documented the package
  migration, and evidence-qualified protocol-coverage and hardware claims
  (Insight-only physical-hardware validation).

### Changed

- Renamed the project to **NeuroClient** to make its independent provenance
  explicit. The library is now published as `neuroclient`, Rust imports use
  `neuroclient`, and the unpublished dashboard is `neuroclient-tui`.
- Froze the former `emotiv-cortex-v2` package at 0.3.4. It remains unyanked for
  reproducible builds, but all development and releases continue under
  `neuroclient`.
- Added `NEUROCLIENT_CONFIG`, the `~/.config/neuroclient/cortex.toml` search
  path, and `NEUROCLIENT_INSTALL_ROOT`; legacy config paths and project-owned
  environment aliases remain fallbacks for the 0.4 release.
- `neuroclient-tui` now requires Rust 1.88 (ratatui 0.30); the published
  `neuroclient` library keeps its MSRV at 1.85.
- Prepared the upcoming `0.4.0` release with lockstep versions for
  `neuroclient` and `neuroclient-tui`.
- Finalized the release contract so the tag, both package versions, the TUI's
  exact `neuroclient =0.4.0` dependency pin, lockfile, README, and changelog are
  validated together.
- Kept crates.io publication scoped to `neuroclient`; `neuroclient-tui`
  remains a binary-only deliverable distributed as Windows assets.
- Reworked release preflight automation to dry-run `neuroclient` publication
  and separately verify the staged Windows TUI release assets.

