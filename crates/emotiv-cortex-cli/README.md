# emotiv-cortex-cli

Interactive CLI explorer for the Emotiv Cortex v2 API.

[![CI](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)
[Coverage Reports](https://github.com/jmduea/emotiv-cortex-rs/actions/workflows/ci.yml)

The CLI supports authentication, headset/session management, stream inspection,
records/markers, profiles, and BCI training workflows.

## Install

Install from crates.io:

```bash
cargo install emotiv-cortex-cli
```

Install from this repository checkout:

```bash
./scripts/install-emotiv-cortex-cli.sh
```

## Optional LSL Support

LSL is disabled by default. Install with LSL support:

```bash
cargo install emotiv-cortex-cli --features lsl
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

Compatibility note: if your resolver/filter logic expected the previous
`EmotivMetrics` empty stream type or `Emotiv*Quality` streams as `EEG`, update
those queries.

## Usage

```bash
emotiv-cortex-cli --help
emotiv-cortex-cli --verbose
```

You can provide credentials through:

- Environment: `EMOTIV_CLIENT_ID`, `EMOTIV_CLIENT_SECRET`
- Config file: `cortex.toml`
