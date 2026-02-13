use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::headset::{HeadsetInfo, QueryHeadsetsOptions};

use crate::app::{SessionState, print_pretty_json};

// ─── Cortex Info ────────────────────────────────────────────────────────

pub async fn cmd_cortex_info(state: &SessionState) {
    match state.client.get_cortex_info().await {
        Ok(info) => {
            println!("\n{}", "Cortex Service Info:".bright_blue());
            print_pretty_json(&info);
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

// ─── Authentication ─────────────────────────────────────────────────────

pub async fn cmd_authentication(state: &mut SessionState) {
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
                    print_pretty_json(&info);
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
                    print_pretty_json(&info);
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}

// ─── Headsets ───────────────────────────────────────────────────────────

pub async fn cmd_headsets(state: &mut SessionState) {
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
        Some(0) => match state
            .client
            .query_headsets(QueryHeadsetsOptions::default())
            .await
        {
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
                        print_pretty_json(&result);
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
                    print_pretty_json(&result);
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

pub async fn cmd_sessions(state: &mut SessionState) {
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
