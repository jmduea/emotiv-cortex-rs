//! # emotiv-cortex-cli
//!
//! Terminal UI dashboard for the Emotiv Cortex v2 API.
//!
//! Displays real-time device status, EEG/motion/band-power stream
//! visualisations, performance metrics, and optional LSL forwarding
//! in a full-screen ratatui interface.

#[cfg(all(feature = "lsl", target_os = "linux"))]
compile_error!(
    "The `lsl` feature is currently unsupported on Linux due to upstream `lsl-sys` \
build incompatibilities. Build without `--features lsl`, or use Windows/macOS for LSL."
);

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use tokio::sync::mpsc;

mod app;
mod bridge;
mod event;
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
mod lsl;
mod tui;
mod ui;

use app::App;
use event::{AppEvent, LogEntry};

use emotiv_cortex_v2::{CortexClient, CortexConfig};

/// Terminal UI dashboard for the Emotiv Cortex v2 API.
#[derive(Parser)]
#[command(name = "emotiv-cortex-cli", version, about)]
struct Cli {
    /// Path to cortex.toml config file
    #[arg(short, long)]
    config: Option<String>,

    /// Cortex API URL override
    #[arg(long, default_value = "wss://localhost:6868")]
    url: Option<String>,

    /// Enable verbose logging (set `RUST_LOG` for fine-grained control)
    #[arg(short, long)]
    verbose: bool,
}

/// Target frame interval (~30 fps).
const TICK_RATE: Duration = Duration::from_millis(33);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // ── Tracing ──────────────────────────────────────────────────────
    // When the TUI is active we only want tracing going to a file or
    // the log panel, not stdout.  For now we just silence console
    // output unless --verbose is given (which is mainly useful when
    // the TUI is not yet fully initialised).
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("emotiv_cortex_v2=debug,emotiv_cortex_cli=debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("emotiv_cortex_v2=warn")
            .init();
    }

    // ── Config ───────────────────────────────────────────────────────
    let mut config =
        CortexConfig::discover(cli.config.as_deref().map(Path::new)).unwrap_or_else(|_| {
            eprintln!(
                "Note: No config file found. Set EMOTIV_CLIENT_ID / \
                 EMOTIV_CLIENT_SECRET env vars, or create a cortex.toml file."
            );
            CortexConfig::new("", "")
        });

    if let Some(url) = &cli.url {
        config.cortex_url = url.clone();
    }

    // ── Connect ──────────────────────────────────────────────────────
    let client = CortexClient::connect(&config).await.map_err(|e| {
        format!("Connection to {} failed: {e}\nMake sure the EMOTIV Launcher is running.", config.cortex_url)
    })?;

    // ── App state ────────────────────────────────────────────────────
    let client = Arc::new(client);

    // ── Event channel ────────────────────────────────────────────────
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    // ── Shutdown broadcast ───────────────────────────────────────────
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

    let mut app = App::new(Arc::clone(&client), config, tx.clone(), shutdown_tx.clone());

    // ── Enter TUI ────────────────────────────────────────────────────
    let mut tui = tui::Tui::enter()?;

    // ── Spawn authenticate + discover background task ────────────────
    spawn_authenticate(
        Arc::clone(&client),
        app.config.clone(),
        tx.clone(),
    );

    // ── Main event loop ──────────────────────────────────────────────
    let mut terminal_events = EventStream::new();
    let mut tick = tokio::time::interval(TICK_RATE);

    loop {
        // Draw
        tui.terminal.draw(|frame| ui::draw(frame, &app))?;

        // Wait for next event
        tokio::select! {
            // Terminal input (keyboard/mouse/resize)
            maybe_event = terminal_events.next() => {
                if let Some(Ok(evt)) = maybe_event {
                    if app.handle_event(AppEvent::Terminal(evt)) {
                        break;
                    }
                }
            }
            // Tick timer
            _ = tick.tick() => {
                if app.handle_event(AppEvent::Tick) {
                    break;
                }
            }
            // Data / lifecycle events from background tasks
            Some(event) = rx.recv() => {
                if app.handle_event(event) {
                    break;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // ── Shutdown ─────────────────────────────────────────────────────
    let _ = shutdown_tx.send(());

    // Gracefully stop LSL streaming if active
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    if let Some(lsl_handle) = app.lsl_streaming.take() {
        if let (Some(token), Some(session_id)) = (&app.token, &app.session_id) {
            let _ =
                lsl::stop_lsl_streaming(lsl_handle, &app.client, token, session_id)
                    .await;
        }
    }

    // Gracefully close the active session so the next run doesn't
    // hit a "headset busy" / stale-session error.
    if let (Some(token), Some(session_id)) = (&app.token, &app.session_id) {
        if let Err(e) = app.client.close_session(token, session_id).await {
            tracing::warn!("Failed to close session on exit: {e}");
        }
        if let Some(hid) = &app.headset_id {
            let _ = app.client.disconnect_headset(hid).await;
        }
    }

    // Tui::drop restores the terminal automatically.
    drop(tui);

    Ok(())
}

/// Spawns the background authenticate + discover task.
///
/// Does NOT connect to any headset — the user selects one from the
/// Device tab and presses Enter.
fn spawn_authenticate(
    client: Arc<CortexClient>,
    config: CortexConfig,
    tx: mpsc::UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        match bridge::authenticate_and_discover(&client, &config, &tx).await {
            Ok(result) => {
                let _ = tx.send(AppEvent::AuthReady {
                    token: result.token,
                });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                    "Authentication failed: {e}"
                ))));
            }
        }
    });
}
