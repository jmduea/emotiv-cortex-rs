# NeuroClient TUI

<img width="1840" height="613" alt="tui_device_panel" src="https://github.com/user-attachments/assets/188bdb44-b059-4477-8c48-7f648c06b4d6" />

The terminal dashboard and LSL bridge for NeuroClient.

[![CI](https://github.com/jmduea/NeuroClient/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/NeuroClient/actions/workflows/ci.yml)

A full-screen ratatui TUI for visualizing device info/streams/etc:

- **Dashboard** — session info, performance metric gauges, mental command /
  facial expression readouts
- **Streams** — live EEG sparklines, motion/IMU line charts, band-power
  breakdowns (cycle views with `v`)
- **LSL** — optional Lab Streaming Layer forwarding with per-stream sample
  counts (toggle with `l`, requires `--features lsl`)
- **Device** — full headset metadata and per-channel contact quality gauges
- **Log** — scrollable timestamped event log

## Install

`neuroclient-tui` is versioned in lockstep with `neuroclient`, but it is not published to crates.io. Install the Windows release binaries from GitHub Releases, or build from source on other platforms.

**Windows:** Download the binary from
[GitHub Releases](https://github.com/jmduea/NeuroClient/releases):

- Baseline (no LSL)
  - `neuroclient-tui-x86_64-pc-windows-msvc.exe`

- [Lab Streaming Layer](https://github.com/sccn/labstreaminglayer) support for streaming to other apps
  - `neuroclient-tui-x86_64-pc-windows-msvc-lsl.exe`

**From source** (any platform):

Building from source requires Rust 1.88 or newer (the published
`neuroclient` library keeps its own MSRV at 1.85).

Clone this repo then:

```bash
# bash / macOS / WSL
./scripts/install-neuroclient-tui.sh

# PowerShell (Windows)
.\scripts\install-neuroclient-tui.ps1

# LSL versions
# LSL support is currently available on Windows and macOS.
# Linux is currently unsupported for `--features lsl`.
./scripts/install-neuroclient-tui.sh --lsl   # bash / macOS
.\scripts\install-neuroclient-tui.ps1 -Lsl   # PowerShell (Windows)
```

Or run with Cargo:

```bash
# No LSL
cargo run -p neuroclient-tui --release --no-default-features
# LSL
cargo run -p neuroclient-tui --release --features lsl
```

## Configuration

The TUI needs Emotiv Cortex API credentials. It discovers config in this order (first found wins):

1. **Environment variables**  
   `EMOTIV_CLIENT_ID` and `EMOTIV_CLIENT_SECRET` (required). Optional: `EMOTIV_CORTEX_URL`, `EMOTIV_LICENSE`.

2. **Config file**
   `cortex.toml` in the current directory, or
   `~/.config/neuroclient/cortex.toml`. The legacy
   `~/.config/emotiv-cortex/cortex.toml` path remains a fallback for 0.4.

   ```toml
   client_id = "your-client-id"
   client_secret = "your-client-secret"
   # optional: cortex_url = "wss://localhost:6868"
   ```

Get credentials from the [Emotiv Developer Portal](https://www.emotiv.com/developer/). The [EMOTIV Launcher](https://www.emotiv.com/emotiv-launcher/) must be running for the TUI to connect.

### Cortex URL precedence

The WebSocket URL is resolved from these sources, highest precedence first:

1. `--url` command-line flag (only when explicitly supplied)
2. `EMOTIV_CORTEX_URL` environment variable
3. `cortex_url` in the selected config file
4. Library default (`wss://localhost:6868`)

Omitting `--url` never overrides a URL configured via the environment or a
config file.

## LSL Metadata Schema

When streaming to LSL, the TUI publishes self-documenting stream metadata so
receivers can parse stream structure without Cortex API-specific knowledge.

Each outlet includes channel metadata at:

- `desc/channels/channel/label`
- `desc/channels/channel/unit`
- `desc/channels/channel/type`
- `desc/channels/channel/location_label` (EEG 10-20 label)
- `desc/channels/channel/location/{X,Y,Z}` (EEG coordinates in millimeters)

EEG outlets also include explicit cap/reference metadata:

- `desc/cap/labelscheme = 10-20`
- `desc/reference/scheme = unknown`
- `desc/reference/notes = not provided by Cortex eeg payload`

Stream-level `type` values are:

- `EmotivEEG` -> `EEG`
- `EmotivMotion` -> `MoCap`
- `EmotivBandPower` -> `EEG`
- `EmotivMetrics` -> `Metrics`
- `EmotivMentalCommands` -> `Markers` (numeric command power)
- `EmotivMentalCommandMarkers` -> `Markers` (string action labels)
- `EmotivFacialExpressions` -> `Markers` (numeric upper/lower face powers)
- `EmotivFacialExpressionMarkers` -> `Markers` (string eye/upper/lower action labels)
- `EmotivDeviceQuality` -> `Quality`
- `EmotivEEGQuality` -> `Quality`

The `Emotiv*` outlet names and `manufacturer = Emotiv` metadata are retained
as factual source identifiers and for compatibility with existing LSL
consumers. Project provenance is reported separately as
`application = neuroclient-tui` and `library = neuroclient`.

Mental-command and facial-expression events are published as **paired
outlets**: an irregular-rate string marker outlet carrying the action labels
(e.g. `push`, `smile`, including arbitrary custom action names) and a numeric
outlet carrying the power values. Both samples of one Cortex event share a
single LSL timestamp, and each outlet declares
`desc/pairing/{role,partner_stream}` so consumers can align them without
Cortex-specific knowledge.

Channel `type` naming follows XDF conventions where defined:

- EEG channels use `EEG`
- Motion quaternion channels use `OrientationA/B/C/D`
- Derived/custom channels fall back to `Misc`
- Numeric marker-like channels use `Stim`; string label channels use `Marker`

**Pre-release.** This crate is under active development; behavior and features may change. Treat as pre-release when using it.

**Not affiliated with Emotiv.** NeuroClient is independent and is not created
by, affiliated with, supported by, sponsored by, or endorsed by Emotiv, Inc.
Emotiv and Emotiv Cortex are trademarks of Emotiv, Inc. For official support
and products, see [emotiv.com](https://www.emotiv.com/).

License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.
