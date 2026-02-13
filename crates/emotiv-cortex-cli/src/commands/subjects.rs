use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::protocol::subjects::{QuerySubjectsRequest, SubjectRequest};

use crate::app::{SessionState, print_pretty_json};

// ─── Subjects ───────────────────────────────────────────────────────────

pub async fn cmd_subjects(state: &mut SessionState) {
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
            let mut request = SubjectRequest::new(name.clone());
            request.date_of_birth = dob_opt.map(ToString::to_string);
            request.sex = sex_opt.map(ToString::to_string);
            match state.client.create_subject_with(&token, &request).await {
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
            let mut request = SubjectRequest::new(name.clone());
            request.date_of_birth = dob_opt.map(ToString::to_string);
            request.sex = sex_opt.map(ToString::to_string);
            match state.client.update_subject_with(&token, &request).await {
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
                    print_pretty_json(&result);
                }
                Err(e) => eprintln!("{} {}", "Error:".red(), e),
            }
        }
        Some(3) => {
            let request = QuerySubjectsRequest {
                query: serde_json::json!({}),
                order_by: serde_json::json!([{"subjectName": "ASC"}]),
                limit: Some(20),
                offset: None,
            };
            match state.client.query_subjects_with(&token, &request).await {
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
