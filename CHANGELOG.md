# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Prepared the upcoming `0.4.0` repository release with lockstep versions for `emotiv-cortex-v2` and `emotiv-cortex-tui`.
- Finalized the post-parity release contract so the release tag, both crate versions, and the TUI's exact `emotiv-cortex-v2 =0.4.0` dependency pin are validated together before release automation runs.
- Kept crates.io publication scoped to `emotiv-cortex-v2`; `emotiv-cortex-tui` remains a binary-only deliverable distributed as Windows assets on GitHub Releases.
- Reworked release preflight automation so `publish-dry-run.yml` now dry-runs `emotiv-cortex-v2` publication and separately verifies the staged Windows TUI release assets instead of simulating a crates.io publish for the TUI.
- Refreshed release-facing documentation so changelog and install guidance match the enforced `0.4.0` distribution policy.
