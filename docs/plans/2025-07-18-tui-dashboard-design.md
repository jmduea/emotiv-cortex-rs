# TUI Dashboard Design — emotiv-cortex-cli

**Date:** 2025-07-18
**Status:** Implemented

## Overview

Convert the `emotiv-cortex-cli` from a dialoguer-based interactive menu system
to a full-screen ratatui terminal UI dashboard focused on real-time device
monitoring and stream visualisation for a single EMOTIV headset.

## Goals

- **Primary:** Real-time dashboard with live data visualisation (EEG sparklines,
  motion charts, band power, performance metrics)
- **Device info:** Summary status bar (battery, signal, headset ID) always
  visible; full detail panel in a dedicated tab
- **Single headset:** The first discovered headset auto-connects on startup
- **Auth flow:** Automatic on startup — no manual authentication step
- **LSL integration:** Embedded in the TUI as a tab showing status and sample
  counts (when built with `--features lsl`)

## Approach

**ratatui 0.29 + crossterm 0.28** with an async-native event loop.

- crossterm's `EventStream` (requires `event-stream` feature) runs inside a
  `tokio::select!` alongside data stream channels
- Single `AppEvent` enum consumed by the main loop
- Ring buffers (`VecDeque`, capacity 256) for chart data
- ~30 fps tick rate via `tokio::time::interval(33ms)`

## Layout

```
┌──────────────────── Status Bar ────────────────────┐  1 line
│ EMOTIV Cortex │ Headset: INSIGHT-X │ ██ 85% │ ████ │
├──────────────────── Tab Bar ───────────────────────┤  3 lines
│  1:Dashboard │ 2:Streams │ 3:LSL │ 4:Device │ 5:Log│
├──────────────── Content Area ──────────────────────┤  fill
│                                                     │
│  (Active tab content rendered here)                 │
│                                                     │
├──────────────────── Key Help ──────────────────────┤  1 line
│ q Quit  Tab Switch  1-5 Jump  ↑↓ Scroll  ? Help    │
└─────────────────────────────────────────────────────┘
```

## Tabs

| Tab       | Content                                                |
|-----------|--------------------------------------------------------|
| Dashboard | Session info (left 40%) + performance metric gauges (right 60%) |
| Streams   | Live sparklines (EEG), line charts (motion), band power display — cycle with `v` |
| LSL       | LSL status, uptime, outlet list, per-stream sample counts (feature-gated) |
| Device    | Full HeadsetInfo fields + per-channel contact quality gauges |
| Log       | Scrollable event log with timestamps and severity badges |

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  main.rs                        │
│  TUI enter → async event loop → TUI exit        │
│  tokio::select! {                               │
│    EventStream (keyboard) → AppEvent::Terminal   │
│    tick interval           → AppEvent::Tick      │
│    mpsc::Receiver          → data/lifecycle      │
│  }                                              │
└──────────────┬──────────────────────────────────┘
               │ AppEvent
┌──────────────▼──────────────────────────────────┐
│               app.rs: App                       │
│  handle_event() → update ring buffers / state   │
│  handle_key()   → tab nav, quit, help toggle    │
└──────────────┬──────────────────────────────────┘
               │ &App
┌──────────────▼──────────────────────────────────┐
│         ui/mod.rs: draw(frame, &app)            │
│  → status_bar → tabs → active tab → key help   │
│  → help overlay (if toggled)                    │
└─────────────────────────────────────────────────┘
               ↑ data via mpsc
┌─────────────────────────────────────────────────┐
│           bridge.rs                             │
│  auto_connect()            → auth, headset, session              │
│  subscribe_default_streams → spawn per-stream   │
│    forwarding tasks via tokio::spawn            │
└─────────────────────────────────────────────────┘
```

## Key Bindings

| Key         | Action                              |
|-------------|-------------------------------------|
| `q`         | Quit                                |
| `Ctrl+C`    | Quit                                |
| `Tab`       | Next tab                            |
| `Shift+Tab` | Previous tab                        |
| `1`–`5`     | Jump to tab by number               |
| `↑`/`k`     | Scroll up                           |
| `↓`/`j`     | Scroll down                         |
| `v`         | Cycle stream view (Streams tab)     |
| `?`         | Toggle help overlay                 |

## Dependencies Changed

| Added            | Removed     |
|------------------|-------------|
| ratatui 0.29     | dialoguer   |
| crossterm 0.28   | colored     |

## Files

| File                  | Purpose                                    |
|-----------------------|--------------------------------------------|
| `src/main.rs`         | Entry point, async event loop              |
| `src/app.rs`          | App state, event/key handling              |
| `src/event.rs`        | AppEvent enum, LogEntry                    |
| `src/tui.rs`          | Terminal setup/teardown (Drop safety)      |
| `src/bridge.rs`       | Async bridge: auto-connect + stream tasks  |
| `src/lsl.rs`          | LSL streaming (unchanged, colored removed) |
| `src/ui/mod.rs`       | Top-level layout composition               |
| `src/ui/status_bar.rs`| Always-visible status line                 |
| `src/ui/tabs.rs`      | Tab bar widget                             |
| `src/ui/dashboard.rs` | Dashboard tab (metrics gauges)             |
| `src/ui/streams.rs`   | Stream visualisation (EEG/motion/power)    |
| `src/ui/device.rs`    | Full headset detail + contact quality      |
| `src/ui/log.rs`       | Scrollable log panel                       |
| `src/ui/lsl.rs`       | LSL status tab (feature-gated)             |
| `src/ui/help.rs`      | Help overlay                               |
| `src/commands/`       | **Removed** — replaced by TUI              |
