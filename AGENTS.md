# NeuroClient Agent Guidelines

## Project scope

NeuroClient is an independent Rust client for the Emotiv Cortex v2 API. It is not an
official Emotiv project. Keep vendor and protocol names where they identify compatibility;
use NeuroClient for project-owned names, metadata, binaries, and documentation.

## Rust toolchains

- Use Rust edition 2024.
- `neuroclient` supports Rust 1.85.
- `neuroclient-tui` supports Rust 1.88 because of its Ratatui dependency.
- Keep workspace lints at `unsafe_code = "warn"`, `clippy::all = "warn"`, and
  `clippy::pedantic = "warn"`.
- Do not raise either MSRV without an explicit compatibility decision and matching CI changes.

## Code standards

- Use `snake_case` for functions and variables, `PascalCase` for types and traits, and
  `SCREAMING_SNAKE_CASE` for constants.
- Keep lines at or below 100 characters where practical.
- In library code, propagate typed errors with `Result` and `?`; do not use `unwrap`,
  `expect`, `panic`, `todo`, or `unimplemented`.
- Avoid holding synchronization guards across `.await`.
- Preserve wire-level Cortex field names and method names exactly.
- Every `unsafe` block must have an adjacent `// SAFETY:` comment that states its invariant.

## Tooling

- Use `uv` for Python execution and tooling. Do not add bare `python` commands to scripts,
  documentation, or automation.
- Prefer `rtk` for verbose shell commands, including each command in a chain.
- Before relying on RTK in a new environment, run `rtk --version` and `rtk gain`.

## Git hooks

- Never use `--no-verify` with `git commit` or `git push`.
- Configure repository hooks with `git config core.hooksPath .githooks`.
- Hooks run through `uv`; do not require a globally installed `pre-commit`.

## Required verification

Run checks proportional to the change. Before release or broad refactors, run the full matrix:

```bash
rtk cargo fmt --all --check
rtk cargo check --workspace
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test -p neuroclient --doc
rtk cargo rustdoc -p neuroclient -- -D warnings
rtk cargo rustdoc -p neuroclient-tui -- -D warnings
rtk cargo audit
rtk cargo deny check
```

Also verify `neuroclient` with Rust 1.85 and `neuroclient-tui` with Rust 1.88. The coverage
floor is defined only in `ci/quality-thresholds.env`; workflows and documentation must not
hard-code a conflicting value.

## Documentation and releases

- Update README examples, rustdoc, migration notes, and the changelog when public behavior
  or protocol coverage changes.
- Keep hardware-validation claims evidence-qualified.
- Release tags, package versions, the TUI's exact library pin, `Cargo.lock`, the README
  install snippet, and the changelog release heading must agree.
