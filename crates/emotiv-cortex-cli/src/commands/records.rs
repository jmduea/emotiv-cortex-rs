use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::protocol::records::{ExportFormat, UpdateRecordRequest};

use crate::app::{SessionState, print_pretty_json};

// ─── Records & Markers ──────────────────────────────────────────────────

pub async fn cmd_records(state: &mut SessionState) {
    let Ok(token) = state.token().map(std::string::ToString::to_string) else {
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
        Some(0) => start_recording(state, &token).await,
        Some(1) => stop_recording(state, &token).await,
        Some(2) => inject_marker(state, &token).await,
        Some(3) => query_records(state, &token).await,
        Some(4) => export_record(state, &token).await,
        Some(5) => update_record(state, &token).await,
        Some(6) => delete_record(state, &token).await,
        Some(7) => download_record(state, &token).await,
        _ => {}
    }
}

fn current_session_id(state: &SessionState) -> Option<String> {
    let sid = state.session_id.clone();
    if sid.is_none() {
        eprintln!("{}", "Create a session first.".yellow());
    }
    sid
}

async fn start_recording(state: &mut SessionState, token: &str) {
    let Some(sid) = current_session_id(state) else {
        return;
    };
    let title: String = Input::new()
        .with_prompt("Recording title")
        .default("CLI Recording".into())
        .interact_text()
        .unwrap_or_default();
    match state.client.create_record(token, &sid, &title).await {
        Ok(r) => println!("{} Recording: {}", "Started!".green(), r.uuid.cyan()),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn stop_recording(state: &mut SessionState, token: &str) {
    let Some(sid) = current_session_id(state) else {
        return;
    };
    match state.client.stop_record(token, &sid).await {
        Ok(r) => println!("{} Recording stopped: {}", "OK".green(), r.uuid.cyan()),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn inject_marker(state: &mut SessionState, token: &str) {
    let Some(sid) = current_session_id(state) else {
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
        .inject_marker(token, &sid, &label, value, "emotiv-cortex-cli", None)
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

async fn query_records(state: &mut SessionState, token: &str) {
    match state.client.query_records(token, Some(10), None).await {
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
    }
}

async fn export_record(state: &mut SessionState, token: &str) {
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
        .export_record(token, &[record_id], &folder, format)
        .await
    {
        Ok(()) => println!("{} Export initiated to {}", "OK".green(), folder.cyan()),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn update_record(state: &mut SessionState, token: &str) {
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
    let request = UpdateRecordRequest {
        record_id,
        title: (!title.is_empty()).then_some(title),
        description: (!desc.is_empty()).then_some(desc),
        tags: None,
    };

    match state.client.update_record_with(token, &request).await {
        Ok(r) => println!("{} Record updated: {}", "OK".green(), r.uuid.cyan()),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn delete_record(state: &mut SessionState, token: &str) {
    let record_id: String = Input::new()
        .with_prompt("Record UUID to delete")
        .interact_text()
        .unwrap_or_default();
    match state.client.delete_record(token, &[record_id]).await {
        Ok(result) => {
            println!("{}", "Record deleted:".green());
            print_pretty_json(&result);
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn download_record(state: &mut SessionState, token: &str) {
    let record_id: String = Input::new()
        .with_prompt("Record UUID to download")
        .interact_text()
        .unwrap_or_default();
    match state.client.download_record(token, &[record_id]).await {
        Ok(result) => {
            println!("{}", "Download requested:".green());
            print_pretty_json(&result);
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}
