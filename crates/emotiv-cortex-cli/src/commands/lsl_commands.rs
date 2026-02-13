#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use colored::Colorize;
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use dialoguer::Select;

#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use emotiv_cortex_v2::headset::HeadsetModel;
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;

#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use crate::app::SessionState;
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
use crate::lsl;

// ─── Quickstart LSL ─────────────────────────────────────────────────────

/// Automated quickstart: authenticate → find first INSIGHT headset →
/// connect → create session → start LSL streaming (EEG by default).
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
pub async fn quickstart_lsl(state: &mut SessionState) -> Result<(), Box<dyn std::error::Error>> {
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
    let headsets = state
        .client
        .query_headsets(QueryHeadsetsOptions::default())
        .await?;
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

#[cfg(all(feature = "lsl", not(target_os = "linux")))]
pub async fn cmd_stream_lsl(state: &mut SessionState) {
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
