use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::protocol::training::{
    DetectionType, FacialExpressionSignatureTypeRequest, FacialExpressionThresholdRequest,
    TrainingStatus,
};

use crate::app::{SessionState, print_pretty_json};

// ─── BCI Training ───────────────────────────────────────────────────────

pub async fn cmd_training(state: &mut SessionState) {
    let Ok(token) = state.token().map(std::string::ToString::to_string) else {
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
        Some(0) => show_detection_info(state, DetectionType::MentalCommand).await,
        Some(1) => show_detection_info(state, DetectionType::FacialExpression).await,
        Some(2..=5) => run_training_action(state, &token, sel).await,
        Some(6) => show_active_mental_commands(state, &token).await,
        Some(7) => show_trained_signature_actions(state, &token).await,
        Some(8) => show_training_time(state, &token).await,
        Some(9) => show_fe_signature_type(state, &token).await,
        Some(10) => show_fe_threshold(state, &token).await,
        _ => {}
    }
}

fn choose_detection_type() -> DetectionType {
    let selection = Select::new()
        .with_prompt("Detection type")
        .items(&["Mental Command", "Facial Expression"])
        .default(0)
        .interact_opt()
        .unwrap_or(Some(0));
    if selection == Some(1) {
        DetectionType::FacialExpression
    } else {
        DetectionType::MentalCommand
    }
}

fn profile_and_session(state: &SessionState) -> (Option<String>, Option<String>) {
    let profile: String = Input::new()
        .with_prompt("Profile name (empty for session-based)")
        .default(String::new())
        .interact_text()
        .unwrap_or_default();
    let session = state.session_id.clone().unwrap_or_default();
    (
        (!profile.is_empty()).then_some(profile),
        (!session.is_empty()).then_some(session),
    )
}

async fn show_detection_info(state: &mut SessionState, detection: DetectionType) {
    match state.client.get_detection_info(detection).await {
        Ok(info) => println!("Detection Info:\n{info:?}"),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn run_training_action(state: &mut SessionState, token: &str, sel: Option<usize>) {
    let Some(sid) = state.session_id.clone() else {
        eprintln!("{}", "Create a session first.".yellow());
        return;
    };
    let detection = choose_detection_type();
    let status = match sel {
        Some(2) => TrainingStatus::Start,
        Some(3) => TrainingStatus::Accept,
        Some(4) => TrainingStatus::Reject,
        _ => TrainingStatus::Erase,
    };
    let action: String = Input::new()
        .with_prompt("Action name (e.g., neutral, push, pull)")
        .default("neutral".into())
        .interact_text()
        .unwrap_or_default();
    match state
        .client
        .training(token, &sid, detection, status, &action)
        .await
    {
        Ok(result) => println!("{} {:?}", "Training result:".green(), result),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn show_active_mental_commands(state: &mut SessionState, token: &str) {
    let Some(sid) = state.session_id.clone() else {
        eprintln!("{}", "Create a session first.".yellow());
        return;
    };
    match state
        .client
        .mental_command_active_action(token, &sid, None)
        .await
    {
        Ok(result) => println!("Active mental commands: {result:?}"),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn show_trained_signature_actions(state: &mut SessionState, token: &str) {
    let detection = choose_detection_type();
    let (profile, session) = profile_and_session(state);
    match state
        .client
        .get_trained_signature_actions(token, detection, profile.as_deref(), session.as_deref())
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

async fn show_training_time(state: &mut SessionState, token: &str) {
    let Some(sid) = state.session_id.clone() else {
        eprintln!("{}", "Create a session first.".yellow());
        return;
    };
    let detection = choose_detection_type();
    match state.client.get_training_time(token, detection, &sid).await {
        Ok(t) => println!("Training time: {:.2}s", t.time),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn show_fe_signature_type(state: &mut SessionState, token: &str) {
    let (profile, session) = profile_and_session(state);
    let request = FacialExpressionSignatureTypeRequest {
        status: "get".to_string(),
        profile,
        session,
        signature: None,
    };
    match state
        .client
        .facial_expression_signature_type_with(token, &request)
        .await
    {
        Ok(result) => {
            println!("{}", "FE Signature Type:".bright_blue());
            print_pretty_json(&result);
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn show_fe_threshold(state: &mut SessionState, token: &str) {
    let action: String = Input::new()
        .with_prompt("FE action name (e.g., smile, frown)")
        .interact_text()
        .unwrap_or_default();
    let (profile, session) = profile_and_session(state);
    let request = FacialExpressionThresholdRequest {
        status: "get".to_string(),
        action,
        profile,
        session,
        value: None,
    };
    match state
        .client
        .facial_expression_threshold_with(token, &request)
        .await
    {
        Ok(result) => {
            println!("{}", "FE Threshold:".bright_blue());
            print_pretty_json(&result);
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}
