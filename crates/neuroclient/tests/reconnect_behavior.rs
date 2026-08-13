mod support;

use std::sync::Arc;
use std::time::Duration;

use neuroclient::CortexConfig;
use neuroclient::protocol::constants::Methods;
use neuroclient::protocol::headset::QueryHeadsetsOptions;
use neuroclient::reconnect::{ConnectionEvent, ResilientClient};
use serde_json::{Value, json};

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
    let info = connection
        .recv_request_method(Methods::GET_CORTEX_INFO)
        .await;
    connection
        .send_result(rpc_id(&info), json!({"version": "mock"}))
        .await;

    let request_access = connection
        .recv_request_method(Methods::REQUEST_ACCESS)
        .await;
    connection
        .send_result(rpc_id(&request_access), json!({"accessGranted": true}))
        .await;

    let authorize = connection.recv_request_method(Methods::AUTHORIZE).await;
    connection
        .send_result(rpc_id(&authorize), json!({"cortexToken": token}))
        .await;
}

async fn drive_auth_handshake_with_warning(
    connection: &mut MockConnection,
    token: &str,
    warning: Value,
) {
    let info = connection
        .recv_request_method(Methods::GET_CORTEX_INFO)
        .await;
    connection.push_event(warning).await;
    connection
        .send_result(rpc_id(&info), json!({"version": "mock"}))
        .await;

    let request_access = connection
        .recv_request_method(Methods::REQUEST_ACCESS)
        .await;
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
    let Some(mut server) =
        start_server_or_skip("auto_reconnect_retries_failed_operation_and_emits_events").await
    else {
        return;
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
async fn reconnect_replaces_and_shuts_down_old_client() {
    let Some(mut server) =
        start_server_or_skip("reconnect_replaces_and_shuts_down_old_client").await
    else {
        return;
    };
    let config = resilient_test_config(server.ws_url());

    let server_task = tokio::spawn(async move {
        let mut first_connection = server.accept_connection().await;
        drive_auth_handshake(&mut first_connection, "token-initial").await;

        let first_query = first_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        assert_eq!(first_query["method"], Methods::QUERY_HEADSETS);
        first_connection.force_close().await;

        let mut second_connection = server.accept_connection().await;
        drive_auth_handshake(&mut second_connection, "token-reconnected").await;

        let retried_query = second_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        second_connection
            .send_result(rpc_id(&retried_query), json!([]))
            .await;
    });

    let client = ResilientClient::connect(config).await.unwrap();
    let old_inner = client.inner_client().await;

    // Force-closed first connection makes this call fail and reconnect.
    client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();
    server_task.await.unwrap();

    let new_inner = client.inner_client().await;
    assert!(
        !std::sync::Arc::ptr_eq(&old_inner, &new_inner),
        "reconnect must install a new client"
    );
    // Even with an external Arc still alive, the replaced client must be
    // fully stopped (reader task exited).
    assert!(
        !old_inner.is_connected(),
        "old client must be shut down after reconnect"
    );
    assert!(new_inner.is_connected());

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn concurrent_failures_install_only_one_reconnected_client() {
    let Some(mut server) =
        start_server_or_skip("concurrent_failures_install_only_one_reconnected_client").await
    else {
        return;
    };
    let config = resilient_test_config(server.ws_url());

    let server_task = tokio::spawn(async move {
        let mut first_connection = server.accept_connection().await;
        drive_auth_handshake(&mut first_connection, "token-initial").await;

        // Both requests must be in flight on A before it is closed.
        let _first = first_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        let _second = first_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        first_connection.force_close().await;

        let mut second_connection = server.accept_connection().await;
        drive_auth_handshake(&mut second_connection, "token-reconnected").await;
        for _ in 0..2 {
            let retry = second_connection
                .recv_request_method(Methods::QUERY_HEADSETS)
                .await;
            second_connection
                .send_result(rpc_id(&retry), json!([]))
                .await;
        }

        server
            .try_accept_connection(Duration::from_millis(300))
            .await
            .is_some()
    });

    let client = Arc::new(ResilientClient::connect(config).await.unwrap());
    let first = tokio::spawn({
        let client = Arc::clone(&client);
        async move { client.query_headsets(QueryHeadsetsOptions::default()).await }
    });
    let second = tokio::spawn({
        let client = Arc::clone(&client);
        async move { client.query_headsets(QueryHeadsetsOptions::default()).await }
    });

    assert!(first.await.unwrap().unwrap().is_empty());
    assert!(second.await.unwrap().unwrap().is_empty());
    assert!(
        !server_task.await.unwrap(),
        "concurrent failures must not create a third client generation"
    );

    let client = Arc::try_unwrap(client).unwrap_or_else(|_| {
        panic!("operation tasks must release their ResilientClient references")
    });
    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn resilient_disconnect_closes_active_socket() {
    let Some(mut server) = start_server_or_skip("resilient_disconnect_closes_active_socket").await
    else {
        return;
    };
    let config = resilient_test_config(server.ws_url());

    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_connection().await;
        drive_auth_handshake(&mut connection, "token-initial").await;
        // The graceful disconnect must actually close the WebSocket.
        connection.wait_for_close().await;
    });

    let client = ResilientClient::connect(config).await.unwrap();
    let inner = client.inner_client().await;
    assert!(inner.is_connected());

    client.disconnect().await.unwrap();
    assert!(
        !inner.is_connected(),
        "graceful disconnect must stop the reader loop"
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn reconnect_disabled_propagates_connection_error() {
    let Some(mut server) =
        start_server_or_skip("reconnect_disabled_propagates_connection_error").await
    else {
        return;
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
async fn warning_relay_survives_reconnect() {
    let Some(mut server) = start_server_or_skip("warning_relay_survives_reconnect").await else {
        return;
    };
    let config = resilient_test_config(server.ws_url());

    let server_task = tokio::spawn(async move {
        let mut first_connection = server.accept_connection().await;
        drive_auth_handshake(&mut first_connection, "token-initial").await;

        // Warning on the first connection.
        first_connection
            .push_event(json!({
                "warning": {"code": 2, "message": "User logged in"}
            }))
            .await;

        let first_query = first_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        assert_eq!(first_query["method"], Methods::QUERY_HEADSETS);
        first_connection.force_close().await;

        let mut second_connection = server.accept_connection().await;
        // The relay subscribes before authentication so this warning is
        // buffered until the new client is conditionally installed.
        drive_auth_handshake_with_warning(
            &mut second_connection,
            "token-reconnected",
            json!({"warning": {"code": 9, "message": "Application approved"}}),
        )
        .await;

        let retried_query = second_connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        second_connection
            .send_result(rpc_id(&retried_query), json!([]))
            .await;

        // Warning on the *new* connection must reach the same receiver.
        second_connection
            .push_event(json!({
                "warning": {"code": 3, "message": "User logged out"}
            }))
            .await;
    });

    let client = ResilientClient::connect(config).await.unwrap();
    let mut warnings = client.warning_receiver();

    let first = tokio::time::timeout(Duration::from_secs(3), warnings.recv())
        .await
        .expect("timed out waiting for pre-reconnect warning")
        .unwrap();
    assert_eq!(first.code, 2);

    // Trigger the reconnect.
    client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();

    let second = tokio::time::timeout(Duration::from_secs(3), warnings.recv())
        .await
        .expect("timed out waiting for authentication-phase warning")
        .unwrap();
    assert_eq!(second.code, 9);

    let third = tokio::time::timeout(Duration::from_secs(3), warnings.recv())
        .await
        .expect("timed out waiting for post-reconnect warning")
        .unwrap();
    assert_eq!(third.code, 3);

    server_task.await.unwrap();
    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn generate_new_token_updates_resilient_state() {
    let Some(mut server) = start_server_or_skip("generate_new_token_updates_resilient_state").await
    else {
        return;
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
