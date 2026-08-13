# Migrating from `emotiv-cortex-v2` 0.3 to NeuroClient 0.4

NeuroClient 0.4 is the continuation of `emotiv-cortex-v2` under an independent
project identity. The former package is frozen and unyanked at 0.3.4; it will
not receive a 0.4 release.

## Dependency and imports

Update the package dependency:

```toml
# Before
emotiv-cortex-v2 = "0.3.4"

# After
neuroclient = "0.4.0"
```

Update Rust import paths:

```rust
// Before
use emotiv_cortex_v2::{CortexClient, CortexConfig};

// After
use neuroclient::{CortexClient, CortexConfig};
```

The public `Cortex*` types and wire-level protocol names remain unchanged
because they describe the upstream Cortex API.

## Stream API changes

`CortexClient::create_stream_channels` and the resilient wrapper now return
`CortexResult<StreamReceivers>`. Handle the result before taking receivers:

```rust
let mut receivers = client.create_stream_channels(
    session_id,
    &["eeg", "mot"],
)?;
let eeg = receivers.remove("eeg");
```

Bulk channel creation no longer replaces routes belonging to other sessions.
The operation is atomic: if the batch repeats a route or collides with an
existing route, it returns `CortexError::StreamError` without adding any of the
requested routes.

`streams::unsubscribe` now removes only streams confirmed in the response's
`success` array. A partial response preserves routes listed in `failure` (or
missing from both arrays) and returns `CortexError::StreamError` describing the
incomplete unsubscribe.

## TUI

The unpublished TUI package, executable, installers, and future release assets
are now named `neuroclient-tui`:

```console
neuroclient-tui --help
./scripts/install-neuroclient-tui.sh
```

## Configuration

Vendor credential variables remain unchanged:

- `EMOTIV_CLIENT_ID`
- `EMOTIV_CLIENT_SECRET`
- `EMOTIV_CORTEX_URL`
- `EMOTIV_LICENSE`

The preferred explicit config pointer is now `NEUROCLIENT_CONFIG`; the legacy
`CORTEX_CONFIG` variable remains a fallback for 0.4.

The preferred per-user path is:

- Linux/macOS: `~/.config/neuroclient/cortex.toml`
- Windows: `%APPDATA%\neuroclient\cortex.toml`

The former `emotiv-cortex` path remains a fallback for 0.4 so existing
installations continue to work without moving their configuration immediately.

The installer uses `NEUROCLIENT_INSTALL_ROOT`; the legacy
`EMOTIV_CLI_INSTALL_ROOT` variable remains a fallback for 0.4.

## LSL compatibility

Project provenance metadata now reports `application = neuroclient-tui` and
`library = neuroclient`. Existing `Emotiv*` outlet names and
`manufacturer = Emotiv` remain unchanged because they identify the data source
and are consumed by existing LSL integrations.

## Repository

The project repository is moving from `jmduea/emotiv-cortex-rs` to
`jmduea/NeuroClient`. GitHub redirects old repository URLs after the rename,
but local clones should update their origin URL during the release cutover.
