use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::protocol::subjects::{QuerySubjectsRequest, SubjectRequest};

use crate::app::{SessionState, print_pretty_json};

// ─── Subjects ───────────────────────────────────────────────────────────

pub async fn cmd_subjects(state: &mut SessionState) {
    let Ok(token) = state.token().map(std::string::ToString::to_string) else {
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
        Some(0) => create_subject(state, &token).await,
        Some(1) => update_subject(state, &token).await,
        Some(2) => delete_subjects(state, &token).await,
        Some(3) => query_subjects(state, &token).await,
        Some(4) => show_demographic_attributes(state, &token).await,
        _ => {}
    }
}

fn prompt_optional(prompt: &str) -> String {
    Input::new()
        .with_prompt(prompt)
        .default(String::new())
        .interact_text()
        .unwrap_or_default()
}

fn build_subject_request(name_prompt: &str) -> SubjectRequest {
    let name: String = Input::new()
        .with_prompt(name_prompt)
        .interact_text()
        .unwrap_or_default();
    let dob = prompt_optional("Date of birth (YYYY-MM-DD, empty to skip)");
    let sex = prompt_optional("Sex (M/F/U, empty to skip)");

    let mut request = SubjectRequest::new(name);
    request.date_of_birth = (!dob.is_empty()).then_some(dob);
    request.sex = (!sex.is_empty()).then_some(sex);
    request
}

async fn create_subject(state: &mut SessionState, token: &str) {
    let request = build_subject_request("Subject name");
    match state.client.create_subject_with(token, &request).await {
        Ok(s) => println!(
            "{} Subject created: {}",
            "OK".green(),
            s.subject_name.cyan()
        ),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn update_subject(state: &mut SessionState, token: &str) {
    let request = build_subject_request("Subject name to update");
    match state.client.update_subject_with(token, &request).await {
        Ok(s) => println!(
            "{} Subject updated: {}",
            "OK".green(),
            s.subject_name.cyan()
        ),
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn delete_subjects(state: &mut SessionState, token: &str) {
    let names: String = Input::new()
        .with_prompt("Subject name(s) to delete (comma-separated)")
        .interact_text()
        .unwrap_or_default();
    let name_list: Vec<String> = names
        .split(',')
        .map(|name| name.trim().to_string())
        .collect();

    match state.client.delete_subjects(token, &name_list).await {
        Ok(result) => {
            println!("{}", "Subjects deleted:".green());
            print_pretty_json(&result);
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}

async fn query_subjects(state: &mut SessionState, token: &str) {
    let request = QuerySubjectsRequest {
        query: serde_json::json!({}),
        order_by: serde_json::json!([{"subjectName": "ASC"}]),
        limit: Some(20),
        offset: None,
    };

    match state.client.query_subjects_with(token, &request).await {
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

async fn show_demographic_attributes(state: &mut SessionState, token: &str) {
    match state.client.get_demographic_attributes(token).await {
        Ok(attrs) => {
            println!("{}", "Demographic Attributes:".bright_blue());
            for a in &attrs {
                println!("  {}: {:?}", a.name.cyan(), a.value);
            }
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
}
