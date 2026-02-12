mod support;

use emotiv_cortex_v2::protocol::{Methods, QueryHeadsetsOptions, Streams};
use emotiv_cortex_v2::{streams, CortexClient, CortexConfig, CortexError};
use futures::StreamExt;
use serde_json::{json, Value};

use support::mock_cortex::MockCortexServer;

fn test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
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

#[tokio::test]
async fn connect_and_get_cortex_info_round_trip() {
    let mut server = match start_server_or_skip("connect_and_get_cortex_info_round_trip").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::GET_CORTEX_INFO).await;
        connection
            .send_result(rpc_id(&request), json!({"version": "mock-1.0.0"}))
            .await;
        request
    });

    let info = client.get_cortex_info().await.unwrap();
    let request = responder.await.unwrap();

    assert_eq!(info["version"], "mock-1.0.0");
    assert_eq!(request["jsonrpc"], "2.0");
    assert!(request.get("params").is_none());

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn authenticate_fallback_request_access_method_not_found() {
    let mut server = match start_server_or_skip("authenticate_fallback_request_access_method_not_found").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let mut methods = Vec::new();

        let request = connection.recv_request().await;
        methods.push(request["method"].as_str().unwrap().to_string());
        connection.send_result(rpc_id(&request), json!({"version": "ok"})).await;

        let request = connection.recv_request().await;
        methods.push(request["method"].as_str().unwrap().to_string());
        connection
            .send_error(rpc_id(&request), -32601, "requestAccess")
            .await;

        let request = connection.recv_request().await;
        methods.push(request["method"].as_str().unwrap().to_string());
        connection
            .send_result(rpc_id(&request), json!({"cortexToken": "token-fallback"}))
            .await;

        methods
    });

    let token = client
        .authenticate("test-client-id", "test-client-secret")
        .await
        .unwrap();
    let methods = responder.await.unwrap();

    assert_eq!(token, "token-fallback");
    assert_eq!(
        methods,
        vec![
            Methods::GET_CORTEX_INFO,
            Methods::REQUEST_ACCESS,
            Methods::AUTHORIZE,
        ]
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn authenticate_fails_when_authorize_method_not_found() {
    let mut server = match start_server_or_skip("authenticate_fails_when_authorize_method_not_found").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::GET_CORTEX_INFO).await;
        connection.send_result(rpc_id(&request), json!({"version": "ok"})).await;

        let request = connection.recv_request_method(Methods::REQUEST_ACCESS).await;
        connection.send_result(rpc_id(&request), json!({})).await;

        let request = connection.recv_request_method(Methods::AUTHORIZE).await;
        connection
            .send_error(rpc_id(&request), -32601, "authorize")
            .await;
    });

    let err = client
        .authenticate("test-client-id", "test-client-secret")
        .await
        .unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::AuthenticationFailed { .. }));
    assert!(
        err.to_string().contains("authorize"),
        "expected authorize detail in error: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn rpc_timeout_is_reported_and_next_call_still_works() {
    let mut server = match start_server_or_skip("rpc_timeout_is_reported_and_next_call_still_works").await {
        Some(server) => server,
        None => return,
    };
    let mut config = test_config(server.ws_url());
    config.timeouts.rpc_timeout_secs = 1;
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let first_request = connection.recv_request_method(Methods::GET_CORTEX_INFO).await;
        let second_request = connection.recv_request_method(Methods::GET_CORTEX_INFO).await;
        connection
            .send_result(rpc_id(&second_request), json!({"ok": true}))
            .await;
        (first_request, second_request)
    });

    let timeout_err = client.get_cortex_info().await.unwrap_err();
    let second = client.get_cortex_info().await.unwrap();
    let _ = responder.await.unwrap();

    assert!(matches!(timeout_err, CortexError::Timeout { seconds: 1 }));
    assert_eq!(second["ok"], true);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_eeg_routes_stream_event_to_typed_stream() {
    let mut server = match start_server_or_skip("subscribe_eeg_routes_stream_event_to_typed_stream").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(rpc_id(&request), json!({"success": [Streams::EEG]}))
            .await;
        connection
            .push_event(json!({
                "sid": "session-1",
                "time": 1609459200.0,
                "eeg": [29, 0, 4262.564, 4264.615, 4265.128, 4267.179, 4263.59, 0.0, 0, []]
            }))
            .await;
    });

    let mut eeg_stream = streams::subscribe_eeg(&client, "token", "session-1", 5)
        .await
        .unwrap();
    let sample = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        eeg_stream.next(),
    )
    .await
    .expect("timed out waiting for eeg sample")
    .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.counter, 29);
    assert_eq!(sample.channels.len(), 5);
    assert!(!sample.interpolated);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_code_maps_to_domain_error() {
    let mut server = match start_server_or_skip("api_error_code_maps_to_domain_error").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::CONTROL_DEVICE).await;
        connection
            .send_error(rpc_id(&request), -32001, "no headset connected")
            .await;
    });

    let err = client.connect_headset("HS-1").await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::NoHeadsetFound));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn query_headsets_options_round_trip_over_transport() {
    let mut server = match start_server_or_skip("query_headsets_options_round_trip_over_transport").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::QUERY_HEADSETS).await;
        connection.send_result(rpc_id(&request), json!([])).await;
        request
    });

    let _ = client
        .query_headsets(QueryHeadsetsOptions {
            id: Some("HS-123".to_string()),
            include_flex_mappings: true,
        })
        .await
        .unwrap();

    let request = responder.await.unwrap();
    assert_eq!(request["params"]["id"], "HS-123");
    assert_eq!(request["params"]["includeFlexMappings"], true);

    client.disconnect().await.unwrap();
}
