use colored::Colorize;

use emotiv_cortex_v2::{CortexClient, CortexConfig, CortexResult};

/// Shared session state passed between menu actions.
pub struct SessionState {
    pub client: CortexClient,
    pub config: CortexConfig,
    pub token: Option<String>,
    pub session_id: Option<String>,
    pub headset_id: Option<String>,
    /// Active background LSL streaming handle, if any.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_streaming: Option<crate::lsl::LslStreamingHandle>,
}

impl SessionState {
    pub fn token(&self) -> CortexResult<&str> {
        self.token
            .as_deref()
            .ok_or(emotiv_cortex_v2::CortexError::ProtocolError {
                reason: "Not authenticated. Run 'Authenticate' first.".into(),
            })
    }
}

pub fn format_status(state: &SessionState) -> String {
    let auth = if state.token.is_some() {
        "authenticated".green().to_string()
    } else {
        "not authenticated".yellow().to_string()
    };

    let headset = state
        .headset_id
        .as_deref()
        .map_or_else(|| "none".dimmed().to_string(), |h| h.cyan().to_string());

    let session = state.session_id.as_deref().map_or_else(
        || "none".dimmed().to_string(),
        |s| s[..12.min(s.len())].cyan().to_string(),
    );

    #[allow(unused_mut)]
    let mut status = format!("Auth: {auth} | Headset: {headset} | Session: {session}");

    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    if let Some(ref handle) = state.lsl_streaming {
        status.push_str(&format!(" | {}", handle.format_status().green()));
    }

    status
}

pub fn print_pretty_json(value: &serde_json::Value) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(err) => {
            eprintln!("{} {}", "Failed to format JSON output:".yellow(), err);
            println!("{value}");
        }
    }
}

/// Gracefully stop streaming/session/headset and disconnect the client.
pub async fn graceful_shutdown(state: &mut SessionState) {
    println!("Disconnecting...");

    // 0. Stop LSL streaming if active.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    if let Some(handle) = state.lsl_streaming.take() {
        let token = state.token.as_deref().unwrap_or_default();
        let session_id = state.session_id.as_deref().unwrap_or_default();
        if let Err(e) =
            crate::lsl::stop_lsl_streaming(handle, &state.client, token, session_id).await
        {
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
