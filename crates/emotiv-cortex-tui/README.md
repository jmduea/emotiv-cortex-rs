# emotiv-cortex-tui

Terminal UI dashboard for the Emotiv Cortex v2 API.

[![CI](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)

**Pre-release.** This crate is under active development; behavior and features may change. Treat as pre-release when using it.

**Not affiliated with Emotiv.** This crate is independent, community-maintained, and is **not** created by, affiliated with, supported by, sponsored by, or endorsed by Emotiv, Inc. For official support and products, see [emotiv.com](https://www.emotiv.com/).

A full-screen ratatui TUI that auto-connects to the first available EMOTIV
headset and displays real-time data:

- **Dashboard** — session info, performance metric gauges, mental command /
  facial expression readouts
- **Streams** — live EEG sparklines, motion/IMU line charts, band-power
  breakdowns (cycle views with `v`)
- **LSL** — optional Lab Streaming Layer forwarding with per-stream sample
  counts (toggle with `l`, requires `--features lsl`)
- **Device** — full headset metadata and per-channel contact quality gauges
- **Log** — scrollable timestamped event log

## Install

Install from crates.io:

```bash
cargo install emotiv-cortex-tui
```

Install from this repository checkout:

```bash
# bash / macOS / WSL
./scripts/install-emotiv-cortex-tui.sh

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-tui.ps1
```

## Optional LSL Support

LSL is disabled by default. Install with LSL support:

```bash
# cargo directly
cargo install emotiv-cortex-tui --features lsl

# bash / macOS 
./scripts/install-emotiv-cortex-tui.sh --lsl

# PowerShell (Windows)
.\scripts\install-emotiv-cortex-tui.ps1 -Lsl
```

LSL support is currently available on Windows and macOS.
Linux is currently unsupported for `--features lsl`.

After installation, ensure your cargo bin directory is in `PATH`:

```bash
# bash/zsh
export PATH="$HOME/.cargo/bin:$PATH"
```

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

## Usage

```bash
emotiv-cortex-tui --help
emotiv-cortex-tui --verbose
```

You can provide credentials through:

- Environment: `EMOTIV_CLIENT_ID`, `EMOTIV_CLIENT_SECRET`
- Config file: `cortex.toml`
