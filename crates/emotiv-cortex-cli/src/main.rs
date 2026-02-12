//! # emotiv-cortex-cli
//!
//! Interactive CLI explorer for the Emotiv Cortex v2 API.
//! Covers authentication, headset management, data streaming,
//! recording, markers, profiles, and BCI training.

use std::path::Path;

use clap::Parser;
use colored::Colorize;
use dialoguer::{Input, Select};
use futures::StreamExt;

#[cfg(feature = "lsl")]
mod lsl;

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::{
    DetectionType, ExportFormat, HeadsetInfo, ProfileAction, Streams, TrainingStatus,
};
use emotiv_cortex_v2::{streams, CortexClient, CortexConfig, CortexResult};

/// Interactive CLI explorer for the Emotiv Cortex v2 API.
#[derive(Parser)]
#[command(name = "emotiv-cortex-cli", version, about)]
struct Cli {
    /// Path to cortex.toml config file
    #[arg(short, long)]
    config: Option<String>,

    /// Cortex API URL override
    #[arg(long, default_value = "wss://localhost:6868")]
    url: Option<String>,

    /// Enable verbose logging (set RUST_LOG for fine-grained control)
    #[arg(short, long)]
    verbose: bool,

    /// Quickstart: authenticate, connect to first INSIGHT headset, create
    /// session, and start LSL streaming with default streams (EEG) — then
    /// drop into the interactive menu.
    #[cfg(feature = "lsl")]
    #[arg(long, alias = "quick")]
    quickstart: bool,
}

/// Shared session state passed between menu actions.
struct SessionState {
    client: CortexClient,
    config: CortexConfig,
    token: Option<String>,
    session_id: Option<String>,
    headset_id: Option<String>,
    /// Active background LSL streaming handle, if any.
    #[cfg(feature = "lsl")]
    lsl_streaming: Option<lsl::LslStreamingHandle>,
}

impl SessionState {
    fn token(&self) -> CortexResult<&str> {
        self.token
            .as_deref()
            .ok_or(emotiv_cortex_v2::CortexError::ProtocolError {
                reason: "Not authenticated. Run 'Authenticate' first.".into(),
            })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("emotiv_cortex_v2=debug,emotiv_cortex_cli=debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("emotiv_cortex_v2=warn")
            .init();
    }

    // Load config
    let mut config = match CortexConfig::discover(cli.config.as_deref().map(Path::new)) {
        Ok(c) => c,
        Err(_) => {
            println!("{} No config file found. Using defaults.", "Note:".yellow());
            println!(
                "  Set {} and {} env vars, or create a cortex.toml file.\n",
                "EMOTIV_CLIENT_ID".cyan(),
                "EMOTIV_CLIENT_SECRET".cyan()
            );
            CortexConfig::new("", "")
        }
    };

    if let Some(url) = &cli.url {
        config.cortex_url = url.clone();
    }

    println!(
        "{} Emotiv Cortex CLI Explorer",
        "╔══════════════════════════════════╗\n║".bright_blue()
    );
    println!("{}", "╚══════════════════════════════════╝".bright_blue());
    println!("Connecting to {}...\n", config.cortex_url.cyan());

    let client = match CortexClient::connect(&config).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {}", "Connection failed:".red(), e);
            eprintln!("Make sure the EMOTIV Launcher is running.");
            return Ok(());
        }
    };

    println!("{}", "Connected!".green());

    let mut state = SessionState {
        client,
        config,
        token: None,
        session_id: None,
        headset_id: None,
        #[cfg(feature = "lsl")]
        lsl_streaming: None,
    };

    // ── Quickstart mode ─────────────────────────────────────────────
    #[cfg(feature = "lsl")]
    if cli.quickstart {
        if let Err(e) = quickstart_lsl(&mut state).await {
            eprintln!("{} {}", "Quickstart failed:".red(), e);
            eprintln!("Falling back to interactive mode.");
        }
    }

    loop {
        println!();
        let status = format_status(&state);
        println!("{}", status);

        let mut items = vec![
            "Cortex Info".to_string(),
            "Authentication".to_string(),
            "Headsets".to_string(),
            "Sessions".to_string(),
            "Stream Data".to_string(),
            "Records & Markers".to_string(),
            "Subjects".to_string(),
            "Profiles".to_string(),
            "BCI Training".to_string(),
        ];
        #[cfg(feature = "lsl")]
        {
            let lsl_label = if state.lsl_streaming.is_some() {
                "Stream to LSL (active \u{25b6})".to_string()
            } else {
                "Stream to LSL".to_string()
            };
            items.push(lsl_label);
        }
        items.push("Quit".to_string());

        let selection = Select::new()
            .with_prompt("Select an action")
            .items(&items)
            .default(0)
            .interact_opt()?;

        let quit_index = items.len() - 1;

        match selection {
            Some(0) => cmd_cortex_info(&state).await,
            Some(1) => cmd_authentication(&mut state).await,
            Some(2) => cmd_headsets(&mut state).await,
            Some(3) => cmd_sessions(&mut state).await,
            Some(4) => cmd_stream_data(&mut state).await,
            Some(5) => cmd_records(&mut state).await,
            Some(6) => cmd_subjects(&mut state).await,
            Some(7) => cmd_profiles(&mut state).await,
            Some(8) => cmd_training(&mut state).await,
            #[cfg(feature = "lsl")]
            Some(9) => cmd_stream_lsl(&mut state).await,
            Some(i) if i == quit_index => {
                graceful_shutdown(&mut state).await;
                break;
            }
            None => {
                graceful_shutdown(&mut state).await;
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

fn format_status(state: &SessionState) -> String {
    let auth = if state.token.is_some() {
        "authenticated".green().to_string()
    } else {
        "not authenticated".yellow().to_string()
    };

    let headset = state
        .headset_id
        .as_deref()
        .map(|h| h.cyan().to_string())
        .unwrap_or_else(|| "none".dimmed().to_string());

    let session = state
        .session_id
        .as_deref()
        .map(|s| s[..12.min(s.len())].cyan().to_string())
        .unwrap_or_else(|| "none".dimmed().to_string());

    #[allow(unused_mut)]
    let mut status = format!(
        "Auth: {} | Headset: {} | Session: {}",
        auth, headset, session
    );

    #[cfg(feature = "lsl")]
    if let Some(ref handle) = state.lsl_streaming {
        status.push_str(&format!(" | {}", handle.format_status().green()));
    }

    status
}

// ─── Graceful Shutdown ──────────────────────────────────────────────────

async fn graceful_shutdown(state: &mut SessionState) {
    println!("Disconnecting...");

    // 0. Stop LSL streaming if active.
    #[cfg(feature = "lsl")]
    if let Some(handle) = state.lsl_streaming.take() {
        let token = state.token.as_deref().unwrap_or_default();
        let session_id = state.session_id.as_deref().unwrap_or_default();
        if let Err(e) = lsl::stop_lsl_streaming(handle, &state.client, token, session_id).await {
            eprintln!("{} {}", "Warning: LSL shutdown error:".yellow(), e);
        }
    }

    // 1. Close the active session (if any).
    if let (Some(token), Some(session_id)) = (state.token.as_deref(), state.session_id.take()) {
        print!("  Closing session {}... ", session_id.cyan());
        match state.client.close_session(token, &session_id).await {
            Ok(()) => println!("{}", "ok".green()),
            Err(e) => println!("{} {}", "failed:".yellow(), e),
        }
    }

    // 2. Disconnect the headset (if any).
    if let Some(headset_id) = state.headset_id.take() {
        print!("  Disconnecting headset {}... ", headset_id.cyan());
        match state.client.disconnect_headset(&headset_id).await {
            Ok(()) => println!("{}", "ok".green()),
            Err(e) => println!("{} {}", "failed:".yellow(), e),
        }
    }

    // 3. Close the WebSocket connection.
    let _ = state.client.disconnect().await;

    println!("{}", "Goodbye!".green());
}

// ─── Cortex Info ────────────────────────────────────────────────────────

async fn cmd_cortex_info(state: &SessionState) {
    match state.client.get_cortex_info().await {
        Ok(info) => {
            println!("\n{}", "Cortex Service Info:".bright_blue());
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

// ─── Authentication ─────────────────────────────────────────────────────

async fn cmd_authentication(state: &mut SessionState) {
    let items = vec!["Authenticate", "User Info", "License Info", "Back"];

    let sel = Select::new()
        .with_prompt("Authentication action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    match sel {
        Some(0) => {
            let mut client_id = state.config.client_id.clone();
            let mut client_secret = state.config.client_secret.clone();

            if client_id.is_empty() {
                client_id = Input::new()
                    .with_prompt("Client ID")
                    .interact_text()
                    .unwrap_or_default();
            }
            if client_secret.is_empty() {
                client_secret = Input::new()
                    .with_prompt("Client Secret")
                    .interact_text()
                    .unwrap_or_default();
            }

            println!("Authenticating...");
            match state.client.authenticate(&client_id, &client_secret).await {
                Ok(token) => {
                    println!(
                        "{} Token: {}...",
                        "Authenticated!".green(),
                        &token[..20.min(token.len())]
                    );
                    state.token = Some(token);
                    state.config.client_id = client_id;
                    state.config.client_secret = client_secret;
                }
                Err(e) => eprintln!("{} {}", "Authentication failed:".red(), e),
            }
        }
        Some(1) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            match state.client.get_user_info(token).await {
                Ok(info) => {
                    println!("\n{}", "User Info:".bright_blue());
                    println!("{}", serde_json::to_string_pretty(&info).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(2) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            match state.client.get_license_info(token).await {
                Ok(info) => {
                    println!("\n{}", "License Info:".bright_blue());
                    println!("{}", serde_json::to_string_pretty(&info).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}

// ─── Headsets ───────────────────────────────────────────────────────────

async fn cmd_headsets(state: &mut SessionState) {
    let items = vec![
        "Query Headsets",
        "Refresh (Bluetooth Scan)",
        "Connect Headset",
        "Disconnect Headset",
        "Update Headset Settings",
        "Update Custom Info",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Headset action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    match sel {
        Some(0) => match state.client.query_headsets().await {
            Ok(headsets) => {
                if headsets.is_empty() {
                    println!("No headsets found.");
                } else {
                    print_headsets(&headsets);
                }
            }
            Err(e) => eprintln!("{} {}", "Error:".red(), e),
        },
        Some(1) => match state.client.refresh_headsets().await {
            Ok(()) => {
                println!(
                    "{}",
                    "Bluetooth scan triggered. Wait a few seconds then query.".green()
                );
            }
            Err(e) => eprintln!("{} {}", "Error:".red(), e),
        },
        Some(2) => {
            let id: String = Input::new()
                .with_prompt("Headset ID to connect")
                .default(state.headset_id.clone().unwrap_or_default())
                .interact_text()
                .unwrap_or_default();

            if !id.is_empty() {
                match state.client.connect_headset(&id).await {
                    Ok(()) => {
                        println!("{} Connection initiated for {}", "OK".green(), id.cyan());
                        state.headset_id = Some(id);
                    }
                    Err(e) => eprintln!("{} {}", "Error:".red(), e),
                }
            }
        }
        Some(3) => {
            if let Some(id) = &state.headset_id {
                match state.client.disconnect_headset(id).await {
                    Ok(()) => {
                        println!("{} Disconnected {}", "OK".green(), id.cyan());
                        state.headset_id = None;
                    }
                    Err(e) => eprintln!("{} {}", "Error:".red(), e),
                }
            } else {
                println!("No headset selected.");
            }
        }
        Some(4) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            let id = state.headset_id.clone().unwrap_or_default();
            if id.is_empty() {
                eprintln!("{}", "Select a headset first.".yellow());
                return;
            }
            let setting_json: String = Input::new()
                .with_prompt("Settings JSON (e.g. {\"mode\":\"EPOC\",\"eegRate\":256})")
                .interact_text()
                .unwrap_or_default();
            match serde_json::from_str::<serde_json::Value>(&setting_json) {
                Ok(setting) => match state.client.update_headset(token, &id, setting).await {
                    Ok(result) => {
                        println!("{}", "Headset settings updated:".green());
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    }
                    Err(e) => eprintln!("{} {}", "Error:".red(), e),
                },
                Err(e) => eprintln!("{} Invalid JSON: {}", "Error:".red(), e),
            }
        }
        Some(5) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            let id = state.headset_id.clone().unwrap_or_default();
            if id.is_empty() {
                eprintln!("{}", "Select a headset first.".yellow());
                return;
            }
            let pos: String = Input::new()
                .with_prompt("Headband position (empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let name: String = Input::new()
                .with_prompt("Custom name (empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let pos_opt = if pos.is_empty() {
                None
            } else {
                Some(pos.as_str())
            };
            let name_opt = if name.is_empty() {
                None
            } else {
                Some(name.as_str())
            };
            match state
                .client
                .update_headset_custom_info(token, &id, pos_opt, name_opt)
                .await
            {
                Ok(result) => {
                    println!("{}", "Custom info updated:".green());
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}

fn print_headsets(headsets: &[HeadsetInfo]) {
    println!("\n{} headset(s) found:", headsets.len());
    for h in headsets {
        let model = HeadsetModel::from_headset_info(h);
        let status_colored = match h.status.as_str() {
            "connected" => h.status.green().to_string(),
            "discovered" => h.status.yellow().to_string(),
            _ => h.status.dimmed().to_string(),
        };
        println!(
            "  {} ({}) — {} ({} ch, {} Hz)",
            h.id.cyan(),
            status_colored,
            model,
            model.num_channels(),
            model.sampling_rate_hz()
        );
    }
}

// ─── Sessions ───────────────────────────────────────────────────────────

async fn cmd_sessions(state: &mut SessionState) {
    let items = vec!["Create Session", "Query Sessions", "Close Session", "Back"];

    let sel = Select::new()
        .with_prompt("Session action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    match sel {
        Some(0) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            let headset_id = state.headset_id.clone().unwrap_or_default();
            if headset_id.is_empty() {
                eprintln!("{}", "Select a headset first.".yellow());
                return;
            }
            match state.client.create_session(token, &headset_id).await {
                Ok(session) => {
                    println!("{} Session: {}", "Created!".green(), session.id.cyan());
                    state.session_id = Some(session.id);
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(1) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            match state.client.query_sessions(token).await {
                Ok(sessions) => {
                    println!("{} session(s):", sessions.len());
                    for s in &sessions {
                        println!("  {} ({})", s.id.cyan(), s.status);
                    }
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(2) => {
            let Ok(token) = state.token() else {
                eprintln!("{}", "Authenticate first.".yellow());
                return;
            };
            if let Some(sid) = state.session_id.clone() {
                match state.client.close_session(token, &sid).await {
                    Ok(()) => {
                        println!("{} Session closed", "OK".green());
                        state.session_id = None;
                    }
                    Err(e) => eprintln!("{} {}", "Error:".red(), e),
                }
            } else {
                println!("No active session.");
            }
        }
        _ => {}
    }
}

// ─── Stream Data ────────────────────────────────────────────────────────

async fn cmd_stream_data(state: &mut SessionState) {
    let Ok(token) = state.token().map(|t| t.to_string()) else {
        eprintln!("{}", "Authenticate first.".yellow());
        return;
    };
    let Some(session_id) = state.session_id.clone() else {
        eprintln!("{}", "Create a session first.".yellow());
        return;
    };

    let model = state
        .headset_id
        .as_deref()
        .map(HeadsetModel::from_headset_id)
        .unwrap_or(HeadsetModel::Insight);
    let num_ch = model.num_channels();

    let items = vec![
        "EEG (raw data)",
        "Device Quality",
        "Motion / IMU",
        "Band Power",
        "Performance Metrics",
        "Mental Commands",
        "Facial Expressions",
        "EEG Quality",
        "System Events",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Select stream")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    // Back or cancel — return before asking for max samples
    match sel {
        Some(9) | None => return,
        _ => {}
    }

    let max_samples: u64 = Input::new()
        .with_prompt("Max samples (0 = unlimited)")
        .default(50)
        .interact_text()
        .unwrap_or(50);

    match sel {
        Some(0) => stream_eeg(state, &token, &session_id, num_ch, max_samples).await,
        Some(1) => stream_dev(state, &token, &session_id, num_ch, max_samples).await,
        Some(2) => stream_motion(state, &token, &session_id, max_samples).await,
        Some(3) => stream_band_power(state, &token, &session_id, num_ch, max_samples).await,
        Some(4) => stream_metrics(state, &token, &session_id, max_samples).await,
        Some(5) => stream_mental_commands(state, &token, &session_id, max_samples).await,
        Some(6) => stream_facial(state, &token, &session_id, max_samples).await,
        Some(7) => stream_eq(state, &token, &session_id, num_ch, max_samples).await,
        Some(8) => stream_sys(state, &token, &session_id, max_samples).await,
        _ => {}
    }
}

/// Run a streaming loop, breaking on max samples or Ctrl+C.
async fn run_stream_loop<T>(
    stream: &mut (impl futures::Stream<Item = T> + Unpin),
    max: u64,
    mut on_item: impl FnMut(&T),
) {
    let mut count = 0u64;
    loop {
        tokio::select! {
            item = stream.next() => {
                let Some(data) = item else { break };
                on_item(&data);
                count += 1;
                if max > 0 && count >= max { break; }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n{}", "Streaming stopped.".yellow());
                break;
            }
        }
    }
}

async fn stream_eeg(state: &SessionState, token: &str, session_id: &str, num_ch: usize, max: u64) {
    match streams::subscribe_eeg(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!("{}", "Streaming EEG data... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |eeg_data| {
                let vals: Vec<String> = eeg_data
                    .channels
                    .iter()
                    .map(|v| format!("{:.2}", v))
                    .collect();
                println!("[sample {}] {}", eeg_data.counter, vals.join(" | "));
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::EEG]).await;
}

async fn stream_dev(state: &SessionState, token: &str, session_id: &str, num_ch: usize, max: u64) {
    match streams::subscribe_dev(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!("{}", "Streaming device quality... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |dq| {
                let quals: Vec<String> = dq
                    .channel_quality
                    .iter()
                    .map(|q| format!("{:.1}", q))
                    .collect();
                println!(
                    "Battery: {}% | Signal: {:.1} | CQ: [{}] | Overall: {:.1}",
                    dq.battery_percent,
                    dq.signal_strength,
                    quals.join(", "),
                    dq.overall_quality
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::DEV]).await;
}

async fn stream_motion(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_motion(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!("{}", "Streaming motion data... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |m| {
                println!(
                    "Accel: ({:.3}, {:.3}, {:.3}) | Mag: ({:.3}, {:.3}, {:.3})",
                    m.accelerometer[0],
                    m.accelerometer[1],
                    m.accelerometer[2],
                    m.magnetometer[0],
                    m.magnetometer[1],
                    m.magnetometer[2],
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::MOT]).await;
}

async fn stream_band_power(
    state: &SessionState,
    token: &str,
    session_id: &str,
    num_ch: usize,
    max: u64,
) {
    match streams::subscribe_band_power(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming band power (theta/alpha/betaL/betaH/gamma per channel)... (Ctrl+C to stop)".green()
            );
            run_stream_loop(&mut s, max, |bp| {
                for (i, powers) in bp.channel_powers.iter().enumerate() {
                    print!(
                        "Ch{}: [{:.2} {:.2} {:.2} {:.2} {:.2}] ",
                        i, powers[0], powers[1], powers[2], powers[3], powers[4]
                    );
                }
                println!();
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::POW]).await;
}

async fn stream_metrics(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_metrics(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming performance metrics... (Ctrl+C to stop)".green()
            );
            println!(
                "{:<10} {:<10} {:<10} {:<10} {:<10}",
                "Engage", "Stress", "Relax", "Interest", "Focus"
            );
            run_stream_loop(&mut s, max, |m| {
                println!(
                    "{:<10.3} {:<10.3} {:<10.3} {:<10.3} {:<10.3}",
                    m.engagement.unwrap_or(0.0),
                    m.stress.unwrap_or(0.0),
                    m.relaxation.unwrap_or(0.0),
                    m.interest.unwrap_or(0.0),
                    m.focus.unwrap_or(0.0),
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::MET]).await;
}

async fn stream_mental_commands(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_mental_commands(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming mental commands (load a profile first!)... (Ctrl+C to stop)".green()
            );
            run_stream_loop(&mut s, max, |cmd| {
                if cmd.power > 0.0 {
                    println!("Action: {:<15} Power: {:.3}", cmd.action.cyan(), cmd.power);
                }
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::COM]).await;
}

async fn stream_facial(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_facial_expressions(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming facial expressions... (Ctrl+C to stop)".green()
            );
            run_stream_loop(&mut s, max, |fac| {
                println!(
                    "Eye: {} | Upper: {} ({:.2}) | Lower: {} ({:.2})",
                    fac.eye_action.cyan(),
                    fac.upper_face_action,
                    fac.upper_face_power,
                    fac.lower_face_action,
                    fac.lower_face_power,
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::FAC]).await;
}

async fn stream_eq(state: &SessionState, token: &str, session_id: &str, num_ch: usize, max: u64) {
    match streams::subscribe_eq(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!("{}", "Streaming EEG quality... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |eq| {
                let quals: Vec<String> = eq
                    .sensor_quality
                    .iter()
                    .map(|q| format!("{:.2}", q))
                    .collect();
                println!(
                    "Battery: {}% | Overall: {:.2} | SR: {:.2} | Sensors: [{}]",
                    eq.battery_percent,
                    eq.overall,
                    eq.sample_rate_quality,
                    quals.join(", ")
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::EQ]).await;
}

async fn stream_sys(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_sys(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!("{}", "Streaming system events... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |event| {
                println!("SYS: {:?}", event);
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::SYS]).await;
}

// ─── Quickstart LSL ─────────────────────────────────────────────────────

/// Automated quickstart: authenticate → find first INSIGHT headset →
/// connect → create session → start LSL streaming (EEG by default).
#[cfg(feature = "lsl")]
async fn quickstart_lsl(state: &mut SessionState) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "── Quickstart ──".bright_blue().bold());

    // 1. Authenticate
    if state.config.client_id.is_empty() || state.config.client_secret.is_empty() {
        return Err("Quickstart requires client_id and client_secret in config or env vars".into());
    }

    print!("  Authenticating... ");
    let token = state
        .client
        .authenticate(&state.config.client_id, &state.config.client_secret)
        .await?;
    println!("{} ({}...)", "ok".green(), &token[..20.min(token.len())]);
    state.token = Some(token.clone());

    // 2. Find first INSIGHT headset
    print!("  Querying headsets... ");
    let headsets = state.client.query_headsets().await?;
    let insight = headsets
        .iter()
        .find(|h| {
            let model = HeadsetModel::from_headset_info(h);
            matches!(model, HeadsetModel::Insight)
        })
        .ok_or("No INSIGHT headset found. Make sure your headset is turned on and nearby.")?;

    let headset_id = insight.id.clone();
    let model = HeadsetModel::from_headset_info(insight);
    println!(
        "{} found {} ({})",
        "ok".green(),
        headset_id.cyan(),
        insight.status
    );

    // 3. Connect (if not already connected)
    if insight.status != "connected" {
        print!("  Connecting to {}... ", headset_id.cyan());
        state.client.connect_headset(&headset_id).await?;
        println!("{}", "ok".green());

        // Give the headset a moment to establish the connection
        println!("  Waiting for connection to stabilize...");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    state.headset_id = Some(headset_id.clone());

    // 4. Create session
    print!("  Creating session... ");
    let session = state.client.create_session(&token, &headset_id).await?;
    println!("{} {}", "ok".green(), session.id.cyan());
    state.session_id = Some(session.id.clone());

    // 5. Start LSL streaming with default streams (EEG)
    let selected = vec![lsl::LslStream::Eeg];
    let source_id = &headset_id;

    print!("  Starting LSL... ");
    let handle = lsl::start_lsl_streaming(
        &state.client,
        &token,
        &session.id,
        &model,
        &selected,
        source_id,
    )
    .await?;
    state.lsl_streaming = Some(handle);

    println!("\n{}", "── Quickstart complete ──".bright_blue().bold());
    println!("  LSL streaming is running in the background. Use the menu to manage it.\n");

    Ok(())
}

// ─── Stream to LSL ──────────────────────────────────────────────────────

#[cfg(feature = "lsl")]
async fn cmd_stream_lsl(state: &mut SessionState) {
    // If LSL is already streaming, show management sub-menu
    if state.lsl_streaming.is_some() {
        let items = vec![
            "View Stream Stats",
            "Add Streams",
            "Stop LSL Streaming",
            "Back",
        ];
        let sel = Select::new()
            .with_prompt("LSL is active")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None);

        match sel {
            Some(0) => {
                // View detailed stats
                if let Some(ref handle) = state.lsl_streaming {
                    println!("\n{}", handle.format_detailed_stats());
                }
            }
            Some(1) => {
                // Add streams: stop current, then restart with expanded selection
                let currently_active: Vec<lsl::LslStream> = state
                    .lsl_streaming
                    .as_ref()
                    .map(|h| h.subscribed_streams())
                    .unwrap_or_default();

                let all_streams = lsl::LslStream::all();
                let labels: Vec<String> = all_streams
                    .iter()
                    .map(|s| {
                        if currently_active.contains(s) {
                            format!("{} (active)", s.label())
                        } else {
                            s.label().to_string()
                        }
                    })
                    .collect();

                let defaults: Vec<bool> = all_streams
                    .iter()
                    .map(|s| currently_active.contains(s))
                    .collect();

                println!(
                    "\n{}",
                    "Select streams (currently active ones are pre-selected):".bright_blue()
                );
                let selections = dialoguer::MultiSelect::new()
                    .with_prompt("Streams (space to toggle, enter to confirm)")
                    .items(&labels)
                    .defaults(&defaults)
                    .interact_opt()
                    .unwrap_or(None);

                let Some(selected_indices) = selections else {
                    return;
                };
                if selected_indices.is_empty() {
                    println!("{}", "No streams selected.".yellow());
                    return;
                }

                let selected: Vec<lsl::LslStream> =
                    selected_indices.iter().map(|&i| all_streams[i]).collect();

                // Only restart if the selection actually changed
                if selected == currently_active {
                    println!("{}", "Stream selection unchanged.".dimmed());
                    return;
                }

                let token = state.token().map(|t| t.to_string()).unwrap_or_default();
                let session_id = state.session_id.clone().unwrap_or_default();
                let model = state
                    .headset_id
                    .as_deref()
                    .map(HeadsetModel::from_headset_id)
                    .unwrap_or(HeadsetModel::Insight);
                let source_id = state
                    .headset_id
                    .as_deref()
                    .unwrap_or("emotiv-unknown")
                    .to_string();

                // Stop current streaming
                if let Some(handle) = state.lsl_streaming.take() {
                    if let Err(e) =
                        lsl::stop_lsl_streaming(handle, &state.client, &token, &session_id).await
                    {
                        eprintln!("{} {}", "LSL stop error:".red(), e);
                    }
                }

                // Restart with new selection
                println!(
                    "\n{} Restarting with {} stream(s)...",
                    "LSL:".green(),
                    selected.len()
                );
                match lsl::start_lsl_streaming(
                    &state.client,
                    &token,
                    &session_id,
                    &model,
                    &selected,
                    &source_id,
                )
                .await
                {
                    Ok(handle) => state.lsl_streaming = Some(handle),
                    Err(e) => eprintln!("{} {}", "LSL error:".red(), e),
                }
            }
            Some(2) => {
                // Stop streaming
                if let Some(handle) = state.lsl_streaming.take() {
                    let token = state.token().map(|t| t.to_string()).unwrap_or_default();
                    let session_id = state.session_id.clone().unwrap_or_default();
                    match lsl::stop_lsl_streaming(handle, &state.client, &token, &session_id).await
                    {
                        Ok(()) => {}
                        Err(e) => eprintln!("{} {}", "LSL stop error:".red(), e),
                    }
                }
            }
            _ => {} // Back or cancel
        }
        return;
    }

    // Start new LSL streaming
    let Ok(token) = state.token().map(|t| t.to_string()) else {
        eprintln!("{}", "Authenticate first.".yellow());
        return;
    };
    let Some(session_id) = state.session_id.clone() else {
        eprintln!("{}", "Create a session first.".yellow());
        return;
    };

    let model = state
        .headset_id
        .as_deref()
        .map(HeadsetModel::from_headset_id)
        .unwrap_or(HeadsetModel::Insight);

    let all_streams = lsl::LslStream::all();
    let labels: Vec<&str> = all_streams.iter().map(|s| s.label()).collect();

    println!("\n{}", "Select streams to forward to LSL:".bright_blue());
    let selections = dialoguer::MultiSelect::new()
        .with_prompt("Streams (space to toggle, enter to confirm)")
        .items(&labels)
        .defaults(&[true, false, false, false, false, false, false, false])
        .interact_opt()
        .unwrap_or(None);

    let Some(selected_indices) = selections else {
        return;
    };
    if selected_indices.is_empty() {
        println!("{}", "No streams selected.".yellow());
        return;
    }

    let selected: Vec<lsl::LslStream> = selected_indices.iter().map(|&i| all_streams[i]).collect();

    let source_id = state.headset_id.as_deref().unwrap_or("emotiv-unknown");

    println!(
        "\n{} Forwarding {} stream(s) to LSL (source: {})...",
        "Starting LSL:".green(),
        selected.len(),
        source_id.cyan(),
    );

    match lsl::start_lsl_streaming(
        &state.client,
        &token,
        &session_id,
        &model,
        &selected,
        source_id,
    )
    .await
    {
        Ok(handle) => {
            state.lsl_streaming = Some(handle);
        }
        Err(e) => eprintln!("{} {}", "LSL error:".red(), e),
    }
}

// ─── Records & Markers ──────────────────────────────────────────────────

async fn cmd_records(state: &mut SessionState) {
    let Ok(token) = state.token().map(|t| t.to_string()) else {
        eprintln!("{}", "Authenticate first.".yellow());
        return;
    };

    let items = vec![
        "Start Recording",
        "Stop Recording",
        "Inject Marker",
        "Query Records",
        "Export Record",
        "Update Record",
        "Delete Record",
        "Download Record",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Records action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    match sel {
        Some(0) => {
            let Some(sid) = state.session_id.clone() else {
                eprintln!("{}", "Create a session first.".yellow());
                return;
            };
            let title: String = Input::new()
                .with_prompt("Recording title")
                .default("CLI Recording".into())
                .interact_text()
                .unwrap_or_default();
            match state.client.create_record(&token, &sid, &title).await {
                Ok(r) => println!("{} Recording: {}", "Started!".green(), r.uuid.cyan()),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(1) => {
            let Some(sid) = state.session_id.clone() else {
                eprintln!("{}", "Create a session first.".yellow());
                return;
            };
            match state.client.stop_record(&token, &sid).await {
                Ok(r) => println!("{} Recording stopped: {}", "OK".green(), r.uuid.cyan()),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(2) => {
            let Some(sid) = state.session_id.clone() else {
                eprintln!("{}", "Create a session first.".yellow());
                return;
            };
            let label: String = Input::new()
                .with_prompt("Marker label")
                .default("event".into())
                .interact_text()
                .unwrap_or_default();
            let value: i32 = Input::new()
                .with_prompt("Marker value")
                .default(1)
                .interact_text()
                .unwrap_or(1);
            match state
                .client
                .inject_marker(&token, &sid, &label, value, "emotiv-cortex-cli", None)
                .await
            {
                Ok(m) => println!(
                    "{} Marker: {} ({})",
                    "Injected!".green(),
                    m.uuid,
                    label.cyan()
                ),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(3) => match state.client.query_records(&token, Some(10), None).await {
            Ok(records) => {
                println!("{} record(s):", records.len());
                for r in &records {
                    println!(
                        "  {} — {}",
                        r.uuid.cyan(),
                        r.title.as_deref().unwrap_or("(untitled)")
                    );
                }
            }
            Err(e) => eprintln!("{} {}", "Error:".red(), e),
        },
        Some(4) => {
            let record_id: String = Input::new()
                .with_prompt("Record UUID to export")
                .interact_text()
                .unwrap_or_default();
            let folder: String = Input::new()
                .with_prompt("Export folder")
                .default(std::env::temp_dir().to_string_lossy().to_string())
                .interact_text()
                .unwrap_or_default();
            let fmt = Select::new()
                .with_prompt("Format")
                .items(&["CSV", "EDF"])
                .default(0)
                .interact_opt()
                .unwrap_or(Some(0));
            let format = if fmt == Some(1) {
                ExportFormat::Edf
            } else {
                ExportFormat::Csv
            };
            match state
                .client
                .export_record(&token, &[record_id], &folder, format)
                .await
            {
                Ok(()) => println!("{} Export initiated to {}", "OK".green(), folder.cyan()),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(5) => {
            let record_id: String = Input::new()
                .with_prompt("Record UUID to update")
                .interact_text()
                .unwrap_or_default();
            let title: String = Input::new()
                .with_prompt("New title (empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let desc: String = Input::new()
                .with_prompt("New description (empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let title_opt = if title.is_empty() {
                None
            } else {
                Some(title.as_str())
            };
            let desc_opt = if desc.is_empty() {
                None
            } else {
                Some(desc.as_str())
            };
            match state
                .client
                .update_record(&token, &record_id, title_opt, desc_opt, None)
                .await
            {
                Ok(r) => println!("{} Record updated: {}", "OK".green(), r.uuid.cyan()),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(6) => {
            let record_id: String = Input::new()
                .with_prompt("Record UUID to delete")
                .interact_text()
                .unwrap_or_default();
            match state.client.delete_record(&token, &[record_id]).await {
                Ok(result) => {
                    println!("{}", "Record deleted:".green());
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(7) => {
            let record_id: String = Input::new()
                .with_prompt("Record UUID to download")
                .interact_text()
                .unwrap_or_default();
            match state.client.download_record(&token, &[record_id]).await {
                Ok(result) => {
                    println!("{}", "Download requested:".green());
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}

// ─── Subjects ───────────────────────────────────────────────────────────

async fn cmd_subjects(state: &mut SessionState) {
    let Ok(token) = state.token().map(|t| t.to_string()) else {
        eprintln!("{}", "Authenticate first.".yellow());
        return;
    };

    let items = vec![
        "Create Subject",
        "Update Subject",
        "Delete Subjects",
        "Query Subjects",
        "Demographic Attributes",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Subjects action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    match sel {
        Some(0) => {
            let name: String = Input::new()
                .with_prompt("Subject name")
                .interact_text()
                .unwrap_or_default();
            let dob: String = Input::new()
                .with_prompt("Date of birth (YYYY-MM-DD, empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let sex: String = Input::new()
                .with_prompt("Sex (M/F/U, empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let dob_opt = if dob.is_empty() {
                None
            } else {
                Some(dob.as_str())
            };
            let sex_opt = if sex.is_empty() {
                None
            } else {
                Some(sex.as_str())
            };
            match state
                .client
                .create_subject(&token, &name, dob_opt, sex_opt, None, None, None, None)
                .await
            {
                Ok(s) => println!(
                    "{} Subject created: {}",
                    "OK".green(),
                    s.subject_name.cyan()
                ),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(1) => {
            let name: String = Input::new()
                .with_prompt("Subject name to update")
                .interact_text()
                .unwrap_or_default();
            let dob: String = Input::new()
                .with_prompt("Date of birth (YYYY-MM-DD, empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let sex: String = Input::new()
                .with_prompt("Sex (M/F/U, empty to skip)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let dob_opt = if dob.is_empty() {
                None
            } else {
                Some(dob.as_str())
            };
            let sex_opt = if sex.is_empty() {
                None
            } else {
                Some(sex.as_str())
            };
            match state
                .client
                .update_subject(&token, &name, dob_opt, sex_opt, None, None, None, None)
                .await
            {
                Ok(s) => println!(
                    "{} Subject updated: {}",
                    "OK".green(),
                    s.subject_name.cyan()
                ),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(2) => {
            let names: String = Input::new()
                .with_prompt("Subject name(s) to delete (comma-separated)")
                .interact_text()
                .unwrap_or_default();
            let name_list: Vec<String> = names.split(',').map(|s| s.trim().to_string()).collect();
            match state.client.delete_subjects(&token, &name_list).await {
                Ok(result) => {
                    println!("{}", "Subjects deleted:".green());
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(3) => {
            match state
                .client
                .query_subjects(
                    &token,
                    serde_json::json!({}),
                    serde_json::json!([{"subjectName": "ASC"}]),
                    Some(20),
                    None,
                )
                .await
            {
                Ok((subjects, count)) => {
                    println!("{} subject(s) (total: {}):", subjects.len(), count);
                    for s in &subjects {
                        println!(
                            "  {} — dob: {} sex: {}",
                            s.subject_name.cyan(),
                            s.date_of_birth.as_deref().unwrap_or("n/a"),
                            s.sex.as_deref().unwrap_or("n/a"),
                        );
                    }
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(4) => match state.client.get_demographic_attributes(&token).await {
            Ok(attrs) => {
                println!("{}", "Demographic Attributes:".bright_blue());
                for a in &attrs {
                    println!("  {}: {:?}", a.name.cyan(), a.value);
                }
            }
            Err(e) => eprintln!("{} {}", "Error:".red(), e),
        },
        _ => {}
    }
}

// ─── Profiles ───────────────────────────────────────────────────────────

async fn cmd_profiles(state: &mut SessionState) {
    let Ok(token) = state.token().map(|t| t.to_string()) else {
        eprintln!("{}", "Authenticate first.".yellow());
        return;
    };

    let items = vec![
        "List Profiles",
        "Current Profile",
        "Load Profile",
        "Unload Profile",
        "Create Profile",
        "Save Profile",
        "Delete Profile",
        "Load Guest Profile",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Profile action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    let headset_id = state.headset_id.clone().unwrap_or_default();

    match sel {
        Some(0) => match state.client.query_profiles(&token).await {
            Ok(profiles) => {
                if profiles.is_empty() {
                    println!("No profiles found.");
                } else {
                    println!("{} profile(s):", profiles.len());
                    for p in &profiles {
                        println!("  {}", p.name.cyan());
                    }
                }
            }
            Err(e) => eprintln!("{} {}", "Error:".red(), e),
        },
        Some(1) => {
            if headset_id.is_empty() {
                eprintln!("{}", "Select a headset first.".yellow());
                return;
            }
            match state.client.get_current_profile(&token, &headset_id).await {
                Ok(Some(p)) => println!("Current profile: {}", p.name.cyan()),
                Ok(None) => println!("No profile loaded."),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(2) | Some(3) | Some(4) | Some(5) | Some(6) => {
            if headset_id.is_empty() {
                eprintln!("{}", "Select a headset first.".yellow());
                return;
            }
            let action = match sel {
                Some(2) => ProfileAction::Load,
                Some(3) => ProfileAction::Unload,
                Some(4) => ProfileAction::Create,
                Some(5) => ProfileAction::Save,
                Some(6) => ProfileAction::Delete,
                _ => unreachable!(),
            };
            let name: String = Input::new()
                .with_prompt("Profile name")
                .interact_text()
                .unwrap_or_default();
            match state
                .client
                .setup_profile(&token, &headset_id, &name, action)
                .await
            {
                Ok(()) => println!("{} Profile action completed", "OK".green()),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(7) => {
            if headset_id.is_empty() {
                eprintln!("{}", "Select a headset first.".yellow());
                return;
            }
            match state.client.load_guest_profile(&token, &headset_id).await {
                Ok(()) => println!("{} Guest profile loaded", "OK".green()),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}

// ─── BCI Training ───────────────────────────────────────────────────────

async fn cmd_training(state: &mut SessionState) {
    let Ok(token) = state.token().map(|t| t.to_string()) else {
        eprintln!("{}", "Authenticate first.".yellow());
        return;
    };

    let items = vec![
        "Detection Info (Mental Commands)",
        "Detection Info (Facial Expressions)",
        "Start Training",
        "Accept Training",
        "Reject Training",
        "Erase Training",
        "Active Mental Commands",
        "Trained Signature Actions",
        "Training Time",
        "FE Signature Type",
        "FE Threshold",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Training action")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    match sel {
        Some(0) => {
            match state
                .client
                .get_detection_info(DetectionType::MentalCommand)
                .await
            {
                Ok(info) => println!("Mental Command Detection:\n{:?}", info),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(1) => {
            match state
                .client
                .get_detection_info(DetectionType::FacialExpression)
                .await
            {
                Ok(info) => println!("Facial Expression Detection:\n{:?}", info),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(2) | Some(3) | Some(4) | Some(5) => {
            let Some(sid) = state.session_id.clone() else {
                eprintln!("{}", "Create a session first.".yellow());
                return;
            };
            let det_type = Select::new()
                .with_prompt("Detection type")
                .items(&["Mental Command", "Facial Expression"])
                .default(0)
                .interact_opt()
                .unwrap_or(Some(0));
            let detection = if det_type == Some(1) {
                DetectionType::FacialExpression
            } else {
                DetectionType::MentalCommand
            };
            let status = match sel {
                Some(2) => TrainingStatus::Start,
                Some(3) => TrainingStatus::Accept,
                Some(4) => TrainingStatus::Reject,
                Some(5) => TrainingStatus::Erase,
                _ => TrainingStatus::Start,
            };
            let action: String = Input::new()
                .with_prompt("Action name (e.g., neutral, push, pull)")
                .default("neutral".into())
                .interact_text()
                .unwrap_or_default();
            match state
                .client
                .training(&token, &sid, detection, status, &action)
                .await
            {
                Ok(result) => println!("{} {:?}", "Training result:".green(), result),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(6) => {
            let Some(sid) = state.session_id.clone() else {
                eprintln!("{}", "Create a session first.".yellow());
                return;
            };
            match state
                .client
                .mental_command_active_action(&token, &sid, None)
                .await
            {
                Ok(result) => println!("Active mental commands: {:?}", result),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(7) => {
            let det_sel = Select::new()
                .with_prompt("Detection type")
                .items(&["Mental Command", "Facial Expression"])
                .default(0)
                .interact_opt()
                .unwrap_or(Some(0));
            let detection = if det_sel == Some(1) {
                DetectionType::FacialExpression
            } else {
                DetectionType::MentalCommand
            };
            let profile: String = Input::new()
                .with_prompt("Profile name (empty for session-based)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let session = state.session_id.clone().unwrap_or_default();
            let p = if profile.is_empty() {
                None
            } else {
                Some(profile.as_str())
            };
            let s = if session.is_empty() {
                None
            } else {
                Some(session.as_str())
            };
            match state
                .client
                .get_trained_signature_actions(&token, detection, p, s)
                .await
            {
                Ok(info) => {
                    println!("Total training sessions: {}", info.total_times_training);
                    for a in &info.trained_actions {
                        println!("  {} — trained {} times", a.action.cyan(), a.times);
                    }
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(8) => {
            let Some(sid) = state.session_id.clone() else {
                eprintln!("{}", "Create a session first.".yellow());
                return;
            };
            let det_sel = Select::new()
                .with_prompt("Detection type")
                .items(&["Mental Command", "Facial Expression"])
                .default(0)
                .interact_opt()
                .unwrap_or(Some(0));
            let detection = if det_sel == Some(1) {
                DetectionType::FacialExpression
            } else {
                DetectionType::MentalCommand
            };
            match state
                .client
                .get_training_time(&token, detection, &sid)
                .await
            {
                Ok(t) => println!("Training time: {:.2}s", t.time),
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(9) => {
            let profile: String = Input::new()
                .with_prompt("Profile name (empty for session-based)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let session = state.session_id.clone().unwrap_or_default();
            let p = if profile.is_empty() {
                None
            } else {
                Some(profile.as_str())
            };
            let s = if session.is_empty() {
                None
            } else {
                Some(session.as_str())
            };
            match state
                .client
                .facial_expression_signature_type(&token, "get", p, s, None)
                .await
            {
                Ok(result) => {
                    println!("{}", "FE Signature Type:".bright_blue());
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(10) => {
            let action: String = Input::new()
                .with_prompt("FE action name (e.g., smile, frown)")
                .interact_text()
                .unwrap_or_default();
            let profile: String = Input::new()
                .with_prompt("Profile name (empty for session-based)")
                .default(String::new())
                .interact_text()
                .unwrap_or_default();
            let session = state.session_id.clone().unwrap_or_default();
            let p = if profile.is_empty() {
                None
            } else {
                Some(profile.as_str())
            };
            let s = if session.is_empty() {
                None
            } else {
                Some(session.as_str())
            };
            match state
                .client
                .facial_expression_threshold(&token, "get", &action, p, s, None)
                .await
            {
                Ok(result) => {
                    println!("{}", "FE Threshold:".bright_blue());
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}
