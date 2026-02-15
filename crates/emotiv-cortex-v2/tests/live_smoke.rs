use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::{CortexClient, CortexConfig};

fn live_test_config() -> Result<CortexConfig, String> {
    if std::env::var("EMOTIV_SKIP_LIVE_TESTS")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        return Err("EMOTIV_SKIP_LIVE_TESTS is enabled".to_string());
    }

    let client_id =
        std::env::var("EMOTIV_CLIENT_ID").map_err(|_| "EMOTIV_CLIENT_ID is not set".to_string())?;
    let client_secret = std::env::var("EMOTIV_CLIENT_SECRET")
        .map_err(|_| "EMOTIV_CLIENT_SECRET is not set".to_string())?;

    let mut config = CortexConfig::new(client_id, client_secret);
    if let Ok(cortex_url) = std::env::var("EMOTIV_CORTEX_URL") {
        config.cortex_url = cortex_url;
    }

    Ok(config)
}

#[tokio::test]
async fn live_auth_and_cortex_info_smoke() {
    let config = match live_test_config() {
        Ok(config) => config,
        Err(reason) => {
            eprintln!("Skipping live smoke test: {reason}");
            return;
        }
    };

    let mut client = CortexClient::connect(&config).await.unwrap();
    let info = client.get_cortex_info().await.unwrap();
    assert!(
        info.is_object() || info.is_array(),
        "unexpected getCortexInfo payload: {info}"
    );

    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await
        .unwrap();
    assert!(!token.trim().is_empty(), "received empty cortex token");

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn live_query_headsets_and_optional_session_lifecycle_smoke() {
    let config = match live_test_config() {
        Ok(config) => config,
        Err(reason) => {
            eprintln!("Skipping live smoke test: {reason}");
            return;
        }
    };

    let preferred_headset = std::env::var("EMOTIV_HEADSET_ID").ok();

    let mut client = CortexClient::connect(&config).await.unwrap();
    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await
        .unwrap();

    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();

    if headsets.is_empty() {
        eprintln!("Skipping session lifecycle smoke: no headsets were discovered.");
        client.disconnect().await.unwrap();
        return;
    }

    let selected = if let Some(headset_id) = preferred_headset.as_deref() {
        if let Some(headset) = headsets.iter().find(|h| h.id == headset_id) {
            headset
        } else {
            eprintln!(
                "Skipping session lifecycle smoke: requested EMOTIV_HEADSET_ID '{headset_id}' was not found."
            );
            client.disconnect().await.unwrap();
            return;
        }
    } else {
        &headsets[0]
    };

    if selected.status != "connected" {
        eprintln!(
            "Skipping session lifecycle smoke: headset '{}' is not connected (status='{}').",
            selected.id, selected.status
        );
        client.disconnect().await.unwrap();
        return;
    }

    match client.create_session(&token, &selected.id).await {
        Ok(session) => {
            client.close_session(&token, &session.id).await.unwrap();
        }
        Err(err) => {
            eprintln!("Skipping session lifecycle smoke: create_session failed: {err}");
        }
    }

    client.disconnect().await.unwrap();
}
