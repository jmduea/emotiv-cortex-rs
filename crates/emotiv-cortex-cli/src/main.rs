//! # emotiv-cortex-cli
//!
//! Interactive CLI explorer for the Emotiv Cortex v2 API.
//! Covers authentication, headset management, data streaming,
//! recording, markers, profiles, and BCI training.
//! When built with `--features lsl`, the CLI can publish self-documenting LSL
//! streams with channel metadata for downstream tool interoperability.

#[cfg(all(feature = "lsl", target_os = "linux"))]
compile_error!(
    "The `lsl` feature is currently unsupported on Linux due upstream `lsl-sys` \
build incompatibilities. Build without `--features lsl`, or use Windows/macOS for LSL."
);

use std::path::Path;

use clap::Parser;
use colored::Colorize;
use dialoguer::Select;

mod app;
mod commands;
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
mod lsl;

use app::{SessionState, format_status, graceful_shutdown};
use commands::{
    cmd_authentication, cmd_cortex_info, cmd_headsets, cmd_profiles, cmd_records, cmd_sessions,
    cmd_stream_data, cmd_subjects, cmd_training,
};
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use commands::{cmd_stream_lsl, quickstart_lsl};

use emotiv_cortex_v2::{CortexClient, CortexConfig};

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
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    #[arg(long, alias = "quick")]
    quickstart: bool,
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
        #[cfg(all(feature = "lsl", not(target_os = "linux")))]
        lsl_streaming: None,
    };

    // ── Quickstart mode ─────────────────────────────────────────────
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
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
        #[cfg(all(feature = "lsl", not(target_os = "linux")))]
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
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
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
