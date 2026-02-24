mod support;

use emotiv_cortex_v2::protocol::constants::{Methods, Streams};
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::{CortexClient, CortexConfig, CortexError, streams};
use futures_util::StreamExt;
use serde_json::{Value, json};

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
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
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
    let mut server =
        match start_server_or_skip("authenticate_fallback_request_access_method_not_found").await {
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
        connection
            .send_result(rpc_id(&request), json!({"version": "ok"}))
            .await;

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
    let mut server =
        match start_server_or_skip("authenticate_fails_when_authorize_method_not_found").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_result(rpc_id(&request), json!({"version": "ok"}))
            .await;

        let request = connection
            .recv_request_method(Methods::REQUEST_ACCESS)
            .await;
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
    let mut server =
        match start_server_or_skip("rpc_timeout_is_reported_and_next_call_still_works").await {
            Some(server) => server,
            None => return,
        };
    let mut config = test_config(server.ws_url());
    config.timeouts.rpc_timeout_secs = 1;
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let first_request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        let second_request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_result(rpc_id(&second_request), json!({"ok": true}))
            .await;
        (first_request, second_request)
    });

    let timeout_err = client.get_cortex_info().await.unwrap_err();
    assert_eq!(client.pending_response_count().await, 0);
    let second = client.get_cortex_info().await.unwrap();
    let _ = responder.await.unwrap();

    assert!(matches!(timeout_err, CortexError::Timeout { seconds: 1 }));
    assert_eq!(second["ok"], true);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn send_failure_cleans_pending_response_entry() {
    let mut server = match start_server_or_skip("send_failure_cleans_pending_response_entry").await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let _connection = server.accept_connection().await;
    client.disconnect().await.unwrap();

    let err = client.get_cortex_info().await.unwrap_err();
    assert!(matches!(err, CortexError::WebSocket(_)));
    assert_eq!(client.pending_response_count().await, 0);
}

#[tokio::test]
async fn stop_reader_finishes_without_polling_delay() {
    let mut server = match start_server_or_skip("stop_reader_finishes_without_polling_delay").await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();
    let _connection = server.accept_connection().await;

    let start = std::time::Instant::now();
    client.stop_reader().await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(150),
        "reader stop took {elapsed:?}"
    );
    assert!(!client.is_connected());

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn stream_dispatch_stats_track_overflow_drops() {
    let mut server = match start_server_or_skip("stream_dispatch_stats_track_overflow_drops").await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let connection = server.accept_connection().await;
    let _receivers = client.create_stream_channels(&[Streams::EEG]);

    let pusher = tokio::spawn(async move {
        let event = json!({
            "sid": "session-1",
            "time": 1609459200.0,
            "eeg": [1, 0, 1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0, []]
        });
        for _ in 0..(1024 + 256) {
            connection.push_event(event.clone()).await;
        }
    });
    pusher.await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let stats = client.stream_dispatch_stats();
    let eeg = stats.get("eeg").copied().unwrap_or_default();
    assert!(eeg.delivered > 0);
    assert!(eeg.dropped_full > 0);
    assert_eq!(eeg.dropped_closed, 0);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_eeg_routes_stream_event_to_typed_stream() {
    let mut server =
        match start_server_or_skip("subscribe_eeg_routes_stream_event_to_typed_stream").await {
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
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), eeg_stream.next())
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
        let request = connection
            .recv_request_method(Methods::CONTROL_DEVICE)
            .await;
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
    let mut server =
        match start_server_or_skip("query_headsets_options_round_trip_over_transport").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
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

// ─── Error-path tests: protocol and API error mapping ───────────────────────

#[tokio::test]
async fn rpc_response_null_result_no_error_yields_protocol_error() {
    let mut server =
        match start_server_or_skip("rpc_response_null_result_no_error_yields_protocol_error").await
        {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_json(json!({"jsonrpc": "2.0", "id": rpc_id(&request)}))
            .await;
        request
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::ProtocolError { .. }));
    assert!(
        err.to_string().contains("no result or error"),
        "expected protocol error about missing result/error, got: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn rpc_response_malformed_json_yields_protocol_error() {
    let mut server =
        match start_server_or_skip("rpc_response_malformed_json_yields_protocol_error").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        let id = rpc_id(&request);
        // Valid numeric id so the client routes to the pending request; "error" as string
        // so CortexResponse deserialization fails (expects { code, message }).
        connection
            .send_json(json!({"jsonrpc": "2.0", "id": id, "error": "not an object"}))
            .await;
        request
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::ProtocolError { .. }));
    assert!(
        err.to_string().contains("parse") || err.to_string().contains("Protocol"),
        "expected protocol/parse error, got: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn get_user_login_result_wrong_type_yields_protocol_error() {
    let mut server = match start_server_or_skip(
        "get_user_login_result_wrong_type_yields_protocol_error",
    )
    .await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_USER_LOGIN)
            .await;
        // Result must be an array of UserLoginInfo; a number is invalid.
        connection.send_result(rpc_id(&request), json!(123)).await;
        request
    });

    let err = client.get_user_login().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::ProtocolError { .. }));
    assert!(
        err.to_string().contains("parse")
            || err.to_string().contains("Protocol")
            || err.to_string().contains("user login"),
        "expected protocol/parse error for wrong result type, got: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_token_expired_maps_to_token_expired() {
    let mut server =
        match start_server_or_skip("api_error_token_expired_maps_to_token_expired").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32015, "cortex token expired")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::TokenExpired));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_session_error_maps_and_preserves_message() {
    let mut server =
        match start_server_or_skip("api_error_session_error_maps_and_preserves_message").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(
                rpc_id(&request),
                -32005,
                "session already exists for this headset",
            )
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::SessionError { reason } => assert!(reason.contains("session already exists")),
        _ => panic!("expected SessionError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_stream_error_maps_and_preserves_message() {
    let mut server =
        match start_server_or_skip("api_error_stream_error_maps_and_preserves_message").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32016, "invalid stream name")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::StreamError { reason } => assert!(reason.contains("invalid stream")),
        _ => panic!("expected StreamError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_user_not_logged_in_maps_correctly() {
    let mut server = match start_server_or_skip("api_error_user_not_logged_in_maps_correctly").await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32033, "user not logged in to emotiv id")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::UserNotLoggedIn));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_not_approved_maps_correctly() {
    let mut server = match start_server_or_skip("api_error_not_approved_maps_correctly").await {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32142, "application not approved")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::NotApproved));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_license_error_maps_and_preserves_message() {
    let mut server =
        match start_server_or_skip("api_error_license_error_maps_and_preserves_message").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32024, "license expired")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::LicenseError { reason } => assert!(reason.contains("license expired")),
        _ => panic!("expected LicenseError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_headset_error_maps_and_preserves_message() {
    let mut server =
        match start_server_or_skip("api_error_headset_error_maps_and_preserves_message").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32152, "headset not ready")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::HeadsetError { reason } => assert!(reason.contains("headset not ready")),
        _ => panic!("expected HeadsetError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_method_not_found_includes_method_name() {
    let mut server =
        match start_server_or_skip("api_error_method_not_found_includes_method_name").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32601, "getCortexInfo")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::MethodNotFound { method } => assert_eq!(method, "getCortexInfo"),
        _ => panic!("expected MethodNotFound with method name, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}
