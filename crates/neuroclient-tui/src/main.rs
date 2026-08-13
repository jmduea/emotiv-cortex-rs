//! # neuroclient-tui
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

use neuroclient::{CortexClient, CortexConfig};

/// Terminal UI dashboard for the Emotiv Cortex v2 API.
#[derive(Parser)]
#[command(name = "neuroclient-tui", version, about)]
struct Cli {
    /// Path to cortex.toml config file
    #[arg(short, long)]
    config: Option<String>,

    /// Cortex API URL override (takes precedence over the environment
    /// variable and any config file)
    #[arg(long)]
    url: Option<String>,

    /// Enable verbose logging (set `RUST_LOG` for fine-grained control)
    #[arg(short, long)]
    verbose: bool,
}

/// Target frame interval (~30 fps).
const TICK_RATE: Duration = Duration::from_millis(33);

/// Apply an explicit `--url` override on top of a discovered config.
///
/// URL precedence is `--url` > `EMOTIV_CORTEX_URL` > selected config file >
/// library default. [`CortexConfig::discover`] already resolves the last
/// three, so this helper only overrides when the flag was actually supplied
/// on the command line.
fn apply_cli_url_override(config: &mut CortexConfig, cli_url: Option<&str>) {
    if let Some(url) = cli_url {
        config.cortex_url = url.to_string();
    }
}

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
            .with_env_filter("neuroclient=debug,neuroclient_tui=debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("neuroclient=warn")
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

    apply_cli_url_override(&mut config, cli.url.as_deref());

    // ── Connect ──────────────────────────────────────────────────────
    let client = CortexClient::connect(&config).await.map_err(|e| {
        format!(
            "Connection to {} failed ({}).\nMake sure the EMOTIV Launcher is running.",
            config.cortex_url,
            e.category()
        )
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
    spawn_authenticate(Arc::clone(&client), app.config.clone(), tx.clone());

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
                if let Some(Ok(evt)) = maybe_event
                    && app.handle_event(AppEvent::Terminal(evt))
                {
                    break;
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
            let _ = lsl::stop_lsl_streaming(lsl_handle, &app.client, token, session_id).await;
        }
    }

    // Gracefully close the active session so the next run doesn't
    // hit a "headset busy" / stale-session error.
    if let (Some(token), Some(session_id)) = (&app.token, &app.session_id) {
        if let Err(e) = app.client.close_session(token, session_id).await {
            tracing::warn!(
                error_category = e.category(),
                api_code = ?e.api_code(),
                "Failed to close session on exit"
            );
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
            Err(_) => {
                let _ = tx.send(AppEvent::Log(LogEntry::error(
                    "Authentication failed; verify credentials and Launcher approval",
                )));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_url_flag_parses_as_none() {
        let cli = Cli::try_parse_from(["neuroclient-tui"]).unwrap();
        assert_eq!(cli.url, None, "--url must not have an implicit default");
    }

    #[test]
    fn explicit_url_flag_parses_as_some() {
        let cli =
            Cli::try_parse_from(["neuroclient-tui", "--url", "wss://cli.example:6868"]).unwrap();
        assert_eq!(cli.url.as_deref(), Some("wss://cli.example:6868"));
    }

    #[test]
    fn omitted_cli_url_keeps_discovered_url() {
        let mut config = CortexConfig::new("id", "secret");
        config.cortex_url = "wss://from-env-or-file.example:6868".to_string();

        apply_cli_url_override(&mut config, None);

        assert_eq!(config.cortex_url, "wss://from-env-or-file.example:6868");
    }

    #[test]
    fn explicit_cli_url_overrides_discovered_url() {
        let mut config = CortexConfig::new("id", "secret");
        config.cortex_url = "wss://from-env-or-file.example:6868".to_string();

        apply_cli_url_override(&mut config, Some("wss://cli.example:6868"));

        assert_eq!(config.cortex_url, "wss://cli.example:6868");
    }

    /// End-to-end precedence: `--url` > `EMOTIV_CORTEX_URL` > config file >
    /// library default. Env manipulation is confined to this single test so
    /// the binary's tests remain safe to run in parallel.
    #[test]
    #[allow(unsafe_code)] // Test-only; sole test in this binary touching EMOTIV_CORTEX_URL.
    fn url_precedence_cli_env_file_default() {
        let dir = std::env::temp_dir().join(format!(
            "emotiv-tui-url-precedence-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("cortex.toml");
        std::fs::write(
            &config_path,
            "client_id = \"file-id\"\nclient_secret = \"file-secret\"\ncortex_url = \"wss://file.example:6868\"\n",
        )
        .unwrap();

        let saved_env = std::env::var_os("EMOTIV_CORTEX_URL");
        // SAFETY: No other threads are mutating or reading the process
        // environment concurrently; this test owns EMOTIV_CORTEX_URL and
        // restores it before returning.
        unsafe { std::env::remove_var("EMOTIV_CORTEX_URL") };

        // File beats library default when env and CLI are absent.
        let mut config = CortexConfig::discover(Some(config_path.as_path())).unwrap();
        apply_cli_url_override(&mut config, None);
        assert_eq!(config.cortex_url, "wss://file.example:6868");

        // Env beats file.
        // SAFETY: See above — exclusive env access within this test.
        unsafe { std::env::set_var("EMOTIV_CORTEX_URL", "wss://env.example:6868") };
        let mut config = CortexConfig::discover(Some(config_path.as_path())).unwrap();
        apply_cli_url_override(&mut config, None);
        assert_eq!(config.cortex_url, "wss://env.example:6868");

        // Explicit CLI beats env and file.
        let mut config = CortexConfig::discover(Some(config_path.as_path())).unwrap();
        apply_cli_url_override(&mut config, Some("wss://cli.example:6868"));
        assert_eq!(config.cortex_url, "wss://cli.example:6868");

        // Library default applies when nothing else supplies a URL.
        // SAFETY: See above — exclusive env access within this test.
        unsafe { std::env::remove_var("EMOTIV_CORTEX_URL") };
        let default_config = CortexConfig::new("id", "secret");
        let mut config = CortexConfig::new("id", "secret");
        apply_cli_url_override(&mut config, None);
        assert_eq!(config.cortex_url, default_config.cortex_url);

        // SAFETY: Restoring the variable this test saved at entry.
        unsafe {
            match saved_env {
                Some(value) => std::env::set_var("EMOTIV_CORTEX_URL", value),
                None => std::env::remove_var("EMOTIV_CORTEX_URL"),
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
