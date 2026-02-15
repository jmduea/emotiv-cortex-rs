use colored::Colorize;
use dialoguer::{Input, Select};
use futures_core::Stream;
use futures_util::StreamExt;

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::constants::Streams;
use emotiv_cortex_v2::streams;

use crate::app::SessionState;

// ─── Stream Data ────────────────────────────────────────────────────────

pub async fn cmd_stream_data(state: &mut SessionState) {
    let Ok(token) = state.token().map(std::string::ToString::to_string) else {
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
        .map_or(HeadsetModel::Insight, HeadsetModel::from_headset_id);
    let num_ch = model.num_channels();

    let items = vec![
        "EEG (raw data)",
        "Device Quality",
        "Motion / IMU",
        "Band Power",
        "Performance Metrics",
        "Mental Commands",
        "Facial Expressions",
        "EEG Quality",
        "System Events",
        "Back",
    ];

    let sel = Select::new()
        .with_prompt("Select stream")
        .items(&items)
        .default(0)
        .interact_opt()
        .unwrap_or(None);

    // Back or cancel — return before asking for max samples
    match sel {
        Some(9) | None => return,
        _ => {}
    }

    let max_samples: u64 = Input::new()
        .with_prompt("Max samples (0 = unlimited)")
        .default(50)
        .interact_text()
        .unwrap_or(50);

    match sel {
        Some(0) => stream_eeg(state, &token, &session_id, num_ch, max_samples).await,
        Some(1) => stream_dev(state, &token, &session_id, num_ch, max_samples).await,
        Some(2) => stream_motion(state, &token, &session_id, max_samples).await,
        Some(3) => stream_band_power(state, &token, &session_id, num_ch, max_samples).await,
        Some(4) => stream_metrics(state, &token, &session_id, max_samples).await,
        Some(5) => stream_mental_commands(state, &token, &session_id, max_samples).await,
        Some(6) => stream_facial(state, &token, &session_id, max_samples).await,
        Some(7) => stream_eq(state, &token, &session_id, num_ch, max_samples).await,
        Some(8) => stream_sys(state, &token, &session_id, max_samples).await,
        _ => {}
    }
}

/// Run a streaming loop, breaking on max samples or Ctrl+C.
async fn run_stream_loop<T>(
    stream: &mut (impl Stream<Item = T> + Unpin),
    max: u64,
    mut on_item: impl FnMut(&T),
) {
    let mut count = 0u64;
    loop {
        tokio::select! {
            item = stream.next() => {
                let Some(data) = item else { break };
                on_item(&data);
                count += 1;
                if max > 0 && count >= max { break; }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n{}", "Streaming stopped.".yellow());
                break;
            }
        }
    }
}

async fn stream_eeg(state: &SessionState, token: &str, session_id: &str, num_ch: usize, max: u64) {
    match streams::subscribe_eeg(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!("{}", "Streaming EEG data... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |eeg_data| {
                let vals: Vec<String> = eeg_data
                    .channels
                    .iter()
                    .map(|v| format!("{v:.2}"))
                    .collect();
                println!("[sample {}] {}", eeg_data.counter, vals.join(" | "));
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::EEG]).await;
}

async fn stream_dev(state: &SessionState, token: &str, session_id: &str, num_ch: usize, max: u64) {
    match streams::subscribe_dev(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!("{}", "Streaming device quality... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |dq| {
                let quals: Vec<String> = dq
                    .channel_quality
                    .iter()
                    .map(|q| format!("{q:.1}"))
                    .collect();
                println!(
                    "Battery: {}% | Signal: {:.1} | CQ: [{}] | Overall: {:.1}",
                    dq.battery_percent,
                    dq.signal_strength,
                    quals.join(", "),
                    dq.overall_quality
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::DEV]).await;
}

async fn stream_motion(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_motion(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!("{}", "Streaming motion data... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |m| {
                println!(
                    "Accel: ({:.3}, {:.3}, {:.3}) | Mag: ({:.3}, {:.3}, {:.3})",
                    m.accelerometer[0],
                    m.accelerometer[1],
                    m.accelerometer[2],
                    m.magnetometer[0],
                    m.magnetometer[1],
                    m.magnetometer[2],
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::MOT]).await;
}

async fn stream_band_power(
    state: &SessionState,
    token: &str,
    session_id: &str,
    num_ch: usize,
    max: u64,
) {
    match streams::subscribe_band_power(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming band power (theta/alpha/betaL/betaH/gamma per channel)... (Ctrl+C to stop)".green()
            );
            run_stream_loop(&mut s, max, |bp| {
                for (i, powers) in bp.channel_powers.iter().enumerate() {
                    print!(
                        "Ch{}: [{:.2} {:.2} {:.2} {:.2} {:.2}] ",
                        i, powers[0], powers[1], powers[2], powers[3], powers[4]
                    );
                }
                println!();
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::POW]).await;
}

async fn stream_metrics(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_metrics(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming performance metrics... (Ctrl+C to stop)".green()
            );
            println!(
                "{:<10} {:<10} {:<10} {:<10} {:<10}",
                "Engage", "Stress", "Relax", "Interest", "Focus"
            );
            run_stream_loop(&mut s, max, |m| {
                println!(
                    "{:<10.3} {:<10.3} {:<10.3} {:<10.3} {:<10.3}",
                    m.engagement.unwrap_or(0.0),
                    m.stress.unwrap_or(0.0),
                    m.relaxation.unwrap_or(0.0),
                    m.interest.unwrap_or(0.0),
                    m.focus.unwrap_or(0.0),
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::MET]).await;
}

async fn stream_mental_commands(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_mental_commands(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming mental commands (load a profile first!)... (Ctrl+C to stop)".green()
            );
            run_stream_loop(&mut s, max, |cmd| {
                if cmd.power > 0.0 {
                    println!("Action: {:<15} Power: {:.3}", cmd.action.cyan(), cmd.power);
                }
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::COM]).await;
}

async fn stream_facial(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_facial_expressions(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!(
                "{}",
                "Streaming facial expressions... (Ctrl+C to stop)".green()
            );
            run_stream_loop(&mut s, max, |fac| {
                println!(
                    "Eye: {} | Upper: {} ({:.2}) | Lower: {} ({:.2})",
                    fac.eye_action.cyan(),
                    fac.upper_face_action,
                    fac.upper_face_power,
                    fac.lower_face_action,
                    fac.lower_face_power,
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::FAC]).await;
}

async fn stream_eq(state: &SessionState, token: &str, session_id: &str, num_ch: usize, max: u64) {
    match streams::subscribe_eq(&state.client, token, session_id, num_ch).await {
        Ok(mut s) => {
            println!("{}", "Streaming EEG quality... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |eq| {
                let quals: Vec<String> = eq
                    .sensor_quality
                    .iter()
                    .map(|q| format!("{q:.2}"))
                    .collect();
                println!(
                    "Battery: {}% | Overall: {:.2} | SR: {:.2} | Sensors: [{}]",
                    eq.battery_percent,
                    eq.overall,
                    eq.sample_rate_quality,
                    quals.join(", ")
                );
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::EQ]).await;
}

async fn stream_sys(state: &SessionState, token: &str, session_id: &str, max: u64) {
    match streams::subscribe_sys(&state.client, token, session_id).await {
        Ok(mut s) => {
            println!("{}", "Streaming system events... (Ctrl+C to stop)".green());
            run_stream_loop(&mut s, max, |event| {
                println!("SYS: {event:?}");
            })
            .await;
        }
        Err(e) => eprintln!("{} {}", "Error:".red(), e),
    }
    let _ = streams::unsubscribe(&state.client, token, session_id, &[Streams::SYS]).await;
}
