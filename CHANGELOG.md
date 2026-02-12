# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial workspace split from `neurohid` into dedicated repository.
- Tag-driven crates.io publish pipeline for `emotiv-cortex-v2` and `emotiv-cortex-cli`.

### Changed

- **Breaking** `emotiv-cortex-v2` refactor to full Cortex parity for prior `partial` rows:
  - `query_headsets` now requires `QueryHeadsetsOptions`.
  - `sync_with_headset_clock` now uses docs-aligned payload (`headset`, `monotonicTime`, `systemTime`) and returns `HeadsetClockSyncResult`.
  - `config_mapping` now accepts `ConfigMappingRequest` and returns typed `ConfigMappingResponse`.
  - `get_current_profile` now returns `CurrentProfileInfo` (with optional `name`) instead of `Option<ProfileInfo>`.
  - `ProfileInfo` is now strictly aligned to `queryProfile` fields.
  - `HeadsetInfo` now includes additional documented optional fields with forward-compatible `extra`.
- Bumped crate versions to `0.2.0` for `emotiv-cortex-v2` and `emotiv-cortex-cli`.
