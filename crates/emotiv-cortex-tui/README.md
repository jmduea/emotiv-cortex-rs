# emotiv-cortex-tui

<img width="1840" height="613" alt="tui_device_panel" src="https://github.com/user-attachments/assets/188bdb44-b059-4477-8c48-7f648c06b4d6" />

Terminal UI dashboard for the Emotiv Cortex v2 API.

[![CI](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)

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

**Windows:** Download the binary from [GitHub Releases](https://github.com/jmduea/emotiv-cortex-rs/releases):

- Baseline (no LSL)
  - `emotiv-cortex-tui-x86_64-pc-windows-msvc.exe` 

- [Lab Streaming Layer](https://github.com/sccn/labstreaminglayer) support for streaming to other apps
  - `emotiv-cortex-tui-x86_64-pc-windows-msvc-lsl.exe`
 

**From source** (any platform):

Clone this repo then:

```bash
# bash / macOS / WSL
./scripts/install-emotiv-cortex-tui.sh

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-tui.ps1

# LSL versions
# LSL support is currently available on Windows and macOS.
# Linux is currently unsupported for `--features lsl`.
./scripts/install-emotiv-cortex-tui.sh --lsl   # bash / macOS
.\scripts\install-emotiv-cortex-tui.ps1 -Lsl   # PowerShell (Windows)
```
Or run with Cargo:

```bash
# No LSL
cargo run -p emotiv-cortex-tui --release --no-default-features
# LSL
cargo run -p emotiv-cortex-tui --release --features lsl
```

## Configuration

The TUI needs Emotiv Cortex API credentials. It discovers config in this order (first found wins):

1. **Environment variables**  
   `EMOTIV_CLIENT_ID` and `EMOTIV_CLIENT_SECRET` (required). Optional: `EMOTIV_CORTEX_URL`, `EMOTIV_LICENSE`.

2. **Config file**  
   `cortex.toml` in the current directory, or `~/.config/emotiv-cortex/cortex.toml`:

   ```toml
   client_id = "your-client-id"
   client_secret = "your-client-secret"
   # optional: cortex_url = "wss://localhost:6868"
   ```

Get credentials from the [Emotiv Developer Portal](https://www.emotiv.com/developer/). The [EMOTIV Launcher](https://www.emotiv.com/emotiv-launcher/) must be running for the TUI to connect.

## LSL Metadata Schema

When streaming to LSL, the CLI publishes self-documenting stream metadata so
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
- `EmotivMentalCommands` -> `Markers`
- `EmotivFacialExpressions` -> `Markers`
- `EmotivDeviceQuality` -> `Quality`
- `EmotivEEGQuality` -> `Quality`

Channel `type` naming follows XDF conventions where defined:

- EEG channels use `EEG`
- Motion quaternion channels use `OrientationA/B/C/D`
- Derived/custom channels fall back to `Misc`
- Marker-like channels use `Stim`

**Pre-release.** This crate is under active development; behavior and features may change. Treat as pre-release when using it.

**Not affiliated with Emotiv.** This crate is independent, community-maintained, and is **not** created by, affiliated with, supported by, sponsored by, or endorsed by Emotiv, Inc. For official support and products, see [emotiv.com](https://www.emotiv.com/)

License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.
