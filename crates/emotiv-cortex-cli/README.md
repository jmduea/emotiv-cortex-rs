# emotiv-cortex-cli

Interactive CLI explorer for the Emotiv Cortex v2 API.

The CLI supports authentication, headset/session management, stream inspection,
records/markers, profiles, and BCI training workflows.

## Install

```bash
cargo install emotiv-cortex-cli
```

## Optional LSL Support

LSL is disabled by default. Install with LSL support:

```bash
cargo install emotiv-cortex-cli --features lsl
```

LSL support is currently available on Windows and macOS.
Linux is currently unsupported for `--features lsl`.

## LSL Metadata Schema

When streaming to LSL, the CLI publishes self-documenting stream metadata so
receivers can parse stream structure without Cortex API-specific knowledge.

Each outlet includes channel metadata at:

- `desc/channels/channel/label`
- `desc/channels/channel/unit`
- `desc/channels/channel/type`

EEG outlets also include per-channel `location` (10-20 label) and explicit
reference metadata:

- `desc/reference/scheme = unknown`
- `desc/reference/notes = not provided by Cortex eeg payload`

Stream-level `type` values are:

- `EmotivEEG` -> `EEG`
- `EmotivMotion` -> `MoCap`
- `EmotivBandPower` -> `EEG`
- `EmotivMetrics` -> `""` (empty)
- `EmotivMentalCommands` -> `Markers`
- `EmotivFacialExpressions` -> `Markers`
- `EmotivDeviceQuality` -> `EEG`
- `EmotivEEGQuality` -> `EEG`

Compatibility note: if your resolver/filter logic matched previous type values
like `Motion`, `FFT`, `Metrics`, or `Quality`, update those queries.

## Usage

```bash
emotiv-cortex-cli --help
emotiv-cortex-cli --verbose
```

You can provide credentials through:

- Environment: `EMOTIV_CLIENT_ID`, `EMOTIV_CLIENT_SECRET`
- Config file: `cortex.toml`
