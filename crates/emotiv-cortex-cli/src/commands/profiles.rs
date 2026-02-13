use colored::Colorize;
use dialoguer::{Input, Select};

use emotiv_cortex_v2::protocol::profiles::ProfileAction;

use crate::app::SessionState;

// ─── Profiles ───────────────────────────────────────────────────────────

pub async fn cmd_profiles(state: &mut SessionState) {
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
                Ok(p) => match p.name {
                    Some(name) => println!("Current profile: {}", name.cyan()),
                    None => println!("No profile loaded."),
                },
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
