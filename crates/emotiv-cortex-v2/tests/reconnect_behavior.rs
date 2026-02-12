mod support;

use std::time::Duration;

use emotiv_cortex_v2::protocol::{Methods, QueryHeadsetsOptions};
use emotiv_cortex_v2::reconnect::{ConnectionEvent, ResilientClient};
use emotiv_cortex_v2::CortexConfig;
use serde_json::{json, Value};

use support::mock_cortex::{MockConnection, MockCortexServer};

fn resilient_test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.reconnect.enabled = true;
    config.reconnect.base_delay_secs = 0;
    config.reconnect.max_delay_secs = 0;
    config.reconnect.max_attempts = 2;
    config.health.enabled = false;
    config.timeouts.rpc_timeout_secs = 1;
    config
}

fn rpc_id(request: &Value) -> u64 {
    request
        .get("id")
        .and_then(Value::as_u64)
        .expect("request missing numeric id")
}

async fn start_server_or_skip(test_name: &str) -> Option<MockCortexServer> {
    match MockCortexServer::start().await {
        Ok(server) => Some(server),
        Err(err) => {
            eprintln!("Skipping {test_name}: unable to start mock server: {err}");
            None
        }
    }
}

async fn drive_auth_handshake(connection: &mut MockConnection, token: &str) {
    let info = connection.recv_request_method(Methods::GET_CORTEX_INFO).await;
    connection.send_result(rpc_id(&info), json!({"version": "mock"})).await;

    let request_access = connection.recv_request_method(Methods::REQUEST_ACCESS).await;
    connection
        .send_result(rpc_id(&request_access), json!({"accessGranted": true}))
        .await;

    let authorize = connection.recv_request_method(Methods::AUTHORIZE).await;
    connection
        .send_result(rpc_id(&authorize), json!({"cortexToken": token}))
        .await;
}

#[tokio::test]
async fn auto_reconnect_retries_failed_operation_and_emits_events() {
    let mut server = match start_server_or_skip("auto_reconnect_retries_failed_operation_and_emits_events").await {
        Some(server) => server,
        None => return,
    };
    let config = resilient_test_config(server.ws_url());

    let server_task = tokio::spawn(async move {
        let mut first_connection = server.accept_connection().await;
        assert_eq!(first_connection.index(), 0);
        drive_auth_handshake(&mut first_connection, "token-initial").await;

        let first_query = first_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        assert_eq!(first_query["method"], Methods::QUERY_HEADSETS);
        first_connection.force_close().await;

        let mut second_connection = server.accept_connection().await;
        assert_eq!(second_connection.index(), 1);
        drive_auth_handshake(&mut second_connection, "token-reconnected").await;

        let retried_query = second_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        second_connection
            .send_result(rpc_id(&retried_query), json!([]))
            .await;
    });

    let client = ResilientClient::connect(config).await.unwrap();
    let mut events = client.event_receiver();

    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();
    assert!(headsets.is_empty());

    let mut saw_disconnected = false;
    let mut saw_reconnecting = false;
    let mut saw_reconnected = false;

    for _ in 0..40 {
        if saw_disconnected && saw_reconnecting && saw_reconnected {
            break;
        }

        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(150), events.recv()).await
        {
            match event {
                ConnectionEvent::Disconnected { .. } => saw_disconnected = true,
                ConnectionEvent::Reconnecting { .. } => saw_reconnecting = true,
                ConnectionEvent::Reconnected => saw_reconnected = true,
                _ => {}
            }
        }
    }

    assert!(saw_disconnected, "missing Disconnected event");
    assert!(saw_reconnecting, "missing Reconnecting event");
    assert!(saw_reconnected, "missing Reconnected event");

    client.disconnect().await.unwrap();
    server_task.await.unwrap();
}

#[tokio::test]
async fn reconnect_disabled_propagates_connection_error() {
    let mut server = match start_server_or_skip("reconnect_disabled_propagates_connection_error").await {
        Some(server) => server,
        None => return,
    };
    let mut config = resilient_test_config(server.ws_url());
    config.reconnect.enabled = false;

    let server_task = tokio::spawn(async move {
        let mut first_connection = server.accept_connection().await;
        drive_auth_handshake(&mut first_connection, "token-initial").await;

        let first_query = first_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        assert_eq!(first_query["method"], Methods::QUERY_HEADSETS);
        first_connection.force_close().await;

        server
            .try_accept_connection(Duration::from_millis(500))
            .await
            .is_some()
    });

    let client = ResilientClient::connect(config).await.unwrap();
    let err = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap_err();

    assert!(err.is_connection_error());
    let saw_reconnect_attempt = server_task.await.unwrap();
    assert!(
        !saw_reconnect_attempt,
        "reconnect should not be attempted when disabled"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn generate_new_token_updates_resilient_state() {
    let mut server = match start_server_or_skip("generate_new_token_updates_resilient_state").await {
        Some(server) => server,
        None => return,
    };
    let config = resilient_test_config(server.ws_url());

    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_connection().await;
        drive_auth_handshake(&mut connection, "token-initial").await;

        let refresh = connection
            .recv_request_method(Methods::GENERATE_NEW_TOKEN)
            .await;
        connection
            .send_result(rpc_id(&refresh), json!({"cortexToken": "token-updated"}))
            .await;
    });

    let client = ResilientClient::connect(config).await.unwrap();
    let original = client.cortex_token().await;
    assert_eq!(original, "token-initial");

    let refreshed = client.generate_new_token().await.unwrap();
    assert_eq!(refreshed, "token-updated");
    assert_eq!(client.cortex_token().await, "token-updated");

    client.disconnect().await.unwrap();
    server_task.await.unwrap();
}
