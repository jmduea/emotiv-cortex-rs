# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial workspace split from `neurohid` into dedicated repository.
- Tag-driven crates.io publish pipeline for `emotiv-cortex-v2` and `emotiv-cortex-tui`.
- Repo-level `.pre-commit-config.yaml` with local pre-commit/pre-push quality gates.

### Changed

- **Breaking** `emotiv-cortex-v2` refactor to full Cortex parity for prior `partial` rows:
  - `query_headsets` now requires `QueryHeadsetsOptions`.
  - `sync_with_headset_clock` now uses docs-aligned payload (`headset`, `monotonicTime`, `systemTime`) and returns `HeadsetClockSyncResult`.
  - `config_mapping` now accepts `ConfigMappingRequest` and returns typed `ConfigMappingResponse`.
  - `get_current_profile` now returns `CurrentProfileInfo` (with optional `name`) instead of `Option<ProfileInfo>`.
  - `ProfileInfo` is now strictly aligned to `queryProfile` fields.
  - `HeadsetInfo` now includes additional documented optional fields with forward-compatible `extra`.
- **Breaking** API redesign for multi-parameter operations:
  - Added request-DTO methods (`*_with`) for record update, subject CRUD query/update paths, and facial/mental threshold/signature paths.
  - Legacy long-argument methods remain as deprecated compatibility wrappers.
- Transport lifecycle hardening:
  - Pending RPC entries now clean up synchronously on send failure and timeout.
  - Reader loop now uses shutdown signaling (non-polling) and drains pending waiters on stop.
  - Stream dispatch now records `delivered`, `dropped_full`, and `dropped_closed` counters.
- CLI LSL transport now removes `unsafe impl Send` and uses dedicated outlet worker ownership threads.
- Workspace/toolchain baseline updated to edition `2024` and rust-version `1.85`.
- CI now uses explicit feature matrix (`rustls`, `native-tls`, CLI no-default-features, CLI LSL on Linux), rustfmt check, and curated blocking clippy gate with pedantic reporting non-blocking.
- CI now enforces workspace line coverage threshold (>= 50%) using `cargo-llvm-cov`.
- CI docs gates now include `emotiv-cortex-v2` doctests and rustdoc warnings-as-errors checks for both crates, plus rustdoc `ignore` snippet scanning for both `src` trees.
- CI now publishes coverage/docs gate results to GitHub job summaries and uploads `coverage.txt` alongside `lcov.info` artifacts.
- Bumped crate versions to `0.3.0` for `emotiv-cortex-v2` and `emotiv-cortex-tui`.
- CLI LSL metadata now aligns stream/channel naming to XDF conventions where defined:
  - `EmotivMetrics` stream `type` changed from empty to `Metrics`.
  - `EmotivDeviceQuality` and `EmotivEEGQuality` stream `type` changed from `EEG` to `Quality`.
  - EEG channel metadata now emits `location_label` plus nested `location/{X,Y,Z}` coordinates and `cap/labelscheme=10-20`.
  - Channel `type` values now use normalized names (`EEG`, `OrientationA..D`, `Stim`, and `Misc` fallback).
