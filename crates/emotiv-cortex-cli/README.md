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

On Linux, install `liblsl-dev` first.

## Usage

```bash
emotiv-cortex-cli --help
emotiv-cortex-cli --verbose
```

You can provide credentials through:

- Environment: `EMOTIV_CLIENT_ID`, `EMOTIV_CLIENT_SECRET`
- Config file: `cortex.toml`

