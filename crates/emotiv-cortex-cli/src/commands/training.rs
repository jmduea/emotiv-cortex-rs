use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::protocol::training::{
    DetectionType, FacialExpressionSignatureTypeRequest, FacialExpressionThresholdRequest,
    TrainingStatus,
};

use crate::app::{SessionState, print_pretty_json};

// ─── BCI Training ───────────────────────────────────────────────────────

pub async fn cmd_training(state: &mut SessionState) {
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
            let request = FacialExpressionSignatureTypeRequest {
                status: "get".to_string(),
                profile: p.map(ToString::to_string),
                session: s.map(ToString::to_string),
                signature: None,
            };
            match state
                .client
                .facial_expression_signature_type_with(&token, &request)
                .await
            {
                Ok(result) => {
                    println!("{}", "FE Signature Type:".bright_blue());
                    print_pretty_json(&result);
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
            let request = FacialExpressionThresholdRequest {
                status: "get".to_string(),
                action: action.clone(),
                profile: p.map(ToString::to_string),
                session: s.map(ToString::to_string),
                value: None,
            };
            match state
                .client
                .facial_expression_threshold_with(&token, &request)
                .await
            {
                Ok(result) => {
                    println!("{}", "FE Threshold:".bright_blue());
                    print_pretty_json(&result);
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        _ => {}
    }
}
